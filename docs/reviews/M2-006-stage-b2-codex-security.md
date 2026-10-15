# M2-006 stage B2 independent Codex review

**Verdict: CHANGES_REQUESTED.** Three reproduced implementation defects require repair. A seeded-defect evidence claim also needs correction. Passing qualification does not demonstrate the missing cases.

Reviewer: Codex, independent of the Claude Code author. Reviewed on 2026-10-14 America/New_York. Role: REVIEWER; no implementation changes, merges or owner decisions. M2-006 B2 returns to its implementer for revision and independent re-review. C remains gated on B2 approval. This record does not modify the task/STATUS files on the implementation branch.

## Immutable scope and startup

- Implementation baseline: `f6f68831a825d9036406356f852a10bd711d4b5b`.
- Candidate: `248ca6ef90d22e270db0e9a3f5bf789502d18720`, exactly one commit above baseline.
- Docs follow-up: `ac28d240d880c1e6b31cd6d3e772c74f2b37af01`; parent is exactly the candidate.
- Earlier docs commits checked: `16944ab29bb6406ce0ed08210a6ef326c2712ee3`, `ff9cb2cffdaf282736d9a146948eeff67623dc30`, `f6f68831a825d9036406356f852a10bd711d4b5b`. The aggregate `5ea2407..f6f6883` is docs-only. E7 is worded as the owner's four prior answers; this review verifies that record, and makes no new owner decision.
- Review branch: `review/m2-006-b2-security`, based on `origin/main` at `9cf882503848f8c6cc4938a588cc9fc062a9392d`. Only this record is published.

Startup commands, all PASS:

```sh
cd /home/dev/cham3
git status --porcelain
git stash list
git fetch origin codex/m2-cryptographic-contract-core main
git checkout codex/m2-cryptographic-contract-core
git reset --hard origin/codex/m2-cryptographic-contract-core
git rev-parse HEAD
git log --oneline -6
```

The working tree and stash list were empty before reset; nothing was moved aside. HEAD after startup was `ac28d240d880c1e6b31cd6d3e772c74f2b37af01`, and the implementation checkout remains there, clean. No force push or implementation file edit occurred.

PASS — `git diff --name-status f6f6883 248ca6e` contains exactly:

```text
core/policyd/src/evaluate.rs
core/policyd/src/lib.rs
core/policyd/src/service.rs
core/policyd/tests/current_authorization.rs
core/registryd/src/lineage.rs
core/registryd/src/record.rs
core/registryd/src/registry.rs
core/registryd/tests/lifecycle_consumption.rs (new)
core/registryd/tests/storage_integrity.rs
docs/specs/m2_registry_storage_v1.md
docs/specs/m2_service_ipc_v1.md
tests/m2/harness/test_m2_registry_policy.py
tests/m2/test_inventory.txt
```

No dependency, lock, pin, workflow, Dockerfile, script, oracle, vector, ADR or other crate changed. PASS — `git diff --check f6f6883 248ca6e`. PASS — both `git diff --name-only 5ea2407 f6f6883` and `git diff --name-only 248ca6e ac28d24` contain only `docs/` paths.

Read from the synchronized checkout: AGENTS, relevant PROJECT requirements, WORKFLOW, TESTING, SECURITY_INVARIANTS, ROLES, ORCHESTRATION, STATUS, ADR 0007 §§1–8 including E7, ADR 0008 §2, the B2 task plan/E7 record and handoff, storage specification §§2/7/9 and changed IPC rows. The storage specification and twelve author placements were treated as claims rather than architecture authority. No applicable SECURITY.md was found by the plugin resolver for the reviewed source scopes.

## Findings requiring revision

### B2-01 — refresh trusted time after reservation preparation

**Low security severity; required acceptance repair.** `core/registryd/src/registry.rs:2333`, `2345–2349`; supporting `fresh_now` at `1515–1519`, lifetime check at `2386`, consumption at `2400`.

