//! Diagnostics compatibility-gate page.
//!
//! Gathers hardware facts, runs all checks, and reports `PageOutput::CanProceed`
//! based on whether any required check has `Status::Fail`.

use super::PageOutput;
use relm4::adw::prelude::*;
use relm4::{adw, gtk, ComponentParts, ComponentSender, SimpleComponent};
use sirius_diag::{is_blocked, run_all_checks, Check, Status, SystemFacts};

/// Init data: the list of check ids that hard-gate the install.
pub struct DiagnosticsInit {
    pub require: Vec<String>,
}

pub struct DiagnosticsPage {
    checks: Vec<Check>,
    blocked: bool,
}

#[derive(Debug)]
pub enum DiagnosticsMsg {}

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
        root.set_title("System compatibility");
        root.set_description(Some("Sirius checked your hardware before installing."));

        let group = adw::PreferencesGroup::new();

        let facts = SystemFacts::gather();
        let checks = run_all_checks(&facts);
        let blocked = is_blocked(&checks, &init.require);

        for c in &checks {
            let row = adw::ActionRow::new();
            row.set_title(&c.label);
            row.set_subtitle(&c.detail);
            let icon = match c.status {
                Status::Pass => "emblem-ok-symbolic",
                Status::Warn => "dialog-warning-symbolic",
                Status::Fail => "dialog-error-symbolic",
            };
            let img = gtk::Image::from_icon_name(icon);
            row.add_suffix(&img);
            group.add(&row);
        }

        root.set_child(Some(&group));

        let model = DiagnosticsPage { checks, blocked };
        let widgets = DiagnosticsPageWidgets { root };

        ComponentParts { model, widgets }
    }

    fn update(&mut self, _msg: Self::Input, _sender: ComponentSender<Self>) {
        match _msg {}
    }

    fn update_view(&self, _widgets: &mut Self::Widgets, _sender: ComponentSender<Self>) {}
}
