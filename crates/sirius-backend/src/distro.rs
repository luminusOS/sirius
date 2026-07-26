//! Loading of the root-owned distribution descriptor.

use sirius_core::DistroDescriptor;

/// Default descriptor path shipped by a live distribution.
pub const DISTRO_PATH: &str = "/etc/sirius/distro.toml";

/// Load the installed descriptor, with the in-tree data file as a development
/// fallback.
pub fn load() -> Result<DistroDescriptor, String> {
    let source = std::fs::read_to_string(DISTRO_PATH)
        .or_else(|_| std::fs::read_to_string("data/distro.toml"))
        .map_err(|error| format!("cannot read distro descriptor ({DISTRO_PATH}): {error}"))?;
    DistroDescriptor::from_toml(&source)
        .map_err(|error| format!("invalid distro descriptor: {error}"))
}
