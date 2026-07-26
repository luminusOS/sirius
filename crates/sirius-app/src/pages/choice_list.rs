//! Shared selected-state marker for open, single-choice lists.

use relm4::adw::prelude::*;
use relm4::gtk;

/// A quiet selected-state marker for rows which are themselves clickable.
///
/// Opacity keeps every row's trailing space identical while avoiding the
/// form-control appearance of a radio button.
pub(crate) fn selected_indicator(selected: bool) -> gtk::Image {
    let indicator = gtk::Image::from_icon_name("object-select-symbolic");
    indicator.set_valign(gtk::Align::Center);
    indicator.set_opacity(if selected { 1.0 } else { 0.0 });
    indicator.set_can_target(false);
    indicator
}
