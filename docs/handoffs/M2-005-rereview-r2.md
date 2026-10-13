# Handoff: M2-005 revision 2 — request for independent re-review (with M2-004 revision 3)

- **State:** M2-005 is **IN_REVIEW** (revision 2). M2-004 is **IN_REVIEW** (revision 3; its two revision-2 records disagree, see the [M2-004 resolutions](../reviews/M2-004-finding-resolutions-r2.md)). The author approves nothing.
- **Role:** IMPLEMENTER. **Model/provider:** Claude Code (Anthropic), cloud VM. Requested reviewer: Codex (independent).
- **Branch:** `codex/m2-cryptographic-contract-core` (pushed fast-forward).
- **Review answered:** `docs/reviews/M2-005-codex-security-review.md` at f9f3b828fcd117ebcaa4179fa6ae05e782381db1 on `review/m2-005-security` (parent 9cf8825, adds only that file, SHA-256 `8b36e400309a8936f7b8ed287edd226905aaa10f117b904240d46d14bbcc0a18`; verified, not merged or edited). Author response: [M2-005 finding resolutions](../reviews/M2-005-finding-resolutions.md). The M2-004 finding of `review/m2-004-r2-security` @ fd6de58 is answered in the same candidate ([M2-004 resolutions r2](../reviews/M2-004-finding-resolutions-r2.md), [M2-004 revision-3 handoff](M2-004-rereview-r3.md)).
- **Owner decisions used:** none were needed for the changes. **Owner decision requested:** which M2-004 revision-2 verdict governs (the earlier CHANGES_REQUESTED or the later APPROVE); until then M2-004 is not DONE.
- **Base:** 24a35c1914d787b936d14b37b500226dde51722f (the revision-1 docs commit after candidate b06d7cb).
- **Revision-2 candidate:** **051b4c8aacdeecfabb23625f93dd16615aec39a0** (range 24a35c1..051b4c8, two commits: the implementation 4f7080b091adef38d3c8b1a36fb975db53ba52b7 with M2-005 R01–R03 and M2-004-R2-01, and 051b4c8 adding the three R03 seeded defects to the harness).
- **Docs-only follow-up:** f575ac3 (this handoff, the two resolutions records, the M2-004 handoff, the evidence, the task progress and STATUS) and one further docs-only commit correcting the candidate description in the VM evidence README.

## Completed work

| Finding | Change | Where |
|---|---|---|
| R01 | Every directory entry the store creates is synced in its parent before acknowledgement, on every attempt: `root` in its parent and `users` in `root` at open; `users` after the user directory exists on every `create_journal` call, then the journal, then the user directory | `registryd/src/journal.rs`, `registry.rs` |
| R02 | The index retains ended session ids; a commit adding a grant bound to an ended session is `duplicate_identifier` at admission and at replay; the view carries the ended sessions of live grants and evaluation denies them before any attestation; the oracle gets the same rule and a published scenario | `registryd/src/lineage.rs`, `registry.rs`, `policyd/src/evaluate.rs`, `tests/m2/oracle/ecr_oracle.py`, `vectors.json` |
| R03 | A per-user preallocated 1 MiB reserve file of written zeros; a restrictive append that meets `ENOSPC`/`EDQUOT` truncates the journal back, releases the reserve and writes again; a non-restrictive append first refills the reserve and is refused while it cannot; the probe `cham3_reserve_probe` shows it on an actually full 2 MiB tmpfs (CI step and author run) | `registryd/src/journal.rs`, `registry.rs`, `src/bin/cham3_reserve_probe.rs`, `tests/m2/storage/check_reserve_full_disk.sh`, `m2_checks.yml`, `run_m2_tests.sh` |
| M2-004-R2-01 | Pending proposal ids count against the retained-id bounds | `vaultd/src/service.rs` |

Corrections to the revision-1 handoff (which stays as the record of what was submitted):

- "creation also syncs the user directory (and `users` when new)" is now "syncs `users` on every creation attempt, then the journal, then the user directory"; opening a store syncs the directories it creates.
- "the 1 MiB reserve is for restrictive records only" was a quota rule; it is now also a preallocated reserve of filesystem blocks, released only by a restrictive append that meets a full filesystem.
- Decision 7 (a session grant needs this session and a live attestation at most 5 s old) is completed by: a durable `session_ended` ends the session itself; no later grant may be bound to it and no attestation revives it.
- Decision 2 ("fail closed on any journal defect") now also applies to a reserve file that fails the path checks; a missing reserve is created at start.

New controls:

