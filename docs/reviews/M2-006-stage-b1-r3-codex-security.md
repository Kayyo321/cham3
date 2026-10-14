# M2-006 stage B1 revision 3 — independent Codex re-review

**Verdict: APPROVE — M2-006 stage B1 revision 3.** Bounded independent REVIEWER / Codex review of M2-006-B1-R2-01; no implementation authored or edited. R2-01 is resolved, required component gates pass, and there are no new findings or owner questions. Approval is scoped to this B1 candidate and the stated evidence limits; it makes no owner decision and performs no merge.

- Authority read from the requested current checkout: AGENTS.md, PROJECT.md (in particular §§3, 5.2–5.7, 14–15), workflow, orchestration, roles, testing and security invariants, status/task, revision-3 resolutions/handoff, approved ADR 0007 §5, §8 Option B, E6 and its owner clarification, storage specification §8. Author statements were verified against source, independently executed controls, and downloaded CI evidence.
- Exact candidate: `5ea2407ff2c639361ec2976bc20bc95f5acbce95`, one commit on `0ec854de46af32f0e6625d0c59deace6e3d4ba9a`.
- Exact docs follow-up: `16944ab29bb6406ce0ed08210a6ef326c2712ee3`, parent exactly candidate; 51 changed paths, all under `docs/`, no implementation changes.
- Owner clone `/home/dev/cham3`: clean before step 0 and after checkout/reset to `16944ab`; no stashes, no changes moved aside. It remains on the implementation branch at that docs commit. No implementation-branch edits/push, merge or force-push.
- Prior independent record read at `9b5afa2e3dfd50ce9c25b8d51662108fda90b58a`; SHA-256 `1537a5c331c5e18dc52b198157ad926ac729de12a73751642ab1c35da011a78d`. Original B1 record at `792f5f0f8836f63f5823e9e98c514bb7b2df93ff`; SHA-256 `534a20807969d586f7c80b59cae0b934060aae1922eed2e7deed9f1584f3e5f9`. Both independently verified with `git show`/`sha256sum`.
- Publication: `review/m2-006-b1-r3-security`, dedicated worktree `/home/dev/cham3-review-m2-006-b1-r3-security`, base `origin/main` = `9cf882503848f8c6cc4938a588cc9fc062a9392d`. Adds only this record. The publication commit and record digest are supplied in the final handoff (a record cannot contain its own commit/digest).
- Environment: Debian 13.7 x86_64, Linux `7.1.13+deb13-amd64`, UID/GID 1001. Direct Docker access through existing group 119, `docker version` succeeds. Image `cham3-m2-independent-r3-final` = `sha256:4528aed8d303c7255b9138392a4b2f7973a1b944b140cac66a9da40360ff3e56`. Containers run unprivileged as `1001:1001`, `--ulimit nofile=1024:1024`; host nofile 524288. No added capabilities, privileged host changes, host clock changes or production secrets.

## Finding disposition and source analysis

**M2-006-B1-R2-01: PASS, resolved.** `trusted_clock_t::start` first resumes the maximum same-boot anomaly mark and computes the original maximum offsets, then calls `clock.observe(sample, false)` at `core/registryd/src/clock.rs:296`. This counts the startup's own observation under approved E6. At exactly 300 seconds behind it closes the resumed episode; at 301 seconds it continues one, or opens the next after a closure. A restart with an anomalous sample does not close an episode.

Startup consumes registryd's own time source at `core/registryd/src/registry.rs:846`. The sole production caller of the changed helper is `:849`. The constructor takes the resulting floor at `:983`, publishes it with the run marker through `anchor.update` at `:986–992`, propagates errors, and only returns a usable registry at `:994`. The unchanged anchor writer replaces/synchronizes the alternate slot, then synchronizes its first directory entry in this run before changing the in-memory map (`core/registryd/src/anchor.rs:610–640`; `journal.rs:117–125,164–165`). Thus startup need not duplicate this transition into the new ledger: the compulsory run-marker anchor write carries it before serving.

