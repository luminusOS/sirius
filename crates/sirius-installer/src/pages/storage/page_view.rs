//! Main storage page widgets: disk picker, segment map, mode toggle and the
//! inline volumes/partitions list (formerly a separate modal editor).

use super::draft::{remaining_region, PartitionDraft};
use super::{StorageMsg, StoragePage};
use crate::backend::storage::{format_size, DiskSnapshot, PartitionSnapshot};
use crate::config_model::{PartitionOperation, PartitionPlan, PartitionRef};
use crate::i18n::{tr, Lang};
use relm4::adw::prelude::*;
use relm4::{adw, gtk, ComponentSender};

pub(super) struct PageView<'a> {
    pub disks: &'a [DiskSnapshot],
    pub selected: Option<usize>,
    pub manual: bool,
    pub encrypt: bool,
    pub tpm: bool,
    pub draft: Option<&'a PartitionDraft>,
    pub draft_error: Option<&'a str>,
    pub uefi: bool,
    pub error: Option<&'a str>,
    pub lang: Lang,
}

pub(super) fn build(state: PageView<'_>, sender: &ComponentSender<StoragePage>) -> adw::Clamp {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
    content.set_hexpand(true);
    content.add_css_class("storage-content");

    let icon = gtk::Image::from_icon_name("drive-multidisk-symbolic");
    icon.set_pixel_size(96);
    icon.set_halign(gtk::Align::Center);
    icon.set_margin_top(24);
    content.append(&icon);

    let title = gtk::Label::new(Some(tr(state.lang, "storage.title")));
    title.add_css_class("title-1");
    title.set_halign(gtk::Align::Center);
    content.append(&title);

    let description = gtk::Label::new(Some(tr(state.lang, "storage.desc")));
    description.add_css_class("dim-label");
    description.set_halign(gtk::Align::Center);
    description.set_justify(gtk::Justification::Center);
    description.set_max_width_chars(64);
    description.set_wrap(true);
    content.append(&description);

    content.append(&disk_selector(&state, sender));

    if let Some(disk) = state.selected.and_then(|index| state.disks.get(index)) {
        let plan = state.draft.map(PartitionDraft::plan);

        content.append(&disk_map(disk, plan, state.lang));
        content.append(&mode_row(&state, sender));
        content.append(&volumes_header(disk, state.draft, state.lang, sender));
        content.append(&partition_list(
            disk,
            plan,
            state.manual,
            sender,
            state.lang,
        ));

        if state.manual {
            let error = state
                .draft_error
                .map(str::to_string)
                .or_else(|| state.draft.and_then(|d| d.validate(state.uefi).err()));
            if let Some(error) = error {
                let warning = gtk::Label::new(Some(&error));
                warning.add_css_class("error");
                warning.set_halign(gtk::Align::Start);
                warning.set_wrap(true);
                content.append(&warning);
            }
        }
    }

    adw::Clamp::builder()
        .maximum_size(720)
        .tightening_threshold(560)
        .child(&content)
        .build()
}

fn disk_selector(
    state: &PageView<'_>,
    sender: &ComponentSender<StoragePage>,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    group.set_title(tr(state.lang, "storage.disk"));

    if let Some(error) = state.error {
        let row = adw::ActionRow::new();
        row.set_title(tr(state.lang, "storage.unavailable"));
        row.set_subtitle(error);
        group.add(&row);
        return group;
    }

    // Only disks that are not already in use can be selected. Keep a map from
    // the ComboRow's position back to the index in `state.disks` since these
    // may not line up once in-use disks are skipped.
    let available: Vec<usize> = state
        .disks
        .iter()
        .enumerate()
        .filter(|(_, disk)| !disk.in_use)
        .map(|(index, _)| index)
        .collect();

    if available.is_empty() {
        let row = adw::ActionRow::new();
        row.set_title(tr(state.lang, "storage.none"));
        row.set_subtitle(tr(state.lang, "storage.none.desc"));
        group.add(&row);
        return group;
    }

    let labels: Vec<String> = available
        .iter()
        .map(|&index| {
            let disk = &state.disks[index];
            format!(
                "{} ({} — {})",
                disk.path,
                disk.model,
                format_size(disk.size_bytes)
            )
        })
        .collect();
    let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let model = gtk::StringList::new(&label_refs);

    let combo = adw::ComboRow::new();
    combo.set_title(tr(state.lang, "storage.disk"));
    combo.add_prefix(&gtk::Image::from_icon_name("drive-harddisk-symbolic"));
    combo.set_model(Some(&model));

    if let Some(selected) = state.selected {
        if let Some(position) = available.iter().position(|&index| index == selected) {
            combo.set_selected(position as u32);
            combo.set_subtitle(&state.disks[selected].table_type.to_ascii_uppercase());
        }
    }

    let page_sender = sender.clone();
    let index_map = available.clone();
    combo.connect_selected_notify(move |row| {
        let position = row.selected() as usize;
        if let Some(&original_index) = index_map.get(position) {
            page_sender.input(StorageMsg::Selected(original_index));
        }
    });

    group.add(&combo);
    group
}

