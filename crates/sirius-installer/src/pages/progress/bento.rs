//! Optional distro link cards shown during installation.

use crate::backend::distro::Bento;
use relm4::adw::prelude::*;
use relm4::gtk;

const MAX_BENTOS: usize = 3;

pub(super) fn append_cards(container: &gtk::Box, bentos: &[Bento]) {
    for bento in bentos.iter().take(MAX_BENTOS) {
        container.append(&card(bento));
    }
}

fn themed_icon(name: &str) -> &str {
    let has = gtk::gdk::Display::default()
        .map(|display| gtk::IconTheme::for_display(&display).has_icon(name))
        .unwrap_or(false);
    if has {
        name
    } else {
        tracing::warn!("bento icon '{name}' not in icon theme; using fallback");
        "web-browser-symbolic"
    }
}

fn card(bento: &Bento) -> gtk::Button {
    let title = gtk::Label::builder()
        .label(&bento.title)
        .halign(gtk::Align::Start)
        .wrap(true)
        .css_classes(["heading"])
        .build();
    let description = gtk::Label::builder()
        .label(&bento.desc)
        .halign(gtk::Align::Start)
        .wrap(true)
        .css_classes(["dim-label", "caption"])
        .build();
    let text = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .valign(gtk::Align::Center)
        .build();
    text.append(&title);
    text.append(&description);

    let icon = gtk::Image::builder()
        .icon_name(themed_icon(&bento.icon))
        .pixel_size(32)
        .valign(gtk::Align::Center)
        .css_classes(["accent"])
        .build();
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    content.append(&icon);
    content.append(&text);

    let card = gtk::Button::builder()
        .child(&content)
        .hexpand(true)
        .css_classes(["card"])
        .build();
    let link = bento.link.clone();
    card.connect_clicked(move |button| {
        gtk::UriLauncher::new(&link).launch(
            button.root().and_downcast_ref::<gtk::Window>(),
            gtk::gio::Cancellable::NONE,
            |_| {},
        );
    });
    card
}
