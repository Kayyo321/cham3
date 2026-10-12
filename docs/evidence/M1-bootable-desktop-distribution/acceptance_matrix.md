# M1 acceptance-to-evidence matrix (repair cycle)

Task: `docs/tasks/M1-bootable-desktop-distribution.md`. Baseline `401c6cf`. This
matrix is filled only with evidence actually produced. Statuses are PASS, FAIL,
NOT_RUN, or BLOCKED — no "PARTIAL". Where a criterion has a static/package part
that is verified and a runtime/native part that is not, both parts are named with
their own status. A build/boot result is never described as proof of M2–M4
security enforcement.

Evidence stages: **static** (host/container, no build) · **container** (amd64
`debian:trixie`, unprivileged) · **native** (x86-64 Debian 13 solver, Codex
Remote) · **VM** (disposable Debian 13 amd64, QEMU) · **install+boot**.

This repair cycle addresses Codex findings F1–F5 and the firmware blocker. Fast
gates were re-verified in-container/at agent level; the real ISO build, graphical
boot, disk install, and native closure/origin gates are assigned to Codex Remote
(`codex_remote_verification.md`) and are NOT_RUN here by design.

| # | Acceptance criterion (abridged) | Stage / method | Status | Evidence |
|---|---|---|---|---|
| A1 | Dedicated task branch/worktree on recorded baseline; exact branch/worktree + target recorded | static | PASS | Task "Assignment and Git"; `git worktree list`; baseline `401c6cf`, target recorded in the handoff |
| A2 | Documented Debian 13 amd64 live-build path produces a bootable, installable ISO from declared inputs; no prod creds/undocumented host state | static (config) + VM | config PASS; real build NOT_RUN → Codex Remote §2–§3 | `build/live-desktop` + `build_image.sh` per-profile staging (F1) documented; firmware fetch removed (F2); real M0 and M1 builds pending |
| A3 | In a disposable VM the ISO boots into Plasma 6 Wayland and completes the documented install path | install+boot | NOT_RUN → Codex Remote §4,§5 | QEMU VNC screendump recipe provided; capture pending |
| A4 | Installed desktop uses cham3 branding defaults; 56–64 logical-px vertical panel on the right edge | static + install+boot | static PASS; render NOT_RUN → Remote §4 | layout/branding sources + packaged wallpaper path verified statically (F4 fix: `Cham3/contents/images/3840x2160.png`); screenshot pending |
| A5 | Panel contains the nine components in ADR-0002 order/presentation (M1 = visual/package foundation) | static + install+boot | static PASS; render NOT_RUN → Remote §4 | panel-layout + plasmoid tests PASS (right edge, height 60, exact order); on-screen render pending |
| A6 | M2–M4-dependent functions visibly report unavailable/disconnected/unverified; no implied active contracts/aliases/modes/kill/identity | static + install+boot | static PASS; render NOT_RUN → Remote §4 | honest-state tests PASS; render pending |
| A7 | Desktop preserves desired/signed/pending/enforced distinction; absent/stale/failed backend → visible unavailable, no false success | static + install+boot | static PASS; render NOT_RUN → Remote §4 | inert-state tests + plasmoid review; render pending |
| A8 | `cham3-desktop`/`cham3-full` valid metadata + dependency relationships; curated suite in default `cham3-full` | container + native | metadata/relations PASS; native suite verification NOT_RUN → Remote §7 | package build + lintian exit 0; curated-suite test PASS. Per the 2026-10-11 owner scope amendment the suite = the Debian-available tools + six foreign roles (LibreWolf, ZAP, amass, searchsploit, ghidra, radare2) via pinned sources; three roles (mitmproxy, wpscan, metasploit-framework) are explicitly deferred and documented. Native closure/real-image presence pending |
| A9 | Manifests prefer Debian 13; Debian-native equivalents where unavailable; Kali only via reviewed narrow pinning; substitutions/omissions documented; no unreviewed foreign repo | static + native | pin/relation design PASS; native closure NOT_RUN → Remote §7 | disposition matrix; a **narrowly pinned** Kali source (Pin -1 + 8-package allowlist) + LibreWolf own repo, both documented and owner-approved; deferrals recorded. No global Kali repo. Native transitive/closure pending |
| A10 | Package-source config authenticates origin/signatures; Kali limited to reviewed allowlist+closure; update behavior documented without silent origin change | static + native | key/pin design PASS; native closure/update NOT_RUN → Remote §7 | keys verified (Kali `827C…E4C5`, LibreWolf `662E…EF16`) and trusted only via `Signed-By` (0 keys in `trusted.gpg.d`, verified in a chroot run); strict `-1` pin; simulated `dist-upgrade` migrates nothing to Kali. Native re-confirmation pending |
| A11 | Installed `.desktop` entries in correct categories, pass desktop-file validation; launch checks resolve to installed apps | static + install+boot | static PASS; installed-launch NOT_RUN → Remote §4 | desktop-file tests PASS; **F5**: the user-launcher presents categorized installed apps + search via the maintained KDE kicker model (plasmoidviewer confirmed categories + a real Wireshark launch); installed/live categories+launch pending |
| A12 | Debian KDE utilities present as designed; named redundant alternatives absent (incl. transitive) | container + VM | package-relation PASS; transitive image NOT_RUN → Remote §3 | duplicate-exclusion tests PASS on deps; transitive absence in the real image manifest pending |
| A13 | Image/package/desktop-file/ISO-boot/install evidence recorded with exact commands/env + PASS/FAIL/NOT_RUN/BLOCKED; build/boot not called M2–M4 proof | all | PASS (for work done); Remote adds ISO/boot/install | this matrix + `container_checks.md` + `codex_b82919c/` review transcripts + handoff; Remote recipe updated |
| A14 | `docs/STATUS.md` updated only with evidence-backed results after verification; no promotion of planned work | n/a | PASS | STATUS updated with fast-gate results and heavy/native-gate NOT_RUN only |
| A15 | Independent reviewer examines exact target; security review required before READY_TO_MERGE/DONE | review | NOT_RUN (re-review pending) | first Codex review returned CHANGES_REQUESTED on `b82919c`; re-review of the new target pending |

