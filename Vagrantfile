# -*- mode: ruby -*-
# vi: set ft=ruby :

require "fileutils"
require "shellwords"

ROOT = __dir__
VAGRANT_STATE = File.join(ROOT, ".vagrant")
PAYLOAD = File.join(VAGRANT_STATE, "sirius-payload")
DISK_POOL = File.join(VAGRANT_STATE, "disks")
GUEST_PAYLOAD = "/home/vagrant/.local/share/sirius-payload"

profile = ENV.fetch("SIRIUS_VM_PROFILE", "default")
unless %w[default low-ram no-tpm secure-boot].include?(profile)
  raise "SIRIUS_VM_PROFILE must be default, low-ram, no-tpm, or secure-boot"
end

disk_profile = ENV.fetch("SIRIUS_DISK_PROFILE", "normal")
unless %w[normal small multiple none].include?(disk_profile)
  raise "SIRIUS_DISK_PROFILE must be normal, small, multiple, or none"
end

FileUtils.mkdir_p(PAYLOAD)
FileUtils.mkdir_p(DISK_POOL)

secure_boot = profile == "secure-boot"
firmware_suffix = secure_boot ? ".secboot" : ""
firmware_code = "/usr/share/edk2/ovmf/OVMF_CODE#{firmware_suffix}.fd"
firmware_vars_template = "/usr/share/edk2/ovmf/OVMF_VARS#{firmware_suffix}.fd"
unless File.exist?(firmware_code) && File.exist?(firmware_vars_template)
  raise "edk2-ovmf firmware is required: #{firmware_code}"
end

# The box references firmware below ~/.vagrant.d/boxes. SELinux confines QEMU
# and denies that user_home_t path even with qemu:///session. Use the host's
# packaged read-only code and a VM-local, writable copy of its matching NVRAM.
firmware_vars = File.join(
  DISK_POOL,
  secure_boot ? "sirius-secure-boot-vars.fd" : "sirius-vars.fd",
)
FileUtils.cp(firmware_vars_template, firmware_vars) unless File.exist?(firmware_vars)
FileUtils.chmod(0o644, firmware_vars)
system("chcon", "-t", "svirt_home_t", firmware_vars, out: File::NULL, err: File::NULL)

stage_payload = <<~SHELL
  set -euo pipefail
  cd #{Shellwords.escape(ROOT)}
  cargo build --bin sirius

  payload=.vagrant/sirius-payload
  install -Dm755 target/debug/sirius "$payload/usr/bin/sirius"
  install -Dm644 data/io.sirius.Installer.policy \
    "$payload/usr/share/polkit-1/actions/io.sirius.Installer.policy"
  install -Dm644 data/io.sirius.Installer.desktop \
    "$payload/usr/share/applications/io.sirius.Installer.desktop"
  install -Dm644 data/sirius.toml "$payload/etc/sirius/sirius.toml"
  install -Dm644 data/images/welcome-banner.png \
    "$payload/usr/share/sirius/welcome-banner.png"
  install -Dm644 data/images/timezone-map.svg \
    "$payload/usr/share/sirius/timezone-map.svg"
  install -Dm644 data/images/timezone-pin.png \
    "$payload/usr/share/sirius/timezone-pin.png"
  install -Dm644 data/repart.d/10-esp.conf \
    "$payload/usr/share/sirius/repart.d/10-esp.conf"
  install -Dm644 data/repart.d/20-root.conf \
    "$payload/usr/share/sirius/repart.d/20-root.conf"

  if [ -f data/locale/pt_BR/LC_MESSAGES/sirius.mo ]; then
    install -Dm644 data/locale/pt_BR/LC_MESSAGES/sirius.mo \
      "$payload/usr/share/locale/pt_BR/LC_MESSAGES/sirius.mo"
  fi
SHELL

