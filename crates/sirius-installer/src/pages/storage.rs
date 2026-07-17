//! Unified disk selection and partitioning page.
//!
//! Manual actions only mutate `PartitionPlan`; no disk write happens here.

mod draft;
mod editor_view;
mod page_view;
mod partition_dialog;

use super::{PageOutput, StorageSelection};
use crate::backend::storage::{scan_disks, DiskSnapshot};
use crate::config_model::{InstallType, PartitionPlan};
use draft::{PartitionDraft, PartitionSpec};
use partition_dialog::DialogTarget;
use relm4::adw::prelude::*;
use relm4::{adw, gtk, ComponentParts, ComponentSender, SimpleComponent};

pub struct StoragePage {
    root: gtk::ScrolledWindow,
    lang: crate::i18n::Lang,
    uefi: bool,
    disks: Vec<DiskSnapshot>,
    selected: Option<usize>,
    manual: bool,
    encrypt: bool,
    tpm: bool,
    plan: Option<PartitionPlan>,
    error: Option<String>,
    editor: Option<adw::Dialog>,
    /// Draft edited by the partition dialog. It is committed only by Apply.
    editor_draft: Option<PartitionDraft>,
    editor_error: Option<String>,
}

#[derive(Debug)]
pub enum StorageMsg {
    Selected(usize),
    SetManual(bool),
    OpenCustom,
    CloseCustom,
    ApplyCustom,
    ToggleEncrypt(bool),
    ToggleTpm(bool),
    OpenCreate(usize),
    Create {
        region: usize,
        spec: PartitionSpec,
    },
    OpenEdit(usize),
    Edit {
        partition: usize,
        spec: PartitionSpec,
    },
    Delete(usize),
    DeletePlanned(String),
    SetLang(crate::i18n::Lang),
}

pub struct StoragePageWidgets {
    root: gtk::ScrolledWindow,
}

impl SimpleComponent for StoragePage {
    type Init = ();
    type Input = StorageMsg;
    type Output = PageOutput;
    type Root = gtk::ScrolledWindow;
    type Widgets = StoragePageWidgets;

    fn init_root() -> Self::Root {
        gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .build()
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let (disks, error) = match scan_disks() {
            Ok(disks) => (disks, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        let model = StoragePage {
            root: root.clone(),
            lang: crate::i18n::Lang::En,
            uefi: std::path::Path::new("/sys/firmware/efi").exists(),
            disks,
            selected: None,
            manual: false,
            encrypt: false,
            tpm: false,
            plan: None,
            error,
            editor: None,
            editor_draft: None,
            editor_error: None,
        };
        let mut widgets = StoragePageWidgets { root };
        // Imperative pages do not receive an automatic first update. Build the
        // disk rows now so the page is usable immediately on arrival.
        model.update_view(&mut widgets, sender.clone());
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            StorageMsg::Selected(index) => {
                self.selected = Some(index);
                self.reset_plan();
                self.emit(&sender);
            }
            StorageMsg::SetManual(manual) => {
                self.manual = manual;
                self.encrypt = false;
                self.tpm = false;
                self.reset_plan();
                if !manual {
                    self.discard_editor();
                }
                self.emit(&sender);
            }
            StorageMsg::OpenCustom => {
                if !self.manual || self.disk().is_none() {
                    return;
                }
                self.open_editor(&sender);
            }
            StorageMsg::CloseCustom => self.discard_editor(),
            StorageMsg::ApplyCustom => {
                let result = self
                    .editor_draft
                    .as_ref()
                    .ok_or_else(|| "partition draft is not available".to_string())
                    .and_then(|draft| draft.validate(self.uefi));
                match result {
                    Ok(()) => {
                        self.plan = self.editor_draft.take().map(PartitionDraft::into_plan);
                        self.editor_error = None;
                        self.close_editor();
                        self.emit(&sender);
                    }
                    Err(error) => {
                        self.editor_error = Some(error);
                        self.refresh_editor(&sender);
                    }
                }
            }
            StorageMsg::ToggleEncrypt(enabled) => {
                self.encrypt = enabled;
                if !enabled {
                    self.tpm = false;
                }
                self.emit(&sender);
            }
            StorageMsg::ToggleTpm(enabled) => {
                self.tpm = enabled && self.encrypt;
                self.emit(&sender);
            }
            StorageMsg::OpenCreate(region) => {
                if let Some(free) = self
                    .editor_draft
                    .as_ref()
                    .and_then(|draft| draft.remaining_region(region))
                {
                    partition_dialog::show(
                        &self.dialog_parent(),
                        &sender,
                        DialogTarget::Create(region, free),
                        None,
                        self.lang,
                    );
                }
            }
            StorageMsg::Create { region, spec } => {
                self.editor_error = self
                    .editor_draft
                    .as_mut()
                    .ok_or_else(|| "partition draft is not available".to_string())
                    .and_then(|draft| draft.create(region, spec))
                    .err();
                self.refresh_editor(&sender);
            }
            StorageMsg::OpenEdit(partition) => {
                if let Some(existing) = self.disk().and_then(|d| d.partitions.get(partition)) {
                    partition_dialog::show(
                        &self.dialog_parent(),
                        &sender,
                        DialogTarget::Edit(partition),
                        Some(existing),
                        self.lang,
                    );
                }
            }
            StorageMsg::Edit { partition, spec } => {
                self.editor_error = self
                    .editor_draft
                    .as_mut()
                    .ok_or_else(|| "partition draft is not available".to_string())
                    .and_then(|draft| draft.edit_existing(partition, spec))
                    .err();
                self.refresh_editor(&sender);
            }
            StorageMsg::Delete(partition) => {
                self.editor_error = self
                    .editor_draft
                    .as_mut()
                    .ok_or_else(|| "partition draft is not available".to_string())
                    .and_then(|draft| draft.delete_existing(partition))
                    .err();
                self.refresh_editor(&sender);
            }
            StorageMsg::DeletePlanned(id) => {
                self.editor_error = self
                    .editor_draft
                    .as_mut()
                    .ok_or_else(|| "partition draft is not available".to_string())
                    .and_then(|draft| draft.delete_planned(&id))
                    .err();
                self.refresh_editor(&sender);
            }
            StorageMsg::SetLang(lang) => self.lang = lang,
        }
    }

    fn update_view(&self, widgets: &mut Self::Widgets, sender: ComponentSender<Self>) {
        let clamp = page_view::build(
            page_view::PageView {
                disks: &self.disks,
                selected: self.selected,
                manual: self.manual,
                encrypt: self.encrypt,
                tpm: self.tpm,
                error: self.error.as_deref(),
                lang: self.lang,
            },
            &sender,
        );
        widgets.root.set_child(Some(&clamp));
        self.refresh_editor(&sender);
    }
}

impl StoragePage {
    fn disk(&self) -> Option<&DiskSnapshot> {
        self.selected.and_then(|index| self.disks.get(index))
    }

