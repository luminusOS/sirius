//! Keyboard chooser mirroring GNOME Initial Setup's `CcInputChooser`: a
//! search entry above a boxed list with the locale's layouts pinned, a
//! "More…" row revealing every other XKB layout, and a checkmark on the
//! active one. Re-activating the selected row confirms the choice and moves
//! on, like `confirm_choice`. Layout data comes from GNOME Desktop (see
//! `xkb`).

mod xkb;

use self::xkb::Layout;
use super::PageOutput;
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, adw, gtk};

pub struct KeyboardPage {
    root: adw::StatusPage,
    heading: gtk::Label,
    search: gtk::SearchEntry,
    list: gtk::ListBox,
    no_results: gtk::Label,
    test_entry: gtk::Entry,
    input_settings: gtk::gio::Settings,
    layouts: Vec<Layout>,
    /// Locale-relevant layout ids shown before the "More…" row is expanded.
    initial_ids: Vec<String>,
    /// Layout indices currently shown as rows, in row order.
    visible: Vec<usize>,
    /// Whether a "More…" row is appended after the visible layouts.
    more_row: bool,
    selected: usize,
    showing_extra: bool,
    user_selected: bool,
}

#[derive(Debug)]
pub enum KeyboardMsg {
    SearchChanged(String),
    RowActivated(usize),
    Retranslate,
}

pub struct KeyboardPageWidgets;

impl SimpleComponent for KeyboardPage {
    type Init = ();
    type Input = KeyboardMsg;
    type Output = PageOutput;
    type Root = adw::StatusPage;
    type Widgets = KeyboardPageWidgets;

    fn init_root() -> Self::Root {
        adw::StatusPage::new()
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        apply_header(&root);
        let layouts = xkb::load();
        let initial_ids = xkb::initial_layout_ids(&current_locale());
        let selected = recommended_layout(&layouts, &initial_ids);
        let (content, heading, search, list, no_results, test_entry) = keyboard_content(&sender);
        root.set_child(Some(&content));

        let mut model = KeyboardPage {
            root: root.clone(),
            heading,
            search,
            list,
            no_results,
            test_entry,
            input_settings: gtk::gio::Settings::new("org.gnome.desktop.input-sources"),
            layouts,
            initial_ids,
            visible: Vec::new(),
            more_row: false,
            selected,
            showing_extra: false,
            user_selected: false,
        };
        model.rebuild_list("");
        model.emit_selection(&sender);

        ComponentParts {
            model,
            widgets: KeyboardPageWidgets,
        }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            KeyboardMsg::SearchChanged(query) => self.rebuild_list(&query),
            KeyboardMsg::RowActivated(index) => {
                if self.more_row && index == self.visible.len() {
                    self.showing_extra = true;
                    self.rebuild_list(&self.search.text());
                } else if let Some(layout_index) = self.visible.get(index).copied() {
                    if layout_index == self.selected {
                        // Re-activating the selected row confirms the choice
                        // (CcInputChooser::confirm_choice).
                        sender.output(PageOutput::RequestNext).ok();
                    } else {
                        self.selected = layout_index;
                        self.user_selected = true;
                        self.emit_selection(&sender);
                        self.rebuild_list(&self.search.text());
                        self.test_entry.grab_focus();
                    }
                }
            }
            KeyboardMsg::Retranslate => {
                apply_header(&self.root);
                self.heading
                    .set_label(&gettext("Select your keyboard layout"));
                self.search
                    .set_placeholder_text(Some(&gettext("Search keyboard layouts")));
                self.no_results.set_label(&gettext("No inputs found"));
                self.test_entry
                    .set_placeholder_text(Some(&gettext("Type here to test your layout")));

                let selected_id = self.layouts[self.selected].id.clone();
                self.layouts = xkb::load();
                self.initial_ids = xkb::initial_layout_ids(&current_locale());
                self.selected = if self.user_selected {
                    self.layouts
                        .iter()
                        .position(|layout| layout.id == selected_id)
                        .unwrap_or_else(|| recommended_layout(&self.layouts, &self.initial_ids))
                } else {
                    recommended_layout(&self.layouts, &self.initial_ids)
                };
                self.rebuild_list(&self.search.text());
                self.emit_selection(&sender);
            }
        }
    }
}

impl KeyboardPage {
    fn is_initial(&self, index: usize) -> bool {
        self.initial_ids.contains(&self.layouts[index].id)
    }

