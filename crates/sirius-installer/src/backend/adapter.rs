//! Converts the UI's `InstallConfig` into a serializable `InstallRequest` that
//! crosses the privilege boundary, and from there into a libreadymade
//! `Playbook` on the privileged side.
//!
//! # Privilege boundary
//!
//! The request carries ONLY the user's choices (disk, encryption, locale,
//! account). What gets installed — the bootc image, repart layout — is read by
//! the privileged runner itself from the root-owned descriptor at
//! `/etc/sirius/distro.toml`. The unprivileged UI must not be able to point the
//! root process at an arbitrary image or repart directory.

use crate::backend::distro::DistroDescriptor;
use crate::config_model::{InstallConfig, InstallType, PartitionPlan};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct InstallRequest {
    pub target_disk: String,
    pub encrypt: bool,
    pub tpm: bool,
    pub encryption_key: String,
    pub locale: String,
    pub keyboard: String,
    pub timezone: String,
    pub hostname: String,
    pub username: String,
    pub full_name: String,
    pub partition_plan: Option<PartitionPlan>,
}

/// Build the wire request from collected UI config.
/// Returns an error string naming the first missing/invalid required field.
pub fn build_request(cfg: &InstallConfig) -> Result<InstallRequest, String> {
    let target_disk = cfg
        .destination_disk
        .clone()
        .ok_or("no destination disk selected")?;
    let install_type = cfg.install_type.ok_or("no partition mode selected")?;
    let encrypt = matches!(install_type, InstallType::Encrypted) || cfg.encrypt;
    if matches!(install_type, InstallType::Manual) {
        let plan = cfg
            .partition_plan
            .as_ref()
            .ok_or("manual partitioning has no partition plan")?;
        if plan.disk_path != target_disk {
            return Err("manual partition plan targets a different disk".into());
        }
        plan.validate(std::path::Path::new("/sys/firmware/efi").exists())?;
    }
    if encrypt {
        cfg.validate_encryption()?;
    }
    if !cfg.user.is_empty() {
        cfg.user.validate()?;
    }
    Ok(InstallRequest {
        target_disk,
        encrypt,
        tpm: cfg.tpm && encrypt,
        encryption_key: if encrypt {
            cfg.encryption_passphrase.clone()
        } else {
            String::new()
        },
        locale: cfg.locale.clone().unwrap_or_else(|| "en_US".into()),
        keyboard: cfg.keyboard.clone().unwrap_or_else(|| "us".into()),
        timezone: cfg.timezone.clone().unwrap_or_else(|| "UTC".into()),
        hostname: cfg.user.hostname.clone(),
        username: cfg.user.username.clone(),
        full_name: cfg.user.full_name.clone(),
        partition_plan: cfg.partition_plan.clone(),
    })
}

