//! Converts the UI's `InstallConfig` (+ distro descriptor) into a serializable
//! `InstallRequest` that crosses the privilege boundary, and from there into a
//! libreadymade `Playbook` on the privileged side.

use crate::backend::distro::DistroDescriptor;
use crate::config_model::{InstallConfig, InstallType};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct InstallRequest {
    pub bootc_image: String,
    #[serde(default)]
    pub bootc_target_imgref: Option<String>,
    #[serde(default)]
    pub bootc_enforce_sigpolicy: bool,
    #[serde(default)]
    pub bootc_kargs: Vec<String>,
    #[serde(default)]
    pub bootc_args: Vec<String>,
    pub repart_dir: String,
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
}

/// Build the wire request from collected UI config + the distro descriptor.
/// Returns an error string naming the first missing/invalid required field.
pub fn build_request(
    cfg: &InstallConfig,
    distro: &DistroDescriptor,
) -> Result<InstallRequest, String> {
    let target_disk = cfg
        .destination_disk
        .clone()
        .ok_or("no destination disk selected")?;
    let install_type = cfg.install_type.ok_or("no partition mode selected")?;
    cfg.user.validate()?;
    let encrypt = matches!(install_type, InstallType::Encrypted) || cfg.encrypt;
    Ok(InstallRequest {
        bootc_image: distro.bootc.image.clone(),
        bootc_target_imgref: distro.bootc.target_imgref.clone(),
        bootc_enforce_sigpolicy: distro.bootc.enforce_sigpolicy,
        bootc_kargs: distro.bootc.kargs.clone(),
        bootc_args: distro.bootc.args.clone(),
        repart_dir: distro.disk.repart_dir.clone(),
        target_disk,
        encrypt,
        tpm: cfg.tpm && encrypt,
        // MVP: bind LUKS to the user password when encrypting (InstallConfig collects no separate key).
        encryption_key: if encrypt {
            cfg.user.password.clone()
        } else {
            String::new()
        },
        locale: cfg.locale.clone().unwrap_or_else(|| "en_US".into()),
        keyboard: cfg.keyboard.clone().unwrap_or_else(|| "us".into()),
        timezone: cfg.timezone.clone().unwrap_or_else(|| "UTC".into()),
        hostname: cfg.user.hostname.clone(),
        username: cfg.user.username.clone(),
        full_name: cfg.user.full_name.clone(),
    })
}

impl InstallRequest {
    /// Construct the real libreadymade [`Playbook`] from this request.
    ///
    /// Runs on the privileged side after the request has crossed the pkexec
    /// boundary as JSON.
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
    pub fn into_playbook(self) -> libreadymade::playbook::Playbook {
        use libreadymade::backend::postinstall::initial_setup::InitialSetup;
        use libreadymade::backend::postinstall::language::Language;
        use libreadymade::backend::postinstall::Module;
        use libreadymade::backend::provisioners::disk::repart::Repart;
        use libreadymade::backend::provisioners::filesystem::Bootc;
        use libreadymade::backend::provisioners::{DiskProvisioner, FileSystemProvisioner};
        use libreadymade::playbook::{EncryptionConfig, Playbook};
        use std::path::PathBuf;

        let encryption = self.encrypt.then(|| EncryptionConfig {
            tpm: self.tpm,
            encryption_key: self.encryption_key,
        });

        let disk_provisioner = DiskProvisioner::Repart(Repart {
            directory: PathBuf::from(self.repart_dir),
            copy_source: None,
        });

        let filesystem_provisioner = Some(FileSystemProvisioner::Bootc(Bootc {
            imgref: self.bootc_image,
            target_imgref: self.bootc_target_imgref,
            enforce_sigpolicy: self.bootc_enforce_sigpolicy,
            kargs: self.bootc_kargs,
            args: self.bootc_args,
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
        }
    }

    fn full_config() -> InstallConfig {
        InstallConfig {
            locale: Some("pt_BR".into()),
            keyboard: Some("br".into()),
            timezone: Some("America/Sao_Paulo".into()),
            destination_disk: Some("/dev/sda".into()),
            install_type: Some(InstallType::Encrypted),
            encrypt: false,
            tpm: true,
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
        let req = build_request(&full_config(), &descriptor()).unwrap();
        assert_eq!(req.target_disk, "/dev/sda");
        assert_eq!(req.bootc_image, "ghcr.io/example/os:latest");
        assert_eq!(req.bootc_target_imgref, None);
        assert!(!req.bootc_enforce_sigpolicy);
        assert!(req.bootc_kargs.is_empty());
        assert!(req.bootc_args.is_empty());
        assert!(req.encrypt);
        assert!(req.tpm);
        assert_eq!(req.timezone, "America/Sao_Paulo");
        assert_eq!(req.encryption_key, "hunter2hunter");
    }

    #[test]
    fn missing_disk_errors() {
        let mut cfg = full_config();
        cfg.destination_disk = None;
        let err = build_request(&cfg, &descriptor()).unwrap_err();
        assert_eq!(err, "no destination disk selected");
    }

    #[test]
    fn tpm_requires_encryption() {
        let mut cfg = full_config();
        cfg.install_type = Some(InstallType::WholeDisk);
        cfg.encrypt = false;
        cfg.tpm = true;
        let req = build_request(&cfg, &descriptor()).unwrap();
        assert!(!req.encrypt);
        assert!(!req.tpm);
    }

    #[test]
    fn no_encryption_key_when_plaintext() {
        let mut cfg = full_config();
        cfg.install_type = Some(InstallType::WholeDisk);
        cfg.encrypt = false;
        let req = build_request(&cfg, &descriptor()).unwrap();
        assert_eq!(req.encryption_key, "");
    }

    #[test]
    fn carries_bootc_options_from_descriptor() {
        let mut distro = descriptor();
        distro.bootc.target_imgref = Some("ghcr.io/example/os:stable".into());
        distro.bootc.enforce_sigpolicy = true;
        distro.bootc.kargs = vec!["rhgb".into(), "quiet".into()];
        distro.bootc.args = vec![
            "--skip-fetch-check".into(),
            "--bootloader".into(),
            "none".into(),
        ];

        let req = build_request(&full_config(), &distro).unwrap();
        assert_eq!(
            req.bootc_target_imgref,
            Some("ghcr.io/example/os:stable".into())
        );
        assert!(req.bootc_enforce_sigpolicy);
        assert_eq!(req.bootc_kargs, vec!["rhgb", "quiet"]);
        assert_eq!(
            req.bootc_args,
            vec!["--skip-fetch-check", "--bootloader", "none"]
        );
    }
}
