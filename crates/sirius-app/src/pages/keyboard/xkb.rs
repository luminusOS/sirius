//! Safe ownership boundary around GNOME Desktop's `GnomeXkbInfo`.
//!
//! GNOME Initial Setup uses this API for the keyboard chooser. Keeping the FFI
//! here lets the page consume the same localized layout names and canonical
//! `layout[+variant]` identifiers without leaking C ownership details into UI
//! code.

use std::ffi::{CStr, CString, c_char};
use std::ptr;

use relm4::gtk;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Layout {
    pub id: String,
    pub name: String,
    pub short_name: String,
    pub xkb_layout: String,
    pub xkb_variant: Option<String>,
    pub search_text: String,
}

#[repr(C)]
struct GnomeXkbInfo {
    _private: [u8; 0],
}

#[link(name = "gnome-desktop-4")]
unsafe extern "C" {
    fn gnome_xkb_info_new() -> *mut GnomeXkbInfo;
    fn gnome_xkb_info_get_all_layouts(info: *mut GnomeXkbInfo) -> *mut gtk::glib::ffi::GList;
    fn gnome_xkb_info_get_layout_info(
        info: *mut GnomeXkbInfo,
        id: *const c_char,
        display_name: *mut *const c_char,
        short_name: *mut *const c_char,
        xkb_layout: *mut *const c_char,
        xkb_variant: *mut *const c_char,
    ) -> gtk::glib::ffi::gboolean;
    fn gnome_xkb_info_get_layouts_for_language(
        info: *mut GnomeXkbInfo,
        language_code: *const c_char,
    ) -> *mut gtk::glib::ffi::GList;
    fn gnome_xkb_info_get_layouts_for_country(
        info: *mut GnomeXkbInfo,
        country_code: *const c_char,
    ) -> *mut gtk::glib::ffi::GList;
    fn gnome_get_input_source_from_locale(
        locale: *const c_char,
        source_type: *mut *const c_char,
        id: *mut *const c_char,
    ) -> gtk::glib::ffi::gboolean;
}

/// Enumerate every XKB layout and variant known to GNOME Desktop.
pub(super) fn load() -> Vec<Layout> {
    // SAFETY: `GnomeXkbInfo` owns the strings referenced by the returned list.
    // We copy every string before freeing the list and unref the object last.
    unsafe {
        let info = gnome_xkb_info_new();
        if info.is_null() {
            return fallback();
        }

        let list = gnome_xkb_info_get_all_layouts(info);
        let mut node = list;
        let mut layouts = Vec::new();
        while !node.is_null() {
            let id_pointer = (*node).data.cast::<c_char>();
            if !id_pointer.is_null()
                && let Some(layout) = layout_info(info, id_pointer)
            {
                layouts.push(layout);
            }
            node = (*node).next;
        }

        gtk::glib::ffi::g_list_free(list);
        gtk::glib::gobject_ffi::g_object_unref(info.cast::<gtk::glib::gobject_ffi::GObject>());

        layouts.sort_by_cached_key(|layout| layout.name.to_lowercase());
        layouts.dedup_by(|a, b| a.id == b.id);
        if layouts.is_empty() {
            fallback()
        } else {
            layouts
        }
    }
}

unsafe fn layout_info(info: *mut GnomeXkbInfo, id_pointer: *const c_char) -> Option<Layout> {
    let mut display_name = ptr::null();
    let mut short_name = ptr::null();
    let mut xkb_layout = ptr::null();
    let mut xkb_variant = ptr::null();

    // SAFETY: all pointers are valid for the lifetime of `info`; output
    // pointers are initialized by GNOME Desktop on success.
    let found = unsafe {
        gnome_xkb_info_get_layout_info(
            info,
            id_pointer,
            &mut display_name,
            &mut short_name,
            &mut xkb_layout,
            &mut xkb_variant,
        )
    };
    if found == gtk::glib::ffi::GFALSE || display_name.is_null() || xkb_layout.is_null() {
        return None;
    }

    // SAFETY: GNOME Desktop returns NUL-terminated strings owned by `info`.
    let id = unsafe { copy(id_pointer) };
    // SAFETY: checked for null above.
    let name = unsafe { copy(display_name) };
    // SAFETY: optional pointers are copied only when non-null.
    let short_name = unsafe { copy_optional(short_name) }.unwrap_or_default();
    // SAFETY: checked for null above.
    let xkb_layout = unsafe { copy(xkb_layout) };
    // SAFETY: optional pointers are copied only when non-null.
    let xkb_variant = unsafe { copy_optional(xkb_variant) }.filter(|value| !value.is_empty());
    let search_text = normalize(&format!(
        "{name} {short_name} {id} {xkb_layout} {}",
        xkb_variant.as_deref().unwrap_or_default()
    ));

    Some(Layout {
        id,
        name,
        short_name,
        xkb_layout,
        xkb_variant,
        search_text,
    })
}

