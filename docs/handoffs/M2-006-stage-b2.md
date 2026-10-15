# Handoff: M2-006 stage B2 — reservations, sessions, confirmations, audit gaps, history

- **State:** M2-006 is **IN_PROGRESS**. Stages A and B1 are DONE (reviews 06a9423 and 3a978fb, APPROVE). Stage **B2 is IMPLEMENTED → IN_REVIEW**. C and D are not started. The author approves nothing.
- **Role:** IMPLEMENTER. **Model/provider:** Claude Code (Anthropic), on the owner's home computer. Requested reviewer: Codex (independent, same computer).
- **Working directory / branch:** `codex/m2-cryptographic-contract-core`, pushed fast-forward. No merge of `main` or of any review branch.
- **Task specification:** [M2 task](../tasks/M2-cryptographic-contract-core.md). The M2-006 scope record (2026-10-13), the stage B split (owner, 2026-10-14) and the B2 plan recorded with erratum E7 define **stage B2**: ADR 0007 §§2–4, 6, 7 and ADR 0008 §2.
  - The lifecycle writers: the expiry latch (`expired`), `session_event` (`session_ended`), and the remaining narrowing records of §3 that stage B owns.
  - One-shot `reserve_use` / `complete_use` (§4 option A) and `consumed_uncertain` at restart.
  - `confirm_enforcement`, startup reconciliation (§7), audit gaps and `history`.
- **Owner decisions used:**
  - ADR 0007 §4 **Option A** (2026-10-12): the reservation consumes.
  - The stage B split (2026-10-14).
  - **Erratum E7** (2026-10-14, recorded before the work in f6f6883), each the recommended option:
    1. policyd fronts `reserve_use`; registryd re-checks its own rules under the user lock.
    2. Enforcement status counts only the current registryd run's confirmations.
    3. `audit_gap` records for lost evidence and for contained starts.
    4. `identity_mismatch` written once per lineage and executable identity; the other derived kinds are not written.
- **Base commit:** f6f6883 (E7, docs-only). The commits since the B1 candidate 5ea2407 are docs-only: 16944ab (B1 revision-3 evidence), ff9cb2c (B1 DONE), f6f6883 (E7).
- **Stage-B2 candidate:** **248ca6ef90d22e270db0e9a3f5bf789502d18720**, one commit (range f6f6883..248ca6e).
- **Docs-only follow-up:** the commit after 248ca6e, whose parent must be 248ca6e. It holds this handoff, the evidence, the task record and STATUS.

Use `PROJECT.md` and the approved ADRs as product-authority references. The formats are recorded in [registryd storage formats v1](../specs/m2_registry_storage_v1.md) §§2, 7 and the new §9; it is an implementation record, and ADR 0007 controls.

## Completed work

`git diff --stat f6f6883 248ca6e`: 13 files changed, 1 of them added.

**Records (`core/registryd/src/record.rs`):**

- New lifecycle record `identity_mismatch {lineage_id, package_identity, executable_sha256}` (evidence only; it ends no grant).
- New registry events:
  - `use_reserved {grant_id, reservation_id, operation_id}`
  - `use_completed {reservation_id, outcome}`
  - `consumed_uncertain {reservation_id}`
  - `session_event {session_id, change}`
  - `enforcement_outcome {commit_id, runtime_id, domain, policy_epoch, result, run_id}`
  - `audit_gap {boot_id, lost}`
- Every field is closed: unknown outcomes, changes, domains, results and lost kinds are refused, and a package identity follows the subject's text rule.

**Index (`core/registryd/src/lineage.rs`):**

- `end_reason_t::consumed`: a reservation ends its grant permanently.
- Reservations by id (open, completed, uncertain) and started sessions.
- `check_event` and `apply_event`: replay applies the new events with the writers' own rules, so a correctly chained event the writer would not have written fails replay (the user is quarantined, `replay_defect`).
- `check_record` for `identity_mismatch`: the lineage has an authoritative subject, and the package identity or executable hash differs from it.

**registryd (`core/registryd/src/registry.rs`):**

- **`reserve_use` (`service:policyd` only, E7.1).** Under the user lock it checks, in order:
  - the user is normal, nothing is owed, and trusted now comes from the operation's own sample;
  - no persisted emergency restriction;
  - the grant exists and the subject equals the lineage's authoritative subject and the caller's user;
  - the signing key is not invalidated as compromised, and the commit's policy epoch is not below the minimum;
  - the lifetime is open: started (including §8 option B), not expired, and a bound session not ended;
  - the grant is current, one-shot, without a terminal record or reservation, under a fresh random reservation id.
  - **Then** it consumes the grant in memory and writes `use_reserved` through the restrictive path before replying. A failed write reports `restriction_not_durable`; the grant stays consumed.
