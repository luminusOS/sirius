//! The shared installer state collected across wizard pages.

use gettextrs::gettext;
use serde::{Deserialize, Serialize};

/// How the target disk should be laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallType {
    /// Erase the whole disk, single root.
    WholeDisk,
    /// Whole disk with LUKS encryption (optionally TPM-bound).
    Encrypted,
    /// A validated, explicitly mounted layout. Disk changes are staged until
    /// the privileged runner starts the installation.
    Manual,
}

/// A stable reference used by a staged partition plan. Existing partitions
/// carry enough geometry to detect a stale/changed disk before any write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PartitionRef {
    Existing {
        path: String,
        start_bytes: u64,
        size_bytes: u64,
        part_uuid: Option<String>,
    },
    Planned {
        id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum PartitionOperation {
    Delete {
        target: PartitionRef,
    },
    Create {
        id: String,
        offset_bytes: u64,
        size_bytes: u64,
        gpt_type: String,
        name: String,
        filesystem: String,
        label: String,
    },
    Format {
        target: PartitionRef,
        filesystem: String,
        label: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MountAssignment {
    pub target: PartitionRef,
    pub mount_point: String,
    pub filesystem: String,
    pub label: String,
}

/// The complete, unprivileged preview of manual disk changes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartitionPlan {
    pub disk_path: String,
    pub disk_size_bytes: u64,
    pub table_type: String,
    pub operations: Vec<PartitionOperation>,
    pub mounts: Vec<MountAssignment>,
}

impl PartitionPlan {
    pub const MIN_ROOT_BYTES: u64 = 20 * 1024 * 1024 * 1024;
    pub const MIN_ESP_BYTES: u64 = 512 * 1024 * 1024;

    /// Validate the non-destructive shape of a manual plan. The privileged
    /// runner repeats this and additionally compares it to the live topology.
    pub fn validate(&self, uefi: bool) -> Result<(), String> {
        if self.table_type != "gpt" {
            return Err("manual partitioning requires a GPT disk".into());
        }
        if !self.disk_path.starts_with("/dev/") || self.disk_size_bytes == 0 {
            return Err("manual partitioning requires a valid destination disk".into());
        }
        let mut created = std::collections::HashSet::new();
        let mut ranges = Vec::new();
        for operation in &self.operations {
            match operation {
                PartitionOperation::Create {
                    id,
                    offset_bytes,
                    size_bytes,
                    gpt_type,
                    filesystem,
                    name,
                    label,
                } => {
                    if id.is_empty() || !created.insert(id.as_str()) {
                        return Err("planned partition ids must be unique".into());
                    }
                    if *offset_bytes < 1024 * 1024
                        || *size_bytes == 0
                        || offset_bytes
                            .checked_add(*size_bytes)
                            .is_none_or(|end| end > self.disk_size_bytes)
                    {
                        return Err("a planned partition is outside the destination disk".into());
                    }
                    if uuid::Uuid::parse_str(gpt_type).is_err() {
                        return Err("a planned partition has an invalid GPT type".into());
                    }
                    validate_filesystem(filesystem)?;
                    if name.contains('\0') || label.contains('\0') {
                        return Err("partition names and labels cannot contain NUL bytes".into());
                    }
                    ranges.push((*offset_bytes, *offset_bytes + *size_bytes));
                }
                PartitionOperation::Format {
                    filesystem, label, ..
                } => {
                    validate_filesystem(filesystem)?;
                    if label.contains('\0') {
                        return Err("partition labels cannot contain NUL bytes".into());
                    }
                }
                _ => {}
            }
        }
        ranges.sort_unstable();
        if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
            return Err("planned partitions overlap".into());
        }
        let mut mountpoints = std::collections::HashSet::new();
        for mount in &self.mounts {
            validate_filesystem(&mount.filesystem)?;
            if !mount.mount_point.starts_with('/') || !mountpoints.insert(&mount.mount_point) {
                return Err("mount points must be unique absolute paths".into());
            }
            validate_reference(&mount.target, &created, self.disk_size_bytes)?;
            if formatted_filesystem(&mount.target, &self.operations)
                .is_some_and(|filesystem| filesystem != mount.filesystem)
            {
                return Err("a mount assignment does not match its formatted filesystem".into());
            }
        }
        for operation in &self.operations {
            match operation {
                PartitionOperation::Delete {
                    target: PartitionRef::Planned { .. },
                } => return Err("a partition cannot be deleted before it is created".into()),
                PartitionOperation::Delete { target }
                | PartitionOperation::Format { target, .. } => {
                    validate_reference(target, &created, self.disk_size_bytes)?
                }
                PartitionOperation::Create { .. } => {}
            }
        }
        let roots: Vec<_> = self
            .mounts
            .iter()
            .filter(|mount| mount.mount_point == "/")
            .collect();
        if roots.len() != 1 {
            return Err("choose exactly one root partition".into());
        }
        if !matches!(roots[0].filesystem.as_str(), "btrfs" | "ext4") {
            return Err("the root partition must use Btrfs or ext4".into());
        }
        let root_size = partition_size(&roots[0].target, &self.operations).unwrap_or(0);
        if root_size < Self::MIN_ROOT_BYTES {
            return Err("the root partition must be at least 20 GiB".into());
        }
        if !is_formatted(&roots[0].target, &self.operations) {
            return Err("the root partition must be explicitly formatted".into());
        }
        if uefi {
            let esps: Vec<_> = self
                .mounts
                .iter()
                .filter(|mount| mount.mount_point == "/boot/efi")
                .collect();
            if esps.len() != 1 || esps[0].filesystem != "vfat" {
                return Err("choose one FAT32 EFI system partition".into());
            }
            let esp_size = partition_size(&esps[0].target, &self.operations).unwrap_or(0);
            if esp_size < Self::MIN_ESP_BYTES {
                return Err("the EFI system partition must be at least 512 MiB".into());
            }
        }
        Ok(())
    }
}

fn validate_filesystem(filesystem: &str) -> Result<(), String> {
    if matches!(filesystem, "btrfs" | "ext4" | "vfat" | "swap") {
        Ok(())
    } else {
        Err(format!("unsupported filesystem: {filesystem}"))
    }
}

fn validate_reference(
    target: &PartitionRef,
    created: &std::collections::HashSet<&str>,
    disk_size: u64,
) -> Result<(), String> {
    match target {
        PartitionRef::Existing {
            path,
            start_bytes,
            size_bytes,
            ..
        } => {
            if !path.starts_with("/dev/")
                || *size_bytes == 0
                || start_bytes
                    .checked_add(*size_bytes)
                    .is_none_or(|end| end > disk_size)
            {
                return Err("an existing partition reference is invalid".into());
            }
        }
        PartitionRef::Planned { id } if !created.contains(id.as_str()) => {
            return Err(format!("planned partition does not exist: {id}"));
        }
        PartitionRef::Planned { .. } => {}
    }
    Ok(())
}

fn partition_size(target: &PartitionRef, operations: &[PartitionOperation]) -> Option<u64> {
    match target {
        PartitionRef::Existing { size_bytes, .. } => Some(*size_bytes),
        PartitionRef::Planned { id } => operations.iter().find_map(|operation| match operation {
            PartitionOperation::Create {
                id: candidate,
                size_bytes,
                ..
            } if candidate == id => Some(*size_bytes),
            _ => None,
        }),
    }
}

fn is_formatted(target: &PartitionRef, operations: &[PartitionOperation]) -> bool {
    operations.iter().any(|operation| match operation {
        PartitionOperation::Create { id, .. } => {
            matches!(target, PartitionRef::Planned { id: target_id } if target_id == id)
        }
        PartitionOperation::Format {
            target: candidate, ..
        } => candidate == target,
        _ => false,
    })
}

fn formatted_filesystem<'a>(
    target: &PartitionRef,
    operations: &'a [PartitionOperation],
) -> Option<&'a str> {
    operations.iter().rev().find_map(|operation| match operation {
        PartitionOperation::Format {
            target: candidate,
            filesystem,
            ..
        } if candidate == target => Some(filesystem.as_str()),
        PartitionOperation::Create { id, filesystem, .. }
            if matches!(target, PartitionRef::Planned { id: target_id } if target_id == id) =>
        {
            Some(filesystem.as_str())
        }
        _ => None,
    })
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
    pub destination_disk_name: Option<String>,
    pub install_type: Option<InstallType>,
    pub encrypt: bool,
    pub tpm: bool,
    /// Dedicated LUKS passphrase collected on the storage page. Never
    /// serialized (like the account password): it only crosses the privilege
    /// boundary inside `InstallRequest::encryption_key`.
    #[serde(skip)]
    pub encryption_passphrase: String,
    #[serde(skip)]
    pub encryption_passphrase_confirm: String,
    pub partition_plan: Option<PartitionPlan>,
    pub user: UserAccount,
}

