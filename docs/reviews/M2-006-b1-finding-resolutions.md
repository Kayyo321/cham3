# M2-006 stage B1 finding resolutions (author response, revision 2)

- **Nature of this record:** the implementer's response to the independent review of M2-006 stage B1. It is **not** a review and approves nothing.
- **Review answered:** `docs/reviews/M2-006-stage-b1-codex-security.md` at 792f5f0f8836f63f5823e9e98c514bb7b2df93ff on `review/m2-006-b1-security`.
  - One commit on `main` 9cf8825, adding only that file (585 lines).
  - SHA-256 `534a20807969d586f7c80b59cae0b934060aae1922eed2e7deed9f1584f3e5f9`, verified against the verdict the owner relayed. Not merged or edited.
- **Reviewed candidate:** c9129bca0e6c4b1e6caf72774e05a27e09a4d542 (range e7803ef..c9129bc) and the docs-only commit b4e4cdc.
- **Verdict:** **CHANGES_REQUESTED**, with two findings:
  - **M2-006-B1-R01** (Medium): a failed `time_sample` still permitted the current eligibility and Direct/Tor.
  - **M2-006-B1-R02** (Low): an uninterrupted anomaly episode was recorded twice across a same-boot restart.
  - PASS: the offset persistence controls, observed start, `timestamp_rule`, new boot, the oracles, the harness (119/119), qualification, static gates, the full-filesystem wrapper, the CI comparison and the Codex Security run (0.1.32).
- **Owner decisions (2026-10-14):** the owner answered the reviewer's two questions, each with the recommended option. Both are recorded in ADR 0007 as wording clarifications, with no mechanism change:
  - **E4:** the refusal itself writes nothing. Clock persistence that §5 and E6 require may happen before the timestamp check.
  - **E6:** registryd's own clock sample is the anomaly observation, so policyd does not report backward samples separately.
- **Branch / base / candidate:** `codex/m2-cryptographic-contract-core`. Base b4e4cdc (the B1 docs-only commit on c9129bc). Revision-2 candidate **77a3d0e65072b60f4ce1780a709dd342fdda67f7**, one commit on the base. Commands and results are in the [revision-2 handoff](../handoffs/M2-006-b1-rereview-r2.md).

## Reproduction first

- **Source:** the reviewer's two snippets were extracted unchanged from the record, and their SHA-256 values match the record's table:
  - `policy_reviewer_snippet.rs` `c33ade32…b25b`
  - `registry_reviewer_snippet.rs` `014db1de…a47`
- **Method:** each snippet was appended to the candidate's `current_authorization.rs` or `trusted_time.rs`, exactly as the record's C6 step composes them. Both were run in clean clones in the pinned container, debug and release, with `--test-threads=1`.

| Clone | policyd `reviewer_` | registryd `reviewer_` |
|---|---|---|
| c9129bc (reviewed) | 3 passed, **2 failed**. `reviewer_time_sample_ledger_failure_denies_current_evaluation`: `ledger_failed=true; current=eligible {…}`. `reviewer_network_time_sample_failure_stays_offline`: `mode { mode: direct, … }`. | 8 passed, **1 failed**. `reviewer_anomaly_episode_survives_same_boot_restart`: two `(1791882000, 400)` records. |
| 77a3d0e (this revision) | **5 passed, 0 failed** | **9 passed, 0 failed** |

These are the record's 11 PASS / 3 FAIL, in both profiles. Logs are in `reviewer_reproductions/` of the [VM evidence](../evidence/M2-cryptographic-contract-core/m2_006b1_r2_amd64_vm_77a3d0e/).

## Resolution matrix

### R01 (Medium): a refused shared-time report must not leave the current decision eligible

- **Finding:** `evaluation_time` discarded a failed `time_sample` (`if let Ok`). It then evaluated the snapshot read before the failure, with the old offset. After the ledger failed during the report, the current decision was still `eligible`, and `network_mode` still returned Direct/Tor.
- **Resolution:** a refused report now decides nothing from that snapshot. The current outcome is `unknown`, so it is denied and the network is Offline:
  - `registry_unavailable` when registryd refused for storage (the failed ledger: ADR 0006 §3, no eligible decision);
  - `clock_unavailable` when registryd answered `malformed_request` (the services disagree on the clock).
- **Not changed:** nothing of the refused sample is adopted. The no-fault path and the registry's own `time_sample` checks are unchanged.
- **Change in an existing test:** `policyd_reports_a_forward_sample_and_relies_on_it_only_after_registryd_took_it` now expects `unknown(clock_unavailable)` for a refused sample. Before, it evaluated at the shared offset.
- **Code:** `core/policyd/src/service.rs` (`evaluation_time`).
- **New policyd tests:**
  - `a_time_sample_whose_ledger_append_fails_denies_the_current_evaluation`: the current and the next decision are `unknown(registry_unavailable)`, the ledger has failed, and no offset is adopted.
  - `a_time_sample_whose_ledger_append_fails_leaves_the_network_offline`: Direct and Tor both give `offline_unknown(registry_unavailable)`.
  - `a_forward_sample_registryd_takes_expires_the_grant_at_once`: the no-fault control, `denied(expired)` with registryd at T1 + 200.
