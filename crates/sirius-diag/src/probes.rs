use crate::check::{Check, Status};
use std::path::Path;

/// Pure UEFI check: passes when the EFI firmware sysfs path exists.
pub fn probe_uefi(efi_path: &Path) -> Check {
    if efi_path.exists() {
        Check::new("uefi", "UEFI firmware", Status::Pass, "EFI firmware detected")
    } else {
        Check::new(
            "uefi",
            "UEFI firmware",
            Status::Fail,
            "System booted in legacy BIOS mode; UEFI is required",
        )
    }
}

/// Pure RAM check: parses `/proc/meminfo` text and compares MemTotal to a minimum.
/// `min_gib` is the required RAM in GiB.
pub fn probe_ram(meminfo: &str, min_gib: u64) -> Check {
    let total_kib = meminfo
        .lines()
        .find_map(|line| {
            let rest = line.strip_prefix("MemTotal:")?;
            rest.split_whitespace().next()?.parse::<u64>().ok()
        })
        .unwrap_or(0);
    // Compare in KiB (no truncation) and display with one decimal, so a machine
    // with ~3.8 GiB usable is not shown as "3 GiB" nor wrongly failed. MemTotal is
    // usable RAM (after kernel/firmware/device reservations) — the right figure to
    // gate an install on; the nominal/hypervisor RAM would need DMI (root-only and
    // unreliable in VMs).
    let min_kib = min_gib.saturating_mul(1024 * 1024);
    let total_gib = total_kib as f64 / (1024.0 * 1024.0);
    if total_kib >= min_kib {
        Check::new(
            "ram",
            "Memory",
            Status::Pass,
            format!("{total_gib:.1} GiB available (need {min_gib} GiB)"),
        )
    } else {
        Check::new(
            "ram",
            "Memory",
            Status::Fail,
            format!("{total_gib:.1} GiB available, {min_gib} GiB required"),
        )
    }
}

/// Pure disk-space check: the largest available disk must hold the install image.
pub fn probe_disk_space(largest_disk_bytes: u64, required_bytes: u64) -> Check {
    if largest_disk_bytes >= required_bytes {
        Check::new(
            "disk_space",
            "Disk space",
            Status::Pass,
            format!(
                "{} GiB disk available (need {} GiB)",
                largest_disk_bytes / (1024 * 1024 * 1024),
                required_bytes / (1024 * 1024 * 1024)
            ),
        )
    } else {
        Check::new(
            "disk_space",
            "Disk space",
            Status::Fail,
            "No disk large enough for the install image",
        )
    }
}

/// Pure Secure Boot check. `enabled` is the parsed efivar state, or `None` if unknown.
/// Informational: warns rather than fails so installs still proceed.
pub fn probe_secure_boot(enabled: Option<bool>) -> Check {
    match enabled {
        Some(true) => Check::new("secure_boot", "Secure Boot", Status::Pass, "enabled"),
        Some(false) => Check::new(
            "secure_boot",
            "Secure Boot",
            Status::Warn,
            "disabled; recommended for a secure system",
        ),
        None => Check::new(
            "secure_boot",
            "Secure Boot",
            Status::Warn,
            "state could not be determined",
        ),
    }
}

/// Pure virtualization check. `detected` is the `systemd-detect-virt` value,
/// or `None` when running on bare metal.
pub fn probe_virt(detected: Option<&str>) -> Check {
    match detected {
        None => Check::new("virt", "Virtualization", Status::Pass, "running on bare metal"),
        Some(kind) => Check::new(
            "virt",
            "Virtualization",
            Status::Warn,
            format!("running inside a virtual machine ({kind})"),
        ),
    }
}

/// Pure network check.
pub fn probe_network(online: bool) -> Check {
    if online {
        Check::new("network", "Network", Status::Pass, "connected")
    } else {
        Check::new(
            "network",
            "Network",
            Status::Warn,
            "no connection; some post-install steps may be skipped",
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

    #[test]
    fn ram_pass_when_enough() {
        let meminfo = "MemTotal:       3145728 kB\nMemFree: 100 kB\n";
        assert_eq!(probe_ram(meminfo, 2).status, Status::Pass);
    }

    #[test]
    fn ram_fail_when_too_little() {
        let meminfo = "MemTotal:       1048576 kB\n";
        assert_eq!(probe_ram(meminfo, 2).status, Status::Fail);
    }

    #[test]
    fn ram_fail_when_unparseable() {
        assert_eq!(probe_ram("garbage", 2).status, Status::Fail);
    }

    #[test]
    fn ram_shows_one_decimal_not_truncated() {
        // 3984588 KiB ~= 3.8 GiB usable: must display "3.8", not "3", and pass at min 2.
        let check = probe_ram("MemTotal:       3984588 kB\n", 2);
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