- **`complete_use` (`fixture_consumer`):** `use_completed`, once per reservation, never after `consumed_uncertain`.
- **Startup reconciliation (§7):** at every start (clean, unclean, new boot), each open reservation of a usable, unquarantined user gets `consumed_uncertain` through the restrictive path.
- **`session_event` (`fixture_consumer`):**
  - A start is evidence. It is refused for an ended session, and a repeated start returns the first one's sequence.
  - An end is restrictive: `session_ended` when grants are bound to the session, otherwise a `session_event` end. Either way the session never becomes live again, and admission refuses grants bound to it. A repeated end returns the first one's sequence.
- **`confirm_enforcement` (`fixture_consumer`, E7.2):** for a live head (`stale_head` otherwise), at the epoch in force (`stale_epoch` otherwise), for a normal user. The record binds this run's id. Status per domain is the latest outcome of this run; replay does not restore it.
- **Evidence and audit gaps (E7.3):**
  - Session starts, use outcomes and enforcement outcomes are ordinary appends.
  - When one cannot be written, the caller gets the error, the effect is not applied, and an `audit_gap` naming the lost kind takes the restrictive path.
  - A contained start writes `containment`, then `audit_gap {previous boot, unverifiable_run}`, for each user it contains.
- **`restrict` (E7.4):** takes `identity_mismatch` from `service:policyd` only, once per lineage and executable identity. A repeat returns the first record's sequence.
- **`history` (ADR 0008 §2):** for the owner's trusted client (another UID is `owner_mismatch`) and admin.
  - Pages hold at most 16 records, or one commit; the cursor is the journal sequence. Identity objects are left out.
  - Each commit carries this run's enforcement status. The page also reports the restrictive records not yet durable, and the policy state in force.
  - Refused for a quarantined user and while the last record is unanchored.
- **Diagnostics:** `enforcement_status`.

**policyd (`core/policyd/src/{evaluate,service}.rs`):**

- **Expiry latch (§5 rule 5):** every evaluation and network-mode query that read a snapshot asks `restrict(expired)` for each current grant of the lineage that its trusted now shows expired and that no record ends yet. A failed latch changes no decision.
- **`identity_mismatch` report (E7.4):** when the runtime identity is bound to the lineage (same owner, application and installation) but differs in package identity or executable hash.
- **`reserve_use` (`fixture_consumer`, E7.1):** `evaluate_grant` runs every rule of §1 for the named grant alone (`not_one_shot` when it is not one-shot). Only an eligible grant goes to registryd. Returns the reservation, or the decision that refused it.

**Tests:**

- **`core/registryd/tests/lifecycle_consumption.rs` (new, 11 tests):**
  - a reservation consumes whatever the outcome; roles; a second reservation and a second outcome are refused; replay
  - registryd's re-checks, each refused without a write: not one-shot, unknown, another executable or owner, not started, a §8 option-B forward step (then reached after observed time), a session ended, expired, a stale clock, the emergency restriction, the epoch
  - a contained user reserves nothing
  - a reservation that cannot be journaled still consumes; the ledger holds it; a same-boot restart re-journals it and reports it uncertain
  - every start (clean, unclean, new boot) reports open reservations uncertain, once
  - session starts and ends; a late grant bound to an ended session is refused; replay
  - enforcement status: the latest per domain, epoch, live head, superseded commit, ended branch, pending again after a restart
  - lost evidence leaves gaps that are journaled once storage recovers
  - `identity_mismatch` once per executable identity; roles; the subject itself; an unknown lineage; replay
  - `history`: roles, pages and cursor, commits alone, the journal byte for byte, status, pending restrictions, an unanchored record refused, an unknown user
  - replay refuses five forged stage-B2 records
- **`core/policyd/tests/current_authorization.rs` (+4):**
  - the latch, once, in evaluation and in the network mode
  - the network-mode latch
  - the mismatch report, once per executable identity, not for another runtime scope
  - `reserve_use`: mandatory policy, roles, scope, not one-shot, another executable, session liveness, the reservation, consumption, a session-bound one-shot with a live attestation, the outcome
- **`record.rs` unit test:** round trips and closed fields of the new records.
- **One changed expectation:** `storage_integrity::an_unclean_run_across_boots_contains_every_user` now expects `containment` followed by `audit_gap {boot-1, unverifiable_run}` (E7.3).
- **Harness:**
  - 35 new seeded defects, 159 in total, each caught by its named test at the intended assertion (`harness/defect_reasons_b2.log`).
  - Four existing anchors that the new code repeated or changed were re-pointed at their original lines: `ended_sessions_forgotten`, `mandatory_policy_ignored`, `containment_without_a_record` and `admission_on_a_stale_clock`.
  - Inventory: 256 cargo tests (+15), 149 Python tests.

