# AGENTS.md — guidance for coding agents & contributors

Sirius is a **distro-agnostic** diagnostic OS installer. Keep it that way: no hardcoded
distribution names, images, or hostnames in code or `data/` (distro specifics live in
`/etc/sirius/distro.toml` and `/etc/sirius/sirius.toml`). The only place a specific
distribution is named is `README.md`, as the origin story.

## Project layout

- `crates/sirius-diag` — pure library: hardware probes, the `Check`/`Status` model,
  install gating (`run_all_checks`, `is_blocked`), and the page-toggle config
  (`SiriusConfig`, `PagesConfig::resolve`). No GTK, fully unit-tested.
- `crates/sirius-installer` — the GTK wizard binary `sirius`. Subcommands: `diag`,
  `--dry-run`, and the hidden `run-playbook` (the privileged install entry point).
- `crates/sirius-installer/src/backend/` — the ONLY module that touches `libreadymade`:
  `distro` (descriptor), `adapter` (`InstallConfig` → `InstallRequest` → `Playbook`),
  `runner` (root-side execute), `spawn` (pkexec + progress parse). Everything else
  depends on the `backend::Progress` boundary type, not on libreadymade directly.
- `vendor/filesystem-table/` — a patched copy of an upstream crate, overridden via
  Cargo `[patch]` (see `docs/GAPS.md`).

## Toolchain

- Rust 2021. relm4 **0.10**, gtk4 **0.10**, libadwaita (`adw`) **0.8**, with relm4
  features `["libadwaita","gnome_45"]`. Needs `libadwaita-devel` / `gtk4-devel`.
- `libreadymade` is a pinned git dependency with a `[patch]` override (see GAPS).

## Commands

```sh
cargo build
cargo test
cargo clippy --workspace --all-targets
cargo run --bin sirius -- diag
cargo run --bin sirius -- --dry-run
# Full VM install test (root, scratch disk, live env):
sudo -E SIRIUS_TEST_DISK=/dev/vdb cargo test --test vm_install -- --ignored vm_full_install
```

## Conventions

- **Commits: never add a `Co-Authored-By` / co-author trailer.**
- **Imperative pages use a manual `SimpleComponent` impl.** Pages that fill an
  `adw::PreferencesGroup` programmatically (`diagnostics`, `disk`) implement
  `SimpleComponent` by hand — `#[name=...]` inside a `set_child` block fights the
  `#[relm4::component]` macro.
- **`AppModel` is authoritative for page-arrival Next-gating** via `gate_for()`.
  Pages emit `PageOutput::CanProceed` only for dynamic changes (e.g. a disk picked,
  account typed). The navigator is filtered to `IMPLEMENTED_PAGES`.
- **The UI never touches disks.** It builds an `InstallRequest` (serializable); the
  install runs only inside `pkexec sirius run-playbook` as root, which streams
  newline-delimited `Progress` JSON back on stdout.
- **Stay distro-agnostic.** Add per-distribution behavior through `distro.toml` /
  `sirius.toml`, not Rust code.