install_sirius = <<~SHELL
  set -euo pipefail

  payload=#{Shellwords.escape(GUEST_PAYLOAD)}
  if ! test -x "$payload/usr/bin/sirius" ||
     ! test -f "$payload/usr/share/sirius/welcome-banner.png" ||
     ! test -f "$payload/usr/share/sirius/timezone-map.svg"; then
    echo "The persistent Sirius payload is not available in this VM."
    echo "Run 'vagrant provision' once (without --provision-with) to upload it."
    exit 0
  fi

  command -v bootc >/dev/null || {
    echo "This box does not provide bootc; update the Silverblue box." >&2
    exit 1
  }

  if ! test -w /usr; then
    # The default development unlock is writable and discarded on reboot.
    # `--transient` is intentionally not used: Fedora mounts that variant
    # read-only unless every write happens in a separate mount namespace.
    ostree admin unlock
  fi

  install -Dm755 "$payload/usr/bin/sirius" /usr/bin/sirius
  install -Dm644 \
    "$payload/usr/share/polkit-1/actions/io.sirius.Installer.policy" \
    /usr/share/polkit-1/actions/io.sirius.Installer.policy
  install -Dm644 \
    "$payload/usr/share/applications/io.sirius.Installer.desktop" \
    /usr/share/applications/io.sirius.Installer.desktop
  sed -i 's/^Icon=.*/Icon=system-software-install-symbolic/' \
    /usr/share/applications/io.sirius.Installer.desktop
  install -Dm644 "$payload/etc/sirius/sirius.toml" \
    /etc/sirius/sirius.toml
  install -Dm644 \
    "$payload/usr/share/sirius/welcome-banner.png" \
    /usr/share/sirius/welcome-banner.png
  install -Dm644 \
    "$payload/usr/share/sirius/timezone-map.svg" \
    /usr/share/sirius/timezone-map.svg
  install -Dm644 \
    "$payload/usr/share/sirius/timezone-pin.png" \
    /usr/share/sirius/timezone-pin.png
  install -Dm644 \
    "$payload/usr/share/sirius/repart.d/10-esp.conf" \
    /usr/share/sirius/repart.d/10-esp.conf
  install -Dm644 \
    "$payload/usr/share/sirius/repart.d/20-root.conf" \
    /usr/share/sirius/repart.d/20-root.conf

  if test -f \
    "$payload/usr/share/locale/pt_BR/LC_MESSAGES/sirius.mo"; then
    install -Dm644 \
      "$payload/usr/share/locale/pt_BR/LC_MESSAGES/sirius.mo" \
      /usr/share/locale/pt_BR/LC_MESSAGES/sirius.mo
  fi

  install -d -m755 /etc/sirius
  cat >/etc/sirius/distro.toml <<'EOF'
  [bootc]
  image = "docker://ghcr.io/luminusos/luminusos-workstation:44"
  target_imgref = "ghcr.io/luminusos/luminusos-workstation:44"
  enforce_sigpolicy = false
  kargs = ["console=tty0", "console=ttyS0,115200n8"]

  [disk]
  repart_dir = "/usr/share/sirius/repart.d"

  [branding]
  name = "LuminusOS"
  icon = "system-software-install-symbolic"
  welcome_button = "Install {name}"
  welcome_banner = "/usr/share/sirius/welcome-banner.png"
  EOF

  cat >/etc/profile.d/sirius-vagrant.sh <<'EOF'
  export SIRIUS_TEST_DISK=/dev/disk/by-id/virtio-SIRIUS_TARGET
  EOF

  # Keep the graphical test runner immediately usable after every boot.
  # These settings are intentionally limited to the disposable Vagrant VM.
  sed -i \
    -e '/^AutomaticLoginEnable=/d' \
    -e '/^AutomaticLogin=/d' \
    /etc/gdm/custom.conf
  sed -i '/^[[]daemon[]]$/a AutomaticLogin=vagrant' /etc/gdm/custom.conf
  sed -i '/^[[]daemon[]]$/a AutomaticLoginEnable=True' /etc/gdm/custom.conf

  install -d -o vagrant -g vagrant -m700 /home/vagrant/.config
  touch /home/vagrant/.config/gnome-initial-setup-done
  chown vagrant:vagrant /home/vagrant/.config/gnome-initial-setup-done

  # dconf database names become D-Bus object path components, so they must not
  # contain hyphens. Clean up the name used by earlier lab revisions.
  install -d -m755 /etc/dconf/profile
  touch /etc/dconf/profile/user
  sed -i '/^system-db:sirius-vagrant$/d' /etc/dconf/profile/user
  rm -f \
    /etc/dconf/db/sirius-vagrant \
    /etc/dconf/db/sirius-vagrant.d/00-session \
    /etc/dconf/db/sirius-vagrant.d/locks/00-session
  rmdir \
    /etc/dconf/db/sirius-vagrant.d/locks \
    /etc/dconf/db/sirius-vagrant.d \
    >/dev/null 2>&1 || true

  install -d -m755 \
    /etc/dconf/db/sirius_vagrant.d \
    /etc/dconf/db/sirius_vagrant.d/locks
  grep -qxF 'user-db:user' /etc/dconf/profile/user ||
    sed -i '1i user-db:user' /etc/dconf/profile/user
  grep -qxF 'system-db:sirius_vagrant' /etc/dconf/profile/user ||
    printf '%s\n' 'system-db:sirius_vagrant' >>/etc/dconf/profile/user

  shell_version="$(gnome-shell --version | awk '{print $NF}')"
  cat >/etc/dconf/db/sirius_vagrant.d/00-session <<EOF
  [org/gnome/desktop/lockdown]
  disable-lock-screen=true

  [org/gnome/desktop/screensaver]
  lock-enabled=false
  idle-activation-enabled=false

  [org/gnome/desktop/session]
  idle-delay=uint32 0

  [org/gnome/settings-daemon/plugins/power]
  sleep-inactive-ac-type='nothing'
  sleep-inactive-ac-timeout=0
  sleep-inactive-battery-type='nothing'
  sleep-inactive-battery-timeout=0

  [org/gnome/shell]
  welcome-dialog-last-shown-version='$shell_version'
  EOF

  cat >/etc/dconf/db/sirius_vagrant.d/locks/00-session <<'EOF'
  /org/gnome/desktop/lockdown/disable-lock-screen
  /org/gnome/desktop/screensaver/lock-enabled
  /org/gnome/desktop/screensaver/idle-activation-enabled
  /org/gnome/desktop/session/idle-delay
  /org/gnome/settings-daemon/plugins/power/sleep-inactive-ac-type
  /org/gnome/settings-daemon/plugins/power/sleep-inactive-ac-timeout
  /org/gnome/settings-daemon/plugins/power/sleep-inactive-battery-type
  /org/gnome/settings-daemon/plugins/power/sleep-inactive-battery-timeout
  /org/gnome/shell/welcome-dialog-last-shown-version
  EOF
  dconf update

  update-desktop-database /usr/share/applications >/dev/null 2>&1 || true
  systemctl try-restart polkit.service >/dev/null 2>&1 || true
  echo "Sirius is installed. GNOME logs in automatically without its tour or screen lock."
