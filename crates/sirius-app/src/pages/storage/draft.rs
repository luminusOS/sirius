//! Pure editing model for custom partitioning.
//!
//! The UI owns a `PartitionDraft` while the modal is open. Mutations are
//! staged here and become an installer `PartitionPlan` only when the user
//! applies the modal.

use gettextrs::gettext;
use sirius_backend::storage::{DiskSnapshot, FreeRegion};
use sirius_core::{MountAssignment, PartitionOperation, PartitionPlan, PartitionRef};

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
const MIN_FREE_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct PartitionSpec {
    pub size_gib: f64,
    pub filesystem: String,
    pub mount_point: String,
    pub label: String,
}

impl PartitionSpec {
    fn validate(&self, allow_resize: bool) -> Result<(), String> {
        if allow_resize && (!self.size_gib.is_finite() || self.size_gib < 0.5) {
            return Err(gettext("partition size must be at least 0.5 GiB"));
        }
        if !matches!(self.filesystem.as_str(), "btrfs" | "ext4" | "vfat" | "swap") {
            return Err(gettext("unsupported filesystem: {filesystem}")
                .replace("{filesystem}", &self.filesystem));
        }
        if !self.mount_point.is_empty() && !self.mount_point.starts_with('/') {
            return Err(gettext("mount point must be empty or an absolute path"));
        }
        if self.filesystem == "swap" && !self.mount_point.is_empty() {
            return Err(gettext("swap partitions cannot have a mount point"));
        }
        Ok(())
    }
}

/// Prefill data for the edit-planned dialog: the planned partition's current
/// values plus how far it could still grow. See `PartitionDraft::planned_details`.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedDetails {
    pub filesystem: String,
    pub size_bytes: u64,
    pub max_size_bytes: u64,
    pub mount_point: String,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct PartitionDraft {
    disk: DiskSnapshot,
    plan: PartitionPlan,
}

impl PartitionDraft {
    pub fn new(disk: &DiskSnapshot, plan: Option<&PartitionPlan>) -> Result<Self, String> {
        let plan = plan.cloned().unwrap_or_else(|| Self::empty_plan(disk));
        if plan.disk_path != disk.path || plan.disk_size_bytes != disk.size_bytes {
            return Err(gettext(
                "partition plan no longer matches the selected disk",
            ));
        }
        Ok(Self {
            disk: disk.clone(),
            plan,
        })
    }

    pub fn empty_plan(disk: &DiskSnapshot) -> PartitionPlan {
        PartitionPlan {
            disk_path: disk.path.clone(),
            disk_size_bytes: disk.size_bytes,
            table_type: disk.table_type.to_ascii_lowercase(),
            operations: Vec::new(),
            mounts: Vec::new(),
        }
    }

    pub fn plan(&self) -> &PartitionPlan {
        &self.plan
    }

    // Kept as part of the pure draft API and exercised by tests; no production caller since the draft now stays alive across messages.
    #[allow(dead_code)]
    pub fn into_plan(self) -> PartitionPlan {
        self.plan
    }

    pub fn validate(&self, uefi: bool) -> Result<(), String> {
        self.plan.validate(uefi)
    }

    pub fn remaining_region(&self, index: usize) -> Option<FreeRegion> {
        remaining_region(&self.disk, Some(&self.plan), index)
    }

    pub fn create(&mut self, region: usize, spec: PartitionSpec) -> Result<(), String> {
        spec.validate(true)?;
        let free = self
            .remaining_region(region)
            .ok_or_else(|| gettext("the selected free region is no longer available"))?;
        let requested = (spec.size_gib * GIB) as u64;
        if requested > free.size_bytes {
            return Err(gettext("partition size exceeds the available space"));
        }

        let id = uuid::Uuid::new_v4().to_string();
        let target = PartitionRef::Planned { id: id.clone() };
        self.plan.operations.push(PartitionOperation::Create {
            id,
            offset_bytes: free.offset_bytes,
            size_bytes: requested,
            gpt_type: gpt_type(&spec).into(),
            name: if spec.label.is_empty() {
                "Sirius".into()
            } else {
                spec.label.clone()
            },
            filesystem: spec.filesystem.clone(),
            label: spec.label.clone(),
        });
        self.add_mount(target, &spec);
        Ok(())
    }

