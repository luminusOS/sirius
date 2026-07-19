//! Keyboard layout page: lets the user pick a keyboard layout and test it.

use super::PageOutput;
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::{adw, gtk, ComponentParts, ComponentSender, SimpleComponent};

const LAYOUTS: &[(&str, &str)] = &[("us", "English (US)"), ("br", "Portuguese (Brazil)")];

pub struct KeyboardPage;

#[derive(Debug)]
pub enum KeyboardMsg {
    Chosen(usize),
    /// The UI language changed; gettext resolves strings at render time, so a
    /// bare re-render (Relm4 runs update_view after update) is enough.
    Retranslate,
}

#[relm4::component(pub)]
impl SimpleComponent for KeyboardPage {
    type Init = ();
    type Input = KeyboardMsg;
    type Output = PageOutput;

    view! {
        adw::StatusPage {
            set_icon_name: Some("input-keyboard-symbolic"),
            #[watch]
            set_title: gettext("Keyboard layout").as_str(),
            #[wrap(Some)]
            set_child = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 12,
                set_halign: gtk::Align::Center,
                gtk::DropDown {
                    set_model: Some(&gtk::StringList::new(
                        &LAYOUTS.iter().map(|(_, label)| *label).collect::<Vec<_>>()
                    )),
                    connect_selected_notify[sender] => move |dd| {
                        sender.input(KeyboardMsg::Chosen(dd.selected() as usize));
                    },
                },
                gtk::Entry {
                    #[watch]
                    set_placeholder_text: Some(gettext("Type here to test your layout").as_str()),
                },
            },
        }
    }

    fn init(
        _i: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        sender
            .output(PageOutput::SetKeyboard(LAYOUTS[0].0.to_string()))
            .ok();
        let model = KeyboardPage;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            KeyboardMsg::Chosen(i) => {
                let code = LAYOUTS.get(i).map(|(c, _)| *c).unwrap_or("us");
                sender
                    .output(PageOutput::SetKeyboard(code.to_string()))
                    .ok();
            }
            KeyboardMsg::Retranslate => {}
        }
    }
}
