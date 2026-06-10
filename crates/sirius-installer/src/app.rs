//! Root wizard component. Owns the window, the navigation stack, and InstallConfig.

use crate::config_model::InstallConfig;
use crate::navigator::Navigator;
use crate::pages::diagnostics::{DiagnosticsInit, DiagnosticsPage};
use crate::pages::disk::DiskPage;
use crate::pages::finished::FinishedPage;
use crate::pages::keyboard::KeyboardPage;
use crate::pages::network::NetworkPage;
use crate::pages::partition::PartitionPage;
use crate::pages::progress::ProgressPage;
use crate::pages::summary::{SummaryMsg, SummaryPage};
use crate::pages::timezone::TimezonePage;
use crate::pages::user::UserPage;
use crate::pages::welcome::WelcomePage;
use crate::pages::PageOutput;
use relm4::adw::prelude::*;
use relm4::prelude::*;
use relm4::{adw, gtk, ComponentController, ComponentParts, ComponentSender, Controller, SimpleComponent};
use sirius_diag::config::CONFIG_PATH;
use sirius_diag::{is_blocked, run_all_checks_with_config, SiriusConfig, SystemFacts};
use std::path::Path;

/// Page ids that actually have mounted widgets in the Stack.
/// Note: NO `manual_partition` — it has no widget and would render blank.
const IMPLEMENTED_PAGES: &[&str] = &[
    "welcome",
    "diagnostics",
    "network",
    "keyboard",
    "timezone",
    "disk",
    "partition",
    "user",
    "summary",
    "progress",
    "finished",
];

pub struct AppModel {
    config: InstallConfig,
    nav: Navigator,
    can_proceed: bool,
    diagnostics_blocked: bool,
    lang: crate::i18n::Lang,
    welcome: Controller<WelcomePage>,
    _diagnostics: Controller<DiagnosticsPage>,
    _network: Controller<NetworkPage>,
    _keyboard: Controller<KeyboardPage>,
    _timezone: Controller<TimezonePage>,
    _disk: Controller<DiskPage>,
    _partition: Controller<PartitionPage>,
    _user: Controller<UserPage>,
    summary: Controller<SummaryPage>,
    progress: Controller<ProgressPage>,
    finished: Controller<FinishedPage>,
    carousel: Option<adw::Carousel>,
    page_widgets: std::collections::HashMap<String, gtk::Widget>,
}

#[derive(Debug)]
pub enum AppMsg {
    Page(PageOutput),
    Next,
    Back,
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
            set_default_width: 900,
            set_default_height: 680,

            #[wrap(Some)]
            set_content = &adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    pack_start = &gtk::Button {
                        #[watch]
                        set_label: crate::i18n::tr(model.lang, "nav.back"),
                        #[watch]
                        set_sensitive: !model.nav.is_first(),
                        connect_clicked => AppMsg::Back,
                    },

                    pack_end = &gtk::Button {
                        #[watch]
                        set_label: crate::i18n::tr(model.lang, "nav.next"),
                        add_css_class: "suggested-action",
                        #[watch]
                        set_sensitive: model.can_proceed,
                        connect_clicked => AppMsg::Next,
                    },

