# M1 verification on Codex Remote (heavy gates)

The fast M1 gates (static, naming, inert-state, package metadata/build + lintian,
desktop-file validation, panel/plasmoid structure, duplicate/GATED exclusion in
package relations) were run locally in an unprivileged amd64 `debian:trixie`
container and are also wired into CI (`.github/workflows/m1_checks.yml`, runs on
push). This document gives the exact commands for the **heavy gates that were
deliberately NOT run on the coordinator's arm64 laptop** (they require either a
privileged x86-64 build or a graphical VM, and emulated runs are too slow to be
useful here). Run them on **Codex Remote** in the `Kayyo321/cham3` environment,
which clones the repository on native x86-64 Linux.

All commands assume the repository root as the working directory and the M1
candidate branch checked out (`task/M1-bootable-desktop`, pushed to `origin`).
A build/boot result proves only that packaging and boot run; it demonstrates no
M2-M4 security enforcement. Record every result as PASS/FAIL/NOT_RUN/BLOCKED
with the exact command, environment, and artifact identity.

## 0. Prerequisites (native x86-64 Debian 13, root available)

```sh
sudo apt-get update
sudo apt-get install -y --no-install-recommends $(grep -v '^#' build/host_packages.txt) desktop-file-utils
```

`build/host_packages.txt` already lists `live-build`, `debootstrap`,
`squashfs-tools`, `xorriso`, `cpio`, and `qemu-system-x86`. live-build needs root
and mounts `/proc` in its chroot, so run the image build in a disposable
environment (a privileged container or a throwaway VM), never on a host you care
about.

## 1. Fast gates (re-confirm on Remote; minutes)

```sh
build/scripts/run_static_checks.sh
tests/run_tests.sh static
tests/run_tests.sh packages          # builds cham3-base, cham3-desktop, cham3-full; lintian must exit 0
tests/run_tests.sh desktop           # M1 panel/plasmoid/package/naming/inert-state suites
```

Expected: all PASS. `out/packages/` then holds the three `.deb` files and
`lintian.txt` ending in `# lintian exit status: 0`.

## 2. Real desktop ISO build (A2) — privileged x86-64, ~tens of minutes

```sh
build/scripts/build_packages.sh
sudo env CHAM3_LIVE_DIR=build/live-desktop CHAM3_IMAGE_NAME=cham3-m1-desktop-amd64 \
     build/scripts/build_image.sh
CHAM3_OUT_DIR="$PWD/out" tests/run_tests.sh image-m1
```

Outputs in `out/image/`: `cham3-m1-desktop-amd64.iso`, `.packages` manifest,
`build.log`, `provenance.json`, `SHA256SUMS`. The `image-m1` suite checks the
manifest contains `cham3-desktop`, `cham3-full`, the KDE core and the curated
suite; that the excluded duplicates and GATED tools are absent transitively; and
that the rootfs carries the Plasma Wayland session, the eight plasmoids, the
look-and-feel, color scheme and wallpaper, with no `usr/bin/cham3-*` backend
binary and no Kali apt source. Record the ISO sha256 and the manifest.

If the Remote container cannot run privileged live-build (no mount/chroot),
build the ISO in a nested disposable VM, or trigger the privileged image build
path the way M0 did on an ephemeral runner. Record which path was used.

## 3. Graphical Plasma 6 Wayland boot (A3, A4, A5, A6) — QEMU + VNC screendump

Boot the live ISO in a throwaway VM with a virtual GPU and capture a real
screenshot of the session (do not rely on the serial classifier, which only
proves boot-to-userspace, not a graphical session):

```sh
qemu-system-x86_64 -machine q35 -accel kvm -cpu host -smp 4 -m 4096 \
  -device virtio-vga-gl -display vnc=127.0.0.1:0 \
  -cdrom out/image/cham3-m1-desktop-amd64.iso \
  -serial file:out/m1_serial.log -monitor unix:out/qmp.sock,server,nowait &
# wait for the live autologin to reach the Plasma Wayland session (sddm autologs
# the live 'user' into plasma.desktop via the 0086-cham3-sddm-wayland component),
# then capture the framebuffer through the QEMU monitor:
printf 'screendump out/m1_desktop.ppm\n' | socat - unix-connect:out/qmp.sock
```

