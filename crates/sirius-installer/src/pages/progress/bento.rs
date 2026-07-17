//! Optional distro link cards shown during installation.

use crate::backend::distro::Bento;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::gtk::gio;
use relm4::gtk::gio::prelude::AppInfoExt;
use relm4::gtk::glib;
use std::process::{Command, Stdio};
use std::thread;

const MAX_BENTOS: usize = 3;

/// XDG desktop-entry field codes that get substituted with file/URL
/// arguments by the launcher. We build the argv ourselves, so these are
/// stripped and the URL is appended explicitly instead.
const FIELD_CODES: [&str; 4] = ["%u", "%U", "%f", "%F"];

/// Cap on how much memory the launched browser (and anything it spawns) may
/// use, enforced via a transient `systemd-run --user --scope`. The live ISO
/// runs entirely from tmpfs, so an uncapped browser can exhaust RAM and take
/// the whole session down with it.
const MEMORY_HIGH: &str = "MemoryHigh=1G";
const MEMORY_MAX: &str = "MemoryMax=1536M";

/// Parses a GIO `AppInfo` commandline (e.g. `"/usr/bin/epiphany %u"`) into
/// an argv with desktop-entry field codes removed. Returns `None` if the
/// commandline can't be parsed or has nothing left after stripping codes.
///
/// Split out as a pure function (no process spawning, no GTK/display
/// dependency beyond `glib::shell_parse_argv`'s string parsing) so it's
/// unit-testable in isolation.
fn parse_commandline_argv(commandline: &str) -> Option<Vec<String>> {
    let argv = glib::shell_parse_argv(commandline).ok()?;
    let argv: Vec<String> = argv
        .into_iter()
        .filter_map(|arg| arg.to_str().map(str::to_owned))
        .filter(|arg| !FIELD_CODES.contains(&arg.as_str()))
        .collect();
    if argv.is_empty() {
        None
    } else {
        Some(argv)
    }
}

/// Resolves the default browser's argv (with field codes stripped) via the
/// desktop's default handler for the `https` URI scheme.
fn resolve_browser_argv() -> Option<Vec<String>> {
    let app_info = gio::AppInfo::default_for_uri_scheme("https")?;
    let commandline = app_info.commandline()?;
    let commandline = commandline.to_str()?;
    parse_commandline_argv(commandline)
}

/// Spawns `argv` followed by `url`, wrapped in a memory-capped transient
/// systemd scope. `--scope` blocks for the lifetime of the wrapped process,
/// so callers must not `.wait()` on the returned child from the GTK main
/// loop thread.
fn spawn_capped(argv: &[String], url: &str) -> std::io::Result<std::process::Child> {
    Command::new("systemd-run")
        .args([
            "--user",
            "--scope",
            "--quiet",
            "-p",
            MEMORY_HIGH,
            "-p",
            MEMORY_MAX,
            "--",
        ])
        .args(argv)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}

/// Opens `url` in the default browser, capped to a bounded amount of memory
/// so it can't exhaust RAM on the tmpfs-backed live session.
///
/// - Resolves the default `https` handler's argv (falling back to
///   `gio open <url>` if none is registered) and runs it under
///   `systemd-run --user --scope` with `MemoryHigh`/`MemoryMax` set.
/// - If `systemd-run` itself can't be spawned (e.g. missing on the live
///   image), falls back to the original uncapped `gtk::UriLauncher` launch
///   and logs a warning.
fn open_link_capped(url: &str, window: Option<&gtk::Window>) {
    let argv = resolve_browser_argv().unwrap_or_else(|| vec!["gio".to_owned(), "open".to_owned()]);

    match spawn_capped(&argv, url) {
        Ok(mut child) => {
            // `--scope` blocks until the wrapped process exits; reap it off
            // the main loop thread so we don't stall the UI or leave a
            // zombie behind.
            //
            // A non-zero exit here can mean the browser ran and exited (not
            // necessarily an error), but it can also mean `systemd-run`
            // itself failed to establish the scope (e.g. no active `--user`
            // D-Bus/systemd session during early live-ISO boot, an
            // unsupported cgroup property, or a permission error) — in which
            // case the browser never launched at all. We can't fully
            // distinguish the two from the exit code alone, and this runs on
            // a background thread after the click handler has already
            // returned, so there's no straightforward way to marshal a
            // `gtk::UriLauncher` fallback back onto the GTK main thread here.
            // Log clearly so a silent no-op click is at least diagnosable.
            thread::spawn(move || match child.wait() {
                Ok(status) if !status.success() => {
                    tracing::warn!(
                        "memory-capped link launch may have failed: systemd-run --scope exited with {status}"
                    );
                }
                Ok(_) => {}
                Err(err) => {
                    tracing::warn!("failed to wait on memory-capped link launch: {err}");
                }
            });
        }
        Err(err) => {
            tracing::warn!("systemd-run unavailable ({err}); opening link without a memory cap");
            gtk::UriLauncher::new(url).launch(window, gio::Cancellable::NONE, |_| {});
        }
    }
}