**Docs:**

- Storage formats §§2, 7 and the new §9.
- The service IPC rows: policyd `evaluate`, `reserve_use` and `network_mode`; registryd `restrict`, `reserve_use`, `complete_use`, `confirm_enforcement`, `session_event` and `history`.

**Author placements, open to review:**

1. **Reservation ids** are random, from registryd; the consumer's operation id is recorded only.
2. **registryd's refusal codes** come from the existing vocabulary: `lifecycle_rule` (grant, lifetime, emergency restriction), `subject_binding_mismatch`, `key_invalidated`, `stale_epoch`, `storage_unavailable` (no fresh sample), and the user-state codes.
3. **`use_reserved` is a registry event,** written only by `reserve_use`; `restrict` cannot write it.
4. **`consumed_uncertain` is written at every start,** a clean one included: no outcome arrived before the restart. An outcome reported after it is refused.
5. **A session end without bound grants** is a `session_event` end, which admission honours. This goes beyond the oracle's journal model: the published scenarios contain no such record, so the differential is unaffected.
6. **Enforcement domains** are `launcher` and the resource categories. A confirmation for a category the commit does not name is accepted. A contained or quarantined user is refused. Status is not part of `lineage_head` (§1 rule 7).
7. **Each lost evidence record** takes one `audit_gap` from the restrictive reserve.
8. **The contained start's gap** names the previous run's boot id.
9. **`identity_mismatch` is once per lineage, package identity and executable hash, ever** (not per boot). A runtime-scope-only mismatch is denied without a record.
10. **The latch covers every current expired grant of the lineage,** not only the requested one. It uses registryd's own trusted now; within the same second registryd may refuse (`lifecycle_rule`), and the next evaluation retries.
11. **`history`** leaves identity objects out; includes user-state records and events; is refused for a quarantined user. An unknown user has an empty history.
12. **Evidence is accepted for a contained user** (`complete_use`, session events); reservations and confirmations are refused.

## Verification (at 248ca6ef90d22e270db0e9a3f5bf789502d18720)

| Check | Environment / command | Result | Evidence / limits |
|---|---|---|---|
| Stage-B2 tests | `lifecycle_consumption` 11, `current_authorization` +4, `record.rs` round trips | **PASS** | in the qualification |
| Seeded defects | `test_m2_registry_policy.py` | **PASS**: 159 of 159 detected (35 new), each by its named test; the unmodified copy passes; each new defect, applied alone to a copy of the clean clone, exits 101 at its intended assertion | `harness/`, `harness/defect_reasons_b2.log` |
| Differentials | `admission_scenarios -- --nocapture`; `trusted_time_model -- --nocapture` | **PASS**: 15 scenarios, 72 steps, 121 comparisons, 0 disagreements; 165 cases, 4 813 steps, 0 disagreements | `admission_scenarios_nocapture.log`, `trusted_time_model_nocapture.log` |
| Full M2 qualification | Clean clone as UID 1001, pinned container (`2a0bcf342c57`), `qualify_m2_rust.sh`, `CHAM3_M2_REQUIRE_CLEAN=1` | **PASS** (`m2 qualification: PASS`): unit 256+256, inventory-exact; harness 149 in 497.7 s; provider PASS; ECR 19/96/18 with 27 scenarios / 365 steps; trusted-time vectors PASS; format differential 5 968 / 0 (corpus unchanged); vault oracle 38; both lock checks PASS; `dirty_tracked 0` | [VM evidence](../evidence/M2-cryptographic-contract-core/m2_006b2_amd64_vm_248ca6e/) |
| Static checks; static tests | same clone and container | **PASS**; 82 OK; clone clean afterwards | `qualification_console.txt` |
| Full-filesystem probe, five wrapper cases | 2 MiB tmpfs | **PASS**; reports byte-identical to 5ea2407's; 5/5 | `storage/` |
| Unchanged inputs | `git diff --exit-code f6f6883 248ca6e -- …` | no dependency, lock, pin, workflow, Dockerfile, script, oracle, vector, ADR or other-crate change; 5ea2407..f6f6883 docs-only | `unchanged_inputs.txt` |
| GitHub Actions at 248ca6e | m2 38657106670, m1 38657106555, m0 38657106631 | **success**, seven jobs and 88 steps, including the vault protection positive path; artifact 11938475946, digest `sha256:5d7adfc6…7dff` verified; contents agree with the author run | [CI evidence](../evidence/M2-cryptographic-contract-core/m2_006b2_amd64_ci_248ca6e/) |
| Real sessiond, launcherd, netd and brokers; runtime-id attribution; real crash tests; real hardware clocks; Miri/sanitizers/fuzzing | — | NOT_RUN | M2-007, M3, later milestones; storage formats §7 |

