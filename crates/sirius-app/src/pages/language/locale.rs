//! Locale enumeration and display names via GNOME Desktop.
//!
//! This is the data source behind GNOME Initial Setup's `CcLanguageChooser`:
//! every locale known to the system, named natively, in the current UI
//! language, and in English. Keeping the FFI here lets the language page
//! consume those names without leaking C ownership details into UI code.
//! `has_font` ports `cc_common_language_has_font` (fontconfig) so rows never
//! show up as empty boxes.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::ptr;

use relm4::gtk;

/// Languages shown before the "More…" row, mirroring
/// `cc_common_language_get_initial_languages` with `pt_BR` added (Sirius is a
/// Brazilian product, so Portuguese must be visible without expanding).
const INITIAL_LOCALES: &[&str] = &[
    "en_US", "pt_BR", "de_DE", "fr_FR", "es_ES", "zh_CN", "ja_JP", "ru_RU", "ar_EG",
];

/// The pinned locale ids, shared with the welcome page's greeting carousel.
pub(crate) fn initial_locale_ids() -> &'static [&'static str] {
    INITIAL_LOCALES
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LocaleEntry {
    /// Locale id without codeset, e.g. `pt_BR` — the `SetLocale` wire format.
    pub id: String,
    /// Name in the locale's own language, e.g. `Português (Brasil)`.
    pub name_native: String,
    /// Native name of the country, shown dimmed on the trailing edge.
    pub country_native: Option<String>,
    /// Initial (pinned) entry, visible before the "More…" row is expanded.
    pub is_initial: bool,
    search_text: String,
}

impl LocaleEntry {
    pub(super) fn matches(&self, query: &str) -> bool {
        query
            .split_whitespace()
            .all(|term| self.search_text.contains(term))
    }
}

#[repr(C)]
struct FcPattern {
    _private: [u8; 0],
}

#[repr(C)]
struct FcObjectSet {
    _private: [u8; 0],
}

#[repr(C)]
struct FcCharSet {
    _private: [u8; 0],
}

#[repr(C)]
struct FcFontSet {
    nfont: c_int,
    sfont: c_int,
    fonts: *mut *mut FcPattern,
}

#[link(name = "gnome-desktop-4")]
unsafe extern "C" {
    fn gnome_get_all_locales() -> *mut *mut c_char;
    fn gnome_normalize_locale(locale: *const c_char) -> *mut c_char;
    fn gnome_parse_locale(
        locale: *const c_char,
        language: *mut *mut c_char,
        country: *mut *mut c_char,
        codeset: *mut *mut c_char,
        modifier: *mut *mut c_char,
    ) -> gtk::glib::ffi::gboolean;
    fn gnome_get_language_from_locale(
        locale: *const c_char,
        translation: *const c_char,
    ) -> *mut c_char;
    fn gnome_get_language_from_code(code: *const c_char, translation: *const c_char)
    -> *mut c_char;
    fn gnome_get_country_from_code(code: *const c_char, translation: *const c_char) -> *mut c_char;
}

#[link(name = "fontconfig")]
unsafe extern "C" {
    fn FcInit() -> c_int;
    fn FcLangGetCharSet(language: *const u8) -> *const FcCharSet;
    fn FcPatternCreate() -> *mut FcPattern;
    fn FcPatternAddString(
        pattern: *mut FcPattern,
        object: *const c_char,
        value: *const u8,
    ) -> c_int;
    fn FcPatternDestroy(pattern: *mut FcPattern);
    fn FcObjectSetCreate() -> *mut FcObjectSet;
    fn FcObjectSetDestroy(object_set: *mut FcObjectSet);
    fn FcFontList(
        config: *mut c_void,
        pattern: *mut FcPattern,
        object_set: *mut FcObjectSet,
    ) -> *mut FcFontSet;
    fn FcFontSetDestroy(font_set: *mut FcFontSet);
}

/// Enumerate every displayable locale, initial entries first (mirroring
/// `sort_languages`: pinned languages, then the extras, each group ordered by
/// its native name).
pub(super) fn load() -> Vec<LocaleEntry> {
    // GNOME Desktop lazily initializes its iso-codes tables without locking,
    // so first use must be serialized across threads.
    let _guard = crate::pages::GNOME_DESKTOP_LOCK.lock().unwrap();
    let mut entries = Vec::new();
    for full_id in all_locales() {
        let Some(entry) = locale_entry(&full_id) else {
            continue;
        };
        entries.push(entry);
    }
    entries.sort_by_cached_key(|entry| {
        (
            !entry.is_initial,
            entry.name_native.to_lowercase(),
            entry.id.clone(),
        )
    });
    entries.dedup_by(|a, b| a.id == b.id);
    entries
}

fn locale_entry(full_id: &str) -> Option<LocaleEntry> {
    let (language, country) = parse_locale(full_id)?;
    if !has_font(&language) {
        return None;
    }

    let name_native = language_from_locale(full_id, Some(full_id))
        .or_else(|| language_from_code(&language, None))?;
    let name_current = language_from_locale(full_id, None).unwrap_or_default();
    let name_english = language_from_locale(full_id, Some("C")).unwrap_or_default();
    let country_native = country.and_then(|code| {
        country_from_code(&code, Some(full_id)).or_else(|| country_from_code(&code, None))
    });
    let id = strip_codeset(full_id);
    let search_text = normalize(&format!(
        "{name_native} {name_current} {name_english} {} {id}",
        country_native.as_deref().unwrap_or_default()
    ));

    Some(LocaleEntry {
        is_initial: INITIAL_LOCALES.contains(&id.as_str()),
        id,
        name_native,
        country_native,
        search_text,
    })
}

/// The current UI locale in `SetLocale` wire format (no codeset), taken from
/// `LANGUAGE`'s first entry or `LANG`, e.g. `pt_BR`.
pub(super) fn current_locale() -> Option<String> {
    let value = std::env::var("LANGUAGE")
        .ok()
        .and_then(|value| value.split(':').next().map(str::to_owned))
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var("LANG").ok())
        .filter(|value| !value.is_empty() && value != "C" && value != "POSIX")?;
    Some(strip_codeset(&normalize_locale(&value).unwrap_or(value)))
}

