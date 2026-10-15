# Handoff: M2-006 stage B2 revision 3 — re-review request (M2-006-B2-R2-01, R2-02)

- **State:** stage B2 revision 3 is **IMPLEMENTED → IN_REVIEW**. Stages A and B1 are DONE; C and D are not started. The author approves nothing.
- **Role:** IMPLEMENTER (Claude Code, Anthropic, on the owner's home computer). Requested reviewer: Codex.
- **Branch:** `codex/m2-cryptographic-contract-core`, pushed fast-forward. No merge.
- **Review answered:** `review/m2-006-b2-r2-security` at dffab2a (SHA-256 `d5a4b010…28a7`, verified): CHANGES_REQUESTED, R2-01 (Low) and R2-02 (Low). No owner question.
- **Base:** 14c3e4c (the revision-2 docs-only commit on 671a7d8).
- **Candidate:** **8b00ebcff89276d2c0a70a238459fd246e7353b7** (one commit, 6 files).
- **Docs-only follow-up:** the commit after 8b00ebc, whose parent must be 8b00ebc.
- **Responses:** [finding resolutions r3](../reviews/M2-006-b2-finding-resolutions-r3.md).

## Completed work

- **R2-01 and R2-02:** `decision_now` relies on its sample only once settled. Each round:
  - observes a sample (offset ledgered, floor refreshed, anomaly recorded);
  - refuses if the ledger failed meanwhile (`storage_unavailable`), an anomaly record is owed (`restriction_not_durable`) or the sample failed (`storage_unavailable`);
  - returns trusted now only when a further sample, which persists nothing, equals the observed one.
  - Otherwise the round repeats, at most three times, then refuses (`storage_unavailable`). Every refusal comes before anything is admitted or consumed.
- **Tests:** three new cargo tests (267 in total). Two cover the decision sample: Five cases each, for reservation and admission, armed on the decision sample: a 600 s anomaly record, a 600 s offset ledger entry, a 600 s floor refresh, a failed offset ledger entry, and clocks that never settle. Quick positive controls. All ten cases are accepted by 671a7d8's code. The third, `a_failed_ledger_refuses_writes_that_take_no_decision_sample`, shows `ready`'s failed-ledger gate apart from the new one. No existing expectation changed.
- **Harness:** 175 seeded defects (+3). Two are re-pointed:
  - `decision_sample_failure_ignored`, to the `fresh_now` line inside the round (now the only guard against a failed sample).
  - The stage-A `failed_ledger_keeps_serving`, to the new `ready`-gate test. Admission now also meets the decision sample's gate, which masked this defect; the first qualification of an unpushed draft (8777fe4) failed on it, 174 of 175 detected. See the resolutions.
- **Docs:** storage formats §§8, 9; the IPC rows of `admit` and `reserve_use`.
- **Author placements (open to review):**
  1. Equality of the observed and a further sample is the settle test (whole-second clocks).
  2. At most three rounds, then a refusal the caller may retry.
  3. The failed-ledger gate refuses `storage_unavailable`, as `ready` does.

## Verification (at 8b00ebcff89276d2c0a70a238459fd246e7353b7)

| Check | Environment / command | Result | Evidence |
|---|---|---|---|
| The records' controls, unchanged | `reviewer_r2.rs` `d83688ac…4724`, `reviewer_b2.rs` `24d4b2ad…48f6`, `reviewer_policy_b2.rs` `c1dbabbe…3d72` (digests verified); the records' commands; clean clones; debug and release | 671a7d8: `reviewer_r2_` 2/6 (the four finding cases fail). 8b00ebc: **`reviewer_r2_` 6/6, `reviewer_b2` 12/12, `reviewer_policy_b2` 1/1, restriction probe 1/1** | `reviewer_reproductions/` |
| The new tests against the old decision code | 8b00ebc's tests (each case printed) with 671a7d8's `registry.rs` | all ten cases accepted (the defect) | `reviewer_reproductions/decision_cases_at_671a7d8.log` |
| Defect reasons | the 3 new, the 2 re-pointed and 4 other decision-time defects alone, with backtraces | all exit 101 at their intended assertion | `harness/defect_reasons_b2r3.log` |
| Full M2 qualification | Clean clone as UID 1001, pinned container (`2a0bcf342c57`), `qualify_m2_rust.sh`, `CHAM3_M2_REQUIRE_CLEAN=1` | **PASS** (`m2 qualification: PASS`): unit 267+267, inventory-exact; harness 149 in 533.6 s with 175 seeded defects, all detected; provider PASS; ECR 19/96/18 with 27 scenarios / 365 steps; trusted-time vectors PASS; differentials 15/72/121/0 and 165 / 4 813 / 0; format differential 5 968 / 0 (corpus unchanged); vault oracle 38; both lock checks PASS; `dirty_tracked 0` | [VM evidence](../evidence/M2-cryptographic-contract-core/m2_006b2_r3_amd64_vm_8b00ebc/) |
| Static checks; static tests | same clone and container | **PASS**; 82 OK; clone clean afterwards | `qualification_console.txt` |
| Full-filesystem probe, five wrapper cases | 2 MiB tmpfs | **PASS**; reports byte-identical to 671a7d8's; 5/5 | `storage/` |
| Unchanged inputs | `git diff --exit-code 14c3e4c 8b00ebc -- …` | only the six files changed; no dependency, lock, workflow, script, oracle, vector, ADR, clock, ledger, record, lineage or policyd change; 671a7d8..14c3e4c docs-only | `unchanged_inputs.txt` |
| GitHub Actions at 8b00ebc | m2 38755817749, m1 38755817592, m0 38755817888 | **success**, seven jobs and 88 steps, including the vault protection positive path; artifact 11984943374, digest `sha256:57177faa…43b2` verified; contents agree with the author run | [CI evidence](../evidence/M2-cryptographic-contract-core/m2_006b2_r3_amd64_ci_8b00ebc/) |
| Real sessiond, launcherd, netd and brokers; runtime-id attribution; real crash tests; real hardware clocks; Miri/sanitizers/fuzzing | — | NOT_RUN | M2-007, M3, later milestones; storage formats §7 |

## Remaining

- The stated B2 limits stand (storage formats §7).
- Stage C waits for this re-review.

## Review prompt for Codex

~~~text
You are the independent REVIEWER (Codex) for cham3 M2-006 stage B2 revision 3 in Kayyo321/cham3: a bounded re-review
of your findings M2-006-B2-R2-01 and R2-02. You did not author this work. You run on the owner's home computer;
Docker worked for you before; record BLOCKED only with the exact command and error.

0. Bring the local clone to the candidate FIRST:
     cd ~/cham3 && git status --porcelain && git stash list
     git fetch origin codex/m2-cryptographic-contract-core review/m2-006-b2-r2-security main
     git checkout codex/m2-cryptographic-contract-core && git reset --hard origin/codex/m2-cryptographic-contract-core
     git rev-parse HEAD        # expect the docs-only commit after 8b00ebc (its parent must be 8b00ebc)
     git log --oneline -4      # 14c3e4c (r2 docs) -> 8b00ebc (revision 3) -> the docs-only commit
   Move unexpected local changes aside (git stash); never force-push or edit the implementation branch. Read every
   document from this checkout; make the clean clone for testing from it
   (git clone --no-hardlinks ~/cham3 <dir>; git -C <dir> checkout --detach 8b00ebcff89276d2c0a70a238459fd246e7353b7).

Read AGENTS.md; your record review/m2-006-b2-r2-security at dffab2aa8dc7cb215f48ac2a5c659468527b7fb9;
docs/reviews/M2-006-b2-finding-resolutions-r3.md; docs/handoffs/M2-006-b2-rereview-r3.md; ADR 0007 §5, §8 and
erratum E7; docs/specs/m2_registry_storage_v1.md §§8, 9; the changed rows of docs/specs/m2_service_ipc_v1.md. Treat
author claims, including the three author placements of the handoff, as claims to verify.

Targets: candidate 8b00ebcff89276d2c0a70a238459fd246e7353b7 (range 14c3e4c..8b00ebc, one commit, 6 files:
core/registryd/src/registry.rs, core/registryd/tests/lifecycle_consumption.rs, tests/m2/harness/test_m2_registry_policy.py,
tests/m2/test_inventory.txt, docs/specs/m2_registry_storage_v1.md, docs/specs/m2_service_ipc_v1.md) and the docs-only
commit after it. Confirm nothing else changed. Do not merge; do not edit implementation files.

1. Your controls, unchanged (reviewer_r2.rs with your record's command, and your earlier reviewer_b2.rs and
   reviewer_policy_b2.rs): expect 6/6, 12/12 and 1/1 at the candidate in debug and release (reviewer_r2_ 2/6 at
   671a7d8); compare with the author's reviewer_reproductions/ logs.
2. R2-01: decision_now's rounds — the observed sample, its own writes (offset ledger entry, floor refresh, anomaly
   record), then a further sample that persists nothing and must equal it; repeating with the newer time; the bound of
   three rounds and its refusal; option B, floors and anomaly episodes unchanged. Try a write that takes time on each
   of the three paths, clocks that keep moving, and a real second boundary.
