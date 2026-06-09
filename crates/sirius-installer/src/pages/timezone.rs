//! Timezone page: lets the user select a time zone from a dropdown.

use super::PageOutput;
use relm4::adw::prelude::*;
use relm4::{adw, gtk, ComponentParts, ComponentSender, SimpleComponent};

const ZONES: &[&str] = &["America/Sao_Paulo", "America/New_York", "Europe/London", "UTC"];

pub struct TimezonePage;

#[derive(Debug)]
pub enum TimezoneMsg {
    Chosen(usize),
}

#[relm4::component(pub)]
impl SimpleComponent for TimezonePage {
    type Init = ();
    type Input = TimezoneMsg;
    type Output = PageOutput;

    view! {
        adw::StatusPage {
            set_icon_name: Some("alarm-symbolic"),
            set_title: "Time zone",
            #[wrap(Some)]
            set_child = &gtk::DropDown {
                set_halign: gtk::Align::Center,
                set_model: Some(&gtk::StringList::new(ZONES)),
                connect_selected_notify[sender] => move |dd| {
                    sender.input(TimezoneMsg::Chosen(dd.selected() as usize));
                },
            },
        }
    }

    fn init(
        _i: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        sender.output(PageOutput::SetTimezone(ZONES[0].to_string())).ok();
        let model = TimezonePage;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        let TimezoneMsg::Chosen(i) = msg;
        let zone = ZONES.get(i).copied().unwrap_or("UTC");
        sender.output(PageOutput::SetTimezone(zone.to_string())).ok();
    }
}
