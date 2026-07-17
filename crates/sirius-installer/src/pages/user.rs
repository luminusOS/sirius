//! User account page: collects name, username, password, and hostname.
//! Emits the complete draft on every keystroke. The root state owns validation.

use super::PageOutput;
use crate::config_model::UserAccount;
use relm4::adw::prelude::*;
use relm4::{adw, gtk, ComponentParts, ComponentSender, SimpleComponent};

#[derive(Default)]
pub struct UserPage {
    lang: crate::i18n::Lang,
    account: UserAccount,
}

#[derive(Debug)]
pub enum UserMsg {
    FullName(String),
    Username(String),
    Password(String),
    PasswordConfirm(String),
    Hostname(String),
    SetLang(crate::i18n::Lang),
}

#[relm4::component(pub)]
impl SimpleComponent for UserPage {
    type Init = ();
    type Input = UserMsg;
    type Output = PageOutput;

    view! {
        adw::StatusPage {
            #[watch]
            set_title: crate::i18n::tr(model.lang, "user.title"),
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
                        #[watch]
                        set_title: crate::i18n::tr(model.lang, "user.full_name"),
                        connect_changed[sender] => move |e| {
                            sender.input(UserMsg::FullName(e.text().to_string()));
                        },
                    },
                    adw::EntryRow {
                        #[watch]
                        set_title: crate::i18n::tr(model.lang, "user.username"),
                        connect_changed[sender] => move |e| {
                            sender.input(UserMsg::Username(e.text().to_string()));
                        },
                    },
                    adw::PasswordEntryRow {
                        #[watch]
                        set_title: crate::i18n::tr(model.lang, "user.password"),
                        connect_changed[sender] => move |e| {
                            sender.input(UserMsg::Password(e.text().to_string()));
                        },
                    },
                    adw::PasswordEntryRow {
                        #[watch]
                        set_title: crate::i18n::tr(model.lang, "user.confirm"),
                        connect_changed[sender] => move |e| {
                            sender.input(UserMsg::PasswordConfirm(e.text().to_string()));
                        },
                    },
                    adw::EntryRow {
                        #[watch]
                        set_title: crate::i18n::tr(model.lang, "user.hostname"),
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
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            UserMsg::FullName(v) => self.account.full_name = v,
            UserMsg::Username(v) => self.account.username = v,
            UserMsg::Password(v) => self.account.password = v,
            UserMsg::PasswordConfirm(v) => self.account.password_confirm = v,
            UserMsg::Hostname(v) => self.account.hostname = v,
            UserMsg::SetLang(l) => {
                self.lang = l;
                return;
            }
        }
        sender
            .output(PageOutput::SetUser(self.account.clone()))
            .ok();
    }
}