SHELL

wifi_setup = <<~SHELL
  set -euo pipefail

  if ! rpm -q hostapd iw >/dev/null 2>&1; then
    rpm-ostree install hostapd iw
    echo "Wi-Fi tools staged. Run: vagrant reload, then provision wifi again."
    exit 0
  fi

  modprobe mac80211_hwsim radios=3
  install -d -m700 /etc/sirius-vagrant/wifi

  cat >/etc/sirius-vagrant/wifi/open.conf <<'EOF'
  interface=wlan0
  driver=nl80211
  ssid=Sirius Open
  hw_mode=g
  channel=1
  EOF

  cat >/etc/sirius-vagrant/wifi/wpa2.conf <<'EOF'
  interface=wlan1
  driver=nl80211
  ssid=Sirius WPA2
  hw_mode=g
  channel=6
  wpa=2
  wpa_key_mgmt=WPA-PSK
  wpa_passphrase=sirius-test
  rsn_pairwise=CCMP
  EOF

  cat >/etc/sirius-vagrant/wifi/wpa3.conf <<'EOF'
  interface=wlan2
  driver=nl80211
  ssid=Sirius WPA3
  hw_mode=g
  channel=11
  wpa=2
  wpa_key_mgmt=SAE
  sae_password=sirius-test
  ieee80211w=2
  rsn_pairwise=CCMP
  EOF

  for network in open wpa2 wpa3; do
    systemctl stop "sirius-hostapd-${network}.service" >/dev/null 2>&1 || true
    systemd-run --unit="sirius-hostapd-${network}" \
      /usr/sbin/hostapd "/etc/sirius-vagrant/wifi/${network}.conf"
  done
