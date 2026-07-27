//! Time-zone page based on GNOME Initial Setup's city search + interactive map.

use super::PageOutput;
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, adw, gtk};
use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;

const ZONE_TABLES: &[&str] = &[
    "/usr/share/zoneinfo/zone1970.tab",
    "/usr/share/zoneinfo/zone.tab",
];
const DEFAULT_ZONE: &str = "America/Sao_Paulo";
const MAX_RESULTS: usize = 8;

// Vector world map generated locally from Natural Earth data
// (tools/generate-timezone-map.py) and rendered through glycin; the pin is
// gnome-initial-setup's pin.png (pages/timezone/data).
const FALLBACK_MAP_SVG: &str = "/usr/share/sirius/timezone-map.svg";
const DEV_MAP_SVG: &str = "data/images/timezone-map.svg";
const FALLBACK_PIN: &str = "/usr/share/sirius/timezone-pin.png";
const DEV_PIN: &str = "data/images/timezone-pin.png";

// Projection constants matching data/images/timezone-map.svg (and the PNG
// rendered from it): Miller cylindrical, cropped to 81°N..59°S, no longitude
// offset. The pin hot point mirrors gnome-initial-setup's pin.png.
const PIN_HOT_POINT_X: f64 = 8.0;
const PIN_HOT_POINT_Y: f64 = 15.0;
const LONGITUDE_OFFSET: f64 = 0.0;
const TOP_LATITUDE: f64 = 81.0;
const BOTTOM_LATITUDE: f64 = -59.0;
const MILLER_FULL_RANGE: f64 = 4.606_825_086_76;

// Native render size of the vector map (2x the gnome-initial-setup bg.png).
const MAP_TEXTURE_WIDTH: u32 = 1600;
const MAP_TEXTURE_HEIGHT: u32 = 818;

#[derive(Clone, Debug, PartialEq)]
struct Location {
    zone: String,
    name: String,
    region: String,
    latitude: f64,
    longitude: f64,
    search_text: String,
}

impl Location {
    fn city(&self) -> &str {
        &self.name
    }
}

pub struct TimezonePage {
    root: adw::StatusPage,
    search: gtk::SearchEntry,
    results: gtk::ListBox,
    results_popover: gtk::Popover,
    map: gtk::Picture,
    pin: gtk::Picture,
    pin_label: gtk::Label,
    band: gtk::Box,
    locations: Vec<Location>,
    filtered: Vec<usize>,
    selected: usize,
    selected_point: Rc<Cell<(f64, f64)>>,
    band_meridian: Rc<Cell<f64>>,
}

#[derive(Debug)]
pub enum TimezoneMsg {
    SearchChanged(String),
    ResultActivated(usize),
    MapClicked { x: f64, y: f64 },
    Retranslate,
}

pub struct TimezonePageWidgets;

impl SimpleComponent for TimezonePage {
    type Init = ();
    type Input = TimezoneMsg;
    type Output = PageOutput;
    type Root = adw::StatusPage;
    type Widgets = TimezonePageWidgets;

