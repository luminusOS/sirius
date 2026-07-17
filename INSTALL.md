# Installing Sirius into a distribution ISO

Sirius is distro-agnostic. To ship it in a live ISO, install the binary, the
polkit/desktop integration, and the distribution's descriptor + repart layout.

| Build artifact / data file | Install path |
|----------------------------|--------------|
| `target/release/sirius` | `/usr/bin/sirius` |
| `data/io.sirius.Installer.policy` | `/usr/share/polkit-1/actions/` |
| `data/io.sirius.Installer.desktop` | `/usr/share/applications/` |
| `data/distro.toml` (per-distribution) | `/etc/sirius/distro.toml` |
| `data/repart.d/*.conf` (per-distribution) | `/usr/share/sirius/repart.d/` |
| `data/sirius.toml` (page toggles, optional) | `/etc/sirius/sirius.toml` |

The pkexec action id is `io.sirius.Installer.run-playbook`; the policy pins the
executable to `/usr/bin/sirius` with first argument `run-playbook`. The unprivileged
UI spawns `pkexec /usr/bin/sirius run-playbook` and pipes the install request as JSON
to its stdin; the privileged process streams progress JSON back on stdout. The
privileged process loads the bootc image and repart layout from the root-owned
`/etc/sirius/distro.toml` itself — the request carries only the user's choices.
If the UI is already running as root, pkexec is skipped.

The default policy requires admin authentication (`auth_admin`). pkexec exits
with status 127 when no polkit authentication agent is available in the session
— a kiosk/live session must either run an agent (e.g. GNOME Shell's, enabled via
`"polkitAgent"` in the shell mode's `components`) or ship a polkit rule that
grants `io.sirius.Installer.run-playbook` to the live user.

Each distribution supplies its own `distro.toml` (bootc/OCI image + repart dir) and
`repart.d/*.conf` partition layout. Note: any options after `:` in a repart
`MountPoint=` are passed **raw to the `mount(2)` syscall** by libreadymade —
use only kernel mount options for that filesystem. Userspace keywords like
`defaults`, `auto`, or `nofail` make the mount fail with EINVAL. `distro.toml` may also declare up to three
optional `[[bento]]` link cards (title/desc/link/icon) shown on the install
progress page — website, help, contribute links, as in Readymade — and an
optional `[branding]` section (`logo` image path, or themed `icon` name) for the
welcome page; see the commented examples in `data/distro.toml`. Bento `icon`
names must exist in the live system's icon theme (ship custom ones under
`/usr/share/icons/hicolor/scalable/actions/`); missing names fall back to a
generic link glyph.

Optional diagnostics policy in `/etc/sirius/sirius.toml`:

```toml
[diagnostics]
require = ["uefi", "ram", "disk_space"]
warn = ["secure_boot", "network", "virt"]
min_ram_gib = 2
```

The canonical page id for disk selection and automatic/manual partitioning is
`storage`. Older configurations that list `disk`, `partition`, or
`manual_partition` are migrated in memory to one `storage` page. The `network`
page is automatically omitted when NetworkManager reports no Wi-Fi device.

## Runtime requirements on the target/live system
`systemd-repart`, `bootc`, `cryptsetup` (for encrypted installs), `pkexec`/polkit,
`mount`, `lsblk`, `udisks2`, and `NetworkManager`.

The live user must be allowed to request NetworkManager scans/connections. Disk
mutations never run in the UI process: the confirmed `PartitionPlan` crosses the
existing pkexec boundary and the root runner applies it through UDisks2 before
passing the resulting mounts to libreadymade's manual provisioner.
