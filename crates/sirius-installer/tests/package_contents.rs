//! Checks the RPM asset list declared in Cargo.toml: what a plain install must
//! and must not put on a system, without building the package.

use std::path::Path;

fn assets() -> Vec<toml::Table> {
    let manifest =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    let manifest: toml::Table = manifest.parse().unwrap();
    manifest["package"]["metadata"]["generate-rpm"]["assets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|asset| asset.as_table().unwrap().clone())
        .collect()
}

fn dest(asset: &toml::Table) -> &str {
    asset["dest"].as_str().unwrap()
}

#[test]
fn no_distribution_descriptor_is_installed_as_active_configuration() {
    for asset in assets() {
        assert_ne!(dest(&asset), "/etc/sirius/distro.toml");
    }
}

#[test]
fn the_descriptor_example_is_documentation_only() {
    let example = assets()
        .into_iter()
        .find(|asset| dest(asset) == "/usr/share/doc/sirius/examples/distro.toml")
        .unwrap();
    assert_eq!(example["doc"].as_bool(), Some(true));
    assert!(!example.contains_key("config"));
}

#[test]
fn existing_administrator_configuration_is_preserved_on_upgrade() {
    for asset in assets()
        .iter()
        .filter(|asset| dest(asset).starts_with("/etc/"))
    {
        assert_eq!(
            asset["config"].as_str(),
            Some("noreplace"),
            "{}",
            dest(asset)
        );
    }
}

#[test]
fn the_policy_keeps_administrator_authentication_and_no_live_session_rule_ships() {
    for asset in assets() {
        assert!(
            !dest(&asset).contains("/polkit-1/rules.d/"),
            "{}",
            dest(&asset)
        );
    }
    let policy = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/io.sirius.Installer.policy"),
    )
    .unwrap();
    assert!(!policy.contains("<allow_active>yes"));
    assert!(policy.contains("<allow_active>auth_admin_keep</allow_active>"));
}

#[test]
fn files_are_not_writable_by_group_or_others() {
    for asset in assets() {
        let mode = asset["mode"].as_str().unwrap();
        assert!(
            matches!(mode, "644" | "755"),
            "{} has mode {mode}",
            dest(&asset)
        );
    }
}
