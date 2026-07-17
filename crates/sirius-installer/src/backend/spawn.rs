//! Unprivileged side: spawn `pkexec sirius run-playbook`, pipe the request to
//! its stdin, and parse its stdout progress lines. When already running as
//! root (live installers often are), pkexec is skipped entirely.

use crate::backend::adapter::InstallRequest;
use crate::backend::Progress;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

/// How many trailing stderr lines to keep for the failure message.
const STDERR_TAIL: usize = 20;

/// Parse one stdout line into a `Progress`, or `None` for blank/garbage lines.
pub fn parse_progress_line(line: &str) -> Option<Progress> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    serde_json::from_str::<Progress>(trimmed).ok()
}

fn is_root() -> bool {
    // SAFETY: geteuid is always safe to call and cannot fail.
    unsafe { libc::geteuid() == 0 }
}

/// Human explanation for a runner exit status. pkexec reserves 126 (auth
/// dialog dismissed) and 127 (not authorized / no polkit agent / exec failed).
fn explain_exit(code: Option<i32>, via_pkexec: bool) -> String {
    match (code, via_pkexec) {
        (Some(126), true) => "authorization dialog was dismissed (pkexec exit 126)".into(),
        (Some(127), true) => "polkit authorization failed (pkexec exit 127) — \
             is a polkit authentication agent running in this session?"
            .into(),
        (Some(c), _) => format!("installer exited with status {c}"),
        (None, _) => "installer was killed by a signal".into(),
    }
}

/// Spawn the privileged runner and stream progress to `on_progress`.
/// Blocks until the child exits; call from a worker thread, not the GTK thread.
pub fn run_install<F: FnMut(Progress)>(
    request: &InstallRequest,
    self_exe: &str,
    mut on_progress: F,
) -> std::io::Result<()> {
    let json = serde_json::to_string(request).map_err(std::io::Error::other)?;

    // Already root (e.g. live session running the installer as root): run the
    // privileged half directly. Otherwise escalate through pkexec/polkit.
    let via_pkexec = !is_root();
    let mut command = if via_pkexec {
        let mut c = Command::new("pkexec");
        c.arg(self_exe);
        c
    } else {
        Command::new(self_exe)
    };
    let mut child = command
        .arg("run-playbook")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(json.as_bytes())?;
    } // stdin dropped here -> EOF for the child

    // Both output streams feed one channel so the UI sees them live and in
    // arrival order: stdout carries Progress JSON, stderr carries the raw
    // libreadymade log (also kept as a tail for the failure message).
    let (tx, rx) = std::sync::mpsc::channel::<Progress>();

    let stdout_reader = child.stdout.take().map(|stdout| {
        let tx = tx.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(p) = parse_progress_line(&line) {
                    let _ = tx.send(p);
                }
            }
        })
    });

    let stderr_tail = child.stderr.take().map(|stderr| {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut tail: VecDeque<String> = VecDeque::with_capacity(STDERR_TAIL);
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if tail.len() == STDERR_TAIL {
                    tail.pop_front();
                }
                tail.push_back(line.clone());
                let _ = tx.send(Progress::Log { line });
            }
            tail
        })
    });

    // The loop ends when both reader threads finish (child closed its pipes).
    drop(tx);
    for p in rx {
        on_progress(p);
    }
    if let Some(h) = stdout_reader {
        let _ = h.join();
    }

    let status = child.wait()?;
    if !status.success() {
        let mut message = explain_exit(status.code(), via_pkexec);
        if let Some(tail) = stderr_tail.and_then(|h| h.join().ok()) {
            if !tail.is_empty() {
                message.push_str("\n--- stderr ---\n");
                message.push_str(&tail.into_iter().collect::<Vec<_>>().join("\n"));
            }
        }
        on_progress(Progress::Error { message });
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
        assert!(matches!(
            parse_progress_line(r#""Finished""#),
            Some(Progress::Finished)
        ));
    }

    #[test]
    fn parses_log_line() {
        match parse_progress_line(r#"{"Log":{"line":"formatting /dev/sda3"}}"#) {
            Some(Progress::Log { line }) => assert_eq!(line, "formatting /dev/sda3"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn explains_pkexec_codes() {
        assert!(explain_exit(Some(127), true).contains("polkit"));
        assert!(explain_exit(Some(126), true).contains("dismissed"));
        assert_eq!(
            explain_exit(Some(1), false),
            "installer exited with status 1"
        );
        assert!(explain_exit(None, true).contains("signal"));
    }
}
