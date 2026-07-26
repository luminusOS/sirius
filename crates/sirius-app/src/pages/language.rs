//! Language picker mirroring GNOME Initial Setup's `CcLanguageChooser`: a
//! search entry above a boxed list with the common languages pinned, a
//! "More…" row revealing every other locale, and a checkmark on the active
//! one. Locale data comes from GNOME Desktop (see `locale`).

mod locale;

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
    /// Whether a "More…" row is appended after the visible locales.
    more_row: bool,
    selected: usize,
    showing_extra: bool,
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
            more_row: false,
            selected,
            showing_extra: false,
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
                if self.more_row && index == self.visible.len() {
                    self.showing_extra = true;
                    self.rebuild_list(&self.search.text());
                } else if let Some(locale_index) = self.visible.get(index).copied() {
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

        let (visible, more_row) = visible_indices(&self.locales, query, self.showing_extra);
        self.more_row = more_row;
        self.visible = visible;

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

            row_box.append(&super::choice_list::selected_indicator(
                index == self.selected,
            ));

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

        if self.more_row {
            let arrow = gtk::Image::from_icon_name("view-more-symbolic");
            arrow.add_css_class("dim-label");
            arrow.set_hexpand(true);
            arrow.set_halign(gtk::Align::Center);
            arrow.set_margin_top(12);
            arrow.set_margin_bottom(12);
            let row = gtk::ListBoxRow::new();
            row.set_activatable(true);
            row.set_tooltip_text(Some(&gettext("More…")));
            row.set_child(Some(&arrow));
            self.list.append(&row);
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

/// Row set shown for a query, mirroring `language_visible`: while searching,
/// every matching locale (initial or extra) shows and the "More…" row hides;
/// otherwise only the pinned entries show until the extras are expanded.
/// Returns the visible locale indices plus whether a "More…" row follows.
fn visible_indices(
    locales: &[locale::LocaleEntry],
    query: &str,
    showing_extra: bool,
) -> (Vec<usize>, bool) {
    let query = locale::normalize(query);
    let searching = !query.is_empty();
    let visible = locales
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            if searching {
                entry.matches(&query)
            } else {
                entry.is_initial || showing_extra
            }
        })
        .map(|(index, _)| index)
        .collect();
    let more_row = !searching && !showing_extra && locales.iter().any(|entry| !entry.is_initial);
    (visible, more_row)
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
    list.set_selection_mode(gtk::SelectionMode::None);
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
        let picture = gtk::Picture::for_filename(path);
        picture.set_width_request(220);
        picture.set_height_request(160);
        picture.set_content_fit(gtk::ContentFit::Contain);
        column.append(&picture);
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
    fn without_a_query_only_pinned_languages_show_behind_more() {
        let locales = locale::load();
        let (visible, more_row) = visible_indices(&locales, "", false);
        assert!(!visible.is_empty());
        assert!(visible.iter().all(|index| locales[*index].is_initial));
        assert!(more_row, "the More… row must gate the extra locales");

        let (expanded, more_row) = visible_indices(&locales, "", true);
        assert!(expanded.len() > visible.len());
        assert!(!more_row);
    }

    #[test]
    fn searching_reaches_extra_locales_and_hides_more() {
        let locales = locale::load();
        let extra = locales
            .iter()
            .find(|entry| !entry.is_initial)
            .expect("extra locales must exist");
        let term = locale::normalize(extra.name_native.split_whitespace().next().unwrap());
        let (visible, more_row) = visible_indices(&locales, &term, false);
        assert!(visible.iter().any(|index| locales[*index].id == extra.id));
        assert!(!more_row);
    }
}
