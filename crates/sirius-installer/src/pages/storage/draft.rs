//! Pure editing model for custom partitioning.
//!
//! The UI owns a `PartitionDraft` while the modal is open. Mutations are
//! staged here and become an installer `PartitionPlan` only when the user
//! applies the modal.

use crate::backend::storage::{DiskSnapshot, FreeRegion};
use crate::config_model::{MountAssignment, PartitionOperation, PartitionPlan, PartitionRef};

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
            return Err("partition size must be at least 0.5 GiB".into());
        }
        if !matches!(self.filesystem.as_str(), "btrfs" | "ext4" | "vfat" | "swap") {
            return Err(format!("unsupported filesystem: {}", self.filesystem));
        }
        if !self.mount_point.is_empty() && !self.mount_point.starts_with('/') {
            return Err("mount point must be empty or an absolute path".into());
        }
        if self.filesystem == "swap" && !self.mount_point.is_empty() {
            return Err("swap partitions cannot have a mount point".into());
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
            return Err("partition plan no longer matches the selected disk".into());
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
            .ok_or_else(|| "the selected free region is no longer available".to_string())?;
        let requested = (spec.size_gib * GIB) as u64;
        if requested > free.size_bytes {
            return Err("partition size exceeds the available space".into());
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
            .ok_or_else(|| "partition no longer exists".to_string())?;
        if !partition.mountpoints.is_empty() {
            return Err("mounted partitions cannot be deleted".into());
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
            .ok_or_else(|| "planned partition no longer exists".to_string())?;

        let requested = (spec.size_gib * GIB) as u64;
        let max_size = self.max_planned_size(id, offset_bytes, current_size);
        if requested > max_size {
            return Err("partition size exceeds the available space".into());
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
            return Err("planned partition no longer exists".into());
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
            .ok_or_else(|| "partition no longer exists".into())
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
mod tests {
    use super::*;
    use crate::backend::storage::PartitionSnapshot;

    const GIB_BYTES: u64 = 1024 * 1024 * 1024;

    fn disk() -> DiskSnapshot {
        DiskSnapshot {
            path: "/dev/sda".into(),
            model: "Test disk".into(),
            size_bytes: 64 * GIB_BYTES,
            table_type: "GPT".into(),
            read_only: false,
            in_use: false,
            partitions: vec![PartitionSnapshot {
                path: "/dev/sda1".into(),
                start_bytes: GIB_BYTES,
                size_bytes: 4 * GIB_BYTES,
                filesystem: "ext4".into(),
                label: "old".into(),
                mountpoints: Vec::new(),
                gpt_type: String::new(),
                part_uuid: "uuid".into(),
            }],
            free_regions: vec![FreeRegion {
                offset_bytes: 5 * GIB_BYTES,
                size_bytes: 59 * GIB_BYTES,
            }],
        }
    }

    fn root_spec() -> PartitionSpec {
        PartitionSpec {
            size_gib: 30.0,
            filesystem: "btrfs".into(),
            mount_point: "/".into(),
            label: "root".into(),
        }
    }

    #[test]
    fn create_consumes_free_space_and_adds_mount() {
        let disk = disk();
        let mut draft = PartitionDraft::new(&disk, None).unwrap();
        draft.create(0, root_spec()).unwrap();
        assert_eq!(draft.plan.mounts.len(), 1);
        assert_eq!(
            draft.remaining_region(0).unwrap().size_bytes,
            29 * GIB_BYTES
        );
    }

    #[test]
    fn oversized_create_is_rejected_without_mutating_the_plan() {
        let disk = disk();
        let mut draft = PartitionDraft::new(&disk, None).unwrap();
        let mut spec = root_spec();
        spec.size_gib = 60.0;
        assert!(draft.create(0, spec).is_err());
        assert!(draft.plan.operations.is_empty());
    }

    #[test]
    fn edit_replaces_previous_format_and_mount() {
        let disk = disk();
        let mut draft = PartitionDraft::new(&disk, None).unwrap();
        draft.edit_existing(0, root_spec()).unwrap();
        let mut spec = root_spec();
        spec.filesystem = "ext4".into();
        draft.edit_existing(0, spec).unwrap();
        assert_eq!(draft.plan.operations.len(), 1);
        assert_eq!(draft.plan.mounts.len(), 1);
        assert!(matches!(
            &draft.plan.operations[0],
            PartitionOperation::Format { filesystem, .. } if filesystem == "ext4"
        ));
    }

    #[test]
    fn existing_and_planned_partitions_can_be_deleted() {
        let disk = disk();
        let mut draft = PartitionDraft::new(&disk, None).unwrap();
        draft.delete_existing(0).unwrap();
        assert!(matches!(
            draft.plan.operations.last(),
            Some(PartitionOperation::Delete { .. })
        ));

        draft.create(0, root_spec()).unwrap();
        let id = draft
            .plan
            .operations
            .iter()
            .find_map(|operation| match operation {
                PartitionOperation::Create { id, .. } => Some(id.clone()),
                _ => None,
            })
            .unwrap();
        draft.delete_planned(&id).unwrap();
        assert!(!draft.plan.operations.iter().any(|operation| {
            matches!(operation, PartitionOperation::Create { id: current, .. } if current == &id)
        }));
    }

    fn planned_id(draft: &PartitionDraft) -> String {
        draft
            .plan
            .operations
            .iter()
            .find_map(|operation| match operation {
                PartitionOperation::Create { id, .. } => Some(id.clone()),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn edit_planned_shrink_frees_up_remaining_region() {
        let disk = disk();
        let mut draft = PartitionDraft::new(&disk, None).unwrap();
        draft.create(0, root_spec()).unwrap();
        let id = planned_id(&draft);
        assert_eq!(
            draft.remaining_region(0).unwrap().size_bytes,
            29 * GIB_BYTES
        );

        let mut spec = root_spec();
        spec.size_gib = 20.0;
        draft.edit_planned(&id, spec).unwrap();

        assert_eq!(
            draft.remaining_region(0).unwrap().size_bytes,
            39 * GIB_BYTES
        );
        assert!(matches!(
            &draft.plan.operations[0],
            PartitionOperation::Create { size_bytes, .. } if *size_bytes == 20 * GIB_BYTES
        ));
        assert_eq!(draft.plan.mounts.len(), 1);
    }

    #[test]
    fn edit_planned_grow_into_free_space_succeeds() {
        let disk = disk();
        let mut draft = PartitionDraft::new(&disk, None).unwrap();
        draft.create(0, root_spec()).unwrap();
        let id = planned_id(&draft);

        let mut spec = root_spec();
        spec.size_gib = 59.0; // grow to fill the entire free region
        draft.edit_planned(&id, spec).unwrap();

        assert!(draft.remaining_region(0).is_none());
        assert!(matches!(
            &draft.plan.operations[0],
            PartitionOperation::Create { size_bytes, .. } if *size_bytes == 59 * GIB_BYTES
        ));
    }

    #[test]
    fn edit_planned_grow_beyond_available_space_is_rejected_without_mutating_the_plan() {
        let disk = disk();
        let mut draft = PartitionDraft::new(&disk, None).unwrap();
        draft.create(0, root_spec()).unwrap();
        let id = planned_id(&draft);
        let before = draft.plan.clone();

        let mut spec = root_spec();
        spec.size_gib = 60.0; // exceeds the 59 GiB free region
        assert!(draft.edit_planned(&id, spec).is_err());
        assert_eq!(draft.plan.operations, before.operations);
        assert_eq!(draft.plan.mounts, before.mounts);
    }

    #[test]
    fn edit_planned_on_missing_id_returns_not_found_error() {
        let disk = disk();
        let mut draft = PartitionDraft::new(&disk, None).unwrap();
        let err = draft
            .edit_planned("does-not-exist", root_spec())
            .unwrap_err();
        assert_eq!(err, "planned partition no longer exists");
    }

    #[test]
    fn edit_planned_growth_is_bounded_by_a_later_planned_create() {
        let disk = disk();
        let mut draft = PartitionDraft::new(&disk, None).unwrap();
        draft.create(0, root_spec()).unwrap();
        let first_id = planned_id(&draft);

        let mut home_spec = root_spec();
        home_spec.size_gib = 10.0;
        home_spec.mount_point = "/home".into();
        home_spec.label = "home".into();
        draft.create(0, home_spec).unwrap();

        // The first partition is immediately followed by the second, so it
        // cannot grow at all without overlapping it.
        let mut grow = root_spec();
        grow.size_gib = 31.0;
        assert!(draft.edit_planned(&first_id, grow).is_err());

        // Deleting the blocking partition frees up room to grow again.
        let second_id = draft
            .plan
            .operations
            .iter()
            .find_map(|operation| match operation {
                PartitionOperation::Create { id, size_bytes, .. }
                    if *size_bytes == 10 * GIB_BYTES =>
                {
                    Some(id.clone())
                }
                _ => None,
            })
            .unwrap();
        draft.delete_planned(&second_id).unwrap();

        let mut grow_more = root_spec();
        grow_more.size_gib = 40.0;
        draft.edit_planned(&first_id, grow_more).unwrap();
        assert!(matches!(
            draft.plan.operations.iter().find(|operation| matches!(
                operation,
                PartitionOperation::Create { id, .. } if id == &first_id
            )),
            Some(PartitionOperation::Create { size_bytes, .. }) if *size_bytes == 40 * GIB_BYTES
        ));
    }

    #[test]
    fn into_plan_commits_only_the_owned_draft() {
        let disk = disk();
        let committed = PartitionDraft::empty_plan(&disk);
        let mut draft = PartitionDraft::new(&disk, Some(&committed)).unwrap();
        draft.create(0, root_spec()).unwrap();
        assert!(committed.operations.is_empty());
        assert!(!draft.into_plan().operations.is_empty());
    }
}
