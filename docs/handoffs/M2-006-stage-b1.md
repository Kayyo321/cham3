# Handoff: M2-006 stage B1 — trusted time, observed start, commit-timestamp rule

- **State:** M2-006 is **IN_PROGRESS**. Stage A is DONE (review 06a9423 APPROVE). Stage **B1 is IMPLEMENTED → IN_REVIEW**. B2, C and D are not started. The author approves nothing.
- **Role:** IMPLEMENTER. **Model/provider:** Claude Code (Anthropic), on the owner's home computer. Requested reviewer: Codex (independent, same computer).
- **Working directory / branch:** `codex/m2-cryptographic-contract-core`, pushed fast-forward. No merge of `main` or of any review branch.
- **Task specification:** [M2 task](../tasks/M2-cryptographic-contract-core.md). The M2-006 scope record (2026-10-13) and the stage B split (owner, 2026-10-14) define **stage B1**: ADR 0007 §5 and §8 option B, errata E4 and E6.
  - registryd's trusted-time clock, its floors in the anchor and the ledger, `clock_anomaly`, `time_sample` and the offsets in `lineage_head`.
  - policyd's offset clock.
  - The observed-time counter and its start rule.
  - The ±300 s commit-timestamp rule `timestamp_rule`.
  - Registry record time from trusted time.
  - The end of every live temporary branch at a new boot.
- **Owner decisions used:**
  - ADR 0007 §8 **Option B** (2026-10-12); forward-error recovery deferred ("defer; never lower").
  - Erratum **E4** (2026-10-13): `timestamp_rule`.
  - Erratum **E6** (2026-10-14, recorded before the work): the floor in the anchor; `clock_anomaly` per user, once per episode.
  - The stage B split (2026-10-14).
- **Base commit:** e7803efaeb346ac94e12aca6ce32e2fabaa3cadd. The commits since the stage-A candidate db5bb59 are docs-only: 84464cc (stage-A evidence), 9d13864 (stage A DONE), 68ca252 (E5 confirmed; stage B split), e7803ef (E6).
- **Stage-B1 candidate:** **c9129bca0e6c4b1e6caf72774e05a27e09a4d542**, one commit (range e7803ef..c9129bc).
- **Docs-only follow-up:** the commit after c9129bc, whose parent must be c9129bc. It holds this handoff, the evidence, the task record and STATUS.

Use `PROJECT.md` and the approved ADRs as product-authority references. The formats are recorded in [registryd storage formats v1](../specs/m2_registry_storage_v1.md), §§2–4 and the new §8; it is an implementation record, and ADR 0007 controls.

## Completed work

`git diff --stat e7803ef c9129bc`: 34 files changed, 5 of them added.

**Code changes:**

- **`core/registryd/src/clock.rs` (new): the clock of ADR 0007 §5.**
  - Trusted now is `boottime + offset`. The offset is the largest `realtime − boottime` accepted in this boot.
  - A forward sample rebases it, but only after the caller has persisted the raise. A sample behind changes nothing.
  - A sample more than 300 s behind opens an anomaly episode, which ends at the first sample within 300 s.
  - The observed counter (option B) is `boottime + observed_offset`.
  - `start` resumes from the largest offsets of this boot's floors. With only earlier-boot floors, it starts no earlier than their trusted and observed values.
  - `merge_floor` never lowers a floor of the same boot.
  - `timestamp_acceptable` checks the two clauses of E4.
  - `kernel_time_source_t` reads CLOCK_BOOTTIME from `/proc/uptime`, between two reads, and CLOCK_REALTIME from the standard library.
- **`anchor.rs`:** format version 2 adds the floor `{boot_id, boottime, trusted, observed}` after the tombstones.
- **`ledger.rs`:** entry kind 3 `time`, whose payload is a floor.
- **`record.rs`:**
  - `commit_admitted.observed` is the observed time at admission. The record's `time` is the trusted time at admission.
  - New registry events: `clock_anomaly {boot_id, trusted, behind_seconds}` and `branch_terminated {boot_id}`.
- **`lineage.rs`:**
  - `check_commit(…, trusted_now)` checks `timestamp_rule` after the placement and before the state checks. Replay passes `None`.
  - The admission time is kept per commit.
  - New functions: `end_branches` and `has_live_branches`.
- **`registry.rs`, at start:**
  - The clock starts from the anchor's floor and, on a `ledger_replayed` start, from the ledger's time entries.
  - The start is refused when the clocks cannot be sampled (`clock_unavailable`).
  - After a reboot (the run marker names another boot) on a run that is not contained, each user's live temporary branches end in memory, and a `branch_terminated` record goes through the restrictive path.