Independent controls establish both the successful publication and refusal paths. The unchanged R2 matrix covers clean/unclean × anomalous/in-range startup, and all four cases pass in both profiles. The candidate integration regression checks the persisted mark immediately, then drops the process, renews the lag and starts again: the closed episode survives that further unclean restart. The additional reviewer fault control crosses clean/unclean with failed slot replacement/failed anchor-directory sync: all four starts return `Err(registry_start_error_t::store(_))` in debug and release. The replacement fault decodes the proposed anchor and checks mark 3 before refusing it; the sync fault confirms the intended directory path was reached. These are real production-source calls with injected I/O faults, not physical power-loss tests.

**Continuity, new boot and containment: PASS.** Same-boot maximum mark merge, ledger-only closure, replayed membership and equal-payload episodes for two users still pass in the unchanged supplemental controls. The changed new-boot unit expectation is correct: marks from old boots are not resumed, but a lagging startup's own sample immediately opens episode 1 of the new boot (mark 2); an in-range startup opens none (mark 0). This is initialization followed by observation, not a relaxation of boot reset. Existing new-boot integration tests still terminate temporary branches. Contained startup ignores untrusted ledger floors; invalid-anchor startup quarantines users and preserves their journal bytes. Both unchanged supplemental startup controls pass.

**Offset and authorization effect: PASS within the component scope.** The startup offset already includes `realtime − boottime` or a greater persisted floor. `raised=false` prevents `observe` from adopting a raise; `last_boottime` already equals the same sample; `observed_offset` is untouched (`clock.rs:257–296,324–345`). Anomaly mark is absent from `time_view_t` and temporal authorization arithmetic (`:178–194`). Replay adds anomaly events only, with no key, policy or grant mutation (`registry.rs:590–595,1137–1143`). The original B1 controls, eight supplemental policy-refusal combinations per profile, and unchanged differentials continue to pass. No claim of identical availability under failed I/O is made; refusing startup is required.

## Acceptance evidence

PASS means the stated component/container or CI evidence only. C0–C9 below identify exact commands and retained output.

