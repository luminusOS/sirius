//! Diagnostics compatibility-gate page.
//!
//! Gathers hardware facts and renders the compatibility report. The pure
//! wizard state owns navigation gating.

use super::PageOutput;
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, adw, gtk};
use sirius_diag::config::DiagnosticsConfig;
use sirius_diag::{Status, SystemFacts, run_all_checks_with_config};

/// Init data: diagnostics thresholds and the check ids that hard-gate the install.
pub struct DiagnosticsInit {
    pub config: DiagnosticsConfig,
}

pub struct DiagnosticsPage {
    config: DiagnosticsConfig,
}

#[derive(Debug)]
pub enum DiagnosticsMsg {
    /// The UI language changed; gettext resolves strings at render time, so a
    /// bare re-render (Relm4 runs update_view after update) is enough.
    Retranslate,
}

pub struct DiagnosticsPageWidgets {
    root: adw::StatusPage,
}

impl SimpleComponent for DiagnosticsPage {
    type Init = DiagnosticsInit;
    type Input = DiagnosticsMsg;
    type Output = PageOutput;
    type Root = adw::StatusPage;
    type Widgets = DiagnosticsPageWidgets;

    fn init_root() -> Self::Root {
        adw::StatusPage::new()
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        apply_header(&root);

        let model = DiagnosticsPage {
            config: init.config,
        };
        let widgets = DiagnosticsPageWidgets { root };

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            DiagnosticsMsg::Retranslate => {}
        }
    }

    fn update_view(&self, widgets: &mut Self::Widgets, _sender: ComponentSender<Self>) {
        apply_header(&widgets.root);

        // Check labels and details are translated when the report is built.
        // Rebuild it after a runtime language change instead of retaining the
        // English strings gathered before the language page was shown.
        let group = adw::PreferencesGroup::new();
        let facts = SystemFacts::gather();
        let checks = run_all_checks_with_config(&facts, &self.config);
        for check in &checks {
            let row = adw::ActionRow::new();
            row.set_title(&check.label);
            row.set_subtitle(&check.detail);
            let (icon, css) = match check.status {
                Status::Pass => ("object-select-symbolic", "success"),
                Status::Warn => ("dialog-warning-symbolic", "warning"),
                Status::Fail => ("dialog-error-symbolic", "error"),
            };
            let image = gtk::Image::from_icon_name(icon);
            image.add_css_class(css);
            row.add_suffix(&image);
            group.add(&row);
        }
        widgets.root.set_child(Some(&group));
    }
}

fn apply_header(root: &adw::StatusPage) {
    super::status_header(
        root,
        &gettext("System compatibility"),
        &gettext("Sirius checked your hardware before installing."),
    );
}
