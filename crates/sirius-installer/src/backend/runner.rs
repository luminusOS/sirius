//! The privileged half. Invoked as `sirius run-playbook` under pkexec.
//! Reads an InstallRequest JSON from stdin, executes it via libreadymade, and
//! writes newline-delimited `Progress` JSON to stdout for the UI to parse.
//!
//! Trust model: stdin comes from the unprivileged UI and is untrusted. This
//! side validates the target disk and loads the bootc image / repart layout
//! from the root-owned `/etc/sirius/distro.toml` itself, so a caller cannot
//! make the root process deploy an arbitrary image or layout.

use crate::backend::adapter::InstallRequest;
use crate::backend::distro::DistroDescriptor;
use crate::backend::Progress;
use gettextrs::gettext;
use libreadymade::playbook::{Playbook, PlaybookProgress};
use std::io::{Read, Write};

/// Upper bound for the request JSON; anything larger is garbage, not a request.
const MAX_REQUEST_BYTES: u64 = 1024 * 1024;

/// Emit one progress record as a JSON line on stdout.
fn emit(p: &Progress) {
    let mut out = std::io::stdout().lock();
    if let Ok(line) = serde_json::to_string(p) {
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
    }
}

/// Translate libreadymade's progress into Sirius's decoupled `Progress`.
fn map_progress(p: PlaybookProgress) -> Progress {
    match p {
        PlaybookProgress::Stage(s) | PlaybookProgress::StageProgress(s) => Progress::Step {
            fraction: 0.0,
            message: s,
        },
        PlaybookProgress::PostModule(name, i, total) => Progress::Step {
            fraction: if total > 0 {
                i as f64 / total as f64
            } else {
                0.0
            },
            message: gettext("post-install: {name} ({index}/{total})")
                .replace("{name}", &name)
                .replace("{index}", &i.to_string())
                .replace("{total}", &total.to_string()),
        },
    }
}

/// The target must be an existing block device under `/dev`. Rejects regular
/// files, directories, and path tricks like `/dev/../etc/passwd`.
fn validate_target_disk(path: &str) -> Result<(), String> {
    use std::os::unix::fs::FileTypeExt;
    let p = std::path::Path::new(path);
    if !p.starts_with("/dev") || p.components().any(|c| c == std::path::Component::ParentDir) {
        return Err(
            gettext("target disk must be an absolute /dev path: {path}").replace("{path}", path)
        );
    }
    let meta = std::fs::metadata(p).map_err(|e| {
        gettext("cannot stat target disk {path}: {error}")
            .replace("{path}", path)
            .replace("{error}", &e.to_string())
    })?;
    if !meta.file_type().is_block_device() {
        return Err(gettext("target disk is not a block device: {path}").replace("{path}", path));
    }
    let disk = crate::backend::storage::scan_disks()?
        .into_iter()
        .find(|disk| disk.path == path)
        .ok_or_else(|| {
            gettext("target is not a supported whole disk: {path}").replace("{path}", path)
        })?;
    if disk.in_use {
        return Err(gettext(
            "target disk has mounted filesystems; unmount them before installing: {path}",
        )
        .replace("{path}", path));
    }
    Ok(())
}

fn fail(message: String) -> i32 {
    emit(&Progress::Error { message });
    1
}

/// Entry point for the privileged subprocess. Returns the process exit code.
pub fn run() -> i32 {
    let mut input = String::new();
    if std::io::stdin()
        .take(MAX_REQUEST_BYTES)
        .read_to_string(&mut input)
        .is_err()
    {
        return fail(gettext("failed to read install request"));
    }
    let request: InstallRequest = match serde_json::from_str(&input) {
        Ok(r) => r,
        Err(e) => {
            return fail(
                gettext("invalid install request: {error}").replace("{error}", &e.to_string()),
            )
        }
    };
    // The request's locale is the UI language picked on the welcome page;
    // pin it so the progress/error lines above follow the same language
    // (pkexec scrubs the environment, so LANGUAGE does not survive the
    // privilege boundary on its own).
    if !request.locale.is_empty() {
        std::env::set_var("LANGUAGE", &request.locale);
    }
    if let Err(e) = validate_target_disk(&request.target_disk) {
        return fail(e);
    }
    let distro = match DistroDescriptor::load() {
        Ok(d) => d,
        Err(e) => return fail(e),
    };

    let manual_mounts = match request.partition_plan.as_ref() {
        Some(plan) => {
            match crate::backend::storage::apply_partition_plan(plan, &request.target_disk) {
                Ok(mounts) => Some(mounts),
                Err(e) => {
                    return fail(
                        gettext("cannot apply partition plan: {error}")
                            .replace("{error}", &e.to_string()),
                    )
                }
            }
        }
        None => None,
    };
    let playbook: Playbook = request.into_playbook(&distro, manual_mounts);
    let (tx, rx) = Playbook::channel();

    // Run the (blocking, root) install on a worker thread; stream progress from the channel.
    // tx is moved into the worker so the channel closes when play() returns, ending the rx loop.
    let worker = std::thread::spawn(move || playbook.play(tx));
    while let Ok(p) = rx.recv() {
        emit(&map_progress(p));
    }
    match worker.join() {
        Ok(Ok(())) => {
            emit(&Progress::Finished);
            0
        }
        Ok(Err(e)) => fail(gettext("install failed: {error}").replace("{error}", &e.to_string())),
        Err(_) => fail(gettext("install thread panicked")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_dev_paths() {
        assert!(validate_target_disk("/etc/passwd").is_err());
        assert!(validate_target_disk("/dev/../etc/passwd").is_err());
        assert!(validate_target_disk("dev/sda").is_err());
    }

    #[test]
    fn rejects_non_block_devices() {
        // /dev/null exists but is a char device, not a block device.
        assert!(validate_target_disk("/dev/null").is_err());
        assert!(validate_target_disk("/dev/does-not-exist").is_err());
    }
}