Confirm and capture evidence of:
- Session type Wayland and Plasma 6: in the VM, `echo $XDG_SESSION_TYPE` is
  `wayland` and `plasmashell --version` reports 6.x. (Use `loginctl show-session`
  for the seat if a terminal is reachable.)
- The default panel is a vertical dock on the **right** edge, thickness 56-64
  logical px, carrying the eight cham3 plasmoids in ADR-0002 order followed by
  the upstream system tray and a vertical digital clock.
- cham3 branding defaults: the Cham3 wallpaper and dark Cham3 color scheme.
- Honest inert state: the backend-dependent plasmoids (alias wheel, tag pager,
  contract dock incl. the ECR Explorer, network protocol launcher, kill switch,
  task manager pips, user `key_id`) visibly show unavailable/disconnected/
  unverified and perform no privileged action. The kill switch must not alter
  any firewall; the ECR Explorer must say it is unavailable pending ADR 0001.
- Positive control: ordinary KDE still works — launch Dolphin, Konsole, Kate,
  Spectacle, System Settings from the launcher.

Save `out/m1_desktop.ppm` (convert to PNG) as the A3-A6 screenshot evidence.

## 4. Disk install and installed-system boot (A3, A4) — Debian installer

The image is built with `--debian-installer live`, so the boot menu offers an
install that copies the live system (including the cham3 packages) to disk.

```sh
qemu-img create -f qcow2 out/m1_install.qcow2 25G
# Boot the ISO with the install target disk attached and run the installer
# (graphical or text). Preseed it for an unattended run if preferred.
qemu-system-x86_64 -machine q35 -accel kvm -cpu host -smp 4 -m 4096 \
  -device virtio-vga-gl -display vnc=127.0.0.1:0 \
  -drive file=out/m1_install.qcow2,if=virtio,format=qcow2 \
  -cdrom out/image/cham3-m1-desktop-amd64.iso -boot d
# After install completes, boot the installed disk (no CD):
qemu-system-x86_64 -machine q35 -accel kvm -cpu host -smp 4 -m 4096 \
  -device virtio-vga-gl -display vnc=127.0.0.1:0 \
  -drive file=out/m1_install.qcow2,if=virtio,format=qcow2
```

Confirm on the installed system: it boots into Plasma 6 Wayland, the right-edge
panel/branding/inert states are present as above, and ordinary KDE functions.
Capture a screenshot. Record VM details and any limitation (e.g. if KVM is
unavailable and TCG was used, note the emulation).

## 5. Capture privilege (dumpcap) and launchers (A11)

On the installed/live system:
- `getcap /usr/bin/dumpcap` shows `cap_net_raw,cap_net_admin=eip` and
  `stat -c '%G' /usr/bin/dumpcap` is `wireshark` (group-restricted capture, not
  root; the image preseeds `wireshark-common/install-setuid` to the non-setuid
  answer). No user is added to the `wireshark` group by default.
- `desktop-file-validate` passes on installed `.desktop` entries and curated-suite
  launchers appear under their menu categories and start the installed binary.

## 6. What to record

For each gate above: PASS/FAIL/NOT_RUN/BLOCKED, exact command, environment
(arch, KVM vs TCG, versions), ISO sha256 + manifest, screenshots, and serial/
install logs. Note explicitly that no build or boot result is evidence of any
M2-M4 enforcement, and that the GATED tools (LibreWolf, ghidra,
metasploit-framework, exploitdb/searchsploit, zaproxy, wpscan, Cutter, radare2,
rizin, mitmproxy, amass) are intentionally absent pending the separate source
review in `docs/research/M1_readiness_packet.md`.