- **`registry.rs`, per operation:**
  - Every IPC operation calls `observe_time` right after its role check and the stage-A retry.
  - An offset increase is appended to the ledger before the clock adopts it. A raise that cannot be ledgered is not adopted, and the failed append makes the run unverifiable.
  - The floor is refreshed in the anchor when it is 60 s old, or from another boot.
  - The user-scoped operations (`lineage_head`, `admit`, `restrict`, `append_identity`, `identity_state`) record `clock_anomaly` once per episode for their user, through the restrictive path.
- **`registry.rs`, writes:**
  - Every anchor write publishes the merged floor.
  - `admit` takes trusted now from its own sample (`storage_unavailable` otherwise). It checks `timestamp_rule`, then the signing window at trusted now, and records `observed`.
  - Restrictive records use the last trusted value, which is a lower bound.
- **`registry.rs`, new operations and reports:**
  - `time_sample` (`service:policyd`) ledgers before it adopts.
  - `lineage_head` carries `time` (the boot id and the two offsets) and each origin's `admitted` time.
  - `rollback_status` carries `clock` (boot id, trusted and observed now, the episode).
- **`core/policyd/src/service.rs`:**
  - `policy_clock_t` replaces the old trait: its own source, its boot id and its own offset, which only ever rises.
  - Evaluation uses `max(own offset, snapshot offset)`. A sample ahead of that is reported through `time_sample`, and adopted only after `ok`.
  - Another boot's snapshot, or a failed sample, is `unknown(clock_unavailable)`.
  - The observed counter comes from the snapshot's offset.
- **`core/policyd/src/evaluate.rs` (the option-B start rule):** a start later than its commit's admission also needs `observed_now − admitted.observed ≥ start − admitted.trusted`.
- **`core/identityd/src/outcome.rs`:** the layer-7 code `timestamp_rule`.

**Oracles and vectors:**

- **`tests/m2/oracle/trusted_time_vectors.py` (new)** generates `tests/m2/vectors/trusted_time_v1/vectors.json` from `trusted_time_model.py`, unchanged except its docstrings.
  - The vectors hold 165 cases and 4 813 steps: the named reproductions plus 160 seeded random sequences of samples, adopted offsets, floors, and restarts in the same boot and in a new boot.
  - `--check` regenerates the file and compares it byte for byte. `run_m2_tests.sh` runs it as `trusted_time_vectors`.
- **`ecr_oracle.py`:** `admit_commit(data, now)` and a step `now` in the scenarios. There are four new E4 scenarios: both bounds, the parent clause, the rule after `stale_head`, and the rule before the state checks. That makes 27 scenarios and 365 steps; every earlier vector and scenario is byte-identical. The vectors readme and wire format §7 are updated.

**Tests:**

- **`core/registryd/tests/trusted_time.rs` (new, 12 tests):**
  - rebase and no going back
  - an unledgered raise is not relied on
  - a same-boot restart from the ledger and from the anchor floor
  - a new boot from the last floor, including observed time
  - a new boot ends branches durably and only once
  - the 60 s floor refresh
  - `timestamp_rule`: both clauses, and nothing written on refusal
  - the admission's trusted and observed time, kept across replay
  - anomalies once per user per episode, at the 300/301 s boundary
  - `time_sample`: roles, another boot, a boottime ahead, lower and higher offsets, a ledger failure
  - a clock that cannot be sampled fails admission but never a restriction
  - a start without a clock is refused
- **`core/registryd/tests/trusted_time_model.rs` (new):** the clock against the vectors; 0 disagreements.
- **Unit tests:** clock 7 (including the kernel source), anchor 1, ledger 1. `record.rs` round trips cover the new records.
- **policyd, 3 tests:**
  - A forward clock error after admission does not advance a future start: refused at a trusted time past the start, eligible after the observed time has passed.
  - policyd reports a forward sample and relies on it only after registryd took it.
  - A snapshot of another boot is unknown.
- **Existing tests now use a settable shared clock:**
  - The test builders gain `test_time_t`.
  - Tests that moved "now" backward were restructured; in particular, commits made after time passes carry a later timestamp.
  - The signing-window test now gives each commit a timestamp within 300 s of trusted now.
  - The quota test invalidates keys until the reserve is used, since its old final assertion depended on record sizes.
  - Count assertions in the differential and the vector tests are updated.
