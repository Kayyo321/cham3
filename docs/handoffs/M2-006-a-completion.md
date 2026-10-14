# Handoff: M2-006 stage A — completion after independent re-review

- **State:** M2-006 **stage A is DONE**. Codex's bounded independent re-review approved revision 4 for the stage-A scope recorded in the task file. One owner action is still open, and it does not block the stage: **confirm the corrected wording of ADR 0006 §3 erratum E5**. The reviewer finds the wording technically accurate, but that is not the owner's confirmation. M2-006 as a whole stays IN_PROGRESS. Stage B starts next on its own candidate, under the owner's standing instruction.
- **Role:** IMPLEMENTER. The author approved nothing; the approval comes from the independent reviewer.
- **Model/provider:** Claude Code (Anthropic), on the owner's home computer. Independent reviewer: Codex, on the same computer.
- **Working directory / branch:** `codex/m2-cryptographic-contract-core` (pushed fast-forward).
- **Task specification:** [M2 task](../tasks/M2-cryptographic-contract-core.md), M2-006 stage A. The scope was recorded before the work, on 2026-10-13.
- **Base commit:** 22b6b5f, the M2-005 completion docs. The code base is 5860f27.
- **Approved candidate:** db5bb59fcf461034461d9e0d2f21f5816986134c (revision 4).
  - The implementation is the sequence f4b0628 (revision 1) → e34fd8a (revision 2) → 9633ce3, a2d0ebd, 2217e62 (revision 3) → db5bb59 (revision 4).
  - Each revision has a docs-only evidence commit. The last one is 84464cc6bf750d8a2128c662b2fcc10cb7f8ed4f.
  - The commit that adds this handoff records the approval and changes only `docs/`.

Use `PROJECT.md` and approved ADRs as product-authority references.

## Completed work

- **Delivered:** stage A of `cham3_registryd` storage integrity (ADR 0006 §§2, 3, 4, 6; ADR 0008 §1). Formats and decision tables are in [registryd storage formats v1](../specs/m2_registry_storage_v1.md).
  - **Global anchor:** two alternating checksummed slots, where the valid higher counter wins and equal counters invalidate. The counter continues above rejected slots. The anchor carries the run marker (`running`, `clean`, or `unverifiable`, which is sticky from the first ledger failure) and the persisted policy state.
  - **Restriction ledger:** started fresh every run and appended before any disk write of a restrictive change. A failed append owes the invalidation of the validity file and the `unverifiable` marker. Every IPC operation retries both after its role check until they succeed (`rollback_status.unverifiable_owed`).
  - **Restart decision:** `resumed`, `ledger_replayed` or `contained`.
  - **Per-user recovery table:** torn-tail truncation; quarantines for stale, unexplained, chain-mismatch, replay-defect, lost-store and anchor-invalid states and for tombstones; roll-forward of one durable unanchored record.
  - **Restrictive write path:** covers `restrict`, key invalidation, containment, `set_policy_epoch` and `set_emergency_restriction`. The order is denial → ledger → journal from the reserve → anchor → reply. A restriction that is not yet durable returns `restriction_not_durable` and is retried while pending. The pending queue is drained one anchored record at a time.
  - **Reads:** reads refuse an unanchored state.
  - **`rollback_status`:** reports the anchor mode, run decision, quarantined, unusable and contained users, pending restrictions, ledger failure, owed retries, depleted reserves and the policy state.
  - **`cham3_policyd`:** reads the persisted policy state and forwards the two admin operations.
  - **Full-filesystem probe:** now also starts a ledger.
- **Findings answered:** the reviewer's tests were reproduced failing at the old candidate and passing at the new one, debug and release.
  - R01–R08 of the first review ([resolutions r2](../reviews/M2-006-a-finding-resolutions-r2.md)).
  - R2-01–R2-04 of the revision-2 re-review ([resolutions r3](../reviews/M2-006-a-finding-resolutions-r3.md)).
  - R3-01 and R3-02 of the revision-3 re-review ([resolutions r4](../reviews/M2-006-a-finding-resolutions-r4.md)).
- **Owner decisions used:**
  - R2-03 **Option A** (2026-10-14): the compound-failure limit is ADR 0006 §3 erratum E5. The corrected wording still needs the owner's confirmation.
  - The amd64 Debian lock refresh for libcurl `8.14.1-2+deb13u7` (2026-10-14, commit a2d0ebd).
- **What the approval covers:** the stage-A scope at db5bb59. It does not cover stages B–D or the milestone, and it covers in-process composition only; real IPC, processes and full-system enforcement are M2-007.

## Verification

