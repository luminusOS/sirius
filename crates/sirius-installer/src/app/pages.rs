//! Lifecycle and message routing for wizard page controllers.

use super::{AppModel, AppMsg};
use crate::backend::distro::{Bento, Branding};
use crate::config_model::InstallConfig;
use crate::pages::diagnostics::{DiagnosticsInit, DiagnosticsMsg, DiagnosticsPage};
use crate::pages::finished::{FinishedMsg, FinishedPage};
use crate::pages::keyboard::{KeyboardMsg, KeyboardPage};
use crate::pages::network::{NetworkMsg, NetworkPage};
use crate::pages::progress::{ProgressMsg, ProgressPage};
use crate::pages::storage::{StorageMsg, StoragePage};
use crate::pages::summary::{SummaryMsg, SummaryPage};
use crate::pages::timezone::{TimezoneMsg, TimezonePage};
use crate::pages::user::{UserMsg, UserPage};
use crate::pages::welcome::{WelcomeMsg, WelcomePage};
use relm4::gtk::prelude::*;
use relm4::prelude::*;
use relm4::{gtk, ComponentController, ComponentSender, Controller};

pub(super) struct PageControllers {
    welcome: Controller<WelcomePage>,
    diagnostics: Controller<DiagnosticsPage>,
    network: Controller<NetworkPage>,
    keyboard: Controller<KeyboardPage>,
    timezone: Controller<TimezonePage>,
    storage: Controller<StoragePage>,
    user: Controller<UserPage>,
    summary: Controller<SummaryPage>,
    progress: Controller<ProgressPage>,
    finished: Controller<FinishedPage>,
}

impl PageControllers {
    pub fn launch(
        sender: &ComponentSender<AppModel>,
        page_order: Vec<String>,
        diagnostics: DiagnosticsInit,
        bentos: Vec<Bento>,
        branding: Branding,
    ) -> Self {
        let output = sender.input_sender();
        Self {
            welcome: WelcomePage::builder()
                .launch(branding)
                .forward(output, AppMsg::Page),
            diagnostics: DiagnosticsPage::builder()
                .launch(diagnostics)
                .forward(sender.input_sender(), AppMsg::Page),
            network: NetworkPage::builder()
                .launch(())
                .forward(sender.input_sender(), AppMsg::Page),
            keyboard: KeyboardPage::builder()
                .launch(())
                .forward(sender.input_sender(), AppMsg::Page),
            timezone: TimezonePage::builder()
                .launch(())
                .forward(sender.input_sender(), AppMsg::Page),
            storage: StoragePage::builder()
                .launch(())
                .forward(sender.input_sender(), AppMsg::Page),
            user: UserPage::builder()
                .launch(())
                .forward(sender.input_sender(), AppMsg::Page),
            summary: SummaryPage::builder()
                .launch(page_order)
                .forward(sender.input_sender(), AppMsg::Page),
            progress: ProgressPage::builder()
                .launch(bentos)
                .forward(sender.input_sender(), AppMsg::Page),
            finished: FinishedPage::builder()
                .launch(())
                .forward(sender.input_sender(), AppMsg::Page),
        }
    }

    pub fn widget(&self, id: &str) -> Option<gtk::Widget> {
        let widget: gtk::Widget = match id {
            "welcome" => self.welcome.widget().clone().upcast(),
            "diagnostics" => self.diagnostics.widget().clone().upcast(),
            "network" => self.network.widget().clone().upcast(),
            "keyboard" => self.keyboard.widget().clone().upcast(),
            "timezone" => self.timezone.widget().clone().upcast(),
            "storage" => self.storage.widget().clone().upcast(),
            "user" => self.user.widget().clone().upcast(),
            "summary" => self.summary.widget().clone().upcast(),
            "progress" => self.progress.widget().clone().upcast(),
            "finished" => self.finished.widget().clone().upcast(),
            _ => return None,
        };
        Some(widget)
    }

    /// Ask every page to rebuild its widgets after a UI language switch.
    /// Translations are resolved by gettext at render time, so the pages only
    /// need a nudge; Relm4 runs update_view after each update.
    pub fn retranslate(&self) {
        self.welcome.sender().send(WelcomeMsg::Retranslate).ok();
        self.diagnostics
            .sender()
            .send(DiagnosticsMsg::Retranslate)
            .ok();
        self.network.sender().send(NetworkMsg::Retranslate).ok();
        self.keyboard.sender().send(KeyboardMsg::Retranslate).ok();
        self.timezone.sender().send(TimezoneMsg::Retranslate).ok();
        self.storage.sender().send(StorageMsg::Retranslate).ok();
        self.user.sender().send(UserMsg::Retranslate).ok();
        self.summary.sender().send(SummaryMsg::Retranslate).ok();
        self.progress.sender().send(ProgressMsg::Retranslate).ok();
        self.finished.sender().send(FinishedMsg::Retranslate).ok();
    }

    pub fn show_summary(&self, config: InstallConfig) {
        self.summary
            .sender()
            .send(SummaryMsg::Show(Box::new(config)))
            .ok();
    }

    pub fn progress(&self, message: ProgressMsg) {
        self.progress.sender().send(message).ok();
    }
}
