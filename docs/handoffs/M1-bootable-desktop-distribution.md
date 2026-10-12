# Handoff: M1 — Bootable desktop distribution (repair cycle 2)

- **State:** IN_PROGRESS (repair cycle 2). Repaired candidate for independent Codex **re-review** after CHANGES_REQUESTED on `0fec3a0`. Not approved; not merged; M1 is not complete.
- **Role:** Claude Code (Opus 5.5), implementation owner and coordinator/integrator. In this cycle the owner asked for the repairs to be coded directly rather than through the three subagents, so `m1_worker`, `m1_test_reviewer` and `m1_docs_research` were not used; their definitions remain in `.claude/agents/`. Independent REVIEWER and SECURITY REVIEWER: Codex (OpenAI).
- **Model/provider:** Claude Opus 5.5 / Anthropic.
- **Working directory / branch:** `/Users/jessicabruce/cham3`, branch `task/M1-bootable-desktop` (pushed to `origin`). The earlier contributor worktrees are unchanged and unused this cycle.
- **Task specification:** `docs/tasks/M1-bootable-desktop-distribution.md`.
- **Baseline commit:** `401c6cf9a550c81decf832cbc346f6908b9c95ef`.
- **Prior reviewed commit:** `0fec3a01c18b95fc238d2168ed0d97e0d3831865` — Codex **CHANGES_REQUESTED** (`review/M1-codex-0fec3a0` at `026d606`: `docs/reviews/M1-bootable-desktop-distribution-codex-rereview.md`, evidence `docs/evidence/M1-bootable-desktop-distribution/codex_0fec3a0/`).
- **Implementation target (resubmit):** `01bf7a1f1066fae15ab51a56a5212c0a69780ab8` on `origin/task/M1-bootable-desktop` (see "Implementation target" below).

Product authority: `PROJECT.md` §§8, 12, 13, 18; approved ADR 0002; SEC-01–18. ADR 0001 remains PROPOSED (ECR Explorer reserved, shown unavailable). Owner-approved sourcing, deferrals and firmware scope are preserved.

## Completed work

### Finding dispositions (Codex re-review of `0fec3a0`)

| # | Finding | Disposition | Change |
|---|---|---|---|
| F6 | Foreign default-deny pins fail open when signed Release identity differs | FIXED | `cham3_kali.pref` / `cham3_librewolf.pref`: every stanza is `Pin: origin "http.kali.org"` / `"repo.librewolf.net"`, which apt matches against the host of the configured source URI, never against Release fields. Allowlists unchanged (8 Kali names; `librewolf` only). Static policy requires host-bound pins, rejects Release-field pins and any source of a pinned repository on another host (`tests/lib/m1_apt_support.py`). New native suite `tests/apt/test_m1_apt_pin_identity.py` (`tests/run_tests.sh apt`): real apt-get/apt-cache, signed synthetic repositories served for the real host names, Origin/Label/Suite/Codename changed or removed; sensitivity (the superseded pins fail open), authentication and host-binding controls |
| F3 | First-install setuid gap; override failure only warned | FIXED | New `cham3-desktop` **preinst** registers the `root root 0750` statoverride at unpack, before wireshark-common can be configured, clears pre-existing set-id bits, and fails (aborting the unpack) if the override cannot be registered or an existing override has a set-id mode. The postinst clears set-id bits first and now **fails** instead of warning. The image preseeds `wireshark-common/install-setuid` **false**, so wireshark-common never chooses the set-user-id layout during the image build in any order |
| F7 | Installer payload still selects the b43 installers | FIXED | `contrib` is no longer an `lb config` archive area (live-build's installer firmware selector scans those; in Debian 13 contrib's only firmware candidates are the two b43 installers). contrib (torbrowser-launcher) comes from the new signed `config/archives/cham3_debian_contrib.list`. Static replay of the selector over the recorded Contents rows gives exactly the 42 reviewed names (control with contrib: 44); `test_m1_image_payload.py` audits the real ISO pool, `/firmware`, `Contents-firmware` and the root manifest |
| F8 | Privileged capture gate used invalid controls | FIXED | dumpcap is executed only after credentials are dropped (setpriv starts a shell that executes it); captures go to a member-owned directory; an output-path failure is a harness error; `CHAM3_REQUIRE_PRIVILEGED_CAPTURE=1` (privileged runner mode) makes a missing capability a FAIL. New scenarios: both dpkg unpack orders with interrupted runs, apt-ordered install, preseed-false ordering, a found setuid dumpcap, preinst/postinst override failures, a set-id override, lifecycle and a real version change |
| F1, F4, F5 | Independently closed or reasonable at `0fec3a0` | PRESERVED | No change to per-profile staging; F4 constants unchanged; launcher source unchanged. Live/installed launcher categories, search and real launch now verified natively (see Verification) |

