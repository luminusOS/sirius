# Contributing to Sirius

Thank you for helping improve Sirius. This project is a distro-agnostic diagnostic operating-system installer written in Rust with GTK4, Libadwaita, and Relm4, on top of the libreadymade backend.

## Development Setup

Install system dependencies on Fedora or inside the project toolbox:

```sh
sudo dnf install -y \
  rust cargo pkgconf-pkg-config \
  gtk4-devel libadwaita-devel libgweather-devel gnome-desktop4-devel gettext \
  lcms2-devel fontconfig-devel libseccomp-devel glycin-loaders bubblewrap
```

The `lcms2`/`fontconfig`/`libseccomp` devel packages build the `glycin` crate
(SVG timezone map); `glycin-loaders` + `bubblewrap` are its sandboxed runtime
decoders. `gnome-desktop4-devel` and `fontconfig-devel` are the link targets
of the keyboard/language pages' FFI modules (`libgnome-desktop-4`,
`libfontconfig`).

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

### Vagrant test VM

The repository's `Vagrantfile` runs Sirius directly in a ready-made Fedora
Silverblue 44 graphical VM with the libvirt provider. It does not require
building a Luminus live ISO. Sirius installs
`docker://ghcr.io/luminusos/luminusos-workstation:44` onto an isolated,
disposable qcow2 disk, so the resulting target is a real bootc system.

There is no wrapper or separate lab command. Vagrant builds Sirius, transfers
the runtime files, unlocks a disposable writable Silverblue `/usr` overlay, and
installs the binary, polkit policy, desktop file, configuration, locale, and
repart definitions at their production paths. The real GNOME polkit agent
handles authorization; the lab does not bypass the policy.

#### Requirements

- Fedora 44 x86_64 with KVM and libvirt.
- Vagrant 2.3 or newer with `vagrant-libvirt`.
- Cargo/Rust, `edk2-ovmf`, and `swtpm`.
- `virt-viewer`, GNOME Boxes, Field Monitor, or another SPICE client.

The VM deliberately uses the host's packaged OVMF firmware. The firmware path
embedded in the downloaded box is below `~/.vagrant.d/boxes` and is blocked by
SELinux when QEMU starts; Vagrant creates a correctly labelled NVRAM copy in
the VM state directory instead.

#### Start and update

From the Sirius repository:

```sh
vagrant up --provider=libvirt
```

Open the VM's graphical display with `virt-viewer`, GNOME Boxes, or Field
Monitor. Silverblue starts and logs into the `vagrant` GNOME session
automatically; launch **Install Operating System** from the app grid. The
runner suppresses GNOME's initial setup and tour, disables idle suspension,
and keeps the screen from locking so long-running installation tests remain
visible. The `vagrant` user's password is `vagrant`, including for the real
polkit authentication dialog.

After source changes, one native command rebuilds, synchronizes, and reinstalls
Sirius in the running VM:

```sh
vagrant provision
```

Close and reopen Sirius to use the new binary. A runner reboot discards the
transient `/usr` overlay. The uploaded payload lives persistently under the
`vagrant` user's data directory, so the `always` provisioner can reinstall it
after a reload:

```sh
vagrant reload --provision
```

`vagrant reload --no-provision` intentionally skips the file upload. On VMs
created by older revisions, run `vagrant provision` once without
`--provision-with`; this migrates the complete payload to its persistent
location. Selecting only the `sirius` shell provisioner cannot perform that
upload.

Other useful standard commands:

```sh
vagrant ssh
vagrant halt
vagrant destroy
```

The runner is an OSTree Silverblue deployment. The disk installed by Sirius
is a real bootc deployment and can be selected from the VM's UEFI boot menu.

#### Repeat an installation

The normal repeat-installation loop does not require deleting the target
qcow2. As long as the VM is running the Silverblue runner, `/dev/vdb` is an
unmounted disposable target and Sirius can overwrite its existing partition
table:

```sh
vagrant provision
```

Close and reopen **Install Operating System**, select the disk with serial
`SIRIUS_TARGET`, and complete the wizard again.

If the VM was rebooted into the installed LuminusOS system, restart it and use
the UEFI boot menu in the SPICE console to select the Silverblue runner disk.
Once the runner is active and `vagrant ssh` works, run `vagrant provision` and
reopen Sirius.

To discard the installed system and recreate a completely blank target,
destroy and create the VM again:

```sh
vagrant destroy -f
vagrant up --provider=libvirt
```

The libvirt provider owns the disposable `target.qcow2` and, in the `multiple`
profile, `extra.qcow2`, so `vagrant destroy` removes them together with the
runner. Use the same `SIRIUS_DISK_PROFILE` value on `destroy` that was used on
`up`; Vagrant evaluates the profile on every command. On a new libvirt
session, Vagrant creates its `default` storage pool under `.vagrant/disks/`.
If that session already has a `default` pool, Vagrant uses the existing pool's
path instead.

#### VM and disk scenarios

Choose profiles through environment variables before the first `vagrant up`:

```sh
SIRIUS_VM_PROFILE=low-ram vagrant up
SIRIUS_VM_PROFILE=no-tpm vagrant up
SIRIUS_VM_PROFILE=secure-boot vagrant up

SIRIUS_DISK_PROFILE=small vagrant up
SIRIUS_DISK_PROFILE=multiple vagrant up
SIRIUS_DISK_PROFILE=none vagrant up
```

