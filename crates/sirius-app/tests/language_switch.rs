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
}