### Defects found by the new native heavy gates and repaired

| Found by | Defect | Repair |
|---|---|---|
| ISO build (run 38200250071) | `firmware-ipw2x00` and `firmware-ivtv` refuse to install without a debconf licence answer, which live-build supplied only under `--firmware-chroot true` | **Owner decision 2026-10-12**: preseed acceptance for the development image (`config/preseed/cham3_firmware_licenses.cfg.chroot`), keeping all 42 firmware packages; revisit before any public release. Static check added |
| ISO build (run 38200777689) | live-build's installer stage chooses package names with `Check_package_available`, which requires exactly one `apt-cache show` stanza; with the Kali index present it picked the nonexistent `libfuse3-3` | `build/live-desktop/auto/build` runs the `lb build` stages one by one and moves the Kali/LibreWolf source and pin files out of `config/archives` for the installer stage only (restored on failure). That stage uses a fresh chroot and restores the live chroot, so the image's sources, pins and indices are unchanged; the installer payload comes from Debian only. Static check added |
| image-m1 (run 38201524750) | The installer stage prepends `deb [trusted=yes] file:/run/live/medium …` to the live root's sources.list (for in-session installers) | `auto/build` removes that entry and its copied indices from the live root before `lb binary`. The image check accepts in `/etc/apt/trusted.gpg.d` only Debian's own archive keys owned by `debian-archive-keyring`; any other key (Kali, LibreWolf, look-alike) still fails |
| VM (scratch run 38203029813) | No live account: live-config creates it only when `user-setup` is installed, a Recommends skipped by `--apt-recommends false`; the live image booted to the SDDM greeter | `config/package-lists/cham3_live.list.chroot_live` installs `user-setup` in the live pass only (removed by the installer). Static and image checks added |
| Repository hygiene | `git diff --check` against the baseline failed on byte-exact captured evidence and one EOF blank line | `.gitattributes` declares `docs/research/evidence/**` and `docs/evidence/**` `-whitespace` (evidence keeps exact bytes); cham3 file fixed. `git diff --check 401c6cf` exits 0 |

### Declared native environment

The Codex Remote proxy returned HTTP 403 for the Kali and LibreWolf hosts. This cycle declares a disposable native x86-64 Debian 13 environment whose ordinary network reaches every required host: GitHub-hosted `ubuntu-latest` runners (ephemeral VMs) running every gate inside `debian:trixie` containers (privileged for live-build and QEMU), KVM when offered. No proxy or CA injection, TLS and apt authentication unchanged, no secrets. Definition and re-run commands: `docs/evidence/M1-bootable-desktop-distribution/codex_remote_verification.md`; workflows `.github/workflows/m1_image.yml` (heavy), `m1_checks.yml` (fast + privileged capture), `m0_checks.yml` (M0 regression).

New verification code: `tests/native/f2_source_gates.py` (real-archive F2 gates), `build/scripts/ci_m1_heavy.sh` (container driver), `build/vm/run_m1_vm_checks.py` with `build/vm/guest/*` and the test-only `build/vm/m1_install.preseed` (VM gates).

## Verification

All rows ran at the implementation target `01bf7a1` in the declared environment (GitHub-hosted x86-64 runners, `debian:trixie` containers, KVM). Evidence, commands, run URLs and artifact hashes: [native_01bf7a1/](../evidence/M1-bootable-desktop-distribution/native_01bf7a1/README.md). Acceptance mapping: [acceptance_matrix.md](../evidence/M1-bootable-desktop-distribution/acceptance_matrix.md).

