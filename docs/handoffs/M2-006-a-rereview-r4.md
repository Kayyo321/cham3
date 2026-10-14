# Handoff: M2-006 stage A revision 4 — request for independent re-review, and one wording confirmation

- **State:** M2-006 is **IN_PROGRESS**; stage A is **IMPLEMENTED → IN_REVIEW** (revision 4). Stages B–D are not started (stage-B reading began in a separate local clone and is not on the branch). The author approves nothing.
- **Role:** IMPLEMENTER. **Model/provider:** Claude Code (Anthropic), the owner's home computer. Requested reviewer: Codex (independent, on the owner's home computer).
- **Review answered:** `docs/reviews/M2-006-stage-a-r3-codex-security.md` at f0d6eb01aeec7858fe67a034f42db09ca04f0177 on `review/m2-006-a-r3-security` (one commit on `main` 9cf8825, the branch adds only that file, SHA-256 `a04041838560744814bd2c9f078155497032a8009b693cefc2b5c95ad1e60c92`; verified, not merged or edited). Author response: [M2-006 stage A finding resolutions r4](../reviews/M2-006-a-finding-resolutions-r4.md).
- **Owner decisions used:** R2-03 Option A and the amd64 lock refresh (both 2026-10-14, recorded in the [revision-3 handoff](M2-006-a-rereview-r3.md)). **Requested:** the owner's confirmation of the corrected E5 wording (below); the correction changes no mechanism.
- **Base:** 908c3911fd08ccf91d4ba6706fbad49d45eaf1a8 (the revision-3 docs-only commit on 2217e62).
- **Revision-4 candidate:** **db5bb59fcf461034461d9e0d2f21f5816986134c** (one commit on the base: range 908c391..db5bb59, 8 files).
- **Docs-only follow-up:** the commit adding this handoff, the resolutions, the evidence, the task progress and STATUS.

## Owner confirmation: the corrected E5 wording (R3-02)

The reviewer found that E5's first wording made every successful retry end in containment. A retry in which only the restriction's journal record becomes durable instead keeps the restriction precisely through the normal roll-forward and replay, without containing anyone; that behaviour is safe and is kept. The corrected text in ADR 0006 §3:

- the guarantee holds when one durable mark is made before the process dies: the restriction's ledger entry; its own record (the journal record of a per-user restriction, or the anchor image of a policy restriction, which has no per-user record); or, after a ledger failure, the invalidation of the ledger's validity file or the `unverifiable` marker;
- outside it is only the case in which every mark fails at once, the process dies before any retry succeeds, and registryd restarts in the same boot;
- a successful invalidation or marker makes the next same-boot start **contain**; a journal record that becomes durable while the ledger and the anchor still fail is **rolled forward and replayed**, the restriction stays in force and the user is not contained by that alone; a policy restriction always ends in containment; a reboot contains.

Please confirm this wording, or say what to change.

## Completed work

| Finding | Change | Where |
|---|---|---|
| R3-01 | Every IPC operation retries the owed invalidation and marker right after its role check, before any early return (`append_identity`, `journal_users`, `admit`, `set_policy_epoch`, `set_emergency_restriction`, `policy_state`, `unusable_users` added; `rollback_status` moved after its role check); a refused caller writes nothing. From the first ledger failure the anchor holds the run unverifiable: every later anchor image carries `unverifiable`, whichever write it belongs to; a published marker is not rewritten. The start's explicit marker choice is that same flag (redundant branch removed) | `anchor.rs`, `registry.rs` |
| R3-02 | E5 and storage formats §§4, 7 distinguish containment (invalidation or marker succeeded) from precise replay (journal-only), and the policy restrictions | ADR 0006 §3, storage formats, IPC `rollback_status` row |

**Complete change list of 908c391..db5bb59 (`git diff --stat`, 8 files):** `core/registryd/src/anchor.rs`, `core/registryd/src/registry.rs`, `core/registryd/tests/storage_integrity.rs`, `docs/decisions/0006_m2_storage_anchor_recovery.md`, `docs/specs/m2_registry_storage_v1.md`, `docs/specs/m2_service_ipc_v1.md`, `tests/m2/harness/test_m2_registry_policy.py`, `tests/m2/test_inventory.txt`. No dependency, `Cargo.lock`, manifest, pinned version, lock file, oracle, vector, workflow or Dockerfile changed; `ledger.rs`, `journal.rs`, `record.rs`, `lineage.rs` untouched.

New controls: `storage_integrity` 30 tests (+3; the fault injector gains a count of failing anchor writes); 97 seeded registry/policy defects (+10, one re-pointed); inventory 209 cargo tests (+3).

## Verification (at db5bb59fcf461034461d9e0d2f21f5816986134c)

