//! Backend boundary between the Sirius UI and `libreadymade`.
//!
//! This module isolates the rest of the installer from the readymade execution
//! API so that UI code never depends directly on upstream types.
//!
//! # Pinned upstream
//!
//! `libreadymade` is pinned to FyraLabs/readymade `main` HEAD
//! `ccdf092314b2241ec13ede2381d8174b051d5d09` (confirmed 2026-06-09).
//!
//! At that HEAD the sibling crate `filesystem-table` (v0.1.2) does not build:
//! a bad clippy autofix at `crates/filesystem-table/lib.rs:306` compared a
//! `PathBuf` (`dev.fullname`) against a `String` (`device_spec_og`). We override
//! it with a vendored, fixed copy at `vendor/filesystem-table` via
//! `[patch."https://github.com/FyraLabs/readymade.git"]` in the workspace root
//! `Cargo.toml`. The fix restores `dev.fullname == PathBuf::from(&device_spec_og)`.
//!
//! # Confirmed `libreadymade` execution API (at the pinned SHA)
//!
//! - `playbook::Playbook` — plain serde struct:
//!   - `destination_disk: PathBuf`
//!   - `encryption: Option<EncryptionConfig { tpm: bool, encryption_key: String }>`
//!   - `disk_provisioner: backend::provisioners::DiskProvisioner`
//!   - `filesystem_provisioner: Option<backend::provisioners::FileSystemProvisioner>`
//!   - `postinstall: Vec<backend::postinstall::Module>`
//! - `Playbook::channel() -> (mpsc::Sender<PlaybookProgress>, mpsc::Receiver<PlaybookProgress>)`
//! - `Playbook::play(&self, mpsc::Sender<PlaybookProgress>) -> color_eyre::Result<()>`
//!   (the crate's `Result` alias is `color_eyre::Result`, via its prelude)
//! - `playbook::PlaybookProgress`:
//!   - `Stage(String)`
//!   - `StageProgress(String)`
//!   - `PostModule(String, usize, usize)`
//!
//! No deviation from the documented API was observed. Note: `color_eyre` is not
//! a direct dependency here, so the anchor below references the API items by name
//! rather than spelling out the `color_eyre::Result<()>` return type.

pub mod distro;

/// Progress reported to the UI, decoupled from libreadymade's `PlaybookProgress`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Progress {
    Step { fraction: f64, message: String },
    Finished,
    Error { message: String },
}

#[allow(dead_code)]
fn _api_anchor() {
    // `color_eyre` is not a direct dependency of this crate, so the return type
    // of `Playbook::play` (`color_eyre::Result<()>`) cannot be named here. We
    // anchor the items by reference instead, which still forces libreadymade to
    // link and breaks the build if these names/paths ever change upstream.
    use libreadymade::playbook::{Playbook, PlaybookProgress};
    let _ = Playbook::channel;
    let _ = Playbook::play;
    let _ = |p: PlaybookProgress| match p {
        PlaybookProgress::Stage(_) => {}
        PlaybookProgress::StageProgress(_) => {}
        PlaybookProgress::PostModule(_, _, _) => {}
    };
}
