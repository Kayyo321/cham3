# M2-006 stage B2 finding resolutions (author response, revision 2)

- **Nature of this record:** the implementer's response to the independent review of M2-006 stage B2. It is **not** a review and approves nothing.
- **Review answered:** `docs/reviews/M2-006-stage-b2-codex-security.md` at 3ca35811776672867440cca632705b67ef920ad6 on `review/m2-006-b2-security`.
  - One commit on `main` 9cf8825; the branch adds only that file (294 lines).
  - SHA-256 `45310b6a22e82ed0736b7c479a045883ffd8b1d97772a067fd663e4345cfc28b`, verified. Not merged or edited.
  - It reviewed 248ca6e and the docs-only commit ac28d24.
- **Verdict:** **CHANGES_REQUESTED**, with four findings:
  - **B2-01** (Low security): reservation lifetime checked at the operation's first sample, after blocking preparation.
  - **B2-02** (Medium correctness/availability): a malformed `identity_mismatch` is persisted and quarantines the user at the next start.
  - **B2-03** (Low lifecycle): recorded sessions survive a new boot.
  - **B2-04** (Low test evidence): `enforcement_of_a_superseded_commit` stops at the unknown-commit assertion.
  - Everything else PASS, including outcomes and reconciliation, confirmations, audit gaps, history, qualification, the static gates, the wrapper, the CI comparison and Codex Security (0.1.32: one low finding, B2-01). No owner question.
- **Branch / base / candidate:** `codex/m2-cryptographic-contract-core`. Base ac28d24 (the docs-only commit on 248ca6e). Revision-2 candidate **671a7d86262a6f77a391dd8fcbc99f9888be671a**, one commit, 9 files. The [revision-2 handoff](../handoffs/M2-006-b2-rereview-r2.md) has the commands and results.

## Reproduction first

The record's two supplemental targets were taken unchanged from the reviewer's reproduction clone, and their SHA-256 values match the record:
- `reviewer_b2.rs` `24d4b2ad…48f6`
- `reviewer_policy_b2.rs` `c1dbabbe…3d72`

Each was run with the record's own commands (`cargo test --frozen -p cham3_registryd --test reviewer_b2 reviewer_`, `cargo test --frozen -p cham3_policyd --test reviewer_policy_b2 reviewer_`) in a clean clone, in debug and release.

| Clone | registryd `reviewer_` | policyd `reviewer_` |
|---|---|---|
| 248ca6e (reviewed) | 10 passed, **2 failed**: `reviewer_reservation_resamples_after_pending_write_delay` (B2-01: `Ok(…)` at actual T1+660) and `reviewer_new_boot_ends_registered_sessions` (B2-03: `Ok(6)`, then `Ok(temporary)`) | **0/1**: all three malformed packages leave one undecodable record and `quarantined=[(1001, replay_defect)]` (B2-02) |
| 671a7d8 (this revision) | **12/12** | **1/1** (`invalid_records=0; quarantined=[]` for each) |

Logs are in `reviewer_reproductions/` of the [VM evidence](../evidence/M2-cryptographic-contract-core/m2_006b2_r2_amd64_vm_671a7d8/).

## Resolutions

### B2-01 (Low): decisions at a sample taken after waiting

- **Finding:** `reserve_use` sampled the clock before the user lock. After the lock, `ready` may flush owed writes, which can block; `fresh_now` then only checked that the earlier sample had succeeded and read the cached clock. The lifetime re-check could use time from before that wait.
- **Fix:** a new `decision_now` takes a second sample under the user lock, after `ready`, immediately before the decision.
  - The sample is observed like any other: an offset increase is ledgered and an anomaly is recorded.
  - A failed sample refuses `storage_unavailable` before anything is consumed. An anomaly record the sample leaves owed refuses `restriction_not_durable`.
  - The first sample keeps its role (the per-operation observation, the anomaly of the user).
- **Same class, also fixed:** `admit` had the same order (sample → lock → `ready` → cached now) for the commit-timestamp rule and the signing window. It now uses `decision_now` too, so a commit stamped more than 300 s before the trusted now after a flush is refused `timestamp_rule`. Every existing admission test and both differentials pass unchanged.
- **Still holding:** §8 option B (the observed counter is read after the second sample), the anomaly and floor persistence, and every fail-closed rule.
- **Code:** `core/registryd/src/registry.rs` (`decision_now`, `admit`, `reserve_use`).
- **New tests:**
  - `lifecycle_consumption::a_reservation_decides_at_a_sample_taken_after_it_waited`. Three cases, each refusing without consuming:
    - A 600 s flush of an owed write crosses the expiration.
    - User-lock contention crosses the expiration. A writer holds the lock inside a gated journal write; the reservation is shown to have taken its first sample (a counting time source); 600 s pass; the writer is released.
    - The clock cannot be sampled after the flush (`storage_unavailable`).
    - Then the positive control.
  - `lifecycle_consumption::an_admission_decides_at_a_sample_taken_after_it_waited`: `timestamp_rule` after a 600 s flush, then a commit stamped at the new trusted now is admitted.