- **Rust tests** (166 per profile, 9 new): registryd `registry_operations` +6 (`every_created_directory_entry_is_durable_before_the_enrollment_is_acknowledged`, `a_failed_parent_directory_sync_is_repeated_before_a_retry_is_acknowledged`, `an_ended_session_refuses_every_later_grant_bound_to_it`, `the_preallocated_reserve_is_used_only_by_restrictive_records`, `a_torn_restrictive_write_on_a_full_filesystem_is_truncated_back_and_retried`, `reserve_files_are_checked_like_journals_and_recreated_when_missing`); policyd `current_authorization` +1 (`a_durably_ended_session_denies_every_grant_bound_to_it_whatever_the_attestation`); vaultd `vault_operations` +2 (`pending_proposals_count_against_the_retained_id_bounds`, `the_global_retained_id_bound_counts_pending_proposals_of_every_user`).
- **Published scenario** `session_ended_refuses_later_grants_of_that_session` (oracle and Rust: 11 scenarios, 59 steps, 105 comparisons, 0 disagreements); `admission_scenarios.rs` positive controls updated (11/59, `duplicate_identifier` reached).
- **Seeded defects:** 36 registry/policy (5 new: `parent_directory_not_synced_on_retry`, `new_users_directory_not_synced_in_root`, `ended_sessions_reusable_at_admission`, `ended_sessions_forgotten`, `ended_session_ignored_by_evaluation`, plus `reserve_not_preallocated`, `admission_proceeds_into_a_depleted_reserve`, `restrictive_append_never_releases_the_reserve`) and 21 identity/vault (1 new: `pending_proposals_not_counted_as_retained`).
- **Harness:** `test_m2_reserve_probe.py` (the probe's allocate-only and refusal paths); `test_m2_ecr_oracle.py` requires the new finding id. 137 harness tests.
- **Full filesystem:** `tests/m2/storage/check_reserve_full_disk.sh` on a 2 MiB tmpfs, a new `m2_checks` step with its report in the artifact.
- **Fixtures:** `sync_spy_t` (records and fails directory syncs) and `space_io_t` (`ENOSPC` before or inside a frame, failed allocation; counts truncations, releases, allocations) in `registry_operations.rs`; `journal_io_t` gains `allocate`, `release` and `truncate` with real default implementations.

## Verification (at 051b4c8aacdeecfabb23625f93dd16615aec39a0)

| Check | Environment / command | Result | Evidence / limits |
|---|---|---|---|
| Full M2 qualification | Clean UID-1000 clone, pinned container, `qualify_m2_rust.sh`, `CHAM3_M2_REQUIRE_CLEAN=1`; no `--privileged`, no added capabilities, no `--record` | **PASS**: unit **166 + 166** cargo tests; provider oracle 7/7; ECR oracle 19/96/18 with **23 scenarios / 352 steps**; format differential 5 955 cases (the five new signed scenario objects join the corpus), 0 mismatches; vault_object_oracle 38/38; harness **137** (36 seeded registry/policy and 21 identity/vault defects detected; protection refusal; reserve probe) | [m2_005_amd64_vm_051b4c8](../evidence/M2-cryptographic-contract-core/m2_005_amd64_vm_051b4c8/) |
| Layer-7 differential | `admission_scenarios -- --nocapture` | PASS: 11 scenarios, 59 steps, 105 comparisons, 0 disagreements; 18/18 admission-invalid vectors; the published lineage | `admission_scenarios_nocapture.log` |
| Static checks; static tests | same clone and container | PASS; PASS (82); the clone was clean afterwards | `static_checks.log`, `static_tests.log` |
| Restrictive reserve under a full filesystem | pinned container, `--tmpfs /full-disk:rw,size=2m,uid=1000,gid=1000,mode=0700`, `check_reserve_full_disk.sh`, release and debug | **PASS** in the author's VM and in CI | `reserve_full_disk_release.txt`, `reserve_full_disk_debug.txt` |
| Reviewer's M2-005 tests (nine, unchanged) | clean clones at b06d7cb and 051b4c8aacdeecfabb23625f93dd16615aec39a0, debug and release | **b06d7cb: 5 passed, 4 failed; 051b4c8aacdeecfabb23625f93dd16615aec39a0: 9 passed** | `reviewer_reproductions/` |
| Reviewer's `filesystem_full_reserve` (unchanged, 2 MiB tmpfs) | same | **b06d7cb: FAIL** (`storage_unavailable`); **051b4c8aacdeecfabb23625f93dd16615aec39a0: PASS**, debug and release | same |
| Reviewer's M2-004 tests (twelve and seventeen, unchanged) | clean clones at ca40c62 and 051b4c8aacdeecfabb23625f93dd16615aec39a0 | ca40c62: 10/2; 051b4c8aacdeecfabb23625f93dd16615aec39a0: 12/0; the seventeen: 16/1 (I01) | [m2_004_amd64_vm_051b4c8](../evidence/M2-cryptographic-contract-core/m2_004_amd64_vm_051b4c8/) |
| Unchanged inputs | `git diff --exit-code 24a35c1 051b4c8aacdeecfabb23625f93dd16615aec39a0` per path | PASS: 38 paths unchanged (manifests except `registryd/Cargo.toml`, `.cargo`, build, PROJECT, specifications, ADRs, every valid/invalid/admission vector byte-identical, every oracle but `ecr_oracle.py`, the harness library, identityd, contract_format, contractd, the native crates, probes, vaultd except `service.rs`); `Cargo.lock` unchanged | `unchanged_inputs.txt` |
| Vault protection, positive path | CI step with `--ulimit memlock=1073741824:1073741824` | **PASS in CI** (m2_checks 38422383429); BLOCKED in this VM (memlock cannot be raised) | [CI evidence](../evidence/M2-cryptographic-contract-core/m2_005_amd64_ci_051b4c8/) |
| GitHub Actions at 051b4c8aacdeecfabb23625f93dd16615aec39a0 | m2 38422383429, m1 38422383455, m0 38422383430 | PASS, every job and step, including the full-filesystem step; artifact `sha256:f05dedc6e999261dddc44ffcf780a97ae7b0fa1ea61b26ddd57ab41fe383a926` | CI evidence; the ZIP was not downloaded by the author (unauthenticated `gh`) |
| Miri, sanitizers, coverage-guided fuzzing | — | NOT_RUN (NOT_AVAILABLE on the pinned toolchain) | Bounded seeded and differential tests only |
| Anchor, ledger, crash recovery, lifecycle writers, trusted time, IPC, processes | — | NOT_RUN | M2-006, M2-007 |

## Remaining

- **Known risks and limits:**
  - in-process composition; the concurrency test uses threads in one process;
  - no anchor or restriction ledger: a restored or rolled-back store is not detected, and a torn tail or failed append denies the user until M2-006's recovery exists;
  - reserve replenishment is lazy (the next non-restrictive append) and not yet reported by a status operation; filesystems that compress or deduplicate zeros do not keep the reserve's blocks;
  - the policy epochs and the mandatory policy are start-time configuration; registry time is the system clock;
  - built on M2-004 revision 3, whose revision-2 records disagree;
  - Miri, sanitizers and fuzzing are NOT_RUN.
- **Uncommitted state:** none after the evidence commit.
- **Next:**
  1. Codex re-reviews M2-005 revision 2 and M2-004 revision 3 (one prompt below, two verdict blocks) and states how the two M2-004 revision-2 records relate.
  2. The owner decides which M2-004 revision-2 verdict governs.
  3. M2-006 continues under the owner's standing instruction after the scope record; the ±300 s commit-timestamp error code is still an open owner question.
- **Reviewer environment (owner note, 2026-10-13):** this re-review runs on the **owner's home computer**, where Codex is unrestricted (its own Docker, network, `gh`, the Codex Security plugin). The owner's local clone `~/cham3` is probably outdated and may be on another branch: the prompt's first step brings it to the exact candidate (fetch, check out `codex/m2-cryptographic-contract-core`, verify the SHA) before anything is read or run, and every clean-clone step clones from the fetched repository, not from a stale checkout. The combined review had still recorded `gh run download` as 403 and the Codex Security plugin's preflight resources as unreadable in the cloud environment; the prompt asks for both again on the home computer and for BLOCKED to be recorded only with the exact command and error.
- **Reminder for the end of M2:** the final review uses the Codex Security plugin skill.

## Review prompt for Codex

~~~text
You are the independent REVIEWER (Codex) for cham3 M2-005 revision 2 and M2-004 revision 3 in Kayyo321/cham3. You did
not author this work. You run on the owner's home computer with unrestricted access (your own Docker, network, gh and
the Codex Security plugin): nothing is expected to be BLOCKED; record BLOCKED only with the exact command and error.

