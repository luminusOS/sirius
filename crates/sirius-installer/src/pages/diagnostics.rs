//! Diagnostics compatibility-gate page.
//!
//! Gathers hardware facts and renders the compatibility report. The pure
//! wizard state owns navigation gating.

use super::PageOutput;
use relm4::adw::prelude::*;
use relm4::{adw, gtk, ComponentParts, ComponentSender, SimpleComponent};
use sirius_diag::config::DiagnosticsConfig;
use sirius_diag::{run_all_checks_with_config, Status, SystemFacts};

/// Init data: diagnostics thresholds and the check ids that hard-gate the install.
pub struct DiagnosticsInit {
    pub config: DiagnosticsConfig,
}

pub struct DiagnosticsPage {
    lang: crate::i18n::Lang,
}

#[derive(Debug)]
pub enum DiagnosticsMsg {
    SetLang(crate::i18n::Lang),
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
        let lang = crate::i18n::Lang::En;
        root.set_title(crate::i18n::tr(lang, "diagnostics.title"));
        root.set_description(Some(crate::i18n::tr(lang, "diagnostics.desc")));

        let group = adw::PreferencesGroup::new();

        // The root computes gating once; this page only renders the report.
        let facts = SystemFacts::gather();
        let checks = run_all_checks_with_config(&facts, &init.config);

        for c in &checks {
            let row = adw::ActionRow::new();
            row.set_title(&c.label);
            row.set_subtitle(&c.detail);
            let (icon, css) = match c.status {
                Status::Pass => ("object-select-symbolic", "success"),
                Status::Warn => ("dialog-warning-symbolic", "warning"),
                Status::Fail => ("dialog-error-symbolic", "error"),
            };
            let img = gtk::Image::from_icon_name(icon);
            img.add_css_class(css);
            row.add_suffix(&img);
            group.add(&row);
        }

        root.set_child(Some(&group));

        let model = DiagnosticsPage { lang };
        let widgets = DiagnosticsPageWidgets { root };

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            DiagnosticsMsg::SetLang(l) => self.lang = l,
        }
    }

    fn update_view(&self, widgets: &mut Self::Widgets, _sender: ComponentSender<Self>) {
        widgets
            .root
            .set_title(crate::i18n::tr(self.lang, "diagnostics.title"));
        widgets
            .root
            .set_description(Some(crate::i18n::tr(self.lang, "diagnostics.desc")));
    }
}
