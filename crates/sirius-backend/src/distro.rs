//! Loading of the root-owned distribution descriptor.

use sirius_core::DistroDescriptor;
use std::path::Path;

/// Path of the descriptor a distribution provides. The package ships only an
/// example under `/usr/share/doc/sirius/examples`, so this file is absent until
/// the distribution or administrator creates it.
pub const DISTRO_PATH: &str = "/etc/sirius/distro.toml";

/// Load the installed descriptor. Debug builds fall back to the in-tree data
/// file; release builds never read a descriptor relative to the working
/// directory, because the privileged runner must only trust root-owned files.
pub fn load() -> Result<DistroDescriptor, String> {
    load_from(Path::new(DISTRO_PATH))
}

fn load_from(path: &Path) -> Result<DistroDescriptor, String> {
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        #[cfg(debug_assertions)]
        Err(_) if path == Path::new(DISTRO_PATH) => {
            std::fs::read_to_string("data/distro.toml").map_err(|error| unreadable(path, &error))?
        }
        Err(error) => return Err(unreadable(path, &error)),
    };
    DistroDescriptor::from_toml(&source)
        .map_err(|error| format!("invalid distro descriptor: {error}"))
}

fn unreadable(path: &Path, error: &std::io::Error) -> String {
    format!(
        "cannot read distro descriptor ({}): {error}. Create it from /usr/share/doc/sirius/examples/distro.toml",
        path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_descriptor_is_an_error_that_names_the_example() {
        let error = load_from(Path::new("/nonexistent/sirius/distro.toml")).unwrap_err();
        assert!(error.contains("/nonexistent/sirius/distro.toml"));
        assert!(error.contains("/usr/share/doc/sirius/examples/distro.toml"));
    }

    #[test]
    fn a_malformed_descriptor_is_rejected() {
        let dir = std::env::temp_dir().join(format!("sirius-distro-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("distro.toml");
        std::fs::write(&path, "[bootc]\nimage = 3\n").unwrap();
        assert!(
            load_from(&path)
                .unwrap_err()
                .starts_with("invalid distro descriptor")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
