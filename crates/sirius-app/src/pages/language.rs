//! Language picker mirroring GNOME Initial Setup's `CcLanguageChooser`: a
//! search entry above a boxed list with the common languages pinned, a
//! "More…" row revealing every other locale, and a checkmark on the active
//! one. Locale data comes from GNOME Desktop (see `locale`).

pub(crate) mod locale;

use super::PageOutput;
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, adw, gtk};
use sirius_core::Branding;
use std::path::Path;

pub struct LanguagePage {
    root: adw::StatusPage,
    heading: gtk::Label,
    search: gtk::SearchEntry,
    list: gtk::ListBox,
    no_results: gtk::Label,
    locales: Vec<locale::LocaleEntry>,
    /// Locale indices currently shown as rows, in row order.
    visible: Vec<usize>,
    selected: usize,
}

#[derive(Debug)]
pub enum LanguageMsg {
    SearchChanged(String),
    RowActivated(usize),
    Retranslate,
}

pub struct LanguagePageWidgets;

impl SimpleComponent for LanguagePage {
    type Init = Branding;
    type Input = LanguageMsg;
    type Output = PageOutput;
    type Root = adw::StatusPage;
    type Widgets = LanguagePageWidgets;

    fn init_root() -> Self::Root {
        adw::StatusPage::new()
    }

    fn init(
        branding: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        apply_header(&root);

        let locales = locale::load();
        let selected = locale::current_locale()
            .and_then(|current| locales.iter().position(|entry| entry.id == current))
            .or_else(|| locales.iter().position(|entry| entry.id == "en_US"))
            .unwrap_or(0);

        let (content, heading, search, list, no_results) = language_content(&branding, &sender);
        root.set_child(Some(&content));

        let mut model = LanguagePage {
            root: root.clone(),
            heading,
            search,
            list,
            no_results,
            locales,
            visible: Vec::new(),
            selected,
        };
        model.rebuild_list("");
        model.emit_selection(&sender);

        ComponentParts {
            model,
            widgets: LanguagePageWidgets,
        }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            LanguageMsg::SearchChanged(query) => self.rebuild_list(&query),
            LanguageMsg::RowActivated(index) => {
                if let Some(locale_index) = self.visible.get(index).copied() {
                    self.selected = locale_index;
                    self.emit_selection(&sender);
                    self.rebuild_list(&self.search.text());
                }
            }
            LanguageMsg::Retranslate => {
                apply_header(&self.root);
                self.heading.set_label(&gettext("Choose your language"));
                self.no_results.set_label(&gettext("No languages found"));
                // The native and country names are stable, but the
                // current-language names behind search_text follow the UI
                // language, so reload before rebuilding.
                let selected_id = self.locales[self.selected].id.clone();
                self.locales = locale::load();
                self.selected = self
                    .locales
                    .iter()
                    .position(|entry| entry.id == selected_id)
                    .unwrap_or(0);
                self.rebuild_list("");
            }
        }
    }
}

impl LanguagePage {
    fn rebuild_list(&mut self, query: &str) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }

        self.visible = visible_indices(&self.locales, query);

        for index in self.visible.clone() {
            let entry = &self.locales[index];
            let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            row_box.set_margin_top(12);
            row_box.set_margin_bottom(12);
            row_box.set_margin_start(12);
            row_box.set_margin_end(12);

            let name = gtk::Label::new(Some(&entry.name_native));
            name.set_xalign(0.0);
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            name.set_max_width_chars(30);
            row_box.append(&name);

            if let Some(country) = &entry.country_native {
                let label = gtk::Label::new(Some(country));
                label.add_css_class("dim-label");
                label.set_xalign(0.0);
                label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                label.set_max_width_chars(30);
                label.set_hexpand(true);
                label.set_halign(gtk::Align::End);
                row_box.append(&label);
            }

            let row = gtk::ListBoxRow::new();
            row.set_activatable(true);
            row.set_child(Some(&row_box));
            self.list.append(&row);
        }

        // The selection shows as the row's default Adwaita selected
        // background — no check mark.
        if let Some(position) = self
            .visible
            .iter()
            .position(|index| *index == self.selected)
            && let Some(row) = self.list.row_at_index(position as i32)
        {
            self.list.select_row(Some(&row));
        }
    }

    fn emit_selection(&self, sender: &ComponentSender<Self>) {
        sender
            .output(PageOutput::SetLocale(
                self.locales[self.selected].id.clone(),
            ))
            .ok();
    }
}