pub(super) fn append_cards(container: &gtk::Box, bentos: &[Bento]) {
    for bento in bentos.iter().take(MAX_BENTOS) {
        container.append(&card(bento));
    }
}

fn themed_icon(name: &str) -> &str {
    let has = gtk::gdk::Display::default()
        .map(|display| gtk::IconTheme::for_display(&display).has_icon(name))
        .unwrap_or(false);
    if has {
        name
    } else {
        tracing::warn!("bento icon '{name}' not in icon theme; using fallback");
        "web-browser-symbolic"
    }
}

fn card(bento: &Bento) -> gtk::Button {
    let title = gtk::Label::builder()
        .label(&bento.title)
        .halign(gtk::Align::Start)
        .wrap(true)
        .css_classes(["heading"])
        .build();
    let description = gtk::Label::builder()
        .label(&bento.desc)
        .halign(gtk::Align::Start)
        .wrap(true)
        .css_classes(["dim-label", "caption"])
        .build();
    let text = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .valign(gtk::Align::Center)
        .build();
    text.append(&title);
    text.append(&description);

    let icon = gtk::Image::builder()
        .icon_name(themed_icon(&bento.icon))
        .pixel_size(32)
        .valign(gtk::Align::Center)
        .css_classes(["accent"])
        .build();
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    content.append(&icon);
    content.append(&text);

    let card = gtk::Button::builder()
        .child(&content)
        .hexpand(true)
        .css_classes(["card"])
        .build();
    let link = bento.link.clone();
    card.connect_clicked(move |button| {
        open_link_capped(&link, button.root().and_downcast_ref::<gtk::Window>());
    });
    card
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_single_field_code() {
        assert_eq!(
            parse_commandline_argv("/usr/bin/epiphany %u"),
            Some(vec!["/usr/bin/epiphany".to_owned()])
        );
    }

    #[test]
    fn strips_upper_and_lower_field_codes() {
        for code in ["%u", "%U", "%f", "%F"] {
            let commandline = format!("/usr/bin/browser --flag {code}");
            assert_eq!(
                parse_commandline_argv(&commandline),
                Some(vec!["/usr/bin/browser".to_owned(), "--flag".to_owned()])
            );
        }
    }

    #[test]
    fn preserves_argument_order_and_quoting() {
        assert_eq!(
            parse_commandline_argv("/usr/bin/firefox --new-window %U"),
            Some(vec![
                "/usr/bin/firefox".to_owned(),
                "--new-window".to_owned()
            ])
        );
        assert_eq!(
            parse_commandline_argv("\"/usr/bin/my browser\" --flag %u"),
            Some(vec!["/usr/bin/my browser".to_owned(), "--flag".to_owned()])
        );
    }

    #[test]
    fn no_field_code_keeps_full_argv() {
        assert_eq!(
            parse_commandline_argv("/usr/bin/epiphany"),
            Some(vec!["/usr/bin/epiphany".to_owned()])
        );
    }

    #[test]
    fn empty_after_stripping_returns_none() {
        assert_eq!(parse_commandline_argv("%u"), None);
    }

    #[test]
    fn unparsable_commandline_returns_none() {
        // Unbalanced quote: g_shell_parse_argv should reject this.
        assert_eq!(parse_commandline_argv("\"unterminated"), None);
    }
}
