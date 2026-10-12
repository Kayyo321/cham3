# M1 heavy and native verification (declared environment and recipe)

The previous re-review (Codex, `0fec3a0`) found the heavy and native M1 gates
BLOCKED on Codex Remote: its supported proxy returned HTTP 403 for
`http.kali.org` and `repo.librewolf.net`. Network recovery alone does not resolve
code findings, and the policy must not be bypassed. This repair cycle therefore
declares a different disposable native x86-64 environment whose ordinary network
access reaches every required host, and runs the gates there from the exact commit
under review. The gates are scripted so that Codex (or anyone) can re-run them
unchanged, there or on Codex Remote once its network policy admits the hosts.

A build or boot result is evidence about packaging, boot and the checked desktop
state only. It demonstrates no M2–M4 security enforcement.

## Declared environment

| Property | Value |
|---|---|
| Hosts | GitHub-hosted `ubuntu-latest` runners: ephemeral native x86-64 VMs, discarded after each job |
| Userland under test | `debian:trixie` (Debian 13) containers; the image and VM jobs start them `--privileged` (live-build chroot/mounts, QEMU) |
| Virtualisation | QEMU from Debian 13 inside the container; KVM when the runner exposes `/dev/kvm` (recorded per run), otherwise TCG |
| Network | The runner's ordinary outbound access: `deb.debian.org`, `security.debian.org`, `http.kali.org` and the Kali mirrors it redirects to, `repo.librewolf.net` (HTTPS, verified). No proxy, no injected CA, no `trusted=yes`, apt authentication on |
| Secrets | None. The install VM uses the throwaway account in `build/vm/m1_install.preseed` |
| Definition | `.github/workflows/m1_image.yml` (heavy, runs on non-documentation pushes to `task/M1-bootable-desktop`, and by hand once on the default branch) and `.github/workflows/m1_checks.yml` (fast + privileged capture, every push); `.github/workflows/m0_checks.yml` (M0 regression: image build and boot) |

Each run records `environment.txt` (commit, kernel, `/etc/os-release`,
architecture, KVM, CPUs, memory, disk) and tool versions beside its evidence.

## Gates and where they run

| Gate | Command (repository root, inside the declared container) | Job |
|---|---|---|
| Static and unit suites | `build/scripts/run_static_checks.sh`; `tests/run_tests.sh static`, `desktop`, `staging`, `apt` (uid 1000) | `m1_checks/static_and_desktop` |
| F6 native pin identity | `tests/run_tests.sh apt`: real apt-get/apt-cache, signed synthetic repositories, Release identity variants, sensitivity/authentication/host-binding controls; no network | `m1_checks/static_and_desktop` |
| Packages + lintian | `build/scripts/build_packages.sh`; `tests/run_tests.sh packages` | `m1_checks/package_build` |
| F3/F8 privileged capture | `CHAM3_REQUIRE_PRIVILEGED_CAPTURE=1 sh tests/container/in_container.sh` (privileged; a missing capability is a FAIL, nothing may skip) | `m1_checks/privileged_capture` |
| M0 regression (F1) | `build/scripts/build_image.sh`; `tests/run_tests.sh image`; `build/vm/run_boot_test.py`; `tests/run_tests.sh boot` | `m0_checks/image_build` |
| F2 native source gates | `python3 tests/native/f2_source_gates.py --repo . --packages out/packages --out out/f2_native` (root, network) | `m1_image/f2_native` |
| M1 ISO + image-m1 + payload | `bash build/scripts/ci_m1_heavy.sh iso` (builds packages and `CHAM3_LIVE_DIR=build/live-desktop CHAM3_IMAGE_NAME=cham3-m1-desktop-amd64 build/scripts/build_image.sh`, runs `tests/run_tests.sh image-m1` including `test_m1_image_payload.py`, checks the build log for firmware fetches, lists the ISO pool and `/firmware`) | `m1_image/m1_iso` |
| Live, install, installed | `bash build/scripts/ci_m1_heavy.sh vm` → `python3 build/vm/run_m1_vm_checks.py live|install|installed` | `m1_image/m1_vm` |

To re-run on another disposable native x86-64 Debian 13 machine with the same
network reach, run the same commands as root in a privileged `debian:trixie`
container from a clean checkout of the target commit:

```sh
docker run --rm --privileged --volume "$PWD:/work" --workdir /work debian:trixie bash build/scripts/ci_m1_heavy.sh iso
docker run --rm --privileged --device /dev/kvm --volume "$PWD:/work" --workdir /work debian:trixie bash build/scripts/ci_m1_heavy.sh vm
docker run --rm --volume "$PWD:/work" --workdir /work debian:trixie bash -c \
  'apt-get update && apt-get install -y --no-install-recommends $(grep -v "^#" build/host_packages.txt) desktop-file-utils && build/scripts/build_packages.sh && python3 tests/native/f2_source_gates.py --repo . --packages out/packages --out out/f2_native'
```