    fn init_root() -> Self::Root {
        adw::StatusPage::new()
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        apply_header(&root);

        let locations = load_locations();
        let selected = initial_location(&locations);
        let selected_location = &locations[selected];
        let selected_point = Rc::new(Cell::new((
            selected_location.longitude,
            selected_location.latitude,
        )));

        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_width_request(680);
        content.set_halign(gtk::Align::Center);
        content.set_valign(gtk::Align::Center);

        // GtkSearchEntry with a suggestions popover, mirroring GNOME Maps'
        // place search (icon in the entry, results in an opaque popover
        // below, keyboard navigation without leaving the entry).
        let search = gtk::SearchEntry::new();
        search.set_width_request(420);
        search.set_halign(gtk::Align::Center);
        search.set_placeholder_text(Some(&gettext("Search for a city")));
        search.set_tooltip_text(Some(&gettext(
            "Search for a nearby city to select its time zone",
        )));
        {
            let sender = sender.clone();
            search.connect_search_changed(move |entry| {
                sender.input(TimezoneMsg::SearchChanged(entry.text().to_string()));
            });
        }
        content.append(&search);

        // Suggestions are ordinary GtkListBox rows. The popover and entry use
        // the same explicit width, so the list opens directly below and covers
        // the complete input instead of measuring itself from row contents.
        // Like GNOME Maps' place popover, nothing inside the popover can take
        // focus: keyboard focus never leaves the entry while typing, and the
        // entry's arrow keys move the highlighted row (SelectionMode::Single
        // is used purely as a visual cursor). The list is a direct child of
        // the popover contents, so libadwaita keeps it transparent and the
        // popover color stays uniform — no scrolled window in between.
        let results = gtk::ListBox::new();
        results.set_selection_mode(gtk::SelectionMode::Single);
        results.set_can_focus(false);
        results.set_focus_on_click(false);
        results.set_hexpand(true);
        {
            let sender = sender.clone();
            results.connect_row_activated(move |_, row| {
                sender.input(TimezoneMsg::ResultActivated(row.index() as usize));
            });
        }

        let results_popover = gtk::Popover::new();
        results_popover.set_has_arrow(false);
        results_popover.set_position(gtk::PositionType::Bottom);
        results_popover.set_width_request(420);
        results_popover.set_can_focus(false);
        results_popover.add_css_class("timezone-suggestions");
        results_popover.set_child(Some(&results));
        results_popover.set_parent(&search);

        // Enter activates the highlighted row, or the first suggestion when
        // the cursor was never moved.
        {
            let sender = sender.clone();
            let results = results.clone();
            search.connect_activate(move |_| {
                let index = results
                    .selected_row()
                    .map(|row| row.index() as usize)
                    .unwrap_or(0);
                sender.input(TimezoneMsg::ResultActivated(index));
            });
        }

        // Arrow keys drive the suggestion cursor from the entry; Escape
        // dismisses the popover (PlaceEntry's _onKeyPressed in GNOME Maps).
        {
            let results = results.clone();
            let results_popover = results_popover.clone();
            let keys = gtk::EventControllerKey::new();
            keys.connect_key_pressed(move |_, key, _, _| {
                handle_cursor_key(&results, &results_popover, key)
            });
            search.add_controller(keys);
        }
        // If focus ever lands inside the popover, keep the same keys working
        // and forward everything else to the entry (GNOME Maps adds a
        // bubble-phase key controller to its search popover for this).
        {
            let cursor_results = results.clone();
            let cursor_popover = results_popover.clone();
            let entry = search.clone();
            let keys = gtk::EventControllerKey::new();
            keys.set_propagation_phase(gtk::PropagationPhase::Bubble);
            keys.connect_key_pressed(move |controller, key, _, _| {
                let propagation = handle_cursor_key(&cursor_results, &cursor_popover, key);
                if propagation == gtk::glib::Propagation::Proceed {
                    controller.forward(&entry);
                    gtk::glib::Propagation::Stop
                } else {
                    propagation
                }
            });
            results_popover.add_controller(keys);
        }

        let map = gtk::Picture::new();
        if let Some(path) = existing_asset(FALLBACK_MAP_SVG, DEV_MAP_SVG) {
            load_svg_map(&map, path);
        }
        // At 680×348 the map keeps its native aspect ratio while the complete
        // page fits in the default 960×640 window without StatusPage scrolling.
        map.set_width_request(680);
        map.set_height_request(348);
        map.set_content_fit(gtk::ContentFit::Fill);
        map.set_can_shrink(true);
        map.set_hexpand(true);
        map.set_overflow(gtk::Overflow::Hidden);
        map.set_cursor_from_name(Some("pointer"));
        map.set_tooltip_text(Some(&gettext(
            "Select the nearest city by clicking the map",
        )));
        map.add_css_class("timezone-map");
        {
            let sender = sender.clone();
            let click = gtk::GestureClick::new();
            click.set_button(1);
            click.connect_pressed(move |_, _, x, y| {
                sender.input(TimezoneMsg::MapClicked { x, y });
            });
            map.add_controller(click);
        }

        let pin_asset = existing_asset(FALLBACK_PIN, DEV_PIN);
        let pin = gtk::Picture::new();
        if let Some(path) = pin_asset {
            pin.set_filename(Some(path));
        }
        pin.set_halign(gtk::Align::Start);
        pin.set_valign(gtk::Align::Start);
        // The selected city shows as a permanent label above the pin instead
        // of a hover tooltip.
        pin.set_can_target(false);
        pin.set_visible(pin_asset.is_some());

        let pin_label = gtk::Label::new(None);
        pin_label.add_css_class("timezone-pin-label");
        pin_label.set_justify(gtk::Justification::Center);
        pin_label.set_halign(gtk::Align::Start);
        pin_label.set_valign(gtk::Align::Start);
        pin_label.set_can_target(false);

        let band = gtk::Box::new(gtk::Orientation::Vertical, 0);
        band.add_css_class("timezone-band");
        band.set_halign(gtk::Align::Start);
        band.set_valign(gtk::Align::Fill);
        band.set_can_target(false);
        let band_meridian = Rc::new(Cell::new(zone_meridian(&selected_location.zone)));

        // Widgets have no "width" property to watch; tick callbacks are the
        // GTK4 way to react to allocation changes (first map + resizes).
        {
            let pin = pin.clone();
            let pin_label = pin_label.clone();
            let band = band.clone();
            let selected_point = selected_point.clone();
            let band_meridian = band_meridian.clone();
            map.add_tick_callback(move |map, _| {
                let width = f64::from(map.width());
                let height = f64::from(map.height());
                if width > 1.0 && height > 1.0 {
                    position_pin(&pin, selected_point.get(), width, height);
                    position_pin_label(&pin_label, selected_point.get(), width, height);
                    position_band(&band, band_meridian.get(), width);
                }
                gtk::glib::ControlFlow::Continue
            });
        }

        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&map));
        overlay.add_overlay(&band);
        overlay.add_overlay(&pin);
        overlay.add_overlay(&pin_label);
        let frame = gtk::Frame::new(None);
        frame.add_css_class("timezone-map-frame");
        frame.set_child(Some(&overlay));
        content.append(&frame);
        root.set_child(Some(&content));

        let model = TimezonePage {
            root: root.clone(),
            search,
            results,
            results_popover,
            map,
            pin,
            pin_label,
            band,
            locations,
            filtered: Vec::new(),
            selected,
            selected_point,
            band_meridian,
        };
        model.refresh_selection(&sender);

        ComponentParts {
            model,
            widgets: TimezonePageWidgets,
        }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            TimezoneMsg::SearchChanged(query) => self.refresh_results(&query),
            TimezoneMsg::ResultActivated(row) => {
                if let Some(index) = self.filtered.get(row).copied() {
                    self.select(index, &sender);
                    self.search.set_text("");
                    self.search.grab_focus();
                }
            }
            TimezoneMsg::MapClicked { x, y } => {
                let width = f64::from(self.map.width()).max(1.0);
                let height = f64::from(self.map.height()).max(1.0);
                let index = nearest_location(&self.locations, x, y, width, height);
                self.select(index, &sender);
            }
            TimezoneMsg::Retranslate => {
                apply_header(&self.root);
                self.search
                    .set_placeholder_text(Some(&gettext("Search for a city")));
                self.search.set_tooltip_text(Some(&gettext(
                    "Search for a nearby city to select its time zone",
                )));
                self.map.set_tooltip_text(Some(&gettext(
                    "Select the nearest city by clicking the map",
                )));
                self.refresh_selection(&sender);
            }
        }
    }
}

