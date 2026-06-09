//! The shared, mutable installer state collected across wizard pages.
//! Plan 3 maps this into a libreadymade `Playbook`.

use serde::Serialize;

/// How the target disk should be laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallType {
    /// Erase the whole disk, single root.
    WholeDisk,
    /// Whole disk with LUKS encryption (optionally TPM-bound).
    Encrypted,
    /// User-defined layout (off by default).
    Manual,
}

/// User account fields collected on the user page.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct UserAccount {
    pub full_name: String,
    pub username: String,
    #[serde(skip)]
    pub password: String,
    #[serde(skip)]
    pub password_confirm: String,
    pub hostname: String,
}

/// Everything the wizard collects. Optional fields are `None` until their page runs.
#[derive(Debug, Clone, Default, Serialize)]
pub struct InstallConfig {
    pub locale: Option<String>,
    pub keyboard: Option<String>,
    pub timezone: Option<String>,
    pub destination_disk: Option<String>,
    pub install_type: Option<InstallType>,
    pub encrypt: bool,
    pub tpm: bool,
    pub user: UserAccount,
}

impl UserAccount {
    /// Validate the account fields, returning a human-readable error if invalid.
    pub fn validate(&self) -> Result<(), String> {
        if self.full_name.trim().is_empty() {
            return Err("Full name is required".into());
        }
        if self.username.is_empty() {
            return Err("Username is required".into());
        }
        if !self
            .username
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        {
            return Err("Username may only contain lowercase letters, digits, '_' and '-'".into());
        }
        if self.username.chars().next().map_or(true, |c| !c.is_ascii_lowercase()) {
            return Err("Username must start with a lowercase letter".into());
        }
        if self.password.len() < 8 {
            return Err("Password must be at least 8 characters".into());
        }
        if self.password != self.password_confirm {
            return Err("Passwords do not match".into());
        }
        if self.hostname.trim().is_empty() {
            return Err("Hostname is required".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_account() -> UserAccount {
        UserAccount {
            full_name: "Ada Lovelace".into(),
            username: "ada".into(),
            password: "hunter2hunter".into(),
            password_confirm: "hunter2hunter".into(),
            hostname: "luminus".into(),
        }
    }

    #[test]
    fn valid_account_passes() {
        assert!(valid_account().validate().is_ok());
    }

    #[test]
    fn mismatched_passwords_fail() {
        let mut a = valid_account();
        a.password_confirm = "different".into();
        assert_eq!(a.validate().unwrap_err(), "Passwords do not match");
    }

    #[test]
    fn short_password_fails() {
        let mut a = valid_account();
        a.password = "short".into();
        a.password_confirm = "short".into();
        assert!(a.validate().unwrap_err().contains("at least 8"));
    }

    #[test]
    fn bad_username_fails() {
        let mut a = valid_account();
        a.username = "Ada Lovelace".into();
        assert!(a.validate().is_err());
    }

    #[test]
    fn password_is_not_serialized() {
        let a = valid_account();
        let json = serde_json::to_string(&a).unwrap();
        assert!(!json.contains("hunter2hunter"));
    }
}
