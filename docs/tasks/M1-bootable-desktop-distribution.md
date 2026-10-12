# Task: M1 — Bootable desktop distribution

- **State:** DONE (owner-authorized merge `50e0ee2c16b2db619d106f969df90c84b88e67ae` published to `origin/main`; all acceptance criteria and post-merge GitHub Actions PASS). Implementation target `01bf7a1f1066fae15ab51a56a5212c0a69780ab8` was independently **APPROVED** by Codex with F2 and F3 security approval **GRANTED** (`docs/reviews/M1-bootable-desktop-distribution-codex-01bf7a1.md`, evidence `docs/evidence/M1-bootable-desktop-distribution/codex_01bf7a1/`).
- **Next owner / action:** Owner/coordinator plans the next milestone. M1 is complete within its development scope. The three deferred tools, the firmware licence decision and package publication metadata remain open before any public release.
- **Owner / role / provider:** Claude Code (Opus 5.5) COORDINATOR/INTEGRATOR + IMPLEMENTER. Cycles 0–1 used the subagents `m1_worker` (Sonnet 5.5), `m1_test_reviewer` (Haiku 5.5) and `m1_docs_research` (Haiku 5.5); in repair cycle 2 the owner asked for direct implementation without them. Independent REVIEWER/SECURITY REVIEWER: Codex (OpenAI) — CHANGES_REQUESTED on `b82919c` and `0fec3a0`; **APPROVE** on `01bf7a1`.
- **Created / updated:** 2026-10-11 / 2026-10-12
- **Base commit (M1 implementation baseline):** `401c6cf9a550c81decf832cbc346f6908b9c95ef` (current `main`; includes approved ADR 0002 and current branding assets). The older historical base `3f4690d80c87f4d1a2da88a9bbc1ba7bacb1dedc` recorded at drafting is superseded as the implementation baseline and kept as context only.

## Objective

Deliver a bootable, installable Debian GNU/Linux 13 (Trixie) x86-64 image with KDE Plasma 6 on Wayland, cham3 branding and desktop defaults, the visual/package foundation for the approved right-side taskbar in ADR 0002, and a curated built-in application suite. Full interactive taskbar behavior is M5 scope. M1 must report M2–M4 security services as unavailable until those services exist and their enforcement is confirmed.

## Approved requirements

- Source: `PROJECT.md` §§8, 12, 13, and 18.
  - §8 requires KDE Plasma 6 on Wayland, cham3 visual styling, a default vertical right-side taskbar with application switching, network-mode indicators, contract-inspection access, KDE tray, clock, session controls, and security/enforcement status.
  - §12 requires a bootable x86-64 installation image, Debian live-build, cham3 packages, installer integration, and the `cham3-desktop` and `cham3-full` metapackages.
  - §13 assigns image configuration to `build/` and Debian package definitions to `packaging/`.
  - §18 requires fail-closed behavior and forbids inferring effective policy from UI state.
- Source: `docs/engineering/ROADMAP.md`, M1 objective and exit criteria.
- Source: [ADR 0002 — Default right-hand taskbar layout and trusted desktop boundaries](../decisions/0002_default_right_taskbar_layout.md), **APPROVED**. M1 supplies the visual/package foundation; the full panel and trusted interactions are assigned to M5-001 in [M5 task specification](M5-default-taskbar-layout.md). ADR 0002 does not claim implementation and reserves ECR Explorer pending ADR 0001 approval/implementation.
- Source: task request dated 2026-10-11. It specifies the privacy browsers, traffic-monitoring tools, curated security-tool suite, and duplicate-utility exclusions below.
- Owner decision recorded 2026-10-11: prefer Debian 13 packages; use a Debian-native equivalent when a requested tool is unavailable or incompatible; permit a selected Kali-origin package only when no suitable Debian-native option exists and only through a narrowly pinned, separately reviewed source. Include the resulting curated suite in the default `cham3-full` package set. The exact package list and dependency closure still require review before implementation.
- **Owner decision recorded 2026-10-11 (M1 repair cycle, after the dependency-closure review in `docs/research/M1_package_disposition.md`):** include these foreign-sourced roles in `cham3-full` via strictly pinned, authenticated, separately-reviewed sources — LibreWolf (its official repository), and OWASP ZAP (`zaproxy`), `amass`, searchsploit (`exploitdb`), Ghidra (`ghidra`), and `radare2` (satisfying the radare2/Cutter role) from a **narrowly pinned** Kali source (apt Pin-Priority `-1` for the Kali origin with a per-package allowlist; closure review shows no Debian base package is admitted, and a simulated `dist-upgrade` migrates nothing to Kali). This is not a global Kali repository. **Defer** `mitmproxy`, `wpscan`, and `metasploit-framework` to a separate, independently-reviewed follow-up task: their dependency closures pull Debian base libraries (libc6/Qt/Ruby) from Kali and no clean route exists yet. This is an explicit, recorded **scope amendment**, not a silent omission; `cham3-full` documents the deferrals. Burp Suite (licence) and Cutter/rizin (closure) are omitted in favour of ZAP and radare2. Including the six is contingent on native dependency-closure confirmation on Codex Remote and independent Codex security review before merge.
- **Owner decision recorded 2026-10-11 (M1 repair cycle):** build firmware without the fetch-at-install Broadcom `b43`/`b43legacy` installers (`--firmware-chroot false` plus an explicit self-contained firmware set), removing the only build-time network fetch. Modern Broadcom (`brcm80211`) and all other firmware remain; legacy b43/b43legacy SoftMAC 43xx Wi-Fi coverage is intentionally dropped, as Debian ships no self-contained firmware blobs for those chips.

