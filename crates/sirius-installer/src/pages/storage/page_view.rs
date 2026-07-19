//! Main storage page widgets: disk picker, partitioning-mode picker, and
//! the entry point into the partition editor modal (see `editor_view`).

use super::draft::PartitionDraft;
use super::{StorageMsg, StoragePage};
use crate::backend::storage::{DiskSnapshot, format_size};
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::{ComponentSender, adw, gtk};

pub(super) struct PageView<'a> {
    pub disks: &'a [DiskSnapshot],
    pub selected: Option<usize>,
    pub manual: bool,
    pub encrypt: bool,
    pub tpm: bool,
    pub encryption_passphrase: &'a str,
    pub encryption_passphrase_confirm: &'a str,
    pub draft: Option<&'a PartitionDraft>,
    pub draft_error: Option<&'a str>,
    pub uefi: bool,
    pub error: Option<&'a str>,
    /// Whether the partition-editor dialog is currently presented; used to
    /// suppress the duplicate validation error on the page behind it.
    pub editor_open: bool,
}

pub(super) fn build(state: PageView<'_>, sender: &ComponentSender<StoragePage>) -> adw::Clamp {
    // Compact header: no decorative hero icon. With the icon the page was
    // ~120px taller than the window's content area at the default 960x640,
    // pushing the partitioning-mode picker below the fold and forcing a
    // scroll to reach it.
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_hexpand(true);
    content.add_css_class("storage-content");

    let title = gtk::Label::new(Some(&gettext("Storage")));
    title.add_css_class("title-1");
    title.set_halign(gtk::Align::Center);
    title.set_margin_top(18);
    content.append(&title);

    let description = gtk::Label::new(Some(&gettext(
        "Choose a disk and how Sirius should use it. Changes are applied only after confirmation.",
    )));
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
        sender,
    ));

    if let Some(disk) = state.selected.and_then(|index| state.disks.get(index)) {
        content.append(&mode_selector(&state, sender));

        if state.manual {
            content.append(&open_editor_row(disk, sender));

            // The editor dialog shows this same error above its volumes
            // list; rendering it here too while the dialog is open makes
            // the message appear twice (once dimmed behind the modal).
            let error = state
                .draft_error
                .map(str::to_string)
                .or_else(|| state.draft.and_then(|d| d.validate(state.uefi).err()));
            if !state.editor_open {
                if let Some(error) = error {
                    let warning = gtk::Label::new(Some(&error));
                    warning.add_css_class("error");
                    warning.set_halign(gtk::Align::Start);
                    warning.set_margin_start(12);
                    warning.set_wrap(true);
                    content.append(&warning);
                }
            }
        } else {
            content.append(&automatic_section(&state, sender));
        }
    }

    adw::Clamp::builder()
        .maximum_size(720)
        .tightening_threshold(560)
        .child(&content)
        .build()
}

/// The disk-picker `PreferencesGroup`: one always-visible row per available
/// disk with a radio button, rather than a dropdown — Sirius only ever
/// installs to a single disk (there is no multi-disk/RAID install), so this
/// is a single-select list, just laid out so every option is visible at
/// once instead of hidden behind a popover.
fn disk_selector(
    disks: &[DiskSnapshot],
    selected: Option<usize>,
    error: Option<&str>,
    sender: &ComponentSender<StoragePage>,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    group.set_title(&gettext("Destination disk"));

    if let Some(error) = error {
        let row = adw::ActionRow::new();
        row.set_title(&gettext("Disks could not be loaded"));
        row.set_subtitle(error);
        group.add(&row);
        return group;
    }

    // Only disks that are not already in use can be selected.
    let available: Vec<usize> = disks
        .iter()
        .enumerate()
        .filter(|(_, disk)| !disk.in_use)
        .map(|(index, _)| index)
        .collect();

    if available.is_empty() {
        let row = adw::ActionRow::new();
        row.set_title(&gettext("No available disks"));
        row.set_subtitle(&gettext(
            "Connect a disk or unmount its filesystems and reopen Sirius.",
        ));
        group.add(&row);
        return group;
    }

    let mut leader: Option<gtk::CheckButton> = None;
    let mut solo: Option<(adw::ActionRow, gtk::CheckButton)> = None;
    for &index in &available {
        let disk = &disks[index];
        let row = adw::ActionRow::new();
        row.set_title(&disk.model);
        row.set_subtitle(&format!(
            "{} • {} • {}",
            disk.path,
            format_size(disk.size_bytes),
            disk.table_type
        ));
        row.add_prefix(&gtk::Image::from_icon_name("drive-harddisk-symbolic"));

        let radio = gtk::CheckButton::new();
        radio.set_group(leader.as_ref());
        radio.set_active(selected == Some(index));
        let page_sender = sender.clone();
        radio.connect_toggled(move |button| {
            if button.is_active() {
                page_sender.input(StorageMsg::Selected(index));
            }
        });
        row.add_suffix(&radio);
        row.set_activatable_widget(Some(&radio));
        if leader.is_none() {
            leader = Some(radio.clone());
        }
        solo = Some((row.clone(), radio));
        group.add(&row);
    }

    // GTK only draws the round radio indicator once a check button is
    // actually grouped with another one; a lone entry renders as a square
    // checkbox otherwise. A single-disk system is the common case, so pair
    // the sole radio with an invisible anchor purely to get the radio look.
    if available.len() == 1 {
        if let Some((row, radio)) = solo {
            let anchor = gtk::CheckButton::new();
            anchor.set_visible(false);
            anchor.set_group(Some(&radio));
            row.add_suffix(&anchor);
        }
    }

    group
}