The defaults are 4 vCPUs, 8 GiB RAM, UEFI, TPM 2.0, and one empty 64 GiB
virtio disk with serial `SIRIUS_TARGET`. `small` creates an 8 GiB target,
`multiple` adds a 48 GiB SATA disk, and `none` presents no install target.

Profiles describe libvirt hardware and must remain the same for the life of a
VM. To switch one, destroy the runner with its current profile first; the
provider removes its disposable install disks automatically.

Create partitioned or mounted-disk scenarios inside the disposable guest:

```sh
vagrant ssh
sudo systemd-repart --empty=create /dev/disk/by-id/virtio-SIRIUS_TARGET
```

For the mounted/in-use case, format one of the created data partitions and
mount it before opening Sirius. Never run these commands against the runner
disk.

#### Network and virtual Wi-Fi

Wired offline/online behavior can be switched inside the runner:

```sh
vagrant ssh -c 'sudo nmcli networking off'
vagrant ssh -c 'sudo nmcli networking on'
```

Virtual Wi-Fi setup is an optional named Vagrant provisioner. The first call
layers `hostapd` and `iw` into Silverblue:

```sh
vagrant provision --provision-with wifi
vagrant reload
vagrant provision --provision-with wifi
```

It creates `Sirius Open`, `Sirius WPA2`, and `Sirius WPA3`. The WPA2/WPA3
password is `sirius-test`. This tests NetworkManager and Sirius behavior with
the kernel's `mac80211_hwsim`; it does not emulate physical radio quality.

#### Installation passes

Exercise at least:

1. Automatic unencrypted installation.
2. Automatic LUKS installation.
3. TPM-sealed LUKS installation.
4. Manual ESP/root partitioning.
5. Polkit cancellation and rejection.
6. Offline, small-disk, multiple-disk, and no-disk behavior.

After installation, reboot and use the UEFI boot menu in the SPICE console to
select the disk with serial `SIRIUS_TARGET`. In the installed system,
`bootc status` verifies that the target is a bootc deployment. Select the
Silverblue runner disk in the same menu to return to Sirius development.

## Development Aids

- `SIRIUS_START_PAGE=<page id>` — open the wizard directly on a page, e.g. `SIRIUS_START_PAGE=progress cargo run --bin sirius` animates the progress UI without installing. Page ids: `welcome`, `language`, `diagnostics`, `network`, `keyboard`, `timezone`, `storage`, `user`, `summary`, `progress`, `finished`.
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

Sirius has five focused Rust crates:

- `sirius-core` — domain models, distro schema, partition plans, and the stable
  `InstallRequest`/`Progress` process protocol. It performs no system or GUI I/O.
- `sirius-diag` — pure library: hardware probes, the `Check`/`Status` model, install gating (`run_all_checks`, `is_blocked`), and the page-toggle config (`SiriusConfig`, `PagesConfig::resolve`). No GTK, fully unit-tested.
- `sirius-backend` — system integrations, request/playbook conversion, pkexec,
  runner, NetworkManager, read-only disk discovery, and privileged UDisks2 writes.
- `sirius-app` — the Relm4/GTK wizard and its GTK-free navigation/state modules.
- `sirius-installer` — thin CLI/dispatch crate that still produces the single
  installed `sirius` binary.

Only `sirius-backend` may touch libreadymade, NetworkManager, or UDisks2, and
only `sirius-app` may depend on GTK/Relm4. See
[ARCHITECTURE.md](ARCHITECTURE.md) for diagrams and dependency rules.

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
- Runtime language switching happens on the `language` page: the wizard sets the `LANGUAGE` environment variable and broadcasts a `Retranslate` message to every page, which re-renders through gettext.

## UI Guidelines

- Follow GNOME HIG and Libadwaita conventions.
- Use symbolic icons from the current icon theme; bento and branding icon names referenced from `distro.toml` must exist in the live system's theme.
- Keep distro identity in `[branding]`: `name`, `logo`, `welcome_banner`, and
  `welcome_button`. The button accepts `{name}`; banner and logo values are file
  paths available in the live system. `/usr/share/sirius/welcome-banner.png` is
  the packaged generic fallback.
- Keep text concise and truthful. Do not describe a capability that is not implemented.
- Leaving the summary page erases a disk: keep the destructive confirmation dialog on that path.

## Packaging

RPM metadata lives in `crates/sirius-installer/Cargo.toml` under `[package.metadata.generate-rpm]`: the binary, the polkit policy, the desktop file, the icon, generic welcome banner, `distro.toml`, `sirius.toml`, the repart layout, and the compiled locale catalogs. CI builds the RPM with `cargo generate-rpm` on every push and attaches it to GitHub releases on `v*` tags. See [INSTALL.md](INSTALL.md) for the install paths, the polkit/pkexec policy (`io.sirius.Installer.run-playbook`), and the runtime requirements on the target system.

## Pull Request Checklist

- The change is scoped to one concern.
- `cargo fmt`, clippy, and tests pass when code changes are involved.
- No distribution names, images, or hostnames crept into code or `data/`.
- UI changes follow GNOME HIG and page gating still works (`WizardState::can_proceed`).
- New or changed user-facing strings are reflected in `po/pt_BR.po` and pass `msgfmt --check`.
- The UI still never writes to disks outside the privileged runner.
- Documentation is updated when behavior, packaging, or commands change.
- The commit message has no `Co-Authored-By` trailer.
