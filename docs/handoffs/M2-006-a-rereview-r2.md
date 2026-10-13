# Handoff: M2-006 stage A revision 2 — request for independent re-review

- **State:** M2-006 is **IN_PROGRESS**; stage A is **IMPLEMENTED → IN_REVIEW** (revision 2). Stages B–D are not started. M2-003, M2-004 and M2-005 are DONE. The author approves nothing.
- **Role:** IMPLEMENTER. **Model/provider:** Claude Code (Anthropic), the owner's home computer. Requested reviewer: Codex (independent, on the owner's home computer).
- **Review answered:** `docs/reviews/M2-006-stage-a-codex-security.md` at 344c37210ff15f297cf015ee047d8e147efe2fbd on `review/m2-006-a-security` (two commits on `main` 9cf8825, the branch adds only that file, SHA-256 `f7c3d57e9e90c11e6a6726571cef51579346c13f7bc1c174097a8344fd4bd37e`; verified, not merged or edited). Author response: [M2-006 stage A finding resolutions r2](../reviews/M2-006-a-finding-resolutions-r2.md).
- **Owner decisions used:** none were needed. The one new byte value (run marker state 3, `unverifiable`) is an author placement in the storage formats record, open to review.
- **Base:** a9e47e47660879af86ede99986b25a18ffb537aa (the stage-A docs-only commit on the reviewed candidate f4b0628).
- **Revision-2 candidate:** **e34fd8a0b97f5a606656ab87914d4d1b19517d7c** (one commit on the base: range a9e47e4..e34fd8a, 10 files).
- **Docs-only follow-up:** the commit adding this handoff, the resolutions, the evidence, the corrected stage-A handoff sentence, the task progress and STATUS.

## Completed work

