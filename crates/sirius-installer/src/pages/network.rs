//! Optional Wi-Fi selection and connection page backed by NetworkManager.

use super::PageOutput;
use crate::backend::network::{WifiNetwork, WifiSecurity, connect_wifi, scan_wifi};
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, adw, gtk};

pub struct NetworkPage {
    root: adw::StatusPage,
    networks: Vec<WifiNetwork>,
    loading: bool,
    connecting: Option<usize>,
    error: Option<String>,
}

#[derive(Debug)]
pub enum NetworkMsg {
    Refresh,
    Loaded(Result<Vec<WifiNetwork>, String>),
    Select(usize),
    Connect { index: usize, password: String },
    Connected(Result<(), String>),
    Retranslate,
}

pub struct NetworkPageWidgets {
    root: adw::StatusPage,
}

impl SimpleComponent for NetworkPage {
    type Init = ();
    type Input = NetworkMsg;
    type Output = PageOutput;
    type Root = adw::StatusPage;
    type Widgets = NetworkPageWidgets;

    fn init_root() -> Self::Root {
        adw::StatusPage::new()
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = NetworkPage {
            root: root.clone(),
            networks: Vec::new(),
            loading: true,
            connecting: None,
            error: None,
        };
        root.set_icon_name(Some("network-wireless-symbolic"));
        apply_header(&root);
        scan_in_background(sender.clone());
        ComponentParts {
            model,
            widgets: NetworkPageWidgets { root },
        }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            NetworkMsg::Refresh => {
                self.loading = true;
                self.error = None;
                scan_in_background(sender);
            }
            NetworkMsg::Loaded(result) => {
                self.loading = false;
                match result {
                    Ok(networks) => self.networks = networks,
                    Err(error) => self.error = Some(error),
                }
            }
            NetworkMsg::Select(index) => {
                let Some(network) = self.networks.get(index).cloned() else {
                    return;
                };
                match network.security {
                    WifiSecurity::Open => sender.input(NetworkMsg::Connect {
                        index,
                        password: String::new(),
                    }),
                    WifiSecurity::WpaPersonal | WifiSecurity::Wpa3Personal => {
                        show_password_dialog(&self.root, index, &network.ssid, &sender)
                    }
                    WifiSecurity::Unsupported => {
                        self.error = Some(security_label(network.security))
                    }
                }
            }
            NetworkMsg::Connect { index, password } => {
                let Some(network) = self.networks.get(index).cloned() else {
                    return;
                };
                self.connecting = Some(index);
                self.error = None;
                let s = sender.clone();
                std::thread::spawn(move || {
                    s.input(NetworkMsg::Connected(connect_wifi(&network, &password)));
                });
            }
            NetworkMsg::Connected(result) => {
                self.connecting = None;
                match result {
                    Ok(()) => {
                        self.loading = true;
                        scan_in_background(sender);
                    }
                    Err(error) => self.error = Some(error),
                }
            }
            NetworkMsg::Retranslate => {}
        }
    }

    fn update_view(&self, widgets: &mut Self::Widgets, sender: ComponentSender<Self>) {
        apply_header(&widgets.root);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 14);
        content.set_width_request(650);
        let group = adw::PreferencesGroup::new();
        group.set_title(&gettext("Available Wi-Fi networks"));
        if self.loading {
            let row = adw::ActionRow::new();
            row.set_title(&gettext("Looking for networks…"));
            let spinner = gtk::Spinner::new();
            spinner.start();
            row.add_suffix(&spinner);
            group.add(&row);
        }
        for (index, network) in self.networks.iter().enumerate() {
            let row = adw::ActionRow::new();
            row.set_title(&network.ssid);
            row.set_subtitle(&security_label(network.security));
            row.add_prefix(&gtk::Image::from_icon_name(signal_icon(network.strength)));
            if network.active {
                let connected = gtk::Label::new(Some(&gettext("Connected")));
                connected.add_css_class("accent");
                row.add_suffix(&connected);
            } else if self.connecting == Some(index) {
                let spinner = gtk::Spinner::new();
                spinner.start();
                row.add_suffix(&spinner);
            } else {
                let button = gtk::Button::with_label(&gettext("Connect"));
                button.set_sensitive(
                    network.security != WifiSecurity::Unsupported && self.connecting.is_none(),
                );
                let s = sender.clone();
                button.connect_clicked(move |_| s.input(NetworkMsg::Select(index)));
                row.add_suffix(&button);
                row.set_activatable_widget(Some(&button));
            }
            group.add(&row);
        }
        content.append(&group);
        if let Some(error) = &self.error {
            let label = gtk::Label::new(Some(error));
            label.add_css_class("error");
            label.set_wrap(true);
            content.append(&label);
        }
        let refresh = gtk::Button::with_label(&gettext("Scan again"));
        refresh.set_halign(gtk::Align::Center);
        refresh.set_sensitive(!self.loading && self.connecting.is_none());
        refresh.connect_clicked(move |_| sender.input(NetworkMsg::Refresh));
        content.append(&refresh);
        widgets.root.set_child(Some(&content));
    }
}

fn scan_in_background(sender: ComponentSender<NetworkPage>) {
    std::thread::spawn(move || sender.input(NetworkMsg::Loaded(scan_wifi())));
}

fn apply_header(root: &adw::StatusPage) {
    super::status_header(
        root,
        &gettext("Network"),
        &gettext("Connect to a network. A connection is optional but recommended."),
    );
}

fn show_password_dialog(
    parent: &adw::StatusPage,
    index: usize,
    ssid: &str,
    sender: &ComponentSender<NetworkPage>,
) {
    let dialog = adw::AlertDialog::builder()
        .heading(gettext("Connect to {ssid}").replace("{ssid}", ssid))
        .body(gettext("Enter the Wi-Fi password."))
        .build();
    let password = gtk::PasswordEntry::new();
    password.set_show_peek_icon(true);
    password.set_activates_default(true);
    dialog.set_extra_child(Some(&password));
    dialog.add_response("cancel", &gettext("Cancel"));
    dialog.add_response("connect", &gettext("Connect"));
    dialog.set_response_appearance("connect", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("connect"));
    dialog.set_close_response("cancel");
    let s = sender.clone();
    dialog.connect_response(Some("connect"), move |_, _| {
        s.input(NetworkMsg::Connect {
            index,
            password: password.text().to_string(),
        });
    });
    dialog.present(Some(parent));
}

fn signal_icon(strength: u8) -> &'static str {
    match strength {
        75..=u8::MAX => "network-wireless-signal-excellent-symbolic",
        50..=74 => "network-wireless-signal-good-symbolic",
        25..=49 => "network-wireless-signal-ok-symbolic",
        _ => "network-wireless-signal-weak-symbolic",
    }
}

fn security_label(security: WifiSecurity) -> String {
    match security {
        WifiSecurity::Open => gettext("Open network"),
        WifiSecurity::WpaPersonal => gettext("WPA/WPA2 Personal"),
        WifiSecurity::Wpa3Personal => gettext("WPA3 Personal"),
        WifiSecurity::Unsupported => gettext("Enterprise or legacy security is not supported here"),
    }
}
