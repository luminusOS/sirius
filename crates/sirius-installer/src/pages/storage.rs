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
use partition_dialog::{DialogTarget, EditSource};
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
    /// Modal dialog showing the disk-usage map and volumes/partitions list.
    /// Only ever `Some` while manual mode is active and the user has opened
    /// it; its content is rebuilt from `draft`/`draft_error` on every mutation.
    editor: Option<adw::Dialog>,
    /// Draft for the manual-partitioning editor. Every successful mutation is
    /// folded into `plan` and emitted immediately.
    draft: Option<PartitionDraft>,
    draft_error: Option<String>,
    /// Dev aid (`SIRIUS_DEV_SHOW_ALL_DISKS`): lets in-use disks be picked in
    /// the selector, for testing the UI on a host where the only disk is the
    /// one you're booted from. This page never writes to disk regardless —
    /// see the module doc comment — so it's safe to browse and edit plans
    /// against an in-use disk; only an actual install (a separate, later
    /// confirmation) would touch it.
    show_in_use_disks: bool,
}

#[derive(Debug)]
pub enum StorageMsg {
    Selected(usize),
    SetManual(bool),
    ResetDraft,
    ToggleEncrypt(bool),
    ToggleTpm(bool),
    OpenEditor,
    CloseEditor,
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
    OpenEditPlanned(String),
    EditPlanned {
        id: String,
        spec: PartitionSpec,
    },
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
        // Dev aid: SIRIUS_DEV_SHOW_ALL_DISKS lets in-use disks be selected in
        // the UI too, for testing the storage page on a host where the real
        // disk is always in use (mounted root/swap/etc). No disk write ever
        // happens from this page, so this is safe outside of an actual
        // install run — see the module doc comment.
        let show_in_use_disks = std::env::var("SIRIUS_DEV_SHOW_ALL_DISKS").is_ok();
        let mut model = StoragePage {
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
            draft: None,
            draft_error: None,
            show_in_use_disks,
        };
        // The ComboRow widget always renders position 0 as visually selected
        // as soon as it has a model, even though nothing has notified us of a
        // selection yet. Seed `self.selected` to match so the model and the
        // widget agree from the very first frame; otherwise the lower page
        // section stays hidden and, with a single available disk, the user
        // has no other position to pick and can never unstick it.
        if let Some(index) = first_available_disk(&model.disks, model.show_in_use_disks) {
            model.select_disk(index);
            model.emit(&sender);
        }
        let mut widgets = StoragePageWidgets { root };
        // Imperative pages do not receive an automatic first update. Build the
        // disk rows now so the page is usable immediately on arrival.
        model.update_view(&mut widgets, sender.clone());
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            StorageMsg::Selected(index) => {
                self.select_disk(index);
                self.refresh_editor(&sender);
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
                    self.close_editor();
                }
                self.emit(&sender);
            }
            StorageMsg::ResetDraft => {
                self.rebuild_draft();
                self.refresh_editor(&sender);
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
            StorageMsg::OpenEditor => {
                if !self.manual || self.disk().is_none() || self.editor.is_some() {
                    return;
                }
                let dialog = adw::Dialog::builder()
                    .title(crate::i18n::tr(self.lang, "storage.editor"))
                    .content_width(760)
                    .content_height(640)
                    .build();
                self.editor = Some(dialog.clone());
                self.refresh_editor(&sender);
                dialog.present(Some(&self.root));
            }
            StorageMsg::CloseEditor => self.close_editor(),
            StorageMsg::OpenCreate(region) => {
                if let Some(free) = self
                    .draft
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
                self.draft_error = self
                    .draft
                    .as_mut()
                    .ok_or_else(|| "partition draft is not available".to_string())
                    .and_then(|draft| draft.create(region, spec))
                    .err();
                self.plan = self.draft.as_ref().map(|d| d.plan().clone());
                self.refresh_editor(&sender);
                self.emit(&sender);
            }
            StorageMsg::OpenEdit(partition) => {
                if let Some(existing) = self.disk().and_then(|d| d.partitions.get(partition)) {
                    partition_dialog::show(
                        &self.dialog_parent(),
                        &sender,
                        DialogTarget::Edit(partition),
                        Some(EditSource::Existing(existing)),
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
                self.refresh_editor(&sender);
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
                self.refresh_editor(&sender);
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
                self.refresh_editor(&sender);
                self.emit(&sender);
            }
            StorageMsg::OpenEditPlanned(id) => {
                if let Some(details) = self.draft.as_ref().and_then(|d| d.planned_details(&id)) {
                    partition_dialog::show(
                        &self.dialog_parent(),
                        &sender,
                        DialogTarget::EditPlanned(id),
                        Some(EditSource::Planned {
                            filesystem: &details.filesystem,
                            size_bytes: details.size_bytes,
                            max_size_bytes: details.max_size_bytes,
                            mount_point: &details.mount_point,
                            label: &details.label,
                        }),
                        self.lang,
                    );
                }
            }
            StorageMsg::EditPlanned { id, spec } => {
                self.draft_error = self
                    .draft
                    .as_mut()
                    .ok_or_else(|| "partition draft is not available".to_string())
                    .and_then(|draft| draft.edit_planned(&id, spec))
                    .err();
                self.plan = self.draft.as_ref().map(|d| d.plan().clone());
                self.refresh_editor(&sender);
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
                show_in_use_disks: self.show_in_use_disks,
            },
            &sender,
        );
        widgets.root.set_child(Some(&clamp));
        self.refresh_editor(&sender);
    }
}

