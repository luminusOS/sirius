//! Root wizard component. Owns the window, the navigation stack, and InstallConfig.

mod pages;
mod state;

use self::pages::{IMPLEMENTED_PAGES, PageControllers};
use self::state::{StateEffect, WizardState};
use crate::pages::PageOutput;
use crate::pages::diagnostics::DiagnosticsInit;
use crate::pages::progress::ProgressMsg;
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, adw, gtk};
use sirius_diag::config::CONFIG_PATH;
use sirius_diag::{SiriusConfig, SystemFacts, is_blocked, run_all_checks_with_config};
use std::path::Path;

pub struct AppModel {
    state: WizardState,
    pages: PageControllers,
    carousel: Option<adw::Carousel>,
    page_widgets: std::collections::HashMap<String, gtk::Widget>,
    window: Option<adw::ApplicationWindow>,
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
    Progress(crate::backend::Progress),
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
                    pack_start = &gtk::Button {
                        set_icon_name: "utilities-terminal-symbolic",
                        add_css_class: "flat",
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
                        set_visible: !matches!(model.state.current_page(), "summary" | "progress" | "finished"),
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
        let (cfg, warning) = SiriusConfig::load_or_default(Path::new(CONFIG_PATH));
        if let Some(w) = warning {
            tracing::warn!("{w}");
        }
        let has_wifi = crate::backend::network::has_wifi_device();
        let pages: Vec<String> = cfg
            .pages
            .resolve()
            .into_iter()
            .filter(|p| IMPLEMENTED_PAGES.contains(&p.as_str()))
            .filter(|p| p != "network" || has_wifi)
            .collect();
        let pages_order = pages.clone();
        let diag_config = cfg.diagnostics.clone();

        let diagnostics_blocked = {
            let facts = SystemFacts::gather();
            let checks = run_all_checks_with_config(&facts, &cfg.diagnostics);
            is_blocked(&checks, &cfg.diagnostics.require)
        };

        // Distro branding + link cards; absence is fine (star icon, no cards).
        let (bentos, branding) = crate::backend::distro::DistroDescriptor::load()
            .map(|d| (d.bentos, d.branding))
            .unwrap_or_default();

        let page_controllers = PageControllers::launch(
            &sender,
            pages_order.clone(),
            DiagnosticsInit {
                config: diag_config,
            },
            bentos,
            branding,
        );

        let state = WizardState::new(
            pages,
            diagnostics_blocked,
            std::path::Path::new("/sys/firmware/efi").exists(),
        );
        let mut model = AppModel {
            state,
            pages: page_controllers,
            carousel: None,
            page_widgets: std::collections::HashMap::new(),
            window: None,
        };

        let widgets = view_output!();

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
                // Next is hidden on it), so the margins would be dead space.
                let side_margin = if id == "progress" { 0 } else { 72 };
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
            AppMsg::OpenTerminal => Self::open_terminal(),
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
                // The distro descriptor (image, repart layout) is loaded by the
                // privileged runner itself; the request carries only user choices.
                match crate::backend::adapter::build_request(self.state.config()) {
                    Ok(req) => {
                        let exe = std::env::current_exe()
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or_else(|_| "/usr/bin/sirius".into());
                        let s = sender.clone();
                        std::thread::spawn(move || {
                            let result = crate::backend::spawn::run_install(&req, &exe, |p| {
                                s.input(AppMsg::Progress(p));
                            });
                            if let Err(e) = result {
                                s.input(AppMsg::Progress(crate::backend::Progress::Error {
                                    message: format!("failed to launch installer: {e}"),
                                }));
                            }
                        });
                    }
                    Err(e) => {
                        self.pages.progress(ProgressMsg::Failed {
                            message: format!("cannot start install: {e}"),
                        });
                    }
                }
            }
            AppMsg::Progress(p) => {
                use crate::backend::Progress;
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

impl AppModel {
    fn open_terminal() {
        if let Err(err) = std::process::Command::new("ptyxis").spawn() {
            tracing::error!(?err, "failed to launch Ptyxis");
        }
    }

    /// Modal "this will erase the disk" gate before leaving the summary page.
    fn confirm_install(&self, sender: &ComponentSender<Self>) {
        let config = self.state.config();
        let mut body = gettext(
            if matches!(
                config.install_type,
                Some(crate::config_model::InstallType::Manual)
            ) {
                "The staged partition changes will now be written to disk and Sirius will be installed. Formatted or deleted data cannot be recovered."
            } else {
                "All data on the selected disk will be permanently erased and the system will be installed. This cannot be undone."
            },
        );
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
