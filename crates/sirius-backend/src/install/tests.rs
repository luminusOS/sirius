use super::*;
use sirius_core::{InstallType, UserAccount};

fn descriptor() -> DistroDescriptor {
    use sirius_core::{BootcConfig, DiskConfig};
    DistroDescriptor {
        bootc: BootcConfig {
            image: "docker://ghcr.io/example/os:latest".into(),
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
    use libreadymade::backend::postinstall::Module;
    use libreadymade::backend::provisioners::{DiskProvisioner, FileSystemProvisioner};

    let mut distro = descriptor();
    distro.bootc.target_imgref = Some("ghcr.io/example/os:stable".into());
    distro.bootc.enforce_sigpolicy = true;
    distro.bootc.kargs = vec!["rhgb".into(), "quiet".into()];
    distro.bootc.args = vec!["--skip-fetch-check".into()];

    let req = build_request(&full_config()).unwrap();
    let playbook = into_playbook(req, &distro, None);

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
    assert_eq!(bootc.imgref, "docker://ghcr.io/example/os:latest");
    assert_eq!(
        bootc.target_imgref,
        Some("ghcr.io/example/os:stable".into())
    );
    assert!(bootc.enforce_sigpolicy);
    assert_eq!(bootc.kargs, vec!["rhgb", "quiet"]);
    assert_eq!(bootc.args, vec!["--skip-fetch-check", "--skip-finalize"]);
    assert!(matches!(
        &playbook.postinstall[1],
        Module::Keyboard(keyboard)
            if keyboard.layout == "br" && keyboard.variant.is_none()
    ));
}

#[test]
fn parses_gnome_xkb_layout_and_variant_ids() {
    assert_eq!(parse_xkb_id("br"), Some(("br".into(), None)));
    assert_eq!(
        parse_xkb_id("us+intl"),
        Some(("us".into(), Some("intl".into())))
    );
    assert_eq!(parse_xkb_id("us+intl+extra"), None);
    assert_eq!(parse_xkb_id("us\"\nEndSection"), None);
}
