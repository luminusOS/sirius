//! Branded installer entry point shown before any configuration question.

use super::PageOutput;
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, gtk};
use sirius_core::Branding;
use std::path::Path;

const FALLBACK_BANNER: &str = "/usr/share/sirius/welcome-banner.png";
const DEV_BANNER: &str = "data/images/welcome-banner.png";

pub struct WelcomePage {
    branding: Branding,
}

#[derive(Debug)]
pub enum WelcomeMsg {
    Begin,
    Retranslate,
}

#[relm4::component(pub)]
impl SimpleComponent for WelcomePage {
    type Init = Branding;
    type Input = WelcomeMsg;
    type Output = PageOutput;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 24,
            set_halign: gtk::Align::Center,
            set_valign: gtk::Align::Center,
            set_margin_top: 24,
            set_margin_bottom: 32,

            #[name = "banner"]
            gtk::Picture {
                set_width_request: 760,
                set_height_request: 300,
                set_content_fit: gtk::ContentFit::Cover,
                set_can_shrink: true,
                add_css_class: "welcome-banner",
            },

            gtk::Label {
                add_css_class: "title-1",
                #[watch]
                set_label: &welcome_title(&model.branding),
            },

            gtk::Button {
                add_css_class: "install-pill",
                add_css_class: "suggested-action",
                set_halign: gtk::Align::Center,
                #[watch]
                set_label: &button_label(&model.branding),
                connect_clicked => WelcomeMsg::Begin,
            },
        }
    }

    fn init(
        branding: Self::Init,
        _root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = WelcomePage { branding };
        let widgets = view_output!();
        widgets
            .banner
            .set_filename(Some(banner_path(&model.branding)));
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            WelcomeMsg::Begin => {
                sender.output(PageOutput::RequestNext).ok();
            }
            WelcomeMsg::Retranslate => {}
        }
    }
}

fn banner_path(branding: &Branding) -> &str {
    let configured = branding
        .welcome_banner
        .as_deref()
        .unwrap_or(FALLBACK_BANNER);
    if Path::new(configured).is_file() {
        configured
    } else if Path::new(DEV_BANNER).is_file() {
        DEV_BANNER
    } else {
        configured
    }
}

fn welcome_title(branding: &Branding) -> String {
    branding
        .name
        .as_ref()
        .map(|name| gettext("Welcome to {name}").replace("{name}", name))
        .unwrap_or_else(|| gettext("Welcome"))
}

fn button_label(branding: &Branding) -> String {
    branding
        .welcome_button
        .as_ref()
        .map(|label| label.replace("{name}", branding.name.as_deref().unwrap_or_default()))
        .unwrap_or_else(|| gettext("Start Installation"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_template_expands_distro_name() {
        let branding = Branding {
            name: Some("Example OS".into()),
            welcome_button: Some("Install {name}".into()),
            ..Branding::default()
        };
        assert_eq!(button_label(&branding), "Install Example OS");
        assert_eq!(welcome_title(&branding), "Welcome to Example OS");
    }
}
