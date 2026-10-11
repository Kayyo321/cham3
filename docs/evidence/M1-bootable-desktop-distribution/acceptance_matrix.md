# M1 acceptance-to-evidence matrix

Task: `docs/tasks/M1-bootable-desktop-distribution.md`. Baseline `401c6cf`. This matrix
is filled only with evidence actually produced. Until a check runs it is `NOT_RUN`.
A build/boot result is never described as proof of M2–M4 security enforcement.

Evidence stages: **static** (host/container, no build) · **container** (amd64 `debian:trixie`,
unprivileged) · **VM** (disposable Debian 13 amd64, QEMU TCG) · **host-QEMU boot** · **install+boot**.

| # | Acceptance criterion (abridged) | Stage / method | Status | Evidence |
|---|---|---|---|---|
| A1 | Dedicated task branch/worktree on recorded baseline; exact branch/worktree + target recorded | static | PASS | Task "Assignment and Git"; `git worktree list`; target recorded at integration |
| A2 | Documented Debian 13 amd64 live-build path produces a bootable, installable ISO from declared inputs, no prod creds/undocumented host state | VM | NOT_RUN | planned: `vm_build_transcript.txt`, `provenance.json`, `SHA256SUMS` |
| A3 | In a disposable VM the ISO boots into Plasma 6 Wayland and completes the documented install path; record VM details/artifact identity/limits | host-QEMU boot + install+boot | NOT_RUN | planned: boot capture, session/Plasma-version evidence, install log |
| A4 | Installed desktop uses cham3 branding defaults; 56–64 logical-px vertical panel on the right edge | install+boot | NOT_RUN | planned: screenshot; layout evidence |
| A5 | Panel contains the nine components in ADR-0002 order/presentation (M1 = visual/package foundation) | static + install+boot | NOT_RUN | planned: layout-parse test PASS + screenshot |
| A6 | M2–M4-dependent functions visibly report unavailable/disconnected/unverified; no implied active contracts/aliases/modes/kill/identity | static + install+boot | NOT_RUN | planned: inert-state test PASS + screenshot |
| A7 | Desktop preserves desired/signed/pending/enforced distinction; absent/stale/failed backend → visible unavailable, no false success | static + install+boot | NOT_RUN | planned: inert-state test + review |
| A8 | `cham3-desktop` and `cham3-full` valid metadata + dependency relationships; curated suite in default `cham3-full` | container | NOT_RUN | planned: package build + metadata tests |
| A9 | Manifests prefer Debian 13; Debian-native equivalents for unavailable; Kali only via reviewed narrow pinning; substitutions/omissions documented; no unreviewed foreign repo enabled | static + VM | NOT_RUN | planned: disposition matrix + source-config test + image manifest |
| A10 | Package-source config authenticates origin/signatures; Kali limited to reviewed allowlist+closure; update behavior documented without silent origin change | static + VM | NOT_RUN | planned: source-config test + readiness packet |
| A11 | Installed `.desktop` entries in correct categories, pass desktop-file validation; launch checks resolve to installed apps | container + install+boot | NOT_RUN | planned: desktop-file-validate + launch checks |
| A12 | Debian KDE utilities present as designed; named redundant alternatives absent (incl. transitive) | container + VM | NOT_RUN | planned: exclusion tests + image manifest grep |
| A13 | Image/package/desktop-file/ISO-boot/install evidence recorded with exact commands/env + PASS/FAIL/NOT_RUN/BLOCKED; build/boot not called M2–M4 proof | all | NOT_RUN | this matrix + handoff |
| A14 | `docs/STATUS.md` updated only with evidence-backed results after verification; no promotion of planned work | n/a | PENDING | STATUS updated only post-verification |
| A15 | Independent reviewer examines exact target; security review required before READY_TO_MERGE/DONE | review | NOT_RUN | Codex review (pending) |

## Notes
- LibreWolf and the not-in-Debian tools (ghidra, metasploit-framework, exploitdb/searchsploit, zaproxy, wpscan, Cutter) are GATED: documented in the disposition matrix / readiness packet and NOT enabled in the candidate build until their narrowly-pinned, separately-reviewed source passes independent review. Their absence from the first build is an explicit, honest acceptance gap, not a silent omission.
- `dumpcap` capture privilege is resolved through the Debian `wireshark` group mechanism (no unnecessary root); recorded in the disposition matrix and verified at the package/install stage.
