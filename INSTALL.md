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
to its stdin; the privileged process streams progress JSON back on stdout.

Each distribution supplies its own `distro.toml` (bootc/OCI image + repart dir) and
`repart.d/*.conf` partition layout.

Optional diagnostics policy in `/etc/sirius/sirius.toml`:

```toml
[diagnostics]
require = ["uefi", "ram", "disk_space"]
warn = ["secure_boot", "network", "virt"]
min_ram_gib = 2
```

## Runtime requirements on the target/live system
`systemd-repart`, `bootc`, `cryptsetup` (for encrypted installs), `pkexec`/polkit, and `mount`.