3. R2-02: the failed-ledger gate after the decision sample (offset entry and anomaly mark appends), refusing before
   anything is admitted or consumed; restrictive writes and recovery unchanged.
4. Harness: 175 seeded defects (decision_sample_not_settled, admission_decision_sample_not_settled and
   decision_ignores_a_failed_ledger new; decision_sample_failure_ignored and the stage-A failed_ledger_keeps_serving
   re-pointed, the latter to the new test a_failed_ledger_refuses_writes_that_take_no_decision_sample) — judge their
   discrimination (harness/defect_reasons_b2r3.log); inventory 267 cargo, 149 Python.
5. Evidence: qualification in a clean clone at 8b00ebcff89276d2c0a70a238459fd246e7353b7 (CHAM3_M2_REQUIRE_CLEAN=1;
   nofile as before): unit 267+267, harness 149 (175 seeded defects), the rest as before; static checks and
   sh tests/run_tests.sh static; the full-filesystem wrapper and the five wrapper cases. Compare GitHub Actions
   m2_checks 38755817749, m1_checks 38755817592, m0_checks 38755817888; download the artifact and check its digest
   (artifact 11984943374, sha256:57177faaa9ff5223bf6611316ccd8aee3296851dcb941986dae7ee401b9443b2).
6. Codex Security plugin on 14c3e4c..8b00ebc; record name/version and findings.

Record every check as PASS/FAIL/NOT_RUN/BLOCKED with exact commands and evidence. Return APPROVE or
CHANGES_REQUESTED for M2-006 stage B2 revision 3 and make no owner decisions (list questions for the owner
separately). Publish only your record on a new review branch (review/m2-006-b2-r3-security) and end with:

=== CHAM3 M2-006 STAGE B2 R3 VERDICT ===
reviewer: Codex
environment: <OS/arch; UID; Docker access: yes/no and how; image id; nofile limit used>
local_clone: <~/cham3 HEAD after step 0; local changes or stashes: none / moved aside (what)>
review_record: <branch> @ <commit> : <path> (sha256 <digest>)
candidate_reviewed: 8b00ebcff89276d2c0a70a238459fd246e7353b7
docs_commits_reviewed: <shas> (docs-only: yes/no)
scope_exact: <the six files and nothing else: yes/no>
verdict: M2-006 stage B2 revision 3: <APPROVE/CHANGES_REQUESTED>
reviewer_controls_at_candidate: <counts, debug and release>
r2_01_settled_decision_time: <PASS/FAIL> (<..>)
r2_02_failed_ledger_gate: <PASS/FAIL> (<..>)
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
