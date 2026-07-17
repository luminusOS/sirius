//! Welcome page: greets the user and collects the install locale.
//! The artwork above the title comes from the distro descriptor's
//! `[branding]` (logo file, or themed icon) — defaulting to a star, for Sirius.

use super::PageOutput;
use crate::backend::distro::Branding;
use relm4::adw::prelude::*;
use relm4::{adw, gtk, ComponentParts, ComponentSender, SimpleComponent};

pub struct WelcomePage {
    lang: crate::i18n::Lang,
}

#[derive(Debug)]
pub enum WelcomeMsg {
    LocaleChosen(String),
    SetLang(crate::i18n::Lang),
}

/// Apply `[branding]` to the status page: prefer the logo file, then the
/// themed icon, then the default star.
fn apply_branding(page: &adw::StatusPage, branding: &Branding) {
    if let Some(path) = &branding.logo {
        match gtk::gdk::Texture::from_filename(path) {
            Ok(texture) => {
                page.set_paintable(Some(&texture));
                return;
            }
            Err(e) => tracing::warn!("cannot load branding logo {path}: {e}"),
        }
    }
    page.set_icon_name(Some(branding.icon.as_deref().unwrap_or("starred-symbolic")));
}

#[relm4::component(pub)]
impl SimpleComponent for WelcomePage {
    type Init = Branding;
    type Input = WelcomeMsg;
    type Output = PageOutput;

    view! {
        adw::StatusPage {
            #[watch]
            set_title: crate::i18n::tr(model.lang, "welcome.title"),
            #[watch]
            set_description: Some(crate::i18n::tr(model.lang, "welcome.desc")),

            #[wrap(Some)]
            set_child = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 12,
                set_halign: gtk::Align::Center,

                gtk::DropDown {
                    set_width_request: 240,
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
        branding: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = WelcomePage {
            lang: crate::i18n::Lang::En,
        };
        let widgets = view_output!();
        apply_branding(&root, &branding);
        sender
            .output(PageOutput::SetLocale("en_US".to_string()))
            .ok();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            WelcomeMsg::LocaleChosen(locale) => {
                sender.output(PageOutput::SetLocale(locale)).ok();
            }
            WelcomeMsg::SetLang(l) => {
                self.lang = l;
            }
        }
    }
}
