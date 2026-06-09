use crate::check::{Check, Status};
use std::path::Path;

/// Pure UEFI check: passes when the EFI firmware sysfs path exists.
pub fn probe_uefi(efi_path: &Path) -> Check {
    if efi_path.exists() {
        Check::new("uefi", "UEFI firmware", Status::Pass, "EFI firmware detected")
    } else {
        Check::new(
            "uefi",
            "UEFI firmware",
            Status::Fail,
            "System booted in legacy BIOS mode; UEFI is required",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn uefi_pass_when_path_exists() {
        // The crate's own src dir always exists, used as a stand-in for an existing path.
        let existing = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(probe_uefi(&existing).status, Status::Pass);
    }

    #[test]
    fn uefi_fail_when_path_missing() {
        let missing = PathBuf::from("/definitely/not/here/sirius-efi");
        assert_eq!(probe_uefi(&missing).status, Status::Fail);
    }
}
