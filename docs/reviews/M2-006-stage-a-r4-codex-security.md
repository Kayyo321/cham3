# M2-006 stage A revision 4 — independent Codex review

**Verdict: APPROVE — M2-006 stage A revision 4.** R3-01 and R3-02 are resolved in the exact candidate. Required bounded review gates passed with the documented legacy assertion failures and verification limits below. No implementation authored or modified. This record makes no owner decision and does not open stage B.

- Reviewer: Codex, independent REVIEWER. Authority: PROJECT.md, scoped approved ADRs, AGENTS.md, workflow/orchestration/testing/security guidance and the assigned review acceptance criteria.
- Exact base: 908c3911fd08ccf91d4ba6706fbad49d45eaf1a8; candidate: db5bb59fcf461034461d9e0d2f21f5816986134c, one commit.
- Docs-only direct child: 84464cc6bf750d8a2128c662b2fcc10cb7f8ed4f, parent exactly the candidate. All changes are under docs/; evidence, resolutions, handoff, task and STATUS statements inspected against independently retrieved/executed results.
- Prior review read directly from Git: review/m2-006-a-r3-security @ f0d6eb01aeec7858fe67a034f42db09ca04f0177. No review/main branch merged into implementation.
- Local implementation checkout /home/dev/cham3: clean at 84464cc after requested fetch/checkout/reset; no local changes or stashes before/after, none moved/discarded. All documents read from this checkout.
- Publication: only this file on review/m2-006-a-r4-security, based on main 9cf882503848f8c6cc4938a588cc9fc062a9392d; separate clone /home/dev/cham3-r4-review/record.
- Environment: Debian 13.7 x86_64, kernel 7.1.13+deb13-amd64, UID/GID 1001. docker ps and sg docker -c 'docker ps' PASS. Reused independently rebuilt image cham3-m2-independent-r3-final, sha256:4528aed8d303c7255b9138392a4b2f7973a1b944b140cac66a9da40360ff3e56, rustc/cargo 1.85.1. Disposable containers use UID 1001, no added capabilities, nofile 1024:1024. No privileged host changes.

## Findings and disposition

**No new findings. R3-01 PASS, resolved.** All twelve IPC entry points audited: lineage_head, admit, restrict, append_identity, identity_state, journal_users, set_policy_epoch, set_emergency_restriction, rollback_status, policy_state, unusable_users and shutdown. Every role-taking operation retries immediately after its role check and before its own early return. The three trusted in-process operations without caller parameters retry first. Inspection accessors standing, anchored_user, anchor_counter, reserve_bytes, config, run_decision and owner write nothing and are not IPC operations.

Relevant controls: registry.rs:1089,1097,1297,1402,1572,1610,1624,1695,1721,1743,1805,1879,1916; anchor.rs:549–599. Wrong callers do not invalidate validity, write markers or alter the counter. Lock audit: entry hooks acquire neither ledger nor anchor beforehand; retry releases the ledger guard before acquiring the owed flag and anchor. ready may hold a user lock; change_policy may hold policy, but the ledger append guard is gone before mark_run_unverifiable. No inverse anchor-to-user/ledger acquisition was added. The existing user → policy → ledger → anchor order remains intact.

hold_unverifiable sets an in-memory flag independent of whether its marker write succeeds. update applies the caller's map change, then overrides its run marker to unverifiable before serialization. This covers startup, every slot anchor, restrictive policy writes, emergency release and any attempted clean image. All first ledger I/O failures route through mark_run_unverifiable after releasing the ledger guard. A successful marker found in the authoritative durable map clears marker debt without another image. Invalidation debt remains separate. The removed startup conditional is fully replaced by the sticky override: its nominal running value cannot overwrite unverifiable after startup ledger failure. Existing startup tests and the re-pointed mutation discriminate this control.

**R3-02 PASS, corrected E5 wording matches behavior.** ADR 0006 §3 E5, storage formats §§4/7 and IPC rollback_status now distinguish successful validity invalidation or unverifiable marker publication (containment) from journal-only retry (one-tail roll-forward, ledger_replayed, restriction precisely retained without containment). Policy restrictions have no per-user journal record; after ledger failure, their successful anchor image also carries unverifiable. With no successful durable witness, the owner-accepted same-boot exclusion is unchanged. Reboot contains in the tested unclean-run case.

The phrase about policy restrictions ending in containment is read in **E5's failed-ledger recovery context**. Ordinary policy restrictions with a successful ledger can replay precisely; this review does not redefine that architecture. ADR E5's full wording supplies the context and needs no technical correction. The owner still confirms its corrected wording; reviewer approval is not that owner decision.

## Scope

**PASS**, git diff --name-status 908c391 db5bb59 and git rev-list --count 908c391..db5bb59 establish one commit and exactly eight files:

