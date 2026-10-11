# M1 acceptance-to-evidence matrix

Task: `docs/tasks/M1-bootable-desktop-distribution.md`. Baseline `401c6cf`. This matrix
is filled only with evidence actually produced. Until a check runs it is `NOT_RUN`.
A build/boot result is never described as proof of M2–M4 security enforcement.

Evidence stages: **static** (host/container, no build) · **container** (amd64 `debian:trixie`,
unprivileged) · **VM** (disposable Debian 13 amd64, QEMU TCG) · **host-QEMU boot** · **install+boot**.

Fast gates were verified in-container at `b82919c` (see `container_checks.md`).
Heavy gates (real ISO build, graphical boot, disk install) are assigned to Codex
Remote (see `codex_remote_verification.md`); they are NOT_RUN here by design.

| # | Acceptance criterion (abridged) | Stage / method | Status | Evidence |
|---|---|---|---|---|
| A1 | Dedicated task branch/worktree on recorded baseline; exact branch/worktree + target recorded | static | PASS | Task "Assignment and Git"; `git worktree list`; target `b82919c` |
| A2 | Documented Debian 13 amd64 live-build path produces a bootable, installable ISO from declared inputs, no prod creds/undocumented host state | VM | NOT_RUN (→ Codex Remote §2) | `build/live-desktop` + `build_image.sh` CHAM3_LIVE_DIR path documented; real build pending |
| A3 | In a disposable VM the ISO boots into Plasma 6 Wayland and completes the documented install path | host-QEMU boot + install+boot | NOT_RUN (→ Codex Remote §3,§4) | QEMU VNC screendump recipe provided; capture pending |
| A4 | Installed desktop uses cham3 branding defaults; 56–64 logical-px vertical panel on the right edge | install+boot | NOT_RUN (→ Codex Remote §3,§4) | layout/branding sources verified statically; screenshot pending |
| A5 | Panel contains the nine components in ADR-0002 order/presentation (M1 = visual/package foundation) | static + install+boot | PARTIAL — static PASS | panel-layout + plasmoid tests PASS (right edge, height 60, exact order); on-screen render → Remote |
| A6 | M2–M4-dependent functions visibly report unavailable/disconnected/unverified; no implied active contracts/aliases/modes/kill/identity | static + install+boot | PARTIAL — static PASS | honest-state test PASS (required wording present; forbidden active-security claims absent); render → Remote |
| A7 | Desktop preserves desired/signed/pending/enforced distinction; absent/stale/failed backend → visible unavailable, no false success | static + install+boot | PARTIAL — static PASS | inert-state test + plasmoid review; render → Remote |
| A8 | `cham3-desktop` and `cham3-full` valid metadata + dependency relationships; curated suite in default `cham3-full` | container | PASS | package build + lintian exit 0; metadata + curated-suite-presence tests PASS |
| A9 | Manifests prefer Debian 13; Debian-native equivalents for unavailable; Kali only via reviewed narrow pinning; substitutions/omissions documented; no unreviewed foreign repo enabled | static + VM | PARTIAL — package-relation PASS | disposition matrix; GATED/absent tools excluded from deps and documented; no Kali source in build; transitive image manifest → Remote |
| A10 | Package-source config authenticates origin/signatures; Kali limited to reviewed allowlist+closure; update behavior documented without silent origin change | static + VM | PARTIAL — PASS (no foreign repo) | readiness packet; live config uses official mirrors + apt-secure; no Kali/external repo enabled; LibreWolf/Kali pinning designs are readiness items |
| A11 | Installed `.desktop` entries in correct categories, pass desktop-file validation; launch checks resolve to installed apps | container + install+boot | PARTIAL — static PASS | desktop-file test PASS (no fake backend launchers shipped); installed-launch checks → Remote §5 |
| A12 | Debian KDE utilities present as designed; named redundant alternatives absent (incl. transitive) | container + VM | PARTIAL — package-relation PASS | duplicate-exclusion test PASS on deps; transitive absence in image manifest → Remote §2 |
| A13 | Image/package/desktop-file/ISO-boot/install evidence recorded with exact commands/env + PASS/FAIL/NOT_RUN/BLOCKED; build/boot not called M2–M4 proof | all | IN PROGRESS | this matrix + `container_checks.md` + handoff; Remote adds ISO/boot/install |
| A14 | `docs/STATUS.md` updated only with evidence-backed results after verification; no promotion of planned work | n/a | PASS | STATUS updated with fast-gate PASS and heavy-gate NOT_RUN only |
| A15 | Independent reviewer examines exact target; security review required before READY_TO_MERGE/DONE | review | NOT_RUN | Codex review (pending) |

## Notes
- LibreWolf and the not-in-Debian tools (ghidra, metasploit-framework, exploitdb/searchsploit, zaproxy, wpscan, Cutter) are GATED: documented in the disposition matrix / readiness packet and NOT enabled in the candidate build until their narrowly-pinned, separately-reviewed source passes independent review. Their absence from the first build is an explicit, honest acceptance gap, not a silent omission.
- `dumpcap` capture privilege is resolved through the Debian `wireshark` group mechanism (no unnecessary root); recorded in the disposition matrix and verified at the package/install stage.
