//! Network page: informs the user about network connectivity; always allows proceeding.

use super::PageOutput;
use relm4::{adw, gtk, ComponentParts, ComponentSender, SimpleComponent};

pub struct NetworkPage;

#[derive(Debug)]
pub enum NetworkMsg {}

#[relm4::component(pub)]
impl SimpleComponent for NetworkPage {
    type Init = ();
    type Input = NetworkMsg;
    type Output = PageOutput;

    view! {
        adw::StatusPage {
            set_icon_name: Some("network-wireless-symbolic"),
            set_title: "Network",
            set_description: Some("Connect to a network. A connection is optional but recommended."),
            #[wrap(Some)]
            set_child = &gtk::Label {
                set_label: "Use the system network indicator to connect.",
            },
        }
    }

    fn init(
        _i: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = NetworkPage;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, _msg: Self::Input, _sender: ComponentSender<Self>) {}
}
