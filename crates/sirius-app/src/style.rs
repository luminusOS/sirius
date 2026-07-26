use relm4::gtk;

pub fn load() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("../../../data/style.css"));
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