`reserve_use` samples before acquiring the user lock. After acquiring it, `ready` may flush prior pending restrictive writes. Those writes can block. `fresh_now(&observed)` then checks only whether the earlier sample succeeded and reads the cached clock; it does not sample BOOTTIME again. Consequently the required lifetime recheck under the user lock can use time from before that blocking work.

**FAIL, debug and release:** begin at T1+60 with a one-shot expiring at T1+300. Leave an unrelated session end pending, recover storage, and advance both fixture clocks by 600 seconds during the prerequisite flush. Eligibility/consumption follows that flush at actual T1+660, but the reservation returns `Ok` using cached T1+60. No wall-clock rollback or privileged clock manipulation is modeled. The delay belongs to an earlier pending write, not the reservation's own durability write.

This violates ADR 0007 E7.1's reservation-time recheck. The request began before expiry; this reproduction does **not** prove formal non-linearizability at every possible abstract point, actual application access, or deployed broker enforcement. The plugin's low severity reflects the narrow one-shot scope and prerequisite I/O delay, rather than a broad externally demonstrated bypass.

Required: obtain a current sample after lock acquisition and blocking preparation, immediately before lifetime validation/consumption. If that sample fails, deny without consuming. Cover delayed pending writes and user-lock contention crossing expiration; retain §8 option B, anomaly/floor persistence and fail-closed behavior.

### B2-02 — reject malformed mismatch evidence before persistence

**Medium correctness/availability severity; required acceptance repair.** `core/policyd/src/evaluate.rs:417–420`, `core/registryd/src/lineage.rs:716–733`; reporting at `service.rs:352–353`, decoding at `record.rs:638–644`.

The public runtime identity's package string is copied into `identity_mismatch`. The registry checks lineage existence and inequality, but not the decoder's nonempty, 255-byte and permitted-text constraints. The restrictive writer can therefore persist a record its own decoder will reject.

**FAIL, debug and release:** an ordinary `evaluate` query with the correct owner/app/installation and an empty package identity returns `denied(unknown_identity)`, but writes one undecodable record. Clean shutdown and restart quarantine UID 1001 with `replay_defect`. The same result was reproduced for a 256-byte package string and a package string containing a newline. A denied runtime query must not poison the user's authority journal.

Required: enforce the record's schema before any in-memory or durable effect, including at registryd's writer boundary; malformed reporting must leave the policy decision denied and the existing journal replayable. Add all three negative controls plus a valid mismatch positive control. No weakening of the strict decoder or quarantine is acceptable.

Runtime identity currently comes from trusted in-process fixtures. No lower-privileged external attribution path is established. The security plugin therefore retains this validated defect as a suppressed security candidate; it remains a blocking API correctness/availability defect for B2.

### B2-03 — end recorded sessions on every new boot

**Low lifecycle/architecture-conformance severity; required acceptance repair.** `core/registryd/src/registry.rs:2471–2476`; supporting `lineage.rs:842–845`, new-boot dispatch at `registry.rs:1416`, and `lineage.rs:682–685`.

B2 persists session starts in `started_sessions`, but new-boot branch termination only clears branch heads. It never makes the known session IDs ended; a user without a live branch is skipped altogether. The idempotent start lookup then acknowledges the old session as a successful start in the new boot.

**FAIL, debug and release:** record an unbound session start in boot 1; clean shutdown; open boot 2. Starting the same ID returns the boot-1 sequence `Ok(6)`. A fresh signed temporary grant bound to that ID is also admitted (`Ok(temporary)`). ADR 0007 §6 says all sessions end at a new boot and IDs are single-use.

Required: reconcile all known sessions on boot change, including started-but-unbound IDs and historical grant-bound IDs, with durable terminal state and pending/replay behavior. Do not conflate ordinary trunk supersession with ending the user's entire session. Test clean/unclean/contained starts, no-live-branch users, repeated start, new grant admission and failure/retry.

Old branch grants stay inactive; new grants still require a fresh owner signature and positive recent liveness. This is not automatic restoration of old authority. With only the fixture producer currently implemented, the plugin suppresses it as a security vulnerability while retaining it as an acceptance defect.

