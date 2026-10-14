# Handoff: M2-006 stage B1 revision 2 — re-review request (M2-006-B1-R01, R02)

- **State:** stage B1 revision 2 is **IMPLEMENTED → IN_REVIEW**. Stage A is DONE; B2, C and D are not started. The author approves nothing.
- **Role:** IMPLEMENTER. **Model/provider:** Claude Code (Anthropic), on the owner's home computer. Requested reviewer: Codex.
- **Branch:** `codex/m2-cryptographic-contract-core`, pushed fast-forward. No merge of `main` or of a review branch.
- **Review answered:** `review/m2-006-b1-security` at 792f5f0 (SHA-256 `534a2080…e5f9`, verified), verdict CHANGES_REQUESTED:
  - R01, Medium;
  - R02, Low.
- **Owner decisions (2026-10-14), each the recommended option:**
  - **E4 clarification:** the refusal itself writes nothing. Required clock persistence may happen before the check.
  - **E6 clarification:** registryd's own sample is the anomaly observation.
  - Both are recorded in ADR 0007 as wording only.
- **Base:** b4e4cdc (the B1 docs-only commit on c9129bc).
- **Candidate:** **77a3d0e65072b60f4ce1780a709dd342fdda67f7** (one commit, 15 files).
- **Docs-only follow-up:** the commit after 77a3d0e, whose parent must be 77a3d0e.
- **Responses:** [finding resolutions](../reviews/M2-006-b1-finding-resolutions.md).

## Completed work

- **R01:** `core/policyd/src/service.rs` `evaluation_time`. A refused `time_sample` makes the current outcome `unknown`: `registry_unavailable` after a storage refusal, `clock_unavailable` after `malformed_request`. That means denied, and Offline for the network. Nothing of the sample is adopted. Three new policyd tests cover `evaluate`, `network_mode` (Direct and Tor) and the no-fault control. One assertion of an existing test changed: a malformed refusal is now unknown instead of a fallback to the shared offset.
- **R02:** the anomaly episode state is a monotonic mark: 0 none, `2·episode` open, `2·episode + 1` closed.
  - The mark is ledgered at every opening and end, published with every anchor floor (merged by maximum), and resumed by a same-boot start. A new boot starts at 0.
  - `clock_anomaly` carries `episode`. A user that already holds a record of this boot and episode, replayed or written this run, writes none.
  - Tests: a new `trusted_time` test (clean and unclean restarts within an episode, a second user, a persisted end then a new episode, a new boot) and a clock unit test.
  - The anchor floor and the ledger time payload gain the mark. The anchor stays at version 2: B1 is unapproved and nothing is stored.
- **Defects and inventory:** 123 seeded defects. The four new ones are `failed_time_sample_ignored`, `anomaly_episode_not_resumed`, `anomaly_transition_not_ledgered` and `replayed_anomaly_records_ignored`. `anomaly_recorded_at_every_operation` is re-pointed at the combined check. `policyd_relies_on_an_unreported_sample` could no longer be detected, because the refused path now returns before that assignment; it is replaced by `policyd_never_reports_a_forward_sample`. Inventory 239.
- **Docs:** ADR 0007, with the E4/E6 clarifications and the errata line, and the decisions index. Storage formats §§2–4 and 8, with the anchor diagram now at `version (2)` and the `anomaly_mark`. The IPC `time_sample` row.

## Verification (at 77a3d0e65072b60f4ce1780a709dd342fdda67f7)

| Check | Environment / command | Result | Evidence |
|---|---|---|---|
| Reviewer's controls, unchanged | the record's snippets (digests verified), composed as in its C6; clean clones; debug and release | c9129bc: **11/14** (policyd 3/5, registryd 8/9, as recorded). 77a3d0e: **14/14** | `reviewer_reproductions/` |
| Full M2 qualification | clean clone as UID 1001, `2a0bcf342c57`, `CHAM3_M2_REQUIRE_CLEAN=1` | **PASS**: unit 239+239; harness 149 in 427.8 s with 123 seeded defects; ECR 27/365; trusted-time vectors 165; clock differential 4 813 / 0; layer-7 differential 15/72/121/0; format differential 5 968 / 0; both lock checks PASS | [VM evidence](../evidence/M2-cryptographic-contract-core/m2_006b1_r2_amd64_vm_77a3d0e/) |
| Defect reasons | each of the 26 B1 defects alone | all fail at their intended assertion | `harness/defect_reasons_b1.log` |
| Static checks; static tests | same clone | PASS; 82 OK | `qualification_console.txt` |
| Full-filesystem probe, wrapper cases | 2 MiB tmpfs | PASS; reports byte-identical; 5/5 | `storage/` |
| GitHub Actions at 77a3d0e | m2 38632964069, m1 38632964091, m0 38632964078 | **success**, every job and step, including the vault protection positive path; artifact 11928017840, digest `sha256:18b67347…1f40` verified | [CI evidence](../evidence/M2-cryptographic-contract-core/m2_006b1_r2_amd64_ci_77a3d0e/) |
| Hardware clocks, suspend, time sync, real crashes, IPC, enforcement; Miri/sanitizers/fuzzing | — | NOT_RUN | — |

## Remaining

- The ADR 0007 §5.7 limits stand.
- B2 waits for this re-review.
- No uncommitted state after the docs commit.

## Review prompt for Codex

~~~text
You are the independent REVIEWER (Codex) for cham3 M2-006 stage B1 revision 2 in Kayyo321/cham3: a bounded re-review
of your two findings (M2-006-B1-R01, R02). You did not author this work. You run on the owner's home computer; Docker
worked for you before; record BLOCKED only with the exact command and error.

