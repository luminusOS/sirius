//! Static description of what Sirius installs: the bootc/OCI image to deploy and
//! the systemd-repart config directory describing the partition layout. Sirius is
//! distro-agnostic — these values come from the distribution's descriptor, shipped
//! at `/etc/sirius/distro.toml`.

use serde::Deserialize;

/// Default on-disk path for the distro descriptor, shipped in the ISO.
pub const DISTRO_PATH: &str = "/etc/sirius/distro.toml";

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct DistroDescriptor {
    /// The bootc/OCI image reference to deploy (e.g. "ghcr.io/example/os:latest").
    pub bootc_image: String,
    /// Optional image reference persisted into the installed bootc deployment.
    #[serde(default)]
    pub bootc_target_imgref: Option<String>,
    /// Whether bootc should enforce container signature policy during install.
    #[serde(default)]
    pub bootc_enforce_sigpolicy: bool,
    /// Kernel arguments passed through `bootc install to-filesystem`.
    #[serde(default)]
    pub bootc_kargs: Vec<String>,
    /// Extra bootc arguments passed through `bootc install to-filesystem`.
    #[serde(default)]
    pub bootc_args: Vec<String>,
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
bootc_image = "ghcr.io/example/os:latest"
repart_dir = "/usr/share/sirius/repart.d"
"#;
        let d = DistroDescriptor::from_toml(src).unwrap();
        assert_eq!(d.bootc_image, "ghcr.io/example/os:latest");
        assert_eq!(d.bootc_target_imgref, None);
        assert!(!d.bootc_enforce_sigpolicy);
        assert!(d.bootc_kargs.is_empty());
        assert!(d.bootc_args.is_empty());
        assert_eq!(d.repart_dir, "/usr/share/sirius/repart.d");
    }

    #[test]
    fn parses_bootc_install_options() {
        let src = r#"
bootc_image = "containers-storage:localhost/luminusos-workstation:44.dev"
bootc_target_imgref = "ghcr.io/luminusos/luminusos-workstation:44"
bootc_enforce_sigpolicy = true
bootc_kargs = ["rhgb", "quiet"]
bootc_args = ["--skip-fetch-check", "--bootloader", "none"]
repart_dir = "/usr/share/sirius/repart.d"
"#;
        let d = DistroDescriptor::from_toml(src).unwrap();
        assert_eq!(
            d.bootc_target_imgref,
            Some("ghcr.io/luminusos/luminusos-workstation:44".into())
        );
        assert!(d.bootc_enforce_sigpolicy);
        assert_eq!(d.bootc_kargs, vec!["rhgb", "quiet"]);
        assert_eq!(
            d.bootc_args,
            vec!["--skip-fetch-check", "--bootloader", "none"]
        );
    }
}
