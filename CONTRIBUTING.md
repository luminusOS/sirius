# Contributing to Sirius

Thank you for helping improve Sirius. This project is a distro-agnostic diagnostic operating-system installer written in Rust with GTK4, Libadwaita, and Relm4, on top of the libreadymade backend.

## Development Setup

Install system dependencies on Fedora or inside the project toolbox:

```sh
sudo dnf install -y \
  rust cargo pkgconf-pkg-config \
  gtk4-devel libadwaita-devel gettext
```

`gettext` provides `msgfmt`, which `crates/sirius-installer/build.rs` requires to compile the translation catalogs — without it the build fails.

Run the wizard and its entry points:

```sh
cargo run --bin sirius                 # launch the GTK wizard
cargo run --bin sirius -- diag         # hardware compatibility report (text)
cargo run --bin sirius -- diag --json  # same report, as JSON
cargo run --bin sirius -- --dry-run    # build & print the install request, no install
```

## Verification

Before opening a PR or handing off a change, run:

```sh
cargo fmt
cargo clippy --workspace --all-targets
cargo test
```

CI runs `cargo fmt --all --check`, `cargo clippy --workspace -- -D warnings`, and `cargo test --workspace` on every push and PR.

For documentation-only changes, a full Rust test run is usually not necessary.

### Full install test in a VM

Real installs are exercised by an ignored end-to-end test (`crates/sirius-installer/tests/vm_install.rs`). It needs root, a throwaway scratch disk, and a live target environment (systemd-repart, bootc, polkit, UDisks2, NetworkManager):

```sh
sudo -E SIRIUS_TEST_DISK=/dev/vdb cargo test --test vm_install -- --ignored vm_full_install
```

`SIRIUS_TEST_DISK` must be a `/dev/...` block device you are willing to erase. The test pipes an install request into the same `run-playbook` entry point the UI spawns under pkexec and asserts the run finishes.

## Development Aids

- `SIRIUS_START_PAGE=<page id>` — open the wizard directly on a page, e.g. `SIRIUS_START_PAGE=progress cargo run --bin sirius` animates the progress UI without installing. Page ids: `welcome`, `diagnostics`, `network`, `keyboard`, `timezone`, `storage`, `user`, `summary`, `progress`, `finished`.
- `--dry-run` — print the `InstallRequest` JSON plus the parsed distro descriptor without touching anything.

## AI-Assisted Contributions

Contributions made with AI assistance are welcome, but the contributor remains
responsible for the change. Do not submit code you do not understand. You must
be able to explain what the code does, why it is correct, and what tradeoffs or
risks it introduces.

AI-assisted changes must be tested thoroughly. Maintainers may ask for evidence
that the functionality works and was tested, such as test output, screenshots,
screen recordings, logs, or clear reproduction steps.

## Project Architecture

Sirius has two Rust crates:

- `sirius-diag` — pure library: hardware probes, the `Check`/`Status` model, install gating (`run_all_checks`, `is_blocked`), and the page-toggle config (`SiriusConfig`, `PagesConfig::resolve`). No GTK, fully unit-tested.
- `sirius-installer` — the GTK wizard binary `sirius`, plus the `diag`, `--dry-run`, and hidden `run-playbook` (privileged install) entry points.

Inside `sirius-installer`, `src/backend/` is the only module that touches libreadymade, NetworkManager, or UDisks2; everything else depends on the `backend::Progress` boundary type. See [ARCHITECTURE.md](ARCHITECTURE.md) for diagrams and module responsibilities.

## Coding Guidelines

- **Stay distro-agnostic.** Add per-distribution behavior through `distro.toml` / `sirius.toml` and the repart layout, never through Rust code. No hardcoded distribution names, images, or hostnames in code or `data/`.
- **The UI never touches disks.** It builds a serializable `InstallRequest`; the install runs only inside `pkexec sirius run-playbook` as root, which streams newline-delimited `Progress` JSON back on stdout.
- Keep `sirius-diag` free of GTK/Relm4 types. Probes take plain values so they stay unit-testable without hardware.
- Pages that build their widget tree programmatically (currently `diagnostics`, `network`, `storage`, `summary`) implement `SimpleComponent` by hand — a `#[name = ...]` binding inside a `set_child` block fights the `#[relm4::component]` macro.
- `WizardState` is authoritative for Next-gating via `can_proceed()`. Pages only report user choices as `PageOutput` values; the folded `InstallConfig` is what the gate reads.
- Use clear, actionable error strings. pkexec exit 126/127 failures should point at the polkit authentication agent.
- **Commits: never add a `Co-Authored-By` / co-author trailer.**

## Translations

- Translations live in `po/` at the repo root as gettext catalogs, not in a Rust table. `po/LINGUAS` lists the enabled languages (currently `pt_BR`); `po/POTFILES` lists the translatable source files.
- msgids are the English literals passed to `gettextrs::gettext()` calls in the UI code. Keep them as plain English — do not introduce symbolic keys.
- When you add or change a user-facing string, keep `po/pt_BR.po` in sync (matching `msgid`/`msgstr` entries) and validate with:

```sh
msgfmt --check -o /dev/null po/pt_BR.po
```

- `crates/sirius-installer/build.rs` compiles every catalog in `LINGUAS` with `msgfmt --check` into `$OUT_DIR/locale` (dev runs) and `data/locale` (packaging), so `msgfmt` is a required build tool.
- Runtime language switching happens on the welcome page: the wizard sets the `LANGUAGE` environment variable and broadcasts a `Retranslate` message to every page, which re-renders through gettext.

## UI Guidelines

- Follow GNOME HIG and Libadwaita conventions.
- Use symbolic icons from the current icon theme; bento and branding icon names referenced from `distro.toml` must exist in the live system's theme.
- Keep text concise and truthful. Do not describe a capability that is not implemented.
- Leaving the summary page erases a disk: keep the destructive confirmation dialog on that path.

## Packaging

RPM metadata lives in `crates/sirius-installer/Cargo.toml` under `[package.metadata.generate-rpm]`: the binary, the polkit policy, the desktop file, the icon, `distro.toml`, `sirius.toml`, the repart layout, and the compiled locale catalogs. CI builds the RPM with `cargo generate-rpm` on every push and attaches it to GitHub releases on `v*` tags. See [INSTALL.md](INSTALL.md) for the install paths, the polkit/pkexec policy (`io.sirius.Installer.run-playbook`), and the runtime requirements on the target system.

## Pull Request Checklist

- The change is scoped to one concern.
- `cargo fmt`, clippy, and tests pass when code changes are involved.
- No distribution names, images, or hostnames crept into code or `data/`.
- UI changes follow GNOME HIG and page gating still works (`WizardState::can_proceed`).
- New or changed user-facing strings are reflected in `po/pt_BR.po` and pass `msgfmt --check`.
- The UI still never writes to disks outside the privileged runner.
- Documentation is updated when behavior, packaging, or commands change.
- The commit message has no `Co-Authored-By` trailer.
