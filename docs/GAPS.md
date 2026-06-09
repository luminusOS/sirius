# Known gaps & TODOs

Sirius is a distro-agnostic installer. Distribution-specific behavior — including which
first-boot agent configures the user account and timezone — is the distribution's
responsibility, configured via `/etc/sirius/distro.toml`; Sirius assumes no particular
distro.

## TODO — postinstall provisioning modules

At the pinned `libreadymade` commit there is **no** postinstall module for the following
settings. The wizard collects them and carries them on `InstallRequest`, but
`into_playbook` currently wires only locale (`Language`) plus `InitialSetup` (which writes
`/.unconfigured` to trigger the distribution's first-boot setup agent, e.g.
gnome-initial-setup). Until dedicated modules exist, these four are deferred to first boot:

- [ ] **User account** — create the primary user (name, username, password).
- [ ] **Hostname** — set `/etc/hostname`.
- [ ] **Timezone** — set the system timezone.
- [ ] **Keyboard layout** — set the console/X11 keymap.

Implement each as a custom `Script` postinstall module (or via an upstream libreadymade
bump) so the installed system is fully configured non-interactively rather than relying on
a first-boot agent.

## Other gaps

- [ ] **Encryption key = user password (MVP).** LUKS currently reuses the account
  password (`adapter::build_request`). Collect a dedicated passphrase instead.
- [ ] **Placeholder repart templates.** `data/repart.d/*.conf` are generic ESP + btrfs
  defaults; ship real per-distribution layouts.
- [ ] **pkexec target is pinned to `/usr/bin/sirius`** (polkit policy). Only works when
  installed, not when run from `target/debug`.
- [ ] **libreadymade pin + patch.** Pinned to readymade HEAD `ccdf092`, which does not
  compile due to a one-line `PathBuf == String` bug in the sibling `filesystem-table`
  crate. `vendor/filesystem-table/` carries a one-line-fixed copy overridden via
  `[patch."https://github.com/FyraLabs/readymade.git"]`. `libreadymade` is pulled with
  `default-features = false` to avoid the `uutils` feature (which needs `libacl-devel`);
  the default `rdm` copy backend is used. **When bumping the `rev`, bump the vendored
  crate's version in lockstep** or the patch silently stops applying.
- [ ] **Progress bar appears static** during the bootc image pull + repart (upstream emits
  no progress there); only postinstall modules report progress. Textual progress and
  errors appear in the on-screen log view.
- [ ] **Real installs are only verified via the ignored VM test** (`tests/vm_install.rs`)
  in a live ISO; there is no unprivileged way to exercise the full path.

## Not yet implemented

- [ ] **Manual partitioning page** (`manual_partition`) — a known page id but no widget;
  it is filtered out of the navigator. Implement an advanced partition editor.
- [ ] **Full Wi-Fi UI** on the network page (currently informational; relies on the
  system network indicator).
