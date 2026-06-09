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
        assert_eq!(d.repart_dir, "/usr/share/sirius/repart.d");
    }
}