```text
core/registryd/src/anchor.rs
core/registryd/src/registry.rs
core/registryd/tests/storage_integrity.rs
docs/decisions/0006_m2_storage_anchor_recovery.md
docs/specs/m2_registry_storage_v1.md
docs/specs/m2_service_ipc_v1.md
tests/m2/harness/test_m2_registry_policy.py
tests/m2/test_inventory.txt
```

git diff --name-status db5bb59 84464cc confirms the docs-only follow-up. git diff --exit-code 908c391 db5bb59 -- Cargo.lock Cargo.toml .cargo build/m2 .github tests/m2/vectors core/registryd/src/ledger.rs core/registryd/src/journal.rs core/registryd/src/record.rs core/registryd/src/lineage.rs exits 0. No dependency, lock, pin, oracle, vector, workflow or Dockerfile changes.

## Independent test evidence

Evidence root R=/home/dev/cham3-r4-review. Clean qualification clone R/candidate created with git clone --no-hardlinks /home/dev/cham3 /home/dev/cham3-r4-review/candidate, then git -C R/candidate checkout --detach db5bb59fcf461034461d9e0d2f21f5816986134c. Reviewer-test clone made the same way; only additional test fixtures, never implementation files, were copied/added there.

Original fixture SHA-256 values, unchanged:

- review_r3_validation.rs: 5331ddd1356d4bc9d64ff4c0a64f26462051a24c631774efd775e4189bc9ec9b.
- review_r2_storage.rs: bd6e59153e9fa89dd1439b5addb31800d4a4f6bf1c127ea3240a5f582d84236f.
- queue_validation.rs: 2e1d442c68ab17b17438e28388d453a6d420a1ba8c8bc0b8112818dc52987953.
- reviewer_matrix.rs: e10c6e40a87431dcfb621ddafe63c16191165a070ebef6c97279a3b9c4352fce.

The R3 fixture came from the prior sealed scan's artifacts/review_r3_validation.rs. R2 fixtures came from the prior independent reviewer clone. No author fixture substituted for them. For each profile, exact commands: cargo test --frozen [--release] -p cham3_registryd --test review_r3_validation review_r3_ -- --nocapture --test-threads=1; substitute review_r2_storage review_r2_, queue_validation queue_, reviewer_matrix review_matrix. Brackets indicate the optional release flag, not literal shell arguments. Full stdout/stderr and every exit code retained in R/evidence/reviewer_tests.log; wrapper uses no grep/head pipeline to hide failures.

| Check | Result | Evidence / interpretation |
|---|---|---|
| Original R3 reviewer suite, debug/release | PASS | 12/12 each, exit 0. Prior independent 7/12 and author baseline reproduction identify the same five resolved operation failures. |
| Original R2 storage, debug/release | FAIL raw, expected | 5/6 each, exit 101. Only review_r2_compound_failure_must_preserve_restriction fails in E5's accepted no-mark case. |
| Original queue, debug/release | FAIL raw, expected | 3/4 each, exit 101. Only queue_expected_partial_flush_read_gate's obsolete anchored==before premise fails; new behavior anchors the appended record. |
| Original reviewer matrix, debug/release | PASS | 4/4 each, exit 0. Counts and named failures match author reviewer_reproductions logs. |
| Additional reviewer R4 tests, debug/release | PASS | 3/3 each, exits 0, R/evidence/reviewer_r4_tests.log and reviewer_tests/core/registryd/tests/review_r4_validation.rs. Nine denied-role operation cases plus all inspection accessors preserve counter/validity/running marker; subsequent authorized operation publishes both retries. 32 policy partial-recovery cases (two policy operations × 16 masks) verify containment only with an invalidation/anchor witness after initial all-write failure. Journal-only persistence followed by marker recovery, identity anchoring, release and repeated status reads keeps unverifiable without redundant marker writes. |
| Harness mutation scope | PASS | AST comparison: 87 → 97, exactly ten additions and only start_marker_ignores_failed_ledger re-pointed. Cargo inventory exactly 209. |
| Eleven changed mutation failure reasons | PASS | Each compiles and named test fails at its intended runtime assertion; artifacts/defect_reasons.log in the current security scan, SHA-256 4a35e4f01b90f1c65eccbaff5753d42fb82e251bdf5501d57c23c74166c8690d. |

Mutation reasons: policy_state, unusable_users, append_identity, journal_users, admit, set_policy_epoch and set_emergency_restriction omission leave validity intact in the corresponding operation case (append_identity is caught at idempotence, admit at unknown user, epoch at malformed input). retry_before_role_check violates the no-write assertion for a refused caller. unverifiable_not_sticky writes running through the emergency release. published_marker_written_again increments the counter unexpectedly on status. The re-pointed start marker mutation publishes running after startup ledger failure. None is merely a compiler failure. Full harness verifies unmodified positive controls for all 97.

