# M2-006 stage A independent Codex review

- State: **CHANGES_REQUESTED** for stage A; do not open stage B on the basis of this review.
- Reviewer: Codex (independent of the implementation). Initial review used Astra; the owner switched this conversation to Sol after a product access rejection.
- Candidate: `f4b062826ae54ba4e79c5abd08eeb9fe21d48510`.
- Implementation base: `22b6b5f8b5fafdbb988be8f71736453efd1175f2`; exactly one implementation commit.
- Documentation follow-up: `a9e47e47660879af86ede99986b25a18ffb537aa`, direct parent is the candidate, changes only `docs/`.
- Review branch: `review/m2-006-a-security`, based on `origin/main` at `9cf882503848f8c6cc4938a588cc9fc062a9392d`; only this review record is published.
- Implementation checkout: `/home/dev/cham3`, clean at a9e47e4 before and after review, no stashes or local changes.
- Review worktree: `/home/dev/cham3-review-m2-006-a`.
- Authority: PROJECT.md; ADR 0006 §§2–6, approved choices A/C1; ADR 0008 §1; the stage-A task scope. The storage specification and handoff were checked as author claims.
- Environment: Debian 13.7, Linux x86_64, kernel reported by qualification `7.1.13+deb13-amd64`, UID/GID 1001. Docker `26.1.5+dfsg1`, build a72d7cd. Disposable containers use `cham3-m2-independent-r3` image 4f4f684d76f7, non-root UID 1001, no added capabilities, no privileged mode; nofile=1024:1024. Host nofile was 524288.
- Owner questions: none. No architecture, risk-acceptance or owner decision is made here.

Six Medium security findings are independently reproduced. Two Low acceptance/correctness observations are also reproduced, with no demonstrated independent authority bypass. Existing qualification and CI are passing; they do not establish the missing failure-path invariants.

## Confirmed findings

### M2-006-A-R01 — Medium — Unanchored admission remains eligible

Location: `core/registryd/src/registry.rs:1696` (candidate lines 1696–1700).

lineage_head ignores slot.anchor_failed and exposes an admission reported storage_unavailable as eligible before restart.

Evidence: `review_unanchored_admission_not_exposed_as_normal_view; policy review proves actual evaluate eligible`. The expected-invariant reviewer test fails on the unchanged candidate. A second test through actual policyd.evaluate confirms eligibility with the expected positive and restrictive controls.

Required: Block all authority-bearing snapshots when anchor_failed; test actual policyd evaluation immediately after failed admission.

### M2-006-A-R02 — Medium — Restriction skips ledger after anchor failure

Location: `core/registryd/src/registry.rs:1523` (candidate lines 1523–1528).

restrict applies denial then returns early on anchor_failed before queueing or ledgering it; same-boot restart restores current grant.

Evidence: `review_restriction_after_anchor_failure_survives_restart`. The expected-invariant reviewer test fails on the unchanged candidate.

Required: Persist every new restrictive intent to current ledger/pending state even when prior anchor update failed; keep denial and return restriction_not_durable.

### M2-006-A-R03 — Medium — Replayed restriction lost on second failed restart

Location: `core/registryd/src/registry.rs:917` (candidate lines 917–949).

rejournal queues/flushed old ledger entry without transferring it to new run ledger; failed journal plus another same-boot restart revives grant.

Evidence: `review_replayed_pending_survives_a_second_same_boot_restart`. The expected-invariant reviewer test fails on the unchanged candidate.

Required: Transfer unresolved replay intents durably into new ledger before publishing new running marker; preserve across repeated restarts.

### M2-006-A-R04 — Medium — Clean shutdown loses pending policy raise

Location: `core/registryd/src/registry.rs:1631` (candidate lines 1631–1669).

change_policy anchor failure is not tracked in shutdown; clean marker can be persisted over old map and next startup discards ledger raising epoch9.

Evidence: `review_policy_pending_prevents_clean_shutdown`. The expected-invariant reviewer test fails on the unchanged candidate.

Required: Track global pending policy writes, flush policy before clean marker or refuse shutdown; expose pending policy status.

### M2-006-A-R05 — Medium — Failed emergency release lifts denial

Location: `core/registryd/src/registry.rs:1564` (candidate lines 1564–1586).

change_policy sets emergency false in live memory before failed anchor update; evaluate becomes eligible while release reports restriction_not_durable.