## Finding dispositions (Codex review of `b82919c`)

| Finding | Disposition | Where |
|---|---|---|
| F1 — M0 build broken by the new package set | FIXED | `build_image.sh` stages per-profile via `cham3-local-packages` (M0 → cham3-base only); fail-closed; `CHAM3_STAGE_DRYRUN`; regression tests (`staging`, 11) + CI gate. Real M0 build → Remote §2 |
| F2 — full metapackage omits required roles | RESOLVED PER OWNER AMENDMENT | 6 roles added via narrowly pinned authenticated sources (LibreWolf own repo; ZAP, searchsploit, ghidra, amass, radare2 from Kali, strict `-1` pin + allowlist); 3 roles (mitmproxy, wpscan, metasploit-framework) deferred by recorded scope amendment. Native closure/origin → Remote §7 |
| F3 — setuid-root dumpcap fallback | FIXED | `dpkg-statoverride` (root root 0750) pins no-setuid across install/upgrade/reconfigure; cham3-desktop postinst/trigger sets group+caps or fails closed (no caps, no setuid). Failure-path + member/nonmember tests; privileged member capture → Remote §6 |
| F4 — image test expected two files not shipped | FIXED | `test_m1_image_contents.py` now checks `plasma.desktop` and `Cham3/contents/images/3840x2160.png` (derived from packaging), with negative controls retained |
| F5 — launcher cannot present the curated suite | FIXED | user-launcher uses the maintained KDE kicker model for categorized apps + search; no hardcoded list; contract badges remain M5. Live categories/launch → Remote §4 |
| Firmware/proxy blocker | FIXED | `--firmware-chroot false` + explicit 42-package self-contained firmware list; no build-time fetch of Broadcom blobs; legacy b43/b43legacy coverage dropped by recorded owner decision |

## Notes

- The included foreign roles are present only through a narrowly pinned,
  separately-reviewed source (owner scope amendment, 2026-10-11). Their actual
  installation and clean closure are confirmed natively on Codex Remote before any
  merge; the strict `-1` pin + allowlist is the enforcement, the native gates are
  the verification.
- The deferred roles (mitmproxy, wpscan, metasploit-framework) and the omitted
  alternatives (Cutter/rizin in favour of radare2; Burp in favour of ZAP) are
  documented in `cham3-full` and the disposition matrix. Their absence is an
  explicit, recorded scope decision, not a silent omission.
- `dumpcap` capture privilege is resolved through the Debian `wireshark` group +
  file capabilities with **no setuid fallback** (F3); no user is added to the
  group by the image.
