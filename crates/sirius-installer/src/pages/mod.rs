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
