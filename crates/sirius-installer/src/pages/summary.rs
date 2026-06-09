//! Summary page: shows a recap of all the user's choices before install begins.

use super::PageOutput;
use crate::config_model::InstallConfig;
use relm4::{adw, gtk, ComponentParts, ComponentSender, SimpleComponent};

/// Build the human-readable recap shown on the summary page.
pub fn summary_lines(cfg: &InstallConfig) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("Language: {}", cfg.locale.as_deref().unwrap_or("—")));
    lines.push(format!("Keyboard: {}", cfg.keyboard.as_deref().unwrap_or("—")));
    lines.push(format!("Time zone: {}", cfg.timezone.as_deref().unwrap_or("—")));
    lines.push(format!("Disk: {}", cfg.destination_disk.as_deref().unwrap_or("—")));
    lines.push(format!(
        "Encryption: {}",
        if cfg.encrypt { "enabled" } else { "disabled" }
    ));
    lines.push(format!("User: {} ({})", cfg.user.full_name, cfg.user.username));
    lines.push(format!("Hostname: {}", cfg.user.hostname));
    lines
}

#[derive(Default)]
pub struct SummaryPage {
    text: String,
}

#[derive(Debug)]
pub enum SummaryMsg {
    Show(InstallConfig),
}

#[relm4::component(pub)]
impl SimpleComponent for SummaryPage {
    type Init = ();
    type Input = SummaryMsg;
    type Output = PageOutput;

    view! {
        adw::StatusPage {
            set_title: "Ready to install",
            set_description: Some("Review your choices. The disk will be erased."),
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
                self.text = summary_lines(&cfg).join("\n");
                sender.output(PageOutput::CanProceed(true)).ok();
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
        let text = summary_lines(&cfg).join("\n");
        assert!(text.contains("/dev/sda"));
        assert!(text.contains("ada"));
    }
}
