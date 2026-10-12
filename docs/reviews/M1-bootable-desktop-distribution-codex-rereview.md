# Independent Review: M1 — Repaired bootable desktop distribution

- **Reviewer / role / provider:** Codex / independent REVIEWER and SECURITY REVIEWER / OpenAI. Claude Code retains implementation ownership. No product implementation was changed.
- **Task specification:** `docs/tasks/M1-bootable-desktop-distribution.md`, including the owner's 2026-10-11 sourcing, deferral, and firmware amendments.
- **Base commit:** `401c6cf9a550c81decf832cbc346f6908b9c95ef`.
- **Reviewed commit:** `0fec3a01c18b95fc238d2168ed0d97e0d3831865`.
- **Documentation tip inspected:** `6f38fc3b86a83dda9913719215639ac3f40771db`; its only difference from the target is the handoff's target line. It was not substituted for the implementation target.
- **Prior review:** `b82919c6ddbdb1f84f8db35e3ded8559c40b242f`, CHANGES_REQUESTED. The requested prior-review file is absent from this candidate. It was recovered from `review/M1-codex-b82919c` at `cde8039a92a1ede38c5dece16ecaa8d6abcb65c4`; a copy is in the evidence directory. The referenced Codex chat was independently read.
- **Worktree / environment:** `/workspace/M1-rereview`, branch `review/M1-codex-0fec3a0`, checked out at the exact target throughout verification. Codex Remote host: Debian 13.6, native x86-64, kernel 6.18.44. Disposable privileged container `codex-m1-rereview`: Debian 13.7 amd64; fast/package gates uid 1000, image/security probes root. Separate native `debian:trixie` container `codex-m1-f2` for source gates. Docker image identities and exact commands are recorded in the evidence README. `/dev/kvm` absent; M0 boot used QEMU 10.0.13 TCG. Review date: **2026-10-11 America/New_York**; transcripts use UTC on 2026-10-12.
- **Review scope:** Baseline-to-target and prior-to-repaired implementation, surrounding build/test code, F1–F5, supply-chain authentication/pins and failure behavior, dumpcap lifecycle/capture, firmware scheduling, panel/launcher/inert-state source, and required native/build/boot/install gates. Read handoff, acceptance matrix, Remote verification recipe, research packet, AGENTS, PROJECT, workflow/testing/security/role guidance, STATUS, task, approved ADR 0002 and proposed ADR 0001. The current owner amendments control over stale research statements saying the sources or decisions remain unapproved.

## Findings

### F6 — High, security: default-deny foreign pins fail open when signed Release identity differs

- **Location:** `build/live-desktop/config/archives/cham3_kali.pref:16` and `cham3_librewolf.pref:6`; the corresponding `.list` files authenticate a repository key but do not enforce these Release field values.
- **Trigger:** A fresh build/first index load receives an otherwise valid signed repository whose `Origin` is different from `Kali`, or whose LibreWolf `Codename` differs from `librewolf`. This is a conditional design defect, not an observed change in the real upstream archives.
- **Requirement:** The owner permits only eight Kali names and `librewolf`, with every other foreign package denied, and requires fail-closed sourcing and independent security approval.
- **Confirmed evidence:** An isolated native APT reproduction used newly generated disposable signing keys, `Signed-By`, valid signed Release files, empty APT roots, and the candidate preferences unchanged. Expected metadata makes the unlisted `codex-unlisted` package priority `-1` with no candidate. Changing only Kali `Origin` to `KaliNew`, or LibreWolf `Codename` to `librewolf-new`, makes it priority `500` with candidate `9.0`. APT update succeeds. No network access, installation, authentication bypass, or actual Kali/LibreWolf key was used by this probe. See `pin_metadata_probe.py` and `.log`.
- **Reason:** Both the deny and allow rules stop matching; APT's ordinary repository priority remains 500. `Signed-By` authenticates who signed metadata, not the required `Origin`/`Codename` text. Existing installations may reject a change against previously cached Release identity; fresh builds have no such prior identity and remain exposed.
- **Required correction:** Bind the default deny to each configured source independently of mutable Release identity, or enforce the expected identity before any solver/install can use its index. Retain the eight-name/one-name allowlists and add native signed-metadata negative controls. Re-run real closure, upgrade, URI/origin, and image-trust gates. The F2 pinned-source design is **not security-approved**.

