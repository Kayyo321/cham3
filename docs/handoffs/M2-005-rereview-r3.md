# Handoff: M2-005 revision 3 — request for independent re-review

- **State:** M2-005 is **IN_REVIEW** (revision 3). M2-004 is **DONE** ([completion handoff](M2-004-completion.md)). The author approves nothing.
- **Role:** IMPLEMENTER. **Model/provider:** Claude Code (Anthropic), cloud VM. Requested reviewer: Codex (independent, on the owner's home computer).
- **Branch:** `codex/m2-cryptographic-contract-core` (pushed fast-forward).
- **Review answered:** `docs/reviews/M2-005-codex-security-rereview-r2.md` at d300b609ccde3eaf1fe8e909f99e78707c4b6c5a on `review/m2-005-r2-security` (parent 9cf8825, adds only that file, SHA-256 `28b4623121c65093d1fdc98963262b30e68da251417a868d6cea6c0f82b17435`; verified, not merged or edited). Author response: [M2-005 finding resolutions r3](../reviews/M2-005-finding-resolutions-r3.md).
- **Owner decisions used:** none were needed. **Owner questions still open:** the error code for the ±300 s commit-timestamp admission check (M2-006). The question which M2-004 revision-2 verdict governs is closed by the reviewer's reconciliation (completion handoff).
- **Base:** 959524cf52212d9db260457b1c458fe6f95b59f7 (the last revision-2 docs commit on candidate 051b4c8).
- **Revision-3 candidate:** **be2415291bf5bcf9dad8df323bf971bd193d5120** (one commit on the base: range 959524c..be24152).
- **Docs-only follow-up:** the commit adding this handoff, the resolutions, the M2-004 completion handoff, the evidence, the task progress and STATUS.

## Completed work

| Finding | Change | Where |
|---|---|---|
| R2-01 | A directory entry's existence is never taken for a completed sync: `root` in its parent and `users` in `root` are synced at every start; the user directory is synced at every reopen (the journal's or a recreated reserve's creation sync may have failed before a restart); `users` stays synced on every creation attempt. A sync that fails at start refuses the store or makes the user unusable | `registryd/src/journal.rs` (`open`, `open_journal`) |
| R2-02 | The reserve's durable size is tracked separately from its length: creation allocates (syncs) on every attempt, also for a full-length reserve; replenishment continues from the file's actual length and counts the reserve as full only after an allocation including its sync succeeded; a full-length reserve found at start is synced before it counts as full, else it is pending (restrictive records unaffected; the next ordinary append repeats the sync) | `registryd/src/journal.rs` (`create_journal`, `reopened_reserve_bytes`, `replenish_reserve`) |
| R2-03 | The probe owns only what it creates: the mode and, in full-disk mode, the parent's filesystem are checked before anything is created; the root and the filler are created exclusively and an existing path is refused untouched (exit 3); only a root this invocation created is removed. The full-filesystem script asserts no leftovers | `registryd/src/bin/cham3_reserve_probe.rs`, `tests/m2/storage/check_reserve_full_disk.sh` |
| Scope-list precision | This handoff's change list is the candidate's `git diff --stat`, complete by construction | below |

Corrections to the revision-2 handoff (which stays as the record of what was submitted):

- "Reopening an existing store creates nothing and syncs nothing" is now "reopening repeats the syncs the acknowledged state depends on: the parent of `root`, `root`, and every replayed user's directory".
- "Bytes currently allocated to the reserve" (`reserve_bytes`) is now "bytes of the reserve known to be durable"; after a restart it is 0 until the start-time sync (or the next ordinary append) succeeds.
- The revision-2 scope list omitted `core/contract_format/tests/ecr_vectors.rs` (signed-object count 129 → 134) and `tests/m2/vectors/ecr_v1/vectors_readme.md`; both were part of candidate 051b4c8.

**Complete change list of 959524c..be24152 (`git diff --stat`, 7 files, +745/−109):** `core/registryd/src/bin/cham3_reserve_probe.rs`, `core/registryd/src/journal.rs`, `core/registryd/tests/registry_operations.rs`, `tests/m2/harness/test_m2_registry_policy.py`, `tests/m2/harness/test_m2_reserve_probe.py`, `tests/m2/storage/check_reserve_full_disk.sh`, `tests/m2/test_inventory.txt`. Nothing else changed: no dependency, `Cargo.lock`, manifest, build input, workflow, specification, ADR, oracle, vector or other crate.

New controls:

- **Rust tests** (169 per profile, 3 new, all in registryd `registry_operations`): `a_failed_startup_directory_sync_is_repeated_at_the_next_start`, `a_failed_creation_sync_is_repeated_when_the_journal_is_reopened`, `a_reserve_written_without_a_completed_sync_is_never_trusted_for_its_length`; `every_created_directory_entry_is_durable_before_the_enrollment_is_acknowledged`, `a_failed_parent_directory_sync_is_repeated_before_a_retry_is_acknowledged` and `the_preallocated_reserve_is_used_only_by_restrictive_records` now assert the exact start-time syncs. Fixtures: `sync_spy_t` fails the next sync of any one path; `unsynced_allocation_io_t` writes every byte and fails the sync.
- **Seeded defects:** 41 registry/policy (5 new: `store_open_trusts_existing_directories`, `reopened_user_directory_not_synced`, `existing_reserve_length_trusted_at_creation`, `replenishment_trusts_the_file_length`, `reopened_reserve_trusted_by_length`; two re-anchored) and 2 probe defects (`probe_adopts_an_existing_root`, `probe_removes_a_root_it_did_not_create`, built from a disposable workspace copy and caught by the preservation check).
- **Harness:** `test_m2_reserve_probe.py` 7 tests (an existing root with a synthetic sentinel refused and preserved in all three modes; full-disk mode on an ordinary filesystem refuses before creating anything; an unknown mode and a missing parent create nothing; allocate-only; the seeded probe defects and their control). 142 harness tests.
- **Full filesystem:** `check_reserve_full_disk.sh` additionally asserts that neither the root nor the filler is left on the tmpfs.

## Verification (at be2415291bf5bcf9dad8df323bf971bd193d5120)

| Check | Environment / command | Result | Evidence / limits |
|---|---|---|---|
| Full M2 qualification | Clean UID-1000 clone `/tmp/cham3_m2005r3`, pinned container, `qualify_m2_rust.sh`, `CHAM3_M2_REQUIRE_CLEAN=1`; no `--privileged`, no added capabilities, no `--record` | **PASS**: unit **169 + 169** cargo tests (inventory-exact); provider oracle 7/7; ECR oracle 19/96/18 with 23 scenarios / 352 steps; format differential 5 955 cases, 0 mismatches (corpus unchanged); vault_object_oracle 38/38; harness **142** (41 seeded registry/policy and 21 identity/vault defects detected; 7 reserve-probe tests with 2 seeded probe defects) | [m2_005_amd64_vm_be24152](../evidence/M2-cryptographic-contract-core/m2_005_amd64_vm_be24152/) |
| Layer-7 differential | `admission_scenarios -- --nocapture` | PASS: 11 scenarios, 59 steps, 105 comparisons, 0 disagreements; 18/18 admission-invalid vectors | `admission_scenarios_nocapture.log` |
| Static checks; static tests | same clone and container | PASS; PASS (82); the clone was clean afterwards | `qualification_console.txt` |
| Restrictive reserve under a full filesystem | pinned container, `--tmpfs /full-disk:rw,size=2m,uid=1000,gid=1000,mode=0700`, `check_reserve_full_disk.sh` (with the leftover check), release and debug | **PASS** in the author's VM and in CI (restrictive append seq 35, refusal while depleted, second restrictive 36, refilled 37, chained; no leftovers) | `storage/` |
| Reviewer's ten new tests (`edge_`, unchanged, appended to the reconstructed first-review package) | clean clones at 051b4c8 and be24152, debug and release | **051b4c8: 4 passed, 6 failed** (the record's six); **be24152: 10 passed, 0 failed** in each profile | `reviewer_reproductions/` |
| Reviewer's probe matrix (existing root with sentinel; unknown mode; overlay; 32 MiB tmpfs), unchanged commands | same | **051b4c8: `sentinel=deleted`** both profiles; **be24152: `sentinel=preserved`** in both profiles; the unknown mode, the overlay refusal and the 32 MiB tmpfs refusal leave no root behind | same |
| Reviewer's original nine M2-005 tests and `filesystem_full_reserve` (unchanged) | be24152, debug and release (tmpfs for the last) | PASS: 9/0 and 1/0 in each profile | same |
| Unchanged inputs | `git diff --stat 959524c be24152` | the 7 files above; `Cargo.lock`, vectors, oracle, specifications, ADRs, every other crate unchanged | `unchanged_inputs.txt` |
| Vault protection, positive path | CI step with `--ulimit memlock=1073741824:1073741824` | **PASS in CI** (m2_checks 38435225455: derivation 421 ms, `protection_verified`); BLOCKED in this VM (memlock cannot be raised) | [CI evidence](../evidence/M2-cryptographic-contract-core/m2_005_amd64_ci_be24152/) |
| GitHub Actions at be24152 | m2 38435225455, m1 38435225444, m0 38435225490 | **PASS**, every job and step, including the protection and full-filesystem steps; artifact 11834225727, `sha256:4168e9190cdef6e6fff0187e7a76099beded24aac32902d34d032f6dc984eef6` (not downloaded by the author) | CI evidence |
| Miri, sanitizers, coverage-guided fuzzing | — | NOT_RUN (NOT_AVAILABLE on the pinned toolchain) | Bounded seeded and differential tests only |
| Anchor, ledger, crash recovery, lifecycle writers, trusted time, IPC, processes | — | NOT_RUN | M2-006, M2-007 |

