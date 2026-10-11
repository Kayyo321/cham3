# Handoff: M1 — Bootable desktop distribution

- **State:** IN_REVIEW. Integrated candidate for independent Codex review (readiness + implementation + security). Not approved; not merged; M1 is not complete.
- **Role:** Claude Code (Opus 5.5) as COORDINATOR / INTEGRATOR and implementation owner, with three subagents: `m1_worker` (Sonnet 5.5, IMPLEMENTER), `m1_test_reviewer` (Haiku 5.5, TEST/internal REVIEWER), `m1_docs_research` (Haiku 5.5, docs/research).
- **Model/provider:** Claude Opus 5.5 / Anthropic. Independent reviewer: Codex / OpenAI (pending).
- **Working directory / branch:** `task/M1-bootable-desktop` in `/Users/jessicabruce/cham3-worktrees/M1-integration` (integration). Contributor worktrees: `task/M1-worker`, `test/M1`, `docs/M1-research` (see below).
- **Task specification:** `docs/tasks/M1-bootable-desktop-distribution.md`.
- **Baseline commit:** `401c6cf9a550c81decf832cbc346f6908b9c95ef` (`main`; includes approved ADR 0002 and branding assets).
- **Current/implementation commit (target):** `b82919c6ddbdb1f84f8db35e3ded8559c40b242f`.

Product authority: `PROJECT.md` §§8, 12, 13, 18; approved ADR 0002; SEC-01–18. ADR 0001 remains PROPOSED (ECR Explorer is reserved and shown unavailable).

## Agent assignments and configuration

| Agent | Model / effort | Role | Branch / worktree | Owned paths |
|---|---|---|---|---|
| coordinator (this) | Opus 5.5 | COORDINATOR/INTEGRATOR + IMPLEMENTER | `task/M1-bootable-desktop` / `…/M1-integration` | `build/scripts/*`, `tests/` reconciliation, `docs/`, `.github/workflows/m1_checks.yml`, integration merges |
| `m1_worker` | Sonnet 5.5 / high | IMPLEMENTER | `task/M1-worker` / `…/M1-worker` | `packaging/cham3-desktop`, `packaging/cham3-full`, `desktop/`, `build/live-desktop/` |
| `m1_test_reviewer` | Haiku 5.5 / high | TEST_ENGINEER + internal REVIEWER | `test/M1` / `…/M1-test` | `tests/desktop/`, `tests/lib/m1_support.py` |
| `m1_docs_research` | Haiku 5.5 / medium | docs/research | `docs/M1-research` / `…/M1-docs` | `docs/research/` |

Persistent definitions: `.claude/agents/m1_worker.md`, `.claude/agents/m1_test_reviewer.md`, `.claude/agents/m1_docs_research.md`. Internal review prepared the candidate; it does not replace independent approval.

## Completed work

### Behavior / files changed (vs `401c6cf`)
- **`packaging/cham3-desktop/`** (Architecture: all): depends on the Plasma 6 Wayland stack and the KDE core utilities to keep (`plasma-desktop, plasma-workspace, kwin-wayland, sddm, systemsettings, plasma-nm, plasma-pa, dolphin, konsole, kate, ark, okular, gwenview, kde-spectacle, plasma-systemmonitor`; Recommends `fonts-noto, kinfocenter, qml6-module-org-kde-desktop`). Because the image uses `--apt-recommends false`, Plasma pieces that are only upstream-Recommends are promoted to hard Depends (recorded in the live config; avoids pulling the LAN-listening `kdeconnect`). Installs the eight plasmoids, look-and-feel `org.cham3.desktop`, color scheme `Cham3Dark.colors`, the `Cham3` wallpaper, and lowest-precedence `/etc/xdg` defaults. `debian/cham3-extra-sources` stages `desktop/` and `assets/`.
- **`packaging/cham3-full/`** (metapackage): `cham3-desktop` + the Debian-available curated suite (`nmap, zenmap, masscan, bind9-dnsutils, whois, wireshark, tshark, tcpdump, nethogs, iftop, sqlmap, nikto, ffuf, gobuster, aircrack-ng, binwalk, libimage-exiftool-perl, hashcat, john, hydra, torbrowser-launcher`). Long description names every GATED/absent requested tool as pending source review (not a dependency).
- **`desktop/`**: eight inert Plasma 6 QML plasmoids (`org.cham3.user_launcher, alias_wheel, tag_pager, contract_dock, netproto_launcher, killswitch, settings_launcher, task_manager`), `cham3_panel_layout.js` (right edge, height 60, ADR order then upstream tray + digital clock), look-and-feel, color scheme, wallpaper metadata, shared `cham3_shared.js`. All cham3-authored identifiers are snake_case; Qt/QML/KDE API names kept at the boundary. Backend-dependent components show unavailable/disconnected/unverified and perform no privileged action (kill switch does not touch any firewall; ECR Explorer says unavailable pending ADR 0001; `key_id` is a placeholder; no fabricated attribution/pips).
- **`build/live-desktop/`**: live-build config — trixie amd64, `main contrib non-free non-free-firmware`, firmware on, `--debian-installer live`, apt-secure, official mirrors, serial console, iso-hybrid, image `cham3-m1-desktop`; package list `cham3-full`; a chroot preseed setting `wireshark-common/install-setuid` to the non-setuid (group) answer; a live-config component setting the SDDM autologin session to Plasma Wayland.
- **Shared `build/scripts/`** (coordinator): `build_image.sh` gained `CHAM3_LIVE_DIR`/`CHAM3_IMAGE_NAME` (M0 defaults unchanged); `build_packages.sh` gained fail-closed `cham3-extra-sources` staging (rejects unsafe/missing paths and repo-escaping symlinks); `write_provenance.py` gained `--live-dir`. M0 invocations are byte-for-byte unchanged.
- **`tests/`**: new M1 suites under `tests/desktop/` + `tests/lib/m1_support.py`; `run_tests.sh` gained `desktop` and `image-m1` targets; the M0 "exactly the expected deb" check generalized to the packaging set; four documented test-correctness fixes (panel `height` thickness, plasmoid-dir scoping, `Cham3` theme-name boundary, M0 hygiene exemption for `build/live-desktop`).
- **`.github/workflows/m1_checks.yml`**: fast M1 CI (static + desktop + package jobs), M0 security posture.
- **`docs/`**: task record (baseline/ownership/state), `docs/research/M1_package_disposition.md` + `M1_readiness_packet.md`, `docs/evidence/M1-bootable-desktop-distribution/` (acceptance matrix, container checks, Codex Remote recipe), this handoff, STATUS update.