0. Bring the local clone to the candidate FIRST:
     cd ~/cham3 && git status --porcelain && git stash list
     git fetch origin codex/m2-cryptographic-contract-core review/m2-006-b1-security main
     git checkout codex/m2-cryptographic-contract-core && git reset --hard origin/codex/m2-cryptographic-contract-core
     git rev-parse HEAD        # expect the docs-only commit after 77a3d0e (its parent must be 77a3d0e)
     git log --oneline -4      # b4e4cdc (B1 docs) -> 77a3d0e (revision 2) -> the docs-only commit
   Move unexpected local changes aside (git stash); never force-push or edit the implementation branch. Read every
   document from this checkout; make the clean clone for testing from it
   (git clone --no-hardlinks ~/cham3 <dir>; git -C <dir> checkout --detach 77a3d0e65072b60f4ce1780a709dd342fdda67f7).
   Your image cham3-m2-independent-r3-final has the current locks.

Read AGENTS.md; your record review/m2-006-b1-security at 792f5f0f8836f63f5823e9e98c514bb7b2df93ff;
docs/reviews/M2-006-b1-finding-resolutions.md; docs/handoffs/M2-006-b1-rereview-r2.md; ADR 0007 erratum E6 and the
E4/E6 clarifications under it; docs/specs/m2_registry_storage_v1.md §§2-4, 8 and the time_sample row of
docs/specs/m2_service_ipc_v1.md. Treat author claims as claims to verify.

Targets: candidate 77a3d0e65072b60f4ce1780a709dd342fdda67f7 (range b4e4cdc..77a3d0e, one commit, 15 files:
core/policyd/src/service.rs, core/policyd/tests/current_authorization.rs, core/registryd/src/{anchor,clock,ledger,
record,registry}.rs, core/registryd/tests/{trusted_time,trusted_time_model}.rs, docs/decisions/0007_m2_lifecycle_time_
consumption.md, docs/decisions/README.md, docs/specs/m2_registry_storage_v1.md, docs/specs/m2_service_ipc_v1.md,
tests/m2/harness/test_m2_registry_policy.py, tests/m2/test_inventory.txt) and the docs-only commit after it. Confirm
nothing else changed (no dependency, lock, workflow, oracle or vector). Do not merge; do not edit implementation files.

1. Your controls, unchanged (your two snippets, SHA-256 c33ade32...b25b and 014db1de...a47, composed as in your C6):
   expect 14/14 at the candidate in debug and release (11/14 at c9129bc); compare with the author's
   reviewer_reproductions/ logs.
2. R01: every path of policyd's evaluation_time after a refused time_sample (storage refusal, malformed, another
   boot) for evaluate and network_mode; that nothing of a refused sample is adopted; that the no-fault expiry control,
   the registry's own time_sample checks and restrictions during a sample failure are preserved; the changed
   assertion (a malformed refusal is now unknown, not a fallback). Try a refusal on the first evaluation of a fresh
   policyd and after an earlier successful report.
3. R02: the anomaly mark (encoding, monotonic order, merge, ledgering of every transition, publication in every
   anchor image, same-boot resume, new-boot reset), the episode field of clock_anomaly and replay, the check against
   the journal's records, distinct episodes and users; clean and unclean restarts within an episode, a closure only
   in the ledger, a closure then a new episode, a contained or invalid-anchor start. Judge whether persisting the
   mark could change any authorization outcome (it should not).
4. The owner's E4/E6 clarifications as recorded (wording only; make no owner decision).
5. Harness: 123 seeded defects (4 new, 2 re-pointed, one replaced) — judge each changed defect's discrimination;
   inventory 239.
6. Evidence: qualification in a clean clone at 77a3d0e (unit 239+239, harness 149, ECR 27/365, trusted-time vectors
   PASS, the rest as before); static checks and static tests; the full-filesystem wrapper and the five wrapper cases.
   Compare GitHub Actions m2_checks 38632964069, m1_checks 38632964091, m0_checks 38632964078; download the
   artifact and check its digest (artifact 11928017840, sha256:18b673470e4e73478a2fe314ed627c7324bb14d092d44bd5bff9f24a08751f40).
7. Codex Security plugin on b4e4cdc..77a3d0e; record name/version and findings.

Record every check as PASS/FAIL/NOT_RUN/BLOCKED with exact commands and evidence. Return APPROVE or
CHANGES_REQUESTED for M2-006 stage B1 revision 2 and make no owner decisions (list questions for the owner
separately). Publish only your record on a new review branch (review/m2-006-b1-r2-security) and end with:

=== CHAM3 M2-006 STAGE B1 R2 VERDICT ===
reviewer: Codex
environment: <OS/arch; UID; Docker access: yes/no and how; image id; nofile limit used>
local_clone: <~/cham3 HEAD after step 0; local changes or stashes: none / moved aside (what)>
review_record: <branch> @ <commit> : <path> (sha256 <digest>)
candidate_reviewed: 77a3d0e65072b60f4ce1780a709dd342fdda67f7
docs_commits_reviewed: <shas> (docs-only: yes/no)
scope_exact: <the fifteen files and nothing else: yes/no>
verdict: M2-006 stage B1 revision 2: <APPROVE/CHANGES_REQUESTED>
reviewer_controls_at_candidate: <x/14 debug; x/14 release>
r01_refused_report_denies: <PASS/FAIL> (<..>)
r02_episodes_across_restarts: <PASS/FAIL> (<..>)
owner_clarifications: <recorded as decided: yes/no>
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
