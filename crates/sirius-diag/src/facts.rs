//! Gathers raw system facts from the live machine. The pure probe functions in
//! `probes` consume these so the probes stay unit-testable without hardware.

use std::path::PathBuf;
use std::process::Command;

const SECTOR_BYTES: u64 = 512;

/// Raw, unjudged facts read from the running system.
#[derive(Debug, Clone)]
pub struct SystemFacts {
    pub efi_path: PathBuf,
    /// Total usable system RAM in bytes (MemTotal on Linux), via sysinfo.
    pub total_ram_bytes: u64,
    pub largest_disk_bytes: u64,
    pub secure_boot: Option<bool>,
    pub virt: Option<String>,
    pub online: bool,
}

impl SystemFacts {
    /// Read all facts from the live system. Best-effort: unreadable facts get safe defaults.
    pub fn gather() -> Self {
        Self {
            efi_path: PathBuf::from("/sys/firmware/efi"),
            total_ram_bytes: total_ram_bytes(),
            largest_disk_bytes: largest_disk_bytes(),
            secure_boot: read_secure_boot(),
            virt: detect_virt(),
            online: false, // refined by the installer's network page; the bare probe defaults offline-safe
        }
    }
}

/// Total usable system RAM in bytes via `sysinfo` (reads MemTotal on Linux).
/// This is usable RAM after kernel/firmware reservations, not the nominal/hypervisor
/// size — the correct figure to gate an install on.
fn total_ram_bytes() -> u64 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.total_memory()
}

/// Largest writable whole-disk size in bytes.
fn largest_disk_bytes() -> u64 {
    list_disks()
        .into_iter()
        .map(|disk| disk.size_bytes)
        .max()
        .unwrap_or(0)
}

/// efivar SecureBoot state. The 5th byte of the variable is 1 when enabled.
fn read_secure_boot() -> Option<bool> {
    let path = "/sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c";
    let bytes = std::fs::read(path).ok()?;
    bytes.get(4).map(|b| *b == 1)
}

/// `systemd-detect-virt`: exit 0 + value = virtualized, exit non-zero = bare metal.
fn detect_virt() -> Option<String> {
    let out = Command::new("systemd-detect-virt").output().ok()?;
    if out.status.success() {
        let kind = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if kind.is_empty() || kind == "none" {
            None
        } else {
            Some(kind)
        }
    } else {
        None
    }
}

/// A whole disk available as an install target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskInfo {
    pub path: String,
    pub model: String,
    pub size_bytes: u64,
}

/// List candidate targets directly through the `lsblk` crate and sysfs.
/// Keeps only writable whole disks: pseudo block devices (zram, loop, ram,
/// device-mapper, md, optical) are never valid install targets.
/// Returns an empty list on error (caller shows "no disks found").
pub fn list_disks() -> Vec<DiskInfo> {
    let Ok(Ok(devices)) = std::panic::catch_unwind(lsblk::BlockDevice::list) else {
        return Vec::new();
    };
    let mut disks = devices.iter().filter_map(disk_info).collect::<Vec<_>>();
    disks.sort_by(|left, right| left.path.cmp(&right.path));
    disks
}

fn disk_info(device: &lsblk::BlockDevice) -> Option<DiskInfo> {
    let sysfs = device.sysfs().ok()?;
    let size = device
        .capacity()
        .ok()
        .flatten()?
        .saturating_mul(SECTOR_BYTES);
    let read_only = read_u64(sysfs.join("ro")).is_some_and(|value| value != 0);
    let is_partition = sysfs.join("partition").exists();
    let model = read_trimmed(sysfs.join("device/model")).unwrap_or_default();

    candidate_disk(&device.name, size, read_only, is_partition, &model)
}

fn candidate_disk(
    name: &str,
    size_bytes: u64,
    read_only: bool,
    is_partition: bool,
    model: &str,
) -> Option<DiskInfo> {
    let pseudo = ["zram", "loop", "ram", "sr", "fd", "dm-", "md"];
    if is_partition
        || read_only
        || size_bytes == 0
        || pseudo.iter().any(|prefix| name.starts_with(prefix))
    {
        return None;
    }
    Some(DiskInfo {
        path: format!("/dev/{name}"),
        model: if model.trim().is_empty() {
            gettextrs::gettext("Disk")
        } else {
            model.trim().into()
        },
        size_bytes,
    })
}

fn read_trimmed(path: impl AsRef<std::path::Path>) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn read_u64(path: impl AsRef<std::path::Path>) -> Option<u64> {
    read_trimmed(path)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gather_does_not_panic() {
        // Smoke test: gathering on the CI host must never panic, whatever the hardware.
        let facts = SystemFacts::gather();
        assert_eq!(facts.efi_path, PathBuf::from("/sys/firmware/efi"));
    }

    #[test]
    fn keeps_real_disks() {
        let d = candidate_disk("vda", 68_719_476_736, false, false, "Virtio Block Device").unwrap();
        assert_eq!(d.path, "/dev/vda");
        assert_eq!(d.model, "Virtio Block Device");
        assert_eq!(d.size_bytes, 68_719_476_736);
    }

    #[test]
    fn drops_pseudo_and_unusable_devices() {
        for candidate in [
            ("zram0", 8_589_934_592, false, false),
            ("loop0", 1234, false, false),
            ("sr0", 2048, true, false),
            ("sda", 0, false, false),
            ("sdb", 1024, true, false),
            ("sdc1", 1024, false, true),
        ] {
            assert!(
                candidate_disk(candidate.0, candidate.1, candidate.2, candidate.3, "").is_none(),
                "should drop: {}",
                candidate.0
            );
        }
    }

    #[test]
    fn defaults_missing_model() {
        let d = candidate_disk("nvme0n1", 512_000_000_000, false, false, "").unwrap();
        assert_eq!(d.model, "Disk");
    }
}
