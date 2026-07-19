//! Install progress page: distro link cards ("bentos", as in readymade) above
//! the progress bar, with a log-toggle button that reveals the live install log
//! above the bottom controls.

mod bento;

use super::PageOutput;
use crate::backend::distro::Bento;
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::{adw, gtk, ComponentParts, ComponentSender, SimpleComponent};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ProgressPhase {
    #[default]
    Idle,
    Running {
        indeterminate: bool,
    },
    Failed,
    Finished,
}

impl ProgressPhase {
    fn is_failed(self) -> bool {
        self == Self::Failed
    }
}

pub struct ProgressPage {
    log: gtk::TextBuffer,
    phase: ProgressPhase,
    has_bentos: bool,
    // Widget handles: the bar is driven imperatively (pulse vs. fraction),
    // and a failure forces the log open.
    bar: Option<gtk::ProgressBar>,
    log_revealer: Option<gtk::Revealer>,
    log_toggle: Option<gtk::ToggleButton>,
}

#[derive(Debug)]
pub enum ProgressMsg {
    /// Install was launched; show immediate indeterminate activity.
    Start,
    /// Timer tick while waiting for determinate progress.
    Pulse,
    /// Sent from the privileged runner's progress stream.
    Update {
        fraction: f64,
        line: String,
    },
    /// A raw runner log line (libreadymade's stderr): log only, pulse the bar.
    Line {
        line: String,
    },
    /// Install failed: switch the page into its error state and show the log.
    Failed {
        message: String,
    },
    Done,
    /// The UI language changed; gettext resolves strings at render time, so a
    /// bare re-render (Relm4 runs update_view after update) is enough.
    Retranslate,
}

#[relm4::component(pub)]
impl SimpleComponent for ProgressPage {
    type Init = Vec<Bento>;
    type Input = ProgressMsg;
    type Output = PageOutput;

