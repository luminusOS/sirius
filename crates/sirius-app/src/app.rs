//! Root wizard component. Owns the window and coordinates the wizard.

mod bootstrap;
mod install_task;
mod pages;
mod state;

use self::pages::PageControllers;
use self::state::{StateEffect, WizardState};
use crate::pages::PageOutput;
use crate::pages::progress::ProgressMsg;
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, adw, gtk};

pub struct AppModel {
    state: WizardState,
    pages: PageControllers,
    carousel: Option<adw::Carousel>,
    page_widgets: std::collections::HashMap<String, gtk::Widget>,
    window: Option<adw::ApplicationWindow>,
    /// Command line for the terminal launcher (button + Ctrl+Shift+P).
    terminal_command: String,
}

#[derive(Debug)]
pub enum AppMsg {
    Page(PageOutput),
    Next,
    Back,
    OpenTerminal,
    /// User confirmed the erase-and-install dialog: advance past summary and start.
    ConfirmInstall,
    StartInstall,
    Progress(sirius_core::Progress),
}

#[relm4::component(pub)]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
        adw::ApplicationWindow {
            set_title: Some("Sirius"),
            set_default_width: 960,
            set_default_height: 640,

            #[wrap(Some)]
            set_content = &adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    set_show_end_title_buttons: false,

                    #[name = "terminal_button"]
                    pack_start = &gtk::Button {
                        set_icon_name: "utilities-terminal-symbolic",
                        add_css_class: "flat",
                        set_visible: false,
                        #[watch]
                        set_tooltip_text: Some(gettext("Open terminal").as_str()),
                        connect_clicked => AppMsg::OpenTerminal,
                    },

                    #[name = "dots"]
                    #[wrap(Some)]
                    set_title_widget = &adw::CarouselIndicatorDots {
                    },
                },

                #[wrap(Some)]
                set_content = &gtk::Overlay {
                    #[name = "carousel"]
                    #[wrap(Some)]
                    set_child = &adw::Carousel {
                        set_vexpand: true,
                        set_interactive: false,
                        set_allow_scroll_wheel: false,
                        set_allow_mouse_drag: false,
                        set_allow_long_swipes: false,
                    },

                    add_overlay = &gtk::Button {
                        set_icon_name: "go-previous-symbolic",
                        add_css_class: "navigation-arrow",
                        set_halign: gtk::Align::Start,
                        set_valign: gtk::Align::Center,
                        set_margin_start: 20,
                        #[watch]
                        set_visible: !model.state.is_first() && !model.state.install_started(),
                        connect_clicked => AppMsg::Back,
                    },

                    add_overlay = &gtk::Button {
                        set_icon_name: "go-next-symbolic",
                        add_css_class: "navigation-arrow",
                        add_css_class: "suggested-action",
                        set_halign: gtk::Align::End,
                        set_valign: gtk::Align::Center,
                        set_margin_end: 20,
                        #[watch]
                        set_visible: !matches!(
                            model.state.current_page(),
                            "welcome" | "summary" | "progress" | "finished"
                        ),
                        #[watch]
                        set_sensitive: model.state.can_proceed(),
                        connect_clicked => AppMsg::Next,
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        crate::style::load();
        let bootstrap = bootstrap::load();
        let pages_order = bootstrap.page_ids.clone();

        let page_controllers = PageControllers::launch(
            &sender,
            pages_order.clone(),
            bootstrap.diagnostics,
            bootstrap.bentos,
            bootstrap.branding,
        );

        let state = WizardState::new(
            bootstrap.page_ids,
            bootstrap.diagnostics_blocked,
            bootstrap.uefi,
        );
        let mut model = AppModel {
            state,
            pages: page_controllers,
            carousel: None,
            page_widgets: std::collections::HashMap::new(),
            window: None,
            terminal_command: bootstrap.terminal.command.clone(),
        };

        let widgets = view_output!();
        widgets
            .terminal_button
            .set_visible(bootstrap.terminal.show_button);

        // Ctrl+Shift+P opens the configured terminal even with the
        // header-bar button hidden (the default).
        {
            let command = bootstrap.terminal.command.clone();
            let keys = gtk::EventControllerKey::new();
            keys.connect_key_pressed(move |_, key, _, mods| {
                let wanted = matches!(key, gtk::gdk::Key::P | gtk::gdk::Key::p)
                    && mods.contains(
                        gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::SHIFT_MASK,
                    );
                if wanted {
                    open_terminal(&command);
                    return gtk::glib::Propagation::Stop;
                }
                gtk::glib::Propagation::Proceed
            });
            root.add_controller(keys);
        }

        for id in &pages_order {
            if let Some(w) = model.pages.widget(id) {
                // Force each page to fill the carousel viewport. Without this an
                // AdwCarousel renders pages at their natural width, centered, and
                // the neighbouring page peeks in from the side.
                w.set_hexpand(true);
                w.set_vexpand(true);
                w.set_halign(gtk::Align::Fill);
                w.set_valign(gtk::Align::Fill);
                // Keep page content clear of the overlay navigation arrows and
                // visually centered at every step of the carousel. The progress
                // page shows neither arrow (Back hides once the install starts,
                // Next is hidden on it) and the welcome page's banner runs
                // edge-to-edge, so the margins would be dead space on both.
                let side_margin = if matches!(id.as_str(), "progress" | "welcome") {
                    0
                } else {
                    72
                };
                w.set_margin_start(side_margin);
                w.set_margin_end(side_margin);
                widgets.carousel.append(&w);
                model.page_widgets.insert(id.clone(), w.clone());
            }
        }
        model.carousel = Some(widgets.carousel.clone());
        model.window = Some(root.clone());
        widgets.dots.set_carousel(Some(&widgets.carousel));

        // Dev aid: SIRIUS_START_PAGE=<page id> opens the wizard directly on
        // that page (e.g. `SIRIUS_START_PAGE=progress` to iterate on the
        // progress UI without running an install).
        if let Ok(start) = std::env::var("SIRIUS_START_PAGE") {
            model.state.seek(&start);
            if let Some(w) = model.page_widgets.get(model.state.current_page()).cloned() {
                // The carousel silently drops scroll_to until it has a frame
                // clock and an allocation, which can happen well after init.
                // Retry on a short timer until the position actually lands.
                let carousel = widgets.carousel.clone();
                let target: f64 = (0..carousel.n_pages())
                    .position(|i| carousel.nth_page(i) == w)
                    .unwrap_or(0) as f64;
                gtk::glib::timeout_add_local(std::time::Duration::from_millis(250), move || {
                    if (carousel.position() - target).abs() < 0.5 {
                        gtk::glib::ControlFlow::Break
                    } else {
                        carousel.scroll_to(&w, false);
                        gtk::glib::ControlFlow::Continue
                    }
                });
            }
            if start == "progress" {
                // Animate the bar as if an install had just started.
                model.pages.progress(ProgressMsg::Start);
            }
        }

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            AppMsg::Page(PageOutput::RequestInstall) => self.confirm_install(&sender),
            AppMsg::Page(out) => self.apply_page_output(out),
            AppMsg::OpenTerminal => open_terminal(&self.terminal_command),
            AppMsg::Next => {
                // Leaving the summary erases the disk: require explicit confirmation.
                if self.state.current_page() == "summary" {
                    self.confirm_install(&sender);
                    return;
                }
                self.state.next();
                self.page_changed();
            }
            AppMsg::ConfirmInstall => {
                self.state.next();
                self.page_changed();
                if self.state.current_page() == "progress" {
                    sender.input(AppMsg::StartInstall);
                }
            }
            AppMsg::Back => {
                self.state.back();
                self.page_changed();
            }
            AppMsg::StartInstall => {
                self.pages.progress(ProgressMsg::Start);
                let progress_sender = sender.clone();
                if let Err(error) = install_task::start(self.state.config(), move |progress| {
                    progress_sender.input(AppMsg::Progress(progress));
                }) {
                    self.pages.progress(ProgressMsg::Failed {
                        message: format!("cannot start install: {error}"),
                    });
                }
            }
            AppMsg::Progress(p) => {
                use sirius_core::Progress;
                match p {
                    Progress::Step { fraction, message } => {
                        self.pages.progress(ProgressMsg::Update {
                            fraction,
                            line: message,
                        });
                    }
                    Progress::Log { line } => {
                        self.pages.progress(ProgressMsg::Line { line });
                    }
                    Progress::Finished => {
                        // Progress page's Done emits RequestNext, which advances the navigator to "finished".
                        self.pages.progress(ProgressMsg::Done);
                    }
                    Progress::Error { message } => {
                        self.pages.progress(ProgressMsg::Failed { message });
                    }
                }
            }
        }
    }
}

