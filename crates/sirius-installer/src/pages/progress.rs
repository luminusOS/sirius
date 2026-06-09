use super::PageOutput;
use relm4::adw::prelude::*;
use relm4::{adw, gtk, ComponentParts, ComponentSender, SimpleComponent};

pub struct ProgressPage {
    fraction: f64,
}

#[derive(Debug)]
pub enum ProgressMsg {
    /// Plan 3 sends these from the privileged runner's progress stream.
    Update { fraction: f64, line: String },
    Done,
}

#[relm4::component(pub)]
impl SimpleComponent for ProgressPage {
    type Init = ();
    type Input = ProgressMsg;
    type Output = PageOutput;

    view! {
        adw::StatusPage {
            set_title: "Installing LuminusOS",
            #[wrap(Some)]
            set_child = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 12,
                gtk::ProgressBar {
                    #[watch]
                    set_fraction: model.fraction,
                },
                gtk::ScrolledWindow {
                    set_vexpand: true,
                    set_min_content_height: 160,
                    gtk::TextView {
                        set_editable: false,
                        set_monospace: true,
                    },
                },
            },
        }
    }

    fn init(_i: Self::Init, root: Self::Root, _sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = ProgressPage { fraction: 0.0 };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            ProgressMsg::Update { fraction, line } => {
                self.fraction = fraction;
                tracing::info!("{line}");
            }
            ProgressMsg::Done => {
                self.fraction = 1.0;
                sender.output(PageOutput::RequestNext).ok();
            }
        }
    }
}
