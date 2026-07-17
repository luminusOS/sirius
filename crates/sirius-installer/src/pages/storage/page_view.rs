//! Main storage page widgets: disk picker, mode toggle, and the entry point
//! into the partition editor modal (see `editor_view`).

use super::draft::PartitionDraft;
use super::{StorageMsg, StoragePage};
use crate::backend::storage::{format_size, DiskSnapshot};
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

    content.append(&disk_selector(
        state.disks,
        state.selected,
        state.error,
        state.lang,
        sender,
    ));

    if let Some(disk) = state.selected.and_then(|index| state.disks.get(index)) {
        content.append(&mode_row(&state, sender));

        if state.manual {
            content.append(&open_editor_row(&state, disk, sender));

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

/// The disk-picker `PreferencesGroup`. Shared between the main page and the
/// editor modal (see `editor_view::build`) so a disk can be switched from
/// either place — takes plain values rather than `&PageView` since the
/// editor modal doesn't have (or need) a full `PageView`.
pub(super) fn disk_selector(
    disks: &[DiskSnapshot],
    selected: Option<usize>,
    error: Option<&str>,
    lang: Lang,
    sender: &ComponentSender<StoragePage>,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    group.set_title(tr(lang, "storage.disk"));

    if let Some(error) = error {
        let row = adw::ActionRow::new();
        row.set_title(tr(lang, "storage.unavailable"));
        row.set_subtitle(error);
        group.add(&row);
        return group;
    }

    // Only disks that are not already in use can be selected. Keep a map from
    // the ComboRow's position back to the index in `disks` since these may
    // not line up once in-use disks are skipped.
    let available: Vec<usize> = disks
        .iter()
        .enumerate()
        .filter(|(_, disk)| !disk.in_use)
        .map(|(index, _)| index)
        .collect();

    if available.is_empty() {
        let row = adw::ActionRow::new();
        row.set_title(tr(lang, "storage.none"));
        row.set_subtitle(tr(lang, "storage.none.desc"));
        group.add(&row);
        return group;
    }

    let labels: Vec<String> = available
        .iter()
        .map(|&index| {
            let disk = &disks[index];
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
    combo.set_title(tr(lang, "storage.disk"));
    combo.add_prefix(&gtk::Image::from_icon_name("drive-harddisk-symbolic"));
    combo.set_model(Some(&model));

    if let Some(selected) = selected {
        if let Some(position) = available.iter().position(|&index| index == selected) {
            combo.set_selected(position as u32);
            combo.set_subtitle(&disks[selected].table_type.to_ascii_uppercase());
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

/// Entry point into the partition editor modal: a summary row plus a button
/// that opens `editor_view`'s dialog with the disk-usage map and the full
/// volumes/partitions list.
fn open_editor_row(
    state: &PageView<'_>,
    disk: &DiskSnapshot,
    sender: &ComponentSender<StoragePage>,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    let row = adw::ActionRow::new();
    row.set_title(tr(state.lang, "storage.volumes"));
    row.set_subtitle(&format!(
        "{}: {}",
        tr(state.lang, "storage.table"),
        disk.table_type.to_ascii_uppercase()
    ));

    let open = gtk::Button::with_label(tr(state.lang, "storage.open_editor"));
    open.add_css_class("flat");
    open.set_valign(gtk::Align::Center);
    let page_sender = sender.clone();
    open.connect_clicked(move |_| page_sender.input(StorageMsg::OpenEditor));
    row.add_suffix(&open);
    row.set_activatable_widget(Some(&open));

    group.add(&row);
    group
}