- **Harness:**
  - 22 new seeded defects, 119 in total, each caught by the named test; the 12 stage-A anchors that the edits moved were re-pointed.
  - `test_m2_trusted_time` gains 2 tests: the published vectors are the model's, and a changed vector is detected.
  - `test_m2_ecr_oracle` accepts the finding `E4`.
  - Inventory: 234 cargo tests (+25) and 149 Python tests (+2).

**Author placements, open to review:**

1. **CLOCK_BOOTTIME source:** `/proc/uptime`, whose kernel source is the boot-time clock including suspend. Whole seconds; the realtime sample is paired with the later of two uptime reads. The alternative, rustix's `time` feature, would change ADR 0003's dependency record.
2. **Floor merge:** within one boot, the largest boottime with the largest offset and observed offset, so concurrent writers cannot lower the floor. A new boot's floor replaces an earlier boot's.
3. **Raises are ledgered before adoption:** an increase that cannot be ledgered (a failed ledger) is never adopted. Trusted time then advances at BOOTTIME pace only. No eligible decision is served in that state anyway.
4. **Anomaly records** go through the restrictive path. A pending one blocks the user's admissions until it is durable, as any pending restrictive record does.
5. **Clock failure, admission:** admission needs a fresh sample. A stale value could admit a commit after its signing key's window closed.
6. **Clock failure, restrictions:** restrictions take the last trusted value. Trusted time never decreases, so an expiration that value already shows is proven.
7. **`time_sample` refusals:** a `boottime` more than 1 s ahead of registryd's own sample is `malformed_request`.
8. **New boot** is detected from the run marker's boot id. A contained run does not need it, because containment already ends every branch.
9. **Journals and anchor slots from before B1** fail to parse: `commit_admitted` without `observed`, and anchor version 1. No store is deployed.

## Verification (at c9129bca0e6c4b1e6caf72774e05a27e09a4d542)

| Check | Environment / command | Result | Evidence / limits |
|---|---|---|---|
| Stage-B1 tests | `trusted_time` 12, `trusted_time_model` 1, clock/anchor/ledger unit tests 9, policyd 3 | **PASS** | in the qualification |
| Clock against the model | `cargo test -p cham3_registryd --test trusted_time_model -- --nocapture` | **PASS**: 165 cases, 4 813 steps, 0 disagreements; `trusted_time_vectors.py --check` PASS | `trusted_time_model_nocapture.log`, `unit/trusted_time_vectors.log` |
| Layer-7 differential | `admission_scenarios -- --nocapture` | **PASS**: 15 scenarios, 72 steps, 121 comparisons, 0 disagreements | `admission_scenarios_nocapture.log` |
| Seeded defects | `test_m2_registry_policy.py` | **PASS**: 119 of 119 detected (22 new), each by its named test; the unmodified copy passes; each new defect's failing assertion is recorded | `harness/`, `harness/defect_reasons_b1.log` |
| Full M2 qualification | Clean clone as UID 1001, pinned container (`2a0bcf342c57`), `qualify_m2_rust.sh`, `CHAM3_M2_REQUIRE_CLEAN=1` | **PASS** (`m2 qualification: PASS`): unit 234+234, inventory-exact; harness 149 in 417.3 s; provider PASS; ECR 19/96/18 with 27 scenarios / 365 steps; format differential 5 968 / 0 (corpus `44ea2aa4…`, +13 objects from the new scenarios); vault oracle 38; both lock checks PASS; `dirty_tracked 0` | [VM evidence](../evidence/M2-cryptographic-contract-core/m2_006b1_amd64_vm_c9129bc/) |
| Static checks; static tests | same clone and container | **PASS**; 82 OK; clone clean afterwards | `qualification_console.txt` |
| Full-filesystem probe, five wrapper cases | 2 MiB tmpfs | **PASS**; reports byte-identical to db5bb59's; 5/5 | `storage/` |
| Unchanged inputs | `git diff --exit-code e7803ef c9129bc -- …` | no dependency, lock, pin, workflow, Dockerfile, ADR or other-crate source change; `trusted_time_model.py` docstrings only; db5bb59..e7803ef docs-only | `unchanged_inputs.txt` |
| GitHub Actions at c9129bc | m2 38609857266, m1 38609857244, m0 38609857191 | **success**, every job and step, including the vault protection positive path; artifact 11918223662, digest `sha256:d8070e9b…d3b5` verified | [CI evidence](../evidence/M2-cryptographic-contract-core/m2_006b1_amd64_ci_c9129bc/) |
| Real hardware clocks, suspend, time synchronization, RTC set back across reboots; real crash tests; Miri/sanitizers/fuzzing | — | NOT_RUN | ADR 0007 §5.7; storage formats §7 |

