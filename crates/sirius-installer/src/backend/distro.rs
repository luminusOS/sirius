//! Static description of what Sirius installs: the bootc/OCI image to deploy and
//! the systemd-repart config directory describing the partition layout. Sirius is
//! distro-agnostic — these values come from the distribution's descriptor, shipped
//! at `/etc/sirius/distro.toml`, organized into `[bootc]` and `[disk]` sections.

use serde::Deserialize;

/// Default on-disk path for the distro descriptor, shipped in the ISO.
pub const DISTRO_PATH: &str = "/etc/sirius/distro.toml";

/// Top-level descriptor: one section per concern.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct DistroDescriptor {
    /// `[bootc]` — the OCI image deployment settings.
    pub bootc: BootcConfig,
    /// `[disk]` — the target partition layout.
    pub disk: DiskConfig,
}

/// `[bootc]` section.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct DiskConfig {
    /// Directory of systemd-repart `.conf` files defining the target partition layout.
    pub repart_dir: String,
}

impl DistroDescriptor {
    pub fn from_toml(src: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(src)
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
args = ["--skip-fetch-check", "--bootloader", "none"]

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
        assert_eq!(
            d.bootc.args,
            vec!["--skip-fetch-check", "--bootloader", "none"]
        );
    }
}