/// Validate a dedicated LUKS passphrase pair. Shared by the storage page
/// (inline hint), the Next-gate, and `build_request`.
pub fn validate_encryption_passphrase(passphrase: &str, confirm: &str) -> Result<(), String> {
    if passphrase.len() < 8 {
        return Err(gettext("Passphrase must be at least 8 characters"));
    }
    if passphrase != confirm {
        return Err(gettext("Passphrases do not match"));
    }
    Ok(())
}

impl InstallConfig {
    /// The collected passphrase pair, valid only when encryption is enabled.
    pub fn validate_encryption(&self) -> Result<(), String> {
        validate_encryption_passphrase(
            &self.encryption_passphrase,
            &self.encryption_passphrase_confirm,
        )
    }
}

impl UserAccount {
    /// Whether no user account data was collected.
    pub fn is_empty(&self) -> bool {
        self.full_name.trim().is_empty()
            && self.username.is_empty()
            && self.password.is_empty()
            && self.password_confirm.is_empty()
            && self.hostname.trim().is_empty()
    }

    /// Validate the account fields, returning a human-readable error if invalid.
    pub fn validate(&self) -> Result<(), String> {
        if self.full_name.trim().is_empty() {
            return Err(gettext("Full name is required"));
        }
        if self.username.is_empty() {
            return Err(gettext("Username is required"));
        }
        if !self
            .username
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        {
            return Err(gettext(
                "Username may only contain lowercase letters, digits, '_' and '-'",
            ));
        }
        if self
            .username
            .chars()
            .next()
            .is_none_or(|c| !c.is_ascii_lowercase())
        {
            return Err(gettext("Username must start with a lowercase letter"));
        }
        if self.password.len() < 8 {
            return Err(gettext("Password must be at least 8 characters"));
        }
        if self.password != self.password_confirm {
            return Err(gettext("Passwords do not match"));
        }
        if self.hostname.trim().is_empty() {
            return Err(gettext("Hostname is required"));
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
            hostname: "localhost".into(),
        }
    }

