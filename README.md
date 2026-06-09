# Sirius

**Sirius** is a distro-agnostic diagnostic operating-system installer — Rust + Relm4 +
GTK4 + libadwaita on the [libreadymade](https://github.com/FyraLabs/readymade) backend.

> Sirius (named for the brightest star in the night sky) was originally created for
> **LuminusOS**, but it is a **distro-agnostic** installer: point it at any bootc/OCI-based
> distribution by supplying `/etc/sirius/distro.toml` and a systemd-repart layout.

## Features

- **Diagnostic compatibility gate** — probes UEFI, RAM, disk space, Secure Boot,
  virtualization, and network before installing, and blocks on hard failures.
- **Toggleable wizard pages** — page order and which pages run are driven by
  `/etc/sirius/sirius.toml`; no recompile needed.
- **bootc install** — deploys an OCI image via systemd-repart + bootc, with optional
  LUKS encryption.
- **Privilege split** — the unprivileged UI builds an install request; a `pkexec`
  child executes it as root and streams progress back.
- **Logging** — every install writes a timestamped log to `/tmp/sirius-install-*.log`
  and shows live progress in the UI.

## Requirements

- Build: Rust (2021), `gtk4` / `gtk4-devel`, `libadwaita` / `libadwaita-devel`.
- Runtime (target/live system): `systemd-repart`, `bootc`, `cryptsetup`, `pkexec`/polkit, `mount`.

## Build & run

```sh
cargo build
cargo run --bin sirius -- diag        # hardware compatibility report (text or --json)
cargo run --bin sirius -- --dry-run   # build & print the install request, no install
cargo run --bin sirius                # launch the GTK wizard
```

## Architecture

- `crates/sirius-diag` — pure hardware-check + config library (probes, gating, page-toggle config).
- `crates/sirius-installer` — the GTK wizard binary plus the `diag`, `--dry-run`, and
  (internal) `run-playbook` entry points.
- `crates/sirius-installer/src/backend/` — the only code that touches libreadymade
  (adapter, distro descriptor, runner, pkexec spawn) behind a `Progress` boundary.

See [`AGENTS.md`](AGENTS.md) for contributor/agent guidance and [`docs/GAPS.md`](docs/GAPS.md)
for known gaps and TODOs.
