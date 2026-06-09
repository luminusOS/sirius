//! Gathers raw system facts from the live machine. The pure probe functions in
//! `probes` consume these so the probes stay unit-testable without hardware.

use std::path::PathBuf;
use std::process::Command;

/// Raw, unjudged facts read from the running system.
#[derive(Debug, Clone)]
pub struct SystemFacts {
    pub efi_path: PathBuf,
    pub meminfo: String,
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
            meminfo: std::fs::read_to_string("/proc/meminfo").unwrap_or_default(),
            largest_disk_bytes: largest_disk_bytes(),
            secure_boot: read_secure_boot(),
            virt: detect_virt(),
            online: false, // refined by Plan 2's network page; bare probe defaults offline-safe
        }
    }
}

/// Largest whole-disk size in bytes via `lsblk -b -d -n -o SIZE`.
fn largest_disk_bytes() -> u64 {
    let out = Command::new("lsblk")
        .args(["-b", "-d", "-n", "-o", "SIZE"])
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(|l| l.trim().parse::<u64>().ok())
            .max()
            .unwrap_or(0),
        Err(_) => 0,
    }
}

/// efivar SecureBoot state. The 5th byte of the variable is 1 when enabled.
fn read_secure_boot() -> Option<bool> {
    let path =
        "/sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c";
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

/// List candidate target disks via `lsblk -b -d -n -o NAME,SIZE,MODEL`.
/// Returns an empty list on error (caller shows "no disks found").
pub fn list_disks() -> Vec<DiskInfo> {
    let out = std::process::Command::new("lsblk")
        .args(["-b", "-d", "-n", "-o", "NAME,SIZE,MODEL"])
        .output();
    let Ok(out) = out else { return Vec::new() };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let name = parts.next()?;
            let size = parts.next()?.parse::<u64>().ok()?;
            let model = parts.collect::<Vec<_>>().join(" ");
            Some(DiskInfo {
                path: format!("/dev/{name}"),
                model: if model.is_empty() { "Disk".into() } else { model },
                size_bytes: size,
            })
        })
        .collect()
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
}