### F3 — Medium, security, reopened: first installation can still leave setuid dumpcap before cham3 configuration

- **Location:** `packaging/cham3-desktop/debian/postinst:16` and `:92`, with `debian/triggers:4`; no preinst is shipped. The override is first established by cham3-desktop's postinst.
- **Trigger:** `wireshark-common` configures before cham3-desktop on a first installation while `setcap` fails, and configuration is interrupted or cham3-desktop has not yet configured. The source comment explicitly acknowledges this ordering gap.
- **Requirement:** F3 requires no setuid on any installation, upgrade, reconfigure, or capability-failure path, and failure must not leave root capture fallback active.
- **Confirmed evidence:** In the disposable privileged system, unpack both the exact candidate control-script harness and authenticated Debian `wireshark-common`, then configure wireshark-common with an injected failing `/usr/sbin/setcap`. The result is **root:wireshark `04754`, no capabilities, no statoverride**, while cham3-desktop is `install ok unpacked`. Configuring cham3-desktop afterward clears it to `0750`. Thus the successful final postinst repairs the file, but does not prevent the earlier state or make an interrupted install safe. See `initial_install_probe.py` and `.log`. The harness retains the built candidate's scripts/triggers unchanged and removes KDE dependencies/payload; it is not a complete desktop installation.
- **Verified improvement:** With the override already established, actual 4.4.18→4.4.19 upgrade and `dpkg-reconfigure wireshark-common` preserve no-setuid protection and the intended capabilities. Missing/failing-setcap reinstall scenarios clear capabilities without setuid. A separate member captured one real loopback packet; a nonmember was denied after a shell dropped credentials.
- **Additional failure handling:** `postinst:96` logs but continues if registering the protective override fails. The next upstream reconfigure is then unprotected. This path must fail safely rather than report successful policy setup.
- **Required correction:** Ensure the protective mechanism exists before upstream can configure dumpcap on every supported first-install ordering, including interrupted configuration; verify it was successfully installed. Add an initial-install/interruption negative control, then repeat upgrade/reconfigure/failure/capture and installed-system gates. The F3 mechanism is **not independently security-approved** as an all-path design.

### F7 — Medium: installer-side firmware selection still admits both b43 installers

- **Location:** `build/live-desktop/auto/config:92` (`--firmware-binary true`) together with `:86` (contrib archive enabled).
- **Trigger:** live-build's Debian-installer stage independently builds its firmware package payload. Disabling chroot firmware autodetection and listing 42 chroot packages does not restrict this separate selector.
- **Requirement:** The firmware amendment excludes `firmware-b43-installer` and `firmware-b43legacy-installer` while retaining brcm80211 and the declared self-contained coverage.
- **Confirmed evidence:** Native live-build `1:20250505+deb13u1`, `installer_debian-installer:497–560`, calls `Firmware_List_From_Contents` with the configured trixie archive areas and downloads the resulting names individually. Independently executing that exact installed selector against current Debian Contents indices yields **44 names**, including both b43 installers and `firmware-brcm80211`. Repeating the installer's authenticated download-only command in the partial M1 chroot retrieves both excluded `.deb` files. Their SHA-256 values and selector input/output are preserved in `firmware_binary_download_probe.log`, `firmware_binary_probe.sh`, and `installer_firmware_packages.txt`.
- **Limit:** This establishes actual native scheduling/download behavior, not their presence in a finished ISO: the M1 build was blocked earlier by foreign-source network denial. No Broadcom blob or external firmware host was contacted by these probes, and no firmware installer postinst was executed.
- **Required correction:** Exclude both names from the installer payload as well as the live root, preserving the approved self-contained firmware set. Verify the full ISO pool/installer payload in addition to the rootfs manifest; do not silently drop other firmware.

### F8 — Medium: the privileged capture gate uses invalid execution and output-directory controls

