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
use relm4::{ComponentController, ComponentSender, Controller, gtk};

/// One row per wizard page: `id: PageType [MsgType]`. The single list
/// generates the controller struct, the id → widget lookup, the retranslate
/// broadcast, and `IMPLEMENTED_PAGES` (page ids are the field names, in
/// wizard order) — adding a page means adding one row here plus its
/// `launch()` line below.
macro_rules! wizard_pages {
    ($( $id:ident: $page:ty [$msg:ty] ),+ $(,)?) => {
        pub(super) struct PageControllers {
            $( $id: Controller<$page>, )+
        }

        /// Page ids that actually have mounted widgets in the Stack.
        pub(super) const IMPLEMENTED_PAGES: &[&str] = &[ $( stringify!($id), )+ ];

        impl PageControllers {
            pub fn widget(&self, id: &str) -> Option<gtk::Widget> {
                match id {
                    $( stringify!($id) => Some(self.$id.widget().clone().upcast()), )+
                    _ => None,
                }
            }

            /// Ask every page to rebuild its widgets after a UI language
            /// switch. Translations are resolved by gettext at render time,
            /// so the pages only need a nudge; Relm4 runs update_view after
            /// each update.
            pub fn retranslate(&self) {
                $( self.$id.sender().send(<$msg>::Retranslate).ok(); )+
            }
        }
    };
}

wizard_pages! {
    welcome: WelcomePage [WelcomeMsg],
    diagnostics: DiagnosticsPage [DiagnosticsMsg],
    network: NetworkPage [NetworkMsg],
    keyboard: KeyboardPage [KeyboardMsg],
    timezone: TimezonePage [TimezoneMsg],
    storage: StoragePage [StorageMsg],
    user: UserPage [UserMsg],
    summary: SummaryPage [SummaryMsg],
    progress: ProgressPage [ProgressMsg],
    finished: FinishedPage [FinishedMsg],
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
