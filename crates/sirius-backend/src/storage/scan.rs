//! Read-only block-device discovery without spawning system utilities.

use super::{DiskSnapshot, FreeRegion, PartitionSnapshot};
use lsblk::{BlockDevice, Mount};
use std::collections::{HashMap, HashSet};
use std::path::Path;

const SECTOR_BYTES: u64 = 512;
const MIB: u64 = 1024 * 1024;

#[derive(Debug, Clone)]
struct DeviceFacts {
    key: String,
    parent: Option<String>,
    holders: Vec<String>,
    path: String,
    size_bytes: u64,
    start_bytes: u64,
    filesystem: String,
    label: String,
    mountpoints: Vec<String>,
    gpt_type: String,
    part_uuid: String,
    model: String,
    table_type: String,
    read_only: bool,
    is_partition: bool,
}

/// Read the current block topology from `/dev`, sysfs, udev data, and
/// `/proc/mounts`. The `lsblk` crate performs the enumeration directly; no
/// external command is executed.
pub fn scan_disks() -> Result<Vec<DiskSnapshot>, String> {
    let mountpoints = mounted_devices()?;
    let devices = std::panic::catch_unwind(BlockDevice::list)
        .map_err(|_| "block-device enumeration panicked".to_owned())?
        .map_err(|error| format!("failed to list block devices: {error}"))?;
    let facts = devices
        .iter()
        .filter_map(|device| collect_facts(device, &mountpoints).ok())
        .collect::<Vec<_>>();
    let mounted_keys = mountpoints.keys().map(String::as_str).collect();

    Ok(build_snapshots(&facts, &mounted_keys))
}

fn mounted_devices() -> Result<HashMap<String, Vec<String>>, String> {
    let mounts = Mount::list().map_err(|error| format!("failed to list mount points: {error}"))?;
    let mut by_device = HashMap::<String, Vec<String>>::new();

    for mount in mounts {
        let path = Path::new(&mount.device);
        if !path.starts_with("/dev") {
            continue;
        }
        let Ok(device) = BlockDevice::from_path_unpopulated(path) else {
            continue;
        };
        let Ok((major, minor)) = device.major_minor() else {
            continue;
        };
        by_device
            .entry(device_key(major, minor))
            .or_default()
            .push(mount.mountpoint.to_string_lossy().into_owned());
    }

    Ok(by_device)
}

fn collect_facts(
    device: &BlockDevice,
    mountpoints: &HashMap<String, Vec<String>>,
) -> Result<DeviceFacts, String> {
    let (major, minor) = device
        .major_minor()
        .map_err(|error| format!("failed to inspect {}: {error}", device.fullname.display()))?;
    let key = device_key(major, minor);
    let sysfs = device
        .sysfs()
        .map_err(|error| format!("failed to locate {} in sysfs: {error}", device.name))?;
    let canonical_sysfs = std::fs::canonicalize(&sysfs)
        .map_err(|error| format!("failed to resolve {} in sysfs: {error}", device.name))?;
    let is_partition = sysfs.join("partition").exists();
    let parent = is_partition
        .then(|| canonical_sysfs.parent())
        .flatten()
        .and_then(read_device_key);
    let udev = read_udev_properties(major, minor);
    let model = read_trimmed(sysfs.join("device/model"))
        .or_else(|| udev.get("ID_MODEL_FROM_DATABASE").cloned())
        .or_else(|| udev.get("ID_MODEL").cloned())
        .unwrap_or_default();

    Ok(DeviceFacts {
        key: key.clone(),
        parent,
        holders: holder_keys(&sysfs),
        path: device.fullname.to_string_lossy().into_owned(),
        size_bytes: device
            .capacity()
            .map_err(|error| format!("failed to read {} capacity: {error}", device.name))?
            .unwrap_or(0)
            .saturating_mul(SECTOR_BYTES),
        start_bytes: read_u64(sysfs.join("start"))
            .unwrap_or(0)
            .saturating_mul(SECTOR_BYTES),
        filesystem: property(&udev, "ID_FS_TYPE"),
        label: device
            .label
            .clone()
            .or_else(|| udev.get("ID_FS_LABEL").cloned())
            .unwrap_or_default(),
        mountpoints: mountpoints.get(&key).cloned().unwrap_or_default(),
        gpt_type: property(&udev, "ID_PART_ENTRY_TYPE"),
        part_uuid: device.partuuid.clone().unwrap_or_default(),
        model,
        table_type: property(&udev, "ID_PART_TABLE_TYPE").to_ascii_uppercase(),
        read_only: read_u64(sysfs.join("ro")).is_some_and(|value| value != 0),
        is_partition,
    })
}

