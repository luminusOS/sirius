//! Sirius installer entry point: diagnostics, dry-run and the GTK assistant.

mod app;
mod backend;
mod config_model;
mod gui;
mod logging;
mod navigator;
mod pages;
mod style;

use clap::{Parser, Subcommand};
use sirius_diag::config::CONFIG_PATH;
use sirius_diag::{SiriusConfig, SystemFacts, is_blocked, run_all_checks_with_config};
use std::path::Path;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "sirius", about = "Distro-agnostic diagnostic installer")]
struct Cli {
    /// Build and print the install config from defaults, then exit without installing.
    #[arg(long)]
    dry_run: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run hardware compatibility checks and print the report.
    Diag {
        /// Emit JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    /// Internal: execute an install request from stdin (run under pkexec). Not for direct use.
    #[command(hide = true)]
    RunPlaybook,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    // Handle the privileged subprocess entry point before any logging or GUI
    // setup — but AFTER gettext, so runner progress/error lines resolve
    // against the same catalogs (LANGUAGE is pinned from the request there).
    if matches!(cli.command, Some(Command::RunPlaybook)) {
        init_gettext();
        return ExitCode::from(backend::runner::run() as u8);
    }
    let _log = logging::init();
    if cli.dry_run {
        let cfg = sirius_installer_dry_run();
        println!("{}", serde_json::to_string_pretty(&cfg).unwrap());
        return ExitCode::SUCCESS;
    }
    match cli.command {
        Some(Command::Diag { json }) => {
            init_gettext();
            run_diag(json)
        }
        Some(Command::RunPlaybook) => unreachable!("handled above"),
        None => {
            init_gettext();
            gui::run();
            ExitCode::SUCCESS
        }
    }
}

/// Set up GNU gettext before any window exists: locale from the environment,
/// UTF-8 catalogs, and the `sirius` textdomain. Runtime language switches are
/// done later via the `LANGUAGE` variable (see `app::state`).
fn init_gettext() {
    use gettextrs::{
        LocaleCategory, bind_textdomain_codeset, bindtextdomain, setlocale, textdomain,
    };
    setlocale(LocaleCategory::LcAll, "");
    bind_textdomain_codeset("sirius", "UTF-8").expect("UTF-8 codeset must be settable");
    // Installed systems ship the catalogs under /usr/share/locale; dev runs
    // use the build-script-compiled ones from the target directory instead.
    let dir = if Path::new("/usr/share/locale/pt_BR/LC_MESSAGES/sirius.mo").exists() {
        "/usr/share/locale"
    } else {
        env!("SIRIUS_DEV_LOCALEDIR")
    };
    bindtextdomain("sirius", dir).expect("sirius textdomain must bind");
    textdomain("sirius").expect("sirius textdomain must be selected");
}

fn sirius_installer_dry_run() -> serde_json::Value {
    use config_model::{InstallConfig, InstallType, UserAccount};
    let cfg = InstallConfig {
        locale: Some("en_US".into()),
        keyboard: Some("us".into()),
        timezone: Some("UTC".into()),
        destination_disk: Some("/dev/sda".into()),
        destination_disk_name: Some("Demo Disk".into()),
        install_type: Some(InstallType::WholeDisk),
        partition_plan: None,
        encrypt: false,
        tpm: false,
        encryption_passphrase: String::new(),
        encryption_passphrase_confirm: String::new(),
        user: UserAccount {
            full_name: "Demo User".into(),
            username: "demo".into(),
            password: "demopassword".into(),
            password_confirm: "demopassword".into(),
            hostname: "localhost".into(),
        },
    };
    let distro = backend::distro::DistroDescriptor::from_toml(
        &std::fs::read_to_string("data/distro.toml").unwrap_or_default(),
    )
    .expect("data/distro.toml must parse");
    let req = backend::adapter::build_request(&cfg).expect("dry-run config must be valid");
    serde_json::json!({ "request": req, "distro": distro })
}

fn run_diag(json: bool) -> ExitCode {
    let (cfg, warning) = SiriusConfig::load_or_default(Path::new(CONFIG_PATH));
    if let Some(w) = warning {
        eprintln!("warning: {w}");
    }
    let facts = SystemFacts::gather();
    let checks = run_all_checks_with_config(&facts, &cfg.diagnostics);
    let blocked = is_blocked(&checks, &cfg.diagnostics.require);

    if json {
        println!("{}", serde_json::to_string_pretty(&checks).unwrap());
    } else {
        for c in &checks {
            let mark = match c.status {
                sirius_diag::Status::Pass => "PASS",
                sirius_diag::Status::Warn => "WARN",
                sirius_diag::Status::Fail => "FAIL",
            };
            println!("[{mark}] {} — {}", c.label, c.detail);
        }
        if blocked {
            println!("\nInstall blocked: a required check failed.");
        } else {
            println!("\nSystem is compatible.");
        }
    }

    if blocked {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
