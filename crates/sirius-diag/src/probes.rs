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
    let total_gib = total_kib / (1024 * 1024);
    if total_gib >= min_gib {
        Check::new(
            "ram",
            "Memory",
            Status::Pass,
            format!("{total_gib} GiB available (need {min_gib} GiB)"),
        )
    } else {
        Check::new(
            "ram",
            "Memory",
            Status::Fail,
            format!("{total_gib} GiB available, {min_gib} GiB required"),
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
        let meminfo = "MemTotal:       16384000 kB\nMemFree: 100 kB\n";
        assert_eq!(probe_ram(meminfo, 4).status, Status::Pass);
    }

    #[test]
    fn ram_fail_when_too_little() {
        let meminfo = "MemTotal:       2048000 kB\n";
        assert_eq!(probe_ram(meminfo, 4).status, Status::Fail);
    }

    #[test]
    fn ram_fail_when_unparseable() {
        assert_eq!(probe_ram("garbage", 4).status, Status::Fail);
    }
}