Evidence: `review_failed_emergency_release_keeps_denial; policy review actual evaluate eligible`. The expected-invariant reviewer test fails on the unchanged candidate. A second test through actual policyd.evaluate confirms eligibility with the expected positive and restrictive controls.

Required: Publish permissive policy releases only after durability; retain prior denial on failure.

### M2-006-A-R06 — Medium — Missing entries file accepted as valid empty ledger

Location: `core/registryd/src/ledger.rs:193` (candidate lines 193–199).

Matching validity with missing entries selects ledger_replayed without restriction evidence, avoiding mandatory containment.

Evidence: `review_missing_ledger_entries_invalidates_run`. The expected-invariant reviewer test fails on the unchanged candidate.

Required: Missing entries with surviving validity must classify invalid and trigger containment.

### M2-006-A-R07 — Low (correctness/acceptance) — Recovery does not reconcile identity_seq

Location: `core/registryd/src/registry.rs:611` (candidate lines 611–637).

Recovery accepts checksum-valid selected anchor whose identity_seq exceeds journal count; parser/freshness invariant test fails.

Evidence: `review_anchor_identity_sequence_must_match_journal`. The expected-invariant reviewer test fails on the unchanged candidate.

Required: Check identity count against anchored prefix, allow exactly justified one-record extension, quarantine unexplained mismatch.

### M2-006-A-R08 — Low (correctness/acceptance) — Anchored quarantine leaves live grants in index

Location: `core/registryd/src/registry.rs:895` (candidate lines 895–910).

Anchored quarantined/contained branch only sets slot.state; it omits lineage containment and retains current grants/branches. State still denies evaluation.

Evidence: `review_anchored_quarantine_ends_grants`. The expected-invariant reviewer test fails on the unchanged candidate.

Required: Apply permanent grant/branch containment consistently for anchored states and test ended reasons.

## Failure-path evidence and interpretation

- **R01:** a durable genesis followed by an injected anchor failure returns `ipc/storage_unavailable`. The anchor is unchanged, but `lineage_head` returns `normal` with a live grant; actual policyd evaluation returns `eligible` for that grant and commit before restart. The existing author test checks later writes and shutdown, then applies a restriction before restart; it omits the read/evaluation immediately after failed admission.
- **R02:** after the same anchor failure, `restrict` applies the grant's denial in memory and returns before its ledger/pending write. Restart rolls forward the durable admission and the restricted grant becomes `current`. This is also present in the author's anchor-failure scenario: it expects the grant current after making a restrictive call. An error reply cannot cancel mandatory denial.
- **R03:** first run: journal writes fail, restriction is in the old ledger. Second same-boot start: old entry is applied in memory, journal still fails, new running marker is durable, but new ledger lacks that intent. Third same-boot start: valid empty new ledger resumes old journal and the grant returns to `current`.
- **R04:** failed anchor write of policy epoch/floor 9 leaves live policy 9 and a ledger entry. Once storage recovers, `shutdown` returns success and writes clean using the old anchor policy. Restart skips the ledger because the marker is clean and policy returns to 1. Shutdown must flush or refuse any global pending restriction.
- **R05:** successful emergency restriction first produces the expected denial. A failed release returns `restriction_not_durable` with no new anchor counter, yet live policy is false and actual policyd evaluation becomes eligible. A permissive release needs ordinary durable-publication semantics.
- **R06:** after failed restrictive journal persistence, deleting only `entries` while matching `validity` survives makes startup select `ledger_replayed`, not containment. The writer always creates entries before validity; missing entries cannot prove an empty valid ledger. This is a file-loss/fault-recovery case; applications are not assumed to have protected-store write access.
- **R07:** a slot crafted with `encode_slot`, valid checksum, unchanged seq/chain/user_id, and incremented identity_seq is accepted without quarantine. This test establishes a missing consistency check, not a realistic unprivileged forgery path; severity is Low.
- **R08:** a readable journal with a crafted anchored quarantined state returns a quarantined view whose ended-grant map is empty. The slot's state still denies authorization, so this is a Low permanent-index/branch-state inconsistency. Apply the same lineage containment used for other quarantine paths.

These are real Rust component tests over real private files with synthetic keys and injected IO. Dropping and reopening registry objects simulates process death. This is not physical power-loss, real daemon-crash, transport or OS enforcement evidence.

