//! Disk discovery for the UI and the privileged UDisks2 partition executor.
//!
//! Discovery is read-only. Mutations are represented by `PartitionPlan` and
//! are applied only by `runner`, after pkexec and the final confirmation.

use crate::config_model::{MountAssignment, PartitionOperation, PartitionPlan, PartitionRef};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use zbus::blocking::Connection;
use zvariant::{OwnedObjectPath, OwnedValue};

const MIB: u64 = 1024 * 1024;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiskSnapshot {
    pub path: String,
    pub model: String,
    pub size_bytes: u64,
    pub table_type: String,
    pub read_only: bool,
    pub in_use: bool,
    pub partitions: Vec<PartitionSnapshot>,
    pub free_regions: Vec<FreeRegion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PartitionSnapshot {
    pub path: String,
    pub start_bytes: u64,
    pub size_bytes: u64,
    pub filesystem: String,
    pub label: String,
    pub mountpoints: Vec<String>,
    pub gpt_type: String,
    pub part_uuid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FreeRegion {
    pub offset_bytes: u64,
    pub size_bytes: u64,
}

pub fn format_size(bytes: u64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GiB", bytes as f64 / GIB)
    } else {
        format!("{:.0} MiB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Read the current block topology. `lsblk` provides the kernel's current,
/// non-privileged view and keeps the UI independent from UDisks policy.
pub fn scan_disks() -> Result<Vec<DiskSnapshot>, String> {
    let output = std::process::Command::new("lsblk")
        .args([
            "--bytes",
            "--json",
            "-o",
            "NAME,PATH,TYPE,SIZE,START,FSTYPE,LABEL,MOUNTPOINTS,PARTTYPE,PARTUUID,MODEL,PTTYPE,RO,LOG-SEC",
        ])
        .output()
        .map_err(|e| format!("failed to run lsblk: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "lsblk failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let root: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("invalid lsblk output: {e}"))?;
    let mut disks = Vec::new();
    for node in root["blockdevices"].as_array().into_iter().flatten() {
        let path = string(node, "path");
        if string(node, "type") != "disk"
            || bool_value(node, "ro")
            || path.starts_with("/dev/zram")
            || path.starts_with("/dev/loop")
        {
            continue;
        }
        let size_bytes = number(node, "size");
        let sector_size = number(node, "log-sec").max(512);
        let mut partitions = Vec::new();
        for child in node["children"].as_array().into_iter().flatten() {
            if string(child, "type") != "part" {
                continue;
            }
            partitions.push(PartitionSnapshot {
                path: string(child, "path"),
                start_bytes: number(child, "start").saturating_mul(sector_size),
                size_bytes: number(child, "size"),
                filesystem: string(child, "fstype"),
                label: string(child, "label"),
                mountpoints: strings(child, "mountpoints"),
                gpt_type: string(child, "parttype"),
                part_uuid: string(child, "partuuid"),
            });
        }
        partitions.sort_by_key(|p| p.start_bytes);
        let free_regions = calculate_free_regions(size_bytes, &partitions);
        let in_use = node_in_use(node);
        let model = string(node, "model").trim().to_string();
        disks.push(DiskSnapshot {
            model: if model.is_empty() {
                path.clone()
            } else {
                model
            },
            path,
            size_bytes,
            table_type: string(node, "pttype").to_ascii_uppercase(),
            read_only: bool_value(node, "ro"),
            in_use,
            partitions,
            free_regions,
        });
    }
    Ok(disks)
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

fn string(value: &serde_json::Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_string()
}

fn number(value: &serde_json::Value, key: &str) -> u64 {
    value[key]
        .as_u64()
        .or_else(|| value[key].as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0)
}

fn bool_value(value: &serde_json::Value, key: &str) -> bool {
    value[key]
        .as_bool()
        .or_else(|| value[key].as_u64().map(|v| v != 0))
        .unwrap_or(false)
}

fn strings(value: &serde_json::Value, key: &str) -> Vec<String> {
    value[key]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .collect()
}

fn node_in_use(node: &serde_json::Value) -> bool {
    !strings(node, "mountpoints").is_empty()
        || node["children"]
            .as_array()
            .is_some_and(|children| children.iter().any(node_in_use))
}

#[zbus::proxy(
    interface = "org.freedesktop.UDisks2.Manager",
    default_service = "org.freedesktop.UDisks2",
    default_path = "/org/freedesktop/UDisks2/Manager"
)]
trait UDisksManager {
    fn get_block_devices(
        &self,
        options: &HashMap<String, OwnedValue>,
    ) -> zbus::Result<Vec<OwnedObjectPath>>;
}

#[zbus::proxy(
    interface = "org.freedesktop.UDisks2.Block",
    default_service = "org.freedesktop.UDisks2"
)]
trait UDisksBlock {
    fn format(&self, filesystem: &str, options: &HashMap<String, OwnedValue>) -> zbus::Result<()>;

    #[zbus(property)]
    fn preferred_device(&self) -> zbus::Result<Vec<u8>>;
}

#[zbus::proxy(
    interface = "org.freedesktop.UDisks2.Partition",
    default_service = "org.freedesktop.UDisks2"
)]
trait UDisksPartition {
    fn delete(&self, options: &HashMap<String, OwnedValue>) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.UDisks2.PartitionTable",
    default_service = "org.freedesktop.UDisks2"
)]
#[allow(clippy::too_many_arguments)]
trait UDisksPartitionTable {
    #[allow(clippy::too_many_arguments)]
    fn create_partition_and_format(
        &self,
        offset: u64,
        size: u64,
        partition_type: &str,
        name: &str,
        options: &HashMap<String, OwnedValue>,
        filesystem: &str,
        format_options: &HashMap<String, OwnedValue>,
    ) -> zbus::Result<OwnedObjectPath>;
}