## Review

- **Requested:** a full independent review of stage B1 by Codex, using the prompt below. The record goes on a new branch, `review/m2-006-b1-security`, based on `main` (9cf8825), and holds one file.
- **Earlier records on this package:** stage A, `review/m2-006-a-r4-security` at 06a9423 (APPROVE). The stage-A approval does not cover B1.

## Remaining

- **Limits (ADR 0007 §5.7, unchanged):**
  - Lag can accumulate across reboots with a clock that was set back.
  - An accepted forward error is never corrected. Option B only stops it from advancing an admitted start; expirations come early, which fails closed. Forward-error recovery is deferred.
- **NOT_RUN:** real hardware clocks, suspend, time synchronization, and repeated reboots with the RTC set back. Restarts and reboots are simulated through `boot_id`, and the clocks come from a test source.
- **Stage B2:** the lifecycle writers (the expiry latch, `session_event`), one-shot reservations, `confirm_enforcement`, reconciliation, audit gaps and `history`.
- **Stages C–D:** the ceremony, deletion, proposals, restore and delegation.
- **M2-007:** transport and processes.
- **Uncommitted state:** none after the docs commit.
- **Next:** Codex reviews B1. Findings come back as B1 revisions, and an APPROVE opens B2.

## Review prompt for Codex

~~~text
You are the independent REVIEWER (Codex) for cham3 M2-006 stage B1 in Kayyo321/cham3: a full security review of
one staged candidate (trusted time, the option-B observed start, the commit-timestamp rule). You did not author this
work. You run on the owner's home computer; Docker worked for you before (docker ps, sg docker); record BLOCKED
only with the exact command and error.

0. Bring the local clone to the candidate FIRST:
     cd ~/cham3 && git status --porcelain && git stash list
     git fetch origin codex/m2-cryptographic-contract-core main
     git checkout codex/m2-cryptographic-contract-core && git reset --hard origin/codex/m2-cryptographic-contract-core
     git rev-parse HEAD        # expect the docs-only commit after c9129bc (its parent must be c9129bc)
     git log --oneline -7      # db5bb59 -> 84464cc -> 9d13864 -> 68ca252 -> e7803ef -> c9129bc -> the docs-only commit
   Move unexpected local changes aside (git stash); never force-push or edit the implementation branch. Read every
   document from this checkout; make the clean clone for testing from it
   (git clone --no-hardlinks ~/cham3 <dir>; git -C <dir> checkout --detach c9129bca0e6c4b1e6caf72774e05a27e09a4d542).
   Your no-cache image from the last review (cham3-m2-independent-r3-final) has the current locks.

Read AGENTS.md; docs/decisions/0007_m2_lifecycle_time_consumption.md (the architecture: §5, the §5.7 limits, §8 option
B, errata E4 and E6); docs/specs/m2_registry_storage_v1.md §§2-4 and §8 (the implementer's record — check it
against the ADR, not the other way round); the changed rows of docs/specs/m2_service_ipc_v1.md and §7 of
docs/specs/m2_wire_format_v1.md; docs/handoffs/M2-006-stage-b1.md; the stage B split and the B1 owner decisions in
docs/tasks/M2-cryptographic-contract-core.md. Treat author claims as claims to verify.

Targets: candidate c9129bca0e6c4b1e6caf72774e05a27e09a4d542 (range e7803ef..c9129bc, one commit, 34 files) and the
docs-only commit after it. Confirm the commits between db5bb59 and e7803ef are docs-only. Confirm the range changes
only core/registryd, core/policyd, core/identityd/src/outcome.rs (the code), the tests of contract_format and
contractd (counts and the shared clock), the two oracles and the trusted-time vector generator, the vectors
(ecr_v1 only by added scenarios; trusted_time_v1 new), the harness modules, run_m2_tests.sh and the inventory, and
three specifications — no dependency, Cargo.lock, pin, workflow, Dockerfile or ADR — and that the docs-only commit
changes only docs/. Do not merge main or review branches; do not edit implementation files.