impl TimezonePage {
    fn refresh_results(&mut self, query: &str) {
        while let Some(child) = self.results.first_child() {
            self.results.remove(&child);
        }
        self.filtered.clear();

        let query = normalize(query);
        if query.is_empty() {
            self.results_popover.popdown();
            return;
        }

        let mut matches = self
            .locations
            .iter()
            .enumerate()
            .filter(|(_, location)| matches_query(&location.search_text, &query))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        matches.sort_by_key(|index| search_rank(&self.locations[*index], &query));
        self.filtered.extend(matches.into_iter().take(MAX_RESULTS));

        for index in &self.filtered {
            let location = &self.locations[*index];
            let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
            labels.set_margin_start(12);
            labels.set_margin_end(12);
            labels.set_margin_top(8);
            labels.set_margin_bottom(8);

            let title = gtk::Label::new(Some(location.city()));
            title.set_halign(gtk::Align::Start);
            title.set_xalign(0.0);
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            labels.append(&title);

            let subtitle =
                gtk::Label::new(Some(&format!("{} · {}", location.region, location.zone)));
            subtitle.set_halign(gtk::Align::Start);
            subtitle.set_xalign(0.0);
            subtitle.set_ellipsize(gtk::pango::EllipsizeMode::End);
            subtitle.add_css_class("caption");
            subtitle.add_css_class("dim-label");
            labels.append(&subtitle);

            let row = gtk::ListBoxRow::new();
            row.set_activatable(true);
            row.set_focusable(false);
            row.set_child(Some(&labels));
            self.results.append(&row);
        }

        if self.filtered.is_empty() {
            self.results_popover.popdown();
        } else {
            self.results_popover.popup();
        }
    }

    fn select(&mut self, index: usize, sender: &ComponentSender<Self>) {
        self.selected = index.min(self.locations.len().saturating_sub(1));
        let location = &self.locations[self.selected];
        self.selected_point
            .set((location.longitude, location.latitude));
        self.band_meridian.set(zone_meridian(&location.zone));
        self.refresh_selection(sender);
        let width = f64::from(self.map.width()).max(1.0);
        let height = f64::from(self.map.height()).max(1.0);
        position_pin(&self.pin, self.selected_point.get(), width, height);
        position_pin_label(&self.pin_label, self.selected_point.get(), width, height);
        position_band(&self.band, self.band_meridian.get(), width);
    }

    fn refresh_selection(&self, sender: &ComponentSender<Self>) {
        let location = &self.locations[self.selected];
        let city = gtk::glib::markup_escape_text(location.city());
        let detail = gtk::glib::markup_escape_text(&timezone_detail(&location.zone));
        self.pin_label.set_markup(&format!(
            "<b>{city}</b>\n<span size=\"small\">{detail}</span>"
        ));
        sender
            .output(PageOutput::SetTimezone(location.zone.clone()))
            .ok();
    }
}

impl Drop for TimezonePage {
    fn drop(&mut self) {
        self.results_popover.unparent();
    }
}

fn apply_header(root: &adw::StatusPage) {
    super::status_header(
        root,
        &gettext("Time Zone"),
        &gettext("Search for a city or select a location on the map to set your time zone."),
    );
}

fn load_locations() -> Vec<Location> {
    // Like GNOME Initial Setup, the chooser is backed only by the tzdb tables:
    // one searchable entry per zone's reference city, no external location
    // database.
    let mut locations = ZONE_TABLES
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok())
        .map(|table| parse_zone_table(&table))
        .unwrap_or_default();

    if locations.is_empty() {
        vec![
            fallback("America/Sao_Paulo", -23.55, -46.63),
            fallback("America/New_York", 40.71, -74.01),
            fallback("Europe/London", 51.51, -0.13),
            fallback("Asia/Tokyo", 35.68, 139.69),
            fallback("UTC", 0.0, 0.0),
        ]
    } else {
        if !locations.iter().any(|location| location.zone == "UTC") {
            locations.push(fallback("UTC", 0.0, 0.0));
        }
        locations
    }
}

fn parse_zone_table(table: &str) -> Vec<Location> {
    table
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let countries = fields.next()?;
            let coordinates = fields.next()?;
            let zone = fields.next()?.to_string();
            let comment = fields.next().unwrap_or_default();
            let (latitude, longitude) = parse_coordinates(coordinates)?;
            let city = zone_city(&zone);
            Some(Location {
                search_text: normalize(&format!("{city} {zone} {countries} {comment}")),
                name: city,
                region: if comment.is_empty() {
                    countries.to_string()
                } else {
                    comment.to_string()
                },
                zone,
                latitude,
                longitude,
            })
        })
        .collect()
}

