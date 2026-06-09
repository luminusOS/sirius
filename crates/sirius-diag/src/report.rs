//! Builds the full compatibility report from facts and decides whether the
//! diagnostics page may proceed.

use crate::check::{Check, Status};
use crate::facts::SystemFacts;
use crate::probes;

/// Minimum RAM in GiB required to install.
pub const MIN_RAM_GIB: u64 = 4;
/// Minimum disk size in bytes required to install (20 GiB).
pub const MIN_DISK_BYTES: u64 = 20 * 1024 * 1024 * 1024;

/// Run every probe against the gathered facts, returning the full report.
pub fn run_all_checks(facts: &SystemFacts) -> Vec<Check> {
    vec![
        probes::probe_uefi(&facts.efi_path),
        probes::probe_ram(&facts.meminfo, MIN_RAM_GIB),
        probes::probe_disk_space(facts.largest_disk_bytes, MIN_DISK_BYTES),
        probes::probe_secure_boot(facts.secure_boot),
        probes::probe_virt(facts.virt.as_deref()),
        probes::probe_network(facts.online),
    ]
}

/// Whether the install may proceed: blocked only when a `required` check failed.
/// `require` is the list of check ids that hard-gate the install.
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
}