    fn rebuild_list(&mut self, query: &str) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }

        let query = xkb::normalize(query);
        let searching = !query.is_empty();
        // Mirror sort_inputs: locale-relevant layouts first, then the extras,
        // each group alphabetical (layouts are loaded pre-sorted by name).
        let mut order: Vec<usize> = (0..self.layouts.len()).collect();
        order.sort_by_key(|index| !self.is_initial(*index));

        self.visible.clear();
        self.more_row = false;
        for index in order {
            let layout = &self.layouts[index];
            let show = if searching {
                layout.search_text.contains(&query)
            } else {
                self.is_initial(index) || self.showing_extra
            };
            if !show {
                continue;
            }

            let row = adw::ActionRow::new();
            row.set_title(&layout.name);
            row.set_activatable(true);
            row.add_suffix(&super::choice_list::selected_indicator(
                index == self.selected,
            ));
            self.list.append(&row);
            self.visible.push(index);
        }

        let has_extra = self
            .layouts
            .iter()
            .enumerate()
            .any(|(index, _)| !self.is_initial(index));
        if !searching && !self.showing_extra && has_extra {
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
            self.more_row = true;
        }
    }

    fn emit_selection(&self, sender: &ComponentSender<Self>) {
        let id = self.layouts[self.selected].id.clone();
        // Match GNOME Initial Setup: update the live session source as soon as
        // it is selected so the test field really uses that layout.
        let _ = self
            .input_settings
            .set("sources", vec![("xkb".to_string(), id.clone())]);
        let _ = self.input_settings.set_uint("current", 0);
        sender.output(PageOutput::SetKeyboard(id)).ok();
    }
}

fn keyboard_content(
    sender: &ComponentSender<KeyboardPage>,
) -> (
    gtk::Box,
    gtk::Label,
    gtk::SearchEntry,
    gtk::ListBox,
    gtk::Label,
    gtk::Entry,
) {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 72);
    content.set_width_request(760);
    content.set_halign(gtk::Align::Center);
    content.set_valign(gtk::Align::Center);

    let illustration = gtk::Image::from_icon_name("input-keyboard-symbolic");
    illustration.set_pixel_size(160);
    illustration.set_width_request(260);
    content.append(&illustration);

    let choices = gtk::Box::new(gtk::Orientation::Vertical, 12);
    choices.set_hexpand(true);
    let heading = gtk::Label::new(Some(&gettext("Select your keyboard layout")));
    heading.add_css_class("title-2");
    heading.set_halign(gtk::Align::Start);
    choices.append(&heading);

    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some(&gettext("Search keyboard layouts")));
    {
        let sender = sender.clone();
        search.connect_search_changed(move |entry| {
            sender.input(KeyboardMsg::SearchChanged(entry.text().to_string()));
        });
    }
    choices.append(&search);

    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.add_css_class("boxed-list");
    list.set_valign(gtk::Align::Start);
    let no_results = gtk::Label::new(Some(&gettext("No inputs found")));
    no_results.add_css_class("dim-label");
    no_results.set_margin_top(12);
    no_results.set_margin_bottom(12);
    no_results.set_sensitive(false);
    list.set_placeholder(Some(&no_results));
    {
        let sender = sender.clone();
        list.connect_row_activated(move |_, row| {
            sender.input(KeyboardMsg::RowActivated(row.index() as usize));
        });
    }
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_min_content_height(190);
    scroll.set_max_content_height(250);
    scroll.set_child(Some(&list));
    choices.append(&scroll);

    let entry = gtk::Entry::new();
    entry.set_placeholder_text(Some(&gettext("Type here to test your layout")));
    choices.append(&entry);
    content.append(&choices);
    (content, heading, search, list, no_results, entry)
}

fn apply_header(root: &adw::StatusPage) {
    super::status_header(
        root,
        &gettext("Keyboard layout"),
        &gettext("Select the keyboard layout you want to use."),
    );
}

/// The current UI locale, from `LANGUAGE`'s first entry or `LANG`.
fn current_locale() -> String {
    std::env::var("LANGUAGE")
        .ok()
        .and_then(|value| value.split(':').next().map(str::to_owned))
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var("LANG").ok())
        .filter(|value| !value.is_empty() && value != "C" && value != "POSIX")
        .unwrap_or_else(|| "en_US".into())
}

/// The locale's default layout when known, otherwise the first entry.
fn recommended_layout(layouts: &[Layout], initial_ids: &[String]) -> usize {
    initial_ids
        .iter()
        .find_map(|id| layouts.iter().position(|layout| &layout.id == id))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portuguese_recommends_brazilian_xkb() {
        let layouts = xkb::load();
        let initial_ids = xkb::initial_layout_ids("pt_BR");
        assert!(initial_ids.contains(&"br".to_string()));
        assert_eq!(layouts[recommended_layout(&layouts, &initial_ids)].id, "br");
    }

    #[test]
    fn initial_ids_cover_the_locale_language_and_country() {
        let initial_ids = xkb::initial_layout_ids("pt_BR.UTF-8");
        assert!(initial_ids.contains(&"br".to_string()));
        // Only a handful of rows should sit in front of the More… row.
        assert!(initial_ids.len() < 20, "got {initial_ids:?}");
    }

    #[test]
    fn variant_ids_remain_canonical() {
        let layouts = xkb::load();
        let variant = layouts
            .iter()
            .find(|layout| layout.xkb_variant.is_some())
            .unwrap();
        assert_eq!(
            variant.id,
            format!(
                "{}+{}",
                variant.xkb_layout,
                variant.xkb_variant.as_deref().unwrap()
            )
        );
    }
}
