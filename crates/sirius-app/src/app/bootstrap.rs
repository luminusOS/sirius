//! Load and resolve everything needed to construct the wizard.

use super::pages::IMPLEMENTED_PAGES;
use crate::pages::diagnostics::DiagnosticsInit;
use sirius_core::{Bento, Branding};
use sirius_diag::config::CONFIG_PATH;
use sirius_diag::{SiriusConfig, SystemFacts, is_blocked, run_all_checks_with_config};
use std::path::Path;

pub(super) struct Bootstrap {
    pub page_ids: Vec<String>,
    pub diagnostics: DiagnosticsInit,
    pub diagnostics_blocked: bool,
    pub uefi: bool,
    pub bentos: Vec<Bento>,
    pub branding: Branding,
    pub terminal: sirius_diag::config::TerminalConfig,
}

pub(super) fn load() -> Bootstrap {
    let (config, warning) = SiriusConfig::load_or_default(Path::new(CONFIG_PATH));
    if let Some(warning) = warning {
        tracing::warn!("{warning}");
    }

    let has_wifi = sirius_backend::network::has_wifi_device();
    let page_ids = config
        .pages
        .resolve()
        .into_iter()
        .filter(|page| IMPLEMENTED_PAGES.contains(&page.as_str()))
        .filter(|page| page != "network" || has_wifi)
        .collect();

    let facts = SystemFacts::gather();
    let checks = run_all_checks_with_config(&facts, &config.diagnostics);
    let diagnostics_blocked = is_blocked(&checks, &config.diagnostics.require);

    // Branding and link cards are optional. Their absence keeps the generic
    // Sirius icon and an empty progress-card area.
    let (bentos, branding) = sirius_backend::distro::load()
        .map(|descriptor| (descriptor.bentos, descriptor.branding))
        .unwrap_or_default();

    Bootstrap {
        page_ids,
        diagnostics: DiagnosticsInit {
            config: config.diagnostics,
        },
        diagnostics_blocked,
        uefi: Path::new("/sys/firmware/efi").exists(),
        bentos,
        branding,
        terminal: config.terminal,
    }
}
