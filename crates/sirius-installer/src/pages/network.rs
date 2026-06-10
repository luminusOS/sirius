//! Network page: informs the user about network connectivity; always allows proceeding.

use super::PageOutput;
use relm4::{adw, gtk, ComponentParts, ComponentSender, SimpleComponent};

pub struct NetworkPage {
    lang: crate::i18n::Lang,
}

#[derive(Debug)]
pub enum NetworkMsg {
    SetLang(crate::i18n::Lang),
}

#[relm4::component(pub)]
impl SimpleComponent for NetworkPage {
    type Init = ();
    type Input = NetworkMsg;
    type Output = PageOutput;

    view! {
        adw::StatusPage {
            set_icon_name: Some("network-wireless-symbolic"),
            #[watch]
            set_title: crate::i18n::tr(model.lang, "network.title"),
            #[watch]
            set_description: Some(crate::i18n::tr(model.lang, "network.desc")),
            #[wrap(Some)]
            set_child = &gtk::Label {
                #[watch]
                set_label: crate::i18n::tr(model.lang, "network.body"),
            },
        }
    }

    fn init(
        _i: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = NetworkPage { lang: crate::i18n::Lang::En };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            NetworkMsg::SetLang(l) => self.lang = l,
        }
    }
}