## Remaining

- **Known risks and limits:** as in revision 2 (in-process composition; no anchor or restriction ledger; lazy reserve replenishment without a status field until M2-006; compressing filesystems; start-time configuration of policy; system clock; Miri, sanitizers and fuzzing NOT_RUN). New in revision 3: three directory fsyncs per start plus one per user, and one reserve fsync per user with a full reserve — bounded, and the price of not inferring durability from existence.
- **Uncommitted state:** none after the evidence commit.
- **Next:**
  1. Codex re-reviews M2-005 revision 3 (prompt below, one verdict block).
  2. M2-006 starts with its docs-only scope record under the owner's standing instruction; the ±300 s commit-timestamp error code is still an open owner question.
- **Reviewer environment (owner note):** Codex runs on the **owner's home computer**, unrestricted. The owner's local clone `~/cham3` (lowercase) is refreshed by the prompt's step 0 before anything is read or run. The revision-2 re-review needed a container-only `--ulimit nofile=1024:1024` for Valgrind there (a descriptor limit of 1 073 741 804 broke Valgrind's private-file creation); that is an environment setting, not a source change, and the prompt allows it again provided it is recorded.
- **Reminder for the end of M2:** the final review uses the Codex Security plugin skill (it ran for the first time in the revision-2 re-review).

