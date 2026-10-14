# M2-006 stage A revision 3 — independent Codex review

**Verdict: CHANGES_REQUESTED.** R2-01, R2-02 and R2-04 are resolved. The owner-selected E5 exception accurately describes the no-durable-mark loss case, but its promised retry mitigation is incomplete and one stated recovery consequence is too broad. Findings R3-01 and R3-02 below require focused correction. This review makes no owner decision and does not open stage B.

- Reviewer: Codex, independent REVIEWER; no implementation authored or modified.
- Base: fc147e0f80188a82a761ffe203a6ac7704ea25f9.
- Candidate: 2217e62ddf42c42573346155727c983d937addc0; code 9633ce30c212cd2cd3515b9a13d7227d1f9f9796; locks a2d0ebd; erratum 2217e62.
- Docs reviewed: 908c3911fd08ccf91d4ba6706fbad49d45eaf1a8, direct child of candidate, docs-only.
- Previous review: review/m2-006-a-r2-security @ 7d570c1ce5b14ce3b37836775881c2013aa051a4, read directly from Git without merging it.
- Local implementation clone: /home/dev/cham3, clean at 908c391 after requested fetch/checkout/reset; no stashes or local changes moved, discarded or created.
- Publication: only this record on review/m2-006-a-r3-security, based on origin/main @ 9cf882503848f8c6cc4938a588cc9fc062a9392d; separate worktree /home/dev/cham3-m2006a-r3-independent/record.
- Authority: PROJECT.md, approved ADRs and recorded owner choices, task scope, AGENTS.md, workflow/testing/security guidance. Author resolutions, handoff and evidence were independently checked.
- Environment: native Debian 13.7, x86_64, kernel 7.1.13+deb13-amd64, UID/GID 1001. Both `docker ps` and `sg docker -c 'docker ps'` succeeded. No host protections changed. Disposable containers use UID 1001, no added capabilities, and container-only nofile 1024:1024.

## Findings

### M2-006-A-R3-01 — Medium — Several operations skip the owed restart-safety retries

Root control: core/registryd/src/registry.rs:1298, the new retry_unverifiable helper and its incomplete dispatch. Affected paths: policy_state :1095; append_identity :1406, its already_present return :1436 and new-user failed-ledger return :1513; journal_users :1613; set_emergency_restriction(false) :1752 and release_policy :1788.

E5, storage formats §§4/6/7, and the revision-3 acceptance scope explicitly promise retrying validity invalidation and the unverifiable marker on every registry operation. These paths bypass the helper. Calls through ready do not cover identity idempotence, new-user refusal, journal enumeration, policy reads or emergency release.

Independent reproduction, in both profiles:

1. Admit a synthetic user's normal grant.
2. Fail ledger append, validity unlink, validity emptying, anchor writes and journal writes; restrict returns restriction_not_durable.
3. Clear **all** faults before calling one affected operation. No intervening rollback_status, lineage_head or identity_state is called.
4. Read actual files: validity remains valid and marker remains running. Drop the registry and reopen in the same boot.
5. The run is ledger_replayed and the grant is current: the restriction is lost.

All five operation-family tests reproduce this. New enrollment returns storage_unavailable before retrying; idempotent identity returns success without retrying; emergency release even writes a new durable anchor image retaining running. Controls using lineage_head and identity_state invalidate the ledger, publish unverifiable, remain refused for the failed run, and contain on restart.

This does **not** reject the owner's accepted all-marks-failed residual: death before any successful mark still fits its exclusion. The defect is that storage recovery plus an authorized operation does not perform the promised mitigation, unnecessarily prolonging that residual window. These are fault-injection recovery findings; no ordinary application's ability to induce the compound faults is claimed.

**Required:** dispatch both owed retries at supported operation entry points before early returns, respecting lock order and role checks; audit all paths rather than relying only on ready. Add discriminating tests for idempotent identity, failed new enrollment and emergency release as well as read/enumeration operations. Keep denial, failed-ledger refusal and shutdown refusal intact.

Evidence: artifacts/review_r3_validation.rs, review_r3_retry_*; artifacts/r3_final_tests.log. Per profile: five failing mitigation assertions and two passing retry controls.

### M2-006-A-R3-02 — Low — E5 equates every successful retry with containment

Location: docs/decisions/0006_m2_storage_anchor_recovery.md:69; matching claim docs/specs/m2_registry_storage_v1.md:98. The resolutions also say every one of the writes is retried on every operation.