(`--device /dev/kvm` only where KVM exists; without it QEMU uses TCG and the VM
phases take much longer.)

## What each heavy gate checks

**F2 native source gates** (`tests/native/f2_source_gates.py`; the apt
configuration is the candidate's, unchanged: keyring package built from the
reviewed keys, `config/archives/*.list` and `*.pref`, and the Debian sources
live-build writes into the image):

- `signatures`: `apt-get -o APT::Update::Error-Mode=any update` succeeds with no
  GPG/NO_PUBKEY error; the two keys exist only under `/usr/share/keyrings`.
- `identity`: records the Origin/Label/Suite/Codename the real InRelease files
  carry. The pins do not depend on them (F6).
- `policy` (python3-apt, every package): no package whose candidate is available
  only from `http.kali.org` or `repo.librewolf.net` is outside that host's
  allowlist; every allowlisted name has a candidate; unlisted Kali-only probes
  (`ghidra-data`, `kali-defaults`, `kali-linux-core`, `burpsuite`,
  `metasploit-framework`) have none.
- `closure_uri`: `apt-get --print-uris` for installing the built `cham3-desktop`
  and `cham3-full` in a fresh container. Origin is attributed by download host:
  only the four approved hosts appear; Kali downloads are allowlisted names only;
  LibreWolf is `librewolf` only; all six foreign roles are present.
- `control_deny`: with only the default-deny stanzas the Kali-only roles and
  `librewolf` have no candidate. `control_leak`: without the cham3 pins the same
  closure downloads un-allowlisted Kali packages (sensitivity).
- `upgrade`: `radare2` (with its Kali libraries) and `librewolf` are really
  installed; `dist-upgrade` then downloads nothing from a foreign host outside
  the allowlists, and `ghidra-data` still has no candidate.

**Image** (`ci_m1_heavy.sh iso`): the real M1 ISO; `image-m1` (contents, apt
policy and final-image trust, retained-index origin audit, manifest exclusions,
checksums) plus `test_m1_image_payload.py`: no `firmware-b43-installer` /
`firmware-b43legacy-installer` in the ISO pool, `/firmware` or
`Contents-firmware`; all 42 reviewed firmware packages in the pool and linked
under `/firmware`; the same 42 in the root file system manifest; in the squashfs,
dumpcap without set-id bits, root:wireshark and no access for others, the
`root root 750` statoverride registered and `install-setuid` answered false. The
build log is searched for the b43 fetch markers.

**VM** (`build/vm/run_m1_vm_checks.py`; q35, BIOS, virtio-vga, USB tablet,
serial console and QMP; every check PASS/FAIL/BLOCKED in `results.json`, with
screenshots, OCR text, the serial transcript and the QEMU command line):

- `live` (no network): boot menu screenshot; serial login as the live user;
  installed suite and roles; firmware set and no b43 installer; apt trust (keys
  only via Signed-By, no `trusted=yes`); apt policy over the retained indices
  (unlisted Kali denied, roles available, base libraries from Debian); dumpcap
  mode, group, capabilities and statoverride; member capture and nonmember denial;
  the autologin session's type (`loginctl`) and the `kwin_wayland` compositor; the
  running panel read from plasmashell (`m1_panel_state.js` through
  `org.kde.PlasmaShell.evaluateScript`: right edge, vertical, 56–64 thick, the ten
  components in ADR 0002 order, cham3 wallpaper); look-and-feel; screenshots of
  the desktop, the categorized launcher (OCR: menu categories and `key_id`
  unavailable), a launcher search for Wireshark and the launched window
  (`pgrep wireshark` after Enter); and each backend-dependent widget opened on
  screen with OCR requiring an unavailable/unverified/disconnected state that was
  not visible before the click.
- `install`: the ISO's own debian-installer (live mode) with the test preseed
  appended to a copy of its initrd; whole-disk install to a fresh qcow2; the
  machine must power off cleanly.
- `installed` (network): serial login as the preseeded account; live packages
  removed; the same system, apt and dumpcap checks; `apt-get update` (signed),
  upgrade origin by download host, `ghidra-data` not installable; dumpcap
  lifecycle (reinstall, `dpkg-reconfigure wireshark-common`, reinstall with
  `setcap` missing → fail closed and member capture denied, `dpkg-reconfigure
  cham3-desktop` → capabilities and member capture restored); SDDM greeter
  screenshot, password login, then the same Wayland/panel/launcher/inert-state
  checks.

## Recording

The evidence directory for each target holds the downloaded workflow artifacts
(`results.json`, logs, screenshots, OCR text), the run URLs and job conclusions,
and SHA-256 sums of the ISO and of every committed evidence file. A gate whose
job did not run is NOT_RUN; an environment that lacks a prerequisite is BLOCKED;
nothing is inferred from a neighbouring gate.
