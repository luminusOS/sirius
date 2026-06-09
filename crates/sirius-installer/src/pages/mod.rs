//! Wizard pages. Each page is a Relm4 SimpleComponent that emits `PageOutput`
//! up to AppModel, which folds the change into InstallConfig.

pub mod diagnostics;
pub mod disk;
pub mod finished;
pub mod keyboard;
pub mod network;
pub mod partition;
pub mod progress;
pub mod summary;
pub mod timezone;
pub mod user;
pub mod welcome;

use crate::config_model::{InstallType, UserAccount};

/// Messages a page can send to the root AppModel.
#[derive(Debug, Clone)]
pub enum PageOutput {
    SetLocale(String),
    SetKeyboard(String),
    SetTimezone(String),
    SetDisk(String),
    SetPartition {
        install_type: InstallType,
        encrypt: bool,
        tpm: bool,
    },
    SetUser(UserAccount),
    /// The page reports whether the user may advance (gating, e.g. diagnostics).
    CanProceed(bool),
    /// Request to advance (from in-page buttons, optional).
    RequestNext,
}
