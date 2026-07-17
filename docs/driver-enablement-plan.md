# Sirius: hardware detection + driver enablement

## Context

MacBook 8.1 (early 2015) and NVIDIA users hit broken hardware on LuminusOS: the
base OCI image lacks out-of-tree drivers, so the MacBook SPI keyboard/trackpad
(`macbook12-spi-driver`) and Bluetooth (`macbook12-bluetooth-driver`) don't work,
and NVIDIA GPUs fall back to nouveau. Goal: Sirius detects the machine's hardware
during install and enables the right drivers, **configurable** the same way the
rest of the installer is.

Hard constraints:

- LuminusOS is immutable bootc. Install is a `bootc install to-filesystem` of the
  embedded payload (`crates/sirius-installer/src/backend/adapter.rs:105`).
- The **live ISO has no `rpm-ostree`** (removed in `images/editions/core/Containerfile`),
  but it has `podman`, `dnf`, `bootc`. The payload is embedded as
  `containers-storage:@WORKSTATION_IMAGE@` in `/etc/sirius/distro.toml`.
- Driver packages come from **network repos at install** (RPMFusion / COPR), not
  bundled on the ISO. No new maintained image variants.

**Drivers are layered by the live installer itself** — the privileged runner
derives a per-machine OCI image from the payload (adding the driver
repos/packages) and `bootc install`s that derived image. No first-boot service.

## Persistence note (outside Sirius)

`bootc upgrade` pulls the registry base image, which reverts the install-time
driver layer. Re-enabling `rpm-ostree` on the installed system (drop the
`dnf -y remove rpm-ostree` in `images/editions/core/Containerfile`) lets the user
re-layer after upgrades; the robust long-term fix is maintained variant images.
This is a known tradeoff of "no maintained variants" — not required for the
install-time layering to work, only for surviving upgrades.

## Design

### 1. Hardware detection — `sirius-diag`

New pure, unit-testable module `crates/sirius-diag/src/hardware.rs` (exported from
`lib.rs`), in the style of `probes.rs`:

- **DMI:** read `/sys/class/dmi/id/{sys_vendor,product_name,product_family,board_name}`.
  Apple `sys_vendor` + `product_name == "MacBook8,1"` → MacBook profile.
- **PCI:** enumerate `/sys/bus/pci/devices/*/{vendor,class}` (no new crate dep).
  Vendor `0x10de` NVIDIA, `0x1002` AMD, `0x8086` Intel; display class `0x03xx`.
- Produce `HardwareFacts { dmi, pci: Vec<PciDevice> }` plus a matcher that returns
  which profiles apply to the detected hardware.

Surface in `sirius diag --json` for debugging (extend `report.rs` / `main.rs`).

### 2. Driver profiles — root-owned `distro.toml`

Profiles live in the root-owned descriptor (`/etc/sirius/distro.toml`), never
trusted from the unprivileged request — same boundary as the bootc image
(`adapter.rs:5-11`). Add a `[[driver_profile]]` array; structs in
`crates/sirius-installer/src/backend/distro.rs` (`DriverProfile`, `ProfileMatch`,
`ProfileRepo`):

```toml
[[driver_profile]]
id = "nvidia"
label = "NVIDIA proprietary"
description = "Akmod NVIDIA driver, blacklists nouveau"
match = { pci_vendor = ["0x10de"] }
repos = [{ id = "rpmfusion-nonfree", baseurl = "..." }]
packages = ["akmod-nvidia", "xorg-x11-drv-nvidia"]
kargs = ["rd.driver.blacklist=nouveau", "modprobe.blacklist=nouveau", "nvidia-drm.modeset=1"]

[[driver_profile]]
id = "macbook8_1"
label = "MacBook 8,1 (early 2015)"
description = "SPI keyboard/trackpad + Broadcom Bluetooth firmware"
match = { dmi_vendor = ["Apple*"], dmi_product = ["MacBook8,1"] }
repos = [{ id = "macbook-spi", copr = "owner/macbook12-spi-driver" }]
packages = ["akmod-applespi", "apple-bcm-firmware"]
kargs = []
```

