# M2-006 stage B1 finding resolutions (author response, revision 3)

- **Nature of this record:** the implementer's response to the independent re-review of M2-006 stage B1 revision 2. It is **not** a review and approves nothing.
- **Review answered:** `docs/reviews/M2-006-stage-b1-r2-codex-security.md` at 9b5afa2e3dfd50ce9c25b8d51662108fda90b58a on `review/m2-006-b1-r2-security`.
  - One commit on `main` 9cf8825; the branch adds only that file (421 lines).
  - SHA-256 `1537a5c331c5e18dc52b198157ad926ac729de12a73751642ab1c35da011a78d`, verified. Not merged or edited.
  - It re-reviewed 77a3d0e and the docs-only commit 0ec854d.
- **Verdict:** **CHANGES_REQUESTED**, with one finding: **M2-006-B1-R2-01** (Low). The startup's own in-range sample did not close a resumed anomaly episode, so the next genuine episode went unrecorded.
  - R01: PASS, including fresh and earlier-success policyd instances and both refusal kinds.
  - Also PASS: the rest of R02, the owner clarifications as recorded, the harness (123), qualification, the static gates, the wrapper, the CI comparison and Codex Security (0.1.32, no promoted vulnerability).
  - No owner question.
- **Branch / base / candidate:** `codex/m2-cryptographic-contract-core`. Base 0ec854d (the revision-2 docs-only commit on 77a3d0e). Revision-3 candidate **5ea2407ff2c639361ec2976bc20bc95f5acbce95**, one commit. The [revision-3 handoff](../handoffs/M2-006-b1-rereview-r3.md) has the commands and results.

## Reproduction first

The record's three supplemental snippets were extracted unchanged, and their SHA-256 values match the record:
- `r2_policy_controls.rs` `8fe26726…5e1d`
- `r2_registry_controls.rs` `de0eb57e…c043`
- `r2_startup_control.rs` `bbb3b9ad…1287`

They were composed as its C6 step does: the fixture, a newline, the original B1 snippet, then the supplemental snippet(s), saved as `rereview_r2.rs`. Each was run in a clean clone, in debug and release.

| Clone | policyd `rereview_` | registryd `rereview_` |
|---|---|---|
| 77a3d0e (reviewed) | 1/1 | 3 passed, **1 failed**: `rereview_startup_sample_closes_the_resumed_episode`, failing in its two in-range cases (`startup_mark=2`, episodes `[("boot-1", 1)]` where `[1, 2]` was expected). Its two anomalous-start controls pass. |
| 5ea2407 (this revision) | **1/1** | **4/4** (the startup matrix passes all four cases) |

The original B1 controls (`reviewer_`) at 5ea2407: 5/5 and 9/9. Logs are in `reviewer_reproductions/` of the [VM evidence](../evidence/M2-cryptographic-contract-core/m2_006b1_r3_amd64_vm_5ea2407/).

## Resolution

### R2-01 (Low): the startup sample counts like any other sample

- **Finding:** `trusted_clock_t::start` restored the largest same-boot anomaly mark, but it never assessed its own startup sample against that mark. An in-range startup sample therefore left the resumed episode open. The next lag then counted as the same episode, and the user's existing record suppressed a new one.
- **Fix:** after the episode is resumed, `start` takes its own sample into account through the same `observe` path that every operation uses. The sample is passed as not raised, because the start offset already covers it, so only the episode changes.
  - A sample within 300 s of trusted time ends a resumed episode.
  - A sample more than 300 s behind continues the episode, or opens the next one after a closed episode.
  - A restart alone ends nothing.
- **Durable before serving:** the run marker's anchor write publishes the resulting mark, and a failed write refuses the start. This covers the closure for a later unclean restart too.
- **Still holding:**
  - Uninterrupted anomalous-start continuity, the same-boot maximum merge, and the new-boot reset.
  - At a new boot, a lagging start now opens episode 1 of that boot at once. Before, it opened at the first operation, with the same episode number.
  - Every fail-closed clock and storage rule is unchanged.
- **Code:** `core/registryd/src/clock.rs` (`start`).
- **New tests:**
  - `trusted_time::a_startup_sample_within_300_seconds_ends_the_resumed_episode` crosses clean and unclean restarts with an in-range and an anomalous start. For each case it checks the anchor's mark (3 or 2), then makes a further unclean restart with a renewed lag. Episodes must be `[1, 2]` after an in-range start and `[1]` after an anomalous one.
  - The unit test `clock::tests::the_start_sample_ends_or_continues_the_resumed_episode`. The new-boot expectation in `anomaly_episodes_resume_in_the_same_boot_only` changes accordingly: a lagging start opens episode 1, and a start within range opens none.
- **Seeded defect:** `startup_sample_not_reconciled` (124 in total).
- **Reviewer test:** `rereview_startup_sample_closes_the_resumed_episode` passes in all four cases.
- **Docs:** storage formats §8, plus the status line.

## Scope of the change

- **Files:** `core/registryd/src/clock.rs`, `core/registryd/tests/trusted_time.rs`, `tests/m2/harness/test_m2_registry_policy.py`, `tests/m2/test_inventory.txt` (241 cargo tests, +2) and `docs/specs/m2_registry_storage_v1.md`.
- **Not changed:** every other source file, the dependencies, the lock, the workflows, the oracles, the vectors and every ADR.