| Finding | Change | Where |
|---|---|---|
| R01 | Reads (`lineage_head`, `identity_state`) retry the anchor update of an unanchored record and refuse while it keeps failing: nothing of an unanchored state is served; the user is denied until the record is anchored (in-run retry or roll-forward), no longer "until restart" | `registry.rs` |
| R02 | A restriction or key invalidation after an anchor failure is denied, ledgered and kept pending behind the unanchored record; the anchor is retried first and nothing is appended beyond an unanchored record | `registry.rs` |
| R03 | Replayed ledger entries take the restrictive path (this run's ledger first); in-memory effects applied as the ledgering operation applies them; journaled records recognized exactly, never written twice | `registry.rs` |
| R04 | The policy state in force is published by every slot write and with the clean marker; `rollback_status.policy_pending` | `registry.rs` |
| R05 | A release of the emergency restriction is anchored first and in force only once durable | `registry.rs` |
| R06 | A validity file without an entries file is an invalid ledger | `ledger.rs` |
| R07 | The anchor's identity count is reconciled with the anchored prefix; `identity_mismatch` quarantines | `registry.rs` |
| R08 | An anchored quarantine or containment ends the grants and branches as the record would | `registry.rs` |
| Open fault cases | Ledger failure recorded as the run marker state `unverifiable` and the validity file emptied when it cannot be unlinked (`journal_io_t::remove_file`); equal slot counters invalidate the anchor; malformed slots, foreign anchors, ledger-entry cases and concurrent policy publication tested; `rollback_status` lock order | `anchor.rs`, `journal.rs`, `ledger.rs`, `registry.rs` |

**Complete change list of a9e47e4..e34fd8a (`git diff --stat`, 10 files):** `core/registryd/src/anchor.rs`, `core/registryd/src/journal.rs`, `core/registryd/src/ledger.rs`, `core/registryd/src/registry.rs`, `core/registryd/tests/storage_integrity.rs`, `core/policyd/tests/current_authorization.rs`, `docs/specs/m2_registry_storage_v1.md`, `docs/specs/m2_service_ipc_v1.md`, `tests/m2/harness/test_m2_registry_policy.py`, `tests/m2/test_inventory.txt`. No dependency, `Cargo.lock`, manifest, ADR, oracle, vector, workflow, Dockerfile or pin changed.

New controls: `storage_integrity` 23 tests (11 new, one rewritten as `a_failed_anchor_write_denies_the_user_until_the_record_is_anchored`, two extended), `current_authorization` 14 (+2), one anchor unit test; 81 seeded registry/policy defects (+19); inventory 202 cargo tests (+14).

## Verification (at e34fd8a0b97f5a606656ab87914d4d1b19517d7c)

| Check | Environment / command | Result | Evidence / limits |
|---|---|---|---|
| Reviewer's tests, unchanged, at f4b0628 and at the candidate | pinned container, UID 1001, clean clones; `cargo test --frozen -p cham3_registryd --test reviewer_storage review_`; `cargo test --frozen -p cham3_policyd --test policy_review` (debug and release) | **f4b0628: 0/8 pass, adverse 2/2 pass; e34fd8a: 8/8 pass, adverse 0/2** (the defects are gone) | `reviewer_reproductions/` |
| Stage-A revision-2 tests | `storage_integrity` (23), `registry_operations` (25), `current_authorization` (14), `anchor::tests` (3): the eleven new tests cover R01–R08 and the open fault cases, each with its restart and recovery exits | PASS (debug and release) | in the qualification |
| Seeded defects | `test_m2_registry_policy.py`: 81 defects (19 new), each caught by the named test, the unmodified copy passes | **PASS** (harness 147 tests in 368.6 s; every anchor unique, `count == 1`, after `cargo fmt`) | in the qualification's harness run |
| Full M2 qualification | Clean clone owned by this host's unprivileged UID 1001, pinned container `cham3-m2-dev` (image id 4f4f684d76f7, the same as the reviewer's), `--ulimit nofile=1024:1024`, `qualify_m2_rust.sh`, `CHAM3_M2_REQUIRE_CLEAN=1` | **PASS** (`m2 qualification: PASS`; unit 202+202 inventory-exact, harness 147 in 368.6 s; provider 7/7, ECR 19/96/18 with 23 scenarios / 352 steps, format differential 5 955 / 0, vault oracle 38; layer-7 differential 11 scenarios / 59 steps / 105 comparisons / 0 disagreements; `dirty_tracked 0`, `Cargo.lock` unchanged) | [VM evidence](../evidence/M2-cryptographic-contract-core/m2_006a_r2_amd64_vm_e34fd8a/) |
| Static checks; static tests | same clone and container | **PASS** (`static checks: PASS`; `Ran 82 tests … OK`; the clone clean afterwards) | `qualification_console.txt` |
| Full-filesystem probe with the ledger, then the five wrapper cases | `--tmpfs /full-disk:rw,size=2m,…` | **PASS** (both profiles, the ledger lines as at f4b0628; wrapper cases 5/5, no leftovers) | `storage/` |
| Unchanged inputs | `git diff --stat a9e47e4 e34fd8a` | the ten files; no dependency, `Cargo.lock`, manifest, ADR, oracle, vector, workflow, probe, wrapper, Dockerfile or pin change; `record.rs`, `lineage.rs`, `policyd/src`, the builders and the other test modules untouched | `unchanged_inputs.txt` |
| GitHub Actions at e34fd8a | m2 38481185159, m1 38481185180, m0 38481185314 | **success**, every step of every job (m2 qualification `m2 qualification: PASS`, protection positive path PASS, full-filesystem step PASS with the ledger lines, wrapper step 5/5; artifact 11857892996 downloaded, its digest as reported) | [CI evidence](../evidence/M2-cryptographic-contract-core/m2_006a_r2_amd64_ci_e34fd8a/) |
| Vault protection, positive path | CI step | **PASS** in CI (`protection_verified`, `vault protection positive path: PASS`); NOT_RUN on this host (memlock limit) | CI evidence |
| Real power loss, kernel crash, process death at each boundary (A08/A10) | — | NOT_RUN: faults are injected at the io boundaries and restarts/reboots simulated | storage spec §7 |
| Miri, sanitizers, coverage-guided fuzzing | — | NOT_RUN (NOT_AVAILABLE on the pinned toolchain) | — |

## Remaining

