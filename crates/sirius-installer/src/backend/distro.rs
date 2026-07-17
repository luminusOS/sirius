//! Static description of what Sirius installs: the bootc/OCI image to deploy and
//! the systemd-repart config directory describing the partition layout. Sirius is
//! distro-agnostic — these values come from the distribution's descriptor, shipped
//! at `/etc/sirius/distro.toml`, organized into `[bootc]` and `[disk]` sections.

use serde::{Deserialize, Serialize};

/// Default on-disk path for the distro descriptor, shipped in the ISO.
pub const DISTRO_PATH: &str = "/etc/sirius/distro.toml";

/// Top-level descriptor: one section per concern.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct DistroDescriptor {
    /// `[bootc]` — the OCI image deployment settings.
    pub bootc: BootcConfig,
    /// `[disk]` — the target partition layout.
    pub disk: DiskConfig,
    /// `[[bento]]` — optional link cards shown on the install progress page
    /// (website, help, contribute, ...). The UI renders at most three.
    #[serde(default, rename = "bento")]
    pub bentos: Vec<Bento>,
    /// `[branding]` — optional welcome-page branding.
    #[serde(default)]
    pub branding: Branding,
}

/// `[branding]` section: what the welcome page shows above the title.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Branding {
    /// Absolute path to a logo image file (preferred when set and readable).
    #[serde(default)]
    pub logo: Option<String>,
    /// Themed icon name fallback (default: a star, for Sirius).
    #[serde(default)]
    pub icon: Option<String>,
}

/// One `[[bento]]` link card on the progress page.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Bento {
    /// Card heading, in the distribution's language of choice.
    pub title: String,
    /// One-line description under the heading.
    pub desc: String,
    /// URL opened when the card is clicked.
    pub link: String,
    /// Themed icon name (e.g. "explore-symbolic").
    pub icon: String,
}

/// `[bootc]` section.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct BootcConfig {
    /// The bootc/OCI image reference to deploy (e.g. "ghcr.io/example/os:latest").
    pub image: String,
    /// Optional image reference persisted into the installed bootc deployment.
    #[serde(default)]
    pub target_imgref: Option<String>,
    /// Whether bootc should enforce container signature policy during install.
    #[serde(default)]
    pub enforce_sigpolicy: bool,
    /// Kernel arguments passed through `bootc install to-filesystem`.
    #[serde(default)]
    pub kargs: Vec<String>,
    /// Extra bootc arguments passed through `bootc install to-filesystem`.
    #[serde(default)]
    pub args: Vec<String>,
}

/// `[disk]` section.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct DiskConfig {
    /// Directory of systemd-repart `.conf` files defining the target partition layout.
    pub repart_dir: String,
}

impl DistroDescriptor {
    pub fn from_toml(src: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(src)
    }

    /// Load the descriptor from the installed path, falling back to the
    /// in-tree `data/distro.toml` for dev/VM runs.
    pub fn load() -> Result<Self, String> {
        let src = std::fs::read_to_string(DISTRO_PATH)
            .or_else(|_| std::fs::read_to_string("data/distro.toml"))
            .map_err(|e| format!("cannot read distro descriptor ({DISTRO_PATH}): {e}"))?;
        Self::from_toml(&src).map_err(|e| format!("invalid distro descriptor: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_descriptor() {
        let src = r#"
[bootc]
image = "ghcr.io/example/os:latest"

[disk]
repart_dir = "/usr/share/sirius/repart.d"
"#;
        let d = DistroDescriptor::from_toml(src).unwrap();
        assert_eq!(d.bootc.image, "ghcr.io/example/os:latest");
        assert_eq!(d.bootc.target_imgref, None);
        assert!(!d.bootc.enforce_sigpolicy);
        assert!(d.bootc.kargs.is_empty());
        assert!(d.bootc.args.is_empty());
        assert_eq!(d.disk.repart_dir, "/usr/share/sirius/repart.d");
    }

    #[test]
    fn parses_bootc_install_options() {
        let src = r#"
[bootc]
image = "containers-storage:localhost/example-os:dev"
target_imgref = "ghcr.io/example/os:stable"
enforce_sigpolicy = true
kargs = ["rhgb", "quiet"]
args = ["--skip-fetch-check"]

[disk]
repart_dir = "/usr/share/sirius/repart.d"
"#;
        let d = DistroDescriptor::from_toml(src).unwrap();
        assert_eq!(
            d.bootc.target_imgref,
            Some("ghcr.io/example/os:stable".into())
        );
        assert!(d.bootc.enforce_sigpolicy);
        assert_eq!(d.bootc.kargs, vec!["rhgb", "quiet"]);
        assert_eq!(d.bootc.args, vec!["--skip-fetch-check"]);
    }

    #[test]
    fn parses_optional_bentos() {
        let src = r#"
[bootc]
image = "ghcr.io/example/os:latest"

[disk]
repart_dir = "/usr/share/sirius/repart.d"

[[bento]]
title = "Website"
desc = "Learn more about the project"
link = "https://example.com"
icon = "explore-symbolic"
"#;
        let d = DistroDescriptor::from_toml(src).unwrap();
        assert_eq!(d.bentos.len(), 1);
        assert_eq!(d.bentos[0].link, "https://example.com");
        // Absent section parses as empty.
        let plain = "[bootc]\nimage = \"x\"\n[disk]\nrepart_dir = \"/r\"\n";
        assert!(DistroDescriptor::from_toml(plain)
            .unwrap()
            .bentos
            .is_empty());
    }

    #[test]
    fn parses_optional_branding() {
        let src = r#"
[bootc]
image = "x"

[disk]
repart_dir = "/r"

[branding]
logo = "/usr/share/sirius/logo.png"
"#;
        let d = DistroDescriptor::from_toml(src).unwrap();
        assert_eq!(
            d.branding.logo.as_deref(),
            Some("/usr/share/sirius/logo.png")
        );
        assert_eq!(d.branding.icon, None);
        // Absent section parses as default (no logo, no icon override).
        let plain = "[bootc]\nimage = \"x\"\n[disk]\nrepart_dir = \"/r\"\n";
        assert_eq!(
            DistroDescriptor::from_toml(plain).unwrap().branding,
            Branding::default()
        );
    }
}
