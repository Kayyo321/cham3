# Handoff: M2-005 — ECR admission and current authorization

- **State:** M2-005 is **IMPLEMENTED → IN_REVIEW**. Its independent review is **deferred by the owner** (instruction of 2026-10-13); it is required before M2-005 is DONE. The author approves nothing.
- **Role:** IMPLEMENTER. **Model/provider:** Claude Code (Anthropic), cloud VM. Requested reviewer: Codex (independent).
- **Branch:** `codex/m2-cryptographic-contract-core` (pushed fast-forward).
- **Task specification:** [M2 task](../tasks/M2-cryptographic-contract-core.md), M2-005 (A06, A07, A08 in part), with the scope recorded before the work (commit 6968bd6); [ADR 0006 §§1–2](../decisions/0006_m2_storage_anchor_recovery.md) (G4), [ADR 0007 §1](../decisions/0007_m2_lifecycle_time_consumption.md) (G5), [wire format v1](../specs/m2_wire_format_v1.md) §§2.3, 6.5–6.12, 7, [service IPC v1](../specs/m2_service_ipc_v1.md) §3.
- **Base:** 6968bd6f8c5fa2701918686164ea50910a8449b8 (the scope record), on M2-004 revision 2 (ca40c62, docs acefb66), **whose re-review is still pending**. An M2-004 finding may require M2-005 rework.
- **Candidate:** **b06d7cb** (full hash in the verification table). The range 6968bd6..b06d7cb has two commits:
  - the implementation 6e9d6d3, whose qualification failed only on rustfmt (one test file formatted after the last format pass);
  - the fix b06d7cb: that formatting, and the signing-key window at admission time (decision 12) with its test and seeded defect.
- **Docs-only follow-up:** the commit adding this handoff, the evidence, the task progress and STATUS.

## Completed work

### registryd (`core/registryd`, safe Rust)

| Module | What it does |
|---|---|
| `record` | Journal frames and records (ADR 0006 §1). File: header `"C3JH" ‖ u16_be(1) ‖ u32_be(uid) ‖ user_id` (42 bytes), then frames `"C3JR" ‖ u32_be(length) ‖ record ‖ chain`, with `chain_n = domain_hash("cham3/journal-chain/v1", chain_{n−1} ‖ u64_be(n) ‖ record)` and `chain_{−1} = domain_hash("cham3/journal-genesis/v1", user_id ‖ u32_be(uid))`. A record is one closed `cham3_dcbor_v1` map of at most 65 536 bytes with `kind`, `seq` (1-based, `n + 1`), `time` and exactly its kind's fields: `identity` (`object_kind`, `object`), `commit_admitted` (`commit_id`, `lineage_id`, `branch`, `envelope`), `expired` (`grant_id`), `session_ended` (`session_id`), `restriction` (`grant_ids`, ascending, 1–64). Any other kind fails replay. |
| `journal` | `<root>/users/<uid>/journal.c3j`; directories `0700` and the file `0600`, owned by registryd's UID; wrong owner, mode, type, a symbolic link or a non-canonical UID entry is refused, and the opened file must be the inode that was checked. An append writes one frame and returns after `fdatasync`; creation also syncs the user directory (and `users` when new). A failed append leaves the journal refusing every later append in the run. `journal_io_t` is the fault-injection boundary. |
| `lineage` | One user's ECR index and **layer 7** (pure; nothing changes until `apply_commit` after the record is durable): parents (`parent_unknown`, `lineage_mismatch`), placement (wire §6.11, `stale_head`), base and result state (`base_state_mismatch`, `delta_rule`, `result_state_mismatch`), uniqueness of grant ids within the user's journal and of request ids per user (`duplicate_identifier`), restore (§6.7, persistent history only, `restore_rule`), subject binding (§6.9), branch revocation scope (`branch_rule`) and the lineage network slot across the trunk head and every live temporary head (§6.12, `network_grant_conflict`). Lifecycle records are validated (`lifecycle_rule`) and end grant ids permanently. Standing of a grant: current / ended / inactive / unknown. |
| `registry` | Per-user slots: the journal, the key-authority index rebuilt from the same journal's identity records (identityd's own code, so layer 6 at admission reads the commit's snapshot) and the ECR index. Operations with role checks: `append_identity`, `identity_state`, `journal_users` (`service:identityd`), `admit` (`service:contractd`), `lineage_head` (`service:contractd`, `service:policyd`). `admit` re-verifies layers 1–6, applies layer 7, requires the signing key's certificate window to contain registry time (decision 12) and `policy.epoch` = the current epoch (`stale_epoch`), checks the lineage limit and the byte quota (the 1 MiB reserve is for restrictive records only: `quota_exhausted`), all under the user lock, then appends durably and admits `pending`. At start every journal is parsed, chain-verified and replayed through the same checks (except the current epoch and quotas, which are admission-time facts). `registry_identity_store_t` implements identityd's journal interface, so identityd runs over the real store. |