E5 says “from the first write that succeeds, a same-boot restart contains.” A journal-only retry is a counterexample. After initial all-write failure, allow journal writes while ledger, both invalidations and anchor remain failed. shutdown refuses an orderly shutdown but flushes the restriction into the journal. Drop and reopen same boot: the one unanchored tail rolls forward, the run is ledger_replayed, and the exact restriction survives; the user is **not contained**.

The behavior is safe and consistent with the recovery table. The consequence sentence is inaccurate. Independent review_r3_only_journal_retry_preserves_without_containment passes in both profiles and explicitly asserts precise restriction retention without containment. Each individual invalidation/marker recovery and the new-boot control separately produces containment.

**Required:** correct the ADR/spec/handoff claim to distinguish successful validity invalidation or unverifiable-marker publication (containment) from successful restrictive journal persistence (precise replay/roll-forward). Do not change the recovery architecture to force containment merely to fit the sentence. The owner must confirm the corrected E5 wording.

Suggested factual wording for owner consideration: “A successful retry of validity invalidation or the unverifiable marker makes the next same-boot start contain. A restriction whose journal record becomes durable instead survives through the normal recovery/replay path; that alone need not contain the user.”

## Scope and prior findings

**PASS**, exactly the declared 13 changed files; three commits in fc147e0..2217e62:

```text
build/m2/debian_debs_amd64.sha256
build/m2/debian_lock_amd64.txt
core/registryd/src/anchor.rs
core/registryd/src/ledger.rs
core/registryd/src/registry.rs
core/registryd/tests/storage_integrity.rs
docs/decisions/0003_m2_rust_toolchain_dependencies.md
docs/decisions/0006_m2_storage_anchor_recovery.md
docs/decisions/README.md
docs/specs/m2_registry_storage_v1.md
docs/specs/m2_service_ipc_v1.md
tests/m2/harness/test_m2_registry_policy.py
tests/m2/test_inventory.txt
```

`git diff --name-status fc147e0 2217e62`, `git rev-list --count fc147e0..2217e62` and `git diff --name-status 2217e62 908c391` establish the scope and docs-only follow-up. An explicit `git diff --exit-code` over Cargo.lock, Cargo.toml, .cargo, pinned packages, Dockerfile, crate source manifest, workflows, oracle and vectors exits 0. No dependency, manifest, pinned version, oracle, vector, workflow or Dockerfile change.

| Item | Result | Evidence and limits |
|---|---|---|
| R2-01 rejected slots | PASS | Counter continues above highest readable rejected counter; equal counters remain rejected; quarantine anchored at startup. Original test passes. Author's both-slot-order/two-invalid/clean/unclean cases pass in qualification. Independent two-user, differing equal-counter images, both orders, eight starts and overflow cases pass. Checked overflow refuses rather than wrapping. |
| R2-02 startup marker | PASS | Startup ledger failure publishes unverifiable or refuses; mismatched old marker/new ledger then contains. Original startup tests pass with intact validity and failed journal. Author's all-write start refusal passes. contain_everyone also uses restrictive_write; failed containment ledger cannot publish running. |
| R2-03 E5 core exclusion | PASS for bounded marks matrix | All 32 combinations of ledger/unlink/empty/anchor/journal failure: only all five failed boundaries lose the restriction on immediate same-boot restart. Three individually recovered invalidation/marker witnesses and reboot contain. Existing residual test fails only in the excluded case. |
| R2-03 complete mitigation/consequences | FAIL | R3-01 omitted retries and R3-02 consequence wording. Status correctly reports owed state, existing retry paths work, no eligible lineage decision is served and shutdown refuses while ledger failed. |
| R2-04 queue | PASS | Each append is anchored before next; journal failure leaves prior records anchored. Original bound/restart tests pass. Independent eight-record queue, journal/anchor failure at positions 0–8, reads and repeated restarts: all 18 parameter cases pass per profile. Early quota/sequence/append failures append nothing; anchor failure leaves at most one tail. |
| Unchanged reviewer tests | FAIL raw suites, explained | storage 5/6, queue 3/4, matrix 4/4 in debug and release. Storage failure is E5's accepted no-mark case. Queue failure is the old premise anchored==before (actual 5, expected 4); appended record is now anchored. The longer matrix verifies reads only at the anchored sequence, or refusal when one tail is unanchored. These two raw failures alone are not new findings. |
| Additional reviewer tests | FAIL overall, discriminating | 7 pass / 5 fail per profile; 32 marks cases, 18 queue cases, repeated-anchor/overflow matrix, three witnesses/reboot and journal-only recovery pass. Five failures reproduce R3-01. |

