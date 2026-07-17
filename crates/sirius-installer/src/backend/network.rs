//! Small NetworkManager client used by the optional Wi-Fi page.

use std::collections::HashMap;
use zbus::blocking::Connection;
use zvariant::{OwnedObjectPath, OwnedValue, Value};

const NM_DEVICE_TYPE_WIFI: u32 = 2;
const AP_FLAGS_PRIVACY: u32 = 0x1;
const SEC_KEY_MGMT_PSK: u32 = 0x100;
const SEC_KEY_MGMT_SAE: u32 = 0x400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WifiSecurity {
    Open,
    WpaPersonal,
    Wpa3Personal,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiNetwork {
    pub ssid: String,
    pub strength: u8,
    pub security: WifiSecurity,
    pub device: OwnedObjectPath,
    pub access_point: OwnedObjectPath,
    pub active: bool,
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
trait NetworkManager {
    fn get_devices(&self) -> zbus::Result<Vec<OwnedObjectPath>>;

    fn add_and_activate_connection2(
        &self,
        connection: HashMap<String, HashMap<String, OwnedValue>>,
        device: OwnedObjectPath,
        specific_object: OwnedObjectPath,
        options: HashMap<String, OwnedValue>,
    ) -> zbus::Result<(
        OwnedObjectPath,
        OwnedObjectPath,
        HashMap<String, OwnedValue>,
    )>;

    #[zbus(property)]
    fn wireless_enabled(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn set_wireless_enabled(&self, enabled: bool) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Device",
    default_service = "org.freedesktop.NetworkManager"
)]
trait NetworkDevice {
    #[zbus(property)]
    fn device_type(&self) -> zbus::Result<u32>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wireless",
    default_service = "org.freedesktop.NetworkManager"
)]
trait WirelessDevice {
    fn get_all_access_points(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
    fn request_scan(&self, options: HashMap<String, OwnedValue>) -> zbus::Result<()>;

    #[zbus(property)]
    fn active_access_point(&self) -> zbus::Result<OwnedObjectPath>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.AccessPoint",
    default_service = "org.freedesktop.NetworkManager"
)]
trait AccessPoint {
    #[zbus(property)]
    fn ssid(&self) -> zbus::Result<Vec<u8>>;
    #[zbus(property)]
    fn strength(&self) -> zbus::Result<u8>;
    #[zbus(property)]
    fn flags(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn wpa_flags(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn rsn_flags(&self) -> zbus::Result<u32>;
}

pub fn has_wifi_device() -> bool {
    wifi_devices().is_ok_and(|devices| !devices.is_empty())
}

fn wifi_devices() -> Result<Vec<(Connection, OwnedObjectPath)>, String> {
    let connection =
        Connection::system().map_err(|e| format!("cannot connect to NetworkManager: {e}"))?;
    let manager = NetworkManagerProxyBlocking::new(&connection)
        .map_err(|e| format!("cannot access NetworkManager: {e}"))?;
    let mut devices = Vec::new();
    for path in manager
        .get_devices()
        .map_err(|e| format!("cannot list network devices: {e}"))?
    {
        let device = NetworkDeviceProxyBlocking::builder(&connection)
            .path(path.clone())
            .map_err(|e| e.to_string())?
            .build()
            .map_err(|e| e.to_string())?;
        if device.device_type().unwrap_or_default() == NM_DEVICE_TYPE_WIFI {
            devices.push((connection.clone(), path));
        }
    }
    Ok(devices)
}

pub fn scan_wifi() -> Result<Vec<WifiNetwork>, String> {
    enable_wireless()?;
    let mut networks = Vec::new();
    for (connection, device_path) in wifi_devices()? {
        let wireless = WirelessDeviceProxyBlocking::builder(&connection)
            .path(device_path.clone())
            .map_err(|e| e.to_string())?
            .build()
            .map_err(|e| format!("cannot access Wi-Fi device: {e}"))?;
        let _ = wireless.request_scan(HashMap::new());
        std::thread::sleep(std::time::Duration::from_millis(800));
        let active = wireless.active_access_point().ok();
        for ap_path in wireless
            .get_all_access_points()
            .map_err(|e| format!("cannot list access points: {e}"))?
        {
            let ap = AccessPointProxyBlocking::builder(&connection)
                .path(ap_path.clone())
                .map_err(|e| e.to_string())?
                .build()
                .map_err(|e| format!("cannot read access point: {e}"))?;
            let ssid = String::from_utf8_lossy(&ap.ssid().unwrap_or_default()).to_string();
            if ssid.trim().is_empty() {
                continue;
            }
            let flags = ap.flags().unwrap_or_default();
            let wpa = ap.wpa_flags().unwrap_or_default();
            let rsn = ap.rsn_flags().unwrap_or_default();
            let security = classify_security(flags, wpa, rsn);
            networks.push(WifiNetwork {
                ssid,
                strength: ap.strength().unwrap_or_default(),
                security,
                device: device_path.clone(),
                active: active.as_ref() == Some(&ap_path),
                access_point: ap_path,
            });
        }
    }
    networks.sort_by(|a, b| b.active.cmp(&a.active).then(b.strength.cmp(&a.strength)));
    let mut strongest = HashMap::<(String, WifiSecurity), WifiNetwork>::new();
    for network in networks {
        let key = (network.ssid.clone(), network.security);
        strongest.entry(key).or_insert(network);
    }
    let mut result: Vec<_> = strongest.into_values().collect();
    result.sort_by(|a, b| b.active.cmp(&a.active).then(b.strength.cmp(&a.strength)));
    Ok(result)
}

fn enable_wireless() -> Result<(), String> {
    let connection =
        Connection::system().map_err(|e| format!("cannot connect to NetworkManager: {e}"))?;
    let manager = NetworkManagerProxyBlocking::new(&connection)
        .map_err(|e| format!("cannot access NetworkManager: {e}"))?;
    if !manager.wireless_enabled().unwrap_or(false) {
        manager
            .set_wireless_enabled(true)
            .map_err(|e| format!("cannot enable Wi-Fi: {e}"))?;
    }
    Ok(())
}

fn classify_security(flags: u32, wpa: u32, rsn: u32) -> WifiSecurity {
    if flags & AP_FLAGS_PRIVACY == 0 && wpa == 0 && rsn == 0 {
        WifiSecurity::Open
    } else if rsn & SEC_KEY_MGMT_SAE != 0 {
        WifiSecurity::Wpa3Personal
    } else if (wpa | rsn) & SEC_KEY_MGMT_PSK != 0 {
        WifiSecurity::WpaPersonal
    } else {
        WifiSecurity::Unsupported
    }
}

pub fn connect_wifi(network: &WifiNetwork, password: &str) -> Result<(), String> {
    if network.security == WifiSecurity::Unsupported {
        return Err("this network uses an unsupported enterprise or legacy security mode".into());
    }
    if network.security != WifiSecurity::Open && password.len() < 8 {
        return Err("the Wi-Fi password must contain at least 8 characters".into());
    }
    let connection =
        Connection::system().map_err(|e| format!("cannot connect to NetworkManager: {e}"))?;
    let manager = NetworkManagerProxyBlocking::new(&connection)
        .map_err(|e| format!("cannot access NetworkManager: {e}"))?;
    let mut settings = HashMap::<String, HashMap<String, OwnedValue>>::new();
    settings.insert(
        "connection".into(),
        HashMap::from([
            ("id".into(), owned(network.ssid.clone())),
            ("uuid".into(), owned(uuid::Uuid::new_v4().to_string())),
            ("type".into(), owned("802-11-wireless".to_string())),
            ("autoconnect".into(), owned(true)),
        ]),
    );
    settings.insert(
        "802-11-wireless".into(),
        HashMap::from([
            ("ssid".into(), owned(network.ssid.as_bytes().to_vec())),
            ("mode".into(), owned("infrastructure".to_string())),
        ]),
    );
    settings.insert(
        "ipv4".into(),
        HashMap::from([("method".into(), owned("auto".to_string()))]),
    );
    settings.insert(
        "ipv6".into(),
        HashMap::from([("method".into(), owned("auto".to_string()))]),
    );
    if network.security != WifiSecurity::Open {
        let key_mgmt = if network.security == WifiSecurity::Wpa3Personal {
            "sae"
        } else {
            "wpa-psk"
        };
        settings.insert(
            "802-11-wireless-security".into(),
            HashMap::from([
                ("key-mgmt".into(), owned(key_mgmt.to_string())),
                ("psk".into(), owned(password.to_string())),
            ]),
        );
    }
    let options = HashMap::from([("persist".into(), owned("memory".to_string()))]);
    manager
        .add_and_activate_connection2(
            settings,
            network.device.clone(),
            network.access_point.clone(),
            options,
        )
        .map_err(|e| format!("NetworkManager could not connect: {e}"))?;
    Ok(())
}

fn owned<'a>(value: impl Into<Value<'a>>) -> OwnedValue {
    OwnedValue::try_from(value.into())
        .expect("plain NetworkManager values never carry file descriptors")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn security_classification_rejects_enterprise_and_wep() {
        assert_eq!(classify_security(0, 0, 0), WifiSecurity::Open);
        assert_eq!(
            classify_security(1, SEC_KEY_MGMT_PSK, 0),
            WifiSecurity::WpaPersonal
        );
        assert_eq!(
            classify_security(1, 0, SEC_KEY_MGMT_SAE),
            WifiSecurity::Wpa3Personal
        );
        assert_eq!(classify_security(1, 0, 0), WifiSecurity::Unsupported);
    }
}