### B2-04 — make the superseded-head mutation reach that assertion

**Low test-evidence severity; required evidence repair.** `tests/m2/harness/test_m2_registry_policy.py:637–639`; `core/registryd/tests/lifecycle_consumption.rs:775`, later checks at `799–804`.

The author claims all 35 new mutations fail at their intended assertion. `enforcement_of_a_superseded_commit` instead stops on the earlier **unknown commit** case: removing the live-head guard changes its error from `ipc/stale_head` to `admission/lifecycle_rule`. It never reaches the actual obsolete trunk/ended branch assertions in that run. The mutation compiles and is caught, but the retained failure reason is not proof of its named stale-head safety property.

Required: separate/reorder the cases or add a focused mutant check so the superseded trunk and ended branch failures are independently demonstrated. Retain the unknown-ID error-contract check. This does not assert that the current live-head implementation is broken; its normal positive and negative tests pass.

## Behavior and acceptance matrix

Each PASS is limited to inspected source and component fixtures. Command definitions below make the exact runs reproducible.

| Area | Result | Evidence and practical limits |
|---|---|---|
| Reservation roles and named-grant evaluation | PASS | policyd fixture-only front; registry service:policyd-only; request, identity, mandatory policy, key/epoch/current/lifetime/session checks inspected; supplied tests. |
| Reservation consumption/durability | PASS | Terminal in memory first, restrictive path, failed journal denies, ledger-only re-journaling then uncertain; exactly one winner in two-thread race; pending/failed-anchor reservation refuses without consuming another grant; trunk supersession refuses without a new record. |
| Reservation fresh lifetime | FAIL | B2-01; cached sample after prerequisite delay. |
| Outcomes and reconciliation | PASS | Once-only completion, no completion after uncertain; supplied clean/same-boot/new-boot cases plus contained restart and second restart; ledger-only reservation re-journaled before uncertain; forged event quarantine. |
| Sessions | FAIL | Explicit start/end idempotence, pending end/retry, bound end versus unbound event, contained evidence and admission refusal pass; new-boot finality fails B2-03. |
| Confirmations | PASS | Live head/current epoch/normal user checks, latest per domain and current-run map only; any replay starts pending; not in lineage_head and not eligibility. Runtime IDs/domains are fixture claims, not attributed enforcement. |
| Audit gaps | PASS | Lost use/session/enforcement evidence, already-owed writes and recovery covered by supplied test; independent ledger-failed and quota-exhausted ordinary evidence produce gap without evidence effect; durable-but-unanchored outcome produces no gap and stays closed. Contained-start gap names previous boot. |
| Mismatch and expiry latch | FAIL | Valid mismatch roles/durable/pending dedup, scope-only/subject/unknown-lineage refusals and all-current-grant expiry latch pass. Same-second expiry refusal is safely retried and does not change denial (source review); malformed mismatch poisons replay, B2-02. |
| History | PASS | Own UID/admin versus other UID/services, ≤16 or one commit, cursor and omission of identity objects, bodies against journal, current-run status, pending count/policy state, unknown user empty, unanchored refusal; supplemental quarantined refusal. |
| Harness execution | FAIL evidence claim; PASS execution (149 tests, all 159 defects detected) | Inventory and reason audit below; B2-04 qualifies the author's discrimination claim. |
| Full filesystem/wrapper | PASS | Actual full 2 MiB tmpfs probe debug/release; reports byte-identical to author and CI; five wrapper cases. |
| Static checks/tests | PASS | shellcheck, py_compile, yamllint, configured secret scan; 82/82 static tests. |
| CI/artifact | PASS | Three exact runs successful at candidate; seven jobs/88 steps; zip digest exact; contents comparison below. |

Audit-gap reserve depletion is the author's stated limit, consistent with ADR 0008 §1: many lost evidence records consume reserve; subsequent restrictions remain pending/denied. Restrictive failures are retained and do not need separate gap events. The initial loss of the ledger while the reservation's journal/anchor succeeds returns a successful durable reservation and marks the run unverifiable; this was observed, not represented as a journal failure or double-use. Later reservations are denied by the failed-ledger gate. The fully durable consumption cannot be repeated; the simultaneous-all-writes-fail residual remains owner-approved E5.