unsafe fn copy(pointer: *const c_char) -> String {
    // SAFETY: caller guarantees a valid NUL-terminated pointer.
    unsafe { CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned()
}

unsafe fn copy_optional(pointer: *const c_char) -> Option<String> {
    (!pointer.is_null()).then(|| {
        // SAFETY: non-null pointers returned by GNOME Desktop are
        // NUL-terminated.
        unsafe { copy(pointer) }
    })
}

/// Initial layout ids for a locale, mirroring `CcInputChooser`'s
/// `get_locale_infos`: the locale's default input source first, then the
/// layouts GNOME Desktop associates with its language and its country. These
/// are the rows visible before the chooser's "More…" row is expanded.
pub(super) fn initial_layout_ids(locale: &str) -> Vec<String> {
    // GNOME Desktop lazily initializes its iso-codes tables without locking,
    // so first use must be serialized across threads.
    let _guard = crate::pages::GNOME_DESKTOP_LOCK.lock().unwrap();
    // SAFETY: `GnomeXkbInfo` owns the strings referenced by the returned
    // lists. We copy every id before freeing the lists and unref the object
    // last; the input-source out pointers are borrowed and never freed.
    unsafe {
        let info = gnome_xkb_info_new();
        if info.is_null() {
            return Vec::new();
        }

        let mut ids = Vec::new();
        if let Ok(c_locale) = CString::new(with_codeset(locale)) {
            let mut source_type = ptr::null();
            let mut source_id = ptr::null();
            if gnome_get_input_source_from_locale(
                c_locale.as_ptr(),
                &mut source_type,
                &mut source_id,
            ) != gtk::glib::ffi::GFALSE
                && !source_type.is_null()
                && !source_id.is_null()
                && copy(source_type) == "xkb"
            {
                ids.push(copy(source_id));
            }
        }

        let (language, country) = split_locale(locale);
        if let Some(language) = language
            && let Ok(c_language) = CString::new(language)
        {
            collect_layout_ids(
                gnome_xkb_info_get_layouts_for_language(info, c_language.as_ptr()),
                &mut ids,
            );
        }
        if let Some(country) = country
            && let Ok(c_country) = CString::new(country)
        {
            collect_layout_ids(
                gnome_xkb_info_get_layouts_for_country(info, c_country.as_ptr()),
                &mut ids,
            );
        }

        gtk::glib::gobject_ffi::g_object_unref(info.cast::<gtk::glib::gobject_ffi::GObject>());
        ids
    }
}

/// Copy every layout id out of a GList owned by `GnomeXkbInfo`, deduplicating
/// while preserving order, and free the list.
///
/// # SAFETY: `list` must be a GList of borrowed `const gchar*` layout ids.
unsafe fn collect_layout_ids(list: *mut gtk::glib::ffi::GList, ids: &mut Vec<String>) {
    // SAFETY: caller guarantees the list element type; elements are borrowed.
    unsafe {
        let mut node = list;
        while !node.is_null() {
            let id_pointer = (*node).data.cast::<c_char>();
            if !id_pointer.is_null() {
                let id = copy(id_pointer);
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            node = (*node).next;
        }
        gtk::glib::ffi::g_list_free(list);
    }
}

/// `pt_BR.UTF-8` → (`pt`, `BR`); modifiers and codesets are dropped.
fn split_locale(locale: &str) -> (Option<String>, Option<String>) {
    let base = locale.split(['.', '@']).next().unwrap_or(locale);
    let mut parts = base.split('_');
    let language = parts.next().filter(|value| !value.is_empty());
    let country = parts.next().filter(|value| !value.is_empty());
    (
        language.map(str::to_owned),
        country.map(|value| value.to_uppercase()),
    )
}

/// `gnome_get_input_source_from_locale` expects full locale names.
fn with_codeset(locale: &str) -> String {
    if locale.contains('.') {
        locale.to_string()
    } else {
        format!("{locale}.UTF-8")
    }
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
        .replace(['_', '+', '-', '(', ')', ','], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn fallback() -> Vec<Layout> {
    [
        ("us", "English (US)", "en"),
        ("gb", "English (UK)", "en"),
        ("br", "Português (Brasil)", "pt"),
    ]
    .into_iter()
    .map(|(id, name, short_name)| Layout {
        id: id.into(),
        name: name.into(),
        short_name: short_name.into(),
        xkb_layout: id.into(),
        xkb_variant: None,
        search_text: normalize(&format!("{id} {name} {short_name}")),
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gnome_xkb_info_exposes_layouts_and_variants() {
        let layouts = load();
        assert!(layouts.iter().any(|layout| layout.id == "us"));
        assert!(layouts.iter().any(|layout| layout.id == "br"));
        assert!(layouts.iter().any(|layout| layout.xkb_variant.is_some()));
    }

    #[test]
    fn search_is_case_and_accent_insensitive() {
        assert_eq!(normalize("Português (Brasil)"), "portugues brasil");
    }
}
