//! Converts collected choices into an install request and libreadymade playbook.
//!
//! # Privilege boundary
//!
//! The request carries ONLY the user's choices (disk, encryption, locale,
//! account). What gets installed — the bootc image, repart layout — is read by
//! the privileged runner itself from the root-owned descriptor at
//! `/etc/sirius/distro.toml`. The unprivileged UI must not be able to point the
//! root process at an arbitrary image or repart directory.

use sirius_core::{DistroDescriptor, InstallConfig, InstallRequest, InstallType};

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

/// Construct the real libreadymade [`Playbook`] from a validated request plus
/// the root-owned distro descriptor.
///
/// Runs on the privileged side after the request has crossed the pkexec
/// boundary as JSON; `distro` is loaded there from the root-owned
/// `/etc/sirius/distro.toml`, never taken from the request.
///
/// # Postinstall coverage
///
/// The current libreadymade `postinstall::Module` enum exposes
/// only these variants: `SELinux`, `Dracut`, `ReinstallKernel`, `GRUB2`,
/// `CleanupBoot`, `PrepareFedora`, `EfiStub { distro_name }`, `InitialSetup`,
/// `Language { lang }`, `Keyboard { layout, variant }`, `CryptSetup`, `Script`,
/// `Fstab`. There is **no** module for setting the hostname, creating the user
/// account, or the timezone. We therefore:
///
/// - map `locale` -> `Module::Language { lang }`,
/// - map the canonical XKB `layout[+variant]` id to `Module::Keyboard`, and
/// - emit `Module::InitialSetup`, which writes `/.unconfigured` to trigger
///   the distribution's first-boot setup agent (e.g. gnome-initial-setup) where the user
///   account and hostname are configured on next boot.
///
/// `username`/`full_name`/`hostname`/`timezone` are carried on the request but
/// have no upstream module to consume them here — see the report.
pub fn into_playbook(
    request: InstallRequest,
    distro: &DistroDescriptor,
    manual_mounts: Option<libreadymade::backend::mounts::Mounts>,
) -> libreadymade::playbook::Playbook {
    use libreadymade::backend::postinstall::Module;
    use libreadymade::backend::postinstall::initial_setup::InitialSetup;
    use libreadymade::backend::postinstall::keyboard::Keyboard;
    use libreadymade::backend::postinstall::language::Language;
    use libreadymade::backend::provisioners::disk::manual::Manual;
    use libreadymade::backend::provisioners::disk::repart::Repart;
    use libreadymade::backend::provisioners::filesystem::Bootc;
    use libreadymade::backend::provisioners::{DiskProvisioner, FileSystemProvisioner};
    use libreadymade::playbook::{EncryptionConfig, Playbook};
    use std::path::PathBuf;

    let encryption = request.encrypt.then_some(EncryptionConfig {
        tpm: request.tpm,
        encryption_key: request.encryption_key,
    });

    let disk_provisioner = if let Some(mounts) = manual_mounts {
        DiskProvisioner::Manual(Manual { mounts })
    } else {
        DiskProvisioner::Repart(Repart {
            directory: PathBuf::from(distro.disk.repart_dir.clone()),
            copy_source: None,
        })
    };

    // Keep the target checkout writable while post-install modules apply the
    // selected locale and first-boot state. libreadymade invokes the official
    // `bootc install finalize` operation after those changes are complete.
    let mut bootc_args = distro.bootc.args.clone();
    if !bootc_args.iter().any(|arg| arg == "--skip-finalize") {
        bootc_args.push("--skip-finalize".into());
    }
    let filesystem_provisioner = Some(FileSystemProvisioner::Bootc(Bootc {
        imgref: distro.bootc.image.clone(),
        target_imgref: distro.bootc.target_imgref.clone(),
        enforce_sigpolicy: distro.bootc.enforce_sigpolicy,
        kargs: distro.bootc.kargs.clone(),
        args: bootc_args,
    }));

    let (keyboard_layout, keyboard_variant) =
        parse_xkb_id(&request.keyboard).unwrap_or_else(|| ("us".into(), None));
    let postinstall = vec![
        Module::Language(Language {
            lang: request.locale,
        }),
        Module::Keyboard(Keyboard {
            layout: keyboard_layout,
            variant: keyboard_variant,
        }),
        Module::InitialSetup(InitialSetup),
    ];

    Playbook {
        destination_disk: PathBuf::from(request.target_disk),
        encryption,
        disk_provisioner,
        filesystem_provisioner,
        postinstall,
    }
}

/// Split GNOME Desktop's canonical XKB id (`layout` or `layout+variant`) while
/// keeping untrusted request data out of the generated Xorg configuration.
pub(crate) fn parse_xkb_id(id: &str) -> Option<(String, Option<String>)> {
    let mut parts = id.split('+');
    let layout = parts.next()?;
    let variant = parts.next();
    if parts.next().is_some()
        || !valid_xkb_component(layout)
        || variant.is_some_and(|value| !valid_xkb_component(value))
    {
        return None;
    }
    Some((
        layout.into(),
        variant.filter(|value| !value.is_empty()).map(Into::into),
    ))
}

fn valid_xkb_component(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

#[cfg(test)]
mod tests;
