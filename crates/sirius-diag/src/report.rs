//! Builds the full compatibility report from facts and decides whether the
//! diagnostics page may proceed.

use crate::check::{Check, Status};
use crate::config::{DiagnosticsConfig, DEFAULT_MIN_RAM_GIB};
use crate::facts::SystemFacts;
use crate::probes;

/// Minimum RAM in GiB required to install.
pub const MIN_RAM_GIB: u64 = DEFAULT_MIN_RAM_GIB;
/// Minimum disk size in bytes required to install (20 GiB).
pub const MIN_DISK_BYTES: u64 = 20 * 1024 * 1024 * 1024;

/// Run every probe against the gathered facts, returning the full report.
pub fn run_all_checks(facts: &SystemFacts) -> Vec<Check> {
    run_all_checks_with_min_ram(facts, MIN_RAM_GIB)
}

/// Run every probe using thresholds from `sirius.toml` diagnostics config.
pub fn run_all_checks_with_config(
    facts: &SystemFacts,
    diagnostics: &DiagnosticsConfig,
) -> Vec<Check> {
    run_all_checks_with_min_ram(facts, diagnostics.min_ram_gib)
}

fn run_all_checks_with_min_ram(facts: &SystemFacts, min_ram_gib: u64) -> Vec<Check> {
    vec![
        probes::probe_uefi(&facts.efi_path),
        probes::probe_ram(&facts.meminfo, min_ram_gib),
        probes::probe_disk_space(facts.largest_disk_bytes, MIN_DISK_BYTES),
        probes::probe_secure_boot(facts.secure_boot),
        probes::probe_virt(facts.virt.as_deref()),
        probes::probe_network(facts.online),
    ]
}

/// Whether the install may proceed: blocked only when a `required` check failed.
/// `require` is the list of check ids that hard-gate the install. A check only
/// blocks if it both reports `Status::Fail` (probes own their severity) and its
/// id is in `require`; warning-level checks never block. See [`crate::config::DiagnosticsConfig`].
pub fn is_blocked(checks: &[Check], require: &[String]) -> bool {
    checks
        .iter()
        .any(|c| c.status == Status::Fail && require.iter().any(|r| r == &c.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::Check;

    fn fail(id: &str) -> Check {
        Check::new(id, id, Status::Fail, "x")
    }
    fn pass(id: &str) -> Check {
        Check::new(id, id, Status::Pass, "x")
    }

    #[test]
    fn blocked_when_required_check_fails() {
        let checks = vec![fail("uefi"), pass("ram")];
        let require = vec!["uefi".to_string(), "ram".to_string()];
        assert!(is_blocked(&checks, &require));
    }

    #[test]
    fn not_blocked_when_only_non_required_fails() {
        let checks = vec![fail("network"), pass("uefi")];
        let require = vec!["uefi".to_string()];
        assert!(!is_blocked(&checks, &require));
    }

    #[test]
    fn run_all_checks_returns_six() {
        let facts = SystemFacts::gather();
        assert_eq!(run_all_checks(&facts).len(), 6);
    }

    #[test]
    fn minimum_ram_is_two_gib() {
        assert_eq!(MIN_RAM_GIB, 2);
    }

    #[test]
    fn run_all_checks_uses_configured_ram_threshold() {
        let mut facts = SystemFacts::gather();
        facts.meminfo = "MemTotal:       3145728 kB\n".into();

        let diagnostics = DiagnosticsConfig {
            min_ram_gib: 4,
            ..DiagnosticsConfig::default()
        };
        let checks = run_all_checks_with_config(&facts, &diagnostics);
        let ram = checks.iter().find(|check| check.id == "ram").unwrap();

        assert_eq!(ram.status, Status::Fail);
        assert!(ram.detail.contains("4 GiB required"));
    }
}