- **Owner decision recorded 2026-10-12 (M1 repair cycle, after the Codex re-review of `0fec3a0`):** the explicit firmware list's `firmware-ipw2x00` and `firmware-ivtv` refuse to install without a debconf licence answer, which live-build's `chroot_firmware` stage gave automatically under the earlier `--firmware-chroot true` design. Preseed acceptance of both licences for the development image (`build/live-desktop/config/preseed/cham3_firmware_licenses.cfg.chroot`), keeping all 42 approved firmware packages; revisit before any public release.

## Scope and exclusions

- **In scope:**
  - Debian 13 Trixie x86-64 live-build configuration and an installable bootable ISO.
  - KDE Plasma 6 Wayland session and cham3 branding defaults.
  - Default right-edge vertical panel sized 56–64 Plasma logical pixels, with visual/package scaffolding for all nine ADR 0002 components in the specified top-to-bottom order. Full interactive behavior belongs to M5-001.
  - Components that depend on M2–M4 services must visibly report unavailable, disconnected, or unverified state; controls that could change security policy must be inert until their trusted backend is available.
  - Debian package metadata/metapackages for `cham3-desktop` and `cham3-full`, with the curated security application suite included in the default `cham3-full` package set and categorized application-menu entries.
  - Privacy browsers, web/network traffic inspection, and the requested security-tool suite, subject to approved package sourcing and compatibility decisions.
  - Documentation of package sources, dependency decisions, image inputs, and installation/update behavior.
- **Out of scope:**
  - Implementing identity, vault, contract, registry, sandbox, or network enforcement backends planned for M2–M4.
  - Claiming that a visual control, signed preference, plasmoid, or desktop configuration enforces a security policy.
  - Replacing or duplicating the Debian KDE core utilities listed below.
  - Enabling a Kali repository globally, importing packages across Debian releases, or changing `PROJECT.md` without an owner-approved decision.
  - Production signing credentials or production release publication.

### ADR 0002 panel foundation for M1

Use the approved composition and component identifiers from [ADR 0002](../decisions/0002_default_right_taskbar_layout.md). M1 includes their packaged visual foundation in this order; the interactions and enforcement integrations described in ADR 0002 are M5 scope:

1. `cham3_user_launcher`: authenticated-user avatar, application/session flyout visual foundation, and public `key_id` placeholder. No placeholder may imply identity enrollment, authentication, or key validity.
2. `cham3_alias_wheel`: trifold Rose Direct/Clearnet, Purple Tor, and White Offline execution-alias visual foundation. These are three user-scoped aliases, not signing identities or virtual machines. Alias selection, HUD previews, profile separation, and launch authority require their approved M5/backend behavior.
3. `cham3_tag_pager`: five dwm-style tags (`1`–`5`) per alias, 15 visual contexts total. Tag navigation and window moves require M5 behavior.
4. `cham3_contract_dock`: 2×2 grid for Contract Handler (`cham3_contract_manager`), ECR Explorer (`cham3_ecr_explorer`), Active Contracts Monitor (`cham3_active_monitor`), and Hot Contract View. ECR Explorer must visibly say unavailable pending ADR 0001 approval/implementation; all other service-dependent actions remain unavailable until their backends exist.
5. `cham3_netproto_launcher`: full-width routing-glyph visual foundation for the three-tab Clearnet/Tor/Offline Network Protocol Editor. Do not apply settings without the approved trusted workflow and backend.
6. `cham3_killswitch`: physical-style Full Isolation Kill rocker visual foundation. Until trusted enforcement exists, it must visibly report unavailable and must not simulate an active kill or alter the host firewall.
7. `cham3_settings_launcher`: entry point to cham3 Unified Control Center sections in KDE System Settings.
8. `cham3_task_manager`: two-tier visual foundation for active-tag apps and dimmed other-tag apps within the current alias. Do not fabricate task attribution or network-status pips.
9. Upstream Plasma tray and vertical digital clock/calendar, placed at the bottom of the panel. Preserve upstream tray functionality.

