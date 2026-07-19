//! Pure state machine for the installer wizard.
//!
//! This module deliberately has no GTK or Relm4 dependency. Pages report user
//! choices; `WizardState` owns navigation and decides whether the current page
//! may advance.

use crate::config_model::{InstallConfig, InstallType};
use crate::navigator::Navigator;
use crate::pages::{PageOutput, StorageSelection};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateEffect {
    None,
    LanguageChanged,
    PageChanged,
    InstallRequested,
}

pub struct WizardState {
    config: InstallConfig,
    nav: Navigator,
    diagnostics_blocked: bool,
    uefi: bool,
    /// UI locale currently fed to gettext via LANGUAGE. Starts empty so the
    /// welcome page's initial SetLocale pins LANGUAGE (and thus the UI) to its
    /// default selection instead of whatever the environment advertises.
    ui_locale: String,
}

impl WizardState {
    pub fn new(pages: Vec<String>, diagnostics_blocked: bool, uefi: bool) -> Self {
        Self {
            config: InstallConfig::default(),
            nav: Navigator::new(pages),
            diagnostics_blocked,
            uefi,
            ui_locale: String::new(),
        }
    }

    pub fn config(&self) -> &InstallConfig {
        &self.config
    }

    pub fn current_page(&self) -> &str {
        self.nav.current()
    }

    pub fn is_first(&self) -> bool {
        self.nav.is_first()
    }

    pub fn is_last(&self) -> bool {
        self.nav.is_last()
    }

    pub fn install_started(&self) -> bool {
        matches!(self.current_page(), "progress" | "finished")
    }

    pub fn can_proceed(&self) -> bool {
        match self.current_page() {
            "diagnostics" => !self.diagnostics_blocked,
            "storage" => self.storage_is_valid(),
            "user" => self.config.user.validate().is_ok(),
            "progress" | "finished" => false,
            _ => true,
        }
    }

    pub fn next(&mut self) -> &str {
        self.nav.next()
    }

    pub fn back(&mut self) -> &str {
        self.nav.prev()
    }

    pub fn seek(&mut self, page: &str) {
        while self.current_page() != page && !self.is_last() {
            self.next();
        }
    }

    pub fn apply(&mut self, output: PageOutput) -> StateEffect {
        match output {
            PageOutput::SetLocale(locale) => {
                let changed = locale != self.ui_locale;
                self.config.locale = Some(locale.clone());
                if changed {
                    self.ui_locale = locale.clone();
                    // glibc gettext consults LANGUAGE on every lookup, so setting
                    // it here plus re-rendering the pages switches the UI language.
                    std::env::set_var("LANGUAGE", locale);
                    StateEffect::LanguageChanged
                } else {
                    StateEffect::None
                }
            }
            PageOutput::SetKeyboard(keyboard) => {
                self.config.keyboard = Some(keyboard);
                StateEffect::None
            }
            PageOutput::SetTimezone(timezone) => {
                self.config.timezone = Some(timezone);
                StateEffect::None
            }
            PageOutput::SetStorage(selection) => {
                self.apply_storage(selection);
                StateEffect::None
            }
            PageOutput::SetUser(user) => {
                self.config.user = user;
                StateEffect::None
            }
            PageOutput::RequestNext => {
                self.next();
                StateEffect::PageChanged
            }
            PageOutput::RequestInstall => StateEffect::InstallRequested,
        }
    }

    fn apply_storage(&mut self, selection: StorageSelection) {
        self.config.destination_disk = Some(selection.path);
        self.config.destination_disk_name = Some(selection.name);
        self.config.install_type = Some(selection.install_type);
        self.config.encrypt = selection.encrypt;
        self.config.tpm = selection.tpm;
        self.config.encryption_passphrase = selection.encryption_passphrase;
        self.config.encryption_passphrase_confirm = selection.encryption_passphrase_confirm;
        self.config.partition_plan = selection.partition_plan;
    }

