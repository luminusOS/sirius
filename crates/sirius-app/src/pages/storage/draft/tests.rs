use super::*;
use sirius_backend::storage::PartitionSnapshot;

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
            PartitionOperation::Create { id, size_bytes, .. } if *size_bytes == 10 * GIB_BYTES => {
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