/// Row set shown for a query: while searching, every matching locale; without
/// a query, the full list (pinned languages first, then the rest — the list
/// scrolls, so there is no "More…" expander).
fn visible_indices(locales: &[locale::LocaleEntry], query: &str) -> Vec<usize> {
    let query = locale::normalize(query);
    let searching = !query.is_empty();
    locales
        .iter()
        .enumerate()
        .filter(|(_, entry)| !searching || entry.matches(&query))
        .map(|(index, _)| index)
        .collect()
}

fn apply_header(root: &adw::StatusPage) {
    super::status_header(
        root,
        &gettext("Language"),
        &gettext("Choose the language used during installation."),
    );
}

fn language_content(
    branding: &Branding,
    sender: &ComponentSender<LanguagePage>,
) -> (
    gtk::Box,
    gtk::Label,
    gtk::SearchEntry,
    gtk::ListBox,
    gtk::Label,
) {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 72);
    content.set_width_request(760);
    content.set_halign(gtk::Align::Center);
    content.set_valign(gtk::Align::Center);
    content.append(&branding_view(branding));

    let choices = gtk::Box::new(gtk::Orientation::Vertical, 12);
    choices.set_hexpand(true);
    let heading = gtk::Label::new(Some(&gettext("Choose your language")));
    heading.add_css_class("title-2");
    heading.set_halign(gtk::Align::Start);
    choices.append(&heading);

    let search = gtk::SearchEntry::new();
    search.set_hexpand(true);
    {
        let sender = sender.clone();
        search.connect_search_changed(move |entry| {
            sender.input(LanguageMsg::SearchChanged(entry.text().to_string()));
        });
    }
    choices.append(&search);

    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.add_css_class("boxed-list");
    list.set_valign(gtk::Align::Start);
    let no_results = gtk::Label::new(Some(&gettext("No languages found")));
    no_results.add_css_class("dim-label");
    no_results.set_margin_top(12);
    no_results.set_margin_bottom(12);
    no_results.set_sensitive(false);
    list.set_placeholder(Some(&no_results));
    {
        let sender = sender.clone();
        list.connect_row_activated(move |_, row| {
            sender.input(LanguageMsg::RowActivated(row.index() as usize));
        });
    }

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_min_content_height(250);
    scroll.set_max_content_height(300);
    scroll.add_css_class("chooser-scroll");
    scroll.set_child(Some(&list));
    choices.append(&scroll);
    content.append(&choices);
    (content, heading, search, list, no_results)
}

fn branding_view(branding: &Branding) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 16);
    column.set_width_request(260);
    column.set_halign(gtk::Align::Center);
    column.set_valign(gtk::Align::Center);

    if let Some(path) = branding
        .logo
        .as_deref()
        .filter(|path| Path::new(path).is_file())
    {
        // A Picture would let the logo's intrinsic pixel size win over the
        // request and grow with the window; Image + pixel_size caps it like
        // the themed-icon fallback below.
        let image = gtk::Image::from_file(path);
        image.set_pixel_size(128);
        column.append(&image);
    } else {
        let image =
            gtk::Image::from_icon_name(branding.icon.as_deref().unwrap_or("starred-symbolic"));
        image.set_pixel_size(128);
        column.append(&image);
    }
    if let Some(name) = branding.name.as_deref() {
        let label = gtk::Label::new(Some(name));
        label.add_css_class("title-1");
        column.append(&label);
    }
    column
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_a_query_everything_shows_pinned_first() {
        let locales = locale::load();
        let visible = visible_indices(&locales, "");
        assert_eq!(visible.len(), locales.len(), "no More… gate: show all");
        let first_extra = visible.iter().position(|index| !locales[*index].is_initial);
        if let Some(first_extra) = first_extra {
            assert!(
                visible[..first_extra]
                    .iter()
                    .all(|index| locales[*index].is_initial)
            );
        }
    }

    #[test]
    fn searching_reaches_extra_locales() {
        let locales = locale::load();
        let extra = locales
            .iter()
            .find(|entry| !entry.is_initial)
            .expect("extra locales must exist");
        let term = locale::normalize(extra.name_native.split_whitespace().next().unwrap());
        let visible = visible_indices(&locales, &term);
        assert!(visible.iter().any(|index| locales[*index].id == extra.id));
    }
}