    fn storage_is_valid(&self) -> bool {
        if self.config.encrypt && self.config.validate_encryption().is_err() {
            return false;
        }
        self.config.destination_disk.is_some()
            && match self.config.install_type {
                Some(InstallType::Manual) => self
                    .config
                    .partition_plan
                    .as_ref()
                    .is_some_and(|plan| plan.validate(self.uefi).is_ok()),
                Some(_) => true,
                None => false,
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_model::{
        MountAssignment, PartitionOperation, PartitionPlan, PartitionRef, UserAccount,
    };

    fn state_at(page: &str, uefi: bool) -> WizardState {
        let mut state = WizardState::new(
            vec!["welcome".into(), page.into(), "summary".into()],
            false,
            uefi,
        );
        state.next();
        state
    }

    fn valid_user() -> UserAccount {
        UserAccount {
            full_name: "Ada Lovelace".into(),
            username: "ada".into(),
            password: "long-enough".into(),
            password_confirm: "long-enough".into(),
            hostname: "sirius".into(),
        }
    }

    #[test]
    fn user_gate_tracks_valid_to_invalid_drafts() {
        let mut state = state_at("user", false);
        state.apply(PageOutput::SetUser(valid_user()));
        assert!(state.can_proceed());

        let mut invalid = valid_user();
        invalid.password_confirm.clear();
        state.apply(PageOutput::SetUser(invalid));
        assert!(!state.can_proceed());
    }

    #[test]
    fn whole_disk_selection_can_advance() {
        let mut state = state_at("storage", false);
        state.apply(PageOutput::SetStorage(StorageSelection {
            path: "/dev/sda".into(),
            name: "Disk".into(),
            install_type: InstallType::WholeDisk,
            encrypt: false,
            tpm: false,
            encryption_passphrase: String::new(),
            encryption_passphrase_confirm: String::new(),
            partition_plan: None,
        }));
        assert!(state.can_proceed());
    }

    #[test]
    fn encrypted_selection_requires_a_valid_passphrase() {
        let mut state = state_at("storage", false);
        let selection = |passphrase: &str, confirm: &str| StorageSelection {
            path: "/dev/sda".into(),
            name: "Disk".into(),
            install_type: InstallType::Encrypted,
            encrypt: true,
            tpm: false,
            encryption_passphrase: passphrase.into(),
            encryption_passphrase_confirm: confirm.into(),
            partition_plan: None,
        };
        state.apply(PageOutput::SetStorage(selection("hunter2hunter", "typo")));
        assert!(!state.can_proceed());
        state.apply(PageOutput::SetStorage(selection("hunter2hunter", "hunter2hunter")));
        assert!(state.can_proceed());
    }

    #[test]
    fn manual_selection_requires_a_valid_plan() {
        let mut state = state_at("storage", false);
        let selection = StorageSelection {
            path: "/dev/sda".into(),
            name: "Disk".into(),
            install_type: InstallType::Manual,
            encrypt: false,
            tpm: false,
            encryption_passphrase: String::new(),
            encryption_passphrase_confirm: String::new(),
            partition_plan: Some(valid_bios_plan()),
        };
        state.apply(PageOutput::SetStorage(selection));
        assert!(state.can_proceed());
    }

    #[test]
    fn diagnostics_gate_uses_the_precomputed_result() {
        let mut blocked =
            WizardState::new(vec!["welcome".into(), "diagnostics".into()], true, false);
        blocked.next();
        assert!(!blocked.can_proceed());
    }

    fn valid_bios_plan() -> PartitionPlan {
        let gib = 1024 * 1024 * 1024;
        let target = PartitionRef::Planned { id: "root".into() };
        PartitionPlan {
            disk_path: "/dev/sda".into(),
            disk_size_bytes: 40 * gib,
            table_type: "gpt".into(),
            operations: vec![PartitionOperation::Create {
                id: "root".into(),
                offset_bytes: 1024 * 1024,
                size_bytes: 30 * gib,
                gpt_type: "0fc63daf-8483-4772-8e79-3d69d8477de4".into(),
                name: "Sirius".into(),
                filesystem: "btrfs".into(),
                label: "Sirius".into(),
            }],
            mounts: vec![MountAssignment {
                target,
                mount_point: "/".into(),
                filesystem: "btrfs".into(),
                label: "Sirius".into(),
            }],
        }
    }
}
