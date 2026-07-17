//! Summary page: shows a recap of all the user's choices before install begins,
//! as a boxed list (manual `SimpleComponent` — rows are rebuilt imperatively).

use super::PageOutput;
use crate::config_model::InstallConfig;
use crate::i18n::{tr, Lang};
use relm4::adw::prelude::*;
use relm4::{adw, ComponentParts, ComponentSender, SimpleComponent};

/// Build the (label, value) pairs shown on the summary page. Rows belonging
/// to a wizard page that is disabled in `sirius.toml` are omitted — the recap
/// must only show what the user could actually choose.
pub fn summary_rows(lang: Lang, cfg: &InstallConfig, pages: &[String]) -> Vec<(String, String)> {
    let on = |page: &str| pages.iter().any(|p| p == page);
    let dash = "—".to_string();
    let mut rows = Vec::new();
    if on("welcome") {
        rows.push((
            tr(lang, "summary.language").into(),
            cfg.locale.clone().unwrap_or_else(|| dash.clone()),
        ));
    }
    if on("keyboard") {
        rows.push((
            tr(lang, "summary.keyboard").into(),
            cfg.keyboard.clone().unwrap_or_else(|| dash.clone()),
        ));
    }
    if on("timezone") {
        rows.push((
            tr(lang, "summary.timezone").into(),
            cfg.timezone.clone().unwrap_or_else(|| dash.clone()),
        ));
    }
    if on("storage") {
        rows.push((
            tr(lang, "summary.disk").into(),
            cfg.destination_disk_name
                .clone()
                .or_else(|| cfg.destination_disk.clone())
                .unwrap_or_else(|| dash.clone()),
        ));
        rows.push((
            tr(lang, "summary.encryption").into(),
            tr(
                lang,
                if matches!(
                    cfg.install_type,
                    Some(crate::config_model::InstallType::Manual)
                ) {
                    "summary.manual"
                } else if cfg.encrypt {
                    "summary.enabled"
                } else {
                    "summary.disabled"
                },
            )
            .into(),
        ));
    }
    if on("user") && !cfg.user.is_empty() {
        rows.push((
            tr(lang, "summary.user").into(),
            format!("{} ({})", cfg.user.full_name, cfg.user.username),
        ));
        rows.push((
            tr(lang, "summary.hostname").into(),
            cfg.user.hostname.clone(),
        ));
    }
    rows
}

pub struct SummaryPage {
    lang: Lang,
    pages: Vec<String>,
    last_cfg: Option<InstallConfig>,
}

#[derive(Debug)]
pub enum SummaryMsg {
    Show(Box<InstallConfig>),
    SetLang(Lang),
}

pub struct SummaryPageWidgets {
    root: adw::StatusPage,
}

impl SimpleComponent for SummaryPage {
    /// The resolved list of enabled wizard page ids.
    type Init = Vec<String>;
    type Input = SummaryMsg;
    type Output = PageOutput;
    type Root = adw::StatusPage;
    type Widgets = SummaryPageWidgets;

    fn init_root() -> Self::Root {
        adw::StatusPage::new()
    }

    fn init(
        pages: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = SummaryPage {
            lang: Lang::En,
            pages,
            last_cfg: None,
        };
        root.set_title(tr(model.lang, "summary.title"));
        root.set_description(Some(tr(model.lang, "summary.desc")));
        let widgets = SummaryPageWidgets { root };
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            SummaryMsg::Show(cfg) => {
                self.last_cfg = Some(*cfg);
            }
            SummaryMsg::SetLang(l) => self.lang = l,
        }
    }

    fn update_view(&self, widgets: &mut Self::Widgets, sender: ComponentSender<Self>) {
        widgets.root.set_title(tr(self.lang, "summary.title"));
        widgets
            .root
            .set_description(Some(tr(self.lang, "summary.desc")));
        if let Some(cfg) = &self.last_cfg {
            let content = relm4::gtk::Box::new(relm4::gtk::Orientation::Vertical, 24);
            content.set_width_request(680);
            let group = adw::PreferencesGroup::new();
            for (label, value) in summary_rows(self.lang, cfg, &self.pages) {
                let row = adw::ActionRow::new();
                row.set_title(&label);
                let value_label = relm4::gtk::Label::builder()
                    .label(&value)
                    .css_classes(["dim-label"])
                    .build();
                row.add_suffix(&value_label);
                group.add(&row);
            }
            content.append(&group);
            let install = relm4::gtk::Button::with_label(tr(self.lang, "nav.install"));
            install.add_css_class("suggested-action");
            install.add_css_class("install-pill");
            install.set_halign(relm4::gtk::Align::Center);
            let s = sender.clone();
            install.connect_clicked(move |_| {
                s.output(PageOutput::RequestInstall).ok();
            });
            content.append(&install);
            widgets.root.set_child(Some(&content));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_pages() -> Vec<String> {
        ["welcome", "keyboard", "timezone", "storage", "user"]
            .map(String::from)
            .to_vec()
    }

    #[test]
    fn summary_lists_disk_and_user() {
        let cfg = InstallConfig {
            destination_disk: Some("/dev/sda".into()),
            user: crate::config_model::UserAccount {
                full_name: "Ada".into(),
                username: "ada".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let rows = summary_rows(crate::i18n::Lang::En, &cfg, &all_pages());
        let text = rows
            .iter()
            .map(|(l, v)| format!("{l}: {v}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("/dev/sda"));
        assert!(text.contains("Ada (ada)"));
        assert!(text.contains("Hostname"));
    }

    #[test]
    fn summary_skips_user_rows_when_empty() {
        let cfg = InstallConfig::default();
        let rows = summary_rows(crate::i18n::Lang::En, &cfg, &all_pages());
        assert_eq!(rows.len(), 5);
        assert!(!rows.iter().any(|(l, _)| l == "User"));
    }

    #[test]
    fn summary_omits_rows_for_disabled_pages() {
        // Mirror of a live sirius.toml with keyboard/timezone/user disabled.
        let pages: Vec<String> = ["welcome", "diagnostics", "network", "storage"]
            .map(String::from)
            .to_vec();
        let cfg = InstallConfig {
            destination_disk: Some("/dev/sda".into()),
            ..Default::default()
        };
        let labels: Vec<String> = summary_rows(crate::i18n::Lang::En, &cfg, &pages)
            .into_iter()
            .map(|(l, _)| l)
            .collect();
        assert_eq!(labels, ["Language", "Disk", "Encryption"]);
    }
}