#[zbus::proxy(
    interface = "org.freedesktop.UDisks2.Filesystem",
    default_service = "org.freedesktop.UDisks2"
)]
trait UDisksFilesystem {
    fn set_label(&self, label: &str, options: &HashMap<String, OwnedValue>) -> zbus::Result<()>;
}

/// Validate the topology again and execute a staged manual plan through
/// UDisks2. This function must only run in the pkexec child.
pub fn apply_partition_plan(
    plan: &PartitionPlan,
    target_disk: &str,
) -> Result<libreadymade::backend::mounts::Mounts, String> {
    if plan.disk_path != target_disk {
        return Err("partition plan does not match the selected disk".into());
    }
    plan.validate(std::path::Path::new("/sys/firmware/efi").exists())?;
    let current = scan_disks()?
        .into_iter()
        .find(|disk| disk.path == target_disk)
        .ok_or_else(|| format!("selected disk disappeared: {target_disk}"))?;
    validate_topology(plan, &current)?;

    let connection =
        Connection::system().map_err(|e| format!("cannot connect to system bus: {e}"))?;
    let empty = HashMap::<String, OwnedValue>::new();
    let disk_object = object_for_device(&connection, target_disk)?;
    let table = UDisksPartitionTableProxyBlocking::builder(&connection)
        .path(disk_object.clone())
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| format!("cannot access partition table: {e}"))?;
    let mut planned_devices = HashMap::<String, String>::new();

    for operation in &plan.operations {
        if let PartitionOperation::Delete { target } = operation {
            let path = existing_path(target)?;
            let object = object_for_device(&connection, path)?;
            let partition = UDisksPartitionProxyBlocking::builder(&connection)
                .path(object)
                .map_err(|e| e.to_string())?
                .build()
                .map_err(|e| format!("cannot access {path}: {e}"))?;
            partition
                .delete(&empty)
                .map_err(|e| format!("cannot delete {path}: {e}"))?;
        }
    }

    for operation in &plan.operations {
        if let PartitionOperation::Create {
            id,
            offset_bytes,
            size_bytes,
            gpt_type,
            name,
            filesystem,
            label,
        } = operation
        {
            let mut format_options = HashMap::new();
            if !label.trim().is_empty() {
                format_options.insert("label".to_string(), owned(label.as_str()));
            }
            let object = table
                .create_partition_and_format(
                    *offset_bytes,
                    *size_bytes,
                    gpt_type,
                    name,
                    &empty,
                    filesystem,
                    &format_options,
                )
                .map_err(|e| format!("cannot create partition {name}: {e}"))?;
            planned_devices.insert(id.clone(), device_for_object(&connection, object)?);
        }
    }

    for operation in &plan.operations {
        match operation {
            PartitionOperation::Format {
                target,
                filesystem,
                label,
            } => {
                let path = resolve_ref(target, &planned_devices)?;
                let object = object_for_device(&connection, &path)?;
                let block = UDisksBlockProxyBlocking::builder(&connection)
                    .path(object)
                    .map_err(|e| e.to_string())?
                    .build()
                    .map_err(|e| format!("cannot access {path}: {e}"))?;
                let mut options = HashMap::new();
                if !label.trim().is_empty() {
                    options.insert("label".to_string(), owned(label.as_str()));
                }
                block
                    .format(filesystem, &options)
                    .map_err(|e| format!("cannot format {path}: {e}"))?;
            }
            PartitionOperation::SetLabel { target, label } => {
                let path = resolve_ref(target, &planned_devices)?;
                let object = object_for_device(&connection, &path)?;
                let filesystem = UDisksFilesystemProxyBlocking::builder(&connection)
                    .path(object)
                    .map_err(|e| e.to_string())?
                    .build()
                    .map_err(|e| format!("cannot access filesystem {path}: {e}"))?;
                filesystem
                    .set_label(label, &empty)
                    .map_err(|e| format!("cannot label {path}: {e}"))?;
            }
            PartitionOperation::Delete { .. } | PartitionOperation::Create { .. } => {}
        }
    }

    build_mounts(&plan.mounts, &planned_devices)
}