    pub fn edit_existing(&mut self, index: usize, spec: PartitionSpec) -> Result<(), String> {
        spec.validate(false)?;
        let target = self.existing_ref(index)?;
        self.plan
            .operations
            .retain(|operation| !operation_targets(operation, &target));
        self.plan
            .mounts
            .retain(|mount| mount.target != target && mount.mount_point != spec.mount_point);
        self.plan.operations.push(PartitionOperation::Format {
            target: target.clone(),
            filesystem: spec.filesystem.clone(),
            label: spec.label.clone(),
        });
        self.add_mount(target, &spec);
        Ok(())
    }

    pub fn delete_existing(&mut self, index: usize) -> Result<(), String> {
        let partition = self
            .disk
            .partitions
            .get(index)
            .ok_or_else(|| gettext("partition no longer exists"))?;
        if !partition.mountpoints.is_empty() {
            return Err(gettext("mounted partitions cannot be deleted"));
        }
        let target = self.existing_ref(index)?;
        self.plan
            .operations
            .retain(|operation| !operation_targets(operation, &target));
        self.plan.mounts.retain(|mount| mount.target != target);
        self.plan
            .operations
            .push(PartitionOperation::Delete { target });
        Ok(())
    }

    pub fn edit_planned(&mut self, id: &str, spec: PartitionSpec) -> Result<(), String> {
        spec.validate(true)?;
        let (offset_bytes, current_size) = self
            .plan
            .operations
            .iter()
            .find_map(|operation| match operation {
                PartitionOperation::Create {
                    id: current,
                    offset_bytes,
                    size_bytes,
                    ..
                } if current == id => Some((*offset_bytes, *size_bytes)),
                _ => None,
            })
            .ok_or_else(|| gettext("planned partition no longer exists"))?;

        let requested = (spec.size_gib * GIB) as u64;
        let max_size = self.max_planned_size(id, offset_bytes, current_size);
        if requested > max_size {
            return Err(gettext("partition size exceeds the available space"));
        }

        let target = PartitionRef::Planned { id: id.to_string() };
        self.plan.operations.retain(|operation| {
            !matches!(operation, PartitionOperation::Create { id: current, .. } if current == id)
        });
        self.plan.mounts.retain(|mount| {
            !matches!(&mount.target, PartitionRef::Planned { id: current } if current == id)
        });
        self.plan.operations.push(PartitionOperation::Create {
            id: id.to_string(),
            offset_bytes,
            size_bytes: requested,
            gpt_type: gpt_type(&spec).into(),
            name: if spec.label.is_empty() {
                "Sirius".into()
            } else {
                spec.label.clone()
            },
            filesystem: spec.filesystem.clone(),
            label: spec.label.clone(),
        });
        self.add_mount(target, &spec);
        Ok(())
    }

    /// Maximum size (in bytes) that the planned `Create` operation with `id`,
    /// currently at `offset_bytes`, could grow to without overlapping
    /// anything else. Mirrors `remaining_region()`'s "find the base free
    /// region this offset came from" framing, but the bound itself has to be
    /// the *nearest* thing that starts after `offset_bytes` — an existing
    /// partition, another planned `Create`, or the end of the original free
    /// region — rather than `remaining_region()`'s trailing-cursor value.
    /// Reusing `remaining_region()` directly does not work here: its cursor
    /// is the rightmost claimed edge across *all* creates in the region, so
    /// if another planned partition sits further down the same free region,
    /// removing our own op and asking `remaining_region()` for the result
    /// would report free space all the way to the end of the region,
    /// letting an edit grow straight through that later partition. Scanning
    /// for the nearest start strictly after `offset_bytes` avoids that.
    fn max_planned_size(&self, id: &str, offset_bytes: u64, current_size: u64) -> u64 {
        let end_bound = self
            .disk
            .free_regions
            .iter()
            .find(|region| {
                region.offset_bytes <= offset_bytes
                    && offset_bytes < region.offset_bytes.saturating_add(region.size_bytes)
            })
            .map(|region| region.offset_bytes.saturating_add(region.size_bytes))
            .unwrap_or_else(|| offset_bytes.saturating_add(current_size));

        let mut bound = end_bound;
        for partition in &self.disk.partitions {
            if partition.start_bytes > offset_bytes {
                bound = bound.min(partition.start_bytes);
            }
        }
        for operation in &self.plan.operations {
            if let PartitionOperation::Create {
                id: current,
                offset_bytes: other_offset,
                ..
            } = operation
                && current != id
                && *other_offset > offset_bytes
            {
                bound = bound.min(*other_offset);
            }
        }
        bound.saturating_sub(offset_bytes)
    }

