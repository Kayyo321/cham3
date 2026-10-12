# M1 verification on Codex Remote (heavy + native gates)

The fast M1 gates (static, naming, inert-state, package metadata/build + lintian,
desktop-file validation, panel/plasmoid structure, duplicate/deferred exclusion in
package relations, and the F1 per-profile staging dry-run) run in an unprivileged
amd64 `debian:trixie` container and in CI (`.github/workflows/m1_checks.yml`, on
push). This document gives the exact commands for the gates that were deliberately
**NOT run on the coordinator's arm64 laptop** — they need a privileged x86-64
build, a graphical VM, or native closure resolution, and emulated runs are too
slow to be useful. Run them on **Codex Remote** in the `Kayyo321/cham3`
environment, which clones the repository on native x86-64 Linux.

All commands assume the repository root as the working directory and the M1
candidate branch checked out (`task/M1-bootable-desktop`, pushed to `origin`).
A build/boot result proves only that packaging and boot run; it demonstrates no
M2–M4 security enforcement. Record every result as PASS/FAIL/NOT_RUN/BLOCKED with
the exact command, environment, and artifact identity.

## Network and the firmware/proxy blocker (read first)

The previous review BLOCKED because `firmware-b43-installer` /
`firmware-b43legacy-installer` download Broadcom blobs from GitHub/OpenWrt in the
build chroot. That is fixed: `build/live-desktop/auto/config` sets
`--firmware-chroot false` and ships an explicit self-contained firmware list, so
**the build contacts no firmware host**. The hosts the M1 build now contacts are
only: `deb.debian.org`, `security.debian.org`, `http.kali.org` (redirects to a
Kali mirror), and `repo.librewolf.net`. All four are reached over apt with
signature verification (apt-secure); the Kali transport is plain HTTP but the
index is signed. Ensure the disposable build environment can reach these four
hosts (supported proxy/CA, or an allowed-egress disposable VM). Do **not** bypass
network policy, disable TLS verification, or silently drop firmware.

## 0. Prerequisites (native x86-64 Debian 13, root available)

```sh
sudo apt-get update
sudo apt-get install -y --no-install-recommends $(grep -v '^#' build/host_packages.txt) desktop-file-utils
```

`build/host_packages.txt` lists `live-build`, `debootstrap`, `squashfs-tools`,
`xorriso`, `cpio`, `gnupg` (new: the M1 profile builds a local keyring at
`lb config`), and `qemu-system-x86`. live-build needs root and mounts `/proc` in
its chroot; run the image build only in a disposable environment (a privileged
container or a throwaway VM).

## 1. Fast gates (re-confirm on Remote; minutes)

```sh
build/scripts/run_static_checks.sh
tests/run_tests.sh static
tests/run_tests.sh staging          # F1 per-profile package selection (Docker-free)
tests/run_tests.sh packages         # builds cham3-base, cham3-desktop, cham3-full; lintian must exit 0
tests/run_tests.sh desktop          # M1 panel/plasmoid/package/naming/inert-state/foreign-source suites
tests/run_tests.sh container        # F3 dumpcap capability suite (needs root + CAP_NET_ADMIN; see §6)
```

Expected: all PASS (the `container` capability sub-cases need a privileged
container — see §6). `out/packages/` then holds the three `.deb` files and a
lintian record ending in exit status 0.

## 2. Real M0 default image build + boot (F1 regression — do this first)

F1 was: the default, unparameterized M0 build pulled the desktop suite and failed.
Confirm the regression is fixed — the default build must produce **only** the M0
baseline:

```sh
build/scripts/build_packages.sh
sudo build/scripts/build_image.sh           # no CHAM3_* selectors: the M0 profile
CHAM3_OUT_DIR="$PWD/out" tests/run_tests.sh image     # M0 image checks
# throwaway VM boot:
CHAM3_OUT_DIR="$PWD/out" tests/run_tests.sh boot
```

Expected: the M0 ISO builds and boots; `build.log` records
`staged local packages: cham3-base` only (no `cham3-desktop`/`cham3-full`). Record
the M0 ISO sha256. Also confirm native `m0_checks` is green (its "Build live image"
step was the failing CI signal last cycle).

## 3. Real M1 desktop ISO build (A2) — privileged x86-64

```sh
sudo env CHAM3_LIVE_DIR=build/live-desktop CHAM3_IMAGE_NAME=cham3-m1-desktop-amd64 \
     build/scripts/build_image.sh
CHAM3_OUT_DIR="$PWD/out" tests/run_tests.sh image-m1
```

