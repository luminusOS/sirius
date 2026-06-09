//! GTK4 + libadwaita + Relm4 wizard entry point.

use relm4::RelmApp;

/// Launch the installer GUI.
pub fn run() {
    let app = RelmApp::new("dev.luminusos.Sirius");
    app.run::<crate::app::AppModel>(());
}
