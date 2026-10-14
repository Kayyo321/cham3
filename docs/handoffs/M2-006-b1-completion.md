# Handoff: M2-006 stage B1 — completion after independent re-review

- **State:** M2-006 **stage B1 is DONE**. Codex's bounded independent re-review approved revision 3 for the stage-B1 scope recorded in the task file, with no new findings and no owner questions. M2-006 as a whole stays IN_PROGRESS. Stage B2 starts next on its own candidate, under the owner's standing instruction.
- **Role:** IMPLEMENTER. The author approved nothing; the approval comes from the independent reviewer.
- **Model/provider:** Claude Code (Anthropic), on the owner's home computer. Independent reviewer: Codex, on the same computer.
- **Working directory / branch:** `codex/m2-cryptographic-contract-core` (pushed fast-forward). No merge of `main` or of a review branch.
- **Task specification:** [M2 task](../tasks/M2-cryptographic-contract-core.md), M2-006 stage B1, as recorded by the owner's stage B split (2026-10-14) and ADR 0007 erratum E6.
- **Base commit:** e7803efaeb346ac94e12aca6ce32e2fabaa3cadd (E6, docs-only, after the stage-A completion).
- **Approved candidate:** 5ea2407ff2c639361ec2976bc20bc95f5acbce95 (revision 3).
  - The implementation is the sequence c9129bc (revision 1) → 77a3d0e (revision 2) → 5ea2407 (revision 3).
  - Each revision has a docs-only evidence commit: b4e4cdc, 0ec854d and 16944ab29bb6406ce0ed08210a6ef326c2712ee3.
  - The commit that adds this handoff records the approval and changes only `docs/`.

Use `PROJECT.md` and approved ADRs as product-authority references. Formats are in [registryd storage formats v1](../specs/m2_registry_storage_v1.md) §§2–4 and 8.

## Completed work

- **Delivered:** stage B1 of M2-006, trusted time (ADR 0007 §5, §8 option B; errata E4 and E6).
  - **registryd's clock:** trusted now is `boottime + offset`, the offset being the largest accepted in this boot. Every increase is ledgered before the clock adopts it. Floors live in the anchor (format version 2), merged by maximum and refreshed at least every 60 s. A same-boot start resumes from the anchor floor and a replayed ledger's time entries; a new boot starts no earlier than the last floor. CLOCK_BOOTTIME comes from `/proc/uptime`.
  - **Option B:** the observed-time counter, `commit_admitted.observed` at admission, and policyd's start rule.
  - **E4:** `timestamp_rule` at admission, from the operation's own sample.
  - **E6:** `clock_anomaly` once per user per episode. The episode state is a monotonic mark, ledgered at every transition, published with the anchor floor and resumed in the same boot. The start's own sample ends or continues a resumed episode, and the result is published before anything is served.
  - **Also:** record time from trusted time, `time_sample`, the offsets in `lineage_head`, the clock in `rollback_status`, and `branch_terminated` for every live temporary branch at a new boot.
  - **policyd:** evaluates with `max(own, snapshot)`, relies on a forward sample only after `time_sample` answered, and denies (unknown; Offline) when the report is refused.
  - **Oracles:** `trusted_time_vectors.py` (165 cases, 4 813 steps, 0 disagreements against the clock); `ecr_oracle.py` with trusted now and four E4 scenarios (27 / 365).
- **Findings answered:** the reviewer's tests were reproduced failing at the old candidate and passing at the new one, debug and release.
  - R01 (Medium) and R02 (Low) of the first review ([resolutions](../reviews/M2-006-b1-finding-resolutions.md)).
  - R2-01 (Low) of the revision-2 re-review ([resolutions r3](../reviews/M2-006-b1-finding-resolutions-r3.md)).
- **Owner decisions used:**
  - ADR 0007 §8 option B (2026-10-12); E4 (2026-10-13).
  - The stage B split and E6 (2026-10-14), recorded before the work.
  - The E4 and E6 wording clarifications (2026-10-14): a refusal itself writes nothing; registryd's own sample is the anomaly observation.
- **What the approval covers:** the stage-B1 scope at 5ea2407. It does not cover stages B2–D or the milestone, and it covers in-process composition only; real IPC, processes and full-system enforcement are M2-007.

## Verification

