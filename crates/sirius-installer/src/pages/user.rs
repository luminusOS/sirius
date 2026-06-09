//! User account page: collects name, username, password, and hostname.
//! Emits `CanProceed(valid)` on every keystroke and `SetUser` when all fields pass.

use super::PageOutput;
use crate::config_model::UserAccount;
use relm4::adw::prelude::*;
use relm4::{adw, gtk, ComponentParts, ComponentSender, SimpleComponent};

#[derive(Default)]
pub struct UserPage {
    account: UserAccount,
}

#[derive(Debug)]
pub enum UserMsg {
    FullName(String),
    Username(String),
    Password(String),
    PasswordConfirm(String),
    Hostname(String),
}

#[relm4::component(pub)]
impl SimpleComponent for UserPage {
    type Init = ();
    type Input = UserMsg;
    type Output = PageOutput;

    view! {
        adw::StatusPage {
            set_title: "Create your account",
            #[wrap(Some)]
            set_child = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 6,
                set_halign: gtk::Align::Center,
                set_width_request: 400,

                gtk::Label {
                    add_css_class: "error",
                    #[watch]
                    set_label: &model.account.validate().err().unwrap_or_default(),
                    #[watch]
                    set_visible: model.account.validate().is_err(),
                },

                adw::PreferencesGroup {
                    adw::EntryRow {
                        set_title: "Full name",
                        connect_changed[sender] => move |e| {
                            sender.input(UserMsg::FullName(e.text().to_string()));
                        },
                    },
                    adw::EntryRow {
                        set_title: "Username",
                        connect_changed[sender] => move |e| {
                            sender.input(UserMsg::Username(e.text().to_string()));
                        },
                    },
                    adw::PasswordEntryRow {
                        set_title: "Password",
                        connect_changed[sender] => move |e| {
                            sender.input(UserMsg::Password(e.text().to_string()));
                        },
                    },
                    adw::PasswordEntryRow {
                        set_title: "Confirm password",
                        connect_changed[sender] => move |e| {
                            sender.input(UserMsg::PasswordConfirm(e.text().to_string()));
                        },
                    },
                    adw::EntryRow {
                        set_title: "Hostname",
                        connect_changed[sender] => move |e| {
                            sender.input(UserMsg::Hostname(e.text().to_string()));
                        },
                    },
                },
            },
        }
    }

    fn init(
        _i: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = UserPage::default();
        let widgets = view_output!();
        sender.output(PageOutput::CanProceed(false)).ok();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            UserMsg::FullName(v) => self.account.full_name = v,
            UserMsg::Username(v) => self.account.username = v,
            UserMsg::Password(v) => self.account.password = v,
            UserMsg::PasswordConfirm(v) => self.account.password_confirm = v,
            UserMsg::Hostname(v) => self.account.hostname = v,
        }
        let valid = self.account.validate().is_ok();
        sender.output(PageOutput::CanProceed(valid)).ok();
        if valid {
            sender.output(PageOutput::SetUser(self.account.clone())).ok();
        }
    }
}
