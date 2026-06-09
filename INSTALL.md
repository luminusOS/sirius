# Installing Sirius into a LuminusOS ISO

| Build artifact / data file | Install path |
|----------------------------|--------------|
| `target/release/sirius` | `/usr/bin/sirius` |
| `data/dev.luminusos.Sirius.policy` | `/usr/share/polkit-1/actions/` |
| `data/dev.luminusos.Sirius.desktop` | `/usr/share/applications/` |
| `data/luminus.toml` | `/etc/sirius/luminus.toml` |
| `data/repart.d/*.conf` | `/usr/share/sirius/repart.d/` |
| `data/sirius.toml` (page toggles, optional) | `/etc/sirius/sirius.toml` |

The pkexec action id is `dev.luminusos.Sirius.run-playbook`; the policy pins the
executable to `/usr/bin/sirius` with first argument `run-playbook`. The unprivileged
UI spawns `pkexec /usr/bin/sirius run-playbook` and pipes the install request as JSON
to its stdin; the privileged process streams progress JSON back on stdout.

## Runtime requirements on the target/live system
`systemd-repart`, `bootc`, `cryptsetup` (for encrypted installs), `pkexec`/polkit, and `mount`.