### policyd (`core/policyd`, safe Rust)

| Module | What it does |
|---|---|
| `mandatory` | The M2 configuration model of mandatory policy: prohibitions (category, optional resource id, operation and owner), forbidden network modes, the global and per-user emergency restriction. Not an administrator policy language. |
| `evaluate` | **Layer 8** as pure functions (ADR 0007 §1). `evaluate`: mandatory prohibition first; user state from the snapshot and from identityd; emergency restriction; the lineage and its authoritative subject equal to the runtime identity (all subject fields, `unknown_identity`); same-owner resources only (`resource_ownership`); the matching grants of the current state (persistent grants of the trunk head, temporary-lifetime grants of live temporary heads), each checked for an ending lifecycle record, its origin commit's signing key not invalidated as `compromised` (in the snapshot **or** in identityd's current answer), the origin's policy epoch at least the minimum valid epoch, `effective_start ≤ now < expiration`, a session grant only in that session with a live attestation at most 5 s old, a one-shot grant without a reservation; delegated grants are refused. `network_mode`: the operation of the single eligible network grant; Offline with none, when mandatory policy forbids the mode, and (unknown) when more than one is eligible. |
| `service` | `evaluate`, `network_mode` (`service:*`, `fixture_consumer`), `policy_status` (`service:*`, `trusted_client`). Mandatory prohibitions and the emergency overlay deny before any snapshot is read. Each evaluation reads one `lineage_head` snapshot and asks identityd for every origin key; anything that cannot be read is `unknown` (denied) or Offline. |

### contractd (`core/contractd`, safe Rust)