/// Partitioning-mode picker: one radio row per mode. The HIG favors
/// visible, mutually exclusive radio options for a choice between two
/// flows — a switch reads as "turn a setting on or off", which made the
/// old row ambiguous about what "off" even meant (manual mode was never
/// named on screen). Same radio-row pattern as the disk selector above.
fn mode_selector(
    state: &PageView<'_>,
    sender: &ComponentSender<StoragePage>,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    group.set_title(&gettext("Partitioning"));

    let mut leader: Option<gtk::CheckButton> = None;
    for (title, desc, manual) in [
        (
            gettext("Automatic partitioning"),
            gettext("Erase the disk and create the needed partitions automatically."),
            false,
        ),
        (
            gettext("Manual partitioning"),
            gettext("Create, resize, and delete partitions yourself."),
            true,
        ),
    ] {
        let row = adw::ActionRow::new();
        row.set_title(&title);
        row.set_subtitle(&desc);

        let radio = gtk::CheckButton::new();
        radio.set_group(leader.as_ref());
        radio.set_active(state.manual == manual);
        let page_sender = sender.clone();
        radio.connect_toggled(move |button| {
            if button.is_active() {
                page_sender.input(StorageMsg::SetManual(manual));
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

/// Automatic-mode section: the encryption options group plus the
/// destructive-action notice, kept together with tight spacing so the
/// notice clearly belongs to the choices above it.
fn automatic_section(state: &PageView<'_>, sender: &ComponentSender<StoragePage>) -> gtk::Box {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 6);

    let group = adw::PreferencesGroup::new();
    group.set_title(&gettext("Encryption"));

    let encrypt = adw::SwitchRow::new();
    encrypt.set_title(&gettext("Encrypt the disk (LUKS)"));
    encrypt.set_active(state.encrypt);
    let page_sender = sender.clone();
    encrypt.connect_active_notify(move |row| {
        page_sender.input(StorageMsg::ToggleEncrypt(row.is_active()));
    });
    group.add(&encrypt);

    let tpm = adw::SwitchRow::new();
    tpm.set_title(&gettext("Bind encryption to TPM"));
    tpm.set_sensitive(state.encrypt);
    tpm.set_active(state.tpm);
    let page_sender = sender.clone();
    tpm.connect_active_notify(move |row| {
        page_sender.input(StorageMsg::ToggleTpm(row.is_active()));
    });
    group.add(&tpm);

    // Dedicated LUKS passphrase (decoupled from the user account password):
    // only relevant while encryption is on, so the rows hide otherwise.
    let passphrase = adw::PasswordEntryRow::new();
    passphrase.set_title(&gettext("Encryption passphrase"));
    passphrase.set_text(state.encryption_passphrase);
    passphrase.set_visible(state.encrypt);
    let page_sender = sender.clone();
    passphrase.connect_changed(move |row| {
        page_sender.input(StorageMsg::SetEncryptionPassphrase(row.text().to_string()));
    });
    group.add(&passphrase);

    let confirm = adw::PasswordEntryRow::new();
    confirm.set_title(&gettext("Confirm passphrase"));
    confirm.set_text(state.encryption_passphrase_confirm);
    confirm.set_visible(state.encrypt);
    let page_sender = sender.clone();
    confirm.connect_changed(move |row| {
        page_sender.input(StorageMsg::SetEncryptionPassphraseConfirm(
            row.text().to_string(),
        ));
    });
    group.add(&confirm);

    container.append(&group);

    // Inline hint explaining why Next is gated; only shown once the user has
    // typed something (an untouched pair stays silent, like the user page).
    if state.encrypt
        && (!state.encryption_passphrase.is_empty()
            || !state.encryption_passphrase_confirm.is_empty())
    {
        if let Err(error) = crate::config_model::validate_encryption_passphrase(
            state.encryption_passphrase,
            state.encryption_passphrase_confirm,
        ) {
            let hint = gtk::Label::new(Some(&error));
            hint.add_css_class("error");
            hint.set_halign(gtk::Align::Start);
            hint.set_margin_start(12);
            hint.set_wrap(true);
            container.append(&hint);
        }
    }

    container.append(&erase_notice());
    container
}

/// Inline caution for the destructive automatic flow: a warning-tinted
/// icon next to a caption, the GNOME pattern for calling attention to
/// data-loss consequences without a disruptive dialog.
fn erase_notice() -> gtk::Box {
    let container = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    container.set_halign(gtk::Align::Start);
    container.set_margin_start(12);

    let icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
    icon.add_css_class("warning");
    container.append(&icon);

    let label = gtk::Label::new(Some(&gettext("All data on this disk will be erased.")));
    label.add_css_class("dim-label");
    label.add_css_class("caption");
    container.append(&label);

    container
}

/// Entry point into the partition editor modal: a navigation-style row
/// (title, subtitle, trailing chevron, whole row activatable) that opens
/// `editor_view`'s dialog with the disk-usage map and the full
/// volumes/partitions list — the standard HIG pattern for drilling into a
/// sub-page, instead of a flat button floating at the row's edge.
fn open_editor_row(
    disk: &DiskSnapshot,
    sender: &ComponentSender<StoragePage>,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    let row = adw::ActionRow::new();
    row.set_title(&gettext("Volumes and partitions"));
    row.set_subtitle(&format!(
        "{}: {}",
        gettext("Table"),
        disk.table_type.to_ascii_uppercase()
    ));
    row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));

    row.set_activatable(true);
    let page_sender = sender.clone();
    row.connect_activated(move |_| page_sender.input(StorageMsg::OpenEditor));

    group.add(&row);
    group
}
