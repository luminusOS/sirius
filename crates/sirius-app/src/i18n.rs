//! Runtime language selection for the graphical application.

use std::ffi::c_int;

unsafe extern "C" {
    #[link_name = "_nl_msg_cat_cntr"]
    static mut GETTEXT_CATALOG_GENERATION: c_int;
}

/// Select the language used by subsequent GNU gettext lookups.
///
/// GNU gettext caches the last loaded catalog. Updating `LANGUAGE` alone can
/// therefore leave the previous translation active; advancing its catalog
/// generation makes the new preference visible immediately.
pub fn set_ui_language(locale: &str) {
    // SAFETY: Sirius changes the process-wide gettext language only from the
    // single-threaded GTK main loop, before installation workers are started.
    unsafe {
        std::env::set_var("LANGUAGE", locale);
        GETTEXT_CATALOG_GENERATION = GETTEXT_CATALOG_GENERATION.wrapping_add(1);
    }
}
