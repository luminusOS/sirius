//! Create/edit partition form.

use super::draft::PartitionSpec;
use super::{StorageMsg, StoragePage};
use crate::backend::storage::{FreeRegion, PartitionSnapshot};
use crate::i18n::{tr, Lang};
use relm4::adw::prelude::*;
use relm4::{adw, gtk, ComponentSender};

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

pub(super) enum DialogTarget {
    Create(usize, FreeRegion),
    Edit(usize),
    EditPlanned(String),
}

/// Prefill values for editing something that already has concrete
/// filesystem/mount/label data — either a real disk partition (`Existing`)
/// or a not-yet-created planned partition (`Planned`). Kept distinct from
/// `PartitionSnapshot` because a planned partition has no `path`/`part_uuid`/
/// `mountpoints` of its own, only what the user chose when creating it, plus
/// the size it is still free to grow into.
pub(super) enum EditSource<'a> {
    Existing(&'a PartitionSnapshot),
    Planned {
        filesystem: &'a str,
        size_bytes: u64,
        max_size_bytes: u64,
        mount_point: &'a str,
        label: &'a str,
    },
}

pub(super) fn show(
    parent: &gtk::Widget,
    sender: &ComponentSender<StoragePage>,
    target: DialogTarget,
    source: Option<EditSource<'_>>,
    lang: Lang,
) {
    let dialog = adw::Dialog::builder()
        .title(if source.is_some() {
            tr(lang, "storage.edit")
        } else {
            tr(lang, "storage.create")
        })
        .content_width(460)
        .build();
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    let cancel = gtk::Button::with_label(tr(lang, "confirm.cancel"));
    let affirmative = gtk::Button::with_label(tr(
        lang,
        if source.is_some() {
            "storage.save_changes"
        } else {
            "storage.create"
        },
    ));
    affirmative.add_css_class("suggested-action");
    header.pack_start(&cancel);
    header.pack_end(&affirmative);
    toolbar.add_top_bar(&header);

    let form = adw::PreferencesGroup::new();
    let filesystems = gtk::StringList::new(&["btrfs", "ext4", "vfat", "swap"]);
    let filesystem = adw::ComboRow::new();
    filesystem.set_title(tr(lang, "storage.filesystem"));
    filesystem.set_model(Some(&filesystems));
    let existing_fs = match &source {
        Some(EditSource::Existing(p)) => p.filesystem.as_str(),
        Some(EditSource::Planned { filesystem, .. }) => filesystem,
        None => "btrfs",
    };
    filesystem.set_selected(
        ["btrfs", "ext4", "vfat", "swap"]
            .iter()
            .position(|fs| *fs == existing_fs)
            .unwrap_or(0) as u32,
    );
    form.add(&filesystem);

    // The SpinRow's upper bound (`range_max`) and its prefilled starting
    // value (`initial_value`) coincide for Create (default to filling the
    // whole free region) and for editing an existing partition (size is
    // fixed, so both are its current size). Editing a planned partition is
    // the one case where they diverge: it starts at its current size but
    // can range up to however far it is still free to grow.
    let (range_max, initial_value) = match (&target, &source) {
        (DialogTarget::Create(_, free), _) => {
            let value = free.size_bytes as f64 / GIB;
            (value, value)
        }
        (DialogTarget::Edit(_), Some(EditSource::Existing(partition))) => {
            let value = partition.size_bytes as f64 / GIB;
            (value, value)
        }
        (
            DialogTarget::EditPlanned(_),
            Some(EditSource::Planned {
                size_bytes,
                max_size_bytes,
                ..
            }),
        ) => (*max_size_bytes as f64 / GIB, *size_bytes as f64 / GIB),
        _ => (1.0, 1.0),
    };
    let size = adw::SpinRow::with_range(0.5, range_max.max(0.5), 0.5);
    size.set_title(tr(lang, "storage.size"));
    size.set_value(initial_value.max(0.5));
    // Size is only fixed (non-editable) when editing a partition that is
    // already written to disk; both creating and editing a planned partition
    // allow picking a size since nothing has been written yet.
    size.set_sensitive(!matches!(target, DialogTarget::Edit(_)));
    form.add(&size);

    let mount = adw::EntryRow::new();
    mount.set_title(tr(lang, "storage.mount"));
    mount.set_text(match &source {
        Some(EditSource::Existing(p)) => p.mountpoints.first().map(String::as_str).unwrap_or(""),
        Some(EditSource::Planned { mount_point, .. }) => mount_point,
        None => "",
    });
    form.add(&mount);

    let label = adw::EntryRow::new();
    label.set_title(tr(lang, "storage.label"));
    label.set_text(match &source {
        Some(EditSource::Existing(p)) => p.label.as_str(),
        Some(EditSource::Planned { label, .. }) => label,
        None => "",
    });
    form.add(&label);

    let clamp = adw::Clamp::builder()
        .maximum_size(520)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .child(&form)
        .build();
    toolbar.set_content(Some(&clamp));
    dialog.set_child(Some(&toolbar));
    let cancel_dialog = dialog.clone();
    cancel.connect_clicked(move |_| {
        cancel_dialog.close();
    });
    let submit_dialog = dialog.clone();
    let page_sender = sender.clone();
    affirmative.connect_clicked(move |_| {
        let filesystem = filesystem
            .selected_item()
            .and_then(|item| item.downcast::<gtk::StringObject>().ok())
            .map(|item| item.string().to_string())
            .unwrap_or_else(|| "btrfs".into());
        let spec = PartitionSpec {
            size_gib: size.value(),
            filesystem,
            mount_point: mount.text().to_string(),
            label: label.text().to_string(),
        };
        let message = match &target {
            DialogTarget::Create(region, _) => StorageMsg::Create {
                region: *region,
                spec,
            },
            DialogTarget::Edit(partition) => StorageMsg::Edit {
                partition: *partition,
                spec,
            },
            DialogTarget::EditPlanned(id) => StorageMsg::EditPlanned {
                id: id.clone(),
                spec,
            },
        };
        page_sender.input(message);
        submit_dialog.close();
    });
    dialog.present(Some(parent));
}