| Check | Result | Evidence and limits |
|---|---|---|
| Startup, history, exact scope, protected inputs | PASS | C0: candidate one commit/five files; follow-up 51 docs paths with correct parent; whitespace checks and unchanged dependencies/workflows/oracles/vectors/other source pass. |
| Original reviewer controls, unchanged | PASS | C5: policyd 5/5 and registryd 9/9, debug and release, 14/14 each; exact original snippet digests match author copies. |
| Three supplemental snippets composed as prior C6 | PASS | C5: policyd 1/1, registryd 4/4, debug and release; includes eight policy combinations and four startup cases each. Snippet bytes and startup observations match author's candidate logs. |
| Baseline startup contrast | PASS evidence comparison | Prior independent record and author's `r2_at_77a3d0e.log` agree: two in-range cases fail, anomalous cases pass in each profile. Baseline execution NOT_RUN again in this bounded assignment; exact candidate controls were independently rerun. |
| R2-01 start sample and further unclean restart | PASS | C5 and C1 candidate integration/unit tests; marks 3 versus 2 before any following user operation; later episodes [1,2] versus [1]. |
| Durability/refused anchor publication | PASS | C6: 1 test with four fault cases per profile (eight cases total), production constructor returns error before serving. |
| New boot, contained/invalid-anchor, offsets, authority | PASS scoped | C1/C5 source and controls, as detailed above. Full-system enforcement is NOT_RUN. |
| New mutation discrimination | PASS | C7: 124 definitions; `startup_sample_not_reconciled` compiles and fails intended `trusted_time.rs:764` assertion, mark `Some(2)` instead of `Some(3)`; unmodified control passes. Author's reason log has same failure. |
| Full clean-clone qualification and harness | PASS | C1 exit0, `m2 qualification: PASS`; clean source at exact candidate, unit241+241, harness149, all124 seeded registry/policy mutations detected with unmodified controls passing. Harness names match author and CI exactly. |
| Unit inventory | PASS | C1: 241 debug + 241 release, exact test names match author and CI receipts; two additions match inventory. |
| Oracles and differentials | PASS | C1/C4: 19 valid, 96 invalid, 18 admission vectors; ECR 27 scenarios/365 steps; trusted-time vectors165 and differential4813/0; layer7 15 scenarios/72 steps/121 comparisons/0; format5968/0. |
| Qualification static/toolchain/dependencies | PASS | C1 completed checks: package pins, both Debian locks, offline metadata, 1454 source manifest files, rustfmt/clippy/naming, profiles, debug/release builds and soft backends. |
| Static checks; static tests | PASS | C2 exit0; shellcheck/py_compile/yamllint/secret scan pass; 82 tests OK. |
| Full-filesystem wrapper and five cases | PASS | C3 exit0 on actual disposable2MiB tmpfs; debug/release reports byte-identical to author and downloaded CI; wrapper5/5. This is not anchor-ENOSPC or power-loss proof. |
| CI run/job/step comparison and artifact | PASS | C8: exact candidate, completed/success for all three runs; seven jobs/88 steps all success. Downloaded ZIP11931692301,110092bytes, digest exact. Unit names and storage reports agree with local and author. |
| Vault protection/provider evidence | PASS scoped | CI positive protection report `protection_verified`, exit0, dumpable0, unlocked_mappings0; local qualification Memcheck probe and secure_memory10tests/0errors pass. Local positive protection path NOT_RUN; Argon2 secret/AD oracle API limit carried forward. |
| Codex Security plugin | PASS execution | C9, codex-security0.1.32, completed/sealed; one production inventory file plus all four other changed paths accounted, six coverage surfaces, zero candidates/findings. |
| Real clocks/suspend/sync/reboot/crash; deployed IPC/enforcement | NOT_RUN | Settable time sources and I/O fault controls are component simulations; hardware/transport/later-milestone verification is outside this bounded re-review. |
| Miri/sanitizers/coverage-guided fuzzing | NOT_RUN | Qualification says NOT_AVAILABLE under pinned Debian stable toolchain; never counted PASS. |
| Implementation fixes/merge/owner architecture or risk decision | NOT_RUN intentionally | Review publication only. No owner decision made. |

The new mutation discriminates the actual omitted startup reconciliation, rather than compilation failure. Its regression's anomalous-start cases are positive continuity controls; its in-range cases assert closure and the following renewed episode. The independently replayed unchanged reviewer matrix confirms all four cases instead of relying solely on the first mutation failure. This mutation does not claim coverage of every possible startup corruption.

The trusted-time differential explicitly omits the anomaly mark, because the unchanged model has no episodes. Its zero disagreements prove the unchanged trusted/observed arithmetic over its corpus, not startup audit durability; the integration and reviewer controls provide the latter evidence.

## Exact scope

```text
core/registryd/src/clock.rs
core/registryd/tests/trusted_time.rs
tests/m2/harness/test_m2_registry_policy.py
tests/m2/test_inventory.txt
docs/specs/m2_registry_storage_v1.md
```

No other path changed in `0ec854d..5ea2407`. The docs follow-up preserves implementation bytes. Its status/task/handoff/resolution/evidence updates accurately describe author implementation and review-pending status; they do not independently approve B1. No ADR, dependency, lock, workflow, Dockerfile, oracle or vector changes.

## Commands and evidence

Independent execution output: `/tmp/cham3-b1-r3-evidence`; retained local evidence destination: `/home/dev/cham3-m2-b1-r3-review-evidence`. Clean qualification clone `/tmp/cham3-m2-b1-r3-independent-qual`; reviewer fixture clone `/tmp/cham3-m2-b1-r3-reviewer`. Both were made from the owner checkout after step0 using `--no-hardlinks`, detached at exact candidate. Reviewer clone adds only untracked integration test files and ignored build outputs; tracked production files stay unchanged. Evidence excludes build/dependency trees.

