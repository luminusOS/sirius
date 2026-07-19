use crate::check::{Check, Status};
use gettextrs::gettext;
use std::path::Path;

/// Pure UEFI check: passes when the EFI firmware sysfs path exists.
pub fn probe_uefi(efi_path: &Path) -> Check {
    if efi_path.exists() {
        Check::new(
            "uefi",
            &gettext("UEFI firmware"),
            Status::Pass,
            gettext("EFI firmware detected"),
        )
    } else {
        Check::new(
            "uefi",
            &gettext("UEFI firmware"),
            Status::Fail,
            gettext("System booted in legacy BIOS mode; UEFI is required"),
        )
    }
}

/// Pure RAM check: compares total usable RAM (bytes) to a minimum.
/// `total_bytes` is `SystemFacts::total_ram_bytes` (gathered via sysinfo); `min_gib`
/// is the required RAM in GiB.
pub fn probe_ram(total_bytes: u64, min_gib: u64) -> Check {
    // Compare in bytes (no truncation) and display with one decimal, so a machine
    // with ~3.8 GiB usable is not shown as "3 GiB" nor wrongly failed. This is usable
    // RAM (after kernel/firmware/device reservations) — the right figure to gate an
    // install on; the nominal/hypervisor RAM would need DMI (root-only and unreliable
    // in VMs).
    const GIB: u64 = 1024 * 1024 * 1024;
    let min_bytes = min_gib.saturating_mul(GIB);
    let total_gib = total_bytes as f64 / GIB as f64;
    if total_bytes >= min_bytes {
        Check::new(
            "ram",
            &gettext("Memory"),
            Status::Pass,
            gettext("{available} GiB available (need {required} GiB)")
                .replace("{available}", &format!("{total_gib:.1}"))
                .replace("{required}", &min_gib.to_string()),
        )
    } else {
        Check::new(
            "ram",
            &gettext("Memory"),
            Status::Fail,
            gettext("{available} GiB available, {required} GiB required")
                .replace("{available}", &format!("{total_gib:.1}"))
                .replace("{required}", &min_gib.to_string()),
        )
    }
}

/// Pure disk-space check: the largest available disk must hold the install image.
pub fn probe_disk_space(largest_disk_bytes: u64, required_bytes: u64) -> Check {
    if largest_disk_bytes >= required_bytes {
        Check::new(
            "disk_space",
            &gettext("Disk space"),
            Status::Pass,
            gettext("{available} GiB disk available (need {required} GiB)")
                .replace(
                    "{available}",
                    &(largest_disk_bytes / (1024 * 1024 * 1024)).to_string(),
                )
                .replace(
                    "{required}",
                    &(required_bytes / (1024 * 1024 * 1024)).to_string(),
                ),
        )
    } else {
        Check::new(
            "disk_space",
            &gettext("Disk space"),
            Status::Fail,
            gettext("No disk large enough for the install image"),
        )
    }
}

/// Pure Secure Boot check. `enabled` is the parsed efivar state, or `None` if unknown.
/// Informational: warns rather than fails so installs still proceed.
pub fn probe_secure_boot(enabled: Option<bool>) -> Check {
    match enabled {
        Some(true) => Check::new(
            "secure_boot",
            &gettext("Secure Boot"),
            Status::Pass,
            // Distinct msgid on purpose: the summary page already translates
            // "enabled" in the feminine (encryption) context.
            gettext("Secure Boot is enabled"),
        ),
        Some(false) => Check::new(
            "secure_boot",
            &gettext("Secure Boot"),
            Status::Warn,
            gettext("disabled; recommended for a secure system"),
        ),
        None => Check::new(
            "secure_boot",
            &gettext("Secure Boot"),
            Status::Warn,
            gettext("state could not be determined"),
        ),
    }
}

/// Pure virtualization check. `detected` is the `systemd-detect-virt` value,
/// or `None` when running on bare metal.
pub fn probe_virt(detected: Option<&str>) -> Check {
    match detected {
        None => Check::new(
            "virt",
            &gettext("Virtualization"),
            Status::Pass,
            gettext("running on bare metal"),
        ),
        Some(kind) => Check::new(
            "virt",
            &gettext("Virtualization"),
            Status::Warn,
            gettext("running inside a virtual machine ({kind})").replace("{kind}", kind),
        ),
    }
}

/// Pure network check.
pub fn probe_network(online: bool) -> Check {
    if online {
        Check::new(
            "network",
            &gettext("Network"),
            Status::Pass,
            gettext("connected"),
        )
    } else {
        Check::new(
            "network",
            &gettext("Network"),
            Status::Warn,
            gettext("no connection; some post-install steps may be skipped"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn uefi_pass_when_path_exists() {
        // The crate's own src dir always exists, used as a stand-in for an existing path.
        let existing = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(probe_uefi(&existing).status, Status::Pass);
    }

    #[test]
    fn uefi_fail_when_path_missing() {
        let missing = PathBuf::from("/definitely/not/here/sirius-efi");
        assert_eq!(probe_uefi(&missing).status, Status::Fail);
    }

    const GIB: u64 = 1024 * 1024 * 1024;

    #[test]
    fn ram_pass_when_enough() {
        assert_eq!(probe_ram(3 * GIB, 2).status, Status::Pass);
    }

    #[test]
    fn ram_fail_when_too_little() {
        assert_eq!(probe_ram(GIB, 2).status, Status::Fail);
    }

    #[test]
    fn ram_fail_when_zero() {
        assert_eq!(probe_ram(0, 2).status, Status::Fail);
    }

    #[test]
    fn ram_shows_one_decimal_not_truncated() {
        // 3_984_588 KiB ~= 3.8 GiB usable: must display "3.8", not "3", and pass at min 2.
        let check = probe_ram(3_984_588 * 1024, 2);
        assert_eq!(check.status, Status::Pass);
        assert!(
            check.detail.contains("3.8 GiB available"),
            "detail was: {}",
            check.detail
        );
    }

    #[test]
    fn disk_pass_when_big_enough() {
        let gib = 1024 * 1024 * 1024;
        assert_eq!(probe_disk_space(40 * gib, 20 * gib).status, Status::Pass);
    }

    #[test]
    fn disk_fail_when_too_small() {
        let gib = 1024 * 1024 * 1024;
        assert_eq!(probe_disk_space(8 * gib, 20 * gib).status, Status::Fail);
    }

    #[test]
    fn secure_boot_states() {
        assert_eq!(probe_secure_boot(Some(true)).status, Status::Pass);
        assert_eq!(probe_secure_boot(Some(false)).status, Status::Warn);
        assert_eq!(probe_secure_boot(None).status, Status::Warn);
    }

    #[test]
    fn virt_states() {
        assert_eq!(probe_virt(None).status, Status::Pass);
        assert_eq!(probe_virt(Some("kvm")).status, Status::Warn);
    }

    #[test]
    fn network_states() {
        assert_eq!(probe_network(true).status, Status::Pass);
        assert_eq!(probe_network(false).status, Status::Warn);
    }
}