## Exact execution and evidence

Local evidence root `R=/home/dev/cham3-m2006a-r3-independent`. Clean qualification clone `R/qual` was made with:

```sh
git clone --no-hardlinks /home/dev/cham3 /home/dev/cham3-m2006a-r3-independent/qual
git -C /home/dev/cham3-m2006a-r3-independent/qual checkout --detach 2217e62ddf42c42573346155727c983d937addc0
docker build --no-cache --network host -f build/m2/dev_container.dockerfile -t cham3-m2-independent-r3-final build/m2
```

Image id: sha256:4528aed8d303c7255b9138392a4b2f7973a1b944b140cac66a9da40360ff3e56. Independently rebuilt layers have their own image id; **the two package inventories**, not image-layer byte identity, were compared.

Qualification/static invocation (outer stdout/stderr saved in R/evidence/qualification_console.txt):

```sh
docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 \
 -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
 -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
 -v /home/dev/cham3-m2006a-r3-independent/qual:/work -w /work cham3-m2-independent-r3-final bash -c \
 'bash build/scripts/qualify_m2_rust.sh; qualification_result=$?; bash build/scripts/run_static_checks.sh; static_result=$?; sh tests/run_tests.sh static; tests_result=$?; printf "qualification=%s static=%s static_tests=%s\n" "$qualification_result" "$static_result" "$tests_result"'
```

Qualification directory: R/qual/out/m2/qualification/amd64_20261014T152015Z_7. Unit: unit_20261014T152043Z_1850. Harness: harness_20261014T152221Z_4469. Source identity: candidate, tree 5f82bac03e478fc258084ab48f19d33141a0a80b, dirty_tracked 0, euid 1001, unchanged Cargo.lock SHA-256 9801453aa7023cffc1e3f5b1ec327c284d365c2ce4b9cdd4dc41efddee384c57.

Unchanged reviewer fixtures were copied into a separate detached candidate clone R/reviewer_tests, with no implementation changes. Digests:

- review_r2_storage.rs: bd6e59153e9fa89dd1439b5addb31800d4a4f6bf1c127ea3240a5f582d84236f.
- queue_validation.rs (e34fd8a storage_integrity plus original additions): 2e1d442c68ab17b17438e28388d453a6d420a1ba8c8bc0b8112818dc52987953.
- reviewer_matrix.rs: e10c6e40a87431dcfb621ddafe63c16191165a070ebef6c97279a3b9c4352fce.

Exact wrapper: docker run with the same UID/nofile/HOME, CARGO_TARGET_DIR=/work/out/m2/reviewer_target, R/reviewer_tests:/work, and the checkout's reviewer_reproductions directory mounted /scratch:ro; `bash /scratch/inner_reviewer_r2.sh`. It invokes `cargo test --frozen -p cham3_registryd --test review_r2_storage review_r2_ -- --nocapture`, `--test queue_validation queue_`, and `--test reviewer_matrix review_matrix`, each debug and --release. R/evidence/reviewer_tests.log SHA-256 aa568f867a207276f81ea9380de9f2169473812ecf423d2e761729b108c85ec0. Results match the author's candidate reproduction log by counts and named assertion failures, not timestamps/log digests.

New tests live only in the plugin-prepared disposable clone; unchanged candidate storage_integrity helpers plus review_r3_validation.rs. Run with the same container settings and target variable, mounted /work:

```sh
cargo test --frozen -p cham3_registryd --test review_r3_validation review_r3_ -- --nocapture
cargo test --frozen --release -p cham3_registryd --test review_r3_validation review_r3_ -- --nocapture
```

Both exit 101 for the five expected R3-01 failures. Source SHA-256 5331ddd1356d4bc9d64ff4c0a64f26462051a24c631774efd775e4189bc9ec9b; final log b0ae9d1788c9b9b011091a24007d50da13e69447be2df1287a4e3927018eebbf. An initial smaller test run had the same five failures; the final file adds the anchor/witness/journal controls. No implementation file was edited.