Use the ADR's Rose/Purple/White status colors with labels or accessible descriptions; color alone cannot communicate state. M1 does not claim that the layout creates isolated aliases, changes application authority, applies network modes, inspects contracts, or enforces global isolation.

### Built-in application suite

Retain the upstream Debian KDE utilities and do not include these redundant alternatives:

- File manager: Dolphin; exclude Thunar and PCManFM.
- Terminal: Konsole; exclude QTerminal, Xfce Terminal, and Tilix.
- Text/code editor: Kate and/or KWrite; exclude Mousepad and Leafpad.
- Archive manager: Ark; exclude Xarchiver and File-Roller.
- Document/PDF viewer: Okular; exclude Evince and Atril.
- Image viewer: Gwenview; exclude Ristretto.
- Resource monitor: Plasma System Monitor and/or KSysGuard.
- Screenshot utility: Spectacle.

Include the following requested application groups in the default `cham3-full` package set. Resolve each item in this order: use its compatible Debian 13 package; if unavailable or incompatible, use a suitable Debian-native equivalent; if no suitable Debian-native option exists, a selected Kali-origin package may be used only through a narrowly pinned, separately reviewed source. Document substitutions and omitted items.

- **Privacy browsers:** LibreWolf for clearnet; official Tor Browser for Tor browsing. Do not install Firefox ESR as the generic stock browser.
- **Traffic analysis:** Wireshark and `tshark`, with packet capture configured through a dedicated `dumpcap` group and no unnecessary root execution; mitmproxy/mitmweb; OWASP ZAP or Burp Suite Community; `tcpdump`, `nethogs`, and `iftop`.
- **Information gathering:** `nmap`; Zenmap or an approved KDE-appropriate equivalent; `masscan`, `amass`, `dnsutils`, and `whois`.
- **Web application analysis:** `sqlmap`, `ffuf`, `gobuster`, `nikto`, and `wpscan`.
- **Wireless and hardware:** `aircrack-ng` suite.
- **Exploitation and auditing:** `metasploit-framework` and `searchsploit`.
- **Password and cryptographic auditing:** `hashcat`, `john`, and `hydra`.
- **Reverse engineering and forensics:** `ghidra`, `radare2` or Cutter, `binwalk`, and `exiftool`.

Package lists and metapackages must avoid desktop conflicts, document unavailable, substituted, or incompatible packages, and organize launchable applications under appropriate menu categories. Every added `.desktop` entry must have a unique, valid desktop-file ID and must launch the intended installed application. Do not add fake launchers for absent tools.

## Dependencies and interfaces

- **Dependencies:** M0 build baseline and package skeleton; approved ADR 0002 (merged at `3f4690d80c87f4d1a2da88a9bbc1ba7bacb1dedc`); Debian 13 Trixie x86-64 build environment; disposable VM for boot/install verification. Any Kali-origin package must pass the separately reviewed, narrowly pinned source and dependency-closure checks below.
- **Agreed interfaces:** Use Debian live-build per `PROJECT.md` §12.1. Keep package boundaries aligned with `PROJECT.md` §12.2. Place image definitions in `build/` and package definitions in `packaging/` per §13. Follow ADR 0002 for layout and internal identifiers. M1 UI is visual/package foundation only; backend-dependent features must report unavailable/disconnected and security-sensitive actions must be inert until the M5/backend interfaces are implemented and verified.
- Do not enable a general Kali repository or unreviewed cross-release dependencies. Any Kali-origin packages require an explicit package allowlist, narrowly pinned source configuration, dependency-closure review, package signature/origin verification, and upgrade behavior evidence before inclusion.

## Acceptance criteria

Status at `01bf7a1` (2026-10-12): every criterion below has evidence, mapped row by row (A1–A15) in `docs/evidence/M1-bootable-desktop-distribution/acceptance_matrix.md`. Author evidence: `native_01bf7a1/`. Independent Codex evidence: `codex_01bf7a1/`.

