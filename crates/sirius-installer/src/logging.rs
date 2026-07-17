//! Installer logging: a tracing subscriber writing to stderr AND a
//! timestamped install log file.

use std::path::PathBuf;
use std::sync::Mutex;

/// Build the install log path: `/tmp/sirius-install-<unix_seconds>.log`.
pub fn log_path(now_unix: u64) -> PathBuf {
    std::env::temp_dir().join(format!("sirius-install-{now_unix}.log"))
}

/// Initialize tracing with a stderr layer plus a file layer (when the log file
/// can be created). Idempotent-safe: errors (e.g. already initialized) are
/// swallowed so callers stay simple.
pub fn init() -> PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = log_path(now);
    // create_new: never follow a pre-existing path (e.g. a symlink planted in /tmp).
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .ok();
    let file_layer = file.map(|f| {
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(Mutex::new(f))
    });
    let _ = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(file_layer)
        .try_init();
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_path_uses_timestamp() {
        assert!(log_path(1234567890)
            .to_string_lossy()
            .ends_with("sirius-install-1234567890.log"));
    }
}