fn build_snapshots(facts: &[DeviceFacts], mounted_keys: &HashSet<&str>) -> Vec<DiskSnapshot> {
    let by_key = facts
        .iter()
        .map(|device| (device.key.as_str(), device))
        .collect::<HashMap<_, _>>();
    let mut children = HashMap::<&str, Vec<&str>>::new();

    for device in facts {
        if let Some(parent) = device.parent.as_deref() {
            children
                .entry(parent)
                .or_default()
                .push(device.key.as_str());
        }
        for holder in &device.holders {
            children
                .entry(device.key.as_str())
                .or_default()
                .push(holder.as_str());
        }
    }

    let mut disks = facts
        .iter()
        .filter(|device| is_install_disk(device))
        .map(|disk| {
            let mut partitions = facts
                .iter()
                .filter(|device| {
                    device.is_partition && device.parent.as_deref() == Some(disk.key.as_str())
                })
                .map(|partition| PartitionSnapshot {
                    path: partition.path.clone(),
                    start_bytes: partition.start_bytes,
                    size_bytes: partition.size_bytes,
                    filesystem: partition.filesystem.clone(),
                    label: partition.label.clone(),
                    mountpoints: partition.mountpoints.clone(),
                    gpt_type: partition.gpt_type.clone(),
                    part_uuid: partition.part_uuid.clone(),
                })
                .collect::<Vec<_>>();
            partitions.sort_by_key(|partition| partition.start_bytes);

            DiskSnapshot {
                model: if disk.model.trim().is_empty() {
                    disk.path.clone()
                } else {
                    disk.model.trim().to_owned()
                },
                path: disk.path.clone(),
                size_bytes: disk.size_bytes,
                table_type: disk.table_type.clone(),
                read_only: disk.read_only,
                in_use: device_tree_in_use(
                    disk.key.as_str(),
                    &by_key,
                    &children,
                    mounted_keys,
                    &mut HashSet::new(),
                ),
                free_regions: calculate_free_regions(disk.size_bytes, &partitions),
                partitions,
            }
        })
        .collect::<Vec<_>>();
    disks.sort_by(|left, right| left.path.cmp(&right.path));
    disks
}

fn is_install_disk(device: &DeviceFacts) -> bool {
    let name = device.path.strip_prefix("/dev/").unwrap_or(&device.path);
    let pseudo_prefixes = ["zram", "loop", "ram", "sr", "fd", "dm-", "md"];

    !device.is_partition
        && !device.read_only
        && device.size_bytes > 0
        && !pseudo_prefixes
            .iter()
            .any(|prefix| name.starts_with(prefix))
}

fn device_tree_in_use<'a>(
    key: &'a str,
    devices: &HashMap<&'a str, &'a DeviceFacts>,
    children: &HashMap<&'a str, Vec<&'a str>>,
    mounted_keys: &HashSet<&'a str>,
    visited: &mut HashSet<&'a str>,
) -> bool {
    if !visited.insert(key) {
        return false;
    }
    mounted_keys.contains(key)
        || devices
            .get(key)
            .is_some_and(|device| !device.mountpoints.is_empty())
        || children.get(key).is_some_and(|descendants| {
            descendants
                .iter()
                .any(|child| device_tree_in_use(child, devices, children, mounted_keys, visited))
        })
}

fn holder_keys(sysfs: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(sysfs.join("holders")) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| read_device_key(&entry.path()))
        .collect()
}

fn read_device_key(sysfs: &Path) -> Option<String> {
    read_trimmed(sysfs.join("dev"))
}

fn device_key(major: u32, minor: u32) -> String {
    format!("{major}:{minor}")
}

fn read_udev_properties(major: u32, minor: u32) -> HashMap<String, String> {
    let path = format!("/run/udev/data/b{major}:{minor}");
    let Ok(contents) = std::fs::read_to_string(path) else {
        return HashMap::new();
    };
    contents
        .lines()
        .filter_map(|line| line.strip_prefix("E:"))
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect()
}

fn property(properties: &HashMap<String, String>, key: &str) -> String {
    properties.get(key).cloned().unwrap_or_default()
}

fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn read_u64(path: impl AsRef<Path>) -> Option<u64> {
    read_trimmed(path)?.parse().ok()
}

fn calculate_free_regions(total: u64, partitions: &[PartitionSnapshot]) -> Vec<FreeRegion> {
    let mut free = Vec::new();
    let mut cursor = MIB.min(total);
    for partition in partitions {
        if partition.start_bytes > cursor.saturating_add(MIB) {
            free.push(FreeRegion {
                offset_bytes: cursor,
                size_bytes: partition.start_bytes - cursor,
            });
        }
        cursor = cursor.max(partition.start_bytes.saturating_add(partition.size_bytes));
    }
    if total > cursor.saturating_add(MIB) {
        free.push(FreeRegion {
            offset_bytes: cursor,
            size_bytes: total - cursor,
        });
    }
    free
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(key: &str, path: &str) -> DeviceFacts {
        DeviceFacts {
            key: key.into(),
            parent: None,
            holders: Vec::new(),
            path: path.into(),
            size_bytes: 200 * MIB,
            start_bytes: 0,
            filesystem: String::new(),
            label: String::new(),
            mountpoints: Vec::new(),
            gpt_type: String::new(),
            part_uuid: String::new(),
            model: String::new(),
            table_type: String::new(),
            read_only: false,
            is_partition: false,
        }
    }

    #[test]
    fn free_regions_account_for_partitions() {
        let partitions = vec![PartitionSnapshot {
            path: "/dev/sda1".into(),
            start_bytes: MIB,
            size_bytes: 100 * MIB,
            filesystem: "ext4".into(),
            label: String::new(),
            mountpoints: vec![],
            gpt_type: String::new(),
            part_uuid: String::new(),
        }];
        assert_eq!(
            calculate_free_regions(200 * MIB, &partitions)[0].size_bytes,
            99 * MIB
        );
    }

    #[test]
    fn mounted_nested_mapper_marks_whole_disk_in_use() {
        let mut disk = facts("8:0", "/dev/sda");
        let mut partition = facts("8:1", "/dev/sda1");
        partition.is_partition = true;
        partition.parent = Some(disk.key.clone());
        partition.holders.push("253:0".into());

        let mounted = HashSet::from(["253:0"]);
        let snapshots = build_snapshots(&[disk.clone(), partition], &mounted);
        assert!(snapshots[0].in_use);

        disk.read_only = true;
        assert!(build_snapshots(&[disk], &HashSet::new()).is_empty());
    }

    #[test]
    fn live_scan_returns_a_result_without_external_tools() {
        assert!(scan_disks().is_ok());
    }
}