`submit_signed` (owner's `trusted_client`): layers 1–5 with the key identityd resolves for the envelope's `kid`, `actor.uid` = the caller's UID (`owner_mismatch`), layer 6 and the signing scope with that resolution, then registryd `admit`. An identityd that cannot answer fails closed (`storage_unavailable`). `verify_object` (`service:*`, `trusted_client`) reports layers 1–5, and layer 6 for commits; it never admits or authorizes.

### Shared changes

- `cham3_contract_format::dcbor::decode_container`: the same strict profile with the wire format §2.3 container limit (65 536 bytes) for journal records and IPC messages; byte strings keep the 61 440-byte limit; tag 18 is refused. `decode` is unchanged.
- `cham3_identityd::outcome::rule_code_t` gains the layer-7 codes; `rule`, `authority` and `ipc` become public.
- Cargo.lock gains only dependency edges between workspace crates (and dev-dependency edges to the pinned `ed25519-dalek`); no new crate or version.

### Tests and evidence tooling

- **Rust: 157 + 157 cargo tests** (debug and release), inventory-exact; 36 new:
  - registryd unit 3 (records closed and round-tripping; frames, chain and every truncation; canonical UID names);
  - **`admission_scenarios`**: every `key_authority: false` journal scenario of the published vectors (**10 scenarios, 53 steps, 90 comparisons, 0 disagreements**), the 18 admission-invalid vectors after their lineage prefix, and the published lineage (8 commits, then their replay refused);
  - **`registry_operations`** 15: restart replays identically (and identityd rebuilds over it); the file holds exactly the chained records; tampering and torn tails make only that user unusable; correctly chained but invalid records fail replay at their sequence (placement, a commit twice, a missing parent, a bad signature, a wrong sequence, a commit before certificates, a non-enrollment first record, a premature expiry); replayed lifecycle records end grants and free the network slot; write/torn/sync failures change nothing and deny until restart (then absent / unusable / present-and-pending); roles; stale epochs, the lineage limit and unknown users; duplicate request and grant ids across lineages; the byte quota and its restrictive reserve; 16 rounds of two conflicting writers (exactly one wins, the other `stale_head`, a third user's writer unaffected, the journal replays to the same head); `identity_state` paging; unsafe paths; delegated grants refused; the signing key's window at registry time (before `not_before`, at the inclusive end, after the temporary `not_after`, a persistent key without one, replay after the window);
  - policyd unit 1 and **`current_authorization`** 8: every rule above over the real composition, mandatory policy and the emergency overlay, the minimum epoch, compromised versus rotated keys, sessions, network mode, unavailable inputs, and pure-function cases admission makes unreachable (two eligible network grants, orphaned grants, identityd-only compromise or containment, a reserved one-shot, a delegated grant);
  - **`end_to_end`**: vaultd creates a user's keys (through identityd into the journal) and signs an exactly approved proposal, contractd submits it, registryd admits it durably, policyd evaluates it eligible; after restarting every service it is still eligible; two proposals against the same head: the second is `stale_head`; a compromise reported through the vault ends the grant;
  - contractd **`submission`** 3; contract_format unit 1 (container limit).
- **Harness: 135 tests**: `test_m2_registry_policy.py` with **28 seeded defects** (ended-branch extension, base/result state, grant and request id reuse, restore of temporary history, unbound subject, unbounded branch revocation, per-state network slot, premature expiry, delegated grants, the key window at admission, epoch, quota reserve, admit role, replayed placement, a forgotten append failure, the chain check, mandatory policy, expiration, session age, compromised and identityd-only compromise, the epoch floor, the runtime identity, network conflict by order, identityd user state, the submission owner), each caught by its named Rust test while the unmodified copy passes; 133 carried.

### Decisions made within the approved design (for review)

1. The journal header (42 bytes) and the record encodings above are this implementation's reading of ADR 0006 §1 ("header + enrollment record") and of the published sizing record (`limits.max_envelope_commit_admitted_record_bytes`). The chain index `n` is 0-based; the `seq` field and every reported journal sequence are 1-based.
2. **Fail closed on any journal defect, and no recovery yet:** a defect at start (header, frame, chain, torn tail, a record that does not decode or re-verify) makes the user unusable for the run; a failed append makes the user unusable until restart; a torn tail stays unusable after restart. The decision table, truncation and the anchor are M2-006.
3. Grant ids are unique within the user's journal, not only the lineage (the oracle's rule, stricter than wire §6.6's wording).
4. **Interim restriction:** a grant with `delegation.parent_grant_id` is refused at admission (`admission/delegation_rule`) and, if one were ever present, denied by evaluation, until M2-006 implements wire §6.4.
5. The current policy epoch and the minimum valid epoch are registry configuration in M2-005. Their raises are restrictive changes with the restrictive write path (M2-006).
6. A resource owned by another UID is denied (`resource_ownership`): M2 has no cross-user resource authorization.
7. A session-bound grant requires the runtime's session to be the grant's session **and** a live attestation at most 5 s old; an attestation from the future denies.
8. When several grants match, the first eligible one (by grant id) is reported; when none is eligible, the first grant's refusal.
9. policyd asks identityd for every origin key in addition to the registry snapshot, so a compromise or user state identityd holds but the journal does not yet hold denies; identityd being unable to answer is `unknown`.
10. Admission and replay share one code path for layers 1–7; the recorded `commit_id`, `lineage_id` and `branch` must equal the recomputed ones.
11. A user directory without a journal file is no user (a creation that never completed its first durable record); a partial journal file is a defect (decision 2).
12. **(beyond the model)** `admit` also requires the signing key's certificate window to contain registry time now (inclusive; `key_authority/certificate_window`), not only the commit's own timestamp. An envelope signed inside a temporary key's window can therefore not be admitted after the window closed. Admission-time only: replay reproduces historical admissions. The ±300 s commit-timestamp check remains M2-006.

### Not in M2-005 (recorded sequencing, before the work)

- **M2-006:** anchor, run marker, restriction ledger, torn-tail and crash recovery, quarantine; containment, `recover_user` and the recovery ceremony operations, and `recovery_rule` at admission; the operations that write lifecycle records (`restrict`, the expiry latch, `session_event`); delegation admission; `reserve_use`/`complete_use`, `confirm_enforcement`, `history`, `time_sample`; the trusted-time service clock and the ±300 s commit-timestamp check; ADR 0007 §8 Option B (observed-time start eligibility), which also adds the trusted and observed admission times to the `commit_admitted` record (a format extension before any deployment; M2-005 evaluation checks `effective_start` against trusted now only); `set_policy_epoch`, `set_emergency_restriction`, `check_proposal`; contractd `prepare_proposal` (the M2-004 handoff listed it under M2-005; the M2-005 scope record moves it to M2-006 with forward-restore and the recovery proposals) and `request_revocation`; `replace_identity`; vault-side deletion cleanup.
- **M2-007:** sockets, processes, the setgid client, systemd units.
- **Open owner question (no change made):** no error code is assigned to the ±300 s commit-timestamp check of ADR 0007 §5.

## Verification (at b06d7cb)

| Check | Environment/command | Result | Evidence/limits |
|---|---|---|---|
| Author amd64 qualification at b06d7cb21e1e3e42bbcd025abd9cea131574751d | Clean UID-1000 clone, pinned container, `qualify_m2_rust.sh` with `CHAM3_M2_REQUIRE_CLEAN=1`; no `--privileged`, no added capabilities, no `--record` | PASS. Unit **157 + 157** cargo tests, inventory-exact. Provider oracle 7/7. ECR 19/96/18 (22 scenarios / 346 steps). Format differential 5 950 cases / 0 mismatches. vault_object_oracle 38/38. Harness **135** (28 seeded registry/policy defects and 20 identity/vault defects detected; protection refusal) | [m2_005_amd64_vm_b06d7cb](../evidence/M2-cryptographic-contract-core/m2_005_amd64_vm_b06d7cb/) |
| Layer-7 differential | `admission_scenarios` against every `key_authority: false` journal scenario, the admission-invalid vectors and the published lineage | PASS: 10 scenarios, 53 steps, 90 comparisons, 0 disagreements; 18/18 admission-invalid vectors; 8/8 lineage commits, their replay refused | `admission_scenarios_nocapture.log`; harness seeded defects |
| Static checks / static tests | Same clone and container: `run_static_checks.sh`, `tests/run_tests.sh static` | PASS / 82 OK; the clone was clean afterwards | `static_checks.log`, `static_tests.log` |
| Unchanged inputs | `git diff --exit-code 6968bd6 b06d7cb` per path | PASS: 29 paths unchanged (manifests, `.cargo`, CI, build, PROJECT, docs, vectors, oracles, harness library, vaultd, native crates, probes, identityd except `outcome.rs`, contract_format except `dcbor.rs`); Cargo.lock: dependency edges only | `unchanged_inputs.txt` |
| Vault protection, positive path | Pinned container with `--ulimit memlock=1073741824:1073741824` | **PASS in CI** (m2_checks 38404018473): non-dumpable, core limit 0, 0 unlocked mappings, release derivation 420 ms. BLOCKED in this VM (memlock cannot be raised) | [CI evidence](../evidence/M2-cryptographic-contract-core/m2_005_amd64_ci_b06d7cb/) |
| GitHub Actions at b06d7cb | m2 38404018473, m1 38404018403, m0 38404018497 | PASS, every job and step; artifact `sha256:c9cfe469…01b5` | CI evidence; the artifact ZIP was not downloaded by the author |
| First attempt 6e9d6d3 | Same gates | FAIL: rustfmt only (author run and m2_checks 38403053054); m1/m0 success. Fixed by b06d7cb | `attempts.md` |
| Miri, sanitizers, coverage-guided fuzzing | — | NOT_RUN (NOT_AVAILABLE on the pinned toolchain) | Bounded seeded and differential tests only |
| Anchor, crash recovery, lifecycle writers, trusted time, IPC, processes | — | NOT_RUN | M2-006, M2-007 |

## Remaining

- **Known risks and limits:**
  - in-process composition only; the concurrency test uses threads in one process, not separate service processes;
  - no anchor: a restored or rolled-back store is not detected (A10 is M2-006); a torn tail or failed append denies the user until M2-006's recovery exists;
  - the policy epochs and the mandatory policy are start-time configuration;
  - registry time is the system clock; trusted time and the commit-timestamp check are M2-006;
  - lifecycle records are replayed but not yet written by any operation;
  - built on M2-004 revision 2, whose re-review is pending;
  - Miri, sanitizers and fuzzing are NOT_RUN.
- **Uncommitted state:** none after the evidence commit.
- **Next:**
  1. When the owner is back: Codex re-reviews M2-004 revision 2, then reviews M2-005 (or both together, as the owner decides).
  2. M2-006 continues under the owner's instruction, marked as needing review.
- **Reviewer environment (owner note, 2026-10-13):** the owner reports that the Codex cloud environment now has **unrestricted internet domain access**, so every earlier blocker should be gone. The items earlier reviews recorded as BLOCKED should now run: the GitHub CLI artifact download and API access (`gh run download`), the Codex Security plugin skill's preflight resources and backend, and upstream retrievals such as the Debian tracker and RustSec. The review prompt below asks the reviewer to run them and to record BLOCKED only with the exact error. This applies to the pending M2-004 revision-2 re-review as well. The author has not verified the environment.
- **Reminder for the end of M2:** the final review uses the Codex Security plugin skill. It was BLOCKED in every reviewer environment so far; with the new network access it should run in these reviews already.

## Review prompt for Codex

~~~text
You are the independent REVIEWER (Codex) for cham3 M2-005 in Kayyo321/cham3. You did not author this work.
Prerequisite: M2-005 is built on M2-004 revision 2 (candidate ca40c62, docs acefb66), whose re-review is still pending
(docs/handoffs/M2-004-rereview-r2.md, verdict block CHAM3 M2-004 R2 VERDICT). Review M2-004 revision 2 first, or record
how you combined the two reviews; an M2-004 finding that M2-005 depends on is a finding here too.
Environment: the owner reports that your Codex cloud environment now has unrestricted internet domain access (2026-10-13),
so the items earlier reviews recorded as BLOCKED (gh run download of CI artifacts, GitHub API access, the Codex Security
plugin skill's preflight resources and backend, upstream retrievals) are expected to run in this review and in the
M2-004 revision-2 re-review. Run them; record BLOCKED only with the exact command and error.
Read AGENTS.md; docs/tasks/M2-cryptographic-contract-core.md (M2-005, A06, A07, A08, and the section "Owner instruction:
deferred reviews, M2-005 started (2026-10-13)", which records the scope before the work); ADR 0006 sections 1-2;
ADR 0007 section 1; docs/specs/m2_wire_format_v1.md sections 2.3, 6.5-6.12 and 7; docs/specs/m2_service_ipc_v1.md
section 3 (registryd append_identity, identity_state, admit, lineage_head; policyd; contractd); and
docs/handoffs/M2-005-admission-authorization.md. Treat author claims and evidence as claims to verify; judge against
the approved ADRs and specifications.

Targets: candidate b06d7cb21e1e3e42bbcd025abd9cea131574751d (range 6968bd6f8c5fa2701918686164ea50910a8449b8..b06d7cb: the implementation
6e9d6d3 and the fix b06d7cb) and the docs-only commit after it on origin/codex/m2-cryptographic-contract-core;
confirm that commit changes only docs/. Do not merge main or review branches into the implementation branch; do not
edit implementation files.

1. Scope and boundaries: registryd, policyd and contractd are safe Rust with #![forbid(unsafe_code)]; Cargo.lock gains
   only dependency edges (no new crate or version); the shared changes (contract_format decode_container, identityd
   rule_code_t) leave M2-003 and M2-004 behavior unchanged. Judge the recorded sequencing (the "Not in M2-005" list,
   including prepare_proposal moving to M2-006) against the task file.
2. Journal (core/registryd/src/record.rs, journal.rs): frame, chain, header, record encodings and the container
   budget against ADR 0006 section 1; durability (fdatasync, directory fsync) and path checks; replay
   (registry.rs replay_user) re-verifies every record through layers 1-7 and fails closed. Try crafted, correctly
   chained journals that should not replay. Judge decisions 1, 2, 10 and 11.
3. Layer 7 (core/registryd/src/lineage.rs): check order and every rule against wire format 6.5-6.12 and 7 and the
   oracle's journal_t; the differential core/registryd/tests/admission_scenarios.rs (check that it compares what it
   claims and cannot pass vacuously); the network slot across live branches; lifecycle-record validation; decisions 3
   and 4. Look for admission sequences beyond the vectors that admit authority the oracle would refuse.
4. Admission service (registry.rs admit): layers 1-6 against the same journal snapshot, layer 7, the signing key's
   window at registry time, the current epoch, the lineage and byte quotas with the restrictive reserve, all under the
   user lock, durable before the index changes; the identity store identityd runs over (append_identity re-validation,
   identity_state paging); roles; lock order and concurrency. Try to make two conflicting writers both win, or a failed
   append leave state behind.
5. Current authorization (core/policyd/src/evaluate.rs, service.rs): ADR 0007 section 1 rules 1-7 and network mode;
   decisions 5-9. Look for any input that yields eligible for a grant that is ended, expired, not started, on an ended
   branch, session-unbound, signed by a compromised key, below the epoch floor, for another subject or owner, delegated,
   or prohibited by mandatory policy, and check that unreadable inputs never become eligible.
6. contractd (core/contractd/src/submit.rs): layer order, the owner binding, layer 6 before admission, fail closed when
   identityd cannot answer; verify_object admits nothing.
7. Evidence: in a clean clone at b06d7cb owned by an unprivileged UID, build the pinned image and run the
   qualification (docker run --user "$(id -u):$(id -g)" ... bash build/scripts/qualify_m2_rust.sh with
   CHAM3_M2_REQUIRE_CLEAN=1; no --privileged, no added capabilities, no --record) and confirm: unit 157+157
   cargo tests, provider oracle 7/7, ECR oracle 19/96/18 with 22 scenarios / 346 steps, format_differential 5950 cases
   / 0 mismatches, vault_object_oracle 38 checks PASS, harness 135 (including 20 identity/vault and 28
   registry/policy seeded defects); then build/scripts/run_static_checks.sh and sh tests/run_tests.sh static. If your
   Docker allows it, run the protection positive path (tests/m2/protection/check_vault_protection.sh with
   --ulimit memlock=1073741824:1073741824); otherwise record BLOCKED and compare the CI step. Write and run your own
   adversarial tests (crafted journals, admission sequences, concurrent writers, evaluation edge cases); record exact
   commands and results.
8. Compare GitHub Actions m2_checks 38404018473, m1_checks 38404018403 and m0_checks 38404018497 for b06d7cb, including the
   "Vault process protection, positive path" step. Download the artifact (gh run download 38404018473 -n
   m2_qualification_amd64), check its digest (sha256:c9cfe469bbc109faff6fe7a82a8eccbe1bb7a4f9f7073292e83b5c21b13301b5) and
   compare its summaries with your own run.
9. Run the Codex Security plugin skill on the candidate range and record its exact name/version, the commands it ran
   and every finding (it remains required for the final M2-008 review). Record BLOCKED only with the exact error.

Record every check as PASS/FAIL/NOT_RUN/BLOCKED with exact commands, environment and evidence. Return APPROVE or
CHANGES_REQUESTED for M2-005; make no owner decisions. Publish only your review record on a new review branch
(review/m2-005-security), and end your reply with this block filled in:

=== CHAM3 M2-005 VERDICT ===
reviewer: Codex
environment: <machine/OS/arch; UID; Docker version; network notes>
review_record: <branch> @ <commit> : <path> (sha256 <digest>)
candidate_reviewed: b06d7cb21e1e3e42bbcd025abd9cea131574751d
docs_commits_reviewed: <shas> (docs-only: yes/no)
m2_004_r2_reviewed_first: <yes, record <branch@commit> / combined (how) / no (why)>
verdict: M2-005: <APPROVE/CHANGES_REQUESTED>
scope_and_boundaries: <PASS/FAIL> (<notes>)
journal: <PASS/FAIL> (<format, durability, paths, replay>)
admission_layer_7: <PASS/FAIL> (<rules, differential, network slot, lifecycle records>)
admission_service: <PASS/FAIL> (<same snapshot, key window, epoch, quotas, lock, identity store, roles>)
current_authorization: <PASS/FAIL> (<rules 1-7, network mode, unknown inputs>)
contractd: <PASS/FAIL>
author_decisions: <1-12 accepted / objections>
qualification_amd64: <PASS/FAIL/BLOCKED> (<counts>)
protection_positive_path: <PASS/FAIL/BLOCKED> (<own run or CI step>)
static_checks: <..>   static_tests: <..>
own_tests: <what you ran and results>
security_plugin: <ran (name/version, findings) / BLOCKED (reason)>
ci_comparison: <..>
new_findings:
- id: <..> | severity: <..> | location: <file:line> | summary: <..> | required: <..>
notes_for_owner: <..>
=== END VERDICT ===
~~~