C0 — PASS (empty initial porcelain/stash output; no stash needed):

```sh
cd /home/dev/cham3
git status --porcelain
git stash list
git fetch origin codex/m2-cryptographic-contract-core review/m2-006-b1-r2-security main
git checkout codex/m2-cryptographic-contract-core
git reset --hard origin/codex/m2-cryptographic-contract-core
git rev-parse HEAD
git log --oneline -4
git rev-parse 16944ab^
git rev-list --count 0ec854d..5ea2407
git diff --name-only 0ec854d 5ea2407
git diff --name-only 5ea2407 16944ab
git diff --check 0ec854d 5ea2407
git diff --check 5ea2407 16944ab
git diff --exit-code 0ec854d 5ea2407 -- Cargo.toml Cargo.lock .cargo build .github PROJECT.md tests/m2/oracle tests/m2/vectors tests/m2/lib core/registryd/src/registry.rs core/registryd/src/anchor.rs core/registryd/src/ledger.rs core/registryd/src/record.rs
git diff --exit-code -G . 0ec854d 5ea2407 -- ':!core/registryd/src/clock.rs' ':!core/registryd/tests/trusted_time.rs' ':!tests/m2/harness/test_m2_registry_policy.py' ':!tests/m2/test_inventory.txt' ':!docs/specs/m2_registry_storage_v1.md'
git show 9b5afa2:docs/reviews/M2-006-stage-b1-r2-codex-security.md > /tmp/cham3-b1-r3-evidence/prior_review.md
git show 792f5f0:docs/reviews/M2-006-stage-b1-codex-security.md > /tmp/cham3-b1-r3-evidence/original_review.md
sha256sum /tmp/cham3-b1-r3-evidence/prior_review.md /tmp/cham3-b1-r3-evidence/original_review.md
id
uname -a
cat /etc/os-release
docker version
docker image inspect cham3-m2-independent-r3-final --format '{{.Id}}'
ulimit -n
```

C1 — PASS exit0 (`qualification.log` and copied `qualification/`/`runs/` receipts):

```sh
git clone --no-hardlinks /home/dev/cham3 /tmp/cham3-m2-b1-r3-independent-qual
git -C /tmp/cham3-m2-b1-r3-independent-qual checkout --detach 5ea2407ff2c639361ec2976bc20bc95f5acbce95
docker run --rm --name cham3-m2-b1-r3-independent-qual --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 -v /tmp/cham3-m2-b1-r3-independent-qual:/work -w /work cham3-m2-independent-r3-final bash build/scripts/qualify_m2_rust.sh > /tmp/cham3-b1-r3-evidence/qualification.log 2>&1
git -C /tmp/cham3-m2-b1-r3-independent-qual status --porcelain
```

C2 — PASS exit0 (`static.log`):

```sh
docker run --rm --name cham3-m2-b1-r3-static --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -v /tmp/cham3-m2-b1-r3-independent-qual:/work -w /work cham3-m2-independent-r3-final bash -c 'bash build/scripts/run_static_checks.sh && sh tests/run_tests.sh static' > /tmp/cham3-b1-r3-evidence/static.log 2>&1
```

C3 — PASS exit0 both (`storage.log`, `wrapper.log`, `storage/` reports):

```sh
docker run --rm --name cham3-m2-b1-r3-storage --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home --tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700 -v /tmp/cham3-m2-b1-r3-independent-qual:/work -w /work cham3-m2-independent-r3-final sh tests/m2/storage/check_reserve_full_disk.sh out/m2/storage > /tmp/cham3-b1-r3-evidence/storage.log 2>&1
docker run --rm --name cham3-m2-b1-r3-wrapper --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home --tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700 -e CHAM3_M2_RESERVE_PROBE=/work/out/m2/storage/target/release/cham3_reserve_probe -e CHAM3_M2_WRAPPER_DISK=/full-disk -v /tmp/cham3-m2-b1-r3-independent-qual:/work -w /work cham3-m2-independent-r3-final python3 -I -m unittest discover -s tests/m2/harness -p test_m2_reserve_probe.py -k wrapper -v > /tmp/cham3-b1-r3-evidence/wrapper.log 2>&1
```

