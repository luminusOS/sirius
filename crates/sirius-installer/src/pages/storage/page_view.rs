//! Main storage page widgets.

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

    content.append(&disk_group(&state, sender));
    if state
        .selected
        .and_then(|index| state.disks.get(index))
        .is_some()
    {
        content.append(&mode_group(&state, sender));
    }

    adw::Clamp::builder()
        .maximum_size(720)
        .tightening_threshold(560)
        .child(&content)
        .build()
}

fn disk_group(
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
    } else if state.disks.is_empty() {
        let row = adw::ActionRow::new();
        row.set_title(tr(state.lang, "storage.none"));
        row.set_subtitle(tr(state.lang, "storage.none.desc"));
        group.add(&row);
    }

    let mut leader = None;
    for (index, disk) in state.disks.iter().enumerate() {
        let row = adw::ActionRow::new();
        row.set_title(&disk.model);
        row.set_subtitle(&format!(
            "{} • {} • {}{}",
            disk.path,
            format_size(disk.size_bytes),
            disk.table_type,
            if disk.in_use {
                format!(" • {}", tr(state.lang, "storage.in_use"))
            } else {
                String::new()
            }
        ));
        row.add_prefix(&gtk::Image::from_icon_name("drive-harddisk-symbolic"));
        let radio = gtk::CheckButton::new();
        radio.set_group(leader.as_ref());
        radio.set_active(state.selected == Some(index));
        radio.set_sensitive(!disk.in_use);
        let sender = sender.clone();
        radio.connect_toggled(move |button| {
            if button.is_active() {
                sender.input(StorageMsg::Selected(index));
            }
        });
        row.add_suffix(&radio);
        row.set_activatable_widget(Some(&radio));
        if leader.is_none() {
            leader = Some(radio);
        }
        group.add(&row);
    }
    group
}

fn mode_group(
    state: &PageView<'_>,
    sender: &ComponentSender<StoragePage>,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    group.set_title(tr(state.lang, "storage.mode"));

    let automatic = adw::ActionRow::new();
    automatic.set_title(tr(state.lang, "storage.automatic"));
    automatic.set_subtitle(tr(state.lang, "storage.automatic.desc"));
    let automatic_radio = gtk::CheckButton::new();
    automatic_radio.set_active(!state.manual);
    automatic.add_prefix(&automatic_radio);
    automatic.set_activatable_widget(Some(&automatic_radio));
    let page_sender = sender.clone();
    automatic_radio.connect_toggled(move |button| {
        if button.is_active() {
            page_sender.input(StorageMsg::SetManual(false));
        }
    });
    group.add(&automatic);

    let custom = adw::ActionRow::new();
    custom.set_title(tr(state.lang, "storage.manual"));
    custom.set_subtitle(tr(state.lang, "storage.manual.desc"));
    let custom_radio = gtk::CheckButton::new();
    custom_radio.set_group(Some(&automatic_radio));
    custom_radio.set_active(state.manual);
    custom.add_prefix(&custom_radio);
    custom.set_activatable_widget(Some(&custom_radio));
    let page_sender = sender.clone();
    custom_radio.connect_toggled(move |button| {
        if button.is_active() {
            page_sender.input(StorageMsg::SetManual(true));
        }
    });
    let open = gtk::Button::with_label(tr(state.lang, "storage.open_editor"));
    open.set_sensitive(state.manual);
    let page_sender = sender.clone();
    open.connect_clicked(move |_| page_sender.input(StorageMsg::OpenCustom));
    custom.add_suffix(&open);
    group.add(&custom);

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
    group
}