Outputs in `out/image/`: `cham3-m1-desktop-amd64.iso`, `.packages` manifest,
`build.log`, `provenance.json`, `SHA256SUMS`. At `lb config` the profile builds a
local `cham3-archive-keyring` package from `build/live-desktop/keys/*.asc` (after
asserting each key's SHA-256 and primary fingerprint) and live-build installs it
before the first chroot `apt-get update`, so the Kali and LibreWolf keys are
trusted only via `Signed-By`. Record the ISO sha256 and the full manifest.

If the Remote container cannot run privileged live-build, build the ISO in a
nested disposable VM. Record which path was used, and whether KVM or TCG was used.

## 4. Graphical Plasma 6 Wayland boot (A3–A7) — QEMU + VNC screendump

Boot the live ISO in a throwaway VM with a virtual GPU and capture a real
screenshot (do not rely on the serial classifier, which only proves
boot-to-userspace):

```sh
qemu-system-x86_64 -machine q35 -accel kvm -cpu host -smp 4 -m 4096 \
  -device virtio-vga-gl -display vnc=127.0.0.1:0 \
  -cdrom out/image/cham3-m1-desktop-amd64.iso \
  -serial file:out/m1_serial.log -monitor unix:out/qmp.sock,server,nowait &
# after the live autologin reaches the Plasma Wayland session (SDDM autologs the
# live user into plasma.desktop via the 0086-cham3-sddm-wayland component):
printf 'screendump out/m1_desktop.ppm\n' | socat - unix-connect:out/qmp.sock
```

Confirm and capture evidence of:
- Session type Wayland and Plasma 6: `echo $XDG_SESSION_TYPE` is `wayland`,
  `plasmashell --version` reports 6.x, `ls /usr/share/wayland-sessions/` contains
  `plasma.desktop`.
- The default panel is a vertical dock on the **right** edge, thickness 56–64
  logical px, carrying the eight cham3 plasmoids in ADR-0002 order followed by
  the upstream system tray and a vertical digital clock.
- cham3 branding defaults: the `Cham3` wallpaper
  (`usr/share/wallpapers/Cham3/contents/images/3840x2160.png`) and the dark
  `Cham3Dark` color scheme.
- **F5 — categorized launcher:** the user-launcher flyout shows the installed
  applications grouped into categories (Development, Graphics, Internet, Office,
  System, Utilities) with a working search field (type "wire" → Wireshark), and
  launching an entry starts the installed binary. No contract badges or network
  pips (those are M5). `key_id: unavailable` is shown.
- Honest inert state: the backend-dependent plasmoids (alias wheel, tag pager,
  contract dock incl. ECR Explorer, network protocol launcher, kill switch, task
  manager pips, user `key_id`) visibly show unavailable/disconnected/unverified
  and perform no privileged action. The kill switch alters no firewall; the ECR
  Explorer says unavailable pending ADR 0001.
- Positive control: ordinary KDE works (Dolphin, Konsole, Kate, Spectacle,
  System Settings).

Save `out/m1_desktop.ppm` (convert to PNG) as the A3–A7 screenshot evidence.

## 5. Disk install and installed-system boot (A3, A4)

The image is built with `--debian-installer live`, so the boot menu offers an
install that copies the live system to disk.

```sh
qemu-img create -f qcow2 out/m1_install.qcow2 25G
qemu-system-x86_64 -machine q35 -accel kvm -cpu host -smp 4 -m 4096 \
  -device virtio-vga-gl -display vnc=127.0.0.1:0 \
  -drive file=out/m1_install.qcow2,if=virtio,format=qcow2 \
  -cdrom out/image/cham3-m1-desktop-amd64.iso -boot d
# after install, boot the installed disk (no CD):
qemu-system-x86_64 -machine q35 -accel kvm -cpu host -smp 4 -m 4096 \
  -device virtio-vga-gl -display vnc=127.0.0.1:0 \
  -drive file=out/m1_install.qcow2,if=virtio,format=qcow2
```

Confirm on the installed system: boots into Plasma 6 Wayland; the right-edge
panel/branding/inert states and the categorized launcher are present as above;
ordinary KDE works. Capture a screenshot. Record VM details and any limitation
(e.g. TCG instead of KVM).

## 6. Capture privilege — dumpcap fails closed, never setuid (F3)

On the installed/live system and in the `container` test suite. The design: a
`dpkg-statoverride` (root root 0750) is pinned on `/usr/bin/dumpcap` so
wireshark-common never applies its setuid fallback, and cham3-desktop's
postinst/trigger makes it `root:wireshark` 0750 with
`cap_net_admin,cap_net_raw=eip` — or, if setcap fails, no caps and no setuid
(capture unavailable, never root).

```sh
getcap /usr/bin/dumpcap            # cap_net_admin,cap_net_raw=eip  (when caps succeeded)
stat -c '%U %G %a' /usr/bin/dumpcap # root wireshark 750  — and NO setuid bit (not 4xxx)
dpkg-statoverride --list /usr/bin/dumpcap   # root root 750
```

Verify, with evidence:
- **No setuid on any path**, including after `dpkg-reconfigure wireshark-common`,
  after a `wireshark-common` upgrade/reinstall, and when a filesystem without
  capability xattrs forces setcap to fail (fail closed: no caps, no setuid).
- **Reconfigure retains valid capabilities:** `dpkg-reconfigure wireshark-common`
  with the statoverride present leaves the file untouched (upstream no-ops), so
  previously valid caps remain and no setuid appears. This is the accepted M1
  behavior (owner acceptance point): the security property is "never setuid"; a
  failed capability environment leaves capture unavailable, not root.
- **Member/nonmember capture (privileged container or VM):** a user in the
  `wireshark` group can capture (e.g. `dumpcap -D`), a non-member is denied
  (exit 126/EPERM). The image adds **no** user to the `wireshark` group by
  default. In an unprivileged container these sub-cases skip NOT_RUN (the Docker
  bounding set lacks `CAP_NET_ADMIN`); run them in a privileged container or the
  installed VM.

## 7. F2 native closure, signatures, pins, and image origin

Foreign-package inclusion was pre-authorized by the owner only via a narrowly
pinned, separately reviewed source (task scope amendment 2026-10-11). Included:
LibreWolf (own repo), and `zaproxy`, `exploitdb` (searchsploit), `ghidra`,
`amass`, `radare2` from Kali. Deferred (NOT present): `mitmproxy`, `wpscan`,
`metasploit-framework`. Omitted: Cutter/rizin (radare2 chosen), Burp (ZAP chosen).

**7a. Closure, signatures, pins (simulation; native solver).** Run the full
`F2-CLOSURE` gate in `docs/research/M1_readiness_packet.md` §6 (it fetches the
Kali key over HTTPS and checks SHA-256 `bbaef4b3…` + fingerprint
`827C8569F2518CC677FECA1AED65462EC8D5E4C5`; the LibreWolf key SHA-256
`cd1c45eb…` + fingerprint `662E3CDD6FE329002D0CA5BB40339DD82B12EF16`; configures
the strict `-1` pin + per-package allowlist; and runs `apt-get -s install` for
each candidate). Pass criteria: each included tool resolves with **no
un-allowlisted Kali package** pulled; each candidate is unavailable with no
allowlist entry; and the deferred tools (`mitmproxy`, `wpscan`,
`metasploit-framework`) do not resolve cleanly (base-library skew).

**7b. Origin preservation (simulation).** Run the `F2-ORIGIN` gate (strict pin,
`apt-get -s dist-upgrade`): **zero** Kali-origin `Inst` lines; `libc6`,
`libgcc-s1`, `libstdc++6`, `curl` stay Debian. The naive negative control (Kali
at priority 500, no allowlist) must show the opposite (base libraries migrating
to Kali) to prove the pin is doing the work.

**7c. Image origin (from the real M1 build).** The build's own simulation
(`apt-get -s install --no-install-recommends cham3-full` in the build chroot) and
`apt-get --print-uris` must show exactly the 8 allowlisted Kali packages
(`zaproxy, exploitdb, ghidra, amass, libpostal-data, radare2,
libradare2-6.0.0t64, libradare2-common`) plus `librewolf` from non-Debian
origins, and nothing else. Record from the build log that every Kali `Get:`/`Inst`
names one of those 8 and every LibreWolf one is `librewolf`. Note: a version-string
scan is insufficient (some Kali packages, e.g. `radare2 6.0.4+dfsg-1`, carry no
"kali" marker) — use the build-time apt origin, not the version string.

**7d. Final-image trust (from the booted/installed system).**
```sh
ls /usr/share/keyrings/cham3_kali_archive.gpg /usr/share/keyrings/cham3_librewolf.gpg
ls /etc/apt/trusted.gpg.d/                 # must contain NO cham3/Kali/LibreWolf key
grep -rl 'trusted=yes' /etc/apt/ || true   # must be empty in the final image
apt-get update                             # GOODSIG for Debian, Kali, LibreWolf; no NO_PUBKEY
```
The `.list` sources (`zzz_cham3_kali.list`, `zzz_cham3_librewolf.list`) each carry
`signed-by=` a dedicated keyring; the `zzz_` prefix is required so apt reads the
Debian sources first and Debian versions win.

## 8. Firmware (FW-B43)

- The M1 `.packages` manifest contains the self-contained firmware set
  (`firmware-brcm80211`, `firmware-iwlwifi`, `firmware-realtek`, …, 42 names) and
  **not** `firmware-b43-installer` or `firmware-b43legacy-installer`.
- The build log shows **no** connection to `github.com` or `downloads.openwrt.org`.
- `--firmware-binary true` is kept; verify the Debian-installer payload does not
  pull a b43 installer `.deb` (NOT_RUN previously). The intentional capability
  reduction (legacy b43/b43legacy SoftMAC 43xx Wi-Fi) is the recorded owner
  decision; modern Broadcom (`brcm80211`) remains.

## 9. What to record

For each gate: PASS/FAIL/NOT_RUN/BLOCKED, exact command, environment (arch, KVM
vs TCG, versions), the M0 and M1 ISO sha256 + manifests, screenshots, and
serial/install logs. State explicitly that no build or boot result is evidence of
any M2–M4 enforcement, and that the deferred tools (`mitmproxy`, `wpscan`,
`metasploit-framework`) are intentionally absent pending the separate, reviewed
source task in `docs/research/M1_readiness_packet.md`.
