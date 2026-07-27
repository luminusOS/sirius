//! Runtime language selection for the graphical application.

use std::ffi::c_int;

unsafe extern "C" {
    #[link_name = "_nl_msg_cat_cntr"]
    static mut GETTEXT_CATALOG_GENERATION: c_int;
}

/// Select the language used by subsequent GNU gettext lookups.
///
/// Two things are needed for a switch to take effect everywhere:
///
/// - `LANGUAGE` plus a catalog-generation bump: GNU gettext caches the last
///   loaded catalog, so advancing the generation makes the new preference
///   visible immediately.
/// - `setlocale(LC_ALL, <locale>)`: when the installer starts with an empty
///   environment (the live session's systemd service), the process sits in
///   the C locale and glibc gettext ignores `LANGUAGE` entirely, always
///   returning the English msgids. Pointing `LC_ALL` at the chosen locale
///   makes the switch work from any starting point — it requires the locale
///   to be installed on the system (e.g. `glibc-all-langpacks`).
pub fn set_ui_language(locale: &str) {
    // SAFETY: Sirius changes the process-wide gettext language only from the
    // single-threaded GTK main loop, before installation workers are started.
    unsafe {
        std::env::set_var("LANGUAGE", locale);
        GETTEXT_CATALOG_GENERATION = GETTEXT_CATALOG_GENERATION.wrapping_add(1);
    }
    gettextrs::setlocale(gettextrs::LocaleCategory::LcAll, with_utf8_codeset(locale));
}

/// `pt_BR` → `pt_BR.UTF-8`, keeping an existing codeset or `@modifier` in
/// place (`sr_RS@latin` → `sr_RS.UTF-8@latin`).
fn with_utf8_codeset(locale: &str) -> String {
    let (before_modifier, modifier) = locale.split_once('@').unwrap_or((locale, ""));
    let base = before_modifier.split('.').next().unwrap_or(before_modifier);
    if modifier.is_empty() {
        format!("{base}.UTF-8")
    } else {
        format!("{base}.UTF-8@{modifier}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_a_utf8_codeset_without_losing_the_modifier() {
        assert_eq!(with_utf8_codeset("pt_BR"), "pt_BR.UTF-8");
        assert_eq!(with_utf8_codeset("pt_BR.UTF-8"), "pt_BR.UTF-8");
        assert_eq!(with_utf8_codeset("sr_RS@latin"), "sr_RS.UTF-8@latin");
        assert_eq!(
            with_utf8_codeset("ca_ES.UTF-8@valencia"),
            "ca_ES.UTF-8@valencia"
        );
    }
}