C4 — PASS exit0 (`differentials.log`):

```sh
docker run --rm --name cham3-m2-b1-r3-differentials --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/work/out/reviewer-target -v /tmp/cham3-m2-b1-r3-reviewer:/work -w /work cham3-m2-independent-r3-final bash -c 'cargo test --frozen -p cham3_registryd --test trusted_time_model -- --nocapture && cargo test --frozen -p cham3_registryd --test admission_scenarios -- --nocapture' > /tmp/cham3-b1-r3-evidence/differentials.log 2>&1
```

C5 — PASS exit0 (`controls.log`): snippets independently extracted from the exact prior records' `~~~rust` blocks, preserving the leading blank line and exactly one trailing newline. Digests equal both records and author's copied sources:

| Snippet | SHA-256 |
|---|---|
| policy_reviewer_snippet.rs | c33ade322b722bee4494f532fd2e7ae6b7cff00d7741760f32412f6af983b25b |
| registry_reviewer_snippet.rs | 014db1de90301e29a0c03e19133929705598164f4a71a9496cdaf5109b1cba47 |
| r2_policy_controls.rs | 8fe2672661c33aabb8aed713dd59e31b91f2e03dbcfa0034aebac6227df05e1d |
| r2_registry_controls.rs | de0eb57e9905cef44105d95f938c74ec3f91ecb6333cd59b59b36d24e711c043 |
| r2_startup_control.rs | bbb3b9ad90bcd2252a85d8b74a28a06a22bcfab2ad325424bb46c33be9441287 |

Clone command: `git clone --no-hardlinks /home/dev/cham3 /tmp/cham3-m2-b1-r3-reviewer`, followed by `git -C /tmp/cham3-m2-b1-r3-reviewer checkout --detach 5ea2407ff2c639361ec2976bc20bc95f5acbce95`. Composition is candidate fixture plus newline plus original snippet, saved as `reviewer_b1.rs`; the same plus respective supplemental snippet(s), saved as `rereview_r2.rs`, exactly prior C6. Extraction uses Python regex `r'### '+re.escape(name)+r'\s+~~~rust\n(.*?)\n~~~'`, `re.S`, group1 `.rstrip('\n')+'\n'`, and asserts byte equality with the author's copy before composition.

```sh
docker run --rm --name cham3-m2-b1-r3-controls --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/work/out/reviewer-target -v /tmp/cham3-m2-b1-r3-reviewer:/work -w /work cham3-m2-independent-r3-final bash -c 'set -e; for profile in "" "--release"; do cargo test --frozen $profile -p cham3_policyd --test reviewer_b1 reviewer_ -- --nocapture --test-threads=1; cargo test --frozen $profile -p cham3_registryd --test reviewer_b1 reviewer_ -- --nocapture --test-threads=1; cargo test --frozen $profile -p cham3_policyd --test rereview_r2 rereview_ -- --nocapture --test-threads=1; cargo test --frozen $profile -p cham3_registryd --test rereview_r2 rereview_ -- --nocapture --test-threads=1; done' > /tmp/cham3-b1-r3-evidence/controls.log 2>&1
```

C6 — PASS exit0 (`anchor_control.log`): candidate `trusted_time.rs` concatenated with the supplemental `startup_anchor_control.rs` into untracked `reviewer_r3.rs`. Source retained locally and in sealed plugin artifacts (SHA-256 `6d8f755e58ab649422a4c635f9af859d403a7180e87e9f82a94e821a5b42ff66`).

