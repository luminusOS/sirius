//! Wizard pages. Each page is a Relm4 SimpleComponent that emits `PageOutput`
//! up to AppModel, which folds the change into InstallConfig.

mod choice_list;

pub mod diagnostics;
pub mod finished;
pub mod keyboard;
pub mod language;
pub mod network;
pub mod progress;
pub mod storage;
pub mod summary;
pub mod timezone;
pub mod user;
pub mod welcome;

use relm4::adw;
use sirius_core::{InstallType, PartitionPlan, UserAccount};

/// GNOME Desktop lazily initializes its iso-codes/xkb tables without locking;
/// the locale and keyboard FFI boundaries take this lock around entry points
/// that trigger that one-time setup, so concurrent first use cannot race it.
pub(crate) static GNOME_DESKTOP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Single GTK-owning thread for interactive tests. GTK can only be
/// initialized from one thread, but the Rust harness gives every test its
/// own thread — so interactive tests hand their body to this one.
#[cfg(test)]
pub(crate) mod testutil {
    use std::sync::OnceLock;
    use std::sync::mpsc::{Sender, channel};

    static GTK_THREAD: OnceLock<Sender<Box<dyn FnOnce() + Send>>> = OnceLock::new();

    fn spawn() -> Sender<Box<dyn FnOnce() + Send>> {
        let (sender, receiver) = channel::<Box<dyn FnOnce() + Send>>();
        std::thread::spawn(move || {
            relm4::gtk::init().expect("gtk init");
            relm4::adw::init().expect("adw init");
            while let Ok(job) = receiver.recv() {
                job();
            }
        });
        sender
    }

    /// Run `test` on the thread that owns GTK for this test binary, blocking
    /// until it finishes. Panics inside the body propagate to the caller.
    pub(crate) fn run_on_gtk_thread(test: impl FnOnce() + Send + 'static) {
        let (done_sender, done_receiver) = channel();
        GTK_THREAD
            .get_or_init(spawn)
            .send(Box::new(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(test));
                done_sender.send(result).ok();
            }))
            .expect("gtk test thread alive");
        match done_receiver.recv() {
            Ok(Ok(())) => {}
            Ok(Err(payload)) => std::panic::resume_unwind(payload),
            Err(_) => panic!("gtk test thread died"),
        }
    }
}

/// Set a status page's translated header. Pages call this both in `init` and
/// on every `update_view`: gettext resolves at call time, so re-applying on
/// the `Retranslate` nudge is what re-renders the header in the new language.
/// One place, no drift between the two call sites.
pub(crate) fn status_header(root: &adw::StatusPage, title: &str, description: &str) {
    root.set_title(title);
    root.set_description(Some(description));
}

/// Storage choices collected by the storage page.
///
/// Keeping this as one value makes the page-to-app boundary explicit and avoids
/// growing `PageOutput` every time storage gains another option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageSelection {
    pub path: String,
    pub name: String,
    pub install_type: InstallType,
    pub encrypt: bool,
    pub tpm: bool,
    /// Dedicated LUKS passphrase pair; only meaningful when `encrypt` is set.
    pub encryption_passphrase: String,
    pub encryption_passphrase_confirm: String,
    pub partition_plan: Option<PartitionPlan>,
}

/// Messages a page can send to the root AppModel.
#[derive(Debug, Clone)]
pub enum PageOutput {
    SetLocale(String),
    SetKeyboard(String),
    SetTimezone(String),
    SetStorage(StorageSelection),
    SetUser(UserAccount),
    /// Request to advance (from in-page buttons, optional).
    RequestNext,
    /// The centered summary pill requests the destructive confirmation.
    RequestInstall,
}