## Qualification, static checks, filesystem and CI

Exact qualification/static wrapper:

```sh
docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 \
 -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
 -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
 -v /home/dev/cham3-r4-review/candidate:/work -w /work cham3-m2-independent-r3-final bash -c \
 'bash build/scripts/qualify_m2_rust.sh; qualification_result=$?; bash build/scripts/run_static_checks.sh; static_result=$?; sh tests/run_tests.sh static; tests_result=$?; printf "qualification=%s static=%s static_tests=%s\n" "$qualification_result" "$static_result" "$tests_result"'
```

Reviewer wrappers use the same UID/image/nofile/HOME, reviewer_tests:/work, CARGO_TARGET_DIR=/work/out/m2/reviewer_target and R/run_reviewer.sh read-only at /scratch/run_reviewer.sh. Additional tests use cargo test --frozen [--release] -p cham3_registryd --test review_r4_validation review_r4_ -- --nocapture --test-threads=1. R/evidence/reviewer_r4_tests.log records both exit codes.

Filesystem wrapper: docker run with the same clean clone/UID/image/nofile and --tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700; sh tests/m2/storage/check_reserve_full_disk.sh out/m2/storage; then CHAM3_M2_RESERVE_PROBE=/work/out/m2/storage/target/release/cham3_reserve_probe CHAM3_M2_WRAPPER_DISK=/full-disk python3 -I -m unittest discover -s tests/m2/harness -p test_m2_reserve_probe.py -k wrapper -v. **PASS**, probe=0 wrapper=0, five cases. Debug/release reports are byte-identical to downloaded CI and author VM reports. This exercises journal/ledger space exhaustion, not an anchor reserve or physical crash.

CI retrieved independently with gh api repos/Kayyo321/cham3/actions/runs/<id>, /jobs and /artifacts; gh run view 38592632636 --repo Kayyo321/cham3 --log; gh api repos/Kayyo321/cham3/actions/artifacts/11910945081/zip. **PASS**: m2 38592632636, m1 38592632234, m0 38592632553 all have exact candidate head_sha and successful jobs/steps. ZIP has 44 entries; downloaded SHA-256 f9d7bb8444648e0ffe486b7e9fb3f214a0e21841b13d4b9988ef7e16d6bd6671 matches both API digest and assignment. Metadata: tree 76b1e2a11cebb94c43be35852e944e79bf43f0fa, dirty_tracked 0, euid 1001, Cargo.lock SHA-256 9801453aa7023cffc1e3f5b1ec327c284d365c2ce4b9cdd4dc41efddee384c57.

Both package inventories byte-match the independent image/committed locks/CI: 612 installed lines, digest 536db59f53ad7fba906cd61a01130e0cd6509dfa0d346e400192fab24bc44530; 535 downloaded hashes, digest 30e909a67d00f651254776a5543e57bfade57f808fcaeaf7f25712e5c2ba7573. Provider/ECR/format/vault, unit inventories, harness counts, differential corpus and filesystem reports compared with author evidence and CI rather than comparing timestamps or image-layer IDs.

Vault protection positive path: **PASS in independently downloaded CI**, protection_verified, dumpable 0, core_limit 0, no unlocked mappings, exit_status 0. **NOT_RUN locally**; no host memlock limit raised. Real process death, kernel crash, power loss, actual deployed service/resource enforcement and arm64 qualification: **NOT_RUN**. Restarts/reboots are injected boundary faults and object drop/reopen. Miri, sanitizers and coverage-guided fuzzing: **NOT_RUN**, pinned toolchain reports NOT_AVAILABLE. No full-system guarantee inferred.

| Final gate | Result | Evidence |
|---|---|---|
| Clean amd64 qualification | PASS | m2 qualification: PASS; exit 0. R/candidate/out/m2/qualification/amd64_20261014T164634Z_6. Source identity: exact candidate/tree, dirty_tracked 0, UID 1001. |
| Workspace tests / inventory | PASS | 209 debug + 209 release, exit 0, inventory-exact. R/candidate/out/m2/runs/unit_20261014T164701Z_1849. Test names match downloaded CI and author JSON exactly. |
| Harness / seeded controls | PASS | 147 tests in 382.322 s, exit 0; unmodified positive controls and all 97 registry/policy mutations detected. Harness run harness_20261014T164842Z_4474; exact names match CI and author JSON. |
| Locks, pins, sources, formatting, clippy, naming, profiles | PASS | Every qualification check PASS. Both inventories and final qualification summary byte-identical to downloaded CI. |
| Provider / ECR / format / vault | PASS | Provider known-answer oracle PASS; ECR 19 valid/96 invalid/18 admission vectors, 23 scenarios/352 steps; format 5955 cases/0 mismatches, corpus 0ad8bc2b1e8d39224de699db64aa45ca7adcf70117c555f35b57c537471d395e; vault 38 checks. |
| Valgrind | PASS | Probe zero errors; secure-memory 10 tests, zero errors; child fault processes not instrumented, as explicitly recorded. |
| Static checks / static tests | PASS | static checks: PASS, exit 0; 82 tests in 25.974 s, OK. Aggregate qualification=0 static=0 static_tests=0. |
| Full filesystem / five wrappers | PASS | probe=0 wrapper=0, 5/5; reports byte-match author and downloaded CI. |
| CI comparison / artifact | PASS | Exact heads, successful jobs/steps, downloaded ZIP digest verified; vault protection positive path PASS in CI. |
| Clean candidate after all checks | PASS | git -C R/candidate status --porcelain empty. Implementation checkout remains clean at 84464cc. |