## Exact commands and verification

Managed temporary execution root:

`/tmp/codex-security-artifacts-9c2a76a22a38fcc87cbdd4b3a7c41f1d989d7a9aa5a6fa67a8c3edc80cfce074`

Retained Codex Security scan/evidence directory:

`/home/dev/.codex/state/plugins/codex-security/scans/cham3/f4b062826ae54ba4e79c5abd08eeb9fe21d48510_20261013T203141Z_wyjgul3p`

Startup, PASS:

```sh
cd /home/dev/cham3
git status --porcelain
git stash list
git fetch origin codex/m2-cryptographic-contract-core main
git checkout codex/m2-cryptographic-contract-core
git reset --hard origin/codex/m2-cryptographic-contract-core
git rev-parse HEAD
git log --oneline -5
git diff --name-status 22b6b5f f4b0628
git diff --name-status f4b0628 a9e47e4
git diff --check 22b6b5f f4b0628
git diff --check f4b0628 a9e47e4
git show --format='%H %P' --no-patch a9e47e4
```

No dirty work was discarded. Both diff-check commands passed. Scope is exactly the 22 requested paths:

```text
Cargo.lock
core/policyd/src/evaluate.rs
core/policyd/src/service.rs
core/policyd/tests/current_authorization.rs
core/registryd/Cargo.toml
core/registryd/src/anchor.rs
core/registryd/src/bin/cham3_reserve_probe.rs
core/registryd/src/journal.rs
core/registryd/src/ledger.rs
core/registryd/src/lib.rs
core/registryd/src/lineage.rs
core/registryd/src/record.rs
core/registryd/src/registry.rs
core/registryd/tests/builders/mod.rs
core/registryd/tests/builders/registry_support.rs
core/registryd/tests/registry_operations.rs
core/registryd/tests/storage_integrity.rs
docs/specs/m2_registry_storage_v1.md
docs/specs/m2_service_ipc_v1.md
tests/m2/harness/test_m2_registry_policy.py
tests/m2/storage/check_reserve_full_disk.sh
tests/m2/test_inventory.txt
```

Cargo.lock adds only the cham3_os_adapter registryd dependency edge. No vector, oracle, ADR, workflow, Dockerfile or pin changes. Documentation follow-up modifies only docs.

Clean qualification clone, PASS:

```sh
git clone --no-hardlinks /home/dev/cham3 /tmp/codex-security-artifacts-9c2a76a22a38fcc87cbdd4b3a7c41f1d989d7a9aa5a6fa67a8c3edc80cfce074/clean
git -C /tmp/codex-security-artifacts-9c2a76a22a38fcc87cbdd4b3a7c41f1d989d7a9aa5a6fa67a8c3edc80cfce074/clean checkout --detach f4b062826ae54ba4e79c5abd08eeb9fe21d48510
sudo -n docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 \
  -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
  -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
  -v /tmp/codex-security-artifacts-9c2a76a22a38fcc87cbdd4b3a7c41f1d989d7a9aa5a6fa67a8c3edc80cfce074/clean:/work -w /work cham3-m2-independent-r3 bash -c \
  'bash build/scripts/qualify_m2_rust.sh > out_review_qualification.log 2>&1; qualification_status=$?; bash build/scripts/run_static_checks.sh > out_review_static.log 2>&1; static_status=$?; sh tests/run_tests.sh static > out_review_static_tests.log 2>&1; test_status=$?; printf "qualification=%s static=%s tests=%s\n" "$qualification_status" "$static_status" "$test_status"'
```