    /// Current values and growth bound for a planned (not yet created)
    /// partition, used to prefill the edit-planned dialog. `None` if `id`
    /// does not match any planned `Create` operation.
    pub fn planned_details(&self, id: &str) -> Option<PlannedDetails> {
        let (offset_bytes, size_bytes, filesystem, label) =
            self.plan
                .operations
                .iter()
                .find_map(|operation| match operation {
                    PartitionOperation::Create {
                        id: current,
                        offset_bytes,
                        size_bytes,
                        filesystem,
                        label,
                        ..
                    } if current == id => Some((
                        *offset_bytes,
                        *size_bytes,
                        filesystem.clone(),
                        label.clone(),
                    )),
                    _ => None,
                })?;
        let max_size_bytes = self.max_planned_size(id, offset_bytes, size_bytes);
        let mount_point = self
            .plan
            .mounts
            .iter()
            .find(|mount| {
                matches!(&mount.target, PartitionRef::Planned { id: current } if current == id)
            })
            .map(|mount| mount.mount_point.clone())
            .unwrap_or_default();
        Some(PlannedDetails {
            filesystem,
            size_bytes,
            max_size_bytes,
            mount_point,
            label,
        })
    }

    pub fn delete_planned(&mut self, id: &str) -> Result<(), String> {
        let before = self.plan.operations.len();
        self.plan.operations.retain(|operation| {
            !matches!(operation, PartitionOperation::Create { id: current, .. } if current == id)
        });
        if before == self.plan.operations.len() {
            return Err(gettext("planned partition no longer exists"));
        }
        self.plan.mounts.retain(|mount| {
            !matches!(&mount.target, PartitionRef::Planned { id: current } if current == id)
        });
        Ok(())
    }

    fn existing_ref(&self, index: usize) -> Result<PartitionRef, String> {
        self.disk
            .partitions
            .get(index)
            .map(|partition| PartitionRef::Existing {
                path: partition.path.clone(),
                start_bytes: partition.start_bytes,
                size_bytes: partition.size_bytes,
                part_uuid: (!partition.part_uuid.is_empty()).then(|| partition.part_uuid.clone()),
            })
            .ok_or_else(|| gettext("partition no longer exists"))
    }

    fn add_mount(&mut self, target: PartitionRef, spec: &PartitionSpec) {
        if !spec.mount_point.is_empty() {
            self.plan.mounts.push(MountAssignment {
                target,
                mount_point: spec.mount_point.clone(),
                filesystem: spec.filesystem.clone(),
                label: spec.label.clone(),
            });
        }
    }
}

pub fn remaining_region(
    disk: &DiskSnapshot,
    plan: Option<&PartitionPlan>,
    index: usize,
) -> Option<FreeRegion> {
    let base = disk.free_regions.get(index)?;
    let end = base.offset_bytes.saturating_add(base.size_bytes);
    let mut cursor = base.offset_bytes;
    if let Some(plan) = plan {
        for operation in &plan.operations {
            if let PartitionOperation::Create {
                offset_bytes,
                size_bytes,
                ..
            } = operation
                && *offset_bytes >= base.offset_bytes
                && *offset_bytes < end
            {
                cursor = cursor.max(offset_bytes.saturating_add(*size_bytes));
            }
        }
    }
    (end > cursor.saturating_add(MIN_FREE_BYTES)).then_some(FreeRegion {
        offset_bytes: cursor,
        size_bytes: end - cursor,
    })
}

fn operation_targets(operation: &PartitionOperation, target: &PartitionRef) -> bool {
    matches!(operation,
        PartitionOperation::Delete { target: current }
        | PartitionOperation::Format { target: current, .. } if current == target)
}

fn gpt_type(spec: &PartitionSpec) -> &'static str {
    if spec.mount_point == "/boot/efi" {
        "c12a7328-f81f-11d2-ba4b-00a0c93ec93b"
    } else if spec.filesystem == "swap" {
        "0657fd6d-a4ab-43c4-84e5-0933c84b4f4f"
    } else {
        "0fc63daf-8483-4772-8e79-3d69d8477de4"
    }
}

#[cfg(test)]
mod tests;
