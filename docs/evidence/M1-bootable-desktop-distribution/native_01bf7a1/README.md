# Native heavy-gate evidence — implementation target 01bf7a1

Target `01bf7a1f1066fae15ab51a56a5212c0a69780ab8` on `task/M1-bootable-desktop`
(baseline `401c6cf9a550c81decf832cbc346f6908b9c95ef`). Produced by Claude Code, the
implementation owner; it is not independent verification. Every gate below ran from
this exact commit in the declared environment of
[codex_remote_verification.md](../codex_remote_verification.md).

## Runs (all at 01bf7a1)

| Workflow | Run | Jobs and conclusions |
|---|---|---|
| `m1_image` | [38207816363](https://github.com/Kayyo321/cham3/actions/runs/38207816363) | plan success, f2_native success, m1_iso success, m1_vm success |
| `m1_checks` | [38207816376](https://github.com/Kayyo321/cham3/actions/runs/38207816376) | static_and_desktop success, package_build success, privileged_capture success |
| `m0_checks` | [38207816428](https://github.com/Kayyo321/cham3/actions/runs/38207816428) | static_checks success, package_build success, image_build success |

Environment (`vm/environment.txt`, `iso/environment.txt`): GitHub-hosted x86-64 runner,
kernel 6.17 (azure), inside `debian:trixie` (Debian 13), 4 CPUs, 16 GiB, KVM usable;
live-build 1:20250505+deb13u1, QEMU 1:10.0.13+ds-0+deb13u1, tesseract 5.5.0. Network:
the runner's ordinary outbound access (no proxy, no injected CA), TLS and apt
authentication on.

## Artifact identity

| Artifact | SHA-256 |
|---|---|
| `cham3-m1-desktop-amd64.iso` (3.6 GB, workflow artifact `m1_iso` of run 38207816363; not committed) | `f25bf75eb6357f76b787cd7a0630a457f7e30519438ba139ea1fa4e7ca4afd16` |
| `cham3-m1-desktop-amd64.packages` (`iso/`) | `d9a6cd26242fbf8409a16caaf8eb6fe7e08b57a30c1626d305152410717f7999` |
| `provenance.json` (`iso/`) | `c8c90a6c9f7e66670b5b2a2a0363bf8f755c83460f8bda15abb398926be9022e` |
| `build.log` (`iso/`) | `981c348935657b22aa28dfbe4ff02daad05f4b47f1d6363ff72b07178d8a3135` |

The VM phases record the ISO hash they booted (`vm/*/results.json`, `iso_sha256`);
it equals the value above. `SHA256SUMS` in this directory binds every committed file.

## Results

| Gate | Command (inside the declared container) | Result | Evidence |
|---|---|---|---|
| Static checks + suites | `build/scripts/run_static_checks.sh`; `tests/run_tests.sh static` / `desktop` / `staging` / `apt` (uid 1000) | PASS: static checks PASS; 82, 261, 16, 22 tests, 0 failures | `ci_logs/38207816376_static_and_desktop.log` |
| F6 native pin identity | `tests/run_tests.sh apt` | PASS, 22 (Origin/Label/Suite/Codename changed or removed: unlisted package -1, no candidate; superseded pins fail open as control; foreign-signed index rejected; host binding control) | same log |
| Packages + lintian | `build/scripts/build_packages.sh`; `tests/run_tests.sh packages` | PASS: lintian exit 0; 8 package tests | `ci_logs/38207816376_package_build.log` |
| F3/F8 privileged capture | `CHAM3_REQUIRE_PRIVILEGED_CAPTURE=1 sh tests/container/in_container.sh` (privileged) | PASS, 77 tests, 0 skipped | `privileged_capture/privileged_container.log` |
| M0 regression (F1) | `build/scripts/build_image.sh`; `tests/run_tests.sh image` (17); `run_boot_test.py --accel auto`; `tests/run_tests.sh boot` (18) | PASS (login prompt on serial) | `ci_logs/38207816428_image_build.log` |
| F2 native source gates | `python3 tests/native/f2_source_gates.py --repo . --packages out/packages --out out/f2_native` | PASS, 7/7 | `f2_native/results.json` and transcripts |
| M1 ISO + image-m1 | `bash build/scripts/ci_m1_heavy.sh iso` | PASS: ISO built; image-m1 54 tests, 0 failures; no firmware fetch in the build log | `iso/` |
| Live session | `run_m1_vm_checks.py live` | PASS, 37/37 | `vm/live/` |
| Disk installation | `run_m1_vm_checks.py install` | PASS (completed and powered off, 330 s, KVM) | `vm/install/` |
| Installed system | `run_m1_vm_checks.py installed` | PASS, 65/65 | `vm/installed/` |

### F2 native source gates (real Debian, Kali and LibreWolf archives)

- `signatures`: update with `Error-Mode=any` exit 0, no signature error, keys only
  `cham3_kali_archive.gpg` and `cham3_librewolf.gpg` under `/usr/share/keyrings`,
  none in global trust.
- `identity` (recorded only): Kali InRelease Origin `Kali`, no Label, Suite/Codename
  `kali-rolling`; LibreWolf Origin and Label `LibreWolf Software Repository`,
  Suite/Codename `librewolf`.
- `policy`: 9 945 packages available only from a foreign host; 9 936 have no
  candidate; 0 escaped; the 9 with candidates are exactly the 8 Kali allowlist names
  (from `http.kali.org`) and `librewolf` (from `repo.librewolf.net`); the unlisted
  probes have no candidate.
- `closure_uri`: 1 045 downloads for installing the built cham3-desktop and
  cham3-full: deb.debian.org 1 002, security.debian.org 34, http.kali.org 8 (exactly
  the allowlist), repo.librewolf.net 1 (`librewolf`); all six foreign roles present.
- `control_deny`: deny stanzas only → all six roles exit 100. `control_leak`: without
  the cham3 pins the same closure downloads 1 155 un-allowlisted Kali packages
  (including libc6).
- `upgrade`: radare2, libradare2-6.0.0t64, libradare2-common (Kali) and librewolf
  really installed; dist-upgrade downloads nothing from a foreign host outside the
  allowlists; `ghidra-data` still exit 100.

### Image (image-m1, 54 tests)

Contents and M1 contract; image apt policy with host-bound pins; no `trusted=yes`
in the final image sources (the installer stage's live-medium entry is removed);
foreign keys only via Signed-By, global trust limited to Debian's own
`debian-archive-keyring` files; retained-index origin audit (no un-allowlisted
foreign-origin package); manifest required/forbidden packages; checksums and
provenance; installer payload: no `firmware-b43-installer` or
`firmware-b43legacy-installer` in the ISO pool, `/firmware` or `Contents-firmware`,
all 42 reviewed firmware packages in the pool and linked under `/firmware`, the same
42 in the root manifest; `apt-mirror-setup` and `choose-mirror-bin` in `/pool-udeb`;
`user-setup`, `live-config` and `sudo` in the live root; dumpcap in the squashfs
without set-id bits, root:wireshark, no access for others; the `root root 750`
statoverride registered; `install-setuid` answered false.

### Live session (37/37, no network)

Boot menu screenshot; serial login as the live user; suite and roles installed;
42+ firmware packages, no b43 installer; keys only via Signed-By, no `trusted=yes`;
apt policy over the retained indices (unlisted Kali probes no candidate, roles from
the expected hosts, libc6 from Debian); dumpcap 750 root:wireshark with only
`cap_net_admin,cap_net_raw=eip` and the statoverride; member captured a packet,
nonmember exit 126; autologin session `Type=wayland` with `kwin_wayland`; running panel
(from plasmashell `evaluateScript`) on the right edge, vertical, 60 thick, the ten
components in ADR 0002 order; cham3 wallpaper and look-and-feel; launcher OCR shows
seven menu categories and `key_id` unavailable; a search for "wireshark" and Enter
started a real `wireshark` process; the panel strip shows the widgets' unavailable
captions; the five backend-dependent widgets (alias wheel, tag pager, contract dock,
network protocol launcher, kill switch) each show an unavailable state when opened,
against a clean baseline. Screenshots `vm/live/live_*.png`, OCR text `vm/live/ocr_*.txt`.

### Installation and installed system (65/65, network)

The ISO's own debian-installer (live mode) with the test-only preseed installed to a
fresh disk and powered off. Installed system: live packages removed; Debian sources
written by the installer (trixie main, trixie-security main, trixie-updates main,
plus cham3's contrib file); the same system, apt, firmware and dumpcap checks;
`apt-get update` authenticated; upgrade origin by download host clean; `ghidra-data`
not installable; dumpcap lifecycle: `dpkg-reconfigure wireshark-common` keeps policy,
reinstall from security.debian.org with `setcap` missing → the cham3 trigger fails
closed (no capabilities, member capture refused, nonmember 126),
`dpkg-reconfigure cham3-desktop` restores capabilities and member capture; the mirror
had no newer wireshark-common, so the installed-system upgrade step was not
applicable (a real version change is covered by the privileged suite); SDDM greeter
screenshot, password login, then the same Wayland, panel, branding, launcher and
inert-state checks. Screenshots `vm/installed/installed_*.png`; the installer's syslog
tail `vm/installed/installer_syslog_tail.txt`.

## Limits

Evidence about this image in this virtual machine (q35, BIOS/SeaBIOS, virtio-vga,
software rendering, 1280x800): not real hardware, not UEFI. OCR reads rendered text
and can miss words; a missed word fails a check, never passes it. The test preseed's
account is a throwaway VM credential. Nothing here is evidence of M2–M4 enforcement,
anonymity, contract-verified launch or any security service; the panel widgets are
inert by design. This evidence was produced by the implementation owner and is not an
independent review.