## Environments, commands and retained evidence

Host: Debian GNU/Linux 13.7, x86_64, kernel `7.1.13+deb13-amd64`; UID/GID 1001. Docker access **yes**, direct `docker info` succeeded as dev in docker group (no sudo/sg needed). Server version `26.1.5+dfsg1`. Container: `cham3-m2-dev`, image `sha256:2a0bcf342c57a9352326376771229a7e78e291095c561728df38636e577630a9`. Container tests use `--user 1001:1001 --ulimit nofile=1024:1024`. Host nofile is 524288; it was not changed. No privileged container or host protection change.

Temporary storage directory `T`:

```text
/tmp/codex-security-artifacts-7f3fda5421aea9b6b256e9085b33eb170d934fdc7693a1e4c8c0e34d9c2dae83
```

Disposable clean qualification clone `Q=/home/dev/cham3-m2006b2-codex/qual`; separate reproduction clone `R=/home/dev/cham3-m2006b2-codex/repro`; record-only worktree `/home/dev/cham3-m2006b2-codex/review`. Both clones were made by `git clone --no-hardlinks /home/dev/cham3 <directory>` then `git checkout --detach 248ca6ef90d22e270db0e9a3f5bf789502d18720`. They initially lived below T and were moved, with T symlinks retained. Q remains clean; R differs only by two untracked reviewer test files. Published implementation files were never edited.

The first supplemental build on host `/tmp` failed with `/usr/bin/ld: final link failed: No space left on device`. `df -h /tmp /home/dev/cham3` showed the 16 GiB host tmpfs full versus over 500 GiB available on home. The first qualification was stopped during its harness after its unit suite passed (`docker stop 80bda208b9bd`). Only this review's disposable clones were moved to home, then qualification and supplemental builds were rerun. Initial interrupted/ENOSPC attempts are NOT_RUN to completion, not candidate failures or Docker BLOCKED claims; `qualification.log` and `reviewer_tests_debug.log` retain them.

The following commands were run, with logs in T. Shell variables in this record abbreviate the exact paths above.

```sh
docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 \
  -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
  -v "$Q:/work" -w /work cham3-m2-dev \
  bash build/scripts/qualify_m2_rust.sh --record
# T/qualification_retry.log; Q/out/m2/qualification/amd64_20261015T002445Z_1/

docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 \
  -e HOME=/tmp/cham3_home -v "$Q:/work" -w /work cham3-m2-dev \
  bash build/scripts/run_static_checks.sh
# T/static_checks.log: exit 0, PASS

docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 \
  -e HOME=/tmp/cham3_home -v "$Q:/work" -w /work cham3-m2-dev \
  sh tests/run_tests.sh static
# T/static_tests.log: exit 0, 82 OK

docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 \
  --tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700 \
  -e HOME=/tmp/cham3_home -v "$Q:/work" -w /work cham3-m2-dev \
  sh tests/m2/storage/check_reserve_full_disk.sh out/m2/storage
# T/storage.log: exit 0, PASS

docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 \
  --tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700 \
  -e HOME=/tmp/cham3_home \
  -e CHAM3_M2_RESERVE_PROBE=/work/out/m2/storage/target/release/cham3_reserve_probe \
  -e CHAM3_M2_WRAPPER_DISK=/full-disk -v "$Q:/work" -w /work cham3-m2-dev \
  python3 -I -m unittest discover -s tests/m2/harness -p test_m2_reserve_probe.py -k wrapper -v
# T/wrapper.log: exit 0, 5 OK

docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 \
  --ulimit memlock=1073741824:1073741824 -e HOME=/tmp/cham3_home \
  -v "$Q:/work" -w /work cham3-m2-dev \
  sh tests/m2/protection/check_vault_protection.sh out/m2/protection
# T/protection.log: exit 0, protection_verified, PASS
```