SHELL

Vagrant.configure("2") do |config|
  config.vm.box = "gnome-shell-box/silverblue44"
  config.vm.box_version = "2026.7.0"
  config.vm.hostname = "sirius-vagrant"
  config.vm.boot_timeout = 900

  # Vagrant builds on the Fedora 44 host, uploads only the runtime payload, and
  # installs it into a disposable writable Silverblue /usr overlay.
  config.trigger.before [:up, :reload, :provision] do |trigger|
    trigger.name = "Build Sirius"
    trigger.info = "Building Sirius and staging the VM payload..."
    trigger.run = {
      inline: "bash -lc #{Shellwords.escape(stage_payload)}",
    }
  end

  config.vm.synced_folder ".", "/vagrant", disabled: true

  config.vm.provision "file",
    source: PAYLOAD,
    destination: GUEST_PAYLOAD

  config.vm.provision "shell",
    name: "sirius",
    run: "always",
    privileged: true,
    inline: install_sirius

  # Optional and intentionally explicit because the first invocation layers
  # packages into Silverblue and therefore needs a reboot.
  config.vm.provision "shell",
    name: "wifi",
    run: "never",
    privileged: true,
    inline: wifi_setup

  config.vm.provider :libvirt do |libvirt|
    libvirt.cpus = 4
    libvirt.memory = profile == "low-ram" ? 1536 : 8192
    libvirt.machine_type = "q35"
    libvirt.graphics_type = "spice"
    libvirt.video_type = "virtio"
    libvirt.video_accel3d = true
    libvirt.video_vram = 128
    libvirt.qemu_use_agent = true
    # vagrant-libvirt only auto-creates a missing pool when it is named
    # "default". Point that provider-managed pool at the project-local state
    # directory instead of requiring users to define a libvirt pool manually.
    libvirt.storage_pool_name = "default"
    libvirt.storage_pool_path = DISK_POOL
    libvirt.disk_driver cache: "writeback", discard: "unmap"
    libvirt.loader = firmware_code
    libvirt.nvram = firmware_vars

    unless profile == "no-tpm"
      libvirt.tpm_model = "tpm-crb"
      libvirt.tpm_type = "emulator"
      libvirt.tpm_version = "2.0"
    end

    unless disk_profile == "none"
      size = disk_profile == "small" ? "8G" : "64G"
      libvirt.storage :file,
        path: "target.qcow2",
        size: size,
        type: "qcow2",
        bus: "virtio",
        cache: "writeback",
        discard: "unmap",
        serial: "SIRIUS_TARGET"
    end

    if disk_profile == "multiple"
      libvirt.storage :file,
        path: "extra.qcow2",
        size: "48G",
        type: "qcow2",
        bus: "sata",
        cache: "writeback",
        discard: "unmap",
        serial: "SIRIUS_EXTRA"
    end
  end
end