| Check | Environment/command | Result | Evidence/limits |
|---|---|---|---|
| Static checks + unit suites | `m1_checks` 38207816376, uid 1000: `run_static_checks.sh`; `run_tests.sh static`/`desktop`/`staging`/`apt` | PASS (82, 261, 16, 22; 0 failures) | `ci_logs/38207816376_static_and_desktop.log`; also run locally in `debian:trixie` before each push |
| F6 native pin identity | `tests/run_tests.sh apt` (real apt, signed synthetic repositories, no network) | PASS (22) | changed/removed Release identity still denies; superseded pins fail open (control); foreign-signed index rejected; host binding |
| Packages + lintian | `build_packages.sh`; `run_tests.sh packages` | PASS (lintian exit 0; 8) | `ci_logs/38207816376_package_build.log` |
| F3/F8 privileged capture | privileged `debian:trixie`, `CHAM3_REQUIRE_PRIVILEGED_CAPTURE=1 sh tests/container/in_container.sh` | PASS (77, 0 skipped) | dependency-free stub of the exact control scripts with real wireshark-common/dpkg; not the KDE payload |
| M0 regression (F1) | `m0_checks` 38207816428: M0 image build, `image` (17), TCG/KVM boot, `boot` (18) | PASS | default build stages cham3-base only; login prompt on serial |
| F2 native source gates | `m1_image` 38207816363 `f2_native`: `tests/native/f2_source_gates.py` against the real archives | PASS (7/7) | 0 of 9 945 foreign-only packages escape; closure downloads 8 Kali (= allowlist) + 1 LibreWolf; 1 155-package leak without pins (control); real install + upgrade |
| M1 ISO + image-m1 | `ci_m1_heavy.sh iso` (privileged) | PASS (image-m1 54/54) | ISO sha256 `f25bf75eb6357f76b787cd7a0630a457f7e30519438ba139ea1fa4e7ca4afd16` (workflow artifact, not committed); payload, trust, origin, dumpcap image state |
| Live Plasma Wayland session | `run_m1_vm_checks.py live` (QEMU q35/KVM, BIOS, no network) | PASS (37/37) | session type, panel, branding, launcher categories/search/real launch, inert widgets, dumpcap and capture, apt policy; 20 screenshots |
| Disk installation | `run_m1_vm_checks.py install` (ISO's own d-i, test preseed) | PASS | completed and powered off in 330 s |
| Installed system | `run_m1_vm_checks.py installed` (network) | PASS (65/65) | Debian sources, live packages removed, apt update/upgrade origin, dumpcap lifecycle incl. fail-closed reinstall and recovery, member/nonmember capture, SDDM → Wayland, panel, launcher, inert widgets; 20 screenshots |
| `git diff --check 401c6cf` | local | PASS | captured evidence declared `-whitespace` in `.gitattributes` |

Not run and why: real hardware and UEFI boot (outside the declared VM gate); the installed-system wireshark-common upgrade step was not applicable because the mirror carries no newer version (a real version change is in the privileged suite); Codex independent re-review (pending).

## Review
- **Reviewer and exact commit:** Codex — independent re-review of `01bf7a1` pending. Prior verdicts: CHANGES_REQUESTED on `b82919c` and on `0fec3a0`.
- **Internal review (not approval):** none independent this cycle (the owner asked for direct implementation, so the test-reviewer subagent was not used); the author does not approve its own security-sensitive work.

## Remaining
- **Known issues/risks:** All evidence above was produced by the implementation owner in an environment it declared; it needs independent review and may be re-run by Codex from the scripted recipe. VM evidence is QEMU (q35, BIOS, virtio-vga, software rendering), not real hardware or UEFI. OCR-based checks read rendered text and can miss words (a miss fails). The firmware licence acceptance for `firmware-ipw2x00`/`firmware-ivtv` is an owner decision for the development image, to revisit before any release. The three deferred tools remain absent pending a separate reviewed task. The Kali archive key expires 2028-04-17 (fail-closed). LibreWolf has no independent second trust channel. Package maintainer/copyright/licence placeholders remain owner decisions before publication. Scratch branches `ci/m1-vm-1` and `ci/m1-vm-2` (VM-harness iteration against earlier ISOs, not evidence) remain on `origin` and can be deleted.
- **Uncommitted paths or dirty state:** none at handoff.
- **Required next action and owner:** Codex (REVIEWER/SECURITY REVIEWER) independently re-reviews the exact implementation target, the F2/F3 security design and the recorded native evidence, and may re-run the scripted gates. On APPROVE: the integrator merges through the owner-approved process. On CHANGES_REQUESTED: the coordinator repairs against the exact reviewed commit.

## Implementation target
- **Target (resubmit to Codex):** `01bf7a1f1066fae15ab51a56a5212c0a69780ab8` on `origin/task/M1-bootable-desktop`, the complete repaired implementation (F6, F3, F7, F8 and the native-gate defects; all tests and verification code). The branch HEAD is this target plus documentation-only commits that record the evidence (`docs/**` only; the heavy workflow ignores them).
- **Baseline:** `401c6cf9a550c81decf832cbc346f6908b9c95ef`. **Supersedes** `0fec3a0`.