- **Seeded defects:** `reservation_decides_on_its_first_sample`, `admission_decides_on_its_first_sample`, `decision_sample_failure_ignored`. Two earlier defects were re-pointed because the lines they mutate changed. They keep their purpose (a cached time instead of a sample) and their tests: `admission_on_a_stale_clock` (stage B1) and `reservation_on_a_stale_clock` (stage B2).
- **Reviewer test:** `reviewer_reservation_resamples_after_pending_write_delay` passes.

### B2-02 (Medium): no record that replay would refuse is ever written

- **Finding:** the runtime identity's package string was copied into `identity_mismatch`. `restrict` checked the lineage and the inequality but not the decoder's rules. An empty, 256-byte or newline-bearing package identity was persisted, and the next start quarantined the user (`replay_defect`).
- **Fix:** one rule at registryd's writer boundary: `record_body_t::well_formed` (the body encodes, and the strict decoder accepts it back unchanged).
  - `restrict` checks it after the role check, before any effect. A record that fails is refused `malformed_request`: nothing is denied, ledgered or journaled, nothing is owed, and the journal stays replayable.
  - The restrictive path checks it again before anything is ledgered (defense in depth: every current internal writer produces decodable bodies, so no test reaches this second check alone).
  - The decoder's package rule is now the shared `valid_package_identity`.
  - policyd is unchanged. Its decision for such an identity stays `unknown_identity`; its report is refused and ignored, as any failed report is.
- **Same class, found by the author:** `restrict`'s `check_record` accepted a `restriction` whose grant ids are known but not strictly ascending, repeated or more than 64, while the decoder refuses all three.
  - Reproduced with `author_restriction_probe.rs` (SHA-256 `d0a900d4…58b7`) at 248ca6e (debug and release) and at stage B1's approved 5ea2407 (debug): a descending pair returns `Ok(5)`, leaves one undecodable record and quarantines the user at the next start.
  - The defect dates to M2-005 (6e9d6d3). No production code builds a `restriction`, so it is reachable only through the in-process `restrict` API (contractd, policyd, identityd, admin).
  - The same check fixes it. At 671a7d8 the probe passes, debug and release (both cases `malformed_request`, nothing undecodable, nobody quarantined).
- **Code:** `core/registryd/src/record.rs` (`valid_package_identity`, `well_formed`), `core/registryd/src/registry.rs` (`restrict`, `restrictive_write`).
- **New tests:**
  - `lifecycle_consumption::restrict_refuses_a_record_that_replay_would_refuse`:
    - Malformed `identity_mismatch` package identities: empty, 256 bytes, a newline, a space.
    - Malformed `restriction` grant ids: descending, repeated, 65 ids.
    - For each: `malformed_request`; the journal and the ledger unchanged; nothing pending; every grant current.
    - Positive controls: the 255-byte package identity and 64 ascending ids are recorded, and a restart quarantines nobody.
  - `current_authorization::a_package_identity_no_record_can_name_leaves_the_journal_replayable` (end to end through policyd): the four malformed identities are denied `unknown_identity` with nothing journaled; the 255-byte one is reported once; a restart quarantines nobody.
  - The record unit test checks `well_formed` for every kind and the new negative cases.
- **Seeded defects:**
  - `records_written_unchecked` (`well_formed` always true), detected by the registryd test.
  - `unnameable_package_identity_journaled` (the same mutation), detected by the policyd test.
  - `malformed_restriction_denied_before_the_check` (`restrict`'s check removed: the restriction's in-memory denial happens, caught by "nothing denied").
- **Reviewer test:** `reviewer_malformed_runtime_package_must_not_poison_journal` passes for all three cases.

### B2-03 (Low): a new boot ends every session the journal knows

- **Finding:** session starts were persisted, but a new boot only ended branches, and a user without a live branch was skipped. An old session's start was acknowledged in the new boot, and a fresh grant bound to it was admitted. ADR 0007 §6 says all sessions end at a new boot and ids are single-use.
- **Fix:** a new registry event `sessions_ended {boot_id}`.
  - It ends every session the journal knows and has not ended, whether started by a `session_event` or only bound to an admitted grant.
  - **When written:** when the anchor's run marker names another boot, after `branch_terminated` or containment. One record per usable, unquarantined user (normal or contained, live branch or not) that has such a session.
  - **How:** in memory at once, then through the restrictive path (ledger, reserve-backed journal, anchor). A failure leaves it pending, retried by the user's next operation and re-journaled by a same-boot restart.
  - A same-boot restart ends nothing.
  - **Replay:** the record requires a live session and ends them all at that point; a forged one without a live session quarantines the user. It has no grant effect of its own.
  - **After it:** a start of such a session is refused (`lifecycle_rule`), and a grant bound to it is refused at admission (`duplicate_identifier`). Its end returns the boot record's sequence.
  - Ordinary trunk supersession is unchanged: it ends branches, not sessions.
