//! Background bridge between the wizard and the privileged backend.

use sirius_core::{InstallConfig, Progress};

pub(super) fn start(
    config: &InstallConfig,
    mut on_progress: impl FnMut(Progress) + Send + 'static,
) -> Result<(), String> {
    // The request contains only user choices. The privileged runner loads the
    // image and repart layout from its root-owned descriptor.
    let request = sirius_backend::install::build_request(config)?;
    let executable = std::env::current_exe()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "/usr/bin/sirius".into());

    std::thread::spawn(move || {
        if let Err(error) =
            sirius_backend::spawn::run_install(&request, &executable, &mut on_progress)
        {
            on_progress(Progress::Error {
                message: format!("failed to launch installer: {error}"),
            });
        }
    });
    Ok(())
}