Full filesystem and wrapper: same clean clone/container, `--tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700`; `sh tests/m2/storage/check_reserve_full_disk.sh out/m2/storage`, then `CHAM3_M2_RESERVE_PROBE=/work/out/m2/storage/target/release/cham3_reserve_probe CHAM3_M2_WRAPPER_DISK=/full-disk python3 -I -m unittest discover -s tests/m2/harness -p test_m2_reserve_probe.py -k wrapper -v`. Both exit 0; wrapper 5/5. R/evidence/full_disk_wrapper.log digest 472687ab457d50dcab1289d38c4894af1300d4b646f9e20bfd0073f133815ecb. Debug/release reports are byte-identical to CI and author reports. This probes journal/ledger space failure, not an anchor reserve.

Admission differential: same reviewer container, `cargo test --frozen -p cham3_registryd --test admission_scenarios -- --nocapture`: PASS, four tests, 11 scenarios / 59 steps / 105 comparisons / zero disagreements.

## Lock refresh, harness and CI

**PASS** lock refresh. Exactly four changed lines: two libcurl version lines deb13u5 → deb13u7 and two corresponding .deb hash lines. Fresh observed inventories are byte-identical to both committed locks and both independently downloaded CI artifacts:

- installed set: 612 lines, SHA-256 536db59f53ad7fba906cd61a01130e0cd6509dfa0d346e400192fab24bc44530.
- downloaded hashes: 535 lines, SHA-256 30e909a67d00f651254776a5543e57bfade57f808fcaeaf7f25712e5c2ba7573.

ADR 0003 §2 correctly records the owner-approved refresh; no pinned inputs changed. arm64 remains unqualified for this refresh.

**PASS** seeded-defect discrimination within its actual scope: AST comparison is 81 → 87 mutations, exactly six added and exactly two existing tuples changed. Each new test must fail at runtime under its named mutation and its baseline must pass. Counter reset and omitted anchored quarantine are caught by repeated starts; startup running overwrite by startup ledger failure; invalidation/marker retry omission by fault recovery plus direct file/status assertions; batch anchoring by tail/read/restart checks. The new retry mutations cover the helper behavior, not every caller: R3-01 demonstrates that coverage gap. Inventory is exactly 206 cargo tests.

CI commands: `gh api repos/Kayyo321/cham3/actions/runs/<id>`, with /jobs and /artifacts; `gh run view <id> --repo Kayyo321/cham3 --log`; `gh api repos/Kayyo321/cham3/actions/artifacts/<id>/zip`, then sha256sum and ZIP inspection. Responses/logs/downloads retained under R/evidence.

**PASS** comparison: all jobs/steps at candidate succeed; m2 38584362113, m1 38584362062, m0 38584362077 have exact candidate head_sha. At 9633ce3, m2 38577923955 fails only installed_packages_lock and downloaded_debs_lock; every other qualification check passes and downstream protection/storage/wrapper steps are skipped. Its m1 38577923430 and m0 38577923571 succeed.

Candidate ZIP: artifact 11906127702, 44 entries, SHA-256 1ed83981cfbe186c42969c96760b96f8bfc189dc2d348d5c78a777078de3937a. Previous ZIP: 11902319817, 41 entries, 1f589df6a09b43bf8d8162b77fa87fdccad3a40962e8c30509fe11634ff444dc. Both match API and assignment digests. Candidate artifact identifies dirty_tracked 0, the same tree/Cargo.lock, unit inventory, differential corpus, locks and filesystem reports as the independent run.

Vault protection positive path: **PASS in independently retrieved CI**, protection_verified and exit 0; **NOT_RUN locally** (no raised host memlock or host change). Physical process death, power loss, kernel crash, real deployed enforcement, arm64 qualification, Miri, sanitizers and coverage-guided fuzzing: **NOT_RUN**; pinned toolchain labels the latter three NOT_AVAILABLE. All local restart tests use injected boundary failures and object drop/reopen.

## Completion and owner handoff

Review state: **M2-006 stage A revision 3 — CHANGES_REQUESTED**. Coordinator returns R3-01 to implementer and obtains corrected E5 wording for R3-02; re-review an immutable focused candidate. Implementation clone and clean qualification clone remain unchanged. No merge, force-push, implementation edit, status update on the implementation branch or stage-B approval.

