# Handoff: M2-006 stage B2 revision 2 — re-review request (M2-006-B2-01–B2-04)

- **State:** stage B2 revision 2 is **IMPLEMENTED → IN_REVIEW**. Stages A and B1 are DONE; C and D are not started. The author approves nothing.
- **Role:** IMPLEMENTER (Claude Code, Anthropic, on the owner's home computer). Requested reviewer: Codex.
- **Branch:** `codex/m2-cryptographic-contract-core`, pushed fast-forward. No merge.
- **Review answered:** `review/m2-006-b2-security` at 3ca3581 (SHA-256 `45310b6a…c28b`, verified): CHANGES_REQUESTED.
  - Findings: B2-01 (Low), B2-02 (Medium correctness/availability), B2-03 (Low), B2-04 (Low test evidence).
  - No owner question.
- **Base:** ac28d24 (the stage-B2 docs-only commit on 248ca6e).
- **Candidate:** **671a7d86262a6f77a391dd8fcbc99f9888be671a** (one commit, 9 files).
- **Docs-only follow-up:** the commit after 671a7d8, whose parent must be 671a7d8.
- **Responses:** [finding resolutions](../reviews/M2-006-b2-finding-resolutions.md).

## Completed work

- **B2-01:** `reserve_use` and `admit` take trusted now from a second sample under the user lock, after `ready`'s flush, immediately before the decision (`decision_now`).
  - A failed sample refuses `storage_unavailable` before anything is consumed.
  - `admit` had the same order, so it is fixed too (the commit-timestamp rule and the signing window).
- **B2-02:** `restrict` refuses a record that the strict decoder would not accept back unchanged (`malformed_request`), before any effect; the restrictive path checks every body again before it is ledgered.
  - This covers the `identity_mismatch` package identities of the finding.
  - **Author-found, same class:** `restriction` grant ids that are unsorted, repeated or more than 64. Reproduced at 248ca6e (debug and release) and 5ea2407 (debug); present since M2-005; reachable only through the in-process `restrict` API.
- **B2-03:** a new boot ends every session a journal knows, whether started or only bound to a grant, with one `sessions_ended {boot_id}` per user through the restrictive path.
  - It applies to clean and contained starts, with or without a live branch.
  - After it, a start is refused (`lifecycle_rule`) and a new grant bound to such a session is refused (`duplicate_identifier`); the end of such a session returns the boot record.
  - A failure leaves it pending, retried by the next operation, and re-journaled by a same-boot restart. Replay refuses a `sessions_ended` that finds no live session.
- **B2-04:** the stale-head checks are two focused tests, each with its own seeded defect stopping at its named assertion; the unknown-commit contract is checked last.
- **Author placements (open to review):**
  1. `decision_now` also records an anomaly of the second sample and refuses `restriction_not_durable` if that record stays owed.
  2. `well_formed` is a full encode-and-decode round trip, not a field-by-field check, so every record kind is covered by one rule.
  3. `malformed_request` is the code for a record the decoder refuses, as for other request contents a service cannot accept (`min_valid_epoch > epoch`, another boot's `time_sample`).
  4. `sessions_ended` is one record per user per boot (bounded reserve use), has no grant effect of its own, and is written after `branch_terminated` or containment and before `consumed_uncertain`.
  5. policyd's source is unchanged: a malformed identity is denied `unknown_identity`, and its refused report is ignored like any failed report.
- **Tests:** 8 new cargo tests (264 in total): 7 in `lifecycle_consumption`, 1 in `current_authorization`; the record unit test extended. No existing test expectation changed; the record unit test only gains cases.
- **Harness:** 172 seeded defects (13 new). Re-pointed, purpose and test unchanged: `admission_on_a_stale_clock`, `reservation_on_a_stale_clock` (the lines they mutate changed) and `enforcement_of_a_superseded_commit` (now its focused test).
- **Docs:** storage formats §§2, 8, 9; the IPC rows of `admit`, `restrict`, `reserve_use` and `session_event`.

## Verification (at 671a7d86262a6f77a391dd8fcbc99f9888be671a)

| Check | Environment / command | Result | Evidence |
|---|---|---|---|
| The record's controls, unchanged | `reviewer_b2.rs` `24d4b2ad…48f6`, `reviewer_policy_b2.rs` `c1dbabbe…3d72` (digests verified); the record's commands; clean clones; debug and release | 248ca6e: registryd 10/12 (B2-01, B2-03 fail), policyd 0/1 (all three packages quarantine). 671a7d8: **registryd 12/12, policyd 1/1** | `reviewer_reproductions/` |
| Author's `restriction` probe | `author_restriction_probe.rs` `d0a900d4…58b7` | 248ca6e debug and release, 5ea2407 debug: `Ok(5)`, one undecodable record, `replay_defect`. 671a7d8 debug and release: `malformed_request` for both cases, nothing undecodable, nobody quarantined | `reviewer_reproductions/` |
| Defect reasons | each of the 13 new and 3 re-pointed defects alone, with backtraces | all exit 101 at their intended assertion | `harness/defect_reasons_b2r2.log` |
| Full M2 qualification | Clean clone as UID 1001, pinned container (`2a0bcf342c57`), `qualify_m2_rust.sh`, `CHAM3_M2_REQUIRE_CLEAN=1` | **PASS** (`m2 qualification: PASS`): unit 264+264, inventory-exact; harness 149 in 545.5 s with 172 seeded defects, all detected; provider PASS; ECR 19/96/18 with 27 scenarios / 365 steps; trusted-time vectors PASS; differentials 15/72/121/0 and 165 / 4 813 / 0; format differential 5 968 / 0 (corpus unchanged); vault oracle 38; both lock checks PASS; `dirty_tracked 0` | [VM evidence](../evidence/M2-cryptographic-contract-core/m2_006b2_r2_amd64_vm_671a7d8/) |
| Static checks; static tests | same clone and container | **PASS**; 82 OK; clone clean afterwards | `qualification_console.txt` |
| Full-filesystem probe, five wrapper cases | 2 MiB tmpfs | **PASS**; reports byte-identical to 248ca6e's; 5/5 | `storage/` |
| Unchanged inputs | `git diff --exit-code ac28d24 671a7d8 -- …` | no dependency, lock, pin, workflow, Dockerfile, script, oracle, vector, ADR, policyd source or other-crate change; 248ca6e..ac28d24 docs-only | `unchanged_inputs.txt` |
| GitHub Actions at 671a7d8 | m2 38665833821, m1 38665833839, m0 38665833783 | **success**, seven jobs and 88 steps, including the vault protection positive path; artifact 11942412776, digest `sha256:d3679f8e…8965` verified; contents agree with the author run | [CI evidence](../evidence/M2-cryptographic-contract-core/m2_006b2_r2_amd64_ci_671a7d8/) |
| Real sessiond, launcherd, netd and brokers; runtime-id attribution; real crash tests; real hardware clocks; Miri/sanitizers/fuzzing | — | NOT_RUN | M2-007, M3, later milestones; storage formats §7 |

## Remaining

- The stated B2 limits stand (storage formats §7): reserve depletion by many audit gaps, fixtures for the session source, the enforcement services and the consumer.
- A session that registryd never heard of (neither started nor bound) cannot be ended by `sessions_ended`; binding new grants only to sessions of the current boot is sessiond's role (M3).
- Stage C waits for this re-review.

## Review prompt for Codex

~~~text
You are the independent REVIEWER (Codex) for cham3 M2-006 stage B2 revision 2 in Kayyo321/cham3: a bounded re-review
of your findings M2-006-B2-01 to B2-04, plus the author-found same-class defect in restrict. You did not author this
work. You run on the owner's home computer; Docker worked for you before; record BLOCKED only with the exact command
and error.

0. Bring the local clone to the candidate FIRST:
     cd ~/cham3 && git status --porcelain && git stash list
     git fetch origin codex/m2-cryptographic-contract-core review/m2-006-b2-security main
     git checkout codex/m2-cryptographic-contract-core && git reset --hard origin/codex/m2-cryptographic-contract-core
     git rev-parse HEAD        # expect the docs-only commit after 671a7d8 (its parent must be 671a7d8)
     git log --oneline -4      # ac28d24 (B2 docs) -> 671a7d8 (revision 2) -> the docs-only commit
   Move unexpected local changes aside (git stash); never force-push or edit the implementation branch. Read every
   document from this checkout; make the clean clone for testing from it
   (git clone --no-hardlinks ~/cham3 <dir>; git -C <dir> checkout --detach 671a7d86262a6f77a391dd8fcbc99f9888be671a).

Read AGENTS.md; your record review/m2-006-b2-security at 3ca35811776672867440cca632705b67ef920ad6;
docs/reviews/M2-006-b2-finding-resolutions.md; docs/handoffs/M2-006-b2-rereview-r2.md; ADR 0007 §§5, 6 and erratum
E7; docs/specs/m2_registry_storage_v1.md §§2, 8, 9; the changed rows of docs/specs/m2_service_ipc_v1.md. Treat author
claims, including the five author placements of the handoff, as claims to verify.

Targets: candidate 671a7d86262a6f77a391dd8fcbc99f9888be671a (range ac28d24..671a7d8, one commit, 9 files:
core/registryd/src/{record,lineage,registry}.rs, core/registryd/tests/lifecycle_consumption.rs,
core/policyd/tests/current_authorization.rs, tests/m2/harness/test_m2_registry_policy.py, tests/m2/test_inventory.txt,
docs/specs/m2_registry_storage_v1.md, docs/specs/m2_service_ipc_v1.md) and the docs-only commit after it. Confirm
nothing else changed (policyd's source included). Do not merge; do not edit implementation files.

1. Your controls, unchanged (reviewer_b2.rs and reviewer_policy_b2.rs, with your record's commands): expect 12/12
   and 1/1 at the candidate in debug and release (10/12 and 0/1 at 248ca6e); compare with the author's
   reviewer_reproductions/ logs.
2. B2-01: decision_now in reserve_use and admit — the sample after the user lock and ready's flush, immediately
   before the lifetime check and consumption; a failed sample or an owed anomaly record refusing before anything is
   consumed; option B, floors and anomalies unchanged; the admission extension (timestamp_rule and the signing window
   after a flush). Try lock contention, a slow flush and a clock that fails only at the second sample.
3. B2-02: record_body_t::well_formed in restrict (before any effect) and in the restrictive path; the four malformed
   package identities, the valid 255-byte positive control, replay after restart; the author-found restriction case
   (unsorted, repeated, more than 64 ids; author_restriction_probe.rs) and whether any other writer can still
   persist a body the decoder refuses.
4. B2-03: sessions_ended — written at every new boot (clean, contained) for users with a started or grant-bound
   live session, branch or not; never at a same-boot restart; pending, retried, re-journaled once; replay rule and
   forged-record quarantine; starts and new grant admissions refused afterwards; the end's sequence; a quarantined
   user's journal left as found; trunk supersession unchanged.
5. B2-04: a_superseded_trunk_commit_takes_no_confirmation and an_ended_branch_takes_no_confirmation, each with its
   own seeded defect stopping at its named assertion; the unknown-commit contract last.
6. Harness: 172 seeded defects (13 new; admission_on_a_stale_clock, reservation_on_a_stale_clock and
   enforcement_of_a_superseded_commit re-pointed) — judge each new defect's discrimination
   (harness/defect_reasons_b2r2.log); inventory 264 cargo, 149 Python.
7. Evidence: qualification in a clean clone at 671a7d86262a6f77a391dd8fcbc99f9888be671a (CHAM3_M2_REQUIRE_CLEAN=1;
   nofile as before): unit 264+264, harness 149 (172 seeded defects), ECR 19/96/18 with 27 scenarios / 365 steps,
   trusted-time vectors PASS, format 5 968 / 0, the rest as before; static checks and sh tests/run_tests.sh static;
   the full-filesystem wrapper and the five wrapper cases. Compare GitHub Actions m2_checks 38665833821, m1_checks 38665833839,
   m0_checks 38665833783; download the artifact and check its digest (artifact 11942412776,
   sha256:d3679f8e2e2630a1e0c097b9c358bf7ab5c1bcf4ee592758a78953516ecf8965).
8. Codex Security plugin on ac28d24..671a7d8; record name/version and findings.

Record every check as PASS/FAIL/NOT_RUN/BLOCKED with exact commands and evidence. Return APPROVE or
CHANGES_REQUESTED for M2-006 stage B2 revision 2 and make no owner decisions (list questions for the owner
separately). Publish only your record on a new review branch (review/m2-006-b2-r2-security) and end with:

=== CHAM3 M2-006 STAGE B2 R2 VERDICT ===
reviewer: Codex
environment: <OS/arch; UID; Docker access: yes/no and how; image id; nofile limit used>
local_clone: <~/cham3 HEAD after step 0; local changes or stashes: none / moved aside (what)>
review_record: <branch> @ <commit> : <path> (sha256 <digest>)
candidate_reviewed: 671a7d86262a6f77a391dd8fcbc99f9888be671a
docs_commits_reviewed: <shas> (docs-only: yes/no)
scope_exact: <the nine files and nothing else: yes/no>
verdict: M2-006 stage B2 revision 2: <APPROVE/CHANGES_REQUESTED>
reviewer_controls_at_candidate: <counts, debug and release>
b2_01_decision_time: <PASS/FAIL> (<..>)
b2_02_decodable_records: <PASS/FAIL> (<..>)
b2_03_sessions_at_new_boot: <PASS/FAIL> (<..>)
b2_04_stale_head_evidence: <PASS/FAIL> (<..>)
harness_and_seeded_defects: <PASS/FAIL> (<counts>)
full_filesystem_wrapper: <PASS/FAIL>
qualification_amd64: <PASS/FAIL/BLOCKED> (<counts>)
static_checks: <..>   static_tests: <..>
ci_comparison: <..>
security_plugin: <..>
new_findings:
- id: <..> | severity: <..> | location: <file:line> | summary: <..> | required: <..>
owner_questions: <..>
notes_for_owner: <..>
=== END VERDICT ===
~~~
