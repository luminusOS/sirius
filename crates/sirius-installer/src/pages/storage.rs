//! Unified disk selection and partitioning page.
//!
//! Manual actions only mutate `PartitionPlan`; no disk write happens here.

mod draft;
mod page_view;
mod partition_dialog;

use super::{PageOutput, StorageSelection};
use crate::backend::storage::{scan_disks, DiskSnapshot};
use crate::config_model::{InstallType, PartitionPlan};
use draft::{PartitionDraft, PartitionSpec};
use partition_dialog::DialogTarget;
use relm4::adw::prelude::*;
use relm4::{gtk, ComponentParts, ComponentSender, SimpleComponent};

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
    /// Draft for the inline manual-partitioning editor. Every successful
    /// mutation is folded into `plan` and emitted immediately.
    draft: Option<PartitionDraft>,
    draft_error: Option<String>,
}

#[derive(Debug)]
pub enum StorageMsg {
    Selected(usize),
    SetManual(bool),
    ResetDraft,
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
            draft: None,
            draft_error: None,
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
                if self.manual {
                    self.rebuild_draft();
                } else {
                    self.reset_plan();
                }
                self.emit(&sender);
            }
            StorageMsg::SetManual(manual) => {
                self.manual = manual;
                self.encrypt = false;
                self.tpm = false;
                if manual {
                    self.rebuild_draft();
                } else {
                    self.reset_plan();
                }
                self.emit(&sender);
            }
            StorageMsg::ResetDraft => {
                self.rebuild_draft();
                self.emit(&sender);
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
                    .draft
                    .as_ref()
                    .and_then(|draft| draft.remaining_region(region))
                {
                    partition_dialog::show(
                        &self.root.clone().upcast(),
                        &sender,
                        DialogTarget::Create(region, free),
                        None,
                        self.lang,
                    );
                }
            }
            StorageMsg::Create { region, spec } => {
                self.draft_error = self
                    .draft
                    .as_mut()
                    .ok_or_else(|| "partition draft is not available".to_string())
                    .and_then(|draft| draft.create(region, spec))
                    .err();
                self.plan = self.draft.as_ref().map(|d| d.plan().clone());
                self.emit(&sender);
            }
            StorageMsg::OpenEdit(partition) => {
                if let Some(existing) = self.disk().and_then(|d| d.partitions.get(partition)) {
                    partition_dialog::show(
                        &self.root.clone().upcast(),
                        &sender,
                        DialogTarget::Edit(partition),
                        Some(existing),
                        self.lang,
                    );
                }
            }
            StorageMsg::Edit { partition, spec } => {
                self.draft_error = self
                    .draft
                    .as_mut()
                    .ok_or_else(|| "partition draft is not available".to_string())
                    .and_then(|draft| draft.edit_existing(partition, spec))
                    .err();
                self.plan = self.draft.as_ref().map(|d| d.plan().clone());
                self.emit(&sender);
            }
            StorageMsg::Delete(partition) => {
                self.draft_error = self
                    .draft
                    .as_mut()
                    .ok_or_else(|| "partition draft is not available".to_string())
                    .and_then(|draft| draft.delete_existing(partition))
                    .err();
                self.plan = self.draft.as_ref().map(|d| d.plan().clone());
                self.emit(&sender);
            }
            StorageMsg::DeletePlanned(id) => {
                self.draft_error = self
                    .draft
                    .as_mut()
                    .ok_or_else(|| "partition draft is not available".to_string())
                    .and_then(|draft| draft.delete_planned(&id))
                    .err();
                self.plan = self.draft.as_ref().map(|d| d.plan().clone());
                self.emit(&sender);
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
                draft: self.draft.as_ref(),
                draft_error: self.draft_error.as_deref(),
                uefi: self.uefi,
                error: self.error.as_deref(),
                lang: self.lang,
            },
            &sender,
        );
        widgets.root.set_child(Some(&clamp));
    }
}

impl StoragePage {
    fn disk(&self) -> Option<&DiskSnapshot> {
        self.selected.and_then(|index| self.disks.get(index))
    }

    /// Rebuild a fresh draft for the currently selected disk, discarding any
    /// staged changes. Used both when entering manual mode and when the
    /// selected disk changes while manual mode is active.
    fn rebuild_draft(&mut self) {
        self.draft = None;
        self.draft_error = None;
        let Some(disk) = self.disk() else {
            self.plan = None;
            return;
        };
        match PartitionDraft::new(disk, None) {
            Ok(draft) => {
                self.plan = Some(draft.plan().clone());
                self.draft = Some(draft);
            }
            Err(error) => {
                self.draft_error = Some(error);
                self.plan = None;
            }
        }
    }

    fn reset_plan(&mut self) {
        self.draft = None;
        self.draft_error = None;
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
