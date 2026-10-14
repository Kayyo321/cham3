# Handoff: M2-006 stage B1 revision 3 — re-review request (M2-006-B1-R2-01)

- **State:** stage B1 revision 3 is **IMPLEMENTED → IN_REVIEW**. Stage A is DONE; B2, C and D are not started. The author approves nothing.
- **Role:** IMPLEMENTER (Claude Code, Anthropic, on the owner's home computer). Requested reviewer: Codex.
- **Branch:** `codex/m2-cryptographic-contract-core`, pushed fast-forward. No merge.
- **Review answered:** `review/m2-006-b1-r2-security` at 9b5afa2 (SHA-256 `1537a5c3…a78d`, verified): CHANGES_REQUESTED, R2-01 (Low). No owner question.
- **Base:** 0ec854d (the revision-2 docs-only commit on 77a3d0e).
- **Candidate:** **5ea2407ff2c639361ec2976bc20bc95f5acbce95** (one commit, 5 files).
- **Docs-only follow-up:** the commit after 5ea2407, whose parent must be 5ea2407.
- **Responses:** [finding resolutions r3](../reviews/M2-006-b1-finding-resolutions-r3.md).

## Completed work

- **R2-01:** `trusted_clock_t::start` now takes its own sample into account once it has resumed the episode of this boot, through the same `observe` path every operation uses.
  - A sample within 300 s of trusted time ends a resumed episode. A sample more than 300 s behind opens or continues one. A restart alone ends nothing.
  - The run marker's anchor write publishes the mark before anything is served, and a failed write refuses the start.
- **Tests:**
  - New `trusted_time::a_startup_sample_within_300_seconds_ends_the_resumed_episode` (clean and unclean restart, crossed with an in-range and an anomalous start, plus a further unclean restart).
  - New clock unit test.
  - One expectation changed in the existing clock test: a lagging start at a new boot now opens episode 1 at once.
- **Harness and inventory:** the seeded defect `startup_sample_not_reconciled` (124 in total); inventory 241.
- **Docs:** storage formats §8.

## Verification (at 5ea2407ff2c639361ec2976bc20bc95f5acbce95)

| Check | Environment / command | Result | Evidence |
|---|---|---|---|
| The record's controls, unchanged | its three supplemental snippets plus the original B1 snippets (digests verified), composed as its C6; clean clones; debug and release | 77a3d0e: registryd 3/4 (the startup matrix's two in-range cases fail), policyd 1/1. 5ea2407: **policyd 1/1, registryd 4/4**. The original B1 controls at 5ea2407: 5/5 and 9/9 | `reviewer_reproductions/` |
| Full M2 qualification | clean clone as UID 1001, `2a0bcf342c57`, `CHAM3_M2_REQUIRE_CLEAN=1` | **PASS**: unit 241+241; harness 149 in 423.3 s with 124 seeded defects; ECR 27/365; trusted-time vectors 165; clock differential 4 813 / 0; layer-7 differential 0; format differential 5 968 / 0; both lock checks PASS | [VM evidence](../evidence/M2-cryptographic-contract-core/m2_006b1_r3_amd64_vm_5ea2407/) |
| Defect reasons | each of the 27 B1 defects alone | all fail at their intended assertion | `harness/defect_reasons_b1.log` |
| Static checks; static tests | same clone | PASS; 82 OK | `qualification_console.txt` |
| Full-filesystem probe, wrapper cases | 2 MiB tmpfs | PASS; reports byte-identical; 5/5 | `storage/` |
| GitHub Actions at 5ea2407 | m2 38641025183, m1 38641025244, m0 38641025091 | **success**, every job and step, including the vault protection positive path; artifact 11931692301, digest `sha256:c0caa359…e2b0` verified | [CI evidence](../evidence/M2-cryptographic-contract-core/m2_006b1_r3_amd64_ci_5ea2407/) |
| Hardware clocks, suspend, time sync, real crashes, IPC, enforcement; Miri/sanitizers/fuzzing | — | NOT_RUN | — |

## Remaining

- The ADR 0007 §5.7 limits stand.
- B2 waits for this re-review.

## Review prompt for Codex

~~~text
You are the independent REVIEWER (Codex) for cham3 M2-006 stage B1 revision 3 in Kayyo321/cham3: a bounded re-review
of your finding M2-006-B1-R2-01. You did not author this work. You run on the owner's home computer; Docker worked for
you before; record BLOCKED only with the exact command and error.