### Decisions made within approved scope
- Debian-available suite included now; LibreWolf + the not-in-trixie tools GATED and documented, not silently dropped (fail-closed for the supply chain). No foreign/Kali repo enabled.
- `--apt-recommends false` with deliberate Depends promotion (closure control + no kdeconnect).
- Installer: `--debian-installer live` (Calamares is in trixie but not wired in M1).
- Panel thickness via `panel.height` (the Plasma scripting thickness property for both orientations; verified against the plasmashell Panel property list).

## Verification

| Check | Environment / command | Result | Evidence / limits |
|---|---|---|---|
| Static checks | amd64 `debian:trixie` container, non-root: `build/scripts/run_static_checks.sh` | PASS | shellcheck/py_compile/yamllint/scan_secrets; `container_checks.md` |
| M0 static tests | same: `tests/run_tests.sh static` | PASS | 82 tests, 0 failures |
| Package build + lintian | same: `build/scripts/build_packages.sh` | PASS | cham3-base + cham3-desktop + cham3-full; lintian exit 0, no tags |
| M0 package tests | same: `tests/run_tests.sh packages` | PASS | 8 tests (generalized built-deb set) |
| M1 desktop tests | same: `tests/run_tests.sh desktop` | PASS | 122 tests (metadata, suite presence, duplicate/GATED exclusion, desktop-file rules, panel geometry/order, plasmoid ids, inert-state, naming, build-script behavior) |
| Plasmoid QML load | worker: `plasmoidviewer` under Xvfb, all 8 | PASS (load only) | no QML errors; a deliberate syntax error was caught (sensitivity). Not a rendered-panel result |
| Real desktop ISO build (A2) | native x86-64, privileged | NOT_RUN | → Codex Remote `codex_remote_verification.md` §2 |
| Graphical Plasma 6 Wayland boot (A3–A6 render) | QEMU + VNC screendump | NOT_RUN | → Codex Remote §3 |
| Disk install + installed boot (A3/A4) | QEMU | NOT_RUN | → Codex Remote §4 |
| Transitive duplicate/GATED absence (A9/A12) | image manifest | NOT_RUN | → Codex Remote §2 (`tests/run_tests.sh image-m1`) |
| Installed launchers + dumpcap capability (A11) | installed/live system | NOT_RUN | → Codex Remote §5 |

Environment note: the coordinator host is macOS arm64 (Apple M2 Pro); fast gates ran in an emulated amd64 container. Per owner instruction, the long emulated ISO/boot/install runs were NOT performed locally and are assigned to Codex Remote (native x86-64) with exact commands. A `m1_checks.yml` CI run on push also exercises the fast gates on native x86-64.

## Review
- **Reviewer and exact commit:** Codex, to review `b82919c6ddbdb1f84f8db35e3ded8559c40b242f` (pending).
- **Internal review (not approval):** `m1_test_reviewer` raised and the coordinator resolved: the M0 suites needed a deliberate scoping update for the M1 package set (done, generalized); the panel thickness and plasmoid-dir tests asserted the wrong thing (fixed with documented rationale); `qqc2-desktop-style` is absent (replaced by `qml6-module-org-kde-desktop`); the extra-sources staging now rejects repo-escaping symlinks; `torbrowser-launcher` provides no Tor routing and must not be presented as enforcement (documented). The author does not approve its own work.

## Remaining
- **Known limitations / risks:** Heavy gates NOT_RUN locally (assigned to Codex Remote). GATED tools (LibreWolf + Kali candidates) absent pending separate authenticated-source/pinning/closure review — concrete readiness packet in `docs/research/M1_readiness_packet.md`. Package maintainer/copyright/license placeholders remain owner decisions before publication. The trixie Release signature was NOT_RUN in the research environment (signing key not retrievable); apt-secure + official mirrors are configured. Honest-state checks are text heuristics; rendered-UI confirmation is a Codex Remote item.
- **Uncommitted paths / dirty state:** integration worktree clean at `b82919c`. The primary checkout `/Users/jessicabruce/cham3` (branch `main`) was not written to.
- **Required next action and owner:**
  1. **Codex (REVIEWER/SECURITY):** independently review `b82919c` (readiness of package dispositions + implementation), and run the Codex Remote heavy gates per `codex_remote_verification.md`; record verdict with `docs/templates/REVIEW.md`.
  2. On APPROVE + heavy-gate evidence: integrator merges through the owner-approved process; post-merge checks recorded; STATUS updated; M1 advances only then.
  3. On CHANGES_REQUESTED/BLOCKED: coordinator returns the task to IN_PROGRESS and assigns repairs to the same three subagents against the exact reviewed commit.