| Check | Result | Evidence |
|---|---|---|
| Clean identity, Debian/toolchain/pins/source manifests/profiles | PASS | candidate f4b0628, dirty_tracked 0, amd64, UID1001; qualification.log |
| Unit debug/release | PASS | 188 + 188, inventory exact; retained unit_debug.json/unit_release.json |
| Requested storage_integrity/registry_operations/policyd suites | PASS | 12/25/12 in each profile; unit JSON |
| Provider oracle | PASS | 7 probe cases and 7 independent oracle vectors |
| ECR oracle | PASS | 19 valid / 96 invalid / 18 admission vectors; 23 scenarios / 352 steps |
| Format differential | PASS | 5955, 0 mismatches; unchanged corpus 0ad8bc2b1e8d39224de699db64aa45ca7adcf70117c555f35b57c537471d395e |
| Vault object oracle | PASS | CHECKS 38 |
| Full harness | PASS | 147 tests, including 12 reserve-probe, two registry/policy tests over 62 seeded defects, two identity/vault tests over 21 seeded defects |
| Static checks | PASS | shellcheck26, py_compile69, yamllint5, configured secret scan; static checks: PASS |
| Static tests | PASS | 82 tests; OK |
| Valgrind | PASS | provider and secure-memory checks, secure-memory10/0errors; child fault processes not instrumented |
| Miri/sanitizers/coverage-guided fuzzing | NOT_RUN | pinned stable toolchain reports NOT_AVAILABLE |

The harness explicitly checks that each defect compiles and the named Rust test fails, while the unmodified copy runs exactly one passing test. The 21 new mutation anchors were reviewed: recovery rows, torn-byte preservation, tombstone/quarantine index, boot/run decision, containment record/grants, ledger-first write/invalidity, write denial, shutdown, monotone policy, slot selection/checksum, rollback bytes and persisted emergency evaluation. They discriminate those edited paths. They omit the reviewer scenarios R01–R08; passing seeded defects are not complete fault coverage.

Full filesystem and wrapper, PASS:

```sh
sudo -n docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 \
  -e HOME=/tmp/cham3_home --tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700 \
  -v /tmp/codex-security-artifacts-9c2a76a22a38fcc87cbdd4b3a7c41f1d989d7a9aa5a6fa67a8c3edc80cfce074/clean:/work -w /work cham3-m2-independent-r3 bash -c \
  'sh tests/m2/storage/check_reserve_full_disk.sh out/m2/review_storage > out_review_full_disk_retry.log 2>&1; probe_status=$?; CHAM3_M2_RESERVE_PROBE=/work/out/m2/review_storage/target/release/cham3_reserve_probe CHAM3_M2_WRAPPER_DISK=/full-disk python3 -I -m unittest discover -s tests/m2/harness -p test_m2_reserve_probe.py -k wrapper -v > out_review_wrapper_retry.log 2>&1; wrapper_status=$?; printf "probe=%s wrapper=%s\n" "$probe_status" "$wrapper_status"'
```

Both profiles show reserve allocation 1048576, restrictive sequence35, admission reserve_depleted, second restrictive sequence36, refill sequence37 and journal chaining. Ledger lines: ledger_started, ledger_append_on_full_filesystem refused, ledger_failed, ledger_validity_removed, previous_ledger missing, ledger_stays_failed. Exit statuses0; wrapper5/5 OK with no leftovers. The anchor is not included in this probe: its no-reserve limit remains explicitly documented.

Reviewer integration, FAIL expected invariants (defects reproduced):

```sh
git clone --no-hardlinks /home/dev/cham3 /tmp/codex-security-artifacts-9c2a76a22a38fcc87cbdd4b3a7c41f1d989d7a9aa5a6fa67a8c3edc80cfce074/adversarial
git -C /tmp/codex-security-artifacts-9c2a76a22a38fcc87cbdd4b3a7c41f1d989d7a9aa5a6fa67a8c3edc80cfce074/adversarial checkout --detach f4b062826ae54ba4e79c5abd08eeb9fe21d48510
# Copy retained artifacts/validation/reviewer_storage.rs into
# adversarial/core/registryd/tests/reviewer_storage.rs; no implementation changes.
sudo -n docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 \
  -e HOME=/tmp/cham3_home -v /tmp/codex-security-artifacts-9c2a76a22a38fcc87cbdd4b3a7c41f1d989d7a9aa5a6fa67a8c3edc80cfce074/adversarial:/work -w /work \
  cham3-m2-independent-r3 bash -c \
  'cargo test --frozen -p cham3_registryd --test reviewer_storage review_ -- --nocapture > /work/reviewer_storage.log 2>&1; result=$?; tail -100 /work/reviewer_storage.log; exit "$result"'
```

Outcome: 0 passed, 8 failed, 12 existing tests filtered out, exit101. These failures assert the intended properties, so they are FAIL verification evidence, not failing compilation. Retained source SHA256 cb3bbbd81e3c56f88cb85d42205c5a99d15535cff96f922d68fed7275d88c6f0; log SHA256 a16b65bad4e91913717c49a03debd6bf9ff6e244080e1a497c11a39ce6666776.