                    #[name = "dots"]
                    #[wrap(Some)]
                    set_title_widget = &adw::CarouselIndicatorDots {
                    },
                },

                #[name = "carousel"]
                #[wrap(Some)]
                set_content = &adw::Carousel {
                    set_vexpand: true,
                    set_interactive: false,
                    set_allow_scroll_wheel: false,
                    set_allow_mouse_drag: false,
                    set_allow_long_swipes: false,
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let (cfg, warning) = SiriusConfig::load_or_default(Path::new(CONFIG_PATH));
        if let Some(w) = warning {
            tracing::warn!("{w}");
        }
        let pages: Vec<String> = cfg
            .pages
            .resolve()
            .into_iter()
            .filter(|p| IMPLEMENTED_PAGES.contains(&p.as_str()))
            .collect();
        let pages_order = pages.clone();
        let nav = Navigator::new(pages);
        let diag_config = cfg.diagnostics.clone();

        let diagnostics_blocked = {
            let facts = SystemFacts::gather();
            let checks = run_all_checks_with_config(&facts, &cfg.diagnostics);
            is_blocked(&checks, &cfg.diagnostics.require)
        };

        let welcome = WelcomePage::builder()
            .launch(())
            .forward(sender.input_sender(), AppMsg::Page);

        let diagnostics = DiagnosticsPage::builder()
            .launch(DiagnosticsInit {
                config: diag_config,
            })
            .forward(sender.input_sender(), AppMsg::Page);

        let network = NetworkPage::builder()
            .launch(())
            .forward(sender.input_sender(), AppMsg::Page);

        let keyboard = KeyboardPage::builder()
            .launch(())
            .forward(sender.input_sender(), AppMsg::Page);

        let timezone = TimezonePage::builder()
            .launch(())
            .forward(sender.input_sender(), AppMsg::Page);

        let disk = DiskPage::builder()
            .launch(())
            .forward(sender.input_sender(), AppMsg::Page);

        let partition = PartitionPage::builder()
            .launch(())
            .forward(sender.input_sender(), AppMsg::Page);

        let user = UserPage::builder()
            .launch(())
            .forward(sender.input_sender(), AppMsg::Page);

        let summary = SummaryPage::builder()
            .launch(())
            .forward(sender.input_sender(), AppMsg::Page);

        let progress = ProgressPage::builder()
            .launch(())
            .forward(sender.input_sender(), AppMsg::Page);

        let finished = FinishedPage::builder()
            .launch(())
            .forward(sender.input_sender(), AppMsg::Page);

        let mut model = AppModel {
            config: InstallConfig::default(),
            nav,
            can_proceed: true,
            diagnostics_blocked,
            lang: crate::i18n::Lang::default(),
            welcome,
            _diagnostics: diagnostics,
            _network: network,
            _keyboard: keyboard,
            _timezone: timezone,
            _disk: disk,
            _partition: partition,
            _user: user,
            summary,
            progress,
            finished,
            carousel: None,
            page_widgets: std::collections::HashMap::new(),
        };

        model.can_proceed = model.gate_for();

        let widgets = view_output!();

        let lookup: Vec<(&str, gtk::Widget)> = vec![
            ("welcome", model.welcome.widget().clone().upcast()),
            ("diagnostics", model._diagnostics.widget().clone().upcast()),
            ("network", model._network.widget().clone().upcast()),
            ("keyboard", model._keyboard.widget().clone().upcast()),
            ("timezone", model._timezone.widget().clone().upcast()),
            ("disk", model._disk.widget().clone().upcast()),
            ("partition", model._partition.widget().clone().upcast()),
            ("user", model._user.widget().clone().upcast()),
            ("summary", model.summary.widget().clone().upcast()),
            ("progress", model.progress.widget().clone().upcast()),
            ("finished", model.finished.widget().clone().upcast()),
        ];
        let lookup: std::collections::HashMap<&str, gtk::Widget> = lookup.into_iter().collect();
        for id in &pages_order {
            if let Some(w) = lookup.get(id.as_str()) {
                // Force each page to fill the carousel viewport. Without this an
                // AdwCarousel renders pages at their natural width, centered, and
                // the neighbouring page peeks in from the side.
                w.set_hexpand(true);
                w.set_vexpand(true);
                w.set_halign(gtk::Align::Fill);
                w.set_valign(gtk::Align::Fill);
                widgets.carousel.append(w);
                model.page_widgets.insert(id.clone(), w.clone());
            }
        }
        model.carousel = Some(widgets.carousel.clone());
        widgets.dots.set_carousel(Some(&widgets.carousel));

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            AppMsg::Page(out) => self.apply_page_output(out),
            AppMsg::Next => {
                let was = self.nav.current().to_string();
                self.nav.next();
                self.can_proceed = self.gate_for();
                self.scroll_to_current();
                if self.nav.current() == "summary" {
                    self.summary.sender().send(SummaryMsg::Show(self.config.clone())).ok();
                }
                if was == "summary" && self.nav.current() == "progress" {
                    sender.input(AppMsg::StartInstall);
                }
            }
            AppMsg::Back => {
                self.nav.prev();
                self.can_proceed = self.gate_for();
                self.scroll_to_current();
            }
            AppMsg::StartInstall => {
                // Load the distro descriptor: prefer the installed path, fall back to the
                // in-tree data file for dev/VM runs.
                let descriptor = std::fs::read_to_string(crate::backend::distro::DISTRO_PATH)
                    .or_else(|_| std::fs::read_to_string("data/distro.toml"))
                    .ok()
                    .and_then(|s| crate::backend::distro::DistroDescriptor::from_toml(&s).ok());
                let Some(descriptor) = descriptor else {
                    self.progress.sender().send(crate::pages::progress::ProgressMsg::Update {
                        fraction: 0.0,
                        line: "ERROR: missing or invalid distro descriptor (distro.toml)".into(),
                    }).ok();
                    return;
                };
                match crate::backend::adapter::build_request(&self.config, &descriptor) {
                    Ok(req) => {
                        let exe = std::env::current_exe()
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or_else(|_| "/usr/bin/sirius".into());
                        let s = sender.clone();
                        std::thread::spawn(move || {
                            let _ = crate::backend::spawn::run_install(&req, &exe, |p| {
                                s.input(AppMsg::Progress(p));
                            });
                        });
                    }
                    Err(e) => {
                        self.progress.sender().send(crate::pages::progress::ProgressMsg::Update {
                            fraction: 0.0,
                            line: format!("ERROR: cannot start install: {e}"),
                        }).ok();
                    }
                }
            }
            AppMsg::Progress(p) => {
                use crate::backend::Progress;
                use crate::pages::progress::ProgressMsg;
                match p {
                    Progress::Step { fraction, message } => {
                        self.progress.sender().send(ProgressMsg::Update { fraction, line: message }).ok();
                    }
                    Progress::Finished => {
                        // Progress page's Done emits RequestNext, which advances the navigator to "finished".
                        self.progress.sender().send(ProgressMsg::Done).ok();
                    }
                    Progress::Error { message } => {
                        self.progress.sender().send(ProgressMsg::Update { fraction: 0.0, line: format!("ERROR: {message}") }).ok();
                    }
                }
            }
        }
    }
}