Full exact local log digests:

- qualification_console.txt: 186cbbb131be760f7f811011b4d8393e1d0dd3b78c3fa71ce52ace97d9161ba7.
- reviewer_tests.log: 370e14de2890340233dffef9afb015fa63f059faab77f13e296eebea21ef4c79.
- reviewer_r4_tests.log: 60bd806349d6d2fe36df2d287c2f07481944e7c409f14e8d6e4cd763e8e6e5e6.
- full_disk_wrapper.log: 1971d4e5818b94e87bf3b3eaa9762f7e56dbceb813c4093d597d5c3aa3ebbfe7.
- Additional review_r4_validation.rs: fa368e8d53e5cefd8bd9cf848664f03e12c9687b9fc28a6543455486836f8fd2.

## Codex Security plugin and handoff

**PASS**, completed and read back using security-diff-scan, producer codex-security-plugin 0.1.32. Scan d1d2eab1-095e-4a7c-a229-56169e960a53, exact 908c391..db5bb59, sealed 2026-10-14T16:57:48.589838Z. Zero discovery candidates and zero findings; no candidate validation or attack-path phase required. Both production files in the plugin inventory inspected, plus all eight changed files and the docs-only follow-up for this project review. A fresh-context read-only architecture worker provided bounded assistance; the parent verified the resource mapping and performed source/test review.

Preflight command: python3 /home/dev/.codex/plugins/cache/openai-curated-remote/codex-security/0.1.32/scripts/config_preflight.py --profile security_diff_scan --cwd /home/dev/cham3 --runtime-check delegation_available=true --runtime-check goal_tools_available=true --multi-agent-runtime-owner native --multi-agent-runtime-version v2 --multi-agent-session-cap 4 --multi-agent-runtime-provenance thread-context. Exit 0, ready, all three checks PASS, no persistent configuration changes. Used start_codex_security_prompt_only_scan, progress checkpoints, prepare/list_codex_security_review_items, record_codex_security_discovery_candidates, semantic drafts, artifact saves, completion and completed-scan readback. The retained threat model distinguishes Stage A composition from actual process authentication, deployment placement and enforcement.

Procedural correction: the first final semantic draft was rejected because readback-only coverage.mode/includePaths/excludePaths/inventoryStrategy were passed back as input. The subsequent completion attempt correctly refused the incomplete draft. That response stopped as required. On continuation, the same running scan was inspected, only supported semantic fields were submitted, and the accepted complete draft was then sealed successfully and read back. No replacement scan, changed implementation, waived gate or invented completion result.

Canonical scan-manifest.json, findings.json, coverage.json, threatmodel.md, report.md and exports/results.sarif are retained under:

```text
/home/dev/.codex/state/plugins/codex-security/scans/cham3/db5bb59fcf461034461d9e0d2f21f5816986134c_20261014T164605Z_q2q79kkk
```

Supplemental artifacts/independent_evidence.tar.gz retains exact qualification/test logs and metadata, all original reviewer fixtures, the additional R4 fixture, CI API/job metadata/log/ZIP and wrapper logs without build/dependency trees. SHA-256 bcfd926a4a8aa9347e705ca89d9b3af442caac00a4c8b1fd1b1aaf7fff1ed7ef. Mutation runtime reasons remain separately at artifacts/defect_reasons.log. Tool-measured cumulative rollout usage at completion: 5,434,907 total tokens across two threads; 5,404,836 input including 5,147,136 cached, 30,071 output. This is tool accounting, not a cost estimate.

Owner question: confirm ADR 0006 §3 E5's corrected wording. The reviewer finds it technically accurate; this does not accept new risk or re-select Option A. Coordinator retains stage/completion/integration authority. Stage B stays paused pending the owner's disposition. No implementation edits, merges or force-pushes.
