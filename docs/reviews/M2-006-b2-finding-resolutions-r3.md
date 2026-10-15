# M2-006 stage B2 finding resolutions (author response, revision 3)

- **Nature of this record:** the implementer's response to the independent re-review of M2-006 stage B2 revision 2. It is **not** a review and approves nothing.
- **Review answered:** `docs/reviews/M2-006-stage-b2-r2-codex-security.md` at dffab2aa8dc7cb215f48ac2a5c659468527b7fb9 on `review/m2-006-b2-r2-security`.
  - One commit on `main` 9cf8825; the branch adds only that file (255 lines).
  - SHA-256 `d5a4b0101f35308a292603f53b6db447dff8fd52c5eba4522df779258e2928a7`, verified. Not merged or edited.
  - It re-reviewed 671a7d8 and the docs-only commit 14c3e4c.
- **Verdict:** **CHANGES_REQUESTED**, with two findings, both in revision 2's `decision_now`:
  - **M2-006-B2-R2-01** (Low): the decision sample's own persistence (an anomaly record, an offset ledger entry, a floor refresh) could take time after the sample, so the decision used a stale trusted now.
  - **M2-006-B2-R2-02** (Low): a ledger append that failed during the decision sample marked the ledger failed, but nothing re-checked it before the admission or consumption.
  - B2-02, B2-03 and B2-04 PASS; the harness, qualification, static gates, wrapper, CI comparison and Codex Security (0.1.32: two Low findings, R2-01 and R2-02) PASS. No owner question.
- **Branch / base / candidate:** `codex/m2-cryptographic-contract-core`. Base 14c3e4c (the revision-2 docs-only commit on 671a7d8). Revision-3 candidate **8b00ebcff89276d2c0a70a238459fd246e7353b7**, one commit, 6 files (it replaces two unpushed drafts, ed7d529 and 8777fe4, described below; neither was pushed). The [revision-3 handoff](../handoffs/M2-006-b2-rereview-r3.md) has the commands and results.

## Reproduction first

The record's supplemental target `reviewer_r2.rs` was taken unchanged from the reviewer's reproduction clone; its SHA-256 `d83688ac…4724` matches the record. It was run with the record's command (`cargo test --frozen -p cham3_registryd --test reviewer_r2 reviewer_r2_`) in clean clones, debug and release.