/// Index of the first disk that is available for selection (not already in
/// use, unless `show_in_use` overrides that), if any. Pure helper shared by
/// `init()` and the `Selected` handler so there is a single place that
/// decides what "the default disk" means.
fn first_available_disk(disks: &[DiskSnapshot], show_in_use: bool) -> Option<usize> {
    disks.iter().position(|disk| show_in_use || !disk.in_use)
}

impl StoragePage {
    fn disk(&self) -> Option<&DiskSnapshot> {
        self.selected.and_then(|index| self.disks.get(index))
    }

    /// Set the selected disk and rebuild whatever derived state (`plan`,
    /// `draft`) depends on it, matching the mode currently in effect. Used
    /// both by the `Selected` message handler and by `init()` to seed the
    /// initial selection.
    fn select_disk(&mut self, index: usize) {
        self.selected = Some(index);
        if self.manual {
            self.rebuild_draft();
        } else {
            self.reset_plan();
        }
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

    /// Close the editor dialog if one is open. Idempotent.
    fn close_editor(&mut self) {
        if let Some(dialog) = self.editor.take() {
            dialog.close();
        }
    }

    /// Rebuild the editor dialog's content from the current draft, if the
    /// dialog is open. Called after every draft mutation (in addition to
    /// `emit`) and from `update_view` so a full re-render (e.g. `SetLang`)
    /// also keeps the modal in sync. No-op when the editor is closed.
    fn refresh_editor(&self, sender: &ComponentSender<Self>) {
        let (Some(dialog), Some(disk)) = (&self.editor, self.disk()) else {
            return;
        };
        dialog.set_child(Some(&editor_view::build(
            disk,
            self.draft.as_ref(),
            self.draft_error.as_deref(),
            self.uefi,
            sender,
            self.lang,
        )));
    }

    /// Parent widget for the small create/edit partition form: the editor
    /// dialog when it is open (so the form stacks on top of it), or the page
    /// root otherwise.
    fn dialog_parent(&self) -> gtk::Widget {
        self.editor
            .as_ref()
            .map(|dialog| dialog.clone().upcast())
            .unwrap_or_else(|| self.root.clone().upcast())
    }

    fn emit(&self, sender: &ComponentSender<Self>) {
        // Always emits the current draft plan, even right after a failed
        // mutation (draft.rs leaves the plan unchanged on error). Next stays
        // gated correctly because `storage_is_valid()` re-validates whatever
        // plan is emitted here, rather than this function withholding it.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn disk(path: &str, in_use: bool) -> DiskSnapshot {
        DiskSnapshot {
            path: path.into(),
            model: "Test disk".into(),
            size_bytes: 64 * 1024 * 1024 * 1024,
            table_type: "GPT".into(),
            read_only: false,
            in_use,
            partitions: Vec::new(),
            free_regions: Vec::new(),
        }
    }

    #[test]
    fn first_available_disk_picks_the_first_non_in_use_disk() {
        let disks = vec![disk("/dev/sda", true), disk("/dev/sdb", false)];
        assert_eq!(first_available_disk(&disks, false), Some(1));
    }

    #[test]
    fn first_available_disk_is_none_when_everything_is_in_use() {
        let disks = vec![disk("/dev/sda", true), disk("/dev/sdb", true)];
        assert_eq!(first_available_disk(&disks, false), None);
    }

    #[test]
    fn first_available_disk_is_none_for_empty_disk_list() {
        let disks: Vec<DiskSnapshot> = Vec::new();
        assert_eq!(first_available_disk(&disks, false), None);
    }

    #[test]
    fn first_available_disk_picks_position_zero_in_the_common_single_disk_case() {
        // This is the case the Critical bug fix targets directly: exactly
        // one available disk, which is also the ComboRow's default visual
        // position. `self.selected` must end up `Some(0)` on init so the
        // model and the freshly built widget agree from the first frame.
        let disks = vec![disk("/dev/sda", false)];
        assert_eq!(first_available_disk(&disks, false), Some(0));
    }

    #[test]
    fn first_available_disk_with_show_in_use_picks_position_zero_even_when_in_use() {
        // SIRIUS_DEV_SHOW_ALL_DISKS: a dev host's only disk is almost always
        // in use (it's booted from it) — the override must still resolve to
        // a selectable default instead of leaving `selected` stuck at `None`.
        let disks = vec![disk("/dev/sda", true)];
        assert_eq!(first_available_disk(&disks, true), Some(0));
    }
}