**PASS — qualification exit 0**, `m2 qualification: PASS`, `CHAM3_M2_REQUIRE_CLEAN=1`, candidate/tree exact and `dirty_tracked 0` before/after. Unit debug/release **256 + 256**, inventory-exact; harness **149** in **520.674 s**, all **159** seeded defects detected (35 new). ECR **19 valid / 96 invalid / 18 admission vectors, 27 scenarios / 365 steps**; trusted-time **165** vectors PASS; format **5968 cases / 0 mismatches**, corpus `44ea2aa43135df93b3639c1c64810e4c16b14506297b314b89ba3e4cd8cf426c`; provider PASS; vault oracle **38**. Both installed-package/downloaded-deb locks PASS; crate-source inventory, dependency inventory, rustfmt, clippy, naming, debug/release build/backend/profile checks PASS. Valgrind probe and secure memory PASS, zero errors. Vault process protection positive path separately PASS.

Exact unit/harness evidence: `Q/out/m2/runs/unit_20261015T002514Z_1865/` and `Q/out/m2/runs/harness_20261015T002657Z_4693/`. `Cargo.lock` SHA-256 `9801453aa7023cffc1e3f5b1ec327c284d365c2ce4b9cdd4dc41efddee384c57` unchanged. The source tree is `6ec605df29ff21aefd4dd39473c5f8dedeef6109`.

PASS — explicit differential commands, using the same Docker options and `CARGO_TARGET_DIR=/work/out/m2/runs/unit_20261015T002514Z_1865/target_debug`: `cargo test --frozen -p cham3_registryd --test admission_scenarios -- --nocapture` (4 tests, **15 scenarios / 72 steps / 121 comparisons / 0 disagreements**) and `cargo test --frozen -p cham3_registryd --test trusted_time_model -- --nocapture` (1 test, **165 cases / 4813 steps / 0 disagreements**). T holds each named nocapture log.

Supplemental tests add new integration targets only in R; copies of candidate `lifecycle_consumption.rs` and `current_authorization.rs` supply their original builders/helpers. Original tests are filtered out by `reviewer_`. Actual source files and logs are retained under the plugin scan's `artifacts/02_discovery/validation_artifacts/b2/`.

```sh
docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 \
  -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/work/out/reviewer_target \
  -v "$R:/work" -w /work cham3-m2-dev \
  cargo test --frozen -p cham3_registryd --test reviewer_b2 reviewer_ -- --nocapture
# repeat with --release before --frozen
# exit 101 each: 10 PASS, 2 FAIL (B2-01, B2-03)

docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 \
  -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/work/out/reviewer_policy_target \
  -v "$R:/work" -w /work cham3-m2-dev \
  cargo test --frozen -p cham3_policyd --test reviewer_policy_b2 reviewer_ -- --nocapture
# repeat with --release before --frozen
# exit 101 each: 1 FAIL testing all three malformed package cases (B2-02)
```

Final supplemental sources: `reviewer_b2.rs` SHA-256 `24d4b2adfaa77fe1755259616746c81dd46412a2caa6526fc701fe34e47448f6`; `reviewer_policy_b2.rs` SHA-256 `c1dbabbe6167b010835159f7d50f8b2b6229aae401c432dc0e78ce5f56b53d72`. Final logs: `reviewer_tests_debug_matrix.log`, `reviewer_tests_release.log`, `reviewer_policy_debug_final.log`, `reviewer_policy_release.log`. These are actual execution bytes, not reconstructed outputs.

## Harness reason audit

PASS — source counts: 124→159 seeded defects, 35 new; 241→256 Cargo tests, 149 Python unchanged; 11 new lifecycle integration tests, four new policy tests; record unit test extended; no duplicate inventory entries. Four changed anchors (`ended_sessions_forgotten`, `mandatory_policy_ignored`, `containment_without_a_record`, `admission_on_a_stale_clock`) remain unique and retain their original purpose. `mandatory_policy_ignored` has two unique edits. The changed storage expectation checks containment then the previous-boot gap, rather than merely accepting extra records.

