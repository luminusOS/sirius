//! Summary page: shows a recap of all the user's choices before install begins.

use super::PageOutput;
use crate::config_model::InstallConfig;
use crate::i18n::{tr, Lang};
use relm4::{adw, gtk, ComponentParts, ComponentSender, SimpleComponent};

/// Build the human-readable recap shown on the summary page.
pub fn summary_lines(lang: Lang, cfg: &InstallConfig) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("{}: {}", tr(lang, "summary.language"), cfg.locale.as_deref().unwrap_or("—")));
    lines.push(format!("{}: {}", tr(lang, "summary.keyboard"), cfg.keyboard.as_deref().unwrap_or("—")));
    lines.push(format!("{}: {}", tr(lang, "summary.timezone"), cfg.timezone.as_deref().unwrap_or("—")));
    lines.push(format!("{}: {}", tr(lang, "summary.disk"), cfg.destination_disk.as_deref().unwrap_or("—")));
    lines.push(format!(
        "{}: {}",
        tr(lang, "summary.encryption"),
        if cfg.encrypt { tr(lang, "summary.enabled") } else { tr(lang, "summary.disabled") }
    ));
    lines.push(format!("{}: {} ({})", tr(lang, "summary.user"), cfg.user.full_name, cfg.user.username));
    lines.push(format!("{}: {}", tr(lang, "summary.hostname"), cfg.user.hostname));
    lines
}

pub struct SummaryPage {
    lang: Lang,
    text: String,
    last_cfg: Option<InstallConfig>,
}

impl Default for SummaryPage {
    fn default() -> Self {
        SummaryPage {
            lang: Lang::En,
            text: String::new(),
            last_cfg: None,
        }
    }
}

#[derive(Debug)]
pub enum SummaryMsg {
    Show(InstallConfig),
    SetLang(Lang),
}

#[relm4::component(pub)]
impl SimpleComponent for SummaryPage {
    type Init = ();
    type Input = SummaryMsg;
    type Output = PageOutput;

    view! {
        adw::StatusPage {
            #[watch]
            set_title: crate::i18n::tr(model.lang, "summary.title"),
            #[watch]
            set_description: Some(crate::i18n::tr(model.lang, "summary.desc")),
            #[wrap(Some)]
            set_child = &gtk::Label {
                set_justify: gtk::Justification::Left,
                #[watch]
                set_label: &model.text,
            },
        }
    }

    fn init(_i: Self::Init, root: Self::Root, _sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = SummaryPage::default();
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            SummaryMsg::Show(cfg) => {
                self.text = summary_lines(self.lang, &cfg).join("\n");
                self.last_cfg = Some(cfg);
                sender.output(PageOutput::CanProceed(true)).ok();
            }
            SummaryMsg::SetLang(l) => {
                self.lang = l;
                if let Some(cfg) = &self.last_cfg {
                    self.text = summary_lines(self.lang, cfg).join("\n");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_lists_disk_and_user() {
        let mut cfg = InstallConfig::default();
        cfg.destination_disk = Some("/dev/sda".into());
        cfg.user.full_name = "Ada".into();
        cfg.user.username = "ada".into();
        let text = summary_lines(crate::i18n::Lang::En, &cfg).join("\n");
        assert!(text.contains("/dev/sda"));
        assert!(text.contains("ada"));
    }
}
