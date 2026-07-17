//! Gathers raw system facts from the live machine. The pure probe functions in
//! `probes` consume these so the probes stay unit-testable without hardware.

use std::path::PathBuf;
use std::process::Command;

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
            online: false, // refined by Plan 2's network page; bare probe defaults offline-safe
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

/// List candidate target disks via `lsblk -b -d -n -P -o NAME,SIZE,MODEL,TYPE,RO`.
/// Keeps only writable whole disks: pseudo block devices (zram, loop, ram,
/// device-mapper, md, optical) are never valid install targets.
/// Returns an empty list on error (caller shows "no disks found").
pub fn list_disks() -> Vec<DiskInfo> {
    let out = std::process::Command::new("lsblk")
        .args(["-b", "-d", "-n", "-P", "-o", "NAME,SIZE,MODEL,TYPE,RO"])
        .output();
    let Ok(out) = out else { return Vec::new() };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(parse_lsblk_line)
        .collect()
}

/// Parse one `lsblk -P` line (`KEY="value" ...`) into a `DiskInfo`, applying
/// the install-target filter. `None` for filtered-out or malformed lines.
fn parse_lsblk_line(line: &str) -> Option<DiskInfo> {
    // -P emits alternating `KEY="`/`value` segments when split on '"'.
    let mut fields = std::collections::HashMap::new();
    let mut parts = line.split('"');
    while let (Some(key), Some(value)) = (parts.next(), parts.next()) {
        fields.insert(key.trim().trim_end_matches('='), value);
    }

    let name = fields.get("NAME")?;
    let size = fields.get("SIZE")?.parse::<u64>().ok()?;
    let pseudo = ["zram", "loop", "ram", "sr", "fd", "dm-", "md"];
    if *fields.get("TYPE")? != "disk"
        || *fields.get("RO")? != "0"
        || size == 0
        || pseudo.iter().any(|p| name.starts_with(p))
    {
        return None;
    }
    let model = fields.get("MODEL").map(|m| m.trim()).unwrap_or_default();
    Some(DiskInfo {
        path: format!("/dev/{name}"),
        model: if model.is_empty() { "Disk".into() } else { model.into() },
        size_bytes: size,
    })
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
    fn lsblk_line_keeps_real_disks() {
        let d = parse_lsblk_line(
            r#"NAME="vda" SIZE="68719476736" MODEL="Virtio Block Device" TYPE="disk" RO="0""#,
        )
        .unwrap();
        assert_eq!(d.path, "/dev/vda");
        assert_eq!(d.model, "Virtio Block Device");
        assert_eq!(d.size_bytes, 68719476736);
    }

    #[test]
    fn lsblk_line_drops_pseudo_devices() {
        // zram reports TYPE="disk" but is never an install target.
        for line in [
            r#"NAME="zram0" SIZE="8589934592" MODEL="" TYPE="disk" RO="0""#,
            r#"NAME="loop0" SIZE="1234" MODEL="" TYPE="loop" RO="0""#,
            r#"NAME="sr0" SIZE="2048" MODEL="QEMU DVD-ROM" TYPE="rom" RO="1""#,
            r#"NAME="sda" SIZE="0" MODEL="Empty Reader" TYPE="disk" RO="0""#,
            r#"NAME="sdb" SIZE="1024" MODEL="WP Disk" TYPE="disk" RO="1""#,
        ] {
            assert!(parse_lsblk_line(line).is_none(), "should drop: {line}");
        }
    }

    #[test]
    fn lsblk_line_defaults_missing_model() {
        let d = parse_lsblk_line(r#"NAME="nvme0n1" SIZE="512000000000" MODEL="" TYPE="disk" RO="0""#)
            .unwrap();
        assert_eq!(d.model, "Disk");
    }
}