1. ADR 0007 §5 rules 1-4 and 6: the offset rule (rebase on every accepted forward sample; never lower; BOOTTIME pace;
   an older sample's own offset), persistence (every increase ledgered before the clock relies on it; the floor in
   every anchor image, merged; the 60 s refresh; same-boot resume from the maximum over the anchor floor and a
   replayed ledger's time entries; a new boot from the last persisted trusted and observed values), the sharing with
   policyd (max of own and snapshot; time_sample before reliance; another boot; a boottime ahead). Try concurrent
   writers of floors, a raise whose ledger append fails, a crash between the ledger entry and the anchor write, a
   restart with the wall clock set back, a contained start, an invalid anchor (no floor), a ledger of another run.
2. Rule 2 anomalies (erratum E6): once per user per episode, the 300/301 s boundary, the record's content and replay,
   which operations record them, a quarantined user, a pending anomaly blocking admissions.
3. §8 option B: the observed counter (never advanced by wall-clock samples or powered-off time; same-boot and
   new-boot resume), commit_admitted.observed, the start rule in policyd, and the bound by an error present at
   admission. Try a forward error after admission, a start equal to the admission time, a grant restored or merged.
4. Erratum E4 (timestamp_rule): both clauses, the check order (after stale_head, before the state checks),
   admission-only (replay, the published vectors without `now`), no durable change on refusal, the signing window
   at trusted now, a clock that cannot be sampled (admission refused; restrictions never blocked; the author's claim
   that the last trusted value is a lower bound that proves an expiry).
5. Rule 6 (a new boot): every live temporary branch ends in memory and durably (branch_terminated), once per boot,
   not on a same-boot restart, with a crash before the record is durable (same boot: ledger; the old marker still
   naming the previous boot: detected again); network slots freed; trunk unchanged.
6. Oracles: the trusted-time vectors are exactly the model's (trusted_time_vectors.py --check), the differential's
   coverage (165 cases, 4 813 steps, restarts in the same and in a new boot), whether the model and the clock could
   agree on a wrong behaviour; ecr_oracle's timestamp rule and the four E4 scenarios against wire format §7.
7. Harness: the twelve trusted_time tests, the differential, the clock/anchor/ledger unit tests, the three policyd
   tests; tests/m2/harness/test_m2_registry_policy.py (119 seeded defects: 22 new, 12 stage-A anchors re-pointed)
   — judge whether each new defect's test discriminates for the right reason; the two new trusted-time harness tests.
8. Evidence: qualification in a clean clone at c9129bca0e6c4b1e6caf72774e05a27e09a4d542 (CHAM3_M2_REQUIRE_CLEAN=1;
   nofile as before): unit 234+234, harness 149, ECR 19/96/18 with 27 scenarios / 365 steps, trusted-time vectors
   PASS, the rest as before; static checks and sh tests/run_tests.sh static; the full-filesystem wrapper and the five
   wrapper cases. Compare GitHub Actions m2_checks 38609857266, m1_checks 38609857244, m0_checks 38609857191; download the artifact
   and check its digest (artifact 11918223662, sha256:d8070e9b69a5ee20c1aa659009146dc019dd33e5123cb385423dbb6db652d3b5).
9. Codex Security plugin on e7803ef..c9129bc; record name/version and findings.

Record every check as PASS/FAIL/NOT_RUN/BLOCKED with exact commands and evidence. Return APPROVE or
CHANGES_REQUESTED for M2-006 stage B1 and make no owner decisions (list questions for the owner separately).
Publish only your record on a new review branch (review/m2-006-b1-security) and end with:

=== CHAM3 M2-006 STAGE B1 VERDICT ===
reviewer: Codex
environment: <OS/arch; UID; Docker access: yes/no and how; image id; nofile limit used>
local_clone: <~/cham3 HEAD after step 0; local changes or stashes: none / moved aside (what)>
review_record: <branch> @ <commit> : <path> (sha256 <digest>)
candidate_reviewed: c9129bca0e6c4b1e6caf72774e05a27e09a4d542
docs_commits_reviewed: <shas> (docs-only: yes/no)
scope_exact: <the listed paths and nothing else: yes/no>
verdict: M2-006 stage B1: <APPROVE/CHANGES_REQUESTED>
offset_rule_and_persistence: <PASS/FAIL> (<..>)
anomalies: <PASS/FAIL> (<..>)
observed_start: <PASS/FAIL> (<..>)
timestamp_rule: <PASS/FAIL> (<..>)
new_boot: <PASS/FAIL> (<..>)
oracles_and_vectors: <PASS/FAIL> (<..>)
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