## Review prompt for Codex

~~~text
You are the independent REVIEWER (Codex) for cham3 M2-005 revision 3 in Kayyo321/cham3. You did not author this
work. You run on the owner's home computer with unrestricted access (your own Docker, network, gh and the Codex
Security plugin): nothing is expected to be BLOCKED; record BLOCKED only with the exact command and error.

0. Bring the local clone to the candidate FIRST. The owner's ~/cham3 checkout is at the previous candidate and may
   be on another branch; do not read or test it as it stands. Run:
     cd ~/cham3 && git status --porcelain && git stash list
     git fetch origin codex/m2-cryptographic-contract-core review/m2-005-r2-security review/m2-005-security main
     git checkout codex/m2-cryptographic-contract-core && git reset --hard origin/codex/m2-cryptographic-contract-core
     git rev-parse HEAD        # the docs-only commit after be2415291bf5bcf9dad8df323bf971bd193d5120 (its parent must be be24152)
     git log --oneline -3      # 959524c -> be24152 (candidate) -> <docs-only commit>
   If `git status` showed local changes or stashes you did not expect, record them and move them aside (git stash)
   rather than discarding them; never force-push or edit the implementation branch. Read every document below from
   this checkout at that HEAD, and make every clean clone for testing from it
   (git clone --no-hardlinks ~/cham3 <dir>; git -C <dir> checkout --detach be2415291bf5bcf9dad8df323bf971bd193d5120).

