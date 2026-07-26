//! Host lifecycle operations exposed by systemd-logind.

use zbus::blocking::Connection;

#[zbus::proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait LoginManager {
    fn reboot(&self, interactive: bool) -> zbus::Result<()>;
}

/// Ask systemd-logind to reboot the host.
///
/// `interactive` lets polkit obtain authorization when necessary. Logind is
/// also the systemd-recommended boundary for graphical applications and can
/// authorize overriding inhibitors held by the live installer session.
pub fn reboot() -> Result<(), String> {
    let connection =
        Connection::system().map_err(|error| format!("cannot connect to system bus: {error}"))?;
    let manager = LoginManagerProxyBlocking::new(&connection)
        .map_err(|error| format!("cannot access systemd-logind: {error}"))?;

    manager
        .reboot(true)
        .map_err(|error| format!("systemd-logind refused to reboot: {error}"))
}