fn mode_row(state: &PageView<'_>, sender: &ComponentSender<StoragePage>) -> gtk::Box {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 6);

    let group = adw::PreferencesGroup::new();
    let automatic = adw::SwitchRow::new();
    automatic.set_title(tr(state.lang, "storage.automatic_mode"));
    automatic.set_subtitle(tr(state.lang, "storage.automatic_mode.desc"));
    automatic.set_active(!state.manual);
    let page_sender = sender.clone();
    automatic.connect_active_notify(move |row| {
        page_sender.input(StorageMsg::SetManual(!row.is_active()));
    });
    group.add(&automatic);

    if !state.manual {
        let encrypt = adw::SwitchRow::new();
        encrypt.set_title(tr(state.lang, "partition.encrypt"));
        encrypt.set_active(state.encrypt);
        let page_sender = sender.clone();
        encrypt.connect_active_notify(move |row| {
            page_sender.input(StorageMsg::ToggleEncrypt(row.is_active()))
        });
        group.add(&encrypt);

        let tpm = adw::SwitchRow::new();
        tpm.set_title(tr(state.lang, "partition.tpm"));
        tpm.set_sensitive(state.encrypt);
        tpm.set_active(state.tpm);
        let page_sender = sender.clone();
        tpm.connect_active_notify(move |row| {
            page_sender.input(StorageMsg::ToggleTpm(row.is_active()))
        });
        group.add(&tpm);
    }

    container.append(&group);

    if !state.manual {
        let notice = gtk::Label::new(Some(tr(state.lang, "storage.erase_notice")));
        notice.add_css_class("dim-label");
        notice.add_css_class("caption");
        notice.set_halign(gtk::Align::Start);
        notice.set_margin_start(12);
        container.append(&notice);
    }

    container
}

fn volumes_header(
    disk: &DiskSnapshot,
    draft: Option<&PartitionDraft>,
    lang: Lang,
    sender: &ComponentSender<StoragePage>,
) -> gtk::Box {
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.set_margin_start(12);
    header.set_margin_end(12);
    let title = gtk::Label::new(Some(tr(lang, "storage.volumes")));
    title.add_css_class("heading");
    title.add_css_class("dim-label");
    title.set_hexpand(true);
    title.set_halign(gtk::Align::Start);
    header.append(&title);

    let dirty = draft.is_some_and(|draft| !draft.plan().operations.is_empty());
    if dirty {
        let discard = gtk::Button::with_label(tr(lang, "storage.discard"));
        discard.add_css_class("flat");
        let page_sender = sender.clone();
        discard.connect_clicked(move |_| page_sender.input(StorageMsg::ResetDraft));
        header.append(&discard);
    }

    let table = gtk::Label::new(Some(&format!(
        "{}: {}",
        tr(lang, "storage.table"),
        disk.table_type.to_ascii_uppercase()
    )));
    table.add_css_class("storage-badge");
    header.append(&table);
    header
}