- [x] The task is assigned on a dedicated task branch/worktree based on the recorded baseline; exact branch/worktree and target commit are recorded.
- [x] A documented Debian 13 Trixie x86-64 live-build path produces a bootable, installable ISO from declared inputs without production credentials or undocumented host state.
- [x] In a disposable VM, the ISO boots into KDE Plasma 6 on Wayland and completes the documented installation path. Record VM details, artifact identity, and limitations.
- [x] The installed desktop uses cham3 branding defaults and starts with a 56–64 Plasma logical-pixel vertical panel on the right edge.
- [x] The panel contains the nine components in the order and presentation defined by ADR 0002: `cham3_user_launcher`, `cham3_alias_wheel`, `cham3_tag_pager`, `cham3_contract_dock`, `cham3_netproto_launcher`, `cham3_killswitch`, `cham3_settings_launcher`, `cham3_task_manager`, and the upstream Plasma tray/clock. M1 verifies visual/package foundation; interactive and trusted behavior is tracked by M5-001.
- [x] M2–M4-dependent functions visibly report unavailable/disconnected/unverified state; M1 controls cannot imply active contracts, aliases/profile isolation, network modes, kill-switch enforcement, or cryptographic identity without authoritative backend evidence.
- [x] The desktop preserves the distinction between desired, signed/verified, pending, and actively enforced policy. An absent, stale, or failed backend produces a visible unavailable/failure state and cannot trigger a false success state.
- [x] `cham3-desktop` and `cham3-full` have valid package metadata and dependency relationships; the curated security suite is included in the default `cham3-full` package set.
- [x] Package manifests prefer Debian 13 packages, use Debian-native equivalents for unavailable/incompatible requests, and include a Kali-origin package only when no suitable Debian-native option exists and that package passes the separately reviewed narrow-pinning/source/dependency checks. Every substitution or omission is documented; no general or unreviewed foreign repository is enabled.
- [x] Package-source configuration authenticates package origin/signatures, limits Kali package selection to the reviewed allowlist and dependency closure, and documents update/upgrade behavior without silently changing package origin.
- [x] Installed `.desktop` entries appear under appropriate launcher/menu categories and pass desktop-file validation; launch checks confirm entries resolve to installed applications.
- [x] The Debian KDE utilities listed above are present as selected by the package design, and package/dependency inspection confirms the named redundant alternatives are absent.
- [x] Image build, package validation, desktop-file validation, ISO boot, and installation evidence are recorded with exact commands/environments and `PASS`, `FAIL`, `NOT_RUN`, or `BLOCKED` results. Build/boot success is not described as proof of M2–M4 security enforcement.
- [x] `docs/STATUS.md` is updated only with evidence-backed M1 results, after verification; it does not promote this draft or planned work to implemented status.
- [x] An independent reviewer examines the exact target commit. Because this task defines security-sensitive status behavior and distribution contents, security review is required before the task can advance to READY_TO_MERGE or DONE.

## Security considerations

- M1 UI is presentation and configuration only for M2–M4-dependent features. A UI toggle, colored context, package presence, or signed preference is not evidence of active enforcement.
- The default behavior for absent, stale, or failed backend state is visibly unavailable/disconnected and fail closed. Do not present Tor as active, Offline as enforced, or Full Isolation Kill as engaged without confirmation from the trusted enforcement service.
- Wireshark capture must not grant broad root access; document group membership and capture privilege boundaries.
- Application suite contents and package sources affect the supply chain. Do not use production signing keys, enable unreviewed cross-distribution package sources, or claim package safety based only on package names.
- Do not claim anonymity, sandboxing, contract verification, or network enforcement from desktop appearance or static configuration.
- **Security review required:** YES — the image supply chain and security-status UI can create user-facing security claims.

## Required verification

- Build the ISO and packages on a declared Debian 13 Trixie x86-64 environment; record exact commands, inputs, package versions, and artifact checksums.
- Validate package metadata, dependency resolution, menu entries, and absence of duplicate utility packages against the built image/package set.
- Boot and install in a disposable x86-64 VM; inspect session type, panel orientation/size, launcher contents, and backend-unavailable behavior.
- Exercise positive and negative UI state cases: unavailable backend does not show active enforcement; backend failure remains visibly unavailable; normal KDE session controls and ordinary desktop use still work.
- Inspect package source configuration and generated artifacts for secrets, unapproved repositories, false security claims, and unintended duplicate utilities.
- Report each check as `PASS`, `FAIL`, `NOT_RUN`, or `BLOCKED`. A static configuration check or VM simulation must be described only within the behavior it actually exercises.