- **Seeded defect:** `failed_time_sample_ignored`. The earlier defect `policyd_relies_on_an_unreported_sample` can no longer be detected, because the refused path now returns before the assignment it changed. It is replaced by `policyd_never_reports_a_forward_sample`, which targets the same test.
- **Reviewer tests:** `reviewer_time_sample_ledger_failure_denies_current_evaluation` and `reviewer_network_time_sample_failure_stays_offline` now pass.

### R02 (Low): an anomaly episode must survive a same-boot restart

- **Finding:** the clock reset the anomaly episode at every start, and the user's reported episode lived only in memory. A same-boot restart inside one episode therefore wrote a second identical `clock_anomaly`.
- **Episode state persisted:** the clock keeps the episode state as one monotonic **mark**: `0` none, `2·episode` open, `2·episode + 1` closed, so a larger mark is always a later state.
  - The mark is written to the ledger as a time entry whenever an episode opens or ends.
  - It is published with every anchor floor, and the merge keeps the larger value.
  - A same-boot start resumes the largest mark among this boot's floors, so the episode continues open or stays closed. A new boot starts at 0, because records name their boot.
- **Users' reports recognised:** each `clock_anomaly` record carries `episode` (≥ 1), so a user has reported an episode when its journal holds a record of this boot and episode. That record may have been replayed or written in this run.
- **Distinct episodes stay distinct:** this is not deduplication of equal payloads. A persisted closure makes the next lag a new episode, which each user reports again.
- **Code:**
  - `core/registryd/src/clock.rs`: `anomaly_state_t::mark` and `from_mark`, `time_floor_t::anomaly`, the merge, and the resume in `start`.
  - `anchor.rs` and `ledger.rs`: the mark appended to the floor and to the time payload (the anchor stays at format version 2: B1 is not approved and nothing is stored).
  - `record.rs`: `clock_anomaly.episode`.
  - `registry.rs`: `observe_time` ledgers every transition; `record_anomaly` checks the journal's records of this boot and episode.
- **Tests:**
  - New `trusted_time::a_clock_anomaly_episode_survives_same_boot_restarts`. One user is followed through a clean restart and then an unclean restart (`ledger_replayed`) within the same episode; neither writes a second record.
  - In the same test: a second user reports the resumed episode exactly once; a persisted end then an unclean restart gives a new episode 2, with records `[1, 2]`; and a new boot counts from 1 again.
  - New unit test `clock::tests::anomaly_episodes_resume_in_the_same_boot_only`.
  - The record and ledger round trips cover the new fields.
- **Seeded defects:**
  - new: `anomaly_episode_not_resumed`, `anomaly_transition_not_ledgered`, `replayed_anomaly_records_ignored`;
  - re-pointed at the combined check: `anomaly_recorded_at_every_operation`.
- **Reviewer test:** `reviewer_anomaly_episode_survives_same_boot_restart` now passes.

### Nonblocking correction

- The anchor diagram in storage formats §3 now reads `version (2)`, and it shows the time floor's `anomaly_mark`.

## Owner clarifications (recorded in ADR 0007 under erratum E6)

1. **E4:** "no other durable change" means the refusal itself writes nothing: no journal record, and no index or anchor change for the refused commit. The clock persistence that §5 and E6 require during the same call may happen before the check. This is a ledger entry for an accepted forward sample, a refreshed floor, or a `clock_anomaly`. With a settled clock, a refusal changes no stored byte, as the reviewer's byte comparison showed.
2. **E6:** registryd's own sample is the anomaly observation. policyd's evaluation reaches registryd through `lineage_head`, which samples the clock and records the anomaly. A backward sample that only policyd sees is recorded by registryd's next operation for that user. No IPC change.

## Scope of the change

- **Rust:**
  - `core/policyd/src/service.rs` and `core/registryd/src/{clock,anchor,ledger,record,registry}.rs`.
  - Tests: `core/policyd/tests/current_authorization.rs` (3 new tests, one assertion changed, a ledger-fault io) and `core/registryd/tests/{trusted_time,trusted_time_model}.rs` (1 new test; the differential ignores the mark, which the model lacks).
- **Harness:** `test_m2_registry_policy.py`, with 123 seeded defects (+4, 2 re-pointed).
- **Inventory:** 239 cargo tests (+5).
- **Docs:** ADR 0007 (the E4/E6 clarifications), the decisions index, and storage formats §§2–4 and 8. In the service IPC spec, the `time_sample` row.
- **Not changed:**
  - any dependency, `Cargo.lock`, pin, workflow or Dockerfile;
  - the oracles and the vectors (the model has no episodes; vectors byte-identical);
  - `lineage.rs`, `journal.rs`, identityd, contractd and vaultd.

## Carried forward

- Real hardware clocks, suspend, time synchronization and reboots with the RTC set back stay NOT_RUN, as do real crash tests, IPC and enforcement.
- The ADR 0007 §5.7 limits stand.
- Stage B2 waits for the re-review.
