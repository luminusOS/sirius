use super::PageOutput;
use relm4::adw::prelude::*;
use relm4::{adw, gtk, ComponentParts, ComponentSender, SimpleComponent};

pub struct FinishedPage {
    lang: crate::i18n::Lang,
}

#[derive(Debug)]
pub enum FinishedMsg {
    Reboot,
    SetLang(crate::i18n::Lang),
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
            set_title: crate::i18n::tr(model.lang, "finished.title"),
            #[watch]
            set_description: Some(crate::i18n::tr(model.lang, "finished.desc")),
            #[wrap(Some)]
            set_child = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 12,
                set_halign: gtk::Align::Center,
                gtk::Button {
                    #[watch]
                    set_label: crate::i18n::tr(model.lang, "finished.restart"),
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
        let model = FinishedPage {
            lang: crate::i18n::Lang::En,
        };
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
            FinishedMsg::SetLang(l) => self.lang = l,
        }
    }
}