    #[test]
    fn valid_account_passes() {
        assert!(valid_account().validate().is_ok());
    }

    #[test]
    fn default_account_is_empty() {
        assert!(UserAccount::default().is_empty());
    }

    #[test]
    fn partial_account_is_not_empty() {
        let a = UserAccount {
            username: "ada".into(),
            ..Default::default()
        };
        assert!(!a.is_empty());
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

    fn valid_partition_plan() -> PartitionPlan {
        let mib = 1024 * 1024;
        let gib = 1024 * mib;
        PartitionPlan {
            disk_path: "/dev/sda".into(),
            disk_size_bytes: 40 * gib,
            table_type: "gpt".into(),
            operations: vec![
                PartitionOperation::Create {
                    id: "esp".into(),
                    offset_bytes: mib,
                    size_bytes: 512 * mib,
                    gpt_type: "c12a7328-f81f-11d2-ba4b-00a0c93ec93b".into(),
                    name: "EFI".into(),
                    filesystem: "vfat".into(),
                    label: "EFI".into(),
                },
                PartitionOperation::Create {
                    id: "root".into(),
                    offset_bytes: 513 * mib,
                    size_bytes: 30 * gib,
                    gpt_type: "0fc63daf-8483-4772-8e79-3d69d8477de4".into(),
                    name: "Sirius".into(),
                    filesystem: "btrfs".into(),
                    label: "Sirius".into(),
                },
            ],
            mounts: vec![
                MountAssignment {
                    target: PartitionRef::Planned { id: "esp".into() },
                    mount_point: "/boot/efi".into(),
                    filesystem: "vfat".into(),
                    label: "EFI".into(),
                },
                MountAssignment {
                    target: PartitionRef::Planned { id: "root".into() },
                    mount_point: "/".into(),
                    filesystem: "btrfs".into(),
                    label: "Sirius".into(),
                },
            ],
        }
    }

    #[test]
    fn manual_plan_requires_valid_root_and_efi() {
        assert!(valid_partition_plan().validate(true).is_ok());
        let mut missing_efi = valid_partition_plan();
        missing_efi
            .mounts
            .retain(|mount| mount.mount_point != "/boot/efi");
        assert!(missing_efi.validate(true).unwrap_err().contains("EFI"));
    }

    #[test]
    fn manual_plan_rejects_overlap_and_unknown_filesystem() {
        let mut overlap = valid_partition_plan();
        if let PartitionOperation::Create { offset_bytes, .. } = &mut overlap.operations[1] {
            *offset_bytes = 256 * 1024 * 1024;
        }
        assert!(overlap.validate(true).unwrap_err().contains("overlap"));

        let mut unknown = valid_partition_plan();
        if let PartitionOperation::Create { filesystem, .. } = &mut unknown.operations[1] {
            *filesystem = "xfs".into();
        }
        assert!(unknown.validate(true).unwrap_err().contains("unsupported"));
    }
}
