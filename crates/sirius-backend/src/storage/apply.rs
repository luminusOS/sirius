//! Privileged application of a confirmed partition plan through UDisks2.

use super::{DiskSnapshot, scan_disks};
use sirius_core::{MountAssignment, PartitionOperation, PartitionPlan, PartitionRef};
use std::collections::HashMap;
use std::path::PathBuf;
use zbus::blocking::Connection;
use zvariant::{OwnedObjectPath, OwnedValue};

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
        PartitionOperation::Delete { target } | PartitionOperation::Format { target, .. } => {
            vec![target]
        }
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