- **Placement (author choice, open to review):** one record per user rather than one per session, so that a boot never consumes more than one reserve record per user. The `branch_terminated` record of stage B1 keeps its meaning.
- **Code:** `core/registryd/src/record.rs` (the event), `lineage.rs` (`live_sessions`, `has_live_sessions`, `end_sessions`, `session_ended_by_boot`, the replay rule), `registry.rs` (`end_sessions` at start, the end lookup in `session_event`).
- **New tests:**
  - `lifecycle_consumption::a_new_boot_ends_every_session_the_journal_knows`:
    - A user without a branch and an unbound started session; a user with a live branch, a grant bound to an unreported session and a started session; a session already ended.
    - A same-boot restart ends nothing. At the new boot, each user gets one record (`branch_terminated` then `sessions_ended` for the second user).
    - All three sessions refuse a start, and new grants bound to old sessions are refused for both users.
    - The old session's end returns the boot record; the already ended one keeps its own.
    - A session of the new boot survives a same-boot restart, and nothing is written again.
    - An unclean run across boots writes `containment`, `audit_gap` and `sessions_ended`.
    - A forged `sessions_ended` quarantines the user.
  - `lifecycle_consumption::a_new_boot_that_cannot_journal_still_ends_every_session`: a failing journal at the new boot leaves the record pending and ledgered with the session already refused. It becomes durable once, through the next operation or through a same-boot restart (`ledger_replayed`).
  - The record unit test covers `sessions_ended`.
- **Seeded defects:** `sessions_survive_a_new_boot`, `grant_bound_sessions_survive_a_new_boot`, `contained_users_keep_their_sessions`, `forged_sessions_ended_replayed`, `boot_ended_session_end_not_durable`, `sessions_end_only_once_durable`.
- **Reviewer test:** `reviewer_new_boot_ends_registered_sessions` passes (`Err(admission(lifecycle_rule))`, then `Err(admission(duplicate_identifier))`).

### B2-04 (Low): each stale-head property on its own

- **Finding:** with the live-head guard removed, `enforcement_status_counts_only_this_runs_outcomes` stopped at the earlier unknown-commit assertion, so the superseded-trunk and ended-branch assertions were never shown to discriminate.
- **Fix (tests only):**
  - Two focused tests, each asserting at its own line with the property named in the message (a local `refusal` helper, so a success fails the test's own assertion rather than the shared `code` helper):
    - `a_superseded_trunk_commit_takes_no_confirmation`: the superseded genesis, then the live trunk head as the positive control.
    - `an_ended_branch_takes_no_confirmation`: a live branch head first as the positive control, then a branch ended by a trunk commit and one ended by a new boot. The unknown-commit contract is checked last.
  - `enforcement_of_a_superseded_commit` is re-pointed to the first test, and the new `enforcement_of_an_ended_branch` (the same mutation) to the second. Each stops at its named assertion: `lifecycle_consumption.rs:1978` and `:2035`.
  - The existing test is unchanged.

## Seeded-defect reasons

Each of the 13 new and 3 re-pointed defects was run alone in a disposable copy of the clean clone at 671a7d8 with its named test and `RUST_BACKTRACE=1` (`harness/defect_reasons_b2r2.log`, with the calling test lines). All exit 101.
- 13 stop at their named assertion in the test file (lines 1432, 1482, 1544, 1616, 1632, 1736, 1755, 1790, 1828, 1859, 1904, 1978 and 2035 of `lifecycle_consumption.rs`).
- `unnameable_package_identity_journaled` stops where the test reads the journal back after the first malformed evaluation (`current_authorization.rs:2241`, `decodes: malformed`): the journal holds an undecodable record, which is the property.
- The two re-pointed stale-clock defects stop inside the shared `code` helper, called from their intended "clock cannot be sampled" assertions: `trusted_time.rs:590` and `lifecycle_consumption.rs:433`.

## Scope of the change

- **Files:**
  - `core/registryd/src/{record,lineage,registry}.rs`
  - `core/registryd/tests/lifecycle_consumption.rs`
  - `core/policyd/tests/current_authorization.rs`
  - `tests/m2/harness/test_m2_registry_policy.py` (172 seeded defects)
  - `tests/m2/test_inventory.txt` (264 cargo tests, +8; 149 Python)
  - `docs/specs/m2_registry_storage_v1.md` (§§2, 8, 9)
  - `docs/specs/m2_service_ipc_v1.md` (the `admit`, `restrict`, `reserve_use` and `session_event` rows)
- **Not changed:** policyd's source, every other source file, the dependencies, the lock, the workflows, the Dockerfile, the scripts, the oracles, the vectors and every ADR.
