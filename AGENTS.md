# AGENTS.md — guidance for coding agents & contributors

Sirius is a **distro-agnostic** diagnostic OS installer. Keep it that way: no hardcoded
distribution names, images, or hostnames in code or `data/` (distro specifics live in
`/etc/sirius/distro.toml` and `/etc/sirius/sirius.toml`). The only place a specific
distribution is named is `README.md`, as the origin story.

## Project layout

- `crates/sirius-core` — shared domain and protocol types: `InstallConfig`,
  partition plans, `DistroDescriptor`, `InstallRequest`, and `Progress`. It has
  no GTK, process spawning, hardware discovery, or disk I/O.
- `crates/sirius-diag` — pure library: hardware probes, the `Check`/`Status` model,
  install gating (`run_all_checks`, `is_blocked`), and the page-toggle config
  (`SiriusConfig`, `PagesConfig::resolve`). No GTK, fully unit-tested.
- `crates/sirius-backend` — the ONLY crate that touches `libreadymade`,
  NetworkManager, UDisks2, pkexec, or the privileged runner. Its storage facade
  keeps read-only discovery through the Rust `lsblk` crate, sysfs, and udev
  separate from confirmed mutations; `sirius-diag` has its own read-only
  hardware probes.
- `crates/sirius-app` — the Relm4/GTK4 frontend: window, pages, navigation,
  wizard state, bootstrap, and the background install task.
- `crates/sirius-installer` — thin launcher producing the single `sirius`
  executable. It dispatches `diag`, `--dry-run`, the GTK app, and the hidden
  `run-playbook` entry points.
- `po/` — gettext catalogs at the repo root: `LINGUAS` (enabled languages),
  `POTFILES` (translatable sources), `pt_BR.po`, `sirius.pot`.

## Toolchain

- Rust 2024. relm4 **0.10**, gtk4 **0.10**, libadwaita (`adw`) **0.8**, with relm4
  features `["libadwaita","gnome_45"]`. Needs `libadwaita-devel` / `gtk4-devel`.
- `libreadymade` comes from the sibling `readymade` workspace through a path
  dependency (`default-features = false` to drop the `uutils`/`libacl` feature;
  the native `rdm` copy backend is used).
- `msgfmt` (gettext) is a required build tool: `crates/sirius-installer/build.rs`
  compiles the `po/` catalogs with it.

## Commands

```sh
cargo build
cargo test
cargo clippy --workspace --all-targets
cargo run --bin sirius -- diag
cargo run --bin sirius -- --dry-run
# Full VM install test (root, scratch disk, live env):
sudo -E SIRIUS_TEST_DISK=/dev/vdb cargo test --test vm_install -- --ignored vm_full_install
# Ready-made manual test VM (Silverblue runner + Luminus bootc target):
vagrant up --provider=libvirt
vagrant provision
vagrant ssh
```

## Conventions

- **Commits: never add a `Co-Authored-By` / co-author trailer.**
- **VM distro details stay in the Vagrant VM.** `Vagrantfile` generates a
  Fedora-specific descriptor and installs it only in the disposable guest;
  never copy those defaults into Rust code or `data/`.
- **Imperative pages use a manual `SimpleComponent` impl.** Pages that build their
  widget tree programmatically (`diagnostics`, `network`, `storage`, `summary`)
  implement `SimpleComponent` by hand — `#[name=...]` inside a `set_child` block
  fights the `#[relm4::component]` macro.
- **`WizardState` is authoritative for Next-gating** via `can_proceed()`. Pages emit
  `PageOutput::Set*` values as the user makes choices (a disk picked, an account
  typed); the folded `InstallConfig` is what the gate reads. The resolved page list
  is filtered to `IMPLEMENTED_PAGES`.
- **The UI never touches disks.** It builds an `InstallRequest` (serializable); the
  install runs only inside `pkexec sirius run-playbook` as root, which streams
  newline-delimited `Progress` JSON back on stdout.
- **Stay distro-agnostic.** Add per-distribution behavior through `distro.toml` /
  `sirius.toml`, not Rust code.
- **Translations live in `po/`, not a Rust table.** msgids are the English literals
  passed to `gettextrs::gettext()`. Keep `po/pt_BR.po` in sync when strings change
  (validate with `msgfmt --check -o /dev/null po/pt_BR.po`); `build.rs` compiles the
  catalogs via `msgfmt`, which is a required build tool. Runtime switching sets the
  `LANGUAGE` environment variable and broadcasts a per-page `Retranslate` message.