    view! {
        // Root is a plain vertical box so the progress-bar row lives OUTSIDE
        // the StatusPage (which scrolls internally and can push anything
        // inside it below the fold). The bar is pinned to the page bottom.
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,

            adw::StatusPage {
                set_vexpand: true,
                #[watch]
                set_title: if model.phase.is_failed() {
                    gettext("Installation failed")
                } else {
                    gettext("Installing the system")
                }
                .as_str(),
                #[watch]
                set_description: model
                    .phase.is_failed()
                    .then(|| {
                        gettext(
                            "Something went wrong during installation. Check the log below for details. No changes were finished — you can reboot and try again.",
                        )
                    })
                    .as_deref(),
                #[watch]
                set_icon_name: model.phase.is_failed().then_some("dialog-error-symbolic"),
                #[wrap(Some)]
                set_child = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 18,

                    // Visible activity while the install runs; hidden on
                    // failure (the error icon takes over) and when done.
                    #[name = "spinner"]
                    gtk::Spinner {
                        set_halign: gtk::Align::Center,
                        #[watch]
                        set_visible: matches!(model.phase, ProgressPhase::Running { .. }),
                    },

                    // Bentos stay in the centered StatusPage content.
                    #[name = "bento_box"]
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 12,
                        set_homogeneous: true,
                        set_valign: gtk::Align::Start,
                        #[watch]
                        set_visible: !model.phase.is_failed() && model.has_bentos,
                    },
                },
            },

            #[name = "log_revealer"]
            gtk::Revealer {
                set_transition_type: gtk::RevealerTransitionType::SlideDown,
                set_margin_start: 24,
                set_margin_end: 24,
                set_margin_bottom: 10,

                // The log scrolls internally within a bounded height.
                gtk::ScrolledWindow {
                    set_min_content_height: 160,
                    set_max_content_height: 280,
                    set_propagate_natural_height: true,
                    add_css_class: "card",
                    #[name = "log_view"]
                    gtk::TextView {
                        set_editable: false,
                        set_cursor_visible: false,
                        set_monospace: true,
                        set_wrap_mode: gtk::WrapMode::WordChar,
                        set_left_margin: 12,
                        set_right_margin: 12,
                        set_top_margin: 12,
                        set_bottom_margin: 12,
                    },
                },
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 8,
                set_margin_start: 24,
                set_margin_end: 24,
                set_margin_bottom: 18,
                // NOTE: never use `set_css_classes` here — it would wipe the
                // widget's own `horizontal` class, which Adwaita's selectors
                // need, leaving the bar unstyled (invisible). The `error`
                // class is added imperatively on failure instead.
                #[name = "bar"]
                gtk::ProgressBar {
                    set_hexpand: true,
                    set_valign: gtk::Align::Center,
                    set_pulse_step: 0.04,
                },
                #[name = "log_toggle"]
                gtk::ToggleButton {
                    set_icon_name: "utilities-terminal-symbolic",
                    set_valign: gtk::Align::Center,
                    add_css_class: "flat",
                    #[watch]
                    set_tooltip_text: Some(gettext("Show install log").as_str()),
                },
            },
        }
    }

    fn init(
        bentos: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let log = gtk::TextBuffer::new(None);
        let mut model = ProgressPage {
            log,
            phase: ProgressPhase::Idle,
            has_bentos: !bentos.is_empty(),
            bar: None,
            log_revealer: None,
            log_toggle: None,
        };
        let widgets = view_output!();

        bento::append_cards(&widgets.bento_box, &bentos);
        // Started once; visibility (driven by the phase) decides whether it
        // actually animates on screen.
        widgets.spinner.start();

        if bentos.is_empty() {
            // No cards above: show the log permanently.
            widgets.log_revealer.set_reveal_child(true);
            widgets.log_toggle.set_visible(false);
        } else {
            let revealer = widgets.log_revealer.clone();
            widgets.log_toggle.connect_toggled(move |t| {
                revealer.set_reveal_child(t.is_active());
            });
        }

        model.bar = Some(widgets.bar.clone());
        model.log_revealer = Some(widgets.log_revealer.clone());
        model.log_toggle = Some(widgets.log_toggle.clone());

        let pulse_sender = _sender.clone();
        gtk::glib::timeout_add_local(std::time::Duration::from_millis(120), move || {
            pulse_sender.input(ProgressMsg::Pulse);
            gtk::glib::ControlFlow::Continue
        });

        widgets.log_view.set_buffer(Some(&model.log));
        // Keep the newest log line in view as output streams in.
        let view = widgets.log_view.clone();
        model.log.connect_changed(move |buf| {
            let mark = buf.create_mark(None, &buf.end_iter(), false);
            view.scroll_to_mark(&mark, 0.0, false, 0.0, 1.0);
            buf.delete_mark(&mark);
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            ProgressMsg::Start => {
                self.phase = ProgressPhase::Running {
                    indeterminate: true,
                };
                self.advance_bar(0.0);
            }
            ProgressMsg::Pulse => {
                if matches!(
                    self.phase,
                    ProgressPhase::Running {
                        indeterminate: true
                    }
                ) {
                    self.advance_bar(0.0);
                }
            }
            ProgressMsg::Update { fraction, line } => {
                tracing::info!("{line}");
                self.append_log(&line);
                // Stage messages carry no fraction (0.0): pulse to show life.
                self.advance_bar(fraction);
            }
            ProgressMsg::Line { line } => {
                self.append_log(&line);
                self.advance_bar(0.0);
            }
            ProgressMsg::Failed { message } => {
                self.phase = ProgressPhase::Failed;
                tracing::error!("{message}");
                self.append_log(&format!("ERROR: {message}"));
                if let Some(bar) = &self.bar {
                    bar.add_css_class("error");
                    bar.set_show_text(false);
                }
                // Bring the log into view so the error is impossible to miss.
                if let Some(toggle) = &self.log_toggle {
                    toggle.set_active(true);
                }
                if let Some(revealer) = &self.log_revealer {
                    revealer.set_reveal_child(true);
                }
            }
            ProgressMsg::Done => {
                self.phase = ProgressPhase::Finished;
                if let Some(bar) = &self.bar {
                    bar.set_fraction(1.0);
                    bar.set_show_text(true);
                    bar.set_text(Some("100%"));
                }
                self.append_log("Done.");
                sender.output(PageOutput::RequestNext).ok();
            }
            ProgressMsg::Retranslate => {}
        }
    }
}

impl ProgressPage {
    fn append_log(&self, line: &str) {
        let mut end = self.log.end_iter();
        self.log.insert(&mut end, &format!("{line}\n"));
    }

    /// Real fractions (post-install modules) drive the bar and show a
    /// percentage; everything else pulses it so there is always visible
    /// motion while work streams in.
    fn advance_bar(&mut self, fraction: f64) {
        let Some(bar) = &self.bar else { return };
        if matches!(self.phase, ProgressPhase::Failed | ProgressPhase::Finished) {
            return;
        }
        if fraction > 0.0 {
            self.phase = ProgressPhase::Running {
                indeterminate: false,
            };
            let clamped = fraction.clamp(0.0, 1.0);
            bar.set_fraction(clamped);
            bar.set_show_text(true);
            bar.set_text(Some(&format!("{:.0}%", clamped * 100.0)));
        } else {
            self.phase = ProgressPhase::Running {
                indeterminate: true,
            };
            // Pulsing with a stale percentage painted on the bar reads as a
            // stuck install — text only while genuinely determinate.
            bar.set_show_text(false);
            bar.pulse();
        }
    }
}
