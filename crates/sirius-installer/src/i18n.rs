//! Minimal in-app translation. `Lang` is the UI language; `tr` looks up a key.
//! Falls back to English (and then to the key itself) so missing keys are never blank.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    PtBr,
}

impl Lang {
    /// Map an install locale string (e.g. "pt_BR", "en_US") to a UI language.
    pub fn from_locale(locale: &str) -> Lang {
        if locale.to_ascii_lowercase().starts_with("pt") {
            Lang::PtBr
        } else {
            Lang::En
        }
    }
}

impl Default for Lang {
    fn default() -> Self {
        Lang::En
    }
}

/// Translate a key for the given language. English is the base; pt_BR overrides.
pub fn tr(lang: Lang, key: &str) -> &'static str {
    // English base table — every key MUST have an entry here.
    let en = |k: &str| -> &'static str {
        match k {
            "nav.back" => "Back",
            "nav.next" => "Next",
            "welcome.title" => "Welcome",
            "welcome.desc" => "This assistant will guide you through installation.",
            _ => "",
        }
    };
    // Portuguese overrides — only keys that are translated.
    let pt = |k: &str| -> &'static str {
        match k {
            "nav.back" => "Voltar",
            "nav.next" => "Avançar",
            "welcome.title" => "Bem-vindo",
            "welcome.desc" => "Este assistente vai guiá-lo pela instalação.",
            _ => "",
        }
    };
    match lang {
        Lang::PtBr => {
            let p = pt(key);
            if !p.is_empty() { p } else { let e = en(key); if e.is_empty() { leak_key(key) } else { e } }
        }
        Lang::En => {
            let e = en(key);
            if e.is_empty() { leak_key(key) } else { e }
        }
    }
}

/// Last-resort fallback: show the key itself (leaked to 'static) so nothing is blank.
fn leak_key(key: &str) -> &'static str {
    Box::leak(key.to_string().into_boxed_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_locale_maps_pt() {
        assert_eq!(Lang::from_locale("pt_BR"), Lang::PtBr);
        assert_eq!(Lang::from_locale("en_US"), Lang::En);
    }

    #[test]
    fn tr_translates_and_falls_back() {
        assert_eq!(tr(Lang::PtBr, "nav.next"), "Avançar");
        assert_eq!(tr(Lang::En, "nav.next"), "Next");
        // Unknown key falls back to the key text, never blank.
        assert_eq!(tr(Lang::En, "unknown.key"), "unknown.key");
    }
}