| Check | Environment / command | Result | Evidence / limits |
|---|---|---|---|
| Reviewer's R3 test source, unchanged, at 2217e62 and at the candidate | pinned container, UID 1001, clean clones; `cargo test --frozen -p cham3_registryd --test review_r3_validation review_r3_ -- --test-threads=1` (debug and release) | **2217e62: 7/12 (the five R3-01 tests fail); candidate: 12/12** | `reviewer_reproductions/` |
| Reviewer's R2 test sources, unchanged, at the candidate | `--test review_r2_storage review_r2_`, `--test queue_validation queue_`, `--test reviewer_matrix review_matrix` | **storage 5/6** (only the case erratum E5 excludes), **queue 3/4** (only the old premise assertion), **matrix 4/4**; unchanged from 2217e62 | `reviewer_reproductions/` |
| Stage-A revision-4 tests | `storage_integrity` (30): the three new tests; `registry_operations` (25), `current_authorization` (14), `anchor::tests` (3) unchanged | PASS (debug and release) | in the qualification |
| Seeded defects | `test_m2_registry_policy.py`: 97 defects (10 new, one re-pointed), each caught by the named test, the unmodified copy passes | **PASS** (`test_m2_registry_policy` 2 tests over 97 defects, in the 147-test harness run, 388.2 s) | in the qualification's harness run |
| Full M2 qualification | Clean clone owned by UID 1001, pinned container `cham3-m2-dev` (image id 2a0bcf342c57, the no-cache rebuild with the refreshed locks), `--ulimit nofile=1024:1024`, `qualify_m2_rust.sh`, `CHAM3_M2_REQUIRE_CLEAN=1` | **PASS** (`m2 qualification: PASS`; both lock checks PASS; unit 209 + 209; provider PASS; ECR 19/96/18 with 23 scenarios / 352 steps; format differential 5955 / 0, corpus unchanged; vault oracle 38; harness 147; admission differential 11 scenarios / 59 steps / 105 comparisons / 0 disagreements; Valgrind memcheck 0 errors; Miri, sanitizers, fuzzing NOT_AVAILABLE) | [VM evidence](../evidence/M2-cryptographic-contract-core/m2_006a_r4_amd64_vm_db5bb59/) |
| Static checks; static tests | same clone and container | **PASS** (`static checks: PASS`; `Ran 82 tests … OK`; the clone clean afterwards) | `qualification_console.txt` |
| Full-filesystem probe with the ledger, then the five wrapper cases | `--tmpfs /full-disk:rw,size=2m,…` | **PASS** (`restrictive reserve under a full filesystem: PASS`, both reports byte-identical to 2217e62's and to CI's; wrapper cases 5/5 OK) | `storage/` |
| Unchanged inputs | `git diff --stat 908c391 db5bb59` | the eight files; nothing else | `unchanged_inputs.txt` |
| GitHub Actions at db5bb59 | m2_checks 38592632636, m1_checks 38592632234, m0_checks 38592632553 | **success**, every job and step (harness 147 in 1024.8 s; protection, full-filesystem and wrapper steps PASS); artifact 11910945081, `sha256:f9d7bb8444648e0ffe486b7e9fb3f214a0e21841b13d4b9988ef7e16d6bd6671`, download digest identical | [CI evidence](../evidence/M2-cryptographic-contract-core/m2_006a_r4_amd64_ci_db5bb59/) |
| Vault protection, positive path | CI step | **PASS** in CI (`protection_verified`, `vault protection positive path: PASS`); NOT_RUN on this host (memlock limit) | CI evidence |
| Real power loss, kernel crash, process death at each boundary (A08/A10) | — | NOT_RUN: faults are injected at the io boundaries and restarts/reboots simulated | storage spec §7 |
| Miri, sanitizers, coverage-guided fuzzing | — | NOT_RUN (NOT_AVAILABLE on the pinned toolchain) | — |

## Review

- Reviewer and exact commit: Codex, `review/m2-006-a-r3-security` @ f0d6eb0 (CHANGES_REQUESTED, R3-01, R3-02); earlier `review/m2-006-a-r2-security` @ 7d570c1 and `review/m2-006-a-security` @ 344c372.
- Findings and disposition: R3-01 resolved; R3-02 wording corrected, owner confirmation requested. Details: [resolutions r4](../reviews/M2-006-a-finding-resolutions-r4.md).

## Remaining