    fn open_editor(&mut self, sender: &ComponentSender<Self>) {
        self.discard_editor();
        let Some(disk) = self.disk() else {
            return;
        };
        match PartitionDraft::new(disk, self.plan.as_ref()) {
            Ok(draft) => self.editor_draft = Some(draft),
            Err(error) => {
                self.editor_error = Some(error);
                return;
            }
        }
        let dialog = adw::Dialog::builder()
            .title(crate::i18n::tr(self.lang, "storage.editor"))
            .content_width(820)
            .content_height(560)
            .build();
        self.editor = Some(dialog.clone());
        self.refresh_editor(sender);
        dialog.present(Some(&self.root));
    }

    fn close_editor(&mut self) {
        if let Some(dialog) = self.editor.take() {
            dialog.close();
        }
    }

    fn discard_editor(&mut self) {
        self.editor_draft = None;
        self.editor_error = None;
        self.close_editor();
    }

    fn refresh_editor(&self, sender: &ComponentSender<Self>) {
        let (Some(dialog), Some(disk)) = (&self.editor, self.disk()) else {
            return;
        };
        dialog.set_child(Some(&editor_view::build(
            disk,
            self.editor_draft.as_ref().map(PartitionDraft::plan),
            self.editor_error.as_deref(),
            self.uefi,
            sender,
            self.lang,
        )));
    }

    fn dialog_parent(&self) -> gtk::Widget {
        self.editor
            .as_ref()
            .map(|dialog| dialog.clone().upcast())
            .unwrap_or_else(|| self.root.clone().upcast())
    }

    fn reset_plan(&mut self) {
        self.editor_draft = None;
        self.editor_error = None;
        self.plan = self.disk().map(PartitionDraft::empty_plan);
    }

    fn emit(&self, sender: &ComponentSender<Self>) {
        let Some(disk) = self.disk() else {
            return;
        };
        let install_type = if self.manual {
            InstallType::Manual
        } else if self.encrypt {
            InstallType::Encrypted
        } else {
            InstallType::WholeDisk
        };
        sender
            .output(PageOutput::SetStorage(StorageSelection {
                path: disk.path.clone(),
                name: disk.model.clone(),
                install_type,
                encrypt: self.encrypt,
                tpm: self.tpm,
                partition_plan: self.manual.then(|| self.plan.clone()).flatten(),
            }))
            .ok();
    }
}