Actual policyd component reproduction, PASS adverse-behavior controls2/2:

```sh
# Separate candidate clone policy_review; add only retained policy_review.rs
# at core/policyd/tests/policy_review.rs.
sudo -n docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 \
  -e HOME=/tmp/cham3_home -e CARGO_PROFILE_TEST_DEBUG=0 -e CARGO_INCREMENTAL=0 \
  -v /tmp/codex-security-artifacts-9c2a76a22a38fcc87cbdd4b3a7c41f1d989d7a9aa5a6fa67a8c3edc80cfce074/policy_review:/work -w /work cham3-m2-independent-r3 \
  cargo test --offline --locked -p cham3_policyd --test policy_review -- --nocapture
```

The tests assert the observed defects: eligible after failed admission/release, with unchanged anchor and appropriate baseline/denial controls. Source SHA25641ec9718ac19b87b60705a62771c51eb6261a55bfbf312df05291b5f0d2fada0; log SHA2560a39f79c27e2ac22bca7d218952c11119604a23f765e552b6265449b876a6ba1.

Environmental interruptions were diagnosed and resolved where possible. Plain docker initially returned permission denied on /var/run/docker.sock; sudo -n docker worked. Parallel test builds encountered actual /tmp ENOSPC. Only this review's completed build targets were moved to /home/dev/cham3-review-builds-m2006a; no unrelated files were removed. The full-disk wrapper and reviewer tests were retried successfully. One initial reviewer source build had a test-only Option<Vec<u8>> type error, corrected with unwrap before the valid runtime result. These setup failures are not product findings.

## CI comparison

PASS exact commands:

```sh
gh api repos/Kayyo321/cham3/actions/runs/38459079087
gh api repos/Kayyo321/cham3/actions/runs/38459079018
gh api repos/Kayyo321/cham3/actions/runs/38459079003
gh api repos/Kayyo321/cham3/actions/runs/38459079087/jobs
gh api repos/Kayyo321/cham3/actions/runs/38459079018/jobs
gh api repos/Kayyo321/cham3/actions/runs/38459079003/jobs
gh api repos/Kayyo321/cham3/actions/artifacts/11846490342
gh api repos/Kayyo321/cham3/actions/artifacts/11846490342/zip > /tmp/codex-security-artifacts-9c2a76a22a38fcc87cbdd4b3a7c41f1d989d7a9aa5a6fa67a8c3edc80cfce074/ci_artifact.zip
sha256sum /tmp/codex-security-artifacts-9c2a76a22a38fcc87cbdd4b3a7c41f1d989d7a9aa5a6fa67a8c3edc80cfce074/ci_artifact.zip
```

All three runs are completed/success with exact candidate head_sha. M2's steps include qualification, vault positive protection, full2MiB filesystem and wrapper success. Artifact id11846490342, m2_qualification_amd64, size105358bytes,44files. Actual downloaded digest matches 6c27239b2149b3a0d39f342f83adc81f0500c1060d5a4437a35dfbd080319a04. Zip identity/summary/unit/harness outputs agree with local counts and immutable candidate. Vault positive protection was checked as CI step success; no separate local positive protection invocation is claimed.

## Requirements disposition and remaining limits

