//! Welcome page: greets the user and collects the install locale.

use super::PageOutput;
use relm4::adw::prelude::*;
use relm4::{adw, gtk, ComponentParts, ComponentSender, SimpleComponent};

pub struct WelcomePage;

#[derive(Debug)]
pub enum WelcomeMsg {
    LocaleChosen(String),
}

#[relm4::component(pub)]
impl SimpleComponent for WelcomePage {
    type Init = ();
    type Input = WelcomeMsg;
    type Output = PageOutput;

    view! {
        adw::StatusPage {
            set_icon_name: Some("starred-symbolic"),
            set_title: "Welcome to LuminusOS",
            set_description: Some("Sirius will guide you through installation."),

            #[wrap(Some)]
            set_child = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 12,
                set_halign: gtk::Align::Center,

                gtk::DropDown {
                    set_model: Some(&gtk::StringList::new(&["English (US)", "Português (BR)"])),
                    connect_selected_notify[sender] => move |dd| {
                        let locale = if dd.selected() == 1 { "pt_BR" } else { "en_US" };
                        sender.input(WelcomeMsg::LocaleChosen(locale.to_string()));
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = WelcomePage;
        let widgets = view_output!();
        sender.output(PageOutput::SetLocale("en_US".to_string())).ok();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            WelcomeMsg::LocaleChosen(locale) => {
                sender.output(PageOutput::SetLocale(locale)).ok();
            }
        }
    }
}
