//! Partition editor modal: the disk-usage map plus the volumes/partitions
//! list with add/edit/delete actions. Reached from the main storage page via
//! "Open editor" while manual partitioning is active.

use super::draft::{remaining_region, PartitionDraft};
use super::{StorageMsg, StoragePage};
use crate::backend::storage::{format_size, DiskSnapshot, PartitionSnapshot};
use crate::config_model::{PartitionOperation, PartitionPlan, PartitionRef};
use crate::i18n::{tr, Lang};
use relm4::adw::prelude::*;
use relm4::{adw, gtk, ComponentSender};

/// Build the editor dialog's content. `draft` drives both the segment map
/// and the volumes list, and whether the "Discard changes" button appears;
/// `draft_error` takes precedence over the draft's own validation error, same
/// as the summary shown on the main page. The disk itself is fixed by the
/// main page's picker — Sirius only ever installs to one disk — so this only
/// shows which disk it is (see `disk_heading`), it doesn't let it be changed.
pub(super) fn build(
    disk: &DiskSnapshot,
    draft: Option<&PartitionDraft>,
    draft_error: Option<&str>,
    uefi: bool,
    sender: &ComponentSender<StoragePage>,
    lang: Lang,
) -> adw::ToolbarView {
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    let done = gtk::Button::with_label(tr(lang, "storage.done"));
    done.add_css_class("suggested-action");
    let page_sender = sender.clone();
    done.connect_clicked(move |_| page_sender.input(StorageMsg::CloseEditor));
    header.pack_end(&done);
    toolbar.add_top_bar(&header);

    let plan = draft.map(PartitionDraft::plan);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 20);
    content.add_css_class("storage-content");
    content.append(&disk_heading(disk, lang));
    content.append(&disk_map(disk, plan, lang));
    content.append(&volumes_header(disk, draft, lang, sender));
    content.append(&partition_list(disk, plan, sender, lang));

    let error = draft_error
        .map(str::to_string)
        .or_else(|| draft.and_then(|d| d.validate(uefi).err()));
    if let Some(error) = error {
        let warning = gtk::Label::new(Some(&error));
        warning.add_css_class("error");
        warning.set_halign(gtk::Align::Start);
        warning.set_wrap(true);
        content.append(&warning);
    }

    let clamp = adw::Clamp::builder()
        .maximum_size(720)
        .tightening_threshold(560)
        .child(&content)
        .build();
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&clamp)
        .build();
    toolbar.set_content(Some(&scroller));
    toolbar
}

/// Highlighted disk name at the top of the editor — a plain label, not a
/// selector. The disk is already fixed by the main page's picker and Sirius
/// only ever installs to one disk, so there is nothing to choose here.
fn disk_heading(disk: &DiskSnapshot, lang: Lang) -> gtk::Box {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 2);
    container.set_margin_start(12);
    container.set_margin_end(12);

    let caption = gtk::Label::new(Some(tr(lang, "storage.disk")));
    caption.add_css_class("dim-label");
    caption.add_css_class("caption");
    caption.set_halign(gtk::Align::Start);
    container.append(&caption);

    let name = gtk::Label::new(Some(&format!(
        "{} ({} — {})",
        disk.path,
        disk.model,
        format_size(disk.size_bytes)
    )));
    name.add_css_class("title-2");
    name.set_halign(gtk::Align::Start);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    container.append(&name);

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

    // No inter-child spacing: adjacent segments should sit flush against
    // each other so the bar reads as one continuous strip divided into
    // colored blocks, not a row of separate rounded pills with the
    // `.disk-map` background showing through the gaps.
    let map = gtk::Box::new(gtk::Orientation::Horizontal, 0);
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
        edit.set_sensitive(!pending_delete);
        edit.set_tooltip_text(Some(tr(lang, "storage.edit")));
        let page_sender = sender.clone();
        edit.connect_clicked(move |_| page_sender.input(StorageMsg::OpenEdit(index)));
        row.add_suffix(&edit);
        let delete = gtk::Button::from_icon_name("user-trash-symbolic");
        delete.add_css_class("flat");
        delete.set_tooltip_text(Some(tr(lang, "storage.delete")));
        delete.set_sensitive(partition.mountpoints.is_empty() && !pending_delete);
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
        add.set_sensitive(remaining.is_some());
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
    // The CSS `min-width`/`min-height` are only a floor; inside an
    // ActionRow's prefix slot the box otherwise stretches to the row's full
    // height. Force a fixed size and center it so it renders as a small
    // rounded square regardless of row height.
    indicator.set_size_request(20, 20);
    indicator.set_valign(gtk::Align::Center);
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