/// Launch the configured terminal command (program plus arguments, split on
/// whitespace — no shell quoting).
fn open_terminal(command: &str) {
    let mut parts = command.split_whitespace();
    let Some(program) = parts.next() else {
        return;
    };
    if let Err(err) = std::process::Command::new(program).args(parts).spawn() {
        tracing::error!(?err, command, "failed to launch the configured terminal");
    }
}

impl AppModel {
    /// Modal "this will erase the disk" gate before leaving the summary page.
    fn confirm_install(&self, sender: &ComponentSender<Self>) {
        let config = self.state.config();
        let mut body = if matches!(config.install_type, Some(sirius_core::InstallType::Manual)) {
            gettext(
                "The staged partition changes will now be written to disk and Sirius will be installed. Formatted or deleted data cannot be recovered.",
            )
        } else {
            gettext(
                "All data on the selected disk will be permanently erased and the system will be installed. This cannot be undone.",
            )
        };
        if let Some(disk) = &config.destination_disk {
            let disk = config
                .destination_disk_name
                .as_deref()
                .filter(|name| !name.trim().is_empty())
                .unwrap_or(disk);
            body.push_str(&format!("\n\n{}: {disk}", gettext("Disk")));
        }

        let dialog = adw::AlertDialog::builder()
            .heading(gettext("Confirm installation"))
            .body(body)
            .build();
        dialog.add_response("cancel", &gettext("Cancel"));
        dialog.add_response("install", &gettext("Erase disk and install"));
        dialog.set_response_appearance("install", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");

        let s = sender.clone();
        dialog.connect_response(Some("install"), move |_, _| {
            s.input(AppMsg::ConfirmInstall);
        });
        dialog.present(self.window.as_ref());
    }

    fn scroll_to_current(&self) {
        if let (Some(carousel), Some(widget)) = (
            &self.carousel,
            self.page_widgets.get(self.state.current_page()),
        ) {
            carousel.scroll_to(widget, true);
        }
    }

    fn apply_page_output(&mut self, out: PageOutput) {
        match self.state.apply(out) {
            StateEffect::None => {}
            StateEffect::LanguageChanged => self.pages.retranslate(),
            StateEffect::PageChanged => self.page_changed(),
            StateEffect::InstallRequested => {
                unreachable!("handled by AppModel::update")
            }
        }
    }

    fn page_changed(&self) {
        self.scroll_to_current();
        if self.state.current_page() == "summary" {
            self.pages.show_summary(self.state.config().clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Interactive: the overlay Back/Next arrows must keep their icons
    // centered inside the 44px circle.
    #[test]
    fn navigation_arrow_icons_stay_centered() {
        if std::env::var_os("WAYLAND_DISPLAY").is_none() && std::env::var_os("DISPLAY").is_none() {
            eprintln!("skipping interactive test: no display available");
            return;
        }
        crate::pages::testutil::run_on_gtk_thread(navigation_arrows_interactive);
    }

    fn navigation_arrows_interactive() {
        crate::style::load();

        let back = gtk::Button::from_icon_name("go-previous-symbolic");
        back.add_css_class("navigation-arrow");
        back.set_halign(gtk::Align::Start);
        back.set_valign(gtk::Align::Center);
        let next = gtk::Button::from_icon_name("go-next-symbolic");
        next.add_css_class("navigation-arrow");
        next.add_css_class("suggested-action");
        next.set_halign(gtk::Align::End);
        next.set_valign(gtk::Align::Center);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.append(&back);
        row.append(&next);
        let window = adw::Window::new();
        window.set_content(Some(&row));
        window.present();

        let context = gtk::glib::MainContext::default();
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(800);
        while std::time::Instant::now() < deadline {
            while context.pending() {
                context.iteration(false);
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        for button in [&back, &next] {
            assert_eq!(
                (button.width(), button.height()),
                (44, 44),
                "navigation-arrow must render as a 44px circle"
            );
            let image = button.first_child().expect("icon child");
            let bounds = image
                .compute_bounds(button)
                .expect("icon bounds inside the button");
            let center_x = f64::from(bounds.x()) + f64::from(bounds.width()) / 2.0;
            let center_y = f64::from(bounds.y()) + f64::from(bounds.height()) / 2.0;
            assert!(
                (center_x - 22.0).abs() < 1.5 && (center_y - 22.0).abs() < 1.5,
                "icon must be centered in the 44px button, got center ({center_x:.1}, {center_y:.1})"
            );
        }
        window.close();
    }
}