0. Bring the local clone to the candidate FIRST. The owner's ~/cham3 checkout is probably outdated and may be on
   another branch; do not read or test it as it stands. Run:
     cd ~/cham3 && git status --porcelain && git stash list
     git fetch origin codex/m2-cryptographic-contract-core review/m2-005-security review/m2-004-r2-security \
       review/m2-004-security main
     git checkout codex/m2-cryptographic-contract-core && git reset --hard origin/codex/m2-cryptographic-contract-core
     git rev-parse HEAD        # the last docs-only commit after 051b4c8aacdeecfabb23625f93dd16615aec39a0
     git log --oneline -5      # 24a35c1 -> 4f7080b -> 051b4c8 -> f575ac3 (docs) -> <docs fix>
   If `git status` showed local changes or stashes you did not expect, record them and move them aside (git stash)
   rather than discarding them; never force-push or edit the implementation branch. Read every document below from
   this checkout at that HEAD, and make every clean clone for testing from it
   (git clone --no-hardlinks ~/cham3 <dir>; git -C <dir> checkout --detach 051b4c8aacdeecfabb23625f93dd16615aec39a0).

Read AGENTS.md; your combined review (review/m2-005-security at
f9f3b828fcd117ebcaa4179fa6ae05e782381db1, docs/reviews/M2-005-codex-security-review.md) and the earlier M2-004
revision-2 record (review/m2-004-r2-security at fd6de588bbee575d936fde448e823bff08f44e7a,
docs/reviews/M2-004-codex-security-rereview-r2.md, CHANGES_REQUESTED with M2-004-R2-01), which the combined record
does not mention; docs/reviews/M2-005-finding-resolutions.md; docs/reviews/M2-004-finding-resolutions-r2.md;
docs/handoffs/M2-005-rereview-r2.md; docs/handoffs/M2-004-rereview-r3.md; ADR 0006 section 1, ADR 0007 section 6,
ADR 0008 section 1; docs/specs/m2_service_ipc_v1.md section 3. Treat author claims and evidence as claims to verify;
judge against the approved ADRs and specifications. State explicitly how the two M2-004 revision-2 records relate and
which verdict stands; the author inferred no approval from the later one.