- **Known risks and limits:** as in the stage-A handoff, plus the recorded residual of a ledger failure whose validity file can neither be unlinked nor emptied while the anchor cannot be written either (storage formats §7).
- **Uncommitted state:** none after the docs commit.
- **Next:** Codex re-reviews stage A revision 2 (bounded, prompt below); an APPROVE closes stage A (completion handoff) and opens stage B.
- **Reviewer environment (owner note):** Codex runs on the owner's home computer, unrestricted; `~/cham3` is refreshed by step 0 of the prompt. The container-only `--ulimit nofile=1024:1024` for Valgrind is acceptable if recorded.

## Review prompt for Codex

~~~text
You are the independent REVIEWER (Codex) for cham3 M2-006 stage A revision 2 in Kayyo321/cham3: a bounded
re-review of your eight findings (M2-006-A-R01–R08) and of the fault cases your record left open. You did not
author this work. You run on the owner's home computer with unrestricted access; record BLOCKED only with the
exact command and error.

0. Bring the local clone to the candidate FIRST:
     cd ~/cham3 && git status --porcelain && git stash list
     git fetch origin codex/m2-cryptographic-contract-core review/m2-006-a-security main
     git checkout codex/m2-cryptographic-contract-core && git reset --hard origin/codex/m2-cryptographic-contract-core
     git rev-parse HEAD        # expect the docs-only commit after e34fd8a (its parent must be e34fd8a)
     git log --oneline -5      # f4b0628 (stage A) -> a9e47e4 (docs) -> e34fd8a (revision 2) -> the docs-only commit after e34fd8a
   Move unexpected local changes aside (git stash); never force-push or edit the implementation branch. Read every
   document from this checkout; make the clean clone for testing from it
   (git clone --no-hardlinks ~/cham3 <dir>; git -C <dir> checkout --detach e34fd8a0b97f5a606656ab87914d4d1b19517d7c).

Read AGENTS.md; your record review/m2-006-a-security at 344c37210ff15f297cf015ee047d8e147efe2fbd;
docs/reviews/M2-006-a-finding-resolutions-r2.md; docs/handoffs/M2-006-a-rereview-r2.md; the changed parts of
docs/specs/m2_registry_storage_v1.md (§§2–7) and docs/specs/m2_service_ipc_v1.md (the rows restrict,
lineage_head, append_identity/identity_state, set_policy_epoch/set_emergency_restriction, rollback_status), with
ADR 0006 §§2–4, §6 as the authority. Treat author claims as claims to verify.

Targets: candidate e34fd8a0b97f5a606656ab87914d4d1b19517d7c (range a9e47e4..e34fd8a, one commit, ten files:
core/registryd/src/{anchor,journal,ledger,registry}.rs, core/registryd/tests/storage_integrity.rs,
core/policyd/tests/current_authorization.rs, docs/specs/m2_registry_storage_v1.md,
docs/specs/m2_service_ipc_v1.md, tests/m2/harness/test_m2_registry_policy.py, tests/m2/test_inventory.txt) and
the docs-only commit after it (docs/ only, including the corrected file-count sentence of
docs/handoffs/M2-006-stage-a.md). Confirm nothing else changed: no dependency, Cargo.lock, manifest, ADR, oracle,
vector, workflow, Dockerfile or pin. Do not merge main or review branches; do not edit implementation files.

1. Your tests, unchanged: re-run reviewer_storage.rs (the eight review_ tests) and policy_review.rs at the
   candidate, debug and release. Expect 8/8 pass, and the two adverse-behaviour tests to fail with
   unknown(identity_unavailable) and denied(emergency_restriction). Compare with the author's
   reviewer_reproductions/ logs in the VM evidence.
2. R01/R02, the anchor-failure contract (registry.rs: anchored_for_reading, ready, flush_pending, anchor_slot,
   restrict, append_identity_to, intended_sequence): after a durable record whose anchor update failed, nothing of
   the unanchored state is served (lineage_head, identity_state, policyd evaluate/network_mode, identityd
   refresh), nothing is appended beyond it, restrictions and key invalidations are denied, ledgered and pending,
   and the record is anchored by the first successful retry (reads included) or by the next start's
   roll-forward. Judge the author placement that denial now lasts "until anchored" rather than "until restart"
   (anchor_failed is cleared only by a successful slot write naming the record) and the retry from a read path.
   Try: an anchor failure followed by a second admission attempt, a restriction and a key invalidation, the
   process dying at each point, same boot and new boot; an anchor that fails for the retry too.