fn strip_codeset(locale: &str) -> String {
    locale.split('.').next().unwrap_or(locale).to_string()
}

fn all_locales() -> Vec<String> {
    // SAFETY: `gnome_get_all_locales` returns a newly allocated GStrv which we
    // copy out of and release with `g_strfreev`.
    unsafe {
        let strv = gnome_get_all_locales();
        if strv.is_null() {
            return Vec::new();
        }
        let mut locales = Vec::new();
        let mut cursor = strv;
        while !(*cursor).is_null() {
            // SAFETY: non-null GStrv element, NUL-terminated. Elements are
            // owned by the strv and released by `g_strfreev` below.
            locales.push(copy(*cursor));
            cursor = cursor.add(1);
        }
        gtk::glib::ffi::g_strfreev(strv.cast());
        locales
    }
}

fn normalize_locale(locale: &str) -> Option<String> {
    let c_locale = CString::new(locale).ok()?;
    // SAFETY: valid input; result is a newly allocated string freed with g_free.
    unsafe {
        let result = gnome_normalize_locale(c_locale.as_ptr());
        (!result.is_null()).then(|| copy_free(result))
    }
}

fn parse_locale(locale: &str) -> Option<(String, Option<String>)> {
    let c_locale = CString::new(locale).ok()?;
    let mut language = ptr::null_mut();
    let mut country = ptr::null_mut();
    // SAFETY: output pointers are initialized by GNOME Desktop on success and
    // are newly allocated strings released with g_free.
    let parsed = unsafe {
        gnome_parse_locale(
            c_locale.as_ptr(),
            &mut language,
            &mut country,
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    if parsed == gtk::glib::ffi::GFALSE || language.is_null() {
        return None;
    }
    // SAFETY: checked for null above.
    let language = unsafe { copy_free(language) };
    // SAFETY: optional pointer copied only when non-null.
    let country = (!country.is_null()).then(|| unsafe { copy_free(country) });
    Some((language, country))
}

fn language_from_locale(locale: &str, translation: Option<&str>) -> Option<String> {
    let c_locale = CString::new(locale).ok()?;
    let c_translation = translation.and_then(|value| CString::new(value).ok());
    // SAFETY: valid inputs; result is a newly allocated string freed with g_free.
    unsafe {
        let result = gnome_get_language_from_locale(
            c_locale.as_ptr(),
            c_translation
                .as_ref()
                .map_or(ptr::null(), |value| value.as_ptr()),
        );
        (!result.is_null()).then(|| copy_free(result))
    }
}

fn language_from_code(code: &str, translation: Option<&str>) -> Option<String> {
    let c_code = CString::new(code).ok()?;
    let c_translation = translation.and_then(|value| CString::new(value).ok());
    // SAFETY: valid inputs; result is a newly allocated string freed with g_free.
    unsafe {
        let result = gnome_get_language_from_code(
            c_code.as_ptr(),
            c_translation
                .as_ref()
                .map_or(ptr::null(), |value| value.as_ptr()),
        );
        (!result.is_null()).then(|| copy_free(result))
    }
}

fn country_from_code(code: &str, translation: Option<&str>) -> Option<String> {
    let c_code = CString::new(code).ok()?;
    let c_translation = translation.and_then(|value| CString::new(value).ok());
    // SAFETY: valid inputs; result is a newly allocated string freed with g_free.
    unsafe {
        let result = gnome_get_country_from_code(
            c_code.as_ptr(),
            c_translation
                .as_ref()
                .map_or(ptr::null(), |value| value.as_ptr()),
        );
        (!result.is_null()).then(|| copy_free(result))
    }
}

/// Port of `cc_common_language_has_font`: when fontconfig does not know the
/// language we assume it renders; otherwise some installed font must cover
/// its charset.
pub(crate) fn has_font(language: &str) -> bool {
    let Ok(c_language) = CString::new(language) else {
        return false;
    };
    // SAFETY: every created object is destroyed before returning; the charset
    // pointer is borrowed from fontconfig and never freed. FcInit is
    // idempotent and thread-safe, and required before first use when no GUI
    // toolkit initialized fontconfig already (e.g. in tests).
    unsafe {
        FcInit();
        let charset = FcLangGetCharSet(c_language.as_ptr().cast());
        if charset.is_null() {
            return true;
        }
        let pattern = FcPatternCreate();
        if pattern.is_null() {
            return false;
        }
        let added = FcPatternAddString(pattern, c"lang".as_ptr(), c_language.as_ptr().cast());
        let object_set = FcObjectSetCreate();
        let font_set = if added != 0 && !object_set.is_null() {
            FcFontList(ptr::null_mut(), pattern, object_set)
        } else {
            ptr::null_mut()
        };
        let displayable = !font_set.is_null() && (*font_set).nfont > 0;
        if !font_set.is_null() {
            FcFontSetDestroy(font_set);
        }
        if !object_set.is_null() {
            FcObjectSetDestroy(object_set);
        }
        FcPatternDestroy(pattern);
        displayable
    }
}

/// Copy a newly allocated C string and release it with `g_free`.
///
/// # SAFETY: `pointer` must be a valid NUL-terminated `g_malloc` string.
unsafe fn copy_free(pointer: *mut c_char) -> String {
    // SAFETY: caller guarantees a valid NUL-terminated pointer.
    let value = unsafe { copy(pointer) };
    // SAFETY: the pointer came from g_malloc.
    unsafe { gtk::glib::ffi::g_free(pointer.cast()) };
    value
}

/// Copy a borrowed C string without freeing it.
///
/// # SAFETY: `pointer` must be a valid NUL-terminated string.
unsafe fn copy(pointer: *const c_char) -> String {
    // SAFETY: caller guarantees a valid NUL-terminated pointer.
    unsafe { CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned()
}

pub(super) fn normalize(value: &str) -> String {
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
        .replace(['_', '.', '-', '(', ')', ','], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerates_the_system_locales() {
        let entries = load();
        assert!(entries.iter().any(|entry| entry.id == "en_US"));
        assert!(entries.iter().any(|entry| entry.id == "pt_BR"));
        let portuguese = entries.iter().find(|entry| entry.id == "pt_BR").unwrap();
        // The native name comes from the system's iso-codes translations;
        // minimal images only ship the English ones ("Portuguese (Brazil)").
        assert!(normalize(&portuguese.name_native).contains("portugu"));
        assert!(portuguese.is_initial);
        assert!(matches!(
            portuguese
                .country_native
                .as_deref()
                .map(normalize)
                .as_deref(),
            Some("brasil") | Some("brazil")
        ));
    }

    #[test]
    fn initial_entries_come_first() {
        let entries = load();
        let first_extra = entries.iter().position(|entry| !entry.is_initial);
        if let Some(first_extra) = first_extra {
            assert!(entries[..first_extra].iter().all(|entry| entry.is_initial));
        }
        assert!(entries.iter().filter(|entry| entry.is_initial).count() <= 10);
    }

    #[test]
    fn search_matches_native_current_and_english_names() {
        let entries = load();
        // ja_JP only shows up when the system has a CJK font (has_font
        // filters locales the system cannot render, like cc-language-chooser).
        if let Some(japanese) = entries.iter().find(|entry| entry.id == "ja_JP") {
            assert!(japanese.matches("jap"));
            assert!(japanese.matches("japanese"));
        }
        // Entries hidden behind the "More…" row must exist and stay searchable.
        let extra = entries
            .iter()
            .find(|entry| !entry.is_initial)
            .expect("the chooser must have entries behind the More… row");
        let term = extra.name_native.split_whitespace().next().unwrap();
        assert!(extra.matches(&normalize(term)));
    }

    #[test]
    fn latin_american_locales_keep_their_country() {
        let entries = load();
        if let Some(mexico) = entries.iter().find(|entry| entry.id == "es_MX") {
            // "México" with full langpacks, "Mexico" on minimal images.
            assert_eq!(
                mexico.country_native.as_deref().map(normalize).as_deref(),
                Some("mexico")
            );
        }
    }

    #[test]
    fn font_coverage_check_accepts_latin() {
        assert!(has_font("en"));
        assert!(has_font("pt"));
    }
}
