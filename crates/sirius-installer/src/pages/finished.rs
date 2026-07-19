use super::PageOutput;
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, adw, gtk};

pub struct FinishedPage;

#[derive(Debug)]
pub enum FinishedMsg {
    Reboot,
    /// The UI language changed; gettext resolves strings at render time, so a
    /// bare re-render (Relm4 runs update_view after update) is enough.
    Retranslate,
}

#[relm4::component(pub)]
impl SimpleComponent for FinishedPage {
    type Init = ();
    type Input = FinishedMsg;
    type Output = PageOutput;

    view! {
        adw::StatusPage {
            set_icon_name: Some("object-select-symbolic"),
            #[watch]
            set_title: gettext("Installation complete").as_str(),
            #[watch]
            set_description: Some(gettext("The system is installed. Reboot to start using it.").as_str()),
            #[wrap(Some)]
            set_child = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 12,
                set_halign: gtk::Align::Center,
                gtk::Button {
                    #[watch]
                    set_label: gettext("Restart now").as_str(),
                    add_css_class: "suggested-action",
                    add_css_class: "install-pill",
                    connect_clicked[sender] => move |_| sender.input(FinishedMsg::Reboot),
                },
            },
        }
    }

    fn init(
        _i: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = FinishedPage;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            FinishedMsg::Reboot => {
                // Reboot the machine. On a dev box without privileges this will fail; log and
                // leave the user to reboot manually.
                match std::process::Command::new("systemctl")
                    .arg("reboot")
                    .status()
                {
                    Ok(_) => {}
                    Err(e) => tracing::error!("reboot failed: {e}"),
                }
            }
            FinishedMsg::Retranslate => {}
        }
    }
}