| Check | Environment/command | Result | Evidence/limits |
|---|---|---|---|
| Author amd64 qualification at 5ea2407 | Clean clone as UID 1001, pinned container (image 2a0bcf342c57), `qualify_m2_rust.sh` with `CHAM3_M2_REQUIRE_CLEAN=1` | PASS: unit 241+241; harness 149 (124 registry/policy seeded defects); ECR 27/365; trusted-time 165 / 4 813 / 0; both lock checks PASS | [VM evidence](../evidence/M2-cryptographic-contract-core/m2_006b1_r3_amd64_vm_5ea2407/) |
| Static checks; static tests | same clone and container | PASS; 82 OK | same directory |
| Full-filesystem probe and the five wrapper cases on a 2 MiB tmpfs | same clone | PASS; 5/5 | same directory |
| Reviewer's R2 startup matrix, unchanged | author: clean clones at 77a3d0e and 5ea2407, debug and release | 2/4 → 4/4; every earlier control passes | same directory, `reviewer_reproductions/` |
| GitHub Actions at 5ea2407 | m2 38641025183, m1 38641025244, m0 38641025091 | PASS, every job and step; artifact 11931692301 digest `c0caa359…e2b0` | [CI evidence](../evidence/M2-cryptographic-contract-core/m2_006b1_r3_amd64_ci_5ea2407/) |
| Reviewer's own qualification, static gates, wrapper, CI comparison, artifact digest | Codex: Debian 13.7 x86_64, UID 1001, own image `sha256:4528aed8…3e56`, `nofile 1024:1024` | PASS, same counts (241+241; harness 149; 124 defects; static 82; 5/5; seven jobs / 88 steps; artifact verified) | Review record below |
| Reviewer's controls at 5ea2407 | original B1 controls and three supplemental snippets; a new fault control (failed slot replacement or directory sync at start, clean and unclean) | 14/14 and 5/5; fault control 4/4 cases refused before serving; debug and release | Review record below |
| New seeded defect | reviewer: `startup_sample_not_reconciled` fails `trusted_time.rs:764` (mark `Some(2)` instead of `Some(3)`) | PASS | Review record below |
| Codex Security plugin | `codex-security` 0.1.32 on 0ec854d..5ea2407 | Ran; zero findings | Review record below; required again at M2-008 |
| Real hardware clocks, suspend, time synchronization, reboots with the RTC set back; physical crashes; real IPC, processes, deployed enforcement; Miri / sanitizers / fuzzing; arm64 qualification | — | NOT_RUN | Clocks come from a settable test source and boots are simulated through `boot_id`; faults are injected at the io boundaries |

## Review

- **Reviewer and record:** Codex, `review/m2-006-b1-r3-security` at 3a978fb3bcdfc9b7546915a763e66604baf4f2a7, `docs/reviews/M2-006-stage-b1-r3-codex-security.md`, SHA-256 `b081399edb8ec14aad147f4bfa1c72eab806450cd5181a2ccf230028be53fb08`.
  - Verified by the author: the parent is 9cf8825 (`main`) and the branch adds only that file (287 lines).
  - The record was not merged or edited.
- **Verdict:** **APPROVE** for M2-006 stage B1 revision 3. No new findings, no owner questions.
  - **R2-01:** PASS. The start's own sample goes through `observe` with `raised` false; the run marker's anchor write publishes the mark before serving, and a failed write refuses the start. The closure survives a further unclean restart.
  - **Continuity, new boot, containment:** PASS. The changed new-boot unit expectation (a lagging start opens episode 1 at once) is initialization followed by observation, not a relaxation of the boot reset.
  - **Offsets and authorization:** PASS within the component scope; the mark is absent from `time_view_t` and from the temporal arithmetic.
- **Earlier records:** both CHANGES_REQUESTED.
  - `review/m2-006-b1-security` at 792f5f0 (R01, R02)
  - `review/m2-006-b1-r2-security` at 9b5afa2 (R2-01)

## Remaining

- **Known limits (ADR 0007 §5.7, unchanged):** lag can accumulate across reboots with a clock that was set back; an accepted forward error is never corrected (option B only keeps it from advancing an admitted start; expirations come early, which fails closed).
- **Reviewer's notes for the owner:** component evidence only; physical clocks, suspend, real crashes and deployed IPC/enforcement remain NOT_RUN. The trusted-time differential omits the anomaly mark, which the model lacks; integration and reviewer controls cover it.
- **Carried into later stages:**
  - **Stage B2:** the lifecycle writers (the expiry latch → `expired`, `session_event` → `session_ended`, the remaining narrowing records), one-shot `reserve_use`/`complete_use`, `confirm_enforcement`, startup reconciliation, audit gaps and `history`.
  - **Stage C:** ceremony, `replace_identity` and deletion.
  - **Stage D:** proposals, restore and delegation.
  - **M2-007:** the ledger's tmpfs placement, transport and processes.
- **Uncommitted paths or dirty state:** none on the branch after this commit.
- **Next action and owner:**
  - **Claude Code:** start M2-006 stage B2 on top of this commit, as its own candidate with its own qualification, CI and independent review.
  - The final M2 review requirement (Codex Security plugin at M2-008) stands.