fn disk_map(disk: &DiskSnapshot, plan: Option<&PartitionPlan>, lang: Lang) -> gtk::Box {
    struct Segment {
        offset: u64,
        size: u64,
        label: String,
        class: &'static str,
        tooltip: String,
        pending_delete: bool,
    }

    let map = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    map.add_css_class("disk-map");
    let mut segments = Vec::new();
    for (index, partition) in disk.partitions.iter().enumerate() {
        let pending_delete = plan.is_some_and(|plan| {
            plan.operations.iter().any(|operation| {
                matches!(operation, PartitionOperation::Delete {
                target: PartitionRef::Existing { path, .. }
            } if path == &partition.path)
            })
        });
        segments.push(Segment {
            offset: partition.start_bytes,
            size: partition.size_bytes,
            label: if partition.label.is_empty() {
                partition.path.clone()
            } else {
                partition.label.clone()
            },
            class: filesystem_class(&partition.filesystem),
            tooltip: format!(
                "{} • {} • #{}",
                partition.path,
                format_size(partition.size_bytes),
                index + 1
            ),
            pending_delete,
        });
    }
    if let Some(plan) = plan {
        for operation in &plan.operations {
            if let PartitionOperation::Create {
                offset_bytes,
                size_bytes,
                filesystem,
                label,
                ..
            } = operation
            {
                segments.push(Segment {
                    offset: *offset_bytes,
                    size: *size_bytes,
                    label: if label.is_empty() {
                        "Sirius".into()
                    } else {
                        label.clone()
                    },
                    class: filesystem_class(filesystem),
                    tooltip: format!(
                        "{} • {}",
                        tr(lang, "storage.pending"),
                        format_size(*size_bytes)
                    ),
                    pending_delete: false,
                });
            }
        }
    }
    for index in 0..disk.free_regions.len() {
        if let Some(free) = remaining_region(disk, plan, index) {
            segments.push(Segment {
                offset: free.offset_bytes,
                size: free.size_bytes,
                label: tr(lang, "storage.free").into(),
                class: "partition-free",
                tooltip: format!(
                    "{} • {}",
                    tr(lang, "storage.free"),
                    format_size(free.size_bytes)
                ),
                pending_delete: false,
            });
        }
    }
    segments.sort_by_key(|segment| segment.offset);

    for item in segments {
        let segment = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        segment.add_css_class("partition-segment");
        segment.set_tooltip_text(Some(&item.tooltip));
        segment.set_width_request(
            ((item.size as f64 / disk.size_bytes as f64) * 700.0).max(18.0) as i32,
        );
        segment.add_css_class(item.class);
        if item.pending_delete {
            segment.add_css_class("pending-delete");
        }
        let label = gtk::Label::new(Some(&item.label));
        label.set_hexpand(true);
        label.set_halign(gtk::Align::Center);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        segment.append(&label);
        map.append(&segment);
    }
    map
}