Targets: candidate 051b4c8aacdeecfabb23625f93dd16615aec39a0 (range 24a35c1914d787b936d14b37b500226dde51722f..051b4c8: the implementation 4f7080b and the harness-only 051b4c8) and the docs-only
commits after it on origin/codex/m2-cryptographic-contract-core (f575ac3 and the one correcting an evidence README
line); confirm they change only docs/. Do not merge
main or review branches into the implementation branch; do not edit implementation files.

1. R01 (core/registryd/src/journal.rs open, create_journal; registry.rs): every created directory entry synced in
   its parent before acknowledgement, the users sync repeated on every creation attempt. Re-run your two directory
   tests; try further orderings (root created and not created, a failed users sync followed by a restart, two
   first enrollments of different UIDs).
2. R02 (core/registryd/src/lineage.rs ended_sessions and check_commit; registry.rs view; core/policyd/src/evaluate.rs
   check_grant; tests/m2/oracle/ecr_oracle.py journal_t; the new published scenario
   session_ended_refuses_later_grants_of_that_session): a grant bound to an ended session is duplicate_identifier at
   admission and at replay; the view carries ended sessions of live grants and evaluation denies them before any
   attestation. Judge the choice of duplicate_identifier (ADR 0007 section 6: session ids are single use) against
   the error vocabulary. Re-run your ended-session test; try network grants, merges and restores that carry such a
   grant, replay of a journal holding one, and a session_ended record repeated.
3. R03 (core/registryd/src/journal.rs reserve: open_reserve, open_journal, create_journal, journal_file_t
   replenish_reserve and append; the journal_io_t defaults allocate/release/truncate; registry.rs restrictive flag;
   core/registryd/src/bin/cham3_reserve_probe.rs; tests/m2/storage/check_reserve_full_disk.sh): the preallocated
   1 MiB reserve of written zeros, released only by a restrictive append meeting ENOSPC/EDQUOT after truncating the
   journal back to its last complete frame, refilled before every non-restrictive append and otherwise refusing it.
   Re-run your allocation test and your filesystem_full_reserve test (2 MiB tmpfs). Run
   tests/m2/storage/check_reserve_full_disk.sh in the pinned container with --tmpfs
   /full-disk:rw,size=2m,uid=$(id -u),gid=$(id -g),mode=0700 and judge the probe (it must refuse any filesystem that
   is not a small tmpfs). Try: a torn restrictive write, two restrictive appends on a full filesystem, a reserve that
   cannot be refilled across a restart, a partially allocated reserve, a reserve file replaced by a symbolic link or
   with other modes. Judge the lazy replenishment and the stated limits (compressing filesystems; no status field
   before M2-006).
4. M2-004-R2-01 (core/vaultd/src/service.rs register_proposal): pending ids count against the retained-id bounds.
   Re-run your two quota tests, your other ten and the original seventeen (the I01 assertion is expected to fail
   as before). Confirm no marker is dropped and no id revived.
