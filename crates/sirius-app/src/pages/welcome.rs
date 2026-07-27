//! Branded installer entry point shown before any configuration question.
//!
//! Mirrors GNOME Initial Setup's welcome: an edge-to-edge banner on top and a
//! greeting that cycles "Welcome" through the common languages every five
//! seconds (`GisWelcomeWidget`).

use super::PageOutput;
use super::language::locale;
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, gtk};
use sirius_core::Branding;
use std::path::Path;
use std::rc::Rc;

const FALLBACK_BANNER: &str = "/usr/share/sirius/welcome-banner.png";
const DEV_BANNER: &str = "data/images/welcome-banner.png";
const GREETING_INTERVAL_SECONDS: u32 = 5;

/// "Welcome" in each pinned language page locale, filtered at runtime by font
/// coverage like `GisWelcomeWidget` does.
const GREETINGS: &[(&str, &str)] = &[
    ("en_US", "Welcome"),
    ("pt_BR", "Boas-vindas"),
    ("de_DE", "Willkommen"),
    ("fr_FR", "Bienvenue"),
    ("es_ES", "Bienvenidos"),
    ("zh_CN", "欢迎"),
    ("ja_JP", "ようこそ"),
    ("ru_RU", "Добро пожаловать"),
    ("ar_EG", "مرحباً"),
];

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
            set_hexpand: true,
            set_vexpand: true,

            #[name = "banner"]
            gtk::Picture {
                set_hexpand: true,
                set_height_request: 300,
                set_content_fit: gtk::ContentFit::Cover,
                set_can_shrink: true,
                set_valign: gtk::Align::Start,
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 24,
                set_vexpand: true,
                set_halign: gtk::Align::Center,
                set_valign: gtk::Align::Center,
                set_margin_bottom: 32,

                #[name = "greetings"]
                adw::Carousel {
                    set_interactive: false,
                    set_halign: gtk::Align::Center,
                },

                gtk::Button {
                    add_css_class: "install-pill",
                    add_css_class: "suggested-action",
                    set_halign: gtk::Align::Center,
                    #[watch]
                    set_label: &button_label(&model.branding),
                    connect_clicked => WelcomeMsg::Begin,
                },
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

        // One title-1 label per displayable greeting (GisWelcomeWidget skips
        // languages the system cannot render).
        for (locale_id, greeting) in GREETINGS {
            if !locale::initial_locale_ids().contains(locale_id) || !has_font(locale_id) {
                continue;
            }
            let label = gtk::Label::new(Some(greeting));
            label.add_css_class("title-1");
            widgets.greetings.append(&label);
        }
        cycle_greetings(&widgets.greetings);

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

/// Auto-advance the greeting carousel every few seconds while it is mapped,
/// mirroring `GisWelcomeWidget`'s 5-second timeout.
fn cycle_greetings(carousel: &adw::Carousel) {
    let source = Rc::new(std::cell::RefCell::new(None::<gtk::glib::SourceId>));

    {
        let map_source = source.clone();
        carousel.connect_map(move |carousel| {
            if map_source.borrow().is_some() {
                return;
            }
            let carousel = carousel.clone();
            let id = gtk::glib::timeout_add_seconds_local(GREETING_INTERVAL_SECONDS, move || {
                let pages = carousel.n_pages();
                if pages > 0 {
                    let next = ((carousel.position().ceil() as u32) + 1) % pages;
                    let page = carousel.nth_page(next);
                    carousel.scroll_to(&page, true);
                }
                gtk::glib::ControlFlow::Continue
            });
            map_source.borrow_mut().replace(id);
        });
    }
    carousel.connect_unmap(move |_| {
        if let Some(id) = source.borrow_mut().take() {
            id.remove();
        }
    });
}

/// Font coverage for a locale's language code, like
/// `cc_common_language_has_font`.
fn has_font(locale_id: &str) -> bool {
    let language = locale_id
        .split(['.', '@'])
        .next()
        .unwrap_or(locale_id)
        .split('_')
        .next()
        .unwrap_or(locale_id);
    locale::has_font(language)
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
    }

    #[test]
    fn greetings_cover_every_pinned_locale() {
        for (locale_id, _) in GREETINGS {
            assert!(
                locale::initial_locale_ids().contains(locale_id),
                "{locale_id} has a greeting but is not a pinned locale"
            );
        }
    }

    // Interactive test: needs a display (skipped on headless CI). The banner
    // must span the page edge-to-edge and the greeting carousel must hold
    // the displayable greetings, GNOME Initial Setup style.
    #[test]
    fn banner_runs_edge_to_edge_and_greetings_fill_the_carousel() {
        use relm4::{Component, ComponentController};

        if std::env::var_os("WAYLAND_DISPLAY").is_none() && std::env::var_os("DISPLAY").is_none() {
            eprintln!("skipping interactive test: no display available");
            return;
        }
        crate::pages::testutil::run_on_gtk_thread(|| {
            let controller = WelcomePage::builder().launch(Branding::default());
            let window = adw::Window::new();
            window.set_content(Some(controller.widget()));
            window.set_default_size(960, 640);
            window.present();

            let context = gtk::glib::MainContext::default();
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(800);
            while std::time::Instant::now() < deadline {
                while context.pending() {
                    context.iteration(false);
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }

            let widgets = controller.widgets();
            assert_eq!(
                widgets.banner.width(),
                controller.widget().width(),
                "the banner must run edge-to-edge with the page"
            );
            assert!(
                widgets.greetings.n_pages() >= 2,
                "at least the English and Portuguese greetings must be present"
            );
            let first = widgets.greetings.nth_page(0);
            assert!(
                first.has_css_class("title-1"),
                "greetings use the title-1 style like GisWelcomeWidget"
            );
            window.close();
        });
    }
}
