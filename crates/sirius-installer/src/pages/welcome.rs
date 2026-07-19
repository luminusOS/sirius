//! Welcome page: greets the user and collects the install locale.
//! The artwork above the title comes from the distro descriptor's
//! `[branding]` (logo file, or themed icon) — defaulting to a star, for Sirius.

use super::PageOutput;
use crate::backend::distro::Branding;
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, adw, gtk};

/// UI languages offered on the welcome page: (locale, native name).
/// The locale also flows into the install config unchanged.
const LANGUAGES: &[(&str, &str)] = &[("en_US", "English (US)"), ("pt_BR", "Português (BR)")];

pub struct WelcomePage;

#[derive(Debug)]
pub enum WelcomeMsg {
    LocaleChosen(String),
    /// The UI language changed; gettext resolves strings at render time, so a
    /// bare re-render (Relm4 runs update_view after update) is enough.
    Retranslate,
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
            set_title: gettext("Welcome").as_str(),
            #[watch]
            set_description: Some(gettext("This assistant will guide you through installation.").as_str()),

            #[wrap(Some)]
            set_child = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 12,
                set_halign: gtk::Align::Center,

                gtk::DropDown {
                    set_width_request: 240,
                    set_model: Some(&gtk::StringList::new(
                        &LANGUAGES.iter().map(|(_, name)| *name).collect::<Vec<_>>(),
                    )),
                    connect_selected_notify[sender] => move |dd| {
                        let locale = LANGUAGES
                            .get(dd.selected() as usize)
                            .map(|(locale, _)| *locale)
                            .unwrap_or("en_US");
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
        let model = WelcomePage;
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
            WelcomeMsg::Retranslate => {}
        }
    }
}