impl AppModel {
    fn scroll_to_current(&self) {
        if let (Some(carousel), Some(widget)) =
            (&self.carousel, self.page_widgets.get(self.nav.current()))
        {
            carousel.scroll_to(widget, true);
        }
    }

    fn apply_page_output(&mut self, out: PageOutput) {
        match out {
            PageOutput::SetLocale(v) => {
                let lang = crate::i18n::Lang::from_locale(&v);
                self.config.locale = Some(v);
                if lang != self.lang {
                    self.lang = lang;
                    self.broadcast_lang(lang);
                }
            }
            PageOutput::SetKeyboard(v) => self.config.keyboard = Some(v),
            PageOutput::SetTimezone(v) => self.config.timezone = Some(v),
            PageOutput::SetDisk(v) => self.config.destination_disk = Some(v),
            PageOutput::SetPartition {
                install_type,
                encrypt,
                tpm,
            } => {
                self.config.install_type = Some(install_type);
                self.config.encrypt = encrypt;
                self.config.tpm = tpm;
            }
            PageOutput::SetUser(u) => self.config.user = u,
            PageOutput::CanProceed(ok) => self.can_proceed = ok,
            PageOutput::RequestNext => {
                self.nav.next();
                self.can_proceed = self.gate_for();
                self.scroll_to_current();
                if self.nav.current() == "summary" {
                    self.summary.sender().send(SummaryMsg::Show(self.config.clone())).ok();
                }
            }
        }
    }

    fn broadcast_lang(&self, lang: crate::i18n::Lang) {
        use crate::pages::diagnostics::DiagnosticsMsg;
        use crate::pages::disk::DiskMsg;
        use crate::pages::finished::FinishedMsg;
        use crate::pages::keyboard::KeyboardMsg;
        use crate::pages::network::NetworkMsg;
        use crate::pages::partition::PartitionMsg;
        use crate::pages::progress::ProgressMsg;
        use crate::pages::summary::SummaryMsg;
        use crate::pages::timezone::TimezoneMsg;
        use crate::pages::user::UserMsg;
        use crate::pages::welcome::WelcomeMsg;
        self.welcome.sender().send(WelcomeMsg::SetLang(lang)).ok();
        self._diagnostics.sender().send(DiagnosticsMsg::SetLang(lang)).ok();
        self._network.sender().send(NetworkMsg::SetLang(lang)).ok();
        self._keyboard.sender().send(KeyboardMsg::SetLang(lang)).ok();
        self._timezone.sender().send(TimezoneMsg::SetLang(lang)).ok();
        self._disk.sender().send(DiskMsg::SetLang(lang)).ok();
        self._partition.sender().send(PartitionMsg::SetLang(lang)).ok();
        self._user.sender().send(UserMsg::SetLang(lang)).ok();
        self.summary.sender().send(SummaryMsg::SetLang(lang)).ok();
        self.progress.sender().send(ProgressMsg::SetLang(lang)).ok();
        self.finished.sender().send(FinishedMsg::SetLang(lang)).ok();
    }

    /// Decide whether Next is allowed for the CURRENT page, based purely on
    /// AppModel state. This is the authoritative arrival-gate.
    fn gate_for(&self) -> bool {
        match self.nav.current() {
            "diagnostics" => !self.diagnostics_blocked,
            "disk" => self.config.destination_disk.is_some(),
            "user" => self.config.user.validate().is_ok(),
            "progress" | "finished" => false,
            _ => true,
        }
    }
}
