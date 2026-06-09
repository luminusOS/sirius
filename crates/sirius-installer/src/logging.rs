//! Installer logging: a tracing subscriber writing to a timestamped file.

use std::path::PathBuf;

/// Build the install log path: `/tmp/sirius-install-<unix_seconds>.log`.
pub fn log_path(now_unix: u64) -> PathBuf {
    PathBuf::from(format!("/tmp/sirius-install-{now_unix}.log"))
}

/// Initialize tracing to stderr and the install log file. Idempotent-safe:
/// errors (e.g. already initialized) are swallowed so callers stay simple.
pub fn init() -> PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let path = log_path(now);
    let file = std::fs::File::create(&path).ok();
    let builder = tracing_subscriber::fmt().with_writer(std::io::stderr);
    let _ = builder.try_init();
    if let Some(f) = file {
        use std::io::Write;
        let mut f = f;
        let _ = writeln!(f, "sirius install log started");
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_path_uses_timestamp() {
        assert_eq!(
            log_path(1234567890),
            PathBuf::from("/tmp/sirius-install-1234567890.log")
        );
    }
}