## Assignment and Git

- **Specification author (historical):** Codex, COORDINATOR / IMPLEMENTER for the original specification change.
- **M1 implementation coordinator / integrator:** Claude Code (Opus 5.5). Owns integration, shared build-script changes, status/task/handoff records, and verification.
- **Implementation owner:** Claude Code — `m1_worker` (Sonnet 5.5) under the coordinator, owning `packaging/cham3-desktop/`, `packaging/cham3-full/`, `desktop/`, `build/live-desktop/`.
- **Test owner:** `m1_test_reviewer` (Haiku 5.5), owning new `tests/` suites and internal review. Internal review does not substitute for independent approval.
- **Independent reviewer / security reviewer:** Codex. CHANGES_REQUESTED on `b82919c` (`docs/reviews/M1-bootable-desktop-distribution-codex.md`) and `0fec3a0` (`docs/reviews/M1-bootable-desktop-distribution-codex-rereview.md`); **APPROVE** on `01bf7a1` with F2/F3 security approval GRANTED (`docs/reviews/M1-bootable-desktop-distribution-codex-01bf7a1.md`). The author did not approve its own security-sensitive work.
- **Implementation baseline:** `401c6cf9a550c81decf832cbc346f6908b9c95ef` on `main`.
- **Branches / worktrees (off the baseline):**
  - `task/M1-bootable-desktop` — `/Users/jessicabruce/cham3-worktrees/M1-integration` (coordinator/integrator; shared `build/scripts/*`, docs, integration merges).
  - `task/M1-worker` — `/Users/jessicabruce/cham3-worktrees/M1-worker` (implementer; packaging/desktop/live-desktop).
  - `test/M1` — `/Users/jessicabruce/cham3-worktrees/M1-test` (test owner; `tests/`).
  - `docs/M1-research` — `/Users/jessicabruce/cham3-worktrees/M1-docs` (package disposition matrix and readiness packet under `docs/research/`).
- **Persistent subagent definitions:** `.claude/agents/m1_worker.md`, `.claude/agents/m1_test_reviewer.md`, `.claude/agents/m1_docs_research.md`.
- **Target commit:** `01bf7a1f1066fae15ab51a56a5212c0a69780ab8` on `task/M1-bootable-desktop` (supersedes `0fec3a0` and `b82919c`; independently approved). Handoff: `docs/handoffs/M1-bootable-desktop-distribution.md`. Heavy/native gates: `.github/workflows/m1_image.yml` on GitHub-hosted x86-64 runners (declared environment in `docs/evidence/M1-bootable-desktop-distribution/codex_remote_verification.md`).
- **Historical specification record:** `task/M1-bootable-desktop-spec` at `7ccbfe7bd6fb71eb3027504e4b34afb6560434dd` (worktree removed during reconciliation).

## Decisions / blockers

- **Resolved owner decisions (2026-10-11):** Prefer Debian 13 packages; use a Debian-native equivalent for an unavailable or incompatible requested tool; permit selected Kali-origin packages only when no suitable Debian-native option exists and only through a narrowly pinned, separately reviewed source; include the suite in default `cham3-full`.
- **Implementation reviews (resolved 2026-10-12):** package-by-package dispositions are in `docs/research/M1_package_disposition.md`. The Kali allowlist, dependency closure, host-bound source pinning, signature/origin verification and upgrade behavior passed independent Codex review and an independent native F2 rerun at `01bf7a1` (F2 security approval GRANTED). Codex Remote's proxy blocked the Kali and LibreWolf hosts, so the native closure confirmation that the sourcing decision made a precondition for including the six foreign roles ran in the declared GitHub Actions environment instead.
- **Blockers:** none for M1. Outside this task and still open before any public release: the three deferred tools (separate reviewed task), the firmware licence decision, package maintainer/copyright/licence metadata, Kali archive key rotation (key expires 2028-04-17), UEFI and real-hardware coverage.

### Independent Codex re-review (2026-10-12)

