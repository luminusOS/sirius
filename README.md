<div align="center">

<img src="data/icons/hicolor/scalable/apps/io.sirius.Installer.svg" alt="Sirius logo" width="160" height="160" />

# Sirius

**Guided by the brightest star.**

A distro-agnostic diagnostic operating-system installer, built in Rust with GTK4 and Libadwaita.

[LuminusOS](https://luminusos.org) · [Report a bug](https://github.com/luminusOS/sirius/issues)

</div>

---

Sirius takes its name from the brightest star in the night sky. It was
originally created for **LuminusOS**, but it is a **distro-agnostic**
installer: point it at any bootc/OCI-based distribution by supplying
`/etc/sirius/distro.toml` and a systemd-repart layout — no recompile needed.

It probes the machine before anything is written, walks the user through a
toggleable wizard, and deploys the system through the
[libreadymade](https://github.com/FyraLabs/readymade) backend: systemd-repart
for partitioning, bootc for the image, with optional LUKS encryption.

## Features

- **Diagnostic compatibility gate** — probes UEFI, RAM, disk space, Secure Boot,
  virtualization, and network before installing, and blocks on hard failures.
- **Toggleable wizard pages** — page order and which pages run are driven by
  `/etc/sirius/sirius.toml`; no recompile needed.
- **bootc install** — deploys an OCI image via systemd-repart + bootc, with optional
  LUKS encryption.
- **Unified storage editor** — choose automatic provisioning or stage a validated
  GPT layout with create/delete/format/label/mount operations. Manual writes run
  through UDisks2 only after the final confirmation.
- **Wi-Fi in the installer** — when a wireless adapter exists, scan and connect to
  open or WPA/WPA2/WPA3 Personal networks through NetworkManager. The page is omitted
  on systems without Wi-Fi hardware.
- **Privilege split** — the unprivileged UI builds an install request; a `pkexec`
  child executes it as root and streams progress back.
- **Translated UI** — English and Brazilian Portuguese, switchable live from the
  welcome page.
- **Logging** — every install writes a timestamped log to `/tmp/sirius-install-*.log`
  and shows live progress in the UI.

## Documentation

- [INSTALL.md](INSTALL.md) — shipping Sirius in a distribution ISO: install paths,
  the polkit policy, the distro descriptor and repart layout, and runtime requirements.
- [CONTRIBUTING.md](CONTRIBUTING.md) — development setup, verification, the VM
  install test, development aids, and the translation workflow.
- [ARCHITECTURE.md](ARCHITECTURE.md) — crate boundaries, the backend boundary,
  wizard flow, the storage subsystem, and internationalization diagrams.
- [docs/GAPS.md](docs/GAPS.md) — known gaps and TODOs.

## Built with

[Rust](https://www.rust-lang.org/) · [GTK4](https://gtk.org/) ·
[Libadwaita](https://gitlab.gnome.org/GNOME/libadwaita) ·
[Relm4](https://relm4.org/) · [libreadymade](https://github.com/FyraLabs/readymade) ·
[bootc](https://github.com/bootc-dev/bootc)

## Acknowledgements

Sirius builds on [Readymade](https://github.com/FyraLabs/readymade) by Fyra Labs —
`libreadymade` does the heavy lifting of every install.
