//! Privileged installation orchestration invoked under pkexec.
//! Reads an InstallRequest JSON from stdin, executes it via libreadymade, and
//! writes newline-delimited `Progress` JSON to stdout for the UI to parse.
//!
//! Trust model: stdin comes from the unprivileged UI and is untrusted. This
//! side validates the target disk and loads the bootc image / repart layout
//! from the root-owned `/etc/sirius/distro.toml` itself, so a caller cannot
//! make the root process deploy an arbitrary image or layout.

use gettextrs::gettext;
use libreadymade::playbook::{Playbook, PlaybookProgress};
use sirius_core::{InstallRequest, Progress};
use std::collections::HashSet;
use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

/// Upper bound for the request JSON; anything larger is garbage, not a request.
const MAX_REQUEST_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
enum PreparationStep {
    SwapOff(String),
    Unmount(String),
    WipeSignatures(String),
    SettleUdev,
}

struct TargetDevices {
    disk_key: String,
    base_keys: HashSet<String>,
    all_keys: HashSet<String>,
}

struct PreparedTarget {
    path: String,
    automatic: bool,
    devices: TargetDevices,
}

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
fn target_disk(path: &str) -> Result<crate::storage::DiskSnapshot, String> {
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
    crate::storage::scan_disks()?
        .into_iter()
        .find(|disk| disk.path == path)
        .ok_or_else(|| {
            gettext("target is not a supported whole disk: {path}").replace("{path}", path)
        })
}

fn validate_target_disk(path: &str) -> Result<(), String> {
    let disk = target_disk(path)?;
    if disk.in_use {
        return Err(gettext(
            "target disk has mounted filesystems; unmount them before installing: {path}",
        )
        .replace("{path}", path));
    }
    Ok(())
}

fn read_request() -> Result<String, String> {
    let mut input = String::new();
    std::io::stdin()
        .take(MAX_REQUEST_BYTES + 1)
        .read_to_string(&mut input)
        .map_err(|_| gettext("failed to read install request"))?;
    if input.len() as u64 > MAX_REQUEST_BYTES {
        return Err(gettext("install request exceeds size limit"));
    }
    Ok(input)
}

fn parse_request(input: &str) -> Result<InstallRequest, String> {
    serde_json::from_str(input).map_err(|error| {
        gettext("invalid install request: {error}").replace("{error}", &error.to_string())
    })
}

fn device_key(path: &std::path::Path) -> Result<String, String> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};

    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
    let device = if metadata.file_type().is_block_device() {
        metadata.rdev()
    } else {
        metadata.dev()
    };
    Ok(format!("{}:{}", libc::major(device), libc::minor(device)))
}

fn target_devices(path: &str) -> Result<TargetDevices, String> {
    let disk_key = device_key(std::path::Path::new(path))?;
    let sysfs = std::path::Path::new("/sys/dev/block").join(&disk_key);
    let entries = std::fs::read_dir(&sysfs)
        .map_err(|error| format!("cannot inspect {}: {error}", sysfs.display()))?;
    let mut base_keys = HashSet::from([disk_key.clone()]);
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("cannot inspect {}: {error}", sysfs.display()))?;
        if !entry.path().join("partition").exists() {
            continue;
        }
        let dev = entry.path().join("dev");
        let key = std::fs::read_to_string(&dev)
            .map_err(|error| format!("cannot inspect {}: {error}", dev.display()))?;
        base_keys.insert(key.trim().to_owned());
    }

    let mut all_keys = base_keys.clone();
    let mut pending = base_keys.iter().cloned().collect::<Vec<_>>();
    while let Some(key) = pending.pop() {
        let holders = std::path::Path::new("/sys/dev/block")
            .join(&key)
            .join("holders");
        let entries = std::fs::read_dir(&holders)
            .map_err(|error| format!("cannot inspect {}: {error}", holders.display()))?;
        for entry in entries {
            let entry =
                entry.map_err(|error| format!("cannot inspect {}: {error}", holders.display()))?;
            let holder = entry.path().join("dev");
            let holder_key = std::fs::read_to_string(&holder)
                .map_err(|error| format!("cannot inspect {}: {error}", holder.display()))?;
            let holder_key = holder_key.trim().to_owned();
            if all_keys.insert(holder_key.clone()) {
                pending.push(holder_key);
            }
        }
    }
    Ok(TargetDevices {
        disk_key,
        base_keys,
        all_keys,
    })
}

