//! Wizard pages. Each page is a Relm4 SimpleComponent that emits `PageOutput`
//! up to AppModel, which folds the change into InstallConfig.

pub mod diagnostics;
pub mod finished;
pub mod keyboard;
pub mod network;
pub mod progress;
pub mod storage;
pub mod summary;
pub mod timezone;
pub mod user;
pub mod welcome;

use crate::config_model::{InstallType, PartitionPlan, UserAccount};
use relm4::adw;

/// Set a status page's translated header. Pages call this both in `init` and
/// on every `update_view`: gettext resolves at call time, so re-applying on
/// the `Retranslate` nudge is what re-renders the header in the new language.
/// One place, no drift between the two call sites.
pub(crate) fn status_header(root: &adw::StatusPage, title: &str, description: &str) {
    root.set_title(title);
    root.set_description(Some(description));
}

/// Storage choices collected by the storage page.
///
/// Keeping this as one value makes the page-to-app boundary explicit and avoids
/// growing `PageOutput` every time storage gains another option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageSelection {
    pub path: String,
    pub name: String,
    pub install_type: InstallType,
    pub encrypt: bool,
    pub tpm: bool,
    /// Dedicated LUKS passphrase pair; only meaningful when `encrypt` is set.
    pub encryption_passphrase: String,
    pub encryption_passphrase_confirm: String,
    pub partition_plan: Option<PartitionPlan>,
}

/// Messages a page can send to the root AppModel.
#[derive(Debug, Clone)]
pub enum PageOutput {
    SetLocale(String),
    SetKeyboard(String),
    SetTimezone(String),
    SetStorage(StorageSelection),
    SetUser(UserAccount),
    /// Request to advance (from in-page buttons, optional).
    RequestNext,
    /// The centered summary pill requests the destructive confirmation.
    RequestInstall,
}
