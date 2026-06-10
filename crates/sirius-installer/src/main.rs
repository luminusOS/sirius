//! Sirius installer entry point. This plan implements only the `diag` subcommand;
//! Plan 2 adds the GTK wizard (default, no subcommand) and Plan 3 adds `--run-playbook`.

mod app;
mod backend;
mod config_model;
mod gui;
mod i18n;
mod logging;
mod navigator;
mod pages;

use clap::{Parser, Subcommand};
use sirius_diag::config::CONFIG_PATH;
use sirius_diag::{is_blocked, run_all_checks, SiriusConfig, SystemFacts};
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
    // Handle the privileged subprocess entry point before any logging or GUI setup.
    if matches!(cli.command, Some(Command::RunPlaybook)) {
        return ExitCode::from(backend::runner::run() as u8);
    }
    let _log = logging::init();
    if cli.dry_run {
        let cfg = sirius_installer_dry_run();
        println!("{}", serde_json::to_string_pretty(&cfg).unwrap());
        return ExitCode::SUCCESS;
    }
    match cli.command {
        Some(Command::Diag { json }) => run_diag(json),
        Some(Command::RunPlaybook) => unreachable!("handled above"),
        None => {
            gui::run();
            ExitCode::SUCCESS
        }
    }
}

fn sirius_installer_dry_run() -> serde_json::Value {
    use config_model::{InstallConfig, InstallType, UserAccount};
    let cfg = InstallConfig {
        locale: Some("en_US".into()),
        keyboard: Some("us".into()),
        timezone: Some("UTC".into()),
        destination_disk: Some("/dev/sda".into()),
        install_type: Some(InstallType::WholeDisk),
        encrypt: false,
        tpm: false,
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
    let req = backend::adapter::build_request(&cfg, &distro).expect("dry-run config must be valid");
    serde_json::to_value(req).unwrap()
}

fn run_diag(json: bool) -> ExitCode {
    let (cfg, warning) = SiriusConfig::load_or_default(Path::new(CONFIG_PATH));
    if let Some(w) = warning {
        eprintln!("warning: {w}");
    }
    let facts = SystemFacts::gather();
    let checks = run_all_checks(&facts);
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