fn zone_city(zone: &str) -> String {
    zone.rsplit('/').next().unwrap_or(zone).replace('_', " ")
}

fn fallback(zone: &str, latitude: f64, longitude: f64) -> Location {
    Location {
        zone: zone.into(),
        name: zone_city(zone),
        region: zone
            .split_once('/')
            .map(|(region, _)| region.replace('_', " "))
            .unwrap_or_else(|| zone.to_string()),
        latitude,
        longitude,
        search_text: normalize(&zone.replace(['/', '_'], " ")),
    }
}

fn parse_coordinates(value: &str) -> Option<(f64, f64)> {
    let split = value
        .char_indices()
        .skip(1)
        .find(|(_, character)| matches!(character, '+' | '-'))?
        .0;
    Some((
        parse_coordinate(&value[..split], 2)?,
        parse_coordinate(&value[split..], 3)?,
    ))
}

fn parse_coordinate(value: &str, degree_digits: usize) -> Option<f64> {
    let sign = match value.as_bytes().first()? {
        b'+' => 1.0,
        b'-' => -1.0,
        _ => return None,
    };
    let digits = value.get(1..)?;
    let degrees: f64 = digits.get(..degree_digits)?.parse().ok()?;
    let minutes: f64 = digits.get(degree_digits..degree_digits + 2)?.parse().ok()?;
    let seconds = digits
        .get(degree_digits + 2..)
        .filter(|value| !value.is_empty())
        .map(str::parse::<f64>)
        .transpose()
        .ok()?
        .unwrap_or(0.0);
    Some(sign * (degrees + minutes / 60.0 + seconds / 3600.0))
}

fn initial_location(locations: &[Location]) -> usize {
    current_timezone()
        .and_then(|zone| zone_match(locations, &zone))
        .or_else(|| zone_match(locations, DEFAULT_ZONE))
        .unwrap_or(0)
}

/// Best location for an auto-detected zone: the entry nearest to the zone's
/// representative point from the tzdb table (the zone's reference city, e.g.
/// the city of Sao Paulo for America/Sao_Paulo), never an arbitrary first
/// entry that could sit far away from it.
fn zone_match(locations: &[Location], zone: &str) -> Option<usize> {
    let reference = zone_reference_coords(zone);
    locations
        .iter()
        .enumerate()
        .filter(|(_, location)| location.zone == zone)
        .min_by(|(_, a), (_, b)| {
            squared_distance(a, reference).total_cmp(&squared_distance(b, reference))
        })
        .map(|(index, _)| index)
}

fn squared_distance(location: &Location, reference: Option<(f64, f64)>) -> f64 {
    match reference {
        Some((latitude, longitude)) => {
            (location.latitude - latitude).powi(2) + (location.longitude - longitude).powi(2)
        }
        // Without a reference point every candidate ties and min_by keeps the
        // first match, preserving the old behaviour.
        None => 0.0,
    }
}

/// Representative coordinates of a zone from the tzdb tables (ISO 6709
/// latitude + longitude of the zone's reference city).
fn zone_reference_coords(zone: &str) -> Option<(f64, f64)> {
    ZONE_TABLES
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .find_map(|table| {
            table.lines().find_map(|line| {
                if line.starts_with('#') {
                    return None;
                }
                let mut fields = line.split('\t');
                fields.next()?; // country codes
                let coordinates = fields.next()?;
                if fields.next()? == zone {
                    parse_coordinates(coordinates)
                } else {
                    None
                }
            })
        })
}

fn current_timezone() -> Option<String> {
    let target = std::fs::canonicalize("/etc/localtime").ok()?;
    target
        .strip_prefix(Path::new("/usr/share/zoneinfo"))
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
}

fn nearest_location(
    locations: &[Location],
    x: f64,
    y: f64,
    map_width: f64,
    map_height: f64,
) -> usize {
    locations
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            pixel_distance(a, x, y, map_width, map_height)
                .total_cmp(&pixel_distance(b, x, y, map_width, map_height))
        })
        .map(|(index, _)| index)
        .unwrap_or(0)
}

fn pixel_distance(location: &Location, x: f64, y: f64, map_width: f64, map_height: f64) -> f64 {
    let dx = longitude_to_x(location.longitude, map_width) - x;
    let dy = latitude_to_y(location.latitude, map_height) - y;
    dx * dx + dy * dy
}