fn validate_topology(plan: &PartitionPlan, disk: &DiskSnapshot) -> Result<(), String> {
    if plan.disk_size_bytes != disk.size_bytes {
        return Err("disk size changed since the partition plan was created".into());
    }
    for reference in plan
        .operations
        .iter()
        .flat_map(operation_refs)
        .chain(plan.mounts.iter().map(|mount| &mount.target))
    {
        if let PartitionRef::Existing {
            path,
            start_bytes,
            size_bytes,
            part_uuid,
        } = reference
        {
            let current = disk
                .partitions
                .iter()
                .find(|part| part.path == *path)
                .ok_or_else(|| format!("partition disappeared: {path}"))?;
            if current.start_bytes != *start_bytes
                || current.size_bytes != *size_bytes
                || part_uuid
                    .as_deref()
                    .is_some_and(|uuid| current.part_uuid != uuid)
            {
                return Err(format!("partition topology changed: {path}"));
            }
            if !current.mountpoints.is_empty() {
                return Err(format!(
                    "partition is mounted and cannot be modified: {path}"
                ));
            }
        }
    }
    for mount in &plan.mounts {
        if let PartitionRef::Existing { path, .. } = &mount.target {
            let current = disk
                .partitions
                .iter()
                .find(|partition| partition.path == *path)
                .ok_or_else(|| format!("partition disappeared: {path}"))?;
            let reformatted = plan.operations.iter().any(|operation| {
                matches!(operation, PartitionOperation::Format { target, filesystem, .. }
                    if target == &mount.target && filesystem == &mount.filesystem)
            });
            if !reformatted && current.filesystem != mount.filesystem {
                return Err(format!(
                    "filesystem changed for {path}: expected {}",
                    mount.filesystem
                ));
            }
        }
    }
    Ok(())
}

fn operation_refs(operation: &PartitionOperation) -> Vec<&PartitionRef> {
    match operation {
        PartitionOperation::Delete { target }
        | PartitionOperation::Format { target, .. }
        | PartitionOperation::SetLabel { target, .. } => vec![target],
        PartitionOperation::Create { .. } => vec![],
    }
}

fn object_for_device(connection: &Connection, device: &str) -> Result<OwnedObjectPath, String> {
    let manager = UDisksManagerProxyBlocking::new(connection)
        .map_err(|e| format!("cannot access UDisks2: {e}"))?;
    for object in manager
        .get_block_devices(&HashMap::new())
        .map_err(|e| format!("cannot enumerate UDisks2 devices: {e}"))?
    {
        if device_for_object(connection, object.clone())? == device {
            return Ok(object);
        }
    }
    Err(format!("UDisks2 has no object for {device}"))
}

fn device_for_object(connection: &Connection, object: OwnedObjectPath) -> Result<String, String> {
    let block = UDisksBlockProxyBlocking::builder(connection)
        .path(object)
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| format!("cannot access UDisks2 block: {e}"))?;
    let bytes = block
        .preferred_device()
        .map_err(|e| format!("cannot read UDisks2 device: {e}"))?;
    Ok(String::from_utf8_lossy(&bytes)
        .trim_end_matches('\0')
        .to_string())
}

fn owned(value: &str) -> OwnedValue {
    OwnedValue::try_from(zvariant::Value::from(value.to_string()))
        .expect("strings never carry file descriptors")
}

fn existing_path(reference: &PartitionRef) -> Result<&str, String> {
    match reference {
        PartitionRef::Existing { path, .. } => Ok(path),
        PartitionRef::Planned { .. } => {
            Err("cannot delete a partition that has not been created".into())
        }
    }
}

fn resolve_ref(
    reference: &PartitionRef,
    planned: &HashMap<String, String>,
) -> Result<String, String> {
    match reference {
        PartitionRef::Existing { path, .. } => Ok(path.clone()),
        PartitionRef::Planned { id } => planned
            .get(id)
            .cloned()
            .ok_or_else(|| format!("planned partition has no device yet: {id}")),
    }
}

fn build_mounts(
    assignments: &[MountAssignment],
    planned: &HashMap<String, String>,
) -> Result<libreadymade::backend::mounts::Mounts, String> {
    use libreadymade::backend::mounts::{Mount, Mounts};
    let mut mounts = Vec::new();
    for assignment in assignments {
        mounts.push(Mount::new(
            PathBuf::from(resolve_ref(&assignment.target, planned)?),
            PathBuf::from(&assignment.mount_point),
            "defaults".into(),
            Some(assignment.filesystem.clone()),
            None,
            (!assignment.label.is_empty()).then(|| assignment.label.clone()),
        ));
    }
    Ok(Mounts(mounts))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_regions_account_for_partitions() {
        let parts = vec![PartitionSnapshot {
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
            calculate_free_regions(200 * MIB, &parts)[0].size_bytes,
            99 * MIB
        );
    }

    #[test]
    fn mounted_nested_mapper_marks_whole_disk_in_use() {
        let node = serde_json::json!({
            "mountpoints": [],
            "children": [{
                "mountpoints": [],
                "children": [{ "mountpoints": ["/"] }]
            }]
        });
        assert!(node_in_use(&node));
    }
}