3. R03, rejournal: the in-memory application per record kind, the entry re-ledgered before the journal write,
   exact-record matching (lifecycle_records), unknown and quarantined users, a containment entry, an identity
   entry; three and four failed same-boot restarts; the identity double-count of the first candidate is gone.
4. R04/R05, policy: change_policy under the policy lock (ledger, then anchor), release_policy (anchor first),
   publication by anchor_slot and by shutdown, policy_pending, the lock order users → user → policy → ledger →
   anchor (no site takes policy or ledger under anchor; rollback_status rewritten). Try concurrent raises,
   releases and enrollments; a release whose anchor update fails followed by a same-boot death and by a reboot.
5. R06/R07/R08: read_previous with a missing entries file; replay_user's identity count at the anchored
   sequence (an identity record as the unanchored a+1 record is the justified case); the anchored
   quarantined/contained branches on the slot helpers; craft slots with encode_slot as before.
6. The open fault cases: run marker state unverifiable (anchor.rs byte 3; registry.rs mark_run_unverifiable and
   open) with the validity-unlink failure, with the anchor failing too; journal_io_t::remove_file (a default
   method: your fault_io_t compiles unchanged); equal slot counters; a foreign anchor; the ledger-entry matrix;
   the residual the author records in storage formats §7 (unlink, truncation and the anchor all failing while a
   restriction's journal write fails too), which you judge for stage A.
7. Harness: 81 seeded defects in test_m2_registry_policy.py (19 new, three anchors re-pointed); judge whether
   each new defect's test discriminates for the right reason. Inventory 202 cargo tests.
8. Evidence: qualification in a clean clone at e34fd8a (CHAM3_M2_REQUIRE_CLEAN=1; the nofile limit as before):
   unit 202+202, provider 7/7, ECR 19/96/18 with 23 scenarios / 352 steps, format differential
   5955 / 0 (corpus unchanged), vault oracle 38, harness 147 (12 reserve-probe, 81 registry/policy and
   21 identity/vault seeded defects); static checks and sh tests/run_tests.sh static; the full-filesystem wrapper
   and the five wrapper cases on the 2 MiB tmpfs. Compare GitHub Actions m2 38481185159, m1 38481185180, m0
   38481185314; download the artifact and check its digest (id 11857892996, 44 files,
   106 575 bytes, sha256:d20cc30e5ef855f61e841e7fe8cfcaf93495bbd00e2e847736292150d3409173).
9. Codex Security plugin on a9e47e4..e34fd8a; record name/version and findings.

Record every check as PASS/FAIL/NOT_RUN/BLOCKED with exact commands and evidence. Return APPROVE or
CHANGES_REQUESTED for M2-006 stage A; make no owner decisions (list questions for the owner separately).
Publish only your record on a new review branch (review/m2-006-a-r2-security) and end with:

=== CHAM3 M2-006 STAGE A R2 VERDICT ===
reviewer: Codex
environment: <OS/arch; UID; Docker version; nofile limit used>
local_clone: <~/cham3 HEAD after step 0; local changes or stashes: none / moved aside (what)>
review_record: <branch> @ <commit> : <path> (sha256 <digest>)
candidate_reviewed: e34fd8a0b97f5a606656ab87914d4d1b19517d7c
docs_commits_reviewed: <shas> (docs-only: yes/no)
scope_exact: <the ten files and nothing else: yes/no>
verdict: M2-006 stage A: <APPROVE/CHANGES_REQUESTED>
reviewer_tests_at_candidate: <8/8 pass; adverse 0/2>
r01_r02_anchor_failure: <PASS/FAIL> (<..>)
r03_ledger_replay: <PASS/FAIL> (<..>)
r04_r05_policy: <PASS/FAIL> (<..>)
r06_r07_r08: <PASS/FAIL> (<..>)
open_fault_cases: <PASS/FAIL> (<unverifiable marker, equal counters, foreign anchor, ledger entries, concurrency; the §7 residual>)
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