## Review

- **Requested:** a full independent review of stage B2 by Codex, using the prompt below. The record goes on a new branch, `review/m2-006-b2-security`, based on `main` (9cf8825), and holds one file.
- **Earlier records on this package:** stage A at 06a9423 and stage B1 at 3a978fb (both APPROVE). Neither covers B2.

## Remaining

- **Limits:**
  - The ADR 0007 §5.7 limits stand.
  - With many lost evidence records on a full filesystem, the gaps can deplete the restrictive reserve; restrictions then stay pending, denied in memory and in the ledger (ADR 0008 §1).
- **NOT_RUN:** real sessiond, launcherd, netd and brokers (fixtures only); runtime-id attribution (M3); real crash tests; real hardware clocks.
- **Stages C–D:** the ceremony, deletion, proposals, restore and delegation.
- **M2-007:** transport and processes.
- **Uncommitted state:** none after the docs commit.
- **Next:** Codex reviews B2. Findings come back as B2 revisions; an APPROVE opens stage C.

## Review prompt for Codex

~~~text
You are the independent REVIEWER (Codex) for cham3 M2-006 stage B2 in Kayyo321/cham3: a full security review of
one staged candidate (one-shot reservations, sessions, enforcement confirmations, audit gaps, identity mismatches,
the expiry latch and history). You did not author this work. You run on the owner's home computer; Docker worked
for you before; record BLOCKED only with the exact command and error.

0. Bring the local clone to the candidate FIRST:
     cd ~/cham3 && git status --porcelain && git stash list
     git fetch origin codex/m2-cryptographic-contract-core main
     git checkout codex/m2-cryptographic-contract-core && git reset --hard origin/codex/m2-cryptographic-contract-core
     git rev-parse HEAD        # expect the docs-only commit after 248ca6e (its parent must be 248ca6e)
     git log --oneline -5      # 16944ab -> ff9cb2c -> f6f6883 -> 248ca6e -> the docs-only commit
   Move unexpected local changes aside (git stash); never force-push or edit the implementation branch. Read every
   document from this checkout; make the clean clone for testing from it
   (git clone --no-hardlinks ~/cham3 <dir>; git -C <dir> checkout --detach 248ca6ef90d22e270db0e9a3f5bf789502d18720).