fn active_swaps(contents: &str) -> Result<Vec<(String, String)>, String> {
    contents
        .lines()
        .skip(1)
        .filter_map(|line| line.split_whitespace().next())
        .map(|path| device_key(std::path::Path::new(path)).map(|key| (path.to_owned(), key)))
        .collect()
}

fn unescape_mount_field(value: &str) -> String {
    value
        .replace("\\040", " ")
        .replace("\\011", "\t")
        .replace("\\012", "\n")
        .replace("\\134", "\\")
}

fn mounted_devices(contents: &str) -> Vec<(String, String)> {
    contents
        .lines()
        .filter_map(|line| {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            (fields.len() > 4).then(|| (fields[2].to_owned(), unescape_mount_field(fields[4])))
        })
        .collect()
}

fn preparation_steps(
    automatic: bool,
    target_keys: &HashSet<String>,
    swaps: &[(String, String)],
    mounts: &[(String, String)],
) -> Vec<PreparationStep> {
    let mut swap_paths = swaps
        .iter()
        .filter(|(_, key)| target_keys.contains(key))
        .map(|(path, _)| path.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    swap_paths.sort();

    let mut mountpoints = if automatic {
        mounts
            .iter()
            .filter(|(key, _)| target_keys.contains(key))
            .map(|(_, mountpoint)| mountpoint.clone())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    mountpoints.sort_by(|left, right| {
        right
            .matches('/')
            .count()
            .cmp(&left.matches('/').count())
            .then_with(|| right.len().cmp(&left.len()))
            .then_with(|| left.cmp(right))
    });

    swap_paths
        .into_iter()
        .map(PreparationStep::SwapOff)
        .chain(mountpoints.into_iter().map(PreparationStep::Unmount))
        .collect()
}

fn run_preparation_step(step: &PreparationStep) -> Result<(), String> {
    let (program, args): (&str, Vec<&str>) = match step {
        PreparationStep::SwapOff(path) => ("swapoff", vec!["--", path]),
        PreparationStep::Unmount(path) => ("umount", vec!["--", path]),
        PreparationStep::WipeSignatures(path) => {
            ("wipefs", vec!["--all", "--force", "--lock=yes", "--", path])
        }
        PreparationStep::SettleUdev => ("udevadm", vec!["settle"]),
    };
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|error| format!("cannot run {program}: {error}"))?;
    if !status.success() {
        return Err(format!("{program} exited with {status}"));
    }
    Ok(())
}

fn execute_preparation(
    steps: &[PreparationStep],
    mut execute: impl FnMut(&PreparationStep) -> Result<(), String>,
) -> Result<(), String> {
    for step in steps {
        execute(step)?;
    }
    Ok(())
}

fn ensure_target_unused(devices: &TargetDevices) -> Result<(), String> {
    let swaps = std::fs::read_to_string("/proc/swaps")
        .map_err(|error| format!("cannot recheck active swaps: {error}"))?;
    if active_swaps(&swaps)?
        .iter()
        .any(|(_, key)| devices.all_keys.contains(key))
    {
        return Err(gettext("target disk still backs active swap"));
    }

    let mountinfo = std::fs::read_to_string("/proc/self/mountinfo")
        .map_err(|error| format!("cannot recheck active mounts: {error}"))?;
    if mounted_devices(&mountinfo)
        .iter()
        .any(|(key, _)| devices.all_keys.contains(key))
    {
        return Err(gettext("target disk still has mounted filesystems"));
    }

    for key in &devices.base_keys {
        let holders = std::path::Path::new("/sys/dev/block")
            .join(key)
            .join("holders");
        let mut entries = std::fs::read_dir(&holders)
            .map_err(|error| format!("cannot inspect {}: {error}", holders.display()))?;
        if entries
            .next()
            .transpose()
            .map_err(|error| format!("cannot inspect {}: {error}", holders.display()))?
            .is_some()
        {
            return Err(gettext("target disk still has active holders"));
        }
    }
    Ok(())
}

fn quiesce_target(request: &InstallRequest) -> Result<PreparedTarget, String> {
    let disk = target_disk(&request.target_disk)?;
    let devices = target_devices(&request.target_disk)?;
    let automatic = request.partition_plan.is_none();
    if !automatic && disk.in_use {
        return Err(gettext(
            "target disk has mounted filesystems; unmount them before installing: {path}",
        )
        .replace("{path}", &request.target_disk));
    }
    if devices.all_keys.len() != devices.base_keys.len() {
        return Err(gettext("cannot prepare target disk: {error}")
            .replace("{error}", &gettext("target disk still has active holders")));
    }
    let swaps = std::fs::read_to_string("/proc/swaps")
        .map_err(|error| format!("cannot read active swaps: {error}"))?;
    let mountinfo = std::fs::read_to_string("/proc/self/mountinfo")
        .map_err(|error| format!("cannot read active mounts: {error}"))?;
    let steps = preparation_steps(
        automatic,
        &devices.all_keys,
        &active_swaps(&swaps)?,
        &mounted_devices(&mountinfo),
    );
    execute_preparation(&steps, run_preparation_step)
        .and_then(|()| run_preparation_step(&PreparationStep::SettleUdev))
        .and_then(|()| ensure_target_unused(&devices))
        .map_err(|error| {
            gettext("cannot prepare target disk: {error}").replace("{error}", &error)
        })?;
    Ok(PreparedTarget {
        path: request.target_disk.clone(),
        automatic,
        devices,
    })
}

fn wipe_target(target: &PreparedTarget) -> Result<(), String> {
    if !target.automatic {
        return Ok(());
    }
    if device_key(std::path::Path::new(&target.path))? != target.devices.disk_key {
        return Err(gettext("target disk identity changed"));
    }
    ensure_target_unused(&target.devices)?;
    run_preparation_step(&PreparationStep::WipeSignatures(target.path.clone()))?;
    run_preparation_step(&PreparationStep::SettleUdev)
}

fn validate_request_before_preparation(request: &InstallRequest) -> Result<(), String> {
    if crate::install::parse_xkb_id(&request.keyboard).is_none() {
        return Err(
            gettext("invalid keyboard layout: {layout}").replace("{layout}", &request.keyboard)
        );
    }
    crate::distro::load()?;
    if let Some(plan) = &request.partition_plan {
        if plan.disk_path != request.target_disk {
            return Err(gettext("cannot apply partition plan: {error}")
                .replace("{error}", "partition plan does not match the selected disk"));
        }
        plan.validate(std::path::Path::new("/sys/firmware/efi").exists())
            .map_err(|error| {
                gettext("cannot apply partition plan: {error}").replace("{error}", &error)
            })?;
    }
    target_disk(&request.target_disk)?;
    Ok(())
}

fn validate_namespace_support() -> Result<(), String> {
    let status = Command::new("unshare")
        .args(["--mount", "--propagation", "slave", "--", "true"])
        .status()
        .map_err(|error| {
            gettext("cannot create the private install namespace: {error}")
                .replace("{error}", &error.to_string())
        })?;
    if !status.success() {
        return Err(
            gettext("cannot create the private install namespace: {error}")
                .replace("{error}", &status.to_string()),
        );
    }
    Ok(())
}

fn fail(message: String) -> i32 {
    emit(&Progress::Error { message });
    1
}

/// Re-execute the privileged installer in a private mount namespace.
///
/// libreadymade temporarily mounts the target filesystems while it runs bootc.
/// Keeping those mounts private prevents them from propagating into the live
/// system and lets the inner process hide the host's OSTree repository.
pub fn run_isolated() -> i32 {
    let input = match read_request() {
        Ok(input) => input,
        Err(error) => return fail(error),
    };
    let request = match parse_request(&input) {
        Ok(request) => request,
        Err(error) => return fail(error),
    };
    if !request.locale.is_empty() {
        // SAFETY: set before the namespace child or any worker thread starts.
        unsafe { std::env::set_var("LANGUAGE", &request.locale) };
    }
    if let Err(error) = validate_request_before_preparation(&request) {
        return fail(error);
    }

    let executable = match std::env::current_exe() {
        Ok(path) => path,
        Err(e) => {
            return fail(
                gettext("cannot locate the installer executable: {error}")
                    .replace("{error}", &e.to_string()),
            );
        }
    };
    if let Err(error) = validate_namespace_support() {
        return fail(error);
    }
    let prepared = match quiesce_target(&request) {
        Ok(prepared) => prepared,
        Err(error) => return fail(error),
    };
    let parent_pid = std::process::id() as libc::pid_t;
    let mut command = Command::new("unshare");
    command
        .args(["--mount", "--propagation", "slave", "--"])
        .arg(executable)
        .arg("run-playbook-inner")
        .stdin(Stdio::piped());
    // SAFETY: pre_exec only invokes async-signal-safe libc calls before exec.
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() != parent_pid {
                return Err(std::io::Error::other(
                    "installer parent exited before namespace setup",
                ));
            }
            Ok(())
        });
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return fail(
                gettext("cannot create the private install namespace: {error}")
                    .replace("{error}", &error.to_string()),
            );
        }
    };
    if let Err(error) = wipe_target(&prepared) {
        let _ = child.kill();
        let _ = child.wait();
        return fail(gettext("cannot prepare target disk: {error}").replace("{error}", &error));
    }
    let write_result = child
        .stdin
        .take()
        .ok_or_else(|| "namespace child has no stdin".to_owned())
        .and_then(|mut stdin| {
            stdin
                .write_all(input.as_bytes())
                .map_err(|error| error.to_string())
        });
    if let Err(error) = write_result {
        let _ = child.kill();
        let _ = child.wait();
        return fail(
            gettext("cannot create the private install namespace: {error}")
                .replace("{error}", &error),
        );
    }
    match child.wait() {
        Ok(status) => status.code().unwrap_or(1),
        Err(error) => fail(
            gettext("cannot create the private install namespace: {error}")
                .replace("{error}", &error.to_string()),
        ),
    }
}

