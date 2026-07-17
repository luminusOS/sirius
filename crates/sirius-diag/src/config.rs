//! Loads the page-toggle and diagnostics configuration from `sirius.toml`.

use serde::Deserialize;
use std::path::Path;

/// Built-in RAM requirement used when `sirius.toml` does not override it.
pub const DEFAULT_MIN_RAM_GIB: u64 = 2;

fn default_min_ram_gib() -> u64 {
    DEFAULT_MIN_RAM_GIB
}

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

/// Diagnostics gating policy.
///
/// Each probe owns its own severity (`Pass`/`Warn`/`Fail`) — config does NOT
/// reclassify a check. `require` selects which *failing* checks (`Status::Fail`)
/// hard-gate the install (see [`crate::report::is_blocked`]); a `Fail` whose id
/// is not in `require` is surfaced but does not block. `warn` is advisory
/// metadata for the UI (which ids to emphasize as warnings) and is not consulted
/// by gating. `min_ram_gib` controls the RAM threshold used by the `ram` probe.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct DiagnosticsConfig {
    #[serde(default)]
    pub require: Vec<String>,
    #[serde(default)]
    pub warn: Vec<String>,
    #[serde(default = "default_min_ram_gib")]
    pub min_ram_gib: u64,
}

impl Default for DiagnosticsConfig {
    fn default() -> Self {
        Self {
            require: Vec::new(),
            warn: Vec::new(),
            min_ram_gib: DEFAULT_MIN_RAM_GIB,
        }
    }
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
    "storage",
    "user",
    "summary",
    "progress",
    "finished",
];

/// Pages that always run and cannot be disabled — you cannot install without them.
pub const MANDATORY_PAGES: &[&str] = &["storage", "progress", "finished"];

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
            .map(|p| match p.as_str() {
                "disk" | "partition" | "manual_partition" => "storage".to_string(),
                _ => p,
            })
            .filter(|p| KNOWN_PAGES.contains(&p.as_str()))
            .filter(|p| {
                MANDATORY_PAGES.contains(&p.as_str()) || !self.disabled.iter().any(|d| d == p)
            })
            .collect();

        let mut seen = std::collections::HashSet::new();
        resolved.retain(|page| seen.insert(page.clone()));

        for m in MANDATORY_PAGES {
            if !resolved.iter().any(|p| p == m) {
                resolved.push(m.to_string());
            }
        }
        resolved
    }
}

/// Default on-disk config path shipped in the ISO.
pub const CONFIG_PATH: &str = "/etc/sirius/sirius.toml";

impl SiriusConfig {
    /// Load config from a path. A missing or malformed file falls back to defaults
    /// rather than aborting (the spec requires the installer to keep working).
    /// Returns the config plus an optional warning string for the caller to log.
    pub fn load_or_default(path: &Path) -> (Self, Option<String>) {
        match std::fs::read_to_string(path) {
            Ok(src) => match Self::from_toml(&src) {
                Ok(cfg) => (cfg, None),
                Err(e) => (
                    Self::default(),
                    Some(format!("malformed config at {}: {e}", path.display())),
                ),
            },
            Err(_) => (
                Self::default(),
                Some(format!(
                    "no config at {}; using built-in defaults",
                    path.display()
                )),
            ),
        }
    }
}

impl Default for SiriusConfig {
    fn default() -> Self {
        Self {
            pages: PagesConfig::default(),
            diagnostics: DiagnosticsConfig {
                require: vec!["uefi".into(), "ram".into(), "disk_space".into()],
                warn: vec!["secure_boot".into(), "network".into(), "virt".into()],
                min_ram_gib: DEFAULT_MIN_RAM_GIB,
            },
        }
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
min_ram_gib = 3
"#;
        let cfg = SiriusConfig::from_toml(src).unwrap();
        assert_eq!(cfg.pages.order.first().unwrap(), "welcome");
        assert_eq!(cfg.pages.disabled, vec!["manual_partition".to_string()]);
        assert_eq!(cfg.diagnostics.require.len(), 3);
        assert_eq!(cfg.diagnostics.min_ram_gib, 3);
    }

    #[test]
    fn empty_config_uses_defaults() {
        let cfg = SiriusConfig::from_toml("").unwrap();
        assert!(cfg.pages.order.is_empty());
        assert!(cfg.diagnostics.require.is_empty());
        assert_eq!(cfg.diagnostics.min_ram_gib, DEFAULT_MIN_RAM_GIB);
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
        assert_eq!(
            resolved,
            vec!["welcome", "storage", "summary", "progress", "finished"]
        );
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
        assert!(resolved.contains(&"storage".to_string()));
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

    #[test]
    fn legacy_partition_pages_collapse_to_storage_once() {
        let pages = PagesConfig {
            order: vec![
                "disk".into(),
                "welcome".into(),
                "partition".into(),
                "manual_partition".into(),
                "storage".into(),
            ],
            disabled: vec![],
        };
        assert_eq!(
            pages.resolve(),
            vec!["storage", "welcome", "progress", "finished"]
        );
    }

    #[test]
    fn load_missing_file_warns_and_defaults() {
        let (cfg, warning) = SiriusConfig::load_or_default(Path::new("/no/such/sirius.toml"));
        assert!(warning.is_some());
        assert_eq!(cfg.diagnostics.require, vec!["uefi", "ram", "disk_space"]);
        assert_eq!(cfg.diagnostics.min_ram_gib, DEFAULT_MIN_RAM_GIB);
    }

    #[test]
    fn load_malformed_file_warns_and_defaults() {
        let dir = std::env::temp_dir();
        let path = dir.join("sirius-test-bad.toml");
        std::fs::write(&path, "this is = = not toml").unwrap();
        let (cfg, warning) = SiriusConfig::load_or_default(&path);
        assert!(warning.unwrap().contains("malformed"));
        assert_eq!(cfg, SiriusConfig::default());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn load_valid_file_no_warning() {
        let dir = std::env::temp_dir();
        let path = dir.join("sirius-test-good.toml");
        std::fs::write(&path, "[pages]\norder = [\"welcome\"]\n").unwrap();
        let (cfg, warning) = SiriusConfig::load_or_default(&path);
        assert!(warning.is_none());
        assert_eq!(cfg.pages.order, vec!["welcome"]);
        std::fs::remove_file(&path).ok();
    }
}