Read AGENTS.md; your re-review record (review/m2-005-r2-security at d300b609ccde3eaf1fe8e909f99e78707c4b6c5a,
docs/reviews/M2-005-codex-security-rereview-r2.md); docs/reviews/M2-005-finding-resolutions-r3.md;
docs/handoffs/M2-005-rereview-r3.md; docs/handoffs/M2-004-completion.md (records your M2-004 APPROVE; confirm it
states your verdict and its scope correctly); ADR 0006 section 1 and ADR 0008 section 1. Treat author claims and
evidence as claims to verify; judge against the approved ADRs and specifications.

Targets: candidate be2415291bf5bcf9dad8df323bf971bd193d5120 (range 959524c..be24152, one commit) and the docs-only
commit after it on origin/codex/m2-cryptographic-contract-core; confirm it changes only docs/. Confirm the candidate
changes exactly the seven files the handoff lists (git diff --stat 959524c be24152) and nothing else: no dependency,
Cargo.lock, manifest, workflow, specification, ADR, oracle or vector change; registry.rs, lineage.rs, policyd,
vaultd, identityd, contractd and contract_format unchanged. Do not merge main or review branches into the
implementation branch; do not edit implementation files.

1. R2-01 (core/registryd/src/journal.rs open, open_journal, create_journal; the module documentation): root in its
   parent and users in root synced at every start; the user directory synced at every reopen; users synced on every
   creation attempt; a sync that fails at start refuses the store or makes the user unusable. Re-run your four
   failing directory tests and your two-user control (it must still see exactly two users syncs after a restart).
   Judge the author's argument that the <uid> entry in users needs no start-time sync (a journal exists only after
   the users sync of the same create_journal call returned success) and the cost (three fsyncs per start, one per
   user). Try: a sync failing at the second start too; a user directory whose sync fails at start while other users
   are usable; a reserve recreated at start with a failing sync followed by two restarts.
2. R2-02 (journal.rs create_journal, reopened_reserve_bytes, journal_file_t::replenish_reserve, reserve_bytes): the
   durable size is tracked separately from the length; creation allocates (syncs) on every attempt; replenishment
   continues from the actual length and counts the reserve as full only after a successful allocation including its
   sync; a full-length reserve found at start is synced before it counts as full, else pending. Re-run your two
   reserve-sync tests and your partial-reserve control. Judge the choice to keep the user usable (restrictive
   records recordable, ordinary appends refused) when the start-time reserve sync fails, against ADR 0008 section 1.
   Try: a sync that fails at start and again at the next append, then succeeds; a reserve longer than 1 MiB; an
   allocation returning Ok without writing (a hostile journal_io_t); release followed by a failed replenishment
   across a restart.
3. R2-03 (core/registryd/src/bin/cham3_reserve_probe.rs create_root, fill_target, exercise, run, main, the
   filler; tests/m2/storage/check_reserve_full_disk.sh; tests/m2/harness/test_m2_reserve_probe.py): mode and
   filesystem decided before anything is created; root and filler created exclusively; an existing path refused
   untouched; only a created root removed. Re-run your probe matrix (existing root with a sentinel in full-disk
   mode on overlay; the 32 MiB tmpfs) and extend it: an existing root in allocate-only mode and with an unknown mode,
   an existing filler file on the tmpfs, a root whose parent is missing, a root path that is a symbolic link, a
   failure after creation (the root must still be removed). Run tests/m2/storage/check_reserve_full_disk.sh in the
   pinned container with --tmpfs /full-disk:rw,size=2m,uid=$(id -u),gid=$(id -g),mode=0700 and confirm the leftover
   check. Judge the two seeded probe defects and whether the harness tests discriminate.
4. Confirm every earlier PASS item of your record is intact (R02, the error vocabulary, the normal and full-disk
   reserve paths, the layer-7 and format differentials, the M2-004 items) and re-run your original nine tests and
   filesystem_full_reserve (2 MiB tmpfs) at the candidate.