| Requirement | Disposition and evidence |
|---|---|
| Anchor serialization/layout/checksum/counter/alternation/private paths/lock order/mode | Static inspection PASS; author anchor tests and restore tests PASS. User→anchor order observed, no user lock under anchor. Equal divergent counters deterministically choose slotA; normal serialized writes do not produce equal divergence. Count reads are bounded; trailing/boot65 checks reject. Foreign populated slot is constrained by per-user chain/user_id checks. Dedicated extended runtime matrix NOT_RUN. Overall acceptance not closed because R07 and remaining matrix. |
| Restart clean/sameboot/differentboot/missing-invalid-other-run ledger | Author rows PASS; reviewer repeated-restart and missing entries FAIL (R03/R06). Already-held lifecycle record and second successful replay are covered; unknown/quarantined user-entry dedicated tests NOT_RUN. |
| Restrictive denial/ledger/journal/anchor/reply; shutdown | FAIL R02/R03/R04. Normal retry and ledger-failure author tests PASS. Full ledger filesystem PASS. Failed validity unlink handling at ledger.rs299 remains a source concern; combined-failure runtime reproduction NOT_RUN, not a confirmed extra finding. |
| Containment | Author multiple-user durable containment, suspended grants, closed branches, identityd denying scope and policyd denial PASS. All pending/failed containment boundaries and real process death NOT_RUN. |
| Recovery table | Author stale/unexplained/chain/lost/invalid-anchor/torn/replay/tombstone/independent-restore rows PASS. Review R07/R08 FAIL. Missing deleted-user cleanup is explicitly stageC. Complete foreign-user frame/torn case and moved-machine dedicated runtime case NOT_RUN. |
| Ordinary failed append rollback | Existing write/sync/failed-rollback cases PASS: successful rollback preserves old bytes/indexes; failed rollback denies pending writes and recovers at next start. Actual abrupt process termination at each boundary NOT_RUN. |
| Anchor after durable append | FAIL R01: writes are denied but actual policy read remains eligible. Documented no-reserve/full-filesystem anchor limit acknowledged, not proved by the reserve probe. |
| Policy persistence/epochs/roles | Normal monotone/config-raised/stale_epoch/epoch_invalid/role tests PASS. Failed policy shutdown/release FAIL R04/R05. Concurrent policy publication uses a captured snapshot outside the anchor lock; dedicated race reproduction NOT_RUN. |
| Author quarantine placement/fresh identityd | Readable quarantine reports state and fresh identityd key_not_certified; denies admission/identity/restrict. ACCEPT for stageA denial semantics; permanent grants/branches for anchored states still require R08. No owner decision needed. |
| Full qualification and static gates | PASS as tabulated; component/qualification evidence only. |
| Physical power-loss/kernel-crash/real process-death tests | NOT_RUN; no full-system claim. |

The docs-only handoff's “17 files changed” sentence disagrees with actual22 paths (its evidence README correctly says22). This is an informational documentation correction, not an implementation scope change.

## Codex Security plugin

Codex Security/security-diff-scan **0.1.32**, scanId5f311ba1-aeb0-401c-a423-d5fb15d73b6c. Preflight ready/exit0; no configuration edits. Command:

```sh
python3 /home/dev/.codex/plugins/cache/openai-curated-remote/codex-security/0.1.32/scripts/config_preflight.py --profile security_diff_scan --cwd /home/dev/cham3 --runtime-check delegation_available=true --runtime-check goal_tools_available=true --multi-agent-runtime-owner native --multi-agent-runtime-version v2 --multi-agent-session-cap 4 --multi-agent-runtime-provenance thread-context
```

The scan reviewed all11 generated source inventory items plus changed tests/specs/docs, recorded8 candidates, validated all8, and promoted6 Medium findings. R07/R08 were suppressed as independent plugin vulnerabilities and retained as Low project acceptance observations. Finalization succeeded; canonical report is [report.md](/home/dev/.codex/state/plugins/codex-security/scans/cham3/f4b062826ae54ba4e79c5abd08eeb9fe21d48510_20261013T203141Z_wyjgul3p/report.md), with findings/coverage/manifest/SARIF. Coverage is **partial** because the additional fault matrix was not completed. No claim of a complete security clearance.

Measured plugin usage (returned completion metadata, coverage complete for measurement): totalTokens11071058; inputTokens11032975; cachedInputTokens10423808; outputTokens38083; five threads. This is the plugin's measured usage, not a cost estimate or a measure of coverage completion.

A delegated extended validation turn was rejected with the exact tool result: “Agent errored: This content was flagged for possible cybersecurity risk.” The result offered Daybreak access enrollment. The account advisory also returned not_granted; the owner's screenshot reported Daybreak unavailable for Astra. That specific extension remained NOT_RUN/BLOCKED by the returned product error; it was not silently counted PASS. Other completed evidence and confirmed findings were retained. Finalization later succeeded after the model switch.

## Next owner/action

Claude should fix R01–R06, reconcile/test R07–R08, add discriminating negative tests, and present a new immutable stage-A candidate with qualification and CI. Close the documented incomplete fault cases before asking for APPROVE. Do not weaken denial, restore authority from missing evidence, or infer approval from passing old tests. Independent re-review is required. Only this review record is committed/pushed; implementation, ADRs, status and task docs were not edited.
