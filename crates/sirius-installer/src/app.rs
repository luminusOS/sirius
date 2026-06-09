//! Root wizard component. Owns the window, the navigation stack, and InstallConfig.

use relm4::adw::prelude::*;
use relm4::prelude::*;

pub struct AppModel;

#[derive(Debug)]
pub enum AppMsg {}

#[relm4::component(pub)]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
        adw::ApplicationWindow {
            set_title: Some("Sirius"),
            set_default_width: 720,
            set_default_height: 540,

            #[wrap(Some)]
            set_content = &adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {},
                #[wrap(Some)]
                set_content = &gtk::Label {
                    set_label: "Sirius wizard — pages land in later tasks",
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = AppModel;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {}
    }
}