5. Evidence: in a clean clone at be2415291bf5bcf9dad8df323bf971bd193d5120 owned by an unprivileged UID (cloned from
   the fetched ~/cham3 of step 0), build the pinned image and run the qualification
   (docker run --user "$(id -u):$(id -g)" ... bash build/scripts/qualify_m2_rust.sh with CHAM3_M2_REQUIRE_CLEAN=1;
   no --privileged, no added capabilities, no --record; the container-only --ulimit nofile=1024:1024 you needed
   for Valgrind is acceptable if recorded) and confirm: unit 169+169 cargo tests, provider oracle 7/7, ECR oracle
   19/96/18 with 23 scenarios / 352 steps, format_differential 5955 cases / 0 mismatches (corpus sha256
   0ad8bc2b1e8d39224de699db64aa45ca7adcf70117c555f35b57c537471d395e, unchanged), vault_object_oracle 38 checks
   PASS, harness 142 (including 21 identity/vault and 41 registry/policy seeded defects and the reserve probe's
   seven tests with two seeded probe defects); then build/scripts/run_static_checks.sh and
   sh tests/run_tests.sh static. Run the protection positive path (--ulimit memlock=1073741824:1073741824) and the
   full-filesystem check (--tmpfs as above).
6. Compare GitHub Actions m2_checks 38435225455, m1_checks 38435225444 and m0_checks 38435225490 for be24152,
   including the protection and full-filesystem steps. Download the artifact (gh run download 38435225455 -n
   m2_qualification_amd64), check its digest (sha256:4168e9190cdef6e6fff0187e7a76099beded24aac32902d34d032f6dc984eef6) and compare its summaries and the two reserve
   reports with your own runs.
7. Run the Codex Security plugin skill on the candidate range (959524c..be24152) and record its exact name/version,
   the commands it ran and every finding; record BLOCKED only with the exact command and error (it remains required
   for M2-008).

Record every check as PASS/FAIL/NOT_RUN/BLOCKED with exact commands, environment and evidence. Return APPROVE or
CHANGES_REQUESTED for M2-005; make no owner decisions. Publish only your review record on a new review branch
(review/m2-005-r3-security), and end your reply with this block filled in:

=== CHAM3 M2-005 R3 VERDICT ===
reviewer: Codex
environment: <home computer: OS/arch; UID; Docker version; network unrestricted yes/no; nofile limit used>
local_clone: <~/cham3 HEAD after step 0; local changes or stashes found: none / moved aside (what)>
review_record: <branch> @ <commit> : <path> (sha256 <digest>)
candidate_reviewed: be2415291bf5bcf9dad8df323bf971bd193d5120
docs_commits_reviewed: <shas> (docs-only: yes/no)
scope_exact: <the seven files and nothing else: yes/no>
verdict: M2-005: <APPROVE/CHANGES_REQUESTED>
prior_findings: R2-01=<PASS/FAIL> R2-02=<PASS/FAIL> R2-03=<PASS/FAIL> R01=<PASS/FAIL> R03=<PASS/FAIL> earlier PASS items intact=<yes/no>
r2_01_uid_entry_argument: <accepted / objection>
r2_02_pending_at_start: <accepted / objection>
reserve: <own full-filesystem run: PASS/FAIL/BLOCKED (counts); leftover check; probe matrix results>
qualification_amd64: <PASS/FAIL/BLOCKED> (<counts>)
protection_positive_path: <PASS/FAIL/BLOCKED> (<own run or CI step>)
full_filesystem_step: <PASS/FAIL/BLOCKED> (<own run or CI step>)
static_checks: <..>   static_tests: <..>
own_tests: <your ten edge_ tests, nine own_ tests and filesystem_full_reserve in debug/release at the candidate; new tests and results>
security_plugin: <ran (name/version, findings) / BLOCKED (exact command and error)>
ci_comparison: <..>
m2_004_completion_handoff: <states the verdict and scope correctly: yes/no>
new_findings:
- id: <..> | severity: <..> | location: <file:line> | summary: <..> | required: <..>
notes_for_owner: <..>
=== END VERDICT ===
~~~