Read AGENTS.md; docs/decisions/0007_m2_lifecycle_time_consumption.md (the architecture: §§1-4, 6, 7 and erratum E7
after §7) and docs/decisions/0008_m2_retention_privacy_update_identity.md §2; docs/specs/m2_registry_storage_v1.md
§§2, 7 and 9 (the implementer's record — check it against the ADRs, not the other way round); the changed rows of
docs/specs/m2_service_ipc_v1.md; docs/handoffs/M2-006-stage-b2.md; the E7 record and the B2 plan in
docs/tasks/M2-cryptographic-contract-core.md. Treat author claims, including the twelve author placements of the
handoff, as claims to verify.

Targets: candidate 248ca6ef90d22e270db0e9a3f5bf789502d18720 (range f6f6883..248ca6e, one commit, 13 files) and the
docs-only commit after it. Confirm 5ea2407..f6f6883 is docs-only, that f6f6883 records E7 as the owner's answers
(wording only; make no owner decision), and that the range changes only core/registryd/src/{record,lineage,
registry}.rs, core/policyd/src/{evaluate,service,lib}.rs, core/registryd/tests/{lifecycle_consumption (new),
storage_integrity}.rs, core/policyd/tests/current_authorization.rs, the registry/policy harness module, the
inventory and two specifications — no dependency, Cargo.lock, pin, workflow, Dockerfile, script, oracle, vector,
ADR or other crate — and that the docs-only commit changes only docs/. Do not merge; do not edit implementation
files.

1. One-shot consumption (§4 option A, E7.1): policyd's reserve_use evaluates every §1 rule for the named grant
   (evaluate_grant, not_one_shot) before registryd is asked; registryd's re-check under the user lock (user state,
   nothing owed, a fresh sample, the persisted emergency restriction, subject, compromised key, epoch, lifetime with
   §8 option B, a bound session, current one-shot without terminal record or reservation); consumption in memory
   first and the restrictive write; restriction_not_durable leaves the grant consumed and the use denied; exactly
   one reservation under concurrency; roles (registryd's reserve_use for service:policyd only). Try a race between
   two reservations, a reservation while a restrictive write is pending or the anchor failed, a ledger failure, a
   reservation of a grant on a branch a later trunk commit ends, and whether a refusal anywhere writes or consumes.
2. Outcomes and reconciliation (§§4, 7): complete_use once, never after consumed_uncertain; consumed_uncertain at
   every start (clean, unclean same boot, new boot, contained), once; a reservation only in the ledger re-journaled
   then reported; replay rules of the new events (check_event) and their quarantine on a forged record.
3. Sessions (§6): a start recorded once, refused after an end; an end restrictive and final (session_ended with
   bound grants, a session_event end otherwise); admission refusing grants bound to an ended session (beyond the
   oracle's model); a repeated or pending end; contained users.
4. Confirmations (E7.2): live head, epoch in force, user state, the per-run status (never restored by replay; any
   restart pending), latest per domain, never an eligibility input (not in lineage_head; policyd unchanged).
5. Audit gaps (E7.3): which failures count as lost evidence (a write owed, a failed ledger or journal, an exhausted
   quota, a failed append) and which do not (a durable but unanchored record); the gap through the restrictive path
   and its journaling once storage recovers; no effect applied for lost evidence; the contained start's gap and the
   boot it names; the reserve consumed by many gaps (a stated limit).
6. identity_mismatch and the expiry latch (E7.4, §5 rule 5): policyd only; once per lineage, package identity and
   executable hash (durable or pending); not for the subject itself, an unknown lineage or a scope-only change; the
   latch for every current expired grant of the lineage, once; a latch refused within the same second; decisions
   unchanged by a failed report.
7. history (ADR 0008 §2): roles (own UID, owner_mismatch, admin, services refused), pages (≤16 records or one commit,
   cursor, identity objects left out), the served bodies against the journal, status, pending restrictions, policy
   state, refusal while unanchored and for a quarantined user, an unknown user.
8. Harness: the eleven lifecycle_consumption tests, the four policyd tests, the record unit test, the one changed
   expectation; tests/m2/harness/test_m2_registry_policy.py (159 seeded defects: 35 new, four anchors re-pointed) —
   judge whether each new defect's test discriminates for the right reason (harness/defect_reasons_b2.log).
9. Evidence: qualification in a clean clone at 248ca6ef90d22e270db0e9a3f5bf789502d18720 (CHAM3_M2_REQUIRE_CLEAN=1;
   nofile as before): unit 256+256, harness 149 (159 seeded defects), ECR 19/96/18 with 27 scenarios /
   365 steps, trusted-time vectors PASS, format 5 968 / 0, the rest as before; static checks and sh tests/run_tests.sh static; the full-filesystem
   wrapper and the five wrapper cases. Compare GitHub Actions m2_checks 38657106670, m1_checks 38657106555, m0_checks
   38657106631; download the artifact and check its digest
   (artifact 11938475946,
   sha256:5d7adfc6fbe4126ef1353b53b2488f5604d8fdc6cb60b557bdb1206f14b67dff).
10. Codex Security plugin on f6f6883..248ca6e; record name/version and findings.

Record every check as PASS/FAIL/NOT_RUN/BLOCKED with exact commands and evidence. Return APPROVE or
CHANGES_REQUESTED for M2-006 stage B2 and make no owner decisions (list questions for the owner separately).
Publish only your record on a new review branch (review/m2-006-b2-security) and end with:

=== CHAM3 M2-006 STAGE B2 VERDICT ===
reviewer: Codex
environment: <OS/arch; UID; Docker access: yes/no and how; image id; nofile limit used>
local_clone: <~/cham3 HEAD after step 0; local changes or stashes: none / moved aside (what)>
review_record: <branch> @ <commit> : <path> (sha256 <digest>)
candidate_reviewed: 248ca6ef90d22e270db0e9a3f5bf789502d18720
docs_commits_reviewed: <shas> (docs-only: yes/no)
scope_exact: <the listed paths and nothing else: yes/no>
verdict: M2-006 stage B2: <APPROVE/CHANGES_REQUESTED>
reservations: <PASS/FAIL> (<..>)
outcomes_and_reconciliation: <PASS/FAIL> (<..>)
sessions: <PASS/FAIL> (<..>)
confirmations: <PASS/FAIL> (<..>)
audit_gaps: <PASS/FAIL> (<..>)
mismatch_and_latch: <PASS/FAIL> (<..>)
history: <PASS/FAIL> (<..>)
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