Owner question: confirm E5 after distinguishing invalidation/marker containment from journal-only precise recovery. Option A itself is already recorded; this reviewer does not re-select an option or accept new risk. Global policy restrictions use ledger/anchor rather than their own per-user journal record, so the final wording should avoid implying every restriction class has four literal write sites.

Final qualification results, Codex Security completion metadata and retained evidence digest follow below.

## Final verification results

| Check | Result | Evidence |
|---|---|---|
| Clean pinned amd64 qualification | PASS | m2 qualification: PASS, exit 0; both locks, pins, sources, naming, formatting, clippy, profiles and builds pass. |
| Cargo inventory | PASS | 206 debug + 206 release, exact committed inventory. |
| Provider / ECR / format / vault | PASS | Provider known answers; ECR 19/96/18 with 23 scenarios/352 steps; differential 5955/0, corpus 0ad8bc2b1e8d39224de699db64aa45ca7adcf70117c555f35b57c537471d395e; vault 38. |
| Harness | PASS | 147 tests in 362.317 s, including 87 registry/policy, 21 identity/vault seeded defects and 12 reserve-probe tests. |
| Valgrind | PASS | Probe zero errors; secure-memory 10 tests, zero errors, with recorded child-process instrumentation limit. |
| Static checks / static tests | PASS | static checks: PASS; 82 tests in 25.527 s, OK; aggregate qualification=0 static=0 static_tests=0. |
| Full filesystem / wrappers | PASS | Unprivileged 2 MiB tmpfs; both profiles, 5/5 wrappers; probe=0 wrapper=0. |
| Reviewer suites | FAIL raw, explained | storage 5/6, queue 3/4, matrix 4/4 in both profiles; two expected old assertions are classified above. |
| New targeted mitigation tests | FAIL | 7/12 pass per profile; five skipped-retry findings reproduced. |
| CI and both artifact digests | PASS | Exact heads, all candidate jobs/steps successful; prior lock-only failure confirmed. |
| Unmodified clone | PASS | git status --porcelain empty after qualification/static tests. |
| Full-system / physical-crash / arm64 / nightly-only tools | NOT_RUN | Stage-A fault injection and native amd64 component evidence only. |

## Codex Security plugin result

**PASS**, completed and read back. Producer: codex-security-plugin 0.1.32; skill security-diff-scan. Scan 61f0da61-d708-4b93-8ae2-dc46e1e2584d; exact fc147e0..2217e62; completed at 2026-10-14T15:31:41.825017Z.

Three capability preflight checks passed without persistent config changes. The plugin inventoried three production Rust files; this project review additionally accounted for all 13 changed files and the documentation follow-up. A fresh-context architecture worker and dedicated preflight worker supplied bounded read-only assistance; the parent inspected all source and executed validation. Threat model, discovery, validation, attack-path analysis, final complete draft and one successful finalization are retained.

Five independently addressable skipped-retry candidates were technically validated. Attack-path analysis promotes **zero exploit-oriented findings**: no lower-trust attacker trigger for the compound I/O-failure/process-death prerequisites was established. This disposition does not erase the reproductions or waive E5's explicit mitigation requirements. The project review retains Medium R3-01 and Low R3-02 and requests changes.

The initial draft input was rejected before writing because exclusions needed pattern instead of path; corrected once in the same scan. An invalid progress phase name was also corrected. No failed scan or replacement scan was created. A mechanical AST literal-evaluation attempt encountered symbolic constants; it was replaced with AST-structure comparison. No source or test condition was weakened.

Canonical artifacts: scan-manifest.json, findings.json, coverage.json, threatmodel.md, report.md and exports/results.sarif under:

```text
/home/dev/.codex/state/plugins/codex-security/scans/cham3/2217e62ddf42c42573346155727c983d937addc0_20261014T151911Z_v1bn714q
```

Retained supplemental evidence: artifacts/review_r3_validation.rs and artifacts/r3_final_tests.log, with digests above; artifacts/independent_evidence.tar.gz contains 73 selected exact logs/metadata, both CI ZIPs and original reviewer fixtures, no build/dependency trees. Archive SHA-256: d3d715fc7ef6d06c4d2b54b128d724f21e0d103550b6203a10322c9096f2b05f.

Tool-measured rollout usage (complete measurement, three threads): 7,149,246 total tokens; 7,114,741 input, including 6,605,312 cached input; 34,505 output. This is cumulative tool accounting, not a cost estimate.