5. Confirm every earlier PASS item of both records is intact and that the diff 24a35c1..051b4c8aacdeecfabb23625f93dd16615aec39a0 changes only
   the files the handoffs list: no dependency, Cargo.lock, specification, ADR or key-object format change; the
   vectors differ only by the added scenario (every other vector byte-identical).
6. Evidence: in a clean clone at 051b4c8aacdeecfabb23625f93dd16615aec39a0 owned by an unprivileged UID (cloned from the
   fetched ~/cham3 of step 0), build the pinned image and run the
   qualification (docker run --user "$(id -u):$(id -g)" ... bash build/scripts/qualify_m2_rust.sh with
   CHAM3_M2_REQUIRE_CLEAN=1; no --privileged, no added capabilities, no --record) and confirm: unit
   166+166 cargo tests, provider oracle 7/7, ECR oracle 19/96/18 with 23 scenarios / 352 steps,
   format_differential 5955 cases / 0 mismatches (corpus sha256 0ad8bc2b1e8d39224de699db64aa45ca7adcf70117c555f35b57c537471d395e: the five new signed scenario objects), vault_object_oracle 38 checks PASS, harness 137
   (including 21 identity/vault and 36 registry/policy seeded defects and the reserve probe's two tests); then
   build/scripts/run_static_checks.sh and sh tests/run_tests.sh static. Run the protection positive path
   (--ulimit memlock=1073741824:1073741824) and the full-filesystem check (--tmpfs as above) if your Docker allows
   them (your home Docker should); otherwise record BLOCKED with the exact error and compare the CI steps.
7. Compare GitHub Actions m2_checks 38422383429, m1_checks 38422383455 and m0_checks 38422383430 for 051b4c8aacdeecfabb23625f93dd16615aec39a0,
   including the protection and full-filesystem steps. Download the artifact (gh run download 38422383429 -n
   m2_qualification_amd64, or the connector fallback you used), check its digest (sha256:f05dedc6e999261dddc44ffcf780a97ae7b0fa1ea61b26ddd57ab41fe383a926) and
   compare its summaries and the two reserve reports with your own runs.
8. Run the Codex Security plugin skill on the candidate range (24a35c1..051b4c8, and the prerequisite
   bb3f1ba..ca40c62 if it did not run before) and record its exact name/version, the commands it ran and every
   finding; on the home computer its preflight resources and backend are expected to be available — record BLOCKED
   only with the exact command and error (it remains required for M2-008).

Record every check as PASS/FAIL/NOT_RUN/BLOCKED with exact commands, environment and evidence. Return APPROVE or
CHANGES_REQUESTED for M2-005 and for M2-004; make no owner decisions. Publish only your review record on a new
review branch (review/m2-005-r2-security), and end your reply with both blocks filled in: the M2-004 R3 block of
docs/handoffs/M2-004-rereview-r3.md and this one:

=== CHAM3 M2-005 R2 VERDICT ===
reviewer: Codex
environment: <home computer: OS/arch; UID; Docker version; network unrestricted yes/no>
local_clone: <~/cham3 HEAD after step 0; local changes or stashes found: none / moved aside (what)>
review_record: <branch> @ <commit> : <path> (sha256 <digest>)
candidate_reviewed: 051b4c8aacdeecfabb23625f93dd16615aec39a0
docs_commits_reviewed: <shas> (docs-only: yes/no)
verdict: M2-005: <APPROVE/CHANGES_REQUESTED>
prior_findings: R01=<PASS/FAIL> R02=<PASS/FAIL> R03=<PASS/FAIL> earlier PASS items intact=<yes/no>
r02_error_code: <duplicate_identifier accepted / objection>
reserve: <own full-filesystem run: PASS/FAIL/BLOCKED (counts); probe judged; lazy replenishment and limits accepted yes/no>
qualification_amd64: <PASS/FAIL/BLOCKED> (<counts>)
protection_positive_path: <PASS/FAIL/BLOCKED> (<own run or CI step>)
full_filesystem_step: <PASS/FAIL/BLOCKED> (<own run or CI step>)
static_checks: <..>   static_tests: <..>
own_tests: <your nine M2-005 tests and filesystem_full_reserve in debug/release at the candidate; new tests and results>
security_plugin: <ran (name/version, findings) / BLOCKED (exact command and error)>
ci_comparison: <..>
new_findings:
- id: <..> | severity: <..> | location: <file:line> | summary: <..> | required: <..>
notes_for_owner: <..>
=== END VERDICT ===
~~~