/// bootc 1.12 can mistake the live host's OSTree commits for the external
/// source image and abort with "Multiple commit objects found". Its upstream
/// outside-container tests avoid that by masking `/sysroot/ostree`. Do the
/// same only inside the private namespace created by [`run_isolated`].
fn mask_host_ostree() -> Result<(), String> {
    let host_ostree = std::path::Path::new("/sysroot/ostree");
    if !host_ostree.exists() {
        return Ok(());
    }
    let empty = std::path::Path::new("/run/sirius/empty");
    std::fs::create_dir_all(empty).map_err(|e| {
        gettext("cannot prepare the private install namespace: {error}")
            .replace("{error}", &e.to_string())
    })?;
    let status = Command::new("mount")
        .args(["--bind"])
        .arg(empty)
        .arg(host_ostree)
        .status()
        .map_err(|e| {
            gettext("cannot hide the host OSTree repository: {error}")
                .replace("{error}", &e.to_string())
        })?;
    if !status.success() {
        return Err(
            gettext("cannot hide the host OSTree repository: mount exited with {status}")
                .replace("{status}", &status.to_string()),
        );
    }
    Ok(())
}

/// Entry point for the privileged subprocess. Returns the process exit code.
pub fn run() -> i32 {
    if let Err(e) = mask_host_ostree() {
        return fail(e);
    }
    let input = match read_request() {
        Ok(input) => input,
        Err(error) => return fail(error),
    };
    let request = match parse_request(&input) {
        Ok(request) => request,
        Err(error) => return fail(error),
    };
    // The request's locale is the UI language picked on the welcome page;
    // pin it so the progress/error lines above follow the same language
    // (pkexec scrubs the environment, so LANGUAGE does not survive the
    // privilege boundary on its own).
    if !request.locale.is_empty() {
        // SAFETY: set before any thread that reads the environment is spawned.
        unsafe { std::env::set_var("LANGUAGE", &request.locale) };
    }
    // libreadymade intentionally defaults systemd-repart to dry-run in debug
    // builds. `run-playbook` is only reached after the user has confirmed the
    // destructive operation and crossed the privilege boundary, so both debug
    // and release binaries must perform the requested partitioning here.
    //
    // SAFETY: set before the playbook worker thread is spawned.
    unsafe { std::env::set_var("READYMADE_DRY_RUN", "0") };
    if let Err(e) = validate_target_disk(&request.target_disk) {
        return fail(e);
    }
    if crate::install::parse_xkb_id(&request.keyboard).is_none() {
        return fail(
            gettext("invalid keyboard layout: {layout}").replace("{layout}", &request.keyboard),
        );
    }
    let distro = match crate::distro::load() {
        Ok(d) => d,
        Err(e) => return fail(e),
    };

    let manual_mounts = match request.partition_plan.as_ref() {
        Some(plan) => match crate::storage::apply_partition_plan(plan, &request.target_disk) {
            Ok(mounts) => Some(mounts),
            Err(e) => {
                return fail(
                    gettext("cannot apply partition plan: {error}")
                        .replace("{error}", &e.to_string()),
                );
            }
        },
        None => None,
    };
    let playbook: Playbook = crate::install::into_playbook(request, &distro, manual_mounts);
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

    #[test]
    fn preparation_is_scoped_and_ordered() {
        let target_keys = HashSet::from(["259:1".to_owned(), "253:0".to_owned()]);
        let swaps = vec![
            ("/dev/dm-0".to_owned(), "253:0".to_owned()),
            ("/dev/zram0".to_owned(), "252:0".to_owned()),
        ];
        let mounts = vec![
            ("259:1".to_owned(), "/run/media/root".to_owned()),
            ("253:0".to_owned(), "/run/media/root/home".to_owned()),
            ("253:0".to_owned(), "/run/media/root/home".to_owned()),
            ("8:1".to_owned(), "/unrelated".to_owned()),
        ];

        assert_eq!(
            preparation_steps(true, &target_keys, &swaps, &mounts),
            vec![
                PreparationStep::SwapOff("/dev/dm-0".into()),
                PreparationStep::Unmount("/run/media/root/home".into()),
                PreparationStep::Unmount("/run/media/root/home".into()),
                PreparationStep::Unmount("/run/media/root".into()),
            ]
        );
    }

    #[test]
    fn manual_partitioning_skips_whole_disk_preparation() {
        let target_keys = HashSet::from(["8:2".to_owned()]);
        let swaps = vec![("/dev/sda2".to_owned(), "8:2".to_owned())];
        let mounts = vec![("8:2".to_owned(), "/preserved".to_owned())];

        assert_eq!(
            preparation_steps(false, &target_keys, &swaps, &mounts),
            vec![PreparationStep::SwapOff("/dev/sda2".into())]
        );
    }

    #[test]
    fn mountinfo_parser_decodes_paths() {
        let mounts =
            mounted_devices("36 25 259:1 / /run/media/My\\040Disk rw - btrfs /dev/nvme0n1p1 rw\n");
        assert_eq!(mounts, vec![("259:1".into(), "/run/media/My Disk".into())]);
    }

    #[test]
    fn preparation_stops_on_first_failure() {
        let steps = vec![
            PreparationStep::SwapOff("/dev/sda2".into()),
            PreparationStep::Unmount("/target".into()),
            PreparationStep::WipeSignatures("/dev/sda".into()),
        ];
        let mut visited = Vec::new();
        let result = execute_preparation(&steps, |step| {
            visited.push(step.clone());
            if matches!(step, PreparationStep::Unmount(_)) {
                Err("busy".into())
            } else {
                Ok(())
            }
        });

        assert_eq!(result, Err("busy".into()));
        assert_eq!(visited, steps[..2]);
    }
}
