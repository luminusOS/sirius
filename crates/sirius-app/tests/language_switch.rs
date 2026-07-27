use gettextrs::{
    LocaleCategory, bind_textdomain_codeset, bindtextdomain, gettext, setlocale, textdomain,
};
use sirius_app::set_ui_language;
use std::path::PathBuf;

#[test]
fn gettext_switches_from_portuguese_back_to_english() {
    setlocale(LocaleCategory::LcAll, "");
    setlocale(LocaleCategory::LcMessages, "en_US.UTF-8");
    let locale_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/locale");
    bind_textdomain_codeset("sirius", "UTF-8").unwrap();
    bindtextdomain("sirius", locale_dir).unwrap();
    textdomain("sirius").unwrap();

    set_ui_language("en_US");
    assert_eq!(gettext("Language"), "Language");

    set_ui_language("pt_BR");
    assert_eq!(gettext("Language"), "Idioma");

    set_ui_language("en_US");
    assert_eq!(gettext("Language"), "Language");

    // The ISO live session starts the installer with an empty environment,
    // i.e. in the C locale, where glibc gettext ignores LANGUAGE and always
    // returns msgids. Switching must work from there too (needs the
    // pt_BR.UTF-8 locale installed, e.g. glibc-all-langpacks).
    setlocale(LocaleCategory::LcAll, "C");
    assert_eq!(gettext("Language"), "Language");
    set_ui_language("pt_BR");
    assert_eq!(gettext("Language"), "Idioma");
}