- **Location:** `tests/container/test_m1_container_capture.py:127`, `:132`, `:233` and `:373`.
- **Trigger:** Run the required capture suite in a native privileged Debian 13 container (util-linux 2.41.5).
- **Requirement:** Security verification must prove both working member capture and actual nonmember denial; environment skips must describe genuine unavailable prerequisites.
- **Confirmed evidence:** The privileged suite runs 51 tests, fails `test_install_nonmember_cannot_execute_dumpcap`, and skips two packet cases. Repeating its direct root-started `setpriv ... /usr/bin/dumpcap -D` returns 0, while `setpriv ... sh -c 'exec /usr/bin/dumpcap -D'` returns 126 with permission denied. The shell performs the executable access after dropping identity. Also, `tempfile.mkdtemp` creates a root-owned `0700` output directory; its member capture fails to open the pcap with permission denied, which `require_capture()` mislabels as a container that cannot capture. A member-writable `/tmp` path succeeds with one packet and 436 bytes in this same container. See `harness_diagnostics.py/.log` and `independent_capture.log`.
- **Required correction:** Execute the tested helper only after credentials are dropped, provision an output directory accessible to the member, and distinguish output-path failures from unavailable capture capability. The privileged gate must run its positive/negative capture cases without these skips. The ordinary Docker wrapper's exit 0 with four skips is not privileged capture approval.

## Verification and limits

Commands, artifact identities, transcripts, and probe scripts: [codex_0fec3a0 evidence](../evidence/M1-bootable-desktop-distribution/codex_0fec3a0/README.md).

