//! Sirius installer entry point. This plan implements only the `diag` subcommand;
//! Plan 2 adds the GTK wizard (default, no subcommand) and Plan 3 adds `--run-playbook`.

mod app;
mod config_model;
mod gui;
mod navigator;
mod pages;

use clap::{Parser, Subcommand};
use sirius_diag::config::CONFIG_PATH;
use sirius_diag::{is_blocked, run_all_checks, SiriusConfig, SystemFacts};
use std::path::Path;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "sirius", about = "LuminusOS diagnostic installer")]
struct Cli {
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
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Diag { json }) => run_diag(json),
        None => {
            gui::run();
            ExitCode::SUCCESS
        }
    }
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
