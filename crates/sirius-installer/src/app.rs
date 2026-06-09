//! Root wizard component. Owns the window, the navigation stack, and InstallConfig.

use crate::config_model::InstallConfig;
use crate::navigator::Navigator;
use crate::pages::diagnostics::{DiagnosticsInit, DiagnosticsPage};
use crate::pages::disk::DiskPage;
use crate::pages::keyboard::KeyboardPage;
use crate::pages::network::NetworkPage;
use crate::pages::timezone::TimezonePage;
use crate::pages::welcome::WelcomePage;
use crate::pages::PageOutput;
use relm4::adw::prelude::*;
use relm4::prelude::*;
use relm4::{adw, gtk, ComponentController, ComponentParts, ComponentSender, Controller, SimpleComponent};
use sirius_diag::config::CONFIG_PATH;
use sirius_diag::SiriusConfig;
use std::path::Path;

pub struct AppModel {
    config: InstallConfig,
    nav: Navigator,
    can_proceed: bool,
    _welcome: Controller<WelcomePage>,
    _diagnostics: Controller<DiagnosticsPage>,
    _network: Controller<NetworkPage>,
    _keyboard: Controller<KeyboardPage>,
    _timezone: Controller<TimezonePage>,
    _disk: Controller<DiskPage>,
}

#[derive(Debug)]
pub enum AppMsg {
    Page(PageOutput),
    Next,
    Back,
}

#[relm4::component(pub)]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
        adw::ApplicationWindow {
            set_title: Some("Sirius"),
            set_default_width: 720,
            set_default_height: 540,

            #[wrap(Some)]
            set_content = &adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {},

                #[wrap(Some)]
                set_content = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,

                    #[name = "stack"]
                    gtk::Stack {
                        set_vexpand: true,
                        #[watch]
                        set_visible_child_name: model.nav.current(),
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 6,
                        set_margin_all: 12,

                        gtk::Button {
                            set_label: "Back",
                            #[watch]
                            set_sensitive: !model.nav.is_first(),
                            connect_clicked => AppMsg::Back,
                        },

                        gtk::Box { set_hexpand: true },

                        gtk::Button {
                            set_label: "Next",
                            add_css_class: "suggested-action",
                            #[watch]
                            set_sensitive: model.can_proceed,
                            connect_clicked => AppMsg::Next,
                        },
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
        let (cfg, warning) = SiriusConfig::load_or_default(Path::new(CONFIG_PATH));
        if let Some(w) = warning {
            tracing::warn!("{w}");
        }
        let nav = Navigator::new(cfg.pages.resolve());
        let diag_require = cfg.diagnostics.require.clone();

        let welcome = WelcomePage::builder()
            .launch(())
            .forward(sender.input_sender(), AppMsg::Page);

        let diagnostics = DiagnosticsPage::builder()
            .launch(DiagnosticsInit { require: diag_require })
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

        let model = AppModel {
            config: InstallConfig::default(),
            nav,
            can_proceed: true,
            _welcome: welcome,
            _diagnostics: diagnostics,
            _network: network,
            _keyboard: keyboard,
            _timezone: timezone,
            _disk: disk,
        };

        let widgets = view_output!();
        widgets
            .stack
            .add_named(model._welcome.widget(), Some("welcome"));
        widgets
            .stack
            .add_named(model._diagnostics.widget(), Some("diagnostics"));
        widgets
            .stack
            .add_named(model._network.widget(), Some("network"));
        widgets
            .stack
            .add_named(model._keyboard.widget(), Some("keyboard"));
        widgets
            .stack
            .add_named(model._timezone.widget(), Some("timezone"));
        widgets
            .stack
            .add_named(model._disk.widget(), Some("disk"));

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            AppMsg::Page(out) => self.apply_page_output(out),
            AppMsg::Next => {
                self.nav.next();
                self.can_proceed = true;
            }
            AppMsg::Back => {
                self.nav.prev();
                self.can_proceed = true;
            }
        }
    }
}

impl AppModel {
    fn apply_page_output(&mut self, out: PageOutput) {
        match out {
            PageOutput::SetLocale(v) => self.config.locale = Some(v),
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
                self.can_proceed = true;
            }
        }
    }
}