| Clone | `reviewer_r2_` | Earlier targets (`reviewer_b2`, `reviewer_policy_b2`, the author's restriction probe) |
|---|---|---|
| 671a7d8 (reviewed) | 2 passed, **4 failed**: the two reservation and two admission cases (`Ok(…)`, `Ok(trunk)`); the owed-anomaly and quarantined-journal controls pass | 12/12, 1/1, 1/1 (revision-2 evidence) |
| 8b00ebc (this revision) | **6/6** | **12/12, 1/1, 1/1** |

Logs are in `reviewer_reproductions/` of the [VM evidence](../evidence/M2-cryptographic-contract-core/m2_006b2_r3_amd64_vm_8b00ebc/).

## Resolution: the decision sample settles before it is used (R2-01, R2-02)

- **Fix:** `decision_now` (`core/registryd/src/registry.rs`) now runs in rounds. Each round:
  1. observes a sample like any other: an offset increase ledgered, the floor refreshed, an anomaly recorded;
  2. refuses `storage_unavailable` if the ledger has failed (R2-02: the same gate `ready` applies, now also after the decision sample's own writes);
  3. refuses `restriction_not_durable` if an anomaly record is owed (a failed journal leaves it owed);
  4. refuses `storage_unavailable` if the sample failed;
  5. takes one further sample, which persists nothing. Trusted now is used only when it equals the observed sample: no time passed during the round's writes and nothing more is owed (R2-01).
  - Otherwise the round repeats with the newer time, at most three rounds (`DecisionSampleRounds`), then refuses `storage_unavailable`. Every refusal comes before anything is admitted or consumed.
- **Why equality:** clocks are sampled in whole seconds, so an unchanged sample means no whole second elapsed and nothing the sample would change (offset, anomaly episode, floor) is pending. Any other answer repeats the round. The bounded refusal fails closed against clocks that never settle.
- **Still holding:** offset ledgering, floors, anomaly episodes and option B are unchanged. Every revision-2 behaviour holds (the reviewer's earlier targets pass). The extra samples change nothing that an ordinary operation's sample would not.
- **Placement (author choice, open to review):** three rounds. In a real system, a round fails only if a second boundary falls between the observed sample and the check, or the round's own writes take a second or more. Three such rounds in a row refuse the operation, which the caller may retry.
- **New tests:**
  - `lifecycle_consumption::a_reservation_waits_for_the_writes_of_its_own_decision_sample`. Five cases are armed on the operation's second sample (the decision sample). Each refuses and consumes nothing:
    - an anomaly record whose journal write takes 600 s (`lifecycle_rule`; the grant expires at 09:05);
    - an offset ledger entry that takes 600 s (`lifecycle_rule`);
    - a floor refresh, 60 s later, whose anchor write takes 600 s (`lifecycle_rule`);
    - a failed offset ledger entry (`storage_unavailable`);
    - clocks that advance at every sample (`storage_unavailable`).
    - Positive controls: the first three writes, quick, reserve the grant.
  - `lifecycle_consumption::an_admission_waits_for_the_writes_of_its_own_decision_sample`: the same five cases at admission (`timestamp_rule` or `storage_unavailable`, nothing admitted), with quick positive controls.
  - **Discrimination:** with 671a7d8's `registry.rs`, every one of the ten cases is accepted (`decision_cases_at_671a7d8.log`).
  - The test fixtures gain ledger and anchor delays and a ledger-append fault (`paced_io_t`) and a scripted time source (`scripted_time_t`).
- **New test for `ready`'s gate:** `lifecycle_consumption::a_failed_ledger_refuses_writes_that_take_no_decision_sample` (see the re-pointed `failed_ledger_keeps_serving` below).
- **Seeded defects (175, +3):**
  - `decision_sample_not_settled` and `admission_decision_sample_not_settled` drop the equality check; they stop at each test's "anomaly record" case.
  - `decision_ignores_a_failed_ledger` drops the ledger gate; it stops at the "failed offset ledger entry" case.
  - **Re-pointed:**
    - `decision_sample_failure_ignored` now mutates the `fresh_now` line inside the round, the only guard against a failed sample. A first draft of this revision guarded it twice, so the defect went undetected; the author's reason run caught this, and the draft was replaced by a single guard before any qualification.
    - `failed_ledger_keeps_serving` (stage A) removes `ready`'s failed-ledger gate. Its test checked that gate only through admission, which now also meets the decision sample's own gate (R2-02), so the defect was masked. The first qualification of this revision (at the unpushed draft 8777fe4) failed on exactly this: `failed_ledger_keeps_serving was not detected`, 174 of 175 detected (the rest of that run passed: unit 266 + 266, static checks, wrapper; its logs were discarded with the draft's clone, so only this line is quoted). The defect is now re-pointed, with its purpose unchanged, to a new test, `a_failed_ledger_refuses_writes_that_take_no_decision_sample` (a session start and an enforcement outcome after the ledger failed: `storage_unavailable`, no record). The stage-A test is unchanged. Both gates are needed: `ready`'s covers failures before the operation, the decision gate failures during its own sample.
    - The two other decision defects of revision 2 and the two stale-clock defects keep their anchors.
- **Reviewer tests:** all six `reviewer_r2_` pass, debug and release.

## Seeded-defect reasons

The three new defects, the two re-pointed ones and the four other decision-time defects were each run alone in a disposable copy of the clean clone at 8b00ebc, with `RUST_BACKTRACE=1` (`harness/defect_reasons_b2r3.log`). All exit 101 at their intended assertion:
- `lifecycle_consumption.rs` lines 1464, 1514, 1576, 2240 (two defects; the failing case's label differs), 2296 and 2364.
- The two stale-clock defects stop inside the shared `code` helper, called from `trusted_time.rs:590` and `lifecycle_consumption.rs:433`.

## Scope of the change

- **Files:**
  - `core/registryd/src/registry.rs`
  - `core/registryd/tests/lifecycle_consumption.rs`
  - `tests/m2/harness/test_m2_registry_policy.py` (175 seeded defects)
  - `tests/m2/test_inventory.txt` (267 cargo tests, +3; 149 Python)
  - `docs/specs/m2_registry_storage_v1.md` (§§8, 9)
  - `docs/specs/m2_service_ipc_v1.md` (the `admit` and `reserve_use` rows)
- **Not changed:** every other source file (`clock.rs`, `ledger.rs`, `record.rs`, `lineage.rs` and policyd's source included), the dependencies, the lock, the workflows, the Dockerfile, the scripts, the oracles, the vectors and every ADR.
