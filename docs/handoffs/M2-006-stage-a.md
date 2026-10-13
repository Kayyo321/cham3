# Handoff: M2-006 stage A — storage integrity (anchor, run marker, restriction ledger, recovery)

- **State:** M2-006 is **IN_PROGRESS**; stage A is **IMPLEMENTED → IN_REVIEW**. Stages B–D are not started. M2-003, M2-004 and M2-005 are DONE. The author approves nothing.
- **Role:** IMPLEMENTER. **Model/provider:** Claude Code (Anthropic), cloud VM. Requested reviewer: Codex (independent, on the owner's home computer).
- **Working directory / branch:** `codex/m2-cryptographic-contract-core` (pushed fast-forward; no merge of `main` or any review branch).
- **Task specification:** [M2 task](../tasks/M2-cryptographic-contract-core.md), M2-006 scope record (2026-10-13), **stage A**: ADR 0006 §§2, 3, 4, 6 and ADR 0008 §1 — the global anchor and its serialization, the run marker and the restart decision, the restriction ledger with `restrict` and `restriction_not_durable`, torn-tail and crash recovery, quarantine and containment, `rollback_status`, the reserve status, `set_policy_epoch` and `set_emergency_restriction` through the restrictive path.
- **Base commit:** 22b6b5f (the M2-005 completion docs on the approved M2-005 candidate 5860f276ab593da34771f704691ea35649324d6e; no code between them).
- **Stage-A candidate:** **f4b062826ae54ba4e79c5abd08eeb9fe21d48510** (one commit: range 22b6b5f..f4b0628).
- **Docs-only follow-up:** the docs-only commit after f4b0628 (its parent must be f4b0628) (this handoff, the evidence, the task record and STATUS).

Use `PROJECT.md` and the approved ADRs as product-authority references. The formats this stage writes are recorded in [registryd storage formats v1](../specs/m2_registry_storage_v1.md) (an implementation record; ADR 0006 controls).

## Completed work

**Behaviour and files (`git diff --stat 22b6b5f f4b0628`: 17 files changed, 5 added):**

- `core/registryd/src/anchor.rs` (new): the anchor map (run marker, policy state, per-user `seq`/`chain`/`state`/`user_id`/`identity_seq`/`min_valid_policy_epoch`, tombstones), two alternating slots `"C3AN" ‖ version ‖ counter ‖ map ‖ SHA-256`, open (the valid slot with the higher counter; `absent`/`invalid`/`loaded`), update (write the older slot, sync, then take the change in memory; the directory synced after the first write to each slot in a run), `independent`/`degraded` by `st_dev`.
- `core/registryd/src/ledger.rs` (new): the restriction ledger — `validity` (`"C3RV" ‖ run_id ‖ boot_id`) and `entries` (`"C3RL"` frames with a per-entry SHA-256) — started fresh at every start, appended before any disk write of a restrictive change, read as the previous run's evidence; a failed append marks it failed and removes the validity file.
- `core/registryd/src/registry.rs`: the start sequence (anchor, previous ledger, the §3 decision `resumed`/`ledger_replayed`/`contained`, policy state raised by configuration and ledger, the §4 recovery of every journal, re-journaling of ledger entries, containment, this run's ledger and the `running` marker — all before anything is served); `recover_user` (torn tail beyond the anchor truncated; stale/unexplained/chain/replay/lost-store/anchor/tombstone rows); the restrictive write path (`restrictive_write`, `flush_pending`, retries by every later operation, `restriction_not_durable`); `restrict`, `set_policy_epoch`, `set_emergency_restriction`, `rollback_status`, `shutdown` (`clean` marker only with nothing pending), `policy_state`, `run_decision`; `admit` checks the persisted epoch and refuses contained/quarantined users and users with a pending write; `identity_state` answers `quarantined` for a quarantined slot; `lineage_head` carries the policy state and the user state and is refused while the ledger has failed.
- `core/registryd/src/record.rs`: the registry-written user records (`containment`, `quarantine`, `admin_recover`, `recovery_opened`, `recovery_closed`, `deletion_pending`) and `parse_journal_prefix` (valid length, torn flag; an over-long declared length is a defect, not a torn tail). `core/registryd/src/lineage.rs`: `contain` (every live grant ends `suspended`/`quarantined`, branches end), `apply_user_record`, the end reasons `suspended`, `quarantined`, `deleted`. `core/registryd/src/journal.rs`: `replace_file`/`append_file` on the io trait, `truncate_tail`, `chain()`, and the rollback of a failed append (truncate to the last durable length; only a failed rollback denies until restart — M2-005 denied until restart on every failed append).
- `core/policyd/src/{evaluate,service}.rs`: the persisted emergency restriction denies in `check_subject`; `policy_status` reports registryd's persisted policy state and the anchor mode (`rollback_protection_t::{degraded, independent}` replaces `none`); `set_policy_epoch` / `set_emergency_restriction` (`admin`) forward to registryd.
- `core/registryd/src/bin/cham3_reserve_probe.rs`, `tests/m2/storage/check_reserve_full_disk.sh`: the full-disk probe also starts a ledger on the full tmpfs — the append is refused, the ledger marked failed, its validity removed, the next start would see no ledger, and it stays failed after space returns; the wrapper asserts those lines.
- `core/registryd/Cargo.toml`, `Cargo.lock`: `cham3_os_adapter` (the random run id) as a registryd dependency; the lock gains that one edge (regenerated offline; no new crate).
- Tests: `core/registryd/tests/storage_integrity.rs` (new, 12 tests), `registry_operations.rs` (the tamper, chained-record, ended-session, failed-append and sync-order tests rewritten to the anchor-first semantics; `failed_appends_change_nothing_and_are_rolled_back`, `a_failed_rollback_denies_the_user_until_restart`; the reserve test checks `depleted_reserves`), `tests/builders/registry_support.rs` (shared helpers), `core/policyd/tests/current_authorization.rs` (+3). Harness: `test_m2_registry_policy.py` gains 21 seeded stage-A defects (62). Inventory: 188 cargo tests (+19: 12 storage, 3 policyd, 1 rollback, 3 anchor/ledger unit tests). Specifications: `docs/specs/m2_registry_storage_v1.md` (new), IPC rows `policy_status`, `set_policy_epoch`/`set_emergency_restriction`, `rollback_status`.

**Decisions within the approved scope (author placements, open to review):**

1. The anchor slot carries a version field after the magic (ADR 0006 §2 names magic, counter, map, checksum); the journal header `"C3JH"` is this implementation's "header" of §4; the ledger's two files and frame layout are the author's. All in the storage spec.
2. Per user the anchor also holds `min_valid_policy_epoch` (raised by `recover_user`, stage C) and the global policy state, so that `set_policy_epoch`/`set_emergency_restriction` are durable without a journal of their own.
3. A failed ordinary append is rolled back (the file truncated to its last durable length) and the user continues; M2-005 denied the user until restart. ADR 0006 §3 requires the restrictive path to retry without exiting, which needs a usable journal after a failed write; a failed rollback still denies until restart. The "reported failed, found admitted" case of §1 is now reached only through a failed rollback of a whole frame (tested).
4. A quarantine decided from a readable journal (anchor state, tombstone, invalid or absent anchor) keeps a slot in the quarantined state with every grant ended `quarantined`, so that `lineage_head` reports the state to policyd and stage C's `recover_user` has the verified prefix; a quarantine decided from the bytes (stale, unexplained, chain mismatch, replay defect, lost store) has no slot. identityd is told `quarantined` instead of records and answers `key_not_certified` for the user's keys from a fresh index (the ADR's `quarantined` answer needs an index that already knows the user; stage C).
5. An absent anchor while journals exist and two invalid slots both quarantine every user as `anchor_invalid`; a fresh anchor is then written for the run and the quarantined users are not re-anchored (stage C's `recover_user` re-anchors).
6. The anchor directory is synced after the first write to each slot in a run (never inferred from the slot's existence, R2-01); the ledger directory is synced in its parent at every start.
7. `set_emergency_restriction(false)` also writes a ledger entry (harmless: replay only raises); a failed anchor write after a release leaves the restriction in force at the next start.
8. The policy state is raised at start by the configuration and never lowered; a lower `set_policy_epoch` request is accepted and changes nothing (the IPC spec says so); `min_valid_epoch > epoch` is `malformed_request`.
9. `rollback_status` separates `quarantined` (recovery-table reasons) from `unusable` (store defects); `depleted_reserves` reports the M2-005 reserve state until stage B's status operations.
10. Owner choice §9.1 option A is implemented (degraded default, `independent` by configuration); §9.2 option C1 is stage C.

## Verification (at f4b062826ae54ba4e79c5abd08eeb9fe21d48510)

| Check | Environment / command | Result | Evidence / limits |
|---|---|---|---|
| Stage-A tests | `storage_integrity` (12): resumed/clean/running marker, restrictive path (denial, ledger first, pending, retry, shutdown refusal), same-boot ledger replay, cross-boot containment (both users, records, anchor, identityd scope, persistence, a later user unaffected), missing/invalid/other-run ledger, ledger failure (no eligible decisions, validity removed, next start contained), anchor failure after a durable record (denied, rolled forward), policy state (restrictive, persisted, never lowered, role checks, ledger entries, slot content), the recovery table from restored copies (stale, unexplained, lost file, lost directory, both slots invalid, anchor gone, restored), tombstones (pending, deleted), one torn slot (older slot taken, rewritten; an unsafe slot refuses), an anchor on `/dev/shm` (independent; a restored store detected as stale) | PASS 12/12 (author, pinned container) | in the qualification |
| Rewritten M2-005 tests | `registry_operations` (25): torn inside the anchor quarantines and keeps the file, torn beyond it is truncated and resumes, an over-long frame is a defect; rewritten prefixes are chain mismatches, shortened journals stale, appended bad records replay defects, a valid extension rolls the anchor forward; failed appends rolled back; failed rollbacks deny until restart (torn discarded / whole frame rolled forward); sync orders with the anchor and ledger directories | PASS 25/25 | in the qualification |
| policyd | `current_authorization` (12): the persisted emergency restriction denies everyone and persists; a raised epoch through registryd ends older grants and persists; a contained user is denied and admits nothing | PASS 12/12 | in the qualification |
| Seeded defects | `test_m2_registry_policy.py`: 62 defects (21 new: stale/unexplained/chain rows, torn tails, tombstones, quarantine grants, cross-boot replay, unverifiable resume, containment record and grants, ledger-first, failed ledger serving/validity, anchor failure, shutdown with pending, epoch lowered, slot counter, slot checksum, failed frame kept, persisted emergency) each caught by the named test, the unmodified copy passes | **PASS** (62/62 detected, each by the named test, the unmodified copy passes: 429.6 s in the author run and in the qualification harness) | in the qualification's harness run |
| Full M2 qualification | Clean UID-1000 clone, pinned container, `qualify_m2_rust.sh`, `CHAM3_M2_REQUIRE_CLEAN=1` | **PASS** (`m2 qualification: PASS`; unit 188+188 inventory-exact, harness 147 in 997.2 s; provider 7/7, ECR 19/96/18 with 23 scenarios / 352 steps, format differential 5 955 / 0, vault oracle 38; layer-7 differential 11 scenarios / 59 steps / 105 comparisons / 0 disagreements; `dirty_tracked 0`) | [VM evidence](../evidence/M2-cryptographic-contract-core/m2_006a_amd64_vm_f4b0628/) |
| Static checks; static tests | same clone and container | **PASS** (`static checks: PASS`; `Ran 82 tests … OK`; the clone clean afterwards) | `qualification_console.txt` |
| Full-filesystem probe with the ledger, then the five wrapper cases | `--tmpfs /full-disk:rw,size=2m,…` | **PASS** (both profiles: reserve path as in M2-005; `ledger_started`, `ledger_append_on_full_filesystem refused`, `ledger_failed`, `ledger_validity_removed`, `previous_ledger missing`, `ledger_stays_failed`); wrapper cases 5/5, no leftovers | `storage/` |
| Unchanged inputs | `git diff --stat 22b6b5f f4b0628` | no vector, oracle, ADR or workflow change; `Cargo.lock` +1 edge | `unchanged_inputs.txt` |
| GitHub Actions at f4b0628 | m2 38459079087, m1 38459079018, m0 38459079003 | **success**, every step of every job (m2 qualification `m2 qualification: PASS`, protection positive path PASS, full-filesystem step PASS with the ledger lines, wrapper step 5/5) | [CI evidence](../evidence/M2-cryptographic-contract-core/m2_006a_amd64_ci_f4b0628/) |
| Vault protection, positive path | CI step | **PASS** in CI (`protection_verified`, `vault protection positive path: PASS`); BLOCKED in this VM (memlock) | CI evidence |
| Real power loss, kernel crash, process death at each boundary (A08/A10) | — | NOT_RUN: faults are injected at the io boundaries and reboots simulated by `boot_id` | storage spec §7 |
| Miri, sanitizers, coverage-guided fuzzing | — | NOT_RUN (NOT_AVAILABLE on the pinned toolchain) | — |

## Review

- **Requested:** independent review of stage A by Codex (prompt below); record on a new branch `review/m2-006-a-security` based on `main` (9cf8825), one file.
- **Earlier records on this package:** none (M2-006 starts here). The M2-005 approval (165e149) does not extend to M2-006 (the reviewer said so).

## Remaining

- **Known risks and limits:** the anchor has no reserve (a restrictive change on a full filesystem is durable in the journal, pending in the anchor, rolled forward at the next start); a whole-volume restore with a `degraded` anchor is undetectable (ADR 0006 §2); no real crash tests; identityd's `key_not_certified` for a quarantined user's keys from a fresh index; the ledger's tmpfs placement, transport and processes are M2-007; `recover_user`, the ceremony, tombstone creation and deletion are stage C; `history`, trusted time, `timestamp_rule`, one-shot reservations and `confirm_enforcement` are stage B; policyd's per-user emergency overlay stays configuration.
- **Uncommitted state:** none after the docs commit.
- **Next:** Codex reviews stage A; findings are answered as stage-A revisions; an APPROVE opens stage B (lifecycle and time). Each stage keeps its own base and candidate.
- **Reviewer environment (owner note):** Codex runs on the owner's home computer, unrestricted; `~/cham3` is refreshed by step 0 of the prompt. The container-only `--ulimit nofile=1024:1024` for Valgrind is acceptable if recorded.

## Review prompt for Codex

~~~text
You are the independent REVIEWER (Codex) for cham3 M2-006 stage A in Kayyo321/cham3: a full security review of
one staged candidate. You did not author this work. You run on the owner's home computer with unrestricted
access; record BLOCKED only with the exact command and error.

0. Bring the local clone to the candidate FIRST:
     cd ~/cham3 && git status --porcelain && git stash list
     git fetch origin codex/m2-cryptographic-contract-core main
     git checkout codex/m2-cryptographic-contract-core && git reset --hard origin/codex/m2-cryptographic-contract-core
     git rev-parse HEAD        # expect the docs-only commit after f4b0628 (its parent must be f4b0628) (docs-only; parent f4b0628)
     git log --oneline -4      # 5860f27 (M2-005 candidate) -> aeaee2d -> 22b6b5f (docs) -> f4b0628 (stage A) -> the docs-only commit after f4b0628 (its parent must be f4b0628)
   Move unexpected local changes aside (git stash); never force-push or edit the implementation branch. Read every
   document from this checkout; make the clean clone for testing from it
   (git clone --no-hardlinks ~/cham3 <dir>; git -C <dir> checkout --detach f4b062826ae54ba4e79c5abd08eeb9fe21d48510).

Read AGENTS.md; docs/decisions/0006_m2_storage_anchor_recovery.md (the architecture; §9 owner decisions A and C1);
docs/decisions/0008_m2_retention_privacy_update_identity.md §1; docs/specs/m2_registry_storage_v1.md (the
implementer's record of the formats and the decision tables — check it against the ADR, not the other way round);
the changed rows of docs/specs/m2_service_ipc_v1.md; docs/handoffs/M2-006-stage-a.md; the M2-006 scope record in
docs/tasks/M2-cryptographic-contract-core.md. Treat author claims as claims to verify.

Targets: candidate f4b062826ae54ba4e79c5abd08eeb9fe21d48510 (range 22b6b5f..f4b0628, one commit) and the docs-only commit after it. Confirm the
range changes only core/registryd, core/policyd, the probe and its wrapper, the harness module and inventory, the
two specifications, core/registryd/Cargo.toml and one Cargo.lock edge (cham3_os_adapter for registryd) — no
vector, oracle, ADR, workflow, Dockerfile or pin — and that the docs-only commit changes only docs/. Do not merge
main or review branches; do not edit implementation files.

1. ADR 0006 §2 anchor: slot layout, alternation, higher-counter selection, checksum, the invalid-slot and unsafe-slot
   paths, the in-memory map as the authority, update order (older slot written and synced before the change is
   taken), lock order (user lock → anchor lock; no user lock taken under the anchor lock), degraded/independent by
   st_dev. Try: both slots valid with equal counters, a slot of another store, a truncated slot, a slot with a
   trailing byte, a huge user count, a boot id of 65 bytes.
2. §3 run marker and ledger: the four rows of the restart table, including a valid ledger of another run and a
   ledger whose entries fail their checksum; re-journaling (no duplicate on a second restart; an entry whose record
   the journal already holds; an entry for an unknown or quarantined user); containment of every user with a
   durable record, suspended grants, ended branches, the anchor state, identityd's scope and policyd's denial;
   the restrictive path order (denial, ledger, journal, anchor, reply) at every failure point, including a journal
   failure followed by process death in the same boot and across boots; `restriction_not_durable`; `shutdown`
   refusing with pending writes or a failed ledger; the ledger failure path (validity removed, no eligible
   decisions, restrictions still journaled, the next start contained).
3. §4 recovery table: every row, including the two tombstone rows (craft a slot with encode_slot), identity_seq and
   user_id checks, a torn tail inside versus beyond the anchor, a torn tail that is a complete frame of another
   user's chain, and the rollback-restore scenarios of a restored journal (stale), a restored anchor (unexplained),
   a restored store with an independent anchor, and a store moved to another machine. Judge the author's
   placement that a readable-journal quarantine keeps a slot (every grant ended `quarantined`, `lineage_head`
   reports the state, `identity_state`/`admit`/`append_identity`/`restrict` answer `quarantined`) and that a
   fresh identityd answers `key_not_certified` for such a user's keys.
4. §6 fault points: the rollback of a failed ordinary append (now the user continues; M2-005 denied until
   restart): is any state visible to callers inconsistent with the bytes after a failed write, a failed sync, or a
   failed rollback? The anchor failure after a durable record (denied until restart, rolled forward). The anchor
   without a reserve on a full filesystem (documented limit).
5. Policy: `set_policy_epoch`/`set_emergency_restriction` as restrictive changes, never lowered, persisted,
   raised by configuration, enforced at admission (`stale_epoch`) and evaluation (`epoch_invalid`,
   `emergency_restriction`); roles.
6. Harness: run the twelve storage_integrity tests, the 25 registry_operations tests and the twelve policyd tests;
   run tests/m2/harness/test_m2_registry_policy.py (62 seeded defects) and judge whether each new defect's test
   discriminates for the right reason; run the full-filesystem wrapper on a 2 MiB tmpfs (the ledger lines) and the
   twelve probe-harness tests.
7. Evidence: qualification in a clean clone at f4b062826ae54ba4e79c5abd08eeb9fe21d48510 (CHAM3_M2_REQUIRE_CLEAN=1; the nofile limit as before):
   unit 188+188, provider 7/7, ECR 19/96/18 with 23 scenarios / 352 steps, format differential 5955 / 0 (corpus
   unchanged), vault oracle 38, harness 147 (12 reserve-probe, 62 registry/policy and 21 identity/vault seeded
   defects); static checks and sh tests/run_tests.sh static. Compare GitHub Actions m2 38459079087, m1 38459079018, m0 38459079003; download the artifact
   and check its digest (id 11846490342, 44 files, 105 358 bytes, sha256:6c27239b2149b3a0d39f342f83adc81f0500c1060d5a4437a35dfbd080319a04).
8. Codex Security plugin on 22b6b5f..f4b0628; record name/version and findings.

Record every check as PASS/FAIL/NOT_RUN/BLOCKED with exact commands and evidence. Return APPROVE or
CHANGES_REQUESTED for M2-006 stage A; make no owner decisions (list questions for the owner separately). Publish
only your record on a new review branch (review/m2-006-a-security) and end with:

=== CHAM3 M2-006 STAGE A VERDICT ===
reviewer: Codex
environment: <OS/arch; UID; Docker version; nofile limit used>
local_clone: <~/cham3 HEAD after step 0; local changes or stashes: none / moved aside (what)>
review_record: <branch> @ <commit> : <path> (sha256 <digest>)
candidate_reviewed: f4b062826ae54ba4e79c5abd08eeb9fe21d48510
docs_commits_reviewed: <shas> (docs-only: yes/no)
scope_exact: <the listed paths and nothing else: yes/no>
verdict: M2-006 stage A: <APPROVE/CHANGES_REQUESTED>
anchor: <PASS/FAIL> (<..>)
run_marker_and_ledger: <PASS/FAIL> (<..>)
recovery_table: <PASS/FAIL> (<..>)
fault_points: <PASS/FAIL> (<..>)
policy_state: <PASS/FAIL> (<..>)
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
