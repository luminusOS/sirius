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
}
