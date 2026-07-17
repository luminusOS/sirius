//! Create/edit partition form.

use super::draft::PartitionSpec;
use super::{StorageMsg, StoragePage};
use crate::backend::storage::{FreeRegion, PartitionSnapshot};
use crate::i18n::{tr, Lang};
use relm4::adw::prelude::*;
use relm4::{adw, gtk, ComponentSender};

pub(super) enum DialogTarget {
    Create(usize, FreeRegion),
    Edit(usize),
}

pub(super) fn show(
    parent: &gtk::Widget,
    sender: &ComponentSender<StoragePage>,
    target: DialogTarget,
    partition: Option<&PartitionSnapshot>,
    lang: Lang,
) {
    let dialog = adw::Dialog::builder()
        .title(if partition.is_some() {
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
        if partition.is_some() {
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
    let existing_fs = partition.map(|p| p.filesystem.as_str()).unwrap_or("btrfs");
    filesystem.set_selected(
        ["btrfs", "ext4", "vfat", "swap"]
            .iter()
            .position(|fs| *fs == existing_fs)
            .unwrap_or(0) as u32,
    );
    form.add(&filesystem);

    let maximum = match &target {
        DialogTarget::Create(_, free) => free.size_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
        DialogTarget::Edit(_) => partition
            .map(|p| p.size_bytes as f64 / (1024.0 * 1024.0 * 1024.0))
            .unwrap_or(1.0),
    };
    let size = adw::SpinRow::with_range(0.5, maximum.max(0.5), 0.5);
    size.set_title(tr(lang, "storage.size"));
    size.set_value(maximum.max(0.5));
    size.set_sensitive(matches!(target, DialogTarget::Create(_, _)));
    form.add(&size);

    let mount = adw::EntryRow::new();
    mount.set_title(tr(lang, "storage.mount"));
    mount.set_text(
        partition
            .and_then(|p| p.mountpoints.first())
            .map(String::as_str)
            .unwrap_or(""),
    );
    form.add(&mount);

    let label = adw::EntryRow::new();
    label.set_title(tr(lang, "storage.label"));
    label.set_text(partition.map(|p| p.label.as_str()).unwrap_or(""));
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
        };
        page_sender.input(message);
        submit_dialog.close();
    });
    dialog.present(Some(parent));
}