Seed both in `images/editions/workstation/files/etc/sirius/distro.toml`. (Exact
COPR owners / package names verified during implementation.)

### 3. Wizard page — `pages/drivers.rs`

New configurable page `drivers`, placed after `partition`, before `summary`:

- On init, AppModel runs `sirius-diag` detection and resolves applicable profiles
  against the read-only `/etc/sirius/distro.toml` (display only).
- Shows detected hardware + a toggle row per matched profile (default ON for
  matches; user can override on/off, or enable a non-matched one).
- Emits `PageOutput::SetDrivers(Vec<profile_id>)`.
- Register in known-pages (`crates/sirius-diag/src/config.rs`), default order
  (`navigator.rs` / `app.rs`), `pages/mod.rs`, and add En + PtBr keys to `i18n.rs`.
  Skippable via `sirius.toml` `pages.disabled`, like the rest.
- `InstallConfig` (`config_model.rs`) gains `driver_profiles: Vec<String>`;
  `apply_page_output` stores it; gate is trivially true.

### 4. Request + privileged resolve — `adapter.rs`, `runner.rs`

- `InstallRequest` gains `driver_profiles: Vec<String>` (**ids only** — never raw
  repos/packages, preserving the privilege boundary).
- `runner.rs` loads the root-owned profiles, rejects unknown ids, resolves each id
  to `{repos, packages, kargs}`.
- `into_playbook`: append resolved `kargs` to the bootc kargs; the deployed image
  is the derived image from step 5 (runner overrides `imgref` to the derived
  `containers-storage:` ref instead of the base payload).

### 5. Install-time driver layering in the live installer (podman-derived image)

When any profile is selected, the privileged runner:

1. Generates a Containerfile in memory: `FROM containers-storage:<payload>`, add
   the resolved profile repo files, then
   `dnf -y install kernel-devel-<payload kernel> akmods <driver packages>`,
   `akmods --force` (build out-of-tree modules against the payload kernel),
   `dnf -y remove kernel-devel` + cleanup.
2. `podman build` it, committing a per-machine local image
   `containers-storage:luminusos-workstation:<tag>-<profilehash>`.
3. `bootc install to-filesystem --source-imgref containers-storage:<derived>` that
   image, with the resolved profile `kargs` appended.

The derived image is built by the trusted runner from the trusted payload + trusted
profiles — the unprivileged request still carries only profile ids, so the
privilege boundary holds.

Network: layering needs internet during install (the diagnostics `network` check
already warns). If offline and a profile is selected, the runner fails the install
step with a clear message rather than deploying a half-built image.

## Files

Create: `crates/sirius-diag/src/hardware.rs`,
`crates/sirius-installer/src/pages/drivers.rs`.
Modify: `crates/sirius-diag/src/{lib.rs,config.rs,report.rs}`,
`crates/sirius-installer/src/{config_model.rs,i18n.rs,app.rs,navigator.rs}`,
`crates/sirius-installer/src/pages/mod.rs`,
`crates/sirius-installer/src/backend/{distro.rs,adapter.rs,runner.rs}`,
`images/editions/workstation/files/etc/sirius/distro.toml` (seed profiles).
Optional (persistence): `images/editions/core/Containerfile` (keep rpm-ostree).

## Verification

- `cargo test` — new unit tests: DMI Apple → `macbook8_1` matches; PCI `0x10de` →
  `nvidia` matches; profile id resolution; `InstallRequest` JSON carries only ids
  (extend the existing `request_carries_no_image_or_repart_fields` test); runner
  rejects unknown profile id.
- `sirius diag --json` lists detected hardware.
- `sirius --dry-run` shows resolved packages/kargs and the generated Containerfile
  / podman build plan without building or writing to disk.
- `cargo clippy` + `cargo fmt --check`.
- VM: `just qemu iso`, run installer, toggle profiles, install; assert the derived
  `containers-storage:` image was built with the driver packages and the deployed
  system carries the modules + kargs. NVIDIA/MacBook module loading needs real
  hardware (MacBook 8,1) to fully confirm.