- **Verdict / exact target:** **APPROVE** `01bf7a1f1066fae15ab51a56a5212c0a69780ab8` from baseline `401c6cf9a550c81decf832cbc346f6908b9c95ef`, for the approved M1 development scope. Evidence revision `058f839` changed only `docs/**`. F6, F3, F7 and F8 are closed, and the native-gate repairs passed independent reruns. Review record: `551266e` on `review/M1-codex-01bf7a1`, `docs/reviews/M1-bootable-desktop-distribution-codex-01bf7a1.md`.
- **Security approval:** F2 GRANTED for the narrow authenticated sources, host-bound pinning, dependency closure and upgrade policy. F3 GRANTED for the dumpcap policy and tested lifecycle.
- **Independent checks:**
  - All PASS:
    - static checks;
    - static/desktop/staging 82/261/16;
    - F6 signed synthetic suite 22;
    - package build, lintian and 8 package tests;
    - committed native SHA256SUMS;
    - the staged `auto/build` failure-path probe;
    - F2 real-archive gates 7/7;
    - privileged capture 77 with no skips;
    - M1 ISO independently rebuilt from `01bf7a1`, with image-m1 54/54;
    - VM live/install/installed 37/1/65 against the rebuilt ISO.
  - M0 regression: reviewed from run 38207816428, not rerun.
  - Commands and logs: `docs/evidence/M1-bootable-desktop-distribution/codex_01bf7a1/README.md`.
- **Limits stated by the reviewer:**
  - Coverage is QEMU BIOS/q35 only, with no UEFI or physical hardware, and no M2–M4 guarantees.
  - The apt origin controls do not make third-party application code safe.
  - Real-archive metadata rolls between runs, so no reproducible ISO is claimed.
  - Key rotation, release licence/publication readiness and the three deferred tools are separate work.
  - The installed phase's ISO identity is traced through the same job rather than recorded in the phase itself.
- **Required actions:** none. The integrator owns the task/status updates and integration.

### Owner-authorized integration (2026-10-12)

- **Authorization:** on 2026-10-12, after the Codex approval, the owner asked to "make its status complete and merge".
- **Records added to the task branch:** the three Codex review records (`review/M1-codex-b82919c` at `cde8039`, `review/M1-codex-0fec3a0` at `026d606`, `review/M1-codex-01bf7a1` at `551266e`), plus READY_TO_MERGE commit `75f586aed8644293d8d74de2e9f1cb91fe0918a6`. These changes are documentation only. `git diff --quiet 01bf7a1 75f586a -- ':!docs'` PASS: the approved implementation is unchanged.
- **Merge:** `git merge --no-ff task/M1-bootable-desktop` on `main` produced `50e0ee2c16b2db619d106f969df90c84b88e67ae`. Its first parent is `401c6cf9a550c81decf832cbc346f6908b9c95ef`; its second parent is `75f586a`. There were no conflicts. Its tree equals `75f586a`.
- **Publication:** `git push origin refs/heads/task/M1-bootable-desktop:refs/heads/task/M1-bootable-desktop refs/heads/main:refs/heads/main` advanced `origin/main` from `401c6cf` to `50e0ee2` (fast-forward, no force push). `git ls-remote --heads origin main` confirmed the remote tip.
- **Post-merge verification: PASS at `50e0ee2`.** All runs used disposable GitHub-hosted x86-64 runners with `debian:trixie` containers.
  - [m1_checks 38256907579](https://github.com/Kayyo321/cham3/actions/runs/38256907579):
    - static checks;
    - static/desktop/staging/apt 82/261/16/22;
    - packages 8 with lintian exit 0;
    - privileged capture 77.
  - [m0_checks 38256907472](https://github.com/Kayyo321/cham3/actions/runs/38256907472): M0 image build, 17 image tests, QEMU boot, 18 boot tests.
  - [m1_image 38256914419](https://github.com/Kayyo321/cham3/actions/runs/38256914419), via `gh workflow run m1_image.yml --ref main`:
    - F2 real-archive gates 7/7: 9 947 foreign-only packages with 0 escaped; closure of 8 Kali and 1 LibreWolf downloads;
    - M1 ISO built from `50e0ee2` with a clean tree, sha256 `477153203491984ca4326744a9563e1a998ce994e3065908b476421eaf81dd10`;
    - image-m1 54 tests, and no firmware host in the build log;
    - VM: live 37/37, install completed in 335 s, installed 65/65.
  - Evidence (run and job metadata, artifact digests, log summary lines, commands): `docs/evidence/M1-bootable-desktop-distribution/postmerge_ci_50e0ee2.json`.
- **Completion:** after post-merge success, this documentation-only record sets DONE. M1 completion establishes the development desktop image, packages and installer path. It does not establish M2–M4 enforcement, UEFI or hardware compatibility, a reproducible ISO, or release readiness.