| Check | Result | Independent evidence / exact scope |
|---|---|---|
| Exact target/base, clean implementation tree, documentation tip diff | PASS | Verified base ancestry; verification checkout at `0fec3a0`; tip differs only in handoff documentation. |
| Fast static checks and unit suites | PASS | `build/scripts/run_static_checks.sh`; `tests/run_tests.sh static` (82), `staging` (16), `desktop` (224), zero skips. |
| Three unsigned package builds, lintian and package tests | PASS | `build/scripts/build_packages.sh`, lintian exit 0; `tests/run_tests.sh packages` (8). Package SHA-256 identities preserved. |
| Real unparameterized M0 image build, F1 regression | PASS | `build/scripts/build_image.sh` as root with no CHAM3 selectors. Stages only `cham3-base`; ISO sha256 **`3074795cc19effff09d86bca0f71a6d101d981e6e83184c37dda664f2cccbb08`**. Final manifest contains no cham3-desktop/full. |
| M0 image and real TCG BIOS boot | PASS | `tests/run_tests.sh image` (17); actual `run_boot_test.py --accel tcg` reaches serial login in 52.4 s; `tests/run_tests.sh boot` (18). Machine pc, 2 CPUs, 1024 MiB, SeaBIOS, no disk/network. |
| Native CI corroboration at documentation tip | PASS | [m0_checks 38195543252](https://github.com/Kayyo321/cham3/actions/runs/38195543252) and [m1_checks 38195543207](https://github.com/Kayyo321/cham3/actions/runs/38195543207) show Success for `6f38fc3`. Public run summaries inspected; remote artifacts not downloaded or independently hashed. |
| Exact M1 real ISO build | BLOCKED | Required M1 selectors; wrapper exit 1 / lb build 100. Supported proxy returns 403 for Kali InRelease and LibreWolf HTTPS CONNECT, even with session proxy/CA configured in disposable chroot and TLS verification enabled. No ISO, final manifest, or provenance was produced. |
| `tests/run_tests.sh image-m1` | FAIL | 36 tests: 28 fixture checks pass, 8 artifact-dependent failures due to missing M1 build artifacts. These missing-artifact failures are distinct from F4's repaired path constants. |
| M1 graphical Plasma 6 Wayland boot | BLOCKED | Explicit QEMU q35/TCG/max/4 CPU/4096 MiB/virtio-vga/VNC attempt exits 1 because the M1 ISO is absent. No session-type evidence or screenshot. Non-GL virtio-vga was selected for TCG/headless VNC. |
| M1 disk installation | BLOCKED | Created disposable 25 GiB qcow2; explicit TCG installation launch exits 1 because ISO is absent. `qemu-utils` was additionally needed for qemu-img. No installer ran. |
| Installed-system boot, desktop screenshot and capture | BLOCKED | No disk installation exists. Required installed-system checks were not executed; no success is inferred from container behavior. |
| F2-CLOSURE native signatures/solver/context gates | BLOCKED | Exact section 6.1 script runs in native disposable Debian; exits 22 at proxy-denied `https://archive.kali.org/archive-key.asc`. No CLEAN/LEAKY/control/M1-context result is claimed. |
| F2-ORIGIN native strict/allowlisted/naive gates | BLOCKED | Dedicated native attempt installs the candidate keyring and source/pin files, but mandatory authenticated-index update exits 100 on 403. Strict/allowlisted/naive simulations cannot be meaningfully run without the indices. No empty-index zero-Kali PASS. |
| F2 real-image closure/URI/origin audit | BLOCKED | No real M1 image. Retained indices are configured, but actual installed allowlist closure and absence of unlisted Kali packages are not established. |
| F2 key construction and partial-chroot trust placement | PASS | Actual lb config verifies both armored hashes and primary fingerprints; generated package puts keys only in `/usr/share/keyrings`, none in global trust. Real initial chroot installs it before update. This is not final-image trust or GOODSIG for blocked foreign origins. Build-only local `packages.list` still uses trusted=yes; final cleanup was not reached. |
| F2 pin fail-closed negative controls | FAIL | Signed synthetic metadata reproduction confirms F6. This is native APT policy behavior, not upstream closure evidence. |
| F3 ordinary Docker wrapper | PASS | `tests/run_tests.sh container`, 51 tests, four skips. Those skipped checks do not pass. |
| F3 privileged suite | FAIL | 51 tests, one failed nonmember execution assertion and two skipped capture cases (F8). Most final-state/reconfigure/failure checks pass. |
| F3 protected real upgrade/reconfigure and independent packet controls | PASS | Actual 4.4.18→4.4.19 upgrade, valid caps after reconfigure; member captures one packet, nonmember exit 126. Exact shipped scripts in dependency-free harness, no full KDE or disk-installed system. |
| F3 initial/interrupted installation with failing setcap | FAIL | Actual dpkg ordering exposes `04754` before cham3 configuration (F3). |
| F4 packaged session/wallpaper path repair | PASS | Constants now name `usr/share/wayland-sessions/plasma.desktop` and `usr/share/wallpapers/Cham3/contents/images/3840x2160.png`; negative fixture controls remain. Actual M1 ISO gate BLOCKED. |
| F5 launcher and inert-state source checks | PASS | Categorized Kicker models/search replace hardcoded utilities; source uses standard entry triggering and absent contract badges. Rendered live/installed categories, search and real application launch remain BLOCKED. |
| Firmware explicit 42-package chroot design | PASS | Excludes both b43 installers; retains brcm80211. Build stopped before installing this set; no final rootfs coverage claim. |
| Firmware installer-side exclusion | FAIL | Native selector/download-only reproduction admits both b43 installers (F7). Full ISO payload audit BLOCKED. |
| Candidate `git diff --check` | FAIL | Exit 2 for whitespace in retained research transcripts and an EOF blank line; recorded separately from runtime/security defects. |

F1 is independently closed by the real M0 build/boot. F2's earlier role omissions are addressed at the dependency/source-definition level under the explicit owner amendment; real inclusion/origin remains BLOCKED and F6 prevents source-design approval. F3 improves steady-state behavior but remains open for the initial-install gap. F4's source/test mismatch is corrected, with real-image verification blocked. F5's source repair is reasonable, with live and installed menu/launch acceptance blocked.

No build, boot, source review, or packet-capture probe is evidence of M2–M4 enforcement. No cham3 backend or contract badges were introduced; ADR 0001 remains proposed. This review supplies neither rendered inert-state confirmation nor a security approval of mixed-source inclusion. Deferred mitmproxy, wpscan and metasploit-framework are accepted scope deferrals, not findings requiring their inclusion.

## Verdict

**CHANGES_REQUESTED** for `0fec3a01c18b95fc238d2168ed0d97e0d3831865`.

Required actions for Claude Code: repair F3 and F6–F8, preserve the approved firmware/suite scope, reconcile the evidence packet with the new results, and resubmit an exact immutable implementation target. Provide a declared disposable environment whose supported network policy permits the required repository/key hosts and any declared Kali redirects; keep TLS and apt signature verification enabled. Then complete real M1 ISO/image, graphical Wayland/panel/branding/inert-state/categories/launch, disk install, installed-system boot/capture, and native F2 closure/origin/trust evidence, repeating affected M0 regression gates.

**Independent security approval is withheld for both F2 and F3. No merge, READY_TO_MERGE, or milestone-complete approval is granted.** Network recovery alone does not resolve the confirmed code/test findings.