| Check | Environment/command | Result | Evidence/limits |
|---|---|---|---|
| Author amd64 qualification at db5bb59 | Clean clone as UID 1001, pinned container (image 2a0bcf342c57), `qualify_m2_rust.sh` with `CHAM3_M2_REQUIRE_CLEAN=1` | PASS: unit 209+209; harness 147 (97 registry/policy seeded defects); both lock checks PASS | [VM evidence](../evidence/M2-cryptographic-contract-core/m2_006a_r4_amd64_vm_db5bb59/) |
| Static checks; static tests | same clone and container | PASS; 82 OK | same directory |
| Full-filesystem probe and the five wrapper cases on a 2 MiB tmpfs | same clone, `--tmpfs /full-disk:rw,size=2m,…` | PASS; 5/5 | same directory |
| Reviewer's R3 tests, unchanged | author: clean clones at 2217e62 and db5bb59, debug and release | 7/12 → 12/12 | same directory, `reviewer_reproductions/` |
| GitHub Actions at db5bb59 | m2 38592632636, m1 38592632234, m0 38592632553 | PASS, every job, including the vault protection positive path; artifact 11910945081 digest `f9d7bb84…6671` | [CI evidence](../evidence/M2-cryptographic-contract-core/m2_006a_r4_amd64_ci_db5bb59/) |
| Reviewer's own qualification, static gates, wrapper, CI comparison, artifact digest | Codex: Debian 13.7 x86_64, UID 1001, own no-cache image `sha256:4528aed8…3e56`, `nofile 1024:1024` | PASS, same counts (209+209; harness 147 in 382.3 s; static 82; 5/5; artifact verified) | Review record below |
| Reviewer's tests at db5bb59 | R3 suite; R2 storage, queue, matrix; three new R4 tests (refused callers for nine operations plus the accessors; 32 policy partial-recovery cases; journal-only persistence then marker recovery) | R3 12/12; storage 5/6 and queue 3/4 (the expected E5-excluded case and the obsolete queue premise); matrix 4/4; R4 3/3; debug and release | Review record below |
| Changed seeded defects | reviewer: each of the 11 changed defects fails its named test at the intended runtime assertion | PASS | Review record below |
| Codex Security plugin | `security-diff-scan` 0.1.32 on 908c391..db5bb59 | Ran; zero findings | Review record below; required again at M2-008 |
| Miri / sanitizers / fuzzing; real process death, power loss, kernel crash; real IPC, service processes, deployed enforcement; arm64 qualification | — | NOT_RUN | Not in the pinned toolchain, or M2-007; restarts and reboots are simulated at the io boundaries |

## Review

- **Reviewer and record:** Codex, `review/m2-006-a-r4-security` at 06a9423258ecd84fa823c4817d3fba615d49ff6d, `docs/reviews/M2-006-stage-a-r4-codex-security.md`, SHA-256 `327149fa5ffec50df535cd17c1cec4ededcb8d997150d7a12618d493685a4121`.
  - Verified by the author: the parent is 9cf8825 (`main`) and the branch adds only that file (126 lines).
  - The record was not merged or edited.
- **Verdict:** **APPROVE** for M2-006 stage A revision 4. No new findings.
  - **R3-01:** PASS. The reviewer audited all twelve IPC entry points, the sticky anchor, refused callers and lock order.
  - **R3-02:** PASS. The corrected E5 wording matches the behaviour.
- **Reviewer's reading of E5, for the owner:** "a policy restriction always ends in containment" is meant within E5's failed-ledger recovery context. A policy restriction whose ledger append succeeded can still replay precisely. The reviewer finds the full E5 text supplies that context and needs no technical correction.
- **Earlier records:** all CHANGES_REQUESTED.
  - `review/m2-006-a-security` at 344c372 (R01–R08)
  - `review/m2-006-a-r2-security` at 7d570c1 (R2-01–R2-04)
  - `review/m2-006-a-r3-security` at f0d6eb0 (R3-01, R3-02)

## Remaining

- **Owner action, not blocking:** confirm the corrected ADR 0006 §3 erratum E5 wording, or give a different wording. A different wording is a docs change for the next stage's review.
- **Known limits:**
  - **E5:** outside the guarantee is only the case where every mark fails at once, the process dies before any retry succeeds, and the restart happens in the same boot.
  - **Anchor:** it has no reserve.
  - **Crash testing:** physical crashes are not exercised; faults are injected at the io boundaries.
  - **arm64 locks:** the development locks still record libcurl deb13u5.
- **Carried into later stages:**
  - **Stage B:** lifecycle writers, trusted time, `timestamp_rule`, one-shot reservations, confirmation, reconciliation and `history`.
  - **Stage C:** ceremony, `replace_identity` and deletion.
  - **Stage D:** proposals, restore and delegation.
  - **M2-007:** the ledger's tmpfs placement, transport and processes.
- **Uncommitted paths or dirty state:** none on the branch after this commit.
- **Next action and owner:**
  - **Claude Code:** start M2-006 stage B on top of this commit, as its own candidate with its own qualification, CI and independent review.
  - **The owner:** confirm the E5 wording.
  - The final M2 review requirement (Codex Security plugin at M2-008) stands.