```sh
cat /tmp/cham3-m2-b1-r3-reviewer/core/registryd/tests/trusted_time.rs /tmp/codex-security-artifacts-9042a4884ab4d7b047eca8091eb26bb988d5aee6682dfa2a43042366411cda5f/artifacts/02_discovery/validation_artifacts/r3/startup_anchor_control.rs > /tmp/cham3-m2-b1-r3-reviewer/core/registryd/tests/reviewer_r3.rs
docker run --rm --name cham3-m2-b1-r3-anchor-control --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/work/out/reviewer-target -v /tmp/cham3-m2-b1-r3-reviewer:/work -w /work cham3-m2-independent-r3-final bash -c 'cargo test --frozen -p cham3_registryd --test reviewer_r3 reviewer_startup_ -- --nocapture --test-threads=1 && cargo test --frozen --release -p cham3_registryd --test reviewer_r3 reviewer_startup_ -- --nocapture --test-threads=1' > /tmp/cham3-b1-r3-evidence/anchor_control.log 2>&1
```

C7 — PASS exit0, two Python tests (`new_defect_reason.log`); intentional mutant cargo exit101, unmodified cargo exit0:

```sh
docker run --rm -i --name cham3-m2-b1-r3-defect --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -v /tmp/cham3-m2-b1-r3-independent-qual:/work -w /work cham3-m2-independent-r3-final python3 - > /tmp/cham3-b1-r3-evidence/new_defect_reason.log 2>&1 <<'PY'
import sys, unittest
sys.path.insert(0, 'tests/m2/harness')
import test_m2_registry_policy as module
print('ALL_DEFECTS', len(module.SeededDefects), flush=True)
module.SeededDefects = [defect for defect in module.SeededDefects if defect[0] == 'startup_sample_not_reconciled']
original_run = module.run
def logged_run(*args, **kwargs):
    status, output = original_run(*args, **kwargs)
    print('COMMAND', args[0], 'EXIT', status, flush=True)
    print(output, flush=True)
    return status, output
module.run = logged_run
result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromModule(module))
raise SystemExit(not result.wasSuccessful())
PY
```

`SeededDefects` is the existing module's required external spelling. Preparation retries were not candidate verdict evidence: the first snippet extraction stripped its leading newline and failed the byte-equality assertion; corrected extraction matches all five digests. A first C7 invocation omitted Docker `-i`, passed no program on stdin and ran no tests (NOT_RUN); the corrected command above executes two tests and its output is retained. No empty log or preparation failure was counted PASS.

C8 — PASS request/download/digest/receipt comparison. For each requested run, execute the following literal `gh api` endpoints, retaining JSON as `ci_m2.json`, `ci_m2_jobs.json`, etc.:

```sh
gh api repos/Kayyo321/cham3/actions/runs/38641025183
gh api repos/Kayyo321/cham3/actions/runs/38641025183/jobs --paginate
gh api repos/Kayyo321/cham3/actions/runs/38641025244
gh api repos/Kayyo321/cham3/actions/runs/38641025244/jobs --paginate
gh api repos/Kayyo321/cham3/actions/runs/38641025091
gh api repos/Kayyo321/cham3/actions/runs/38641025091/jobs --paginate
gh api repos/Kayyo321/cham3/actions/artifacts/11931692301 > /tmp/cham3-b1-r3-evidence/ci_artifact_meta.json
gh api repos/Kayyo321/cham3/actions/artifacts/11931692301/zip > /tmp/cham3-b1-r3-evidence/ci_artifact.zip
sha256sum /tmp/cham3-b1-r3-evidence/ci_artifact.zip
unzip -q /tmp/cham3-b1-r3-evidence/ci_artifact.zip -d /tmp/cham3-b1-r3-evidence/ci_artifact
```

