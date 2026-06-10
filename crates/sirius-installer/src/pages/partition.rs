//! Partition-mode page: lets the user choose automatic or encrypted disk layout.

use super::PageOutput;
use crate::config_model::InstallType;
use relm4::adw::prelude::*;
use relm4::{adw, ComponentParts, ComponentSender, SimpleComponent};

pub struct PartitionPage {
    lang: crate::i18n::Lang,
    encrypt: bool,
    tpm: bool,
}

#[derive(Debug)]
pub enum PartitionMsg {
    ToggleEncrypt(bool),
    ToggleTpm(bool),
    SetLang(crate::i18n::Lang),
}

#[relm4::component(pub)]
impl SimpleComponent for PartitionPage {
    type Init = ();
    type Input = PartitionMsg;
    type Output = PageOutput;

    view! {
        adw::StatusPage {
            set_icon_name: Some("drive-multidisk-symbolic"),
            #[watch]
            set_title: crate::i18n::tr(model.lang, "partition.title"),
            #[wrap(Some)]
            set_child = &adw::PreferencesGroup {
                #[watch]
                set_title: crate::i18n::tr(model.lang, "partition.group"),
                adw::SwitchRow {
                    #[watch]
                    set_title: crate::i18n::tr(model.lang, "partition.encrypt"),
                    connect_active_notify[sender] => move |r| {
                        sender.input(PartitionMsg::ToggleEncrypt(r.is_active()));
                    },
                },
                adw::SwitchRow {
                    #[watch]
                    set_title: crate::i18n::tr(model.lang, "partition.tpm"),
                    set_sensitive: false,
                    connect_active_notify[sender] => move |r| {
                        sender.input(PartitionMsg::ToggleTpm(r.is_active()));
                    },
                },
            },
        }
    }

    fn init(_i: Self::Init, root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = PartitionPage { lang: crate::i18n::Lang::En, encrypt: false, tpm: false };
        let widgets = view_output!();
        sender.output(PageOutput::SetPartition {
            install_type: InstallType::WholeDisk,
            encrypt: false,
            tpm: false,
        }).ok();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        let emit = match msg {
            PartitionMsg::ToggleEncrypt(on) => {
                self.encrypt = on;
                if !on { self.tpm = false; }
                true
            }
            PartitionMsg::ToggleTpm(on) => { self.tpm = on; true }
            PartitionMsg::SetLang(l) => { self.lang = l; false }
        };
        if emit {
            let install_type = if self.encrypt { InstallType::Encrypted } else { InstallType::WholeDisk };
            sender.output(PageOutput::SetPartition {
                install_type,
                encrypt: self.encrypt,
                tpm: self.tpm,
            }).ok();
        }
    }
}
