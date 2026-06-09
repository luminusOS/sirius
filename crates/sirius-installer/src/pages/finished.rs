use super::PageOutput;
use relm4::adw::prelude::*;
use relm4::{adw, gtk, ComponentParts, ComponentSender, SimpleComponent};

pub struct FinishedPage;

#[derive(Debug)]
pub enum FinishedMsg {
    Reboot,
}

#[relm4::component(pub)]
impl SimpleComponent for FinishedPage {
    type Init = ();
    type Input = FinishedMsg;
    type Output = PageOutput;

    view! {
        adw::StatusPage {
            set_icon_name: Some("emblem-ok-symbolic"),
            set_title: "Installation complete",
            set_description: Some("LuminusOS is installed. Reboot to start using it."),
            #[wrap(Some)]
            set_child = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 12,
                set_halign: gtk::Align::Center,
                gtk::Button {
                    set_label: "Restart now",
                    add_css_class: "suggested-action",
                    connect_clicked[sender] => move |_| sender.input(FinishedMsg::Reboot),
                },
            },
        }
    }

    fn init(_i: Self::Init, root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        sender.output(PageOutput::CanProceed(false)).ok();
        let model = FinishedPage;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        let FinishedMsg::Reboot = msg;
        // Plan 3 replaces this with an actual reboot via the runner.
        tracing::info!("reboot requested");
    }
}
