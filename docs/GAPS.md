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

- [x] ~~**Encryption key = user password (MVP).**~~ Resolved: the storage page
  now collects a dedicated LUKS passphrase pair (`encryption_passphrase` on
  `InstallConfig`, gated by `WizardState::storage_is_valid`), and
  `adapter::build_request` uses it instead of the account password.
- [ ] **Placeholder repart templates.** `data/repart.d/*.conf` are generic ESP + btrfs
  defaults; ship real per-distribution layouts.
- [ ] **pkexec target is pinned to `/usr/bin/sirius`** (polkit policy). Only works when
  installed, not when run from `target/debug`. Mitigations: when the UI already runs as
  root the runner is spawned directly (no pkexec), and pkexec exits 126/127 are reported
  with a clear polkit-agent hint in the progress log. A live session still needs a polkit
  agent or a rule granting the action (see INSTALL.md).
- [ ] **libreadymade comes from the LuminusOS fork**, pinned to rev `c58f56d`, while
  upstream fixes required by Sirius are pending (the fork carries the patched
  `filesystem-table` crate in-tree, so no Cargo `[patch]` override is needed).
  `libreadymade` is pulled with `default-features = false` to avoid the `uutils`
  feature (which needs `libacl-devel`); the default `rdm` copy backend is used.
- [x] ~~**Progress bar appears static** during the bootc image pull + repart.~~
  Resolved: the progress page pulses the bar for any stage message without a
  fraction (`ProgressMsg::Pulse` on a 120 ms timer plus `advance_bar(0.0)`);
  only postinstall modules report real fractions. Textual progress and errors
  appear in the log view behind the progress page's log toggle button.
- [ ] **Real installs are only verified via the ignored VM test** (`tests/vm_install.rs`)
  in a live ISO; there is no unprivileged way to exercise the full path.

## i18n — dynamic strings remain English

The wizard UI (page titles, descriptions, field labels, buttons) is translated live
(en/pt-BR) via gettextrs and the catalogs in `po/` (`POTFILES` lists the translatable
sources; `build.rs` compiles them with `msgfmt`). Dynamic strings produced outside the
page widgets are now mostly covered too:

- [x] Diagnostic check labels/details — `sirius-diag` resolves them through the
  process textdomain at probe time (the `diag` subcommand also initializes gettext).
- [x] Install progress/log lines emitted by Sirius's own runner code — the runner
  initializes gettext and pins `LANGUAGE` from `InstallRequest.locale` (pkexec
  scrubs the environment). Stage strings originating inside **libreadymade**
  itself remain English-only (upstream has no catalogs).
- [x] Account validation error messages (`UserAccount::validate`) and the LUKS
  passphrase validation.
- [ ] Error strings from `backend::storage` / `backend::distro` surfaced through
  runner `fail(...)` wrappers are still English-only.
- [ ] Storage and NetworkManager runtime flows still need hardware-in-the-loop
  coverage across SATA, NVMe, WPA3 transition mode, and multiple Wi-Fi adapters.

Add new UI strings to `po/sirius.pot` AND `po/pt_BR.po`, and the source file to
`po/POTFILES`.

## Deliberate storage limits

- [ ] **Resize and move partitions.** The manual editor intentionally supports only
  create, delete, format, label, and mount assignment. Resizing/moving needs a separate
  filesystem-aware safety design.
- [ ] **Manual LUKS layouts.** Encryption remains available in automatic mode only;
  custom encrypted mount graphs are not yet represented by `PartitionPlan`.
