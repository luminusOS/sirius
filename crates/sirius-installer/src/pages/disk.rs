//! Disk-selection page.
//!
//! Lists whole disks via `lsblk` and lets the user pick a target.
//! Emits `PageOutput::SetDisk` and `PageOutput::CanProceed` up to the root
//! `AppModel`.

use super::PageOutput;
use relm4::adw::prelude::*;
use relm4::{adw, ComponentParts, ComponentSender, SimpleComponent};
use sirius_diag::{list_disks, DiskInfo};

pub struct DiskPage {
    disks: Vec<DiskInfo>,
}

#[derive(Debug)]
pub enum DiskMsg {
    Selected(usize),
}

pub struct DiskPageWidgets {
    root: adw::StatusPage,
}

impl SimpleComponent for DiskPage {
    type Init = ();
    type Input = DiskMsg;
    type Output = PageOutput;
    type Root = adw::StatusPage;
    type Widgets = DiskPageWidgets;

    fn init_root() -> Self::Root {
        adw::StatusPage::new()
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        root.set_title("Select a disk");
        root.set_description(Some("The chosen disk will be erased."));
        root.set_icon_name(Some("drive-harddisk-symbolic"));

        let group = adw::PreferencesGroup::new();

        let disks = list_disks();

        if disks.is_empty() {
            let row = adw::ActionRow::new();
            row.set_title("No disks found");
            group.add(&row);
            sender.output(PageOutput::CanProceed(false)).ok();
        } else {
            sender.output(PageOutput::CanProceed(false)).ok();
            for (i, d) in disks.iter().enumerate() {
                let row = adw::ActionRow::new();
                row.set_title(&d.path);
                row.set_subtitle(&format!(
                    "{} — {} GiB",
                    d.model,
                    d.size_bytes / (1024 * 1024 * 1024)
                ));
                row.set_activatable(true);
                let s = sender.clone();
                row.connect_activated(move |_| s.input(DiskMsg::Selected(i)));
                group.add(&row);
            }
        }

        root.set_child(Some(&group));

        let model = DiskPage { disks };
        let widgets = DiskPageWidgets { root };

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            DiskMsg::Selected(i) => {
                if let Some(d) = self.disks.get(i) {
                    sender.output(PageOutput::SetDisk(d.path.clone())).ok();
                    sender.output(PageOutput::CanProceed(true)).ok();
                }
            }
        }
    }

    fn update_view(&self, _widgets: &mut Self::Widgets, _sender: ComponentSender<Self>) {}
}
