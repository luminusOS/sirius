//! Unprivileged side: spawn `pkexec sirius run-playbook`, pipe the request to
//! its stdin, and parse its stdout progress lines.

use crate::backend::adapter::InstallRequest;
use crate::backend::Progress;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

/// Parse one stdout line into a `Progress`, or `None` for blank/garbage lines.
pub fn parse_progress_line(line: &str) -> Option<Progress> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    serde_json::from_str::<Progress>(trimmed).ok()
}

/// Spawn the privileged runner and stream progress to `on_progress`.
/// Blocks until the child exits; call from a worker thread, not the GTK thread.
pub fn run_install<F: FnMut(Progress)>(
    request: &InstallRequest,
    self_exe: &str,
    mut on_progress: F,
) -> std::io::Result<()> {
    let mut child = Command::new("pkexec")
        .arg(self_exe)
        .arg("run-playbook")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        let json = serde_json::to_string(request).unwrap_or_default();
        stdin.write_all(json.as_bytes())?;
    } // stdin dropped here -> EOF for the child

    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(p) = parse_progress_line(&line) {
                on_progress(p);
            }
        }
    }

    let status = child.wait()?;
    if !status.success() {
        on_progress(Progress::Error {
            message: format!("installer exited with status {status}"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_step_line() {
        let line = r#"{"Step":{"fraction":0.5,"message":"partitioning"}}"#;
        match parse_progress_line(line) {
            Some(Progress::Step { fraction, message }) => {
                assert_eq!(fraction, 0.5);
                assert_eq!(message, "partitioning");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn ignores_blank_and_garbage() {
        assert!(parse_progress_line("").is_none());
        assert!(parse_progress_line("not json").is_none());
    }

    #[test]
    fn parses_finished_line() {
        assert!(matches!(parse_progress_line(r#""Finished""#), Some(Progress::Finished)));
    }
}