The author log contains exactly 35 exit-101 entries. All mutations compile and their named tests fail. The following judgments are based on independent source/assertion/log inspection, not a claim that the harness automatically proves assertion identity. B2-04 is the one failure of the stronger stated reason claim.

| New mutation | Actual discriminated property / judgment |
|---|---|
| reservation_not_consumed | PASS: standing becomes consumed. |
| reservation_of_any_lifetime | PASS: persistent grant refused. |
| reservation_by_the_consumer | PASS: direct consumer role refused. |
| reservation_subject_not_checked | PASS: substituted executable/owner refused. |
| reservation_lifetime_not_checked | PASS: future start refused. |
| reservation_start_ignores_observed_time | PASS: forward clock without observed elapsed time refused. |
| reservation_epoch_not_checked | PASS: epoch floor refusal. |
| reservation_during_emergency | PASS: persisted emergency refusal. |
| reservation_on_a_stale_clock | PASS: missing sample refusal (not B2-01's waiting case). |
| reservation_consumed_only_once_durable | PASS: journal failure still consumes. |
| open_reservations_not_reconciled | PASS: exactly open reservation becomes uncertain. |
| closed_reservation_completed_again | PASS: second outcome refused. |
| completion_by_any_service | PASS: policyd completion refused. |
| ended_session_started_again | PASS: explicitly ended session cannot start. |
| session_event_end_not_final | PASS: repeat end retains first sequence. |
| bound_session_ended_as_an_event | PASS: wrong record kind refused at end operation before body assertion. |
| enforcement_status_from_earlier_runs | PASS: status empty after restart. |
| enforcement_of_a_superseded_commit | FAIL intended reason: stops at unknown-commit error-code comparison, B2-04. |
| enforcement_epoch_not_bound | PASS: epoch mismatch refusal. |
| first_enforcement_outcome_wins | PASS: latest per domain replaces previous result. |
| lost_evidence_without_a_gap | PASS: completion gap appears in ledger. |
| contained_start_without_a_gap | PASS: containment plus previous-boot gap advances two records. |
| identity_mismatch_repeated | PASS: repeats return first sequence. |
| identity_mismatch_from_any_service | PASS: contractd mismatch role refused. |
| identity_mismatch_of_the_subject | PASS: authoritative identity refused. |
| history_for_other_users | PASS: cross-UID owner_mismatch. |
| history_pages_unbounded | PASS: ≤16 records. |
| history_commit_shares_a_page | PASS: one commit alone. |
| history_serves_an_unanchored_record | PASS: storage_unavailable while anchor fails. |
| expiry_not_latched | PASS: exact two current expired grant bodies. |
| ended_grants_latched_again | PASS: no repeated writes. |
| identity_mismatch_not_reported | PASS: one mismatch record. |
| reservation_without_evaluation | PASS: wrong-operation request actually reaches reservation instead of refused no_matching_grant. |
| policyd_reserves_any_lifetime | PASS interface check: registry still refuses persistent grant, but error differs from policyd's not_one_shot decision; not proof of persistent consumption. |
| policyd_reserves_for_any_caller | PASS: contractd front role refused. |

## CI comparison

PASS — independent `gh api` reads for runs `38657106670`, `38657106555`, `38657106631` and each `/jobs?per_page=100` response: all head SHAs equal candidate; conclusions success; job counts 1/3/3, step counts 10/39/39, zero steps outside success/skipped. Local API responses retained in T as `ci_run_<id>.json`.

```sh
gh api repos/Kayyo321/cham3/actions/artifacts/11938475946
gh api repos/Kayyo321/cham3/actions/artifacts/11938475946/zip > "$T/ci_artifact.zip"
sha256sum "$T/ci_artifact.zip"
```

PASS — digest is exactly `5d7adfc6fbe4126ef1353b53b2488f5604d8fdc6cb60b557bdb1206f14b67dff`, equal to API digest; not expired; 111308 bytes, 45 extracted files. PASS — parsed CI/local JSON results, exact tests/suites/counts, format corpus/coverage/mismatch count and provider result agree. Both full-filesystem reports compare byte-for-byte. Identity files name the candidate with dirty_tracked 0. Local timings and run-path fields differ as expected; no byte-identical build or hardware-enforcement claim.

## Codex Security plugin

PASS — **Codex Security 0.1.32**, exact diff `f6f6883..248ca6e`, scan `fdeb335d-9cb9-4712-a0c6-ee485a5b6cd8`, completed and sealed once. Preflight exit 0 READY, all three capability checks pass; no user configuration changed. Exact preflight command:

```sh
python3 /home/dev/.codex/plugins/cache/openai-curated-remote/codex-security/0.1.32/scripts/config_preflight.py \
  --profile security_diff_scan --cwd /home/dev/cham3 \
  --runtime-check delegation_available=true --runtime-check goal_tools_available=true \
  --multi-agent-runtime-owner native --multi-agent-runtime-provenance thread-context
```

The plugin-directed independent architecture pass and bounded source/coverage workers were read-only. All six source inventory entries and all seven changed test/doc files were reviewed. Three candidates were validated dynamically. One survives security calibration: low finding `csf_cd1e490ae2cd30332188cc55` (B2-01, CWE-367). B2-02 and B2-03 are confirmed task defects, suppressed from security findings for current trusted-fixture/self-only prerequisites, with their evidence retained in candidate outcomes and coverage. Suppression does not approve their B2 acceptance behavior.

Generated report and evidence root:

```text
/home/dev/.codex/state/plugins/codex-security/scans/cham3/248ca6ef90d22e270db0e9a3f5bf789502d18720_20261015T001814Z_p4y_t_sc/report.md
```

The completed result was read back through `get_codex_security_completed_scan`. Finalization normalized legacy finding details; it did not discard the finding. Measured plugin usage (coverage complete, five threads): total 15483806 tokens, input 15430429, cached input 14822400, output 53377. These are returned workbench measurements, not cost estimates. Daybreak access was not granted; its advisory did not gate the scan.

## Twelve author placements and limits

1. PASS: registry-generated random reservation ID; operation ID is evidence only.
2. PASS existing refusal vocabulary; FAIL fresh-time validation after waiting, B2-01.
3. PASS: use_reserved is an event, not writable through restrict.
4. PASS: uncertain at every start, including clean/contained, once; late completion refused.
5. PASS explicit unbound end and subsequent admission refusal; FAIL new-boot finality, B2-03.
6. PASS domain/latest-run/live-head/epoch/status behavior; category absent from commit is accepted as documented fixture evidence, not verified enforcement.
7. PASS one restrictive gap per lost evidence; reserve depletion limit retained.
8. PASS contained gap names previous run's boot.
9. PASS valid mismatch durable/pending dedup per lineage/package/hash ever; FAIL malformed evidence, B2-02. Scope-only mismatch is denied without record. Per-tuple dedup is not a bound on arbitrarily many distinct identities.
10. PASS source review of all-current expiry latch, registry-own-time refusal/retry and unchanged denied decision; no real subsecond clock probe claimed.
11. PASS history access/page/body/status/pending/policy/quarantine/unknown behavior.
12. PASS evidence accepted for contained users; reservation and confirmation refused.

NOT_RUN: actual sessiond, launcherd, netd/brokers, attributed runtime IDs, real process/power/kernel crashes, physical clocks, suspend/reboot hardware, deployed IPC isolation. These are future stages, not demonstrated by fixtures. Miri, sanitizer and coverage-guided fuzzing checks are unavailable on the pinned stable toolchain and remain NOT_RUN, not PASS. Existing ADR 0006 E5 and ADR 0007 §5.7 limits remain unchanged.

Owner questions: **none required to repair the demonstrated behaviors**; the approved ADRs already specify the outcomes. No architecture/risk option is chosen here. Next owner: implementer repairs B2-01–B2-04, reruns affected tests/full qualification and supplies a new immutable candidate for independent review. Do not merge this implementation on the strength of its existing passing tests.
