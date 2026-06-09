//! The privileged half. Invoked as `sirius run-playbook` under pkexec.
//! Reads an InstallRequest JSON from stdin, executes it via libreadymade, and
//! writes newline-delimited `Progress` JSON to stdout for the UI to parse.

use crate::backend::adapter::InstallRequest;
use crate::backend::Progress;
use libreadymade::playbook::{Playbook, PlaybookProgress};
use std::io::{Read, Write};

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
        PlaybookProgress::Stage(s) => Progress::Step { fraction: 0.0, message: s },
        PlaybookProgress::StageProgress(s) => Progress::Step { fraction: 0.0, message: s },
        PlaybookProgress::PostModule(name, i, total) => Progress::Step {
            fraction: if total > 0 { i as f64 / total as f64 } else { 0.0 },
            message: format!("post-install: {name} ({i}/{total})"),
        },
    }
}

/// Entry point for the privileged subprocess. Returns the process exit code.
pub fn run() -> i32 {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        emit(&Progress::Error { message: "failed to read install request".into() });
        return 1;
    }
    let request: InstallRequest = match serde_json::from_str(&input) {
        Ok(r) => r,
        Err(e) => {
            emit(&Progress::Error { message: format!("invalid install request: {e}") });
            return 1;
        }
    };

    let playbook: Playbook = request.into_playbook();
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
        Ok(Err(e)) => {
            emit(&Progress::Error { message: format!("install failed: {e}") });
            1
        }
        Err(_) => {
            emit(&Progress::Error { message: "install thread panicked".into() });
            1
        }
    }
}