fn partition_list(
    disk: &DiskSnapshot,
    plan: Option<&PartitionPlan>,
    manual: bool,
    sender: &ComponentSender<StoragePage>,
    lang: Lang,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    for (index, partition) in disk.partitions.iter().enumerate() {
        let row = adw::ActionRow::new();
        row.set_title(&format!(
            "{} ({})",
            partition.path,
            partition_description(partition, lang)
        ));
        add_indicator(&row, filesystem_class(&partition.filesystem));
        let mount = partition
            .mountpoints
            .first()
            .map(String::as_str)
            .unwrap_or(tr(lang, "storage.not_mounted"));
        row.set_subtitle(&format!(
            "{} • {}",
            if partition.filesystem.is_empty() {
                tr(lang, "storage.unknown")
            } else {
                &partition.filesystem
            },
            mount
        ));
        add_size(&row, partition.size_bytes);
        let pending_delete = plan.is_some_and(|plan| {
            plan.operations.iter().any(|operation| {
                matches!(operation, PartitionOperation::Delete {
                target: PartitionRef::Existing { path, .. }
            } if path == &partition.path)
            })
        });
        let edit = gtk::Button::from_icon_name("document-edit-symbolic");
        edit.add_css_class("flat");
        edit.set_sensitive(manual && !pending_delete);
        edit.set_tooltip_text(Some(tr(lang, "storage.edit")));
        let page_sender = sender.clone();
        edit.connect_clicked(move |_| page_sender.input(StorageMsg::OpenEdit(index)));
        row.add_suffix(&edit);
        let delete = gtk::Button::from_icon_name("user-trash-symbolic");
        delete.add_css_class("flat");
        delete.set_tooltip_text(Some(tr(lang, "storage.delete")));
        delete.set_sensitive(manual && partition.mountpoints.is_empty() && !pending_delete);
        let page_sender = sender.clone();
        delete.connect_clicked(move |_| page_sender.input(StorageMsg::Delete(index)));
        row.add_suffix(&delete);
        if pending_delete {
            row.add_css_class("pending-delete");
        }
        group.add(&row);
    }

    for (index, free) in disk.free_regions.iter().enumerate() {
        let remaining = remaining_region(disk, plan, index);
        let used = remaining
            .as_ref()
            .is_some_and(|region| region.size_bytes < free.size_bytes);
        let row = adw::ActionRow::new();
        add_indicator(&row, "partition-free");
        row.set_title(if used {
            tr(lang, "storage.pending")
        } else {
            tr(lang, "storage.free")
        });
        row.set_subtitle(tr(lang, "storage.unformatted"));
        add_size(
            &row,
            remaining.as_ref().map_or(0, |region| region.size_bytes),
        );
        let add = gtk::Button::from_icon_name("list-add-symbolic");
        add.add_css_class("flat");
        add.set_tooltip_text(Some(tr(lang, "storage.create")));
        add.set_sensitive(manual && remaining.is_some());
        let page_sender = sender.clone();
        add.connect_clicked(move |_| page_sender.input(StorageMsg::OpenCreate(index)));
        row.add_suffix(&add);
        group.add(&row);
    }

    if let Some(plan) = plan {
        for operation in &plan.operations {
            let PartitionOperation::Create {
                id,
                size_bytes,
                filesystem,
                label,
                ..
            } = operation
            else {
                continue;
            };
            let row = adw::ActionRow::new();
            add_indicator(&row, filesystem_class(filesystem));
            row.set_title(if label.is_empty() {
                tr(lang, "storage.new_partition")
            } else {
                label
            });
            let mount = plan
                .mounts
                .iter()
                .find(|assignment| {
                    matches!(&assignment.target, PartitionRef::Planned { id: current } if current == id)
                })
                .map(|assignment| assignment.mount_point.as_str())
                .unwrap_or(tr(lang, "storage.not_mounted"));
            row.set_subtitle(&format!("{} • {}", filesystem, mount));
            add_size(&row, *size_bytes);
            row.add_css_class("accent");
            let edit = gtk::Button::from_icon_name("document-edit-symbolic");
            edit.add_css_class("flat");
            edit.set_sensitive(manual);
            edit.set_tooltip_text(Some(tr(lang, "storage.edit")));
            let page_sender = sender.clone();
            let edit_id = id.clone();
            edit.connect_clicked(move |_| {
                page_sender.input(StorageMsg::OpenEditPlanned(edit_id.clone()))
            });
            row.add_suffix(&edit);
            let remove = gtk::Button::from_icon_name("user-trash-symbolic");
            remove.add_css_class("flat");
            remove.set_tooltip_text(Some(tr(lang, "storage.delete")));
            remove.set_sensitive(manual);
            let page_sender = sender.clone();
            let id = id.clone();
            remove
                .connect_clicked(move |_| page_sender.input(StorageMsg::DeletePlanned(id.clone())));
            row.add_suffix(&remove);
            group.add(&row);
        }
    }
    group
}

fn add_indicator(row: &adw::ActionRow, class: &str) {
    let indicator = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    indicator.add_css_class("partition-indicator");
    indicator.add_css_class(class);
    row.add_prefix(&indicator);
}

fn add_size(row: &adw::ActionRow, bytes: u64) {
    let size = gtk::Label::new(Some(&format_size(bytes)));
    size.add_css_class("dim-label");
    size.add_css_class("partition-size");
    row.add_suffix(&size);
}

fn partition_description(partition: &PartitionSnapshot, lang: Lang) -> String {
    if partition
        .mountpoints
        .iter()
        .any(|mount| mount == "/boot/efi")
        || matches!(partition.filesystem.as_str(), "vfat" | "fat32")
    {
        tr(lang, "storage.efi_partition").into()
    } else if partition.mountpoints.iter().any(|mount| mount == "/") {
        tr(lang, "storage.root_partition").into()
    } else if partition.filesystem == "swap" {
        tr(lang, "storage.swap_partition").into()
    } else if !partition.label.is_empty() {
        partition.label.clone()
    } else if !partition.filesystem.is_empty() {
        partition.filesystem.to_ascii_uppercase()
    } else {
        tr(lang, "storage.unknown").into()
    }
}

fn filesystem_class(filesystem: &str) -> &'static str {
    match filesystem {
        "vfat" | "fat32" => "partition-efi",
        "swap" => "partition-swap",
        "ntfs" => "partition-ntfs",
        "btrfs" | "ext4" => "partition-linux",
        _ => "partition-free",
    }
}
