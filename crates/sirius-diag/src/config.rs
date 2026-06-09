//! Loads the page-toggle and diagnostics configuration from `sirius.toml`.

use serde::Deserialize;

/// The full installer configuration file.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SiriusConfig {
    #[serde(default)]
    pub pages: PagesConfig,
    #[serde(default)]
    pub diagnostics: DiagnosticsConfig,
}

/// Which wizard pages are enabled and in what order.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
pub struct PagesConfig {
    #[serde(default)]
    pub order: Vec<String>,
    #[serde(default)]
    pub disabled: Vec<String>,
}

/// Which diagnostics checks hard-gate vs warn.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
pub struct DiagnosticsConfig {
    #[serde(default)]
    pub require: Vec<String>,
    #[serde(default)]
    pub warn: Vec<String>,
}

impl SiriusConfig {
    /// Parse config from a TOML string.
    pub fn from_toml(src: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(src)
    }
}

/// Every page Sirius knows how to render. Used to drop unknown ids from config.
pub const KNOWN_PAGES: &[&str] = &[
    "welcome",
    "diagnostics",
    "network",
    "keyboard",
    "timezone",
    "disk",
    "partition",
    "manual_partition",
    "user",
    "summary",
    "progress",
    "finished",
];

/// Pages that always run and cannot be disabled — you cannot install without them.
pub const MANDATORY_PAGES: &[&str] = &["progress", "finished"];

/// The built-in fallback order, used when config is missing or has no `order`.
pub const DEFAULT_ORDER: &[&str] = KNOWN_PAGES;

impl PagesConfig {
    /// Resolve the final ordered list of pages to show:
    /// 1. Start from `order` (or the default order if empty).
    /// 2. Drop ids not in `KNOWN_PAGES`.
    /// 3. Drop ids listed in `disabled` (unless mandatory).
    /// 4. Ensure every mandatory page is present, appended in canonical order if missing.
    pub fn resolve(&self) -> Vec<String> {
        let source: Vec<String> = if self.order.is_empty() {
            DEFAULT_ORDER.iter().map(|s| s.to_string()).collect()
        } else {
            self.order.clone()
        };

        let mut resolved: Vec<String> = source
            .into_iter()
            .filter(|p| KNOWN_PAGES.contains(&p.as_str()))
            .filter(|p| {
                MANDATORY_PAGES.contains(&p.as_str())
                    || !self.disabled.iter().any(|d| d == p)
            })
            .collect();

        for m in MANDATORY_PAGES {
            if !resolved.iter().any(|p| p == m) {
                resolved.push(m.to_string());
            }
        }
        resolved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_config() {
        let src = r#"
[pages]
order = ["welcome", "diagnostics", "summary", "progress", "finished"]
disabled = ["manual_partition"]

[diagnostics]
require = ["uefi", "ram", "disk_space"]
warn = ["secure_boot", "network", "virt"]
"#;
        let cfg = SiriusConfig::from_toml(src).unwrap();
        assert_eq!(cfg.pages.order.first().unwrap(), "welcome");
        assert_eq!(cfg.pages.disabled, vec!["manual_partition".to_string()]);
        assert_eq!(cfg.diagnostics.require.len(), 3);
    }

    #[test]
    fn empty_config_uses_defaults() {
        let cfg = SiriusConfig::from_toml("").unwrap();
        assert!(cfg.pages.order.is_empty());
        assert!(cfg.diagnostics.require.is_empty());
    }

    #[test]
    fn resolve_drops_disabled_and_unknown() {
        let pages = PagesConfig {
            order: vec![
                "welcome".into(),
                "manual_partition".into(),
                "bogus_page".into(),
                "summary".into(),
                "progress".into(),
                "finished".into(),
            ],
            disabled: vec!["manual_partition".into()],
        };
        let resolved = pages.resolve();
        assert_eq!(resolved, vec!["welcome", "summary", "progress", "finished"]);
    }

    #[test]
    fn resolve_appends_missing_mandatory_pages() {
        let pages = PagesConfig {
            order: vec!["welcome".into()],
            disabled: vec![],
        };
        let resolved = pages.resolve();
        assert!(resolved.contains(&"progress".to_string()));
        assert!(resolved.contains(&"finished".to_string()));
    }

    #[test]
    fn resolve_cannot_disable_mandatory() {
        let pages = PagesConfig {
            order: vec!["progress".into(), "finished".into()],
            disabled: vec!["progress".into()],
        };
        assert!(pages.resolve().contains(&"progress".to_string()));
    }

    #[test]
    fn resolve_empty_order_uses_default() {
        let pages = PagesConfig::default();
        assert_eq!(pages.resolve(), DEFAULT_ORDER);
    }
}