impl InstallRequest {
    /// Construct the real libreadymade [`Playbook`] from this request plus the
    /// distro descriptor.
    ///
    /// Runs on the privileged side after the request has crossed the pkexec
    /// boundary as JSON; `distro` is loaded there from the root-owned
    /// `/etc/sirius/distro.toml`, never taken from the request.
    ///
    /// # Postinstall coverage
    ///
    /// libreadymade's `postinstall::Module` enum (at the pinned SHA) exposes
    /// only these variants: `SELinux`, `Dracut`, `ReinstallKernel`, `GRUB2`,
    /// `CleanupBoot`, `PrepareFedora`, `EfiStub { distro_name }`, `InitialSetup`,
    /// `Language { lang }`, `CryptSetup`, `Script`, `Fstab`. There is **no**
    /// module for setting the hostname, creating the user account, the timezone,
    /// or the keyboard layout. We therefore:
    ///
    /// - map `locale` -> `Module::Language { lang }`, and
    /// - emit `Module::InitialSetup`, which writes `/.unconfigured` to trigger
    ///   the distribution's first-boot setup agent (e.g. gnome-initial-setup) where the user
    ///   account and hostname are configured on next boot.
    ///
    /// `username`/`full_name`/`hostname`/`timezone`/`keyboard` are carried on the
    /// request but have no upstream module to consume them here — see the report.
    pub fn into_playbook(
        self,
        distro: &DistroDescriptor,
        manual_mounts: Option<libreadymade::backend::mounts::Mounts>,
    ) -> libreadymade::playbook::Playbook {
        use libreadymade::backend::postinstall::Module;
        use libreadymade::backend::postinstall::initial_setup::InitialSetup;
        use libreadymade::backend::postinstall::language::Language;
        use libreadymade::backend::provisioners::disk::manual::Manual;
        use libreadymade::backend::provisioners::disk::repart::Repart;
        use libreadymade::backend::provisioners::filesystem::Bootc;
        use libreadymade::backend::provisioners::{DiskProvisioner, FileSystemProvisioner};
        use libreadymade::playbook::{EncryptionConfig, Playbook};
        use std::path::PathBuf;

        let encryption = self.encrypt.then_some(EncryptionConfig {
            tpm: self.tpm,
            encryption_key: self.encryption_key,
        });

        let disk_provisioner = if let Some(mounts) = manual_mounts {
            DiskProvisioner::Manual(Manual { mounts })
        } else {
            DiskProvisioner::Repart(Repart {
                directory: PathBuf::from(distro.disk.repart_dir.clone()),
                copy_source: None,
            })
        };

        let filesystem_provisioner = Some(FileSystemProvisioner::Bootc(Bootc {
            imgref: distro.bootc.image.clone(),
            target_imgref: distro.bootc.target_imgref.clone(),
            enforce_sigpolicy: distro.bootc.enforce_sigpolicy,
            kargs: distro.bootc.kargs.clone(),
            args: distro.bootc.args.clone(),
        }));

        let postinstall = vec![
            Module::Language(Language { lang: self.locale }),
            Module::InitialSetup(InitialSetup),
        ];

        Playbook {
            destination_disk: PathBuf::from(self.target_disk),
            encryption,
            disk_provisioner,
            filesystem_provisioner,
            postinstall,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_model::{InstallType, UserAccount};

    fn descriptor() -> DistroDescriptor {
        use crate::backend::distro::{BootcConfig, DiskConfig};
        DistroDescriptor {
            bootc: BootcConfig {
                image: "ghcr.io/example/os:latest".into(),
                target_imgref: None,
                enforce_sigpolicy: false,
                kargs: vec![],
                args: vec![],
            },
            disk: DiskConfig {
                repart_dir: "/usr/share/sirius/repart.d".into(),
            },
            bentos: vec![],
            branding: Default::default(),
        }
    }

    fn full_config() -> InstallConfig {
        InstallConfig {
            locale: Some("pt_BR".into()),
            keyboard: Some("br".into()),
            timezone: Some("America/Sao_Paulo".into()),
            destination_disk: Some("/dev/sda".into()),
            destination_disk_name: Some("Test Disk".into()),
            install_type: Some(InstallType::Encrypted),
            partition_plan: None,
            encrypt: false,
            tpm: true,
            encryption_passphrase: "correct horse battery staple".into(),
            encryption_passphrase_confirm: "correct horse battery staple".into(),
            user: UserAccount {
                full_name: "Ada Lovelace".into(),
                username: "ada".into(),
                password: "hunter2hunter".into(),
                password_confirm: "hunter2hunter".into(),
                hostname: "localhost".into(),
            },
        }
    }

    #[test]
    fn builds_request_from_full_config() {
        let req = build_request(&full_config()).unwrap();
        assert_eq!(req.target_disk, "/dev/sda");
        assert!(req.encrypt);
        assert!(req.tpm);
        assert_eq!(req.timezone, "America/Sao_Paulo");
        // The LUKS key is the dedicated passphrase, not the account password.
        assert_eq!(req.encryption_key, "correct horse battery staple");
    }

    #[test]
    fn request_carries_no_image_or_repart_fields() {
        // The privilege boundary: what gets installed comes from the root-owned
        // descriptor, never from the unprivileged request.
        let req = build_request(&full_config()).unwrap();
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("bootc"));
        assert!(!json.contains("repart"));
    }

    #[test]
    fn missing_disk_errors() {
        let mut cfg = full_config();
        cfg.destination_disk = None;
        let err = build_request(&cfg).unwrap_err();
        assert_eq!(err, "no destination disk selected");
    }

    #[test]
    fn tpm_requires_encryption() {
        let mut cfg = full_config();
        cfg.install_type = Some(InstallType::WholeDisk);
        cfg.encrypt = false;
        cfg.tpm = true;
        let req = build_request(&cfg).unwrap();
        assert!(!req.encrypt);
        assert!(!req.tpm);
    }

    #[test]
    fn no_encryption_key_when_plaintext() {
        let mut cfg = full_config();
        cfg.install_type = Some(InstallType::WholeDisk);
        cfg.encrypt = false;
        let req = build_request(&cfg).unwrap();
        assert_eq!(req.encryption_key, "");
    }

    #[test]
    fn plaintext_install_allows_missing_user() {
        let mut cfg = full_config();
        cfg.install_type = Some(InstallType::WholeDisk);
        cfg.encrypt = false;
        cfg.tpm = false;
        cfg.user = UserAccount::default();

        let req = build_request(&cfg).unwrap();
        assert!(!req.encrypt);
        assert_eq!(req.username, "");
        assert_eq!(req.encryption_key, "");
    }

    #[test]
    fn encrypted_install_requires_passphrase() {
        let mut cfg = full_config();
        cfg.install_type = Some(InstallType::Encrypted);
        cfg.encrypt = true;
        cfg.encryption_passphrase.clear();
        cfg.encryption_passphrase_confirm.clear();

        let err = build_request(&cfg).unwrap_err();
        assert_eq!(err, "Passphrase must be at least 8 characters");
    }

    #[test]
    fn encrypted_install_does_not_require_user_account() {
        // The passphrase is dedicated now, so encryption no longer binds to
        // (or requires) the account password.
        let mut cfg = full_config();
        cfg.install_type = Some(InstallType::Encrypted);
        cfg.encrypt = true;
        cfg.user = UserAccount::default();

        let req = build_request(&cfg).unwrap();
        assert!(req.encrypt);
        assert_eq!(req.encryption_key, "correct horse battery staple");
        assert_eq!(req.username, "");
    }

    #[test]
    fn playbook_takes_image_and_repart_from_descriptor() {
        use libreadymade::backend::provisioners::{DiskProvisioner, FileSystemProvisioner};

        let mut distro = descriptor();
        distro.bootc.target_imgref = Some("ghcr.io/example/os:stable".into());
        distro.bootc.enforce_sigpolicy = true;
        distro.bootc.kargs = vec!["rhgb".into(), "quiet".into()];
        distro.bootc.args = vec!["--skip-fetch-check".into()];

        let req = build_request(&full_config()).unwrap();
        let playbook = req.into_playbook(&distro, None);

        let DiskProvisioner::Repart(repart) = &playbook.disk_provisioner else {
            panic!("expected repart disk provisioner");
        };
        assert_eq!(
            repart.directory,
            std::path::PathBuf::from("/usr/share/sirius/repart.d")
        );
        let Some(FileSystemProvisioner::Bootc(bootc)) = &playbook.filesystem_provisioner else {
            panic!("expected bootc filesystem provisioner");
        };
        assert_eq!(bootc.imgref, "ghcr.io/example/os:latest");
        assert_eq!(
            bootc.target_imgref,
            Some("ghcr.io/example/os:stable".into())
        );
        assert!(bootc.enforce_sigpolicy);
        assert_eq!(bootc.kargs, vec!["rhgb", "quiet"]);
        assert_eq!(bootc.args, vec!["--skip-fetch-check"]);
    }
}