0. Bring the local clone to the candidate FIRST:
     cd ~/cham3 && git status --porcelain && git stash list
     git fetch origin codex/m2-cryptographic-contract-core review/m2-006-b1-r2-security main
     git checkout codex/m2-cryptographic-contract-core && git reset --hard origin/codex/m2-cryptographic-contract-core
     git rev-parse HEAD        # expect the docs-only commit after 5ea2407 (its parent must be 5ea2407)
     git log --oneline -4      # 0ec854d (r2 docs) -> 5ea2407 (revision 3) -> the docs-only commit
   Move unexpected local changes aside (git stash); never force-push or edit the implementation branch. Read every
   document from this checkout; make the clean clone for testing from it
   (git clone --no-hardlinks ~/cham3 <dir>; git -C <dir> checkout --detach 5ea2407ff2c639361ec2976bc20bc95f5acbce95).

Read AGENTS.md; your record review/m2-006-b1-r2-security at 9b5afa2e3dfd50ce9c25b8d51662108fda90b58a;
docs/reviews/M2-006-b1-finding-resolutions-r3.md; docs/handoffs/M2-006-b1-rereview-r3.md; ADR 0007 erratum E6 and its
clarification; docs/specs/m2_registry_storage_v1.md §8. Treat author claims as claims to verify.

Targets: candidate 5ea2407ff2c639361ec2976bc20bc95f5acbce95 (range 0ec854d..5ea2407, one commit, 5 files:
core/registryd/src/clock.rs, core/registryd/tests/trusted_time.rs, tests/m2/harness/test_m2_registry_policy.py,
tests/m2/test_inventory.txt, docs/specs/m2_registry_storage_v1.md) and the docs-only commit after it. Confirm nothing
else changed. Do not merge; do not edit implementation files.

1. Your controls, unchanged (your three supplemental snippets composed as in your C6, and your original B1 controls):
   expect all to pass at the candidate in debug and release (the startup matrix's two in-range cases failed at
   77a3d0e); compare with the author's reviewer_reproductions/ logs.
2. R2-01: the start's own sample through observe (raised false): an in-range start ends a resumed episode; an
   anomalous start continues it or opens the next after a closure; a restart alone ends nothing; durability before
   serving through the run marker's anchor write (and a refused start when it fails); a further unclean restart after
   the closure; a new boot (a lagging start opens episode 1 of that boot at once — the changed unit expectation); a
   contained or invalid-anchor start; that no offset or authorization outcome changes.
3. Harness: 124 seeded defects (startup_sample_not_reconciled new) — judge its discrimination; inventory 241.
4. Evidence: qualification in a clean clone at 5ea2407 (unit 241+241, harness 149, the rest as before); static checks
   and static tests; the full-filesystem wrapper and the five wrapper cases. Compare GitHub Actions m2_checks 38641025183, m1_checks 38641025244,
   m0_checks 38641025091; download the artifact and check its digest (artifact 11931692301,
   sha256:c0caa359a6c5e6a7daba6931f1c5fa8589c18ec0694ed8ff7dc0429c947ae2b0).
5. Codex Security plugin on 0ec854d..5ea2407; record name/version and findings.

Record every check as PASS/FAIL/NOT_RUN/BLOCKED with exact commands and evidence. Return APPROVE or
CHANGES_REQUESTED for M2-006 stage B1 revision 3 and make no owner decisions (list questions for the owner
separately). Publish only your record on a new review branch (review/m2-006-b1-r3-security) and end with:

=== CHAM3 M2-006 STAGE B1 R3 VERDICT ===
reviewer: Codex
environment: <OS/arch; UID; Docker access: yes/no and how; image id; nofile limit used>
local_clone: <~/cham3 HEAD after step 0; local changes or stashes: none / moved aside (what)>
review_record: <branch> @ <commit> : <path> (sha256 <digest>)
candidate_reviewed: 5ea2407ff2c639361ec2976bc20bc95f5acbce95
docs_commits_reviewed: <shas> (docs-only: yes/no)
scope_exact: <the five files and nothing else: yes/no>
verdict: M2-006 stage B1 revision 3: <APPROVE/CHANGES_REQUESTED>
reviewer_controls_at_candidate: <counts, debug and release>
r2_01_startup_sample: <PASS/FAIL> (<..>)
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