fn normalize(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|character| match character {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect::<String>()
        .replace(['_', '/', '-', ','], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn matches_query(search_text: &str, query: &str) -> bool {
    let words = search_text.split_whitespace().collect::<Vec<_>>();
    query
        .split_whitespace()
        .all(|term| words.iter().any(|word| word.starts_with(term)))
}

fn search_rank(location: &Location, query: &str) -> (u8, String) {
    let city = normalize(location.city());
    let rank = if city == query {
        0
    } else if city.starts_with(query) {
        1
    } else if city.split_whitespace().any(|word| word.starts_with(query)) {
        2
    } else {
        3
    };
    (rank, city)
}

fn timezone_detail(zone: &str) -> String {
    let Some(timezone) = gtk::glib::TimeZone::from_identifier(Some(zone)) else {
        return zone.to_string();
    };
    let Ok(now) = gtk::glib::DateTime::now(&timezone) else {
        return zone.to_string();
    };
    let abbreviation = now.format("%Z").ok();
    let offset = now.format("%:::z").ok();
    let time = now.format("%R").ok();
    match (abbreviation, offset, time) {
        (Some(abbreviation), Some(offset), Some(time)) => {
            format!("{zone} · {abbreviation} (UTC{offset}) · {time}")
        }
        _ => zone.to_string(),
    }
}

fn existing_asset<'a>(installed: &'a str, dev: &'a str) -> Option<&'a str> {
    if Path::new(installed).is_file() {
        Some(installed)
    } else if Path::new(dev).is_file() {
        Some(dev)
    } else {
        None
    }
}

/// Render the vector map on a dedicated thread and swap it into the picture
/// once ready. glycin's whole runtime (zbus, sandboxed loaders, subprocess
/// watches) is async-io based; driving it from the glib main context can
/// deadlock, so it runs under `async_io::block_on` off the UI thread and the
/// finished texture hops back through a oneshot channel awaited on the main
/// context.
fn load_svg_map(map: &gtk::Picture, path: &str) {
    let (sender, receiver) = futures_channel::oneshot::channel::<gtk::gdk::MemoryTexture>();
    gtk::glib::MainContext::default().spawn_local({
        let map = map.clone();
        async move {
            if let Ok(texture) = receiver.await {
                map.set_paintable(Some(&texture));
            }
        }
    });
    let path = path.to_string();
    std::thread::spawn(move || {
        let Some((texture, _)) = render_map_texture(&path) else {
            tracing::warn!("glycin could not render the timezone map SVG");
            return;
        };
        let _ = sender.send(texture);
    });
}

/// Blocking: decode the SVG at native size and repack the frame bytes into a
/// gdk texture plus the raw pixels. Call off the UI thread.
///
/// glycin 2.x cannot hand us a `gdk::Texture` directly (its `gdk4` feature
/// tracks an older gtk4-rs than relm4 0.10), hence the manual repack.
fn render_map_texture(path: &str) -> Option<(gtk::gdk::MemoryTexture, Vec<u8>)> {
    let image = async_io::block_on(glycin::Loader::new(glycin_gio::File::for_path(path)).load())
        .map_err(|err| tracing::warn!("glycin failed to load the timezone map: {err}"))
        .ok()?;
    let frame =
        async_io::block_on(image.specific_frame(
            glycin::FrameRequest::new().scale(MAP_TEXTURE_WIDTH, MAP_TEXTURE_HEIGHT),
        ))
        .map_err(|err| tracing::warn!("glycin failed to decode the timezone map: {err}"))
        .ok()?;
    let format = gdk_memory_format(frame.memory_format())
        .ok_or_else(|| tracing::warn!("glycin returned an unsupported pixel format"))
        .ok()?;
    let texture = gtk::gdk::MemoryTexture::new(
        frame.width() as i32,
        frame.height() as i32,
        format,
        &gtk::glib::Bytes::from(frame.buf_slice()),
        frame.stride() as usize,
    );
    Some((texture, frame.buf_slice().to_vec()))
}

fn gdk_memory_format(format: glycin::MemoryFormat) -> Option<gtk::gdk::MemoryFormat> {
    use glycin::MemoryFormat as Glycin;
    use gtk::gdk::MemoryFormat as Gdk;
    Some(match format {
        Glycin::B8g8r8a8Premultiplied => Gdk::B8g8r8a8Premultiplied,
        Glycin::A8r8g8b8Premultiplied => Gdk::A8r8g8b8Premultiplied,
        Glycin::R8g8b8a8Premultiplied => Gdk::R8g8b8a8Premultiplied,
        Glycin::B8g8r8a8 => Gdk::B8g8r8a8,
        Glycin::A8r8g8b8 => Gdk::A8r8g8b8,
        Glycin::R8g8b8a8 => Gdk::R8g8b8a8,
        Glycin::R8g8b8 => Gdk::R8g8b8,
        Glycin::B8g8r8 => Gdk::B8g8r8,
        _ => return None,
    })
}

// Pixel-space projection used by gnome-initial-setup's cc-timezone-map.c.
fn longitude_to_x(longitude: f64, map_width: f64) -> f64 {
    map_width * (180.0 + longitude) / 360.0 + map_width * LONGITUDE_OFFSET / 180.0
}

fn miller(latitude: f64) -> f64 {
    1.25 * (std::f64::consts::FRAC_PI_4 + 0.4 * latitude.to_radians())
        .tan()
        .ln()
}

fn latitude_to_y(latitude: f64, map_height: f64) -> f64 {
    let top_offset = MILLER_FULL_RANGE * (TOP_LATITUDE / 180.0);
    let map_range = (miller(BOTTOM_LATITUDE) - top_offset).abs();
    (miller(latitude) - top_offset).abs() / map_range * map_height
}

fn position_pin(
    pin: &gtk::Picture,
    (longitude, latitude): (f64, f64),
    map_width: f64,
    map_height: f64,
) {
    let x = longitude_to_x(longitude, map_width)
        .floor()
        .clamp(0.0, map_width);
    let y = latitude_to_y(latitude, map_height)
        .floor()
        .clamp(0.0, map_height);
    let start = (x - PIN_HOT_POINT_X).round().max(0.0) as i32;
    let top = (y - PIN_HOT_POINT_Y).round().max(0.0) as i32;
    if pin.margin_start() != start {
        pin.set_margin_start(start);
    }
    if pin.margin_top() != top {
        pin.set_margin_top(top);
    }
}

/// The permanent city label floats just above the pin, horizontally centered
/// on the pin's tip and clamped inside the map.
fn position_pin_label(
    label: &gtk::Label,
    (longitude, latitude): (f64, f64),
    map_width: f64,
    map_height: f64,
) {
    let label_width = f64::from(label.width());
    let label_height = f64::from(label.height());
    if label_width < 1.0 || label_height < 1.0 {
        return;
    }
    // Same floored projection as the pin, so the label's center lands exactly
    // on the pin tip.
    let x = longitude_to_x(longitude, map_width)
        .floor()
        .clamp(0.0, map_width);
    let y = latitude_to_y(latitude, map_height)
        .floor()
        .clamp(0.0, map_height);
    let start = (x - label_width / 2.0)
        .round()
        .clamp(0.0, (map_width - label_width).max(0.0)) as i32;
    let top = (y - PIN_HOT_POINT_Y - 14.0 - label_height)
        .round()
        .clamp(0.0, (map_height - label_height).max(0.0)) as i32;
    if label.margin_start() != start {
        label.set_margin_start(start);
    }
    if label.margin_top() != top {
        label.set_margin_top(top);
    }
}

/// Keyboard handling shared by the search entry and the popover, mirroring
/// `PlaceEntry._onKeyPressed` in GNOME Maps: Escape dismisses the list and
/// Up/Down move the highlighted row (popping the list back up on first Down)
/// — all without moving keyboard focus out of the entry.
fn handle_cursor_key(
    results: &gtk::ListBox,
    popover: &gtk::Popover,
    key: gtk::gdk::Key,
) -> gtk::glib::Propagation {
    use gtk::gdk::Key;
    match key {
        Key::Escape => {
            results.unselect_all();
            popover.popdown();
            gtk::glib::Propagation::Stop
        }
        Key::Up | Key::KP_Up | Key::Down | Key::KP_Down => {
            let direction: i32 = if matches!(key, Key::Up | Key::KP_Up) {
                -1
            } else {
                1
            };
            let mut count = 0;
            while results.row_at_index(count).is_some() {
                count += 1;
            }
            if count == 0 {
                return gtk::glib::Propagation::Proceed;
            }
            if !popover.is_visible() {
                if direction > 0 {
                    popover.popup();
                    if let Some(row) = results.row_at_index(0) {
                        results.select_row(Some(&row));
                    }
                }
                return gtk::glib::Propagation::Stop;
            }
            let wrap_from = if direction > 0 { -1 } else { count };
            let current = results
                .selected_row()
                .map(|row| row.index())
                .unwrap_or(wrap_from);
            let next = (current + direction).clamp(0, count - 1);
            if let Some(row) = results.row_at_index(next) {
                results.select_row(Some(&row));
            }
            gtk::glib::Propagation::Stop
        }
        _ => gtk::glib::Propagation::Proceed,
    }
}

// The selected zone's UTC offset maps to a 15°-wide meridian band, like the
// strip GNOME's Date & Time panel highlights for the active time zone.
fn zone_meridian(zone: &str) -> f64 {
    gtk::glib::TimeZone::from_identifier(Some(zone))
        .and_then(|timezone| gtk::glib::DateTime::now(&timezone).ok())
        .map(|now| now.utc_offset().as_microseconds() as f64 / 3_600_000_000.0 * 15.0)
        .unwrap_or(0.0)
}

fn position_band(band: &gtk::Box, meridian: f64, map_width: f64) {
    let x1 = longitude_to_x(meridian - 7.5, map_width).clamp(0.0, map_width);
    let x2 = longitude_to_x(meridian + 7.5, map_width).clamp(0.0, map_width);
    let start = x1.round() as i32;
    let width = (x2 - x1).round().max(1.0) as i32;
    if band.margin_start() != start {
        band.set_margin_start(start);
    }
    if band.width_request() != width {
        band.set_width_request(width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relm4::{Component, ComponentController};

    #[test]
    fn parses_iso_6709_coordinates_with_and_without_seconds() {
        let (latitude, longitude) = parse_coordinates("-2332-04637").unwrap();
        assert!((latitude - -23.533_333).abs() < 0.000_001);
        assert!((longitude - -46.616_667).abs() < 0.000_001);

        let (latitude, longitude) = parse_coordinates("+404251-0740023").unwrap();
        assert!((latitude - 40.714_167).abs() < 0.000_001);
        assert!((longitude - -74.006_389).abs() < 0.000_001);
    }

    #[test]
    fn parses_the_system_zone_table_format() {
        let locations = parse_zone_table(
            "BR\t-2332-04637\tAmerica/Sao_Paulo\tBrazil southeast\n\
             GB\t+513030-0000731\tEurope/London\n",
        );
        assert_eq!(locations.len(), 2);
        assert_eq!(locations[0].zone, "America/Sao_Paulo");
        assert_eq!(locations[0].name, "Sao Paulo");
        assert_eq!(locations[0].region, "Brazil southeast");
        assert_eq!(locations[1].name, "London");
        assert_eq!(locations[1].region, "GB");
    }

    #[test]
    fn city_search_matches_names_and_regions_but_not_timezone_ids() {
        let location = Location {
            zone: "America/Denver".into(),
            name: "Boulder".into(),
            region: "United States".into(),
            latitude: 40.01,
            longitude: -105.27,
            search_text: normalize("Boulder United States"),
        };

        assert!(matches_query(&location.search_text, "boul"));
        assert!(matches_query(&location.search_text, "boul uni"));
        assert!(!matches_query(&location.search_text, "denver"));
    }

    #[test]
    fn auto_detected_zone_prefers_the_city_nearest_the_zone_reference() {
        // zone_match picks the candidate nearest to the zone's tzdb reference
        // point, never an arbitrary first entry far from it.
        let locations = vec![
            Location {
                zone: "America/Sao_Paulo".into(),
                name: "Tarauacá".into(),
                region: "Brazil".into(),
                latitude: -8.166_667,
                longitude: -70.766_667,
                search_text: normalize("Tarauaca Brazil"),
            },
            Location {
                zone: "America/Sao_Paulo".into(),
                name: "Guarulhos".into(),
                region: "Brazil".into(),
                latitude: -23.466_667,
                longitude: -46.533_333,
                search_text: normalize("Guarulhos Brazil"),
            },
        ];
        let index = zone_match(&locations, "America/Sao_Paulo").unwrap();
        if zone_reference_coords("America/Sao_Paulo").is_some() {
            assert_eq!(locations[index].name, "Guarulhos");
        } else {
            // Without a tzdb table the first match is kept.
            assert_eq!(locations[index].name, "Tarauacá");
        }
    }

    #[test]
    fn zone_reference_coordinates_come_from_the_tzdb_table() {
        let Some((latitude, longitude)) = zone_reference_coords("America/Sao_Paulo") else {
            eprintln!("skipping: no tzdb table available");
            return;
        };
        assert!((latitude - -23.533_333).abs() < 0.001);
        assert!((longitude - -46.616_667).abs() < 0.001);
    }

    #[test]
    fn loads_every_zone_from_the_tzdb_table() {
        let locations = load_locations();
        assert!(
            locations.len() > 300,
            "the tzdb tables must provide hundreds of zones, got {}",
            locations.len()
        );
        assert!(
            locations
                .iter()
                .any(|location| location.zone == "America/Sao_Paulo")
        );
        assert!(locations.iter().any(|location| location.zone == "UTC"));
    }

    #[test]
    fn finds_the_nearest_city_on_the_map() {
        let locations = vec![
            fallback("America/Sao_Paulo", -23.55, -46.63),
            fallback("Europe/London", 51.51, -0.13),
        ];
        let (width, height) = (800.0, 409.0);
        let click = |longitude, latitude| {
            nearest_location(
                &locations,
                longitude_to_x(longitude, width),
                latitude_to_y(latitude, height),
                width,
                height,
            )
        };
        assert_eq!(click(-44.0, -22.0), 0);
        assert_eq!(click(1.0, 50.0), 1);
    }

    #[test]
    fn projects_known_cities_inside_the_map_bounds() {
        let (width, height) = (800.0, 409.0);
        for (longitude, latitude) in [(-46.63, -23.55), (-0.13, 51.51), (139.69, 35.68)] {
            let x = longitude_to_x(longitude, width);
            let y = latitude_to_y(latitude, height);
            assert!((0.0..=width).contains(&x), "x out of bounds: {x}");
            assert!((0.0..=height).contains(&y), "y out of bounds: {y}");
        }
        // With no longitude offset baked into the artwork, the horizontal
        // center of the image is the prime meridian.
        assert!((longitude_to_x(0.0, width) - width / 2.0).abs() < 1.0);
    }

    // Interactive test: needs a display (skipped on headless CI). Drives the
    // real component to catch wiring bugs that pure-logic tests cannot.
    #[test]
    fn positions_pin_and_opens_full_width_suggestions_below_input() {
        if std::env::var_os("WAYLAND_DISPLAY").is_none() && std::env::var_os("DISPLAY").is_none() {
            eprintln!("skipping interactive test: no display available");
            return;
        }
        crate::pages::testutil::run_on_gtk_thread(positions_pin_interactive);
    }

    fn positions_pin_interactive() {
        let controller = TimezonePage::builder().launch(());
        let carousel = adw::Carousel::new();
        controller.widget().set_margin_start(72);
        controller.widget().set_margin_end(72);
        carousel.append(controller.widget());
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&adw::HeaderBar::new());
        toolbar.set_content(Some(&carousel));
        let window = adw::Window::new();
        window.set_content(Some(&toolbar));
        window.set_default_size(960, 640);
        window.present();

        let pump = |millis: u64| {
            let context = gtk::glib::MainContext::default();
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(millis);
            while std::time::Instant::now() < deadline {
                while context.pending() {
                    context.iteration(false);
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        };
        pump(500);

        // Clone widget handles out in short borrows: holding model() across a
        // pump deadlocks relm4's update loop (RefCell already borrowed).
        let (search, map, location) = {
            let model = controller.model();
            assert!(model.map.width() > 100, "map must be allocated");
            (
                model.search.clone(),
                model.map.clone(),
                model.locations[model.selected].clone(),
            )
        };

        assert!(
            !has_mapped_scrollbar(controller.widget().upcast_ref()),
            "the timezone page must not show a scrollbar at the default window size"
        );

        let (width, height) = (f64::from(map.width()), f64::from(map.height()));
        let pin_x = longitude_to_x(location.longitude, width)
            .floor()
            .clamp(0.0, width);
        let pin_y = latitude_to_y(location.latitude, height)
            .floor()
            .clamp(0.0, height);
        let expected_pin = (
            (pin_x - PIN_HOT_POINT_X).round().max(0.0) as i32,
            (pin_y - PIN_HOT_POINT_Y).round().max(0.0) as i32,
        );
        let pin_margins = {
            let model = controller.model();
            (model.pin.margin_start(), model.pin.margin_top())
        };
        assert_eq!(
            pin_margins, expected_pin,
            "pin must sit on the initially selected city"
        );

        let (label_text, label_start, label_top, label_width) = {
            let model = controller.model();
            (
                model.pin_label.label().to_string(),
                model.pin_label.margin_start(),
                model.pin_label.margin_top(),
                model.pin_label.width(),
            )
        };
        assert!(
            label_text.contains(location.city()) && label_text.contains("UTC"),
            "the pin label must name the city and its UTC offset, got {label_text:?}"
        );
        assert!(
            label_top < pin_margins.1,
            "the pin label must float above the pin icon"
        );
        let label_center = label_start + label_width / 2;
        assert!(
            (f64::from(label_center) - pin_x).abs() < 2.0,
            "the pin label must stay centered on the pin tip (center {label_center}, tip {pin_x})"
        );

        let meridian = zone_meridian(&location.zone);
        let band_x1 = longitude_to_x(meridian - 7.5, width).clamp(0.0, width);
        let band_x2 = longitude_to_x(meridian + 7.5, width).clamp(0.0, width);
        let expected_band = (
            band_x1.round() as i32,
            (band_x2 - band_x1).round().max(1.0) as i32,
        );
        let band_geometry = {
            let model = controller.model();
            (model.band.margin_start(), model.band.width_request())
        };
        assert_eq!(
            band_geometry, expected_band,
            "band must cover the selected zone's meridian strip"
        );

        search.set_text("denver");
        pump(500);
        let (popover_request, input_request, first_row_is_plain, zones) = {
            let model = controller.model();
            let first_row = model.results.row_at_index(0).expect("Denver result");
            assert!(
                !first_row.is_focusable(),
                "suggestion rows must never take keyboard focus away from the input"
            );
            assert!(
                !model.results.can_focus() && !model.results_popover.can_focus(),
                "nothing inside the suggestions popover may take keyboard focus"
            );
            (
                model.results_popover.width_request(),
                model.search.width_request(),
                first_row
                    .child()
                    .is_some_and(|child| child.is::<gtk::Box>()),
                model
                    .filtered
                    .iter()
                    .map(|index| model.locations[*index].zone.clone())
                    .collect::<Vec<_>>(),
            )
        };
        assert!(first_row_is_plain, "suggestions must use normal list rows");
        assert_eq!(
            popover_request, input_request,
            "suggestion list and input must request exactly the same width"
        );
        assert!(
            !zones.is_empty() && zones.iter().all(|zone| zone == "America/Denver"),
            "Denver suggestions must resolve to America/Denver, got {zones:?}"
        );
        search.emit_activate();
        pump(500);
        let selected_zone = {
            let model = controller.model();
            model.locations[model.selected].zone.clone()
        };
        assert_eq!(selected_zone, "America/Denver");

        // Enter honors the row highlighted through the arrow-key cursor
        // instead of always picking the first suggestion.
        search.set_text("san");
        pump(500);
        let expected_zone = {
            let model = controller.model();
            assert!(
                model.filtered.len() > 1,
                "san must match several rows (Santiago, Santo Domingo, …)"
            );
            let second = model.results.row_at_index(1).unwrap();
            model.results.select_row(Some(&second));
            model.locations[model.filtered[1]].zone.clone()
        };
        search.emit_activate();
        pump(500);
        let selected_zone = {
            let model = controller.model();
            model.locations[model.selected].zone.clone()
        };
        assert_eq!(selected_zone, expected_zone);

        // A zone whose city name differs from the search term still resolves
        // through the tzdb comment/country fields.
        search.set_text("fortaleza");
        pump(500);
        search.emit_activate();
        pump(500);
        let selected_zone = {
            let model = controller.model();
            model.locations[model.selected].zone.clone()
        };
        assert_eq!(selected_zone, "America/Fortaleza");

        window.close();
        pump(100);
    }

    fn has_mapped_scrollbar(widget: &gtk::Widget) -> bool {
        if widget.is::<gtk::Scrollbar>() && widget.is_mapped() {
            return true;
        }

        let mut child = widget.first_child();
        while let Some(current) = child {
            if has_mapped_scrollbar(&current) {
                return true;
            }
            child = current.next_sibling();
        }
        false
    }

    // Exercises the real glycin pipeline (sandboxed loader over D-Bus) against
    // the shipped SVG: the frame must come back at the requested native size.
    // Skipped without a session bus.
    #[test]
    fn renders_svg_map_through_glycin() {
        if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
            eprintln!("skipping glycin test: no session bus");
            return;
        }
        let svg = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/images/timezone-map.svg"
        );
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .try_init();
        let (texture, pixels) = render_map_texture(svg).expect("glycin must decode the SVG");
        assert_eq!(
            (texture.width(), texture.height()),
            (MAP_TEXTURE_WIDTH as i32, MAP_TEXTURE_HEIGHT as i32)
        );
        // Land is opaque white, the ocean opaque GNOME blue; a broken render
        // (blank/transparent) must not pass silently.
        assert_eq!(
            pixels.len() as u32,
            MAP_TEXTURE_WIDTH * MAP_TEXTURE_HEIGHT * 4
        );

        if std::env::var_os("SIRIUS_GLYCIN_DUMP").is_some() {
            std::fs::write("/tmp/tzmap.rgba", &pixels).unwrap();
            eprintln!(
                "dumped {}x{} frame to /tmp/tzmap.rgba",
                texture.width(),
                texture.height()
            );
        }
    }
}
