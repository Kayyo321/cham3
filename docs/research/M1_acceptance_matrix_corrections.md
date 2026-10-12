# M1 acceptance matrix corrections (draft for the coordinator)

- **Purpose:** proposed corrections to `docs/evidence/M1-bootable-desktop-distribution/acceptance_matrix.md`. That file is coordinator-owned and is NOT edited by this branch.
- **Trigger:** Codex review of `b82919c` (`docs/reviews/M1-bootable-desktop-distribution-codex.md`, finding F2) says the supplemental matrix marks A8 PASS while the full-suite part is unsatisfied. Codex also says the matrix must be corrected so unresolved suite coverage cannot pass.
- **Sources:** `M1_package_disposition.md` (sections 2, 7, 8) and `M1_readiness_packet.md` (sections 2 and 6).

## 1. Status vocabulary

The task (`docs/tasks/M1-bootable-desktop-distribution.md`, Required verification) and `AGENTS.md` require each check to be reported as **PASS, FAIL, NOT_RUN, or BLOCKED**. The current matrix also uses `PARTIAL`, which is not one of the four permitted statuses. Replace each `PARTIAL — ...` with `FAIL` (criterion not met), `NOT_RUN` (not executed), or `BLOCKED` (cannot run until a named prerequisite lands), and keep the passing sub-results in the evidence column.

## 2. Replacement rows

The rows below use the criterion wording already in the matrix, abridged. Only the status and evidence columns change.

| # | Proposed status | Proposed evidence text |
|---|---|---|
| A2 | BLOCKED | Build is blocked. live-build `--firmware-chroot true` schedules `firmware-b43-installer` and `firmware-b43legacy-installer`, whose postinsts download from GitHub and `downloads.openwrt.org` during the chroot build (Codex `codex_b82919c/image_attempt.log`; disposition section 6). Unblocks after the config change and a rebuild. |
| A3 | BLOCKED | Depends on A2 (no ISO). |
| A8 | **FAIL** | Criterion clause "the curated security suite is included in the default `cham3-full` package set" is NOT met. Sub-result PASS: `cham3-desktop` and `cham3-full` metadata and dependency relations (package build, lintian exit 0, metadata tests, per Codex). Nine requested items have no approved and verified source (readiness packet section 2): LibreWolf (owner approval pending); mitmproxy (LEAKY); ZAP or Burp (choice pending; Burp licence blocking); amass (allowlist pending); wpscan (LEAKY); metasploit-framework (LEAKY; Rapid7 apt returned HTTP 403, BLOCKED); searchsploit (allowlist pending); ghidra (allowlist and footprint pending); radare2 or Cutter (choice pending; Cutter LEAKY). Do not record A8 as PASS until each item is included with an approved, verified source or recorded as an owner-approved scope amendment. |
| A9 | **FAIL** (sourcing incomplete) | Sub-results: documentation of substitutions and omissions PASS (disposition section 2). Owner-approved narrow pinning NOT_RUN (no allowlist approved). Closure gate NOT_RUN natively (readiness section 6.1; early emulated signal only). No foreign repository is enabled in any build (verified by configuration review). Documented omissions are not owner-approved reductions and do not satisfy the criterion. |
| A10 | **NOT_RUN** (foreign-source clauses) | Sub-results: the Debian-only configuration authenticates origin through the signed archive (apt-secure; PASS, static). The Kali and LibreWolf authenticated-source designs (keyring provenance, `-1` default pin, per-package allowlist, origin-change rule) are written and emulated, not implemented, approved, or run natively (disposition section 3; gates F2-CLOSURE and F2-ORIGIN). Early signal: the `-1` default is required; a Pin-Priority below 100 installed unlisted Kali packages. |
| A11 | FAIL (unchanged) | Codex F5: the default launcher does not expose categorized access to the installed suite. Research does not change this. |
| A12 | NOT_RUN (duplicate exclusion, transitive image) | Dependency-level exclusion (PASS, static). Transitive absence in the built image is NOT_RUN and depends on A2. |
| A13 | NOT_RUN (until native gates run) | Record the research evidence index (disposition section 9) and the gate transcripts with checksums. Emulated results are not native evidence. |
| A14 | Coordinator decision | `docs/STATUS.md` must not describe M1 as complete while A8 is FAIL. Record the fast-gate results and the open suite gap only. |
| A15 | NOT_RUN (unchanged) | Codex rereview is pending. |

## 3. Notes column corrections

- Replace "Their absence from the first build is an explicit, honest acceptance gap, not a silent omission." with: "This is an unresolved acceptance gap (A8 FAIL, A9 FAIL). It is not an accepted reduction. Acceptance requires an owner-approved scope amendment, or the items must be resolved."
- Replace the dumpcap note "resolved through the Debian `wireshark` group mechanism (no unnecessary root)" with: "Capture mechanism is the `wireshark` group with file capabilities. The setuid fallback in the upstream postinst is not closed (Codex F3). The install-setuid answer must stay `true`; `false` removes non-root capture. Gate F3-CAP is NOT_RUN." (disposition section 5).
- The A10 note "LibreWolf/Kali pinning designs are readiness items" is correct. Add: "Early signal: Pin-Priority below 100 is insufficient (unlisted Kali packages were installed in simulation); use -1."

## 4. Gates to fold into `codex_remote_verification.md`

Add these native gates to the Codex Remote list. Full commands and pass criteria are in `M1_readiness_packet.md` section 6.

- **F2-CLOSURE** (native x86-64; simulation only): Kali and LibreWolf key checks, strict closures for the 11 Kali candidates and LibreWolf, controls, measure transcripts, origin preservation, M1-context closures. Pass criteria P1 to P7.
- **F2-ORIGIN** (optional; coordinator approval): upgrade behaviour with one allowlisted package in simulated installed state.
- **FW-B43** (firmware scheduling and fetch): no `firmware-b43*` package in the chroot or the installed system, and no firmware fetch in the build log.
- **F3-CAP** (dumpcap privilege): no setuid bit under a forced `setcap` failure; installed-system mode and capabilities.

Each gate result must be recorded as PASS, FAIL, NOT_RUN, or BLOCKED with the transcript path and checksum.

## 5. Scope check

Nothing here approves a foreign source, an allowlist, or a reduction of scope. Those require owner decisions listed in `M1_package_disposition.md` section 10. This document only changes how the matrix reports the current state.