- **Known risks and limits:** as in the revision-3 handoff; the E5 window is the owner-accepted limit (wording pending the owner's confirmation). The arm64 development locks still record libcurl deb13u5.
- **Uncommitted state:** none after the docs commit. The stage-B reading clone (`/home/dev/cham3-m2006b/work`) is not on the branch.
- **Next:** the owner confirms the E5 wording; Codex re-reviews revision 4 (bounded, prompt below); an APPROVE closes stage A (completion handoff) and opens stage B.

## Review prompt for Codex

~~~text
You are the independent REVIEWER (Codex) for cham3 M2-006 stage A revision 4 in Kayyo321/cham3: a bounded
re-review of your two findings (M2-006-A-R3-01, R3-02). You did not author this work. You run on the owner's
home computer; Docker worked for you last time (docker ps, sg docker); record BLOCKED only with the exact command
and error.

0. Bring the local clone to the candidate FIRST:
     cd ~/cham3 && git status --porcelain && git stash list
     git fetch origin codex/m2-cryptographic-contract-core review/m2-006-a-r3-security main
     git checkout codex/m2-cryptographic-contract-core && git reset --hard origin/codex/m2-cryptographic-contract-core
     git rev-parse HEAD        # expect the docs-only commit after db5bb59 (its parent must be db5bb59)
     git log --oneline -4      # 908c391 (r3 docs) -> db5bb59 (revision 4) -> the docs-only commit
   Move unexpected local changes aside (git stash); never force-push or edit the implementation branch. Read every
   document from this checkout; make the clean clone for testing from it
   (git clone --no-hardlinks ~/cham3 <dir>; git -C <dir> checkout --detach db5bb59fcf461034461d9e0d2f21f5816986134c).
   Your no-cache image from the last review (cham3-m2-independent-r3-final) has the refreshed locks.

Read AGENTS.md; your record review/m2-006-a-r3-security at f0d6eb01aeec7858fe67a034f42db09ca04f0177;
docs/reviews/M2-006-a-finding-resolutions-r4.md; docs/handoffs/M2-006-a-rereview-r4.md; ADR 0006 §3 erratum E5
(corrected wording); docs/specs/m2_registry_storage_v1.md §§4, 7 and the rollback_status row of
docs/specs/m2_service_ipc_v1.md. Treat author claims as claims to verify.

Targets: candidate db5bb59fcf461034461d9e0d2f21f5816986134c (range 908c391..db5bb59, one commit, eight files:
core/registryd/src/{anchor,registry}.rs, core/registryd/tests/storage_integrity.rs,
docs/decisions/0006_m2_storage_anchor_recovery.md, docs/specs/m2_registry_storage_v1.md,
docs/specs/m2_service_ipc_v1.md, tests/m2/harness/test_m2_registry_policy.py, tests/m2/test_inventory.txt) and the
docs-only commit after it (docs/ only). Confirm nothing else changed. Do not merge main or review branches; do not
edit implementation files.

1. Your tests, unchanged: review_r3_validation (artifacts/review_r3_validation.rs, sha256 5331ddd1…9ec9b) at the
   candidate, debug and release: expect 12/12. Your R2 sources (review_r2_storage, queue_validation,
   reviewer_matrix): expect storage 5/6 (the E5 case), queue 3/4 (the old premise), matrix 4/4. Compare with the
   author's reviewer_reproductions/ logs.
2. R3-01: every IPC entry point of registry_t retries right after its role check and before any early return
   (audit all of them, including rollback_status's moved hook); a refused caller writes nothing; the inspection
   accessors write nothing and are not IPC operations; lock order (retry with neither ledger nor anchor held).
   The sticky flag in anchor_t: every anchor image after a ledger failure carries `unverifiable` (slot anchors,
   policy changes, the emergency release, the start's marker write); a published marker is not rewritten; the
   removed start branch is fully replaced by the flag. Try further operation orders and partial recoveries.
3. R3-02: the corrected E5 text and storage formats §§4, 7 against the behaviour: containment after a successful
   invalidation or marker; precise roll-forward and replay after a journal-only retry; policy restrictions always
   contained; the excluded case unchanged. Comment on the wording for the owner; make no owner decision.
4. Harness: 97 seeded defects (10 new, one re-pointed: start_marker_ignores_failed_ledger now removes the sticky
   flag's setter); judge whether each new defect's test discriminates for the right reason. Inventory 209.
5. Evidence: qualification in a clean clone at db5bb59 (unit 209+209, harness 147 with 97 registry/policy
   defects, both lock checks PASS, the rest unchanged); static checks and static tests; the full-filesystem
   wrapper and the five wrapper cases. Compare GitHub Actions m2_checks 38592632636, m1_checks 38592632234, m0_checks 38592632553; download the artifact and check its
   digest (artifact 11910945081, sha256:f9d7bb8444648e0ffe486b7e9fb3f214a0e21841b13d4b9988ef7e16d6bd6671).
6. Codex Security plugin on 908c391..db5bb59; record name/version and findings.

Record every check as PASS/FAIL/NOT_RUN/BLOCKED with exact commands and evidence. Return APPROVE or
CHANGES_REQUESTED for M2-006 stage A revision 4 and make no owner decisions (list questions for the owner
separately). Publish only your record on a new review branch (review/m2-006-a-r4-security) and end with:

=== CHAM3 M2-006 STAGE A R4 VERDICT ===
reviewer: Codex
environment: <OS/arch; UID; Docker access: yes/no and how; image id; nofile limit used>
local_clone: <~/cham3 HEAD after step 0; local changes or stashes: none / moved aside (what)>
review_record: <branch> @ <commit> : <path> (sha256 <digest>)
candidate_reviewed: db5bb59fcf461034461d9e0d2f21f5816986134c
docs_commits_reviewed: <shas> (docs-only: yes/no)
scope_exact: <the eight files and nothing else: yes/no>
verdict: M2-006 stage A revision 4: <APPROVE/CHANGES_REQUESTED>
reviewer_tests_at_candidate: <r3 x/12; storage x/6; queue x/4; matrix x/4; debug and release>
r3_01_retry_coverage: <PASS/FAIL> (<entry points audited; sticky anchor; role checks; lock order>)
r3_02_e5_wording: <PASS/FAIL> (<containment vs precise replay; policy restrictions>)
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