Python assertions check head_sha exactly candidate, completed/success, every job and step success, and SHA-256 against API plus supplied digest `c0caa359a6c5e6a7daba6931f1c5fa8589c18ec0694ed8ff7dc0429c947ae2b0`. m2:1job/10steps; m1:3/39; m0:3/39. Unit receipt `tests` dictionaries match across independent run, author unit JSON and downloaded CI artifact. Debug/release full-filesystem reports compare byte-identically. M0/M1 are CI comparison evidence; local reruns of their unchanged distribution suites are NOT_RUN.

- [m2_checks 38641025183](https://github.com/Kayyo321/cham3/actions/runs/38641025183)
- [m1_checks 38641025244](https://github.com/Kayyo321/cham3/actions/runs/38641025244)
- [m0_checks 38641025091](https://github.com/Kayyo321/cham3/actions/runs/38641025091)

C9 — PASS execution: [security-diff-scan skill](/home/dev/.codex/plugins/cache/openai-curated-remote/codex-security/0.1.32/skills/security-diff-scan/SKILL.md), Codex Security plugin `codex-security`0.1.32. Scan `38a1e5e5-271e-4bcb-ba8f-b6f28ff1d13f`, exact requested range. Dedicated skill-required preflight worker ran:

```sh
python3 /home/dev/.codex/plugins/cache/openai-curated-remote/codex-security/0.1.32/scripts/config_preflight.py --profile security_diff_scan --cwd /home/dev/cham3 --runtime-check delegation_available=true --runtime-check goal_tools_available=true --multi-agent-runtime-owner native --multi-agent-runtime-version v2 --multi-agent-session-cap 4 --multi-agent-runtime-provenance thread-context > /tmp/cham3-b1-r3-evidence/preflight.json
```

Exit0, ready, all three capability checks pass; no config changes. A fresh-context architecture worker mapped only the authorized startup boundary offline from immutable candidate; parent verified material source/consumer claims. No SECURITY.md applies. Parent reviewed the sole compact inventory source file and all four supporting changed paths; recorded no candidates and six closed coverage surfaces, retained model/evidence, submitted final complete draft, called completion exactly once and read back canonical sealed results. Completed `2026-10-14T22:33:50.806920Z`, zero reportable findings. Validation/attack-path candidate phases were unnecessary with zero candidates, not skipped for any candidate. Plugin-reported usage at sealing: total4045550 tokens, input4024036 (cached3809664), output21514 including reasoning2002, three threads, coverage complete; this is the tool's measured rollout accounting, not an estimate or candidate-only cost.

Managed persistent directory: `/home/dev/.codex/state/plugins/codex-security/scans/cham3/5ea2407ff2c639361ec2976bc20bc95f5acbce95_20261014T222802Z_sqq_6aai`. Canonical manifest/findings/coverage/report/SARIF and retained controls/logs are sealed there. Daybreak advisory said not_granted; it did not gate completion. No protected-output or account configuration change.

## Owner questions and handoff

Owner questions: **none**. No architecture/risk/clock-policy decision is made. ADR0007 §5.7 and Option B, approved E4/E6 clarifications, and ADR0006 E5 residual remain unchanged. Component role enums, development storage defaults and injected faults do not establish deployed service authentication, protected mounts or OS enforcement.

Next owner: coordinator/integrator records this scoped B1 approval and applies the established integration process; B2 remains a separate candidate requiring its own review. Reviewer performs no merge, status-owner update, or later-stage implementation. Owner clone and qualification clone remain porcelain-clean; reviewer clone has only five untracked test files and ignored build output, with `git diff --exit-code` passing. Publication branch adds only this record to its exact main base.

Retained local evidence contains 114 files (1,370,020 bytes, excluding its subsequently generated SHA256SUMS), including downloaded ZIP/CI JSON, original snippets, supplemental fault source, qualification/unit/harness receipts and logs. No build/dependency trees are retained. Complete local digest inventory: `/home/dev/cham3-m2-b1-r3-review-evidence/SHA256SUMS`.

| Independent evidence file | SHA-256 |
|---|---|
| qualification.log | 10878be36bc17680816c113489697f63c5c484fbba11fd5fd861c2a76dd101b6 |
| controls.log | c94fe808c4cf66e47ad10012c0877d6fc7843e4e350093fd01cd2919398c0ffd |
| anchor_control.log | 7ff71d2219e1644facfff57ad4c0197c808a66dc45dfd391781dc1ee391a7ddc |
| new_defect_reason.log | 9b09202573d7c49b0d0efde064fcbe5c75b5d3503f2ddb3990b39cb23337bcc8 |
| static.log | 82170eec484e5331697e54f8842983906a6da37282d93af9eb7b1765b2bfc405 |
| storage.log | 6f42d5b1f641f6ddde788272874731a6db49b6dfc319fa7babf89992f0a9c1db |
| wrapper.log | 44170022fea44c971b9680aefa7c687f5593be9223e973dbd05d7a59febd720f |
| differentials.log | da194f094c813e21025bde1e414029577e87249b5d0e13ac01b4c550f752d456 |
| startup_anchor_control.rs | 6d8f755e58ab649422a4c635f9af859d403a7180e87e9f82a94e821a5b42ff66 |

## Supplemental reviewer control source

Append the following test-only source to the candidate trusted_time fixture as C6 specifies. Production files remain unchanged.

~~~rust

#[derive(Clone)]
struct startup_anchor_fault_io_t {
    directory_failure: bool,
    hit: Arc<AtomicBool>,
}
impl journal_io_t for startup_anchor_fault_io_t {
    fn write_all(&self, file: &mut fs::File, bytes: &[u8]) -> io::Result<()> {
        system_io_t.write_all(file, bytes)
    }
    fn sync_data(&self, file: &fs::File) -> io::Result<()> {
        system_io_t.sync_data(file)
    }
    fn sync_directory(&self, path: &Path) -> io::Result<()> {
        if self.directory_failure && path.file_name().is_some_and(|name| name == "anchor") {
            self.hit.store(true, Ordering::SeqCst);
            return Err(io::Error::other("injected startup anchor directory sync failure"));
        }
        system_io_t.sync_directory(path)
    }
    fn replace_file(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        if !self.directory_failure && path.parent().is_some_and(|parent| parent.file_name().is_some_and(|name| name == "anchor")) {
            let (_, map) = decode_slot(bytes).expect("valid proposed anchor");
            assert_eq!(map.time_floor.unwrap().anomaly, 3, "closure included in refused write");
            self.hit.store(true, Ordering::SeqCst);
            return Err(io::Error::other("injected startup anchor replace failure"));
        }
        system_io_t.replace_file(path, bytes)
    }
}
#[test]
fn reviewer_startup_closure_refuses_failed_anchor_publication() {
    for clean in [false, true] {
        for directory_failure in [false, true] {
            let root = temp_root(&format!("r3_startup_failure_{clean}_{directory_failure}"));
            let time = test_time_t::at(T1Seconds);
            let alice = test_user(1001);
            let registry = start(&root, "boot-1", &time);
            enroll(&registry, &alice);
            time.step_wall_clock(T1Seconds - 400);
            registry.lineage_head(Policyd, alice.uid, &NoLineage).unwrap();
            if clean { registry.shutdown().unwrap(); }
            drop(registry);
            assert_eq!(newest_anchor(&root).time_floor.unwrap().anomaly, 2);
            time.step_wall_clock(T1Seconds);
            let hit = Arc::new(AtomicBool::new(false));
            let result = registry_t::open(&root, owner_of(&root), config("boot-1"),
                Box::new(startup_anchor_fault_io_t { directory_failure, hit: Arc::clone(&hit) }),
                Box::new(time.clone()));
            assert!(matches!(result, Err(registry_start_error_t::store(_))), "no registry returned before durable publication");
            assert!(hit.load(Ordering::SeqCst), "the intended anchor operation was reached");
            println!("PASS refused clean={clean} directory_failure={directory_failure}");
        }
    }
}
~~~
