//! Sirius graphical application.
//!
//! This crate owns only presentation and wizard interaction. Hardware and
//! installation side effects are delegated to `sirius-diag` and
//! `sirius-backend`.

mod app;
mod i18n;
mod navigator;
mod pages;
mod style;

use relm4::RelmApp;

pub use i18n::set_ui_language;

/// Launch the GTK4/Libadwaita installer wizard.
pub fn run() {
    let app = RelmApp::new("io.sirius.Installer");
    app.run::<app::AppModel>(());
}
