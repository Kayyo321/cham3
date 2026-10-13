# M2-005 independent bounded review, revision 4

Verdict: **APPROVE** for M2-005 at `5860f276ab593da34771f704691ea35649324d6e`. Reviewer: Codex, independent of implementation. Date: 2026-10-13. **M2-005-R3-01: PASS, resolved. No new findings.** No owner decision, implementation edit, merge, M2-006 approval or claim of deployed enforcement is made. This record alone is published on `review/m2-005-r4-security`, based on main `9cf882503848f8c6cc4938a588cc9fc062a9392d`.

## Scope and provenance — PASS

Base: `ad18d8be68d2e5a7fd50d4577ed90edd37ce5235`; one-commit candidate: `5860f276ab593da34771f704691ea35649324d6e`; docs follow-up: `aeaee2dcf106a19c7607b0051b3ba4616e4b1816`. Initial status/stash list empty. Fetched implementation, R3 review and main; checked out implementation and reset the clean checkout to its fetched remote head, exactly aeaee2d. No unknown work was discarded. Testing clone `/tmp/cham3_review_r4/candidate` created with `git clone --no-hardlinks /home/dev/cham3 /tmp/cham3_review_r4/candidate`, detached at 5860f27, owned by UID/GID 1001. Qualification used CHAM3_M2_REQUIRE_CLEAN=1.

Read AGENTS.md, relevant PROJECT.md and engineering/security guidance, the author's R4 resolutions/handoff and the immutable R3 record at `4865d7b7a4c885fddf71b3e0f0b14a361c3c609e`, `docs/reviews/M2-005-codex-security-rereview-r3.md`, SHA-256 `6ce7449c8784cbf67ccd5b1ee89c5da9f97c088e9001f04201cb6c4165288c50`. Author claims were checked against source and independent runs.

`git diff --stat ad18d8b 5860f27`: exactly four files, 207 insertions / 10 deletions:

- tests/m2/storage/check_reserve_full_disk.sh
- tests/m2/harness/test_m2_reserve_probe.py
- tests/m2/test_inventory.txt
- .github/workflows/m2_checks.yml

`git diff --name-only 30f77b2 ad18d8b` and `git diff --name-only 5860f27 aeaee2d`: only docs/. The first records owner erratum E4 and M2-006 scope, including documentation changes to ADR/spec error vocabulary; these are outside the candidate code diff. The second records evidence and handoff. No Rust source, dependency, Cargo.lock, manifest, specification, ADR, oracle or vector change in the candidate. `git diff --exit-code be24152 5860f27 -- core build Cargo.lock Cargo.toml .cargo` also passed, confirming unchanged reviewed runtime and pinned build inputs.

## R3-01 resolution and independent preservation tests — PASS

At `tests/m2/storage/check_reserve_full_disk.sh:34-43`, both intended paths are checked with `-e` OR `-L`, so regular files, directories and dangling/live symlinks refuse with exit 1. This occurs before output mkdir and Cargo build. The destructive pre-deletion is gone. After each successful profile, lines 73-76 check both existence and symlink status. The wrapper itself removes no disk paths.

`CHAM3_REPO_ROOT` chooses the manifest/checkout; `CHAM3_M2_RESERVE_TARGET_DIR` selects Cargo output and the executed profile binary. Neither changes the disk argument or root/filler basenames. These are trusted developer build configuration, not new application-facing authority. Refusal is an appropriate fix: a private directory cannot contain the probe's fixed mount-root filler. Existing objects must not be adopted or removed.

Re-ran the R3 wrapper_run.py scenario against this candidate with only its expected outcome assertions changed from deletion to preservation. The actual wrapper exits 1 at its precheck, keeps both original synthetic paths/content, and never reaches the old overlay refusal path. Source and captured output embedded below.

Independent extended matrix: **25/25 PASS**. Eleven preservation cases each on overlay and directly at the 2 MiB tmpfs mount root: original root+filler, root directory only, filler file only, root regular file, filler directory, root live/dangling symlink alone, filler live/dangling symlink alone, both live links, both dangling links. Tests compare contents, modes, inode, ownership, size, mtime and link destinations, including targets; a Cargo spy proves no build invocation, and output/target directories remain absent. The overlay pathname contains spaces, a quote and shell metacharacters. Three fresh-path controls prove overlay refusal leaves no paths, and tmpfs mount-root and nested-directory runs execute **both profiles**, report 37 records and leave neither disk-directory nor mount-root filler leftovers.

Real wrapper positive path followed by the repository's **five wrapper cases on the same fresh tmpfs: PASS**, 5/5, entire /full-disk empty afterwards. Full-disk debug/release reports each show restrictive seq 35/36, ordinary admission refused while depleted, refill seq 37, 37 chained records, and 1 MiB reserve preallocated/replenished.

Seed discrimination: **PASS**. Additional observation runs use the repository's seeded_copy/existing_paths_survive helpers without changing their logic. On overlay, pristine exits 1 with both sentinels surviving; restored pre-deletion exits 1 after filesystem refusal with sentinels lost. On tmpfs, pristine exits 1 preserving them; seeded pre-deletion exits 0 through the positive path but loses them. Thus detection depends on data preservation, not just nonzero exit. Temporary seeded copies and synthetic data only; no candidate source changes.

### Precise limits of the checked paths

Wrapper argv documents a tmpfs mount. With disk=/full-disk, checked filler and actual probe filler coincide. The Python harness instead uses /full-disk/wrapper_case_* subdirectories: wrapper precheck/leftover paths are inside the case directory, while the unchanged binary creates the actual filler at /full-disk/cham3_reserve_probe_filler (`core/registryd/src/bin/cham3_reserve_probe.rs:87-121,230-245`). The binary independently uses exclusive create_new and preserves an existing actual filler. The harness's combined root+filler tests exit at root first, and its dangling variant changes only the root target; its path-free test permits either success or refusal. Those five tests alone therefore do not independently prove filler-only/dangling-filler refusal or mount-root cleanup. The reviewer matrix explicitly covers those at the actual mount root, and requires success for fresh tmpfs controls. This is a documented test-coverage limitation, not a demonstrated remaining data-loss defect. The tested contract assumes stable disposable mounts and trusted build paths, not a hostile same-account process replacing ancestors concurrently.

## Earlier results — intact

No Rust code changed since R3. The earlier directory/reserve fixes, direct-probe preservation, error vocabulary and M2-004 conclusions remain intact within their prior scope. The reviewer's original ten edge_, nine own_, seven additional journal tests, filesystem_full_reserve and 34 direct-probe cases were **NOT_RUN again in R4**, as expressly permitted for this bounded review; their immutable R3 evidence is carried forward. The current qualification does rerun the repository's unit and harness tests, including the original seven probe tests and five new wrapper tests. The full-filesystem wrapper itself was rerun independently. Local protection positive path was **NOT_RUN again**; its unchanged code had R3 local evidence and the exact-head R4 CI protection step passed.

Miri, sanitizers and coverage-guided fuzzing: **NOT_RUN/NOT_AVAILABLE** on the pinned toolchain. Actual power-loss, production IPC/isolation and staged M2-006 anchors/ledger are **NOT_RUN**, not certified by this approval. Local M0/M1 full suites **NOT_RUN**; exact-head CI compared instead.

## Environment and verification ledger

Home computer: Debian 13.7, x86_64, kernel 7.1.13+deb13-amd64, UID/GID 1001:1001. Docker 26.1.5+dfsg1; unrestricted network. Reused the exact R3 pinned image `cham3-m2-independent-r3`, SHA-256 `4f4f684d76f7c3c299932846ebe66765204c7e15c9414d77922d15ff561bb327`, linux/amd64, Rust 1.85.1; build context unchanged. Image rebuild **NOT_RUN**, reuse deliberate. Containers ran non-root with no --privileged, added capabilities or --record. Qualification uses container-only nofile 1024:1024 for Valgrind. Preservation runs use --network none; all disk data synthetic. No host protection/configuration change.

| Check | Result | Evidence |
| --- | --- | --- |
| Clean amd64 qualification | PASS | qualification.log; 169 debug + 169 release unit |
| Provider oracle | PASS | 7/7 |
| ECR oracle | PASS | 19 valid / 96 invalid / 18 admission; 23 scenarios / 352 steps |
| Format differential | PASS | 5955 cases / 0 mismatches; unchanged corpus 0ad8bc2b1e8d39224de699db64aa45ca7adcf70117c555f35b57c537471d395e |
| Vault object oracle | PASS | 38 checks |
| Harness | PASS | 147; all 12 reserve-probe module tests; 41 registry/policy and 21 identity/vault seeded defects |
| Static checks | PASS | out/static_checks_review.log |
| Static tests | PASS | 82 tests; out/static_tests_review.log |
| Full-filesystem positive | PASS | own debug/release, 2 MiB tmpfs, leftovers absent |
| Five tmpfs wrapper cases | PASS | tmpfs_wrapper.log; 5/5, entire tmpfs empty |
| Original scenario + extended matrix | PASS | wrapper_run_r4.log and r4_wrapper_matrix.log |
| Seeded wrapper sensitivity | PASS | r4_seeded_trace.log plus qualification/tmpfs suites |
| CI identity, jobs/steps and artifact comparison | PASS | ci_m{0,1,2}.json/log, ci_comparison.txt |
| Codex Security diff scan | PASS | completed canonical artifacts; zero findings |

### Exact commands

Cwd for qualification `/tmp/cham3_review_r4/candidate`; all source mounts use that clone. The static shell runs both gates and checks both exit statuses (each 0).

```sh
sudo -n docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 -v /tmp/cham3_review_r4/candidate:/work -w /work cham3-m2-independent-r3 bash build/scripts/qualify_m2_rust.sh > /tmp/cham3_review_r4/qualification.log 2>&1
sudo -n docker run --rm --user 1001:1001 -e HOME=/tmp/cham3_home -v /tmp/cham3_review_r4/candidate:/work -w /work cham3-m2-independent-r3 sh -c 'bash build/scripts/run_static_checks.sh > out/static_checks_review.log 2>&1; checks_status=$?; sh tests/run_tests.sh static > out/static_tests_review.log 2>&1; tests_status=$?; echo checks_status=$checks_status tests_status=$tests_status; test "$checks_status" = 0 && test "$tests_status" = 0' > /tmp/cham3_review_r4/static.log 2>&1
sudo -n docker run --rm --user 1001:1001 --tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700 -e HOME=/tmp/cham3_home -v /tmp/cham3_review_r4/candidate:/work -w /work cham3-m2-independent-r3 sh -c 'sh tests/m2/storage/check_reserve_full_disk.sh out/m2/storage && CHAM3_M2_RESERVE_PROBE=/work/out/m2/storage/target/release/cham3_reserve_probe CHAM3_M2_WRAPPER_DISK=/full-disk python3 -I -m unittest discover -s tests/m2/harness -p test_m2_reserve_probe.py -k wrapper -v && test -z "$(ls -A /full-disk)" && echo tmpfs_leftovers_absent' > /tmp/cham3_review_r4/tmpfs_wrapper.log 2>&1
```

External reproduction commands use the temporary plugin directory returned by the artifact tool (path below), mounted at /evidence. Matrix and seed trace each run in their own fresh container; wrapper scenario uses a read-only candidate mount and no tmpfs.

```sh
review_artifacts=/tmp/codex-security-artifacts-8ffbec5cabc6871094527d9e4505cb5c39a45cec45f7576d2e3541c9174287ee
sudo -n docker run --rm --network none --user 1001:1001 --tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700 -e HOME=/tmp/cham3_home -v /tmp/cham3_review_r4/candidate:/work -v "$review_artifacts:/evidence" -w /work cham3-m2-independent-r3 sh -c 'python3 /evidence/artifacts/r4_wrapper_matrix.py > /evidence/artifacts/r4_wrapper_matrix.log 2>&1'
sudo -n docker run --rm --network none --user 1001:1001 --tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700 -e HOME=/tmp/cham3_home -v /tmp/cham3_review_r4/candidate:/work -v "$review_artifacts:/evidence" -w /work cham3-m2-independent-r3 sh -c 'python3 /evidence/artifacts/r4_seeded_trace.py > /evidence/artifacts/r4_seeded_trace.log 2>&1'
sudo -n docker run --rm --network none --user 1001:1001 -e HOME=/tmp/cham3_home -v /tmp/cham3_review_r4/candidate:/work:ro -v "$review_artifacts:/evidence" -w /work cham3-m2-independent-r3 sh -c 'python3 /evidence/artifacts/wrapper_run_r4.py > /evidence/artifacts/wrapper_run_r4.log 2>&1'
```

## CI and artifact comparison — PASS

[m2_checks 38444270032](https://github.com/Kayyo321/cham3/actions/runs/38444270032), [m1_checks 38444270097](https://github.com/Kayyo321/cham3/actions/runs/38444270097), [m0_checks 38444270083](https://github.com/Kayyo321/cham3/actions/runs/38444270083): exact candidate head, every job/step successful. Explicitly verified protection, full filesystem and new “Wrapper preservation on the tmpfs (unprivileged)” step. Its five tests pass. The new CI step uses a fresh container/tmpfs of the same configuration, rather than preserving the previous step's mount instance; the reviewer's combined command additionally runs positive and wrapper cases in one mount instance.

Commands: `gh run view RUN --repo Kayyo321/cham3 --json headSha,conclusion,jobs`, `gh run view RUN --repo Kayyo321/cham3 --log`, `gh run download 38444270032 --repo Kayyo321/cham3 -n m2_qualification_amd64 --dir /tmp/cham3_review_r4/ci_artifact`, `gh api repos/Kayyo321/cham3/actions/artifacts/11839175077/zip > /tmp/cham3_review_r4/ci_artifact.zip`.

Archive **44 files, 103622 bytes**, SHA-256 **3b9cb3b99a7dcdf04f4998c4303be3195d732a1646a0dc065ba1d340a9720ba1**, exact match. Compared unit/harness inventories/counts/results, provider output, format cases/coverage/corpus, ECR/vault logs, unit/harness/qualification summaries and both reserve reports. Summaries and oracle logs byte-identical; reserve reports identical except mount UID/GID. Comparison source/output embedded below. Evidence timestamps and host environment need not match; no assertion of identical binaries or whole artifact bytes to the local run.

## Codex Security — completed

Skill **codex-security:security-diff-scan**, plugin **0.1.32**, scan **aa6e9f86-92ed-4f22-be4f-b1adad5764a6**, exact ad18d8b..5860f27 range. **Zero findings**, all four changed files manually accounted for, no unresolved candidates. The automatic inventory returned only .github/workflows/m2_checks.yml; shell/Python tests and inventory were explicitly reviewed despite that source filter. No source coverage claim relies on the filtered inventory alone.

Dedicated worker preflight command: `python3 /home/dev/.codex/plugins/cache/openai-curated-remote/codex-security/0.1.32/scripts/config_preflight.py --profile security_diff_scan --cwd /home/dev/cham3 --runtime-check delegation_available=true --runtime-check goal_tools_available=true`; exit 0, ready, all three capability checks pass. No configuration edit. A fresh-context architecture worker reviewed actual consumers and source anchors; parent verified material facts. SECURITY.md resolver returned empty for all four files. The canonical model preserves the nested/mount filler distinction and the test limitations above; its worker runtime NOT_RUN statement does not describe the parent's executed tests.

Plugin workflow: start_codex_security_prompt_only_scan once; publish preflight and phase progress; independent source-backed model with early incomplete semantic draft; prepare/list review items; parent review of all four files and bounded supporting probe/dispatcher; record_codex_security_discovery_candidates with empty list; retained source/output artifacts; final complete semantic draft; complete_codex_security_scan once; get_codex_security_completed_scan. No plausible vulnerability required validation/attack-path escalation. The Daybreak account advisory was not_granted and non-gating; it did not prevent local scan completion.

Measured plugin usage (`codex_rollout`): cacheWriteInputTokens=0, cachedInputTokens=4520064, coverage=complete, inputTokens=4646029, outputTokens=26133, reasoningOutputTokens=2255, source=codex_rollout, threadCount=2, totalTokens=4672162. Aggregate counters include cached context and associated threads.

Review limitations remain those stated above and in R3; ordinary same-authority build configuration, path aliases or hypothetical concurrent ancestor replacement are not treated as a newly introduced privilege boundary. No remaining acceptance blocker was found in this bounded fix.

Next: coordinator/integrator may use this independent approval in the established workflow. No merge or status/implementation update performed by this reviewer.

## Evidence digests

Local evidence paths and SHA-256 digests. New reproduction sources/output are embedded below; prior tests are in the immutable R3 record.

| Path | SHA-256 |
| --- | --- |
| /tmp/cham3_review_r4/ci_comparison.txt | 7994c839bfe6fba18d8d85aeb9688b2ee1e8243530f0abc8688898cd58f1aced |
| /tmp/cham3_review_r4/compare_ci.py | f02a5fa5b15a6b0938d6c92c516daa9cbe85f7eb13a90e55e636bd99ab71f1af |
| /tmp/cham3_review_r4/tmpfs_wrapper.log | 141bbc437b79710e4532074e58bf169c305f524f5a9578dc2d4df52b6c083346 |
| /tmp/codex-security-artifacts-8ffbec5cabc6871094527d9e4505cb5c39a45cec45f7576d2e3541c9174287ee/artifacts/wrapper_run_r4.py | b4e5b8506ba71d6b57aef9baba52b5310b8fa6c0fed106aa4f40a5398f471f02 |
| /tmp/codex-security-artifacts-8ffbec5cabc6871094527d9e4505cb5c39a45cec45f7576d2e3541c9174287ee/artifacts/wrapper_run_r4.log | f48134a87791b6953d6a4fc92aff7b2384dc227f651318f30470ec664157994a |
| /tmp/codex-security-artifacts-8ffbec5cabc6871094527d9e4505cb5c39a45cec45f7576d2e3541c9174287ee/artifacts/r4_wrapper_matrix.py | 8d65dd38ac4c070e82a57c8037b91e61c3465617fa74da9a3b2aeb28be079c6b |
| /tmp/codex-security-artifacts-8ffbec5cabc6871094527d9e4505cb5c39a45cec45f7576d2e3541c9174287ee/artifacts/r4_wrapper_matrix.log | e66a562875f335610e980ff8b7fe87465434fd91500a8edd3d9ba361d5719679 |
| /tmp/codex-security-artifacts-8ffbec5cabc6871094527d9e4505cb5c39a45cec45f7576d2e3541c9174287ee/artifacts/r4_seeded_trace.py | f46a3607d79ecf01d84acffa4f61e5648057a20c6870dc9ba1e1c5eb6d39eaab |
| /tmp/codex-security-artifacts-8ffbec5cabc6871094527d9e4505cb5c39a45cec45f7576d2e3541c9174287ee/artifacts/r4_seeded_trace.log | d6759524f856c4359e49ed4856a1e7acbcc8dd8197a97aaf6693b2615680cf80 |
| /tmp/cham3_review_r4/qualification.log | 59f79c1fb3af01196169f826a0c78e3ba944f29178b79d80d78bdd1aff3d214c |
| /tmp/cham3_review_r4/static.log | 22d031d7b32cae0c71fd66940dccc3197caf640e78c1e5d2c36390c65947e55a |
| /tmp/cham3_review_r4/candidate/out/static_checks_review.log | 3b060ae8c6cbd18697388da94cc0ad5bef0416d593b2b86f18ab503fc87b7cd0 |
| /tmp/cham3_review_r4/candidate/out/static_tests_review.log | 7a70f89241c39ad71f0aa0fcd30fae2de57ee5a75d22e54e51753124284d4323 |
| /tmp/cham3_review_r4/ci_artifact.zip | 3b9cb3b99a7dcdf04f4998c4303be3195d732a1646a0dc065ba1d340a9720ba1 |
| /tmp/cham3_review_r4/preflight.json | 2adff52e10170d20d37e648faea26f28ea62fdd8d794fe57e2bdff2bcf160bd1 |
| /tmp/codex-security-artifacts-8ffbec5cabc6871094527d9e4505cb5c39a45cec45f7576d2e3541c9174287ee/artifacts/r4_threat_model.json | 6bcf1918a67bdf140dccacefc31f5d5e2d17ae0647f4ea50ea7e8eed8b2a4b7f |
| /home/dev/.codex/state/plugins/codex-security/scans/cham3/5860f276ab593da34771f704691ea35649324d6e_20261013T184213Z_qg1hx6c_/scan-manifest.json | db880c742c9203490d8f2478c9e800b3c3203d9aa0a318ec049a666335f60eb1 |
| /home/dev/.codex/state/plugins/codex-security/scans/cham3/5860f276ab593da34771f704691ea35649324d6e_20261013T184213Z_qg1hx6c_/coverage.json | 8daa7ef957202ebc6856bdcd71bae02e9c87bebde815f4f3a19b436bf9895e83 |
| /home/dev/.codex/state/plugins/codex-security/scans/cham3/5860f276ab593da34771f704691ea35649324d6e_20261013T184213Z_qg1hx6c_/findings.json | d8f4b93d7452f990b47a6cb5ded1d03e12b083586c10c0e077a7cee41a9a0635 |
| /home/dev/.codex/state/plugins/codex-security/scans/cham3/5860f276ab593da34771f704691ea35649324d6e_20261013T184213Z_qg1hx6c_/report.md | 97f1707932c80358b3359b52076d513884d7ca2d91d72cdba2ca58857b7cce53 |
| /home/dev/.codex/state/plugins/codex-security/scans/cham3/5860f276ab593da34771f704691ea35649324d6e_20261013T184213Z_qg1hx6c_/threatmodel.md | c87edf882b290f03ac4a11aa4b97cde518420b9591b3f522a1855f540c6c1a27 |
| /home/dev/.codex/state/plugins/codex-security/scans/cham3/5860f276ab593da34771f704691ea35649324d6e_20261013T184213Z_qg1hx6c_/exports/results.sarif | 4ca4f400cc3ba833a649721e5aae344feba56dfd1ac9217b1938d60d0164d6d9 |

## Reproduction sources and output

### ci_comparison.txt

```text
cargo_test_debug.json PASS total 169
cargo_test_release.json PASS total 169
provider_oracle.json PASS total n/a
format_differential.json PASS total 5955
python_harness.json PASS total 147
reserve_probe_tests 12
ecr_vector_oracle.log PASS byte-identical
vault_object_oracle.log PASS byte-identical
unit summary PASS byte-identical
harness summary PASS byte-identical
qualification summary PASS byte-identical
reserve_full_disk_debug.txt PASS identical except mount UID/GID
reserve_full_disk_release.txt PASS identical except mount UID/GID
m2 exact head, all jobs/steps PASS
m1 exact head, all jobs/steps PASS
m0 exact head, all jobs/steps PASS
artifact PASS 44 files, 103622 bytes, exact digest
```

### compare_ci.py

```python
from pathlib import Path
import json,re,zipfile,hashlib
root=Path('/tmp/cham3_review_r4')
local=root/'candidate/out/m2'
remote=root/'ci_artifact'
for name,keys in {
 'cargo_test_debug.json':['tests','total_tests','result','exit_status'],
 'cargo_test_release.json':['tests','total_tests','result','exit_status'],
 'provider_oracle.json':['oracle_output','probe_output','result'],
 'format_differential.json':['cases','coverage','distinct_outcomes','corpus_sha256','mismatch_count','seed','result'],
 'python_harness.json':['tests','total_tests','result','exit_status'],
}.items():
 a=json.loads(next(local.rglob(name)).read_text());b=json.loads(next(remote.rglob(name)).read_text())
 assert {k:a[k] for k in keys}=={k:b[k] for k in keys},name
 print(name,'PASS', 'total',a.get('total_tests',a.get('cases','n/a')))
 if name=='format_differential.json': assert a['corpus_sha256']=='0ad8bc2b1e8d39224de699db64aa45ca7adcf70117c555f35b57c537471d395e'
 if name=='python_harness.json':
  print('reserve_probe_tests',len([x for x in a['tests'] if 'test_m2_reserve_probe.' in str(x)]))
for name in ['ecr_vector_oracle.log','vault_object_oracle.log']:
 assert next(local.rglob(name)).read_bytes()==next(remote.rglob(name)).read_bytes(),name
 print(name,'PASS byte-identical')
for suite in ['unit','harness']:
 a=next((local/'runs').glob(suite+'*/summary.txt'));b=next((remote/'runs').glob(suite+'*/summary.txt'))
 assert a.read_bytes()==b.read_bytes()
 print(suite,'summary PASS byte-identical')
a=next((local/'qualification').glob('*/summary.txt'));b=next((remote/'qualification').glob('*/summary.txt'))
assert a.read_bytes()==b.read_bytes()
print('qualification summary PASS byte-identical')
for profile in ['debug','release']:
 name='reserve_full_disk_'+profile+'.txt'
 a=(local/'storage'/name).read_text();b=(remote/'storage'/name).read_text()
 normalize=lambda v:re.sub(r'(uid|gid)=\d+',r'\1=<uid>',v)
 assert normalize(a)==normalize(b)
 print(name,'PASS identical except mount UID/GID')
for kind in ['m2','m1','m0']:
 d=json.loads((root/f'ci_{kind}.json').read_text())
 assert d['headSha']=='5860f276ab593da34771f704691ea35649324d6e' and d['conclusion']=='success'
 assert all(j['conclusion']=='success' and all(s['conclusion']=='success' for s in j['steps']) for j in d['jobs'])
 print(kind,'exact head, all jobs/steps PASS')
z=root/'ci_artifact.zip'
assert z.stat().st_size==103622 and len([i for i in zipfile.ZipFile(z).infolist() if not i.is_dir()])==44
assert hashlib.sha256(z.read_bytes()).hexdigest()=='3b9cb3b99a7dcdf04f4998c4303be3195d732a1646a0dc065ba1d340a9720ba1'
print('artifact PASS 44 files, 103622 bytes, exact digest')
```

### tmpfs_wrapper.log

```text
   Compiling version_check v0.9.5
   Compiling typenum v1.17.0
   Compiling semver v1.0.23
   Compiling cpufeatures v0.2.16
   Compiling cfg-if v1.0.0
   Compiling subtle v2.6.1
   Compiling zeroize v1.8.1
   Compiling signature v2.2.0
   Compiling rustix v0.38.37
   Compiling fiat-crypto v0.2.2
   Compiling ed25519 v2.2.3
   Compiling linux-raw-sys v0.4.14
   Compiling bitflags v2.8.0
   Compiling generic-array v0.14.7
   Compiling rustc_version v0.4.0
   Compiling curve25519-dalek v4.1.3
   Compiling crypto-common v0.1.6
   Compiling block-buffer v0.10.4
   Compiling digest v0.10.7
   Compiling sha2 v0.10.8
   Compiling cham3_os_adapter v0.1.0 (/work/core/native/os_adapter)
   Compiling ed25519-dalek v2.1.1
   Compiling cham3_contract_format v0.1.0 (/work/core/contract_format)
   Compiling cham3_identityd v0.1.0 (/work/core/identityd)
   Compiling cham3_registryd v0.1.0 (/work/core/registryd)
    Finished `release` profile [optimized] target(s) in 9.63s
profile release
mount tmpfs /full-disk tmpfs rw,nosuid,nodev,noexec,relatime,size=2048k,mode=700,uid=1001,gid=1001,inode64 0 0
reserve_bytes 1048576
reserve_allocated_bytes 1048576
reserve_promised_bytes 1048576
reserve_preallocated
tmpfs_mount /full-disk
filler_bytes 1044480
restrictive_append_on_full_filesystem ok seq=35
reserve_bytes_after_release 0
admission_refused reserve_depleted
second_restrictive_append ok seq=36
append_after_space_returned ok seq=37
reserve_bytes 1048576
reserve_allocated_bytes 1048576
reserve_promised_bytes 1048576
reserve_preallocated
reserve_replenished
journal_records 37
journal_chained
reserve_verified
exit_status 0
   Compiling typenum v1.17.0
   Compiling version_check v0.9.5
   Compiling semver v1.0.23
   Compiling cfg-if v1.0.0
   Compiling cpufeatures v0.2.16
   Compiling rustix v0.38.37
   Compiling subtle v2.6.1
   Compiling zeroize v1.8.1
   Compiling fiat-crypto v0.2.2
   Compiling signature v2.2.0
   Compiling bitflags v2.8.0
   Compiling ed25519 v2.2.3
   Compiling linux-raw-sys v0.4.14
   Compiling generic-array v0.14.7
   Compiling rustc_version v0.4.0
   Compiling curve25519-dalek v4.1.3
   Compiling block-buffer v0.10.4
   Compiling crypto-common v0.1.6
   Compiling digest v0.10.7
   Compiling sha2 v0.10.8
   Compiling cham3_os_adapter v0.1.0 (/work/core/native/os_adapter)
   Compiling ed25519-dalek v2.1.1
   Compiling cham3_contract_format v0.1.0 (/work/core/contract_format)
   Compiling cham3_identityd v0.1.0 (/work/core/identityd)
   Compiling cham3_registryd v0.1.0 (/work/core/registryd)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.85s
profile debug
mount tmpfs /full-disk tmpfs rw,nosuid,nodev,noexec,relatime,size=2048k,mode=700,uid=1001,gid=1001,inode64 0 0
reserve_bytes 1048576
reserve_allocated_bytes 1048576
reserve_promised_bytes 1048576
reserve_preallocated
tmpfs_mount /full-disk
filler_bytes 1044480
restrictive_append_on_full_filesystem ok seq=35
reserve_bytes_after_release 0
admission_refused reserve_depleted
second_restrictive_append ok seq=36
append_after_space_returned ok seq=37
reserve_bytes 1048576
reserve_allocated_bytes 1048576
reserve_promised_bytes 1048576
reserve_preallocated
reserve_replenished
journal_records 37
journal_chained
reserve_verified
exit_status 0
restrictive reserve under a full filesystem: PASS
test_a_disk_without_the_paths_is_left_as_found (test_m2_reserve_probe.full_disk_wrapper_t.test_a_disk_without_the_paths_is_left_as_found) ... ok
test_existing_root_and_filler_are_refused_and_preserved (test_m2_reserve_probe.full_disk_wrapper_t.test_existing_root_and_filler_are_refused_and_preserved) ... ok
test_symbolic_links_and_their_targets_are_refused_and_preserved (test_m2_reserve_probe.full_disk_wrapper_t.test_symbolic_links_and_their_targets_are_refused_and_preserved) ... ok
test_restored_pre_deletion_is_detected (test_m2_reserve_probe.seeded_wrapper_defects_t.test_restored_pre_deletion_is_detected) ... ok
test_unmodified_copy_preserves_existing_paths (test_m2_reserve_probe.seeded_wrapper_defects_t.test_unmodified_copy_preserves_existing_paths) ... ok

----------------------------------------------------------------------
Ran 5 tests in 0.157s

OK
tmpfs_leftovers_absent
```

### wrapper_run_r4.py

```python
from pathlib import Path
import os
import shutil
import subprocess
root = Path('/tmp/wrapper_synthetic_disk')
root.mkdir()
existing_root = root / 'cham3_reserve_probe_root'
existing_root.mkdir()
(existing_root / 'sentinel').write_text('synthetic existing root must survive refusal\n')
filler = root / 'cham3_reserve_probe_filler'
filler.write_text('synthetic existing filler must survive refusal\n')
shutil.copytree('/work/out/m2/storage/target', '/tmp/wrapper_out/target')
environment = dict(os.environ, CARGO_HOME='/tmp/cargo_home')
command = ['sh', '/work/tests/m2/storage/check_reserve_full_disk.sh', '/tmp/wrapper_out', str(root)]
print('COMMAND', repr(command), flush=True)
completed = subprocess.run(command, cwd='/work', env=environment, capture_output=True, text=True, timeout=600)
print(completed.stdout, completed.stderr, sep='\n')
print('wrapper_exit_status', completed.returncode)
print('existing_root_exists', existing_root.exists())
print('existing_filler_exists', filler.exists())
assert completed.returncode == 1 and existing_root.exists() and filler.exists()
assert (existing_root / 'sentinel').read_text() == 'synthetic existing root must survive refusal\n'
assert filler.read_text() == 'synthetic existing filler must survive refusal\n'
print('PASS: wrapper refuses before build and preserves both pre-existing paths')
```

### wrapper_run_r4.log

```text
COMMAND ['sh', '/work/tests/m2/storage/check_reserve_full_disk.sh', '/tmp/wrapper_out', '/tmp/wrapper_synthetic_disk']

refusing: /tmp/wrapper_synthetic_disk/cham3_reserve_probe_root exists; this check creates and removes only its own paths

wrapper_exit_status 1
existing_root_exists True
existing_filler_exists True
PASS: wrapper refuses before build and preserves both pre-existing paths
```

### r4_wrapper_matrix.py

```python
from pathlib import Path
import json
import os
import shutil
import stat
import subprocess
import tempfile

RootName = "cham3_reserve_probe_root"
FillerName = "cham3_reserve_probe_filler"
Wrapper = "/work/tests/m2/storage/check_reserve_full_disk.sh"

def snapshot(path):
    info = path.lstat()
    row = [info.st_mode, info.st_ino, info.st_uid, info.st_gid, info.st_size, info.st_mtime_ns]
    if path.is_symlink():
        row += ["link", os.readlink(path)]
    elif path.is_dir():
        row += ["dir", {child.name: snapshot(child) for child in sorted(path.iterdir())}]
    else:
        row += ["file", path.read_bytes().hex()]
    return row

def populate(path, kind, targets, label):
    if kind == "dir":
        path.mkdir(mode=0o700)
        (path / "sentinel").write_text("synthetic " + label + "\n")
    elif kind == "file":
        path.write_text("synthetic " + label + "\n")
        path.chmod(0o600)
    elif kind in ("live", "dangling"):
        target = targets / label
        if kind == "live":
            if label == "root":
                target.mkdir(mode=0o700)
                (target / "sentinel").write_text("target root must survive\n")
            else:
                target.write_text("target filler must survive\n")
        path.symlink_to(target)

def run_case(fs_name, disk, label, root_kind, filler_kind):
    with tempfile.TemporaryDirectory(prefix="r4_fixture_") as fixture_name:
        fixture = Path(fixture_name)
        targets = fixture / "targets"
        targets.mkdir()
        populate(disk / RootName, root_kind, targets, "root")
        populate(disk / FillerName, filler_kind, targets, "filler")
        before = (snapshot(disk), snapshot(targets))
        spy_dir = fixture / "bin"
        spy_dir.mkdir()
        marker = fixture / "cargo_called"
        cargo = spy_dir / "cargo"
        cargo.write_text('#!/bin/sh\nprintf called > "$R4_CARGO_MARKER"\nexit 91\n')
        cargo.chmod(0o700)
        out = fixture / "out_not_created"
        target_dir = fixture / "build_not_created"
        environment = dict(os.environ, PATH=str(spy_dir) + ":" + os.environ["PATH"],
                           R4_CARGO_MARKER=str(marker), CHAM3_REPO_ROOT="/work",
                           CHAM3_M2_RESERVE_TARGET_DIR=str(target_dir))
        result = subprocess.run(["sh", Wrapper, str(out), str(disk)], cwd="/work",
                                env=environment, capture_output=True, text=True, timeout=30)
        assert result.returncode == 1 and "refusing:" in result.stderr, result
        assert (snapshot(disk), snapshot(targets)) == before, label
        assert not marker.exists() and not out.exists() and not target_dir.exists(), label
        print(json.dumps({"filesystem": fs_name, "case": label, "result": "PASS",
                          "exit": result.returncode, "contents_metadata_links_preserved": True,
                          "cargo_not_called": True, "output_and_target_not_created": True}), flush=True)
    for child in list(disk.iterdir()):
        if child.is_symlink() or not child.is_dir():
            child.unlink()
        else:
            shutil.rmtree(child)

def fresh(fs_name, disk, expected):
    with tempfile.TemporaryDirectory(prefix="r4_positive_out_") as out:
        environment = dict(os.environ, CHAM3_REPO_ROOT="/work",
                           CHAM3_M2_RESERVE_TARGET_DIR="/work/out/m2/storage/target")
        result = subprocess.run(["sh", Wrapper, out, str(disk)], cwd="/work",
                                env=environment, capture_output=True, text=True, timeout=600)
        assert result.returncode == expected, result.stdout + result.stderr
        if expected == 0:
            assert result.stdout.count("reserve_verified") == 2, result.stdout
            assert result.stdout.count("journal_records 37") == 2, result.stdout
        else:
            assert "refusing_to_fill" in result.stdout, result.stdout
        assert list(disk.iterdir()) == [], list(disk.iterdir())
        assert not os.path.lexists("/full-disk/" + FillerName)
        print(json.dumps({"filesystem": fs_name, "case": "fresh", "result": "PASS",
                          "exit": result.returncode, "both_profiles": expected == 0,
                          "leftovers_absent_at_disk_and_mount": True}), flush=True)

Cases = [
    ("original_root_and_filler", "dir", "file"),
    ("root_directory_only", "dir", None),
    ("filler_file_only", None, "file"),
    ("root_regular_file_only", "file", None),
    ("filler_directory_only", None, "dir"),
    ("root_live_symlink_only", "live", None),
    ("root_dangling_symlink_only", "dangling", None),
    ("filler_live_symlink_only", None, "live"),
    ("filler_dangling_symlink_only", None, "dangling"),
    ("both_live_symlinks", "live", "live"),
    ("both_dangling_symlinks", "dangling", "dangling"),
]
with tempfile.TemporaryDirectory(prefix="r4_overlay space_'_$()_") as overlay:
    for fs_name, disk in (("overlay", Path(overlay)), ("tmpfs_mount_root", Path("/full-disk"))):
        assert not list(disk.iterdir())
        for label, root_kind, filler_kind in Cases:
            run_case(fs_name, disk, label, root_kind, filler_kind)
        fresh(fs_name, disk, 1 if fs_name == "overlay" else 0)
    nested = Path("/full-disk/nested")
    nested.mkdir()
    fresh("tmpfs_subdirectory", nested, 0)
    nested.rmdir()
print("TOTAL 25 PASS: 22 preservation/refusal cases, 3 fresh paths")
```

### r4_wrapper_matrix.log

```text
{"filesystem": "overlay", "case": "original_root_and_filler", "result": "PASS", "exit": 1, "contents_metadata_links_preserved": true, "cargo_not_called": true, "output_and_target_not_created": true}
{"filesystem": "overlay", "case": "root_directory_only", "result": "PASS", "exit": 1, "contents_metadata_links_preserved": true, "cargo_not_called": true, "output_and_target_not_created": true}
{"filesystem": "overlay", "case": "filler_file_only", "result": "PASS", "exit": 1, "contents_metadata_links_preserved": true, "cargo_not_called": true, "output_and_target_not_created": true}
{"filesystem": "overlay", "case": "root_regular_file_only", "result": "PASS", "exit": 1, "contents_metadata_links_preserved": true, "cargo_not_called": true, "output_and_target_not_created": true}
{"filesystem": "overlay", "case": "filler_directory_only", "result": "PASS", "exit": 1, "contents_metadata_links_preserved": true, "cargo_not_called": true, "output_and_target_not_created": true}
{"filesystem": "overlay", "case": "root_live_symlink_only", "result": "PASS", "exit": 1, "contents_metadata_links_preserved": true, "cargo_not_called": true, "output_and_target_not_created": true}
{"filesystem": "overlay", "case": "root_dangling_symlink_only", "result": "PASS", "exit": 1, "contents_metadata_links_preserved": true, "cargo_not_called": true, "output_and_target_not_created": true}
{"filesystem": "overlay", "case": "filler_live_symlink_only", "result": "PASS", "exit": 1, "contents_metadata_links_preserved": true, "cargo_not_called": true, "output_and_target_not_created": true}
{"filesystem": "overlay", "case": "filler_dangling_symlink_only", "result": "PASS", "exit": 1, "contents_metadata_links_preserved": true, "cargo_not_called": true, "output_and_target_not_created": true}
{"filesystem": "overlay", "case": "both_live_symlinks", "result": "PASS", "exit": 1, "contents_metadata_links_preserved": true, "cargo_not_called": true, "output_and_target_not_created": true}
{"filesystem": "overlay", "case": "both_dangling_symlinks", "result": "PASS", "exit": 1, "contents_metadata_links_preserved": true, "cargo_not_called": true, "output_and_target_not_created": true}
{"filesystem": "overlay", "case": "fresh", "result": "PASS", "exit": 1, "both_profiles": false, "leftovers_absent_at_disk_and_mount": true}
{"filesystem": "tmpfs_mount_root", "case": "original_root_and_filler", "result": "PASS", "exit": 1, "contents_metadata_links_preserved": true, "cargo_not_called": true, "output_and_target_not_created": true}
{"filesystem": "tmpfs_mount_root", "case": "root_directory_only", "result": "PASS", "exit": 1, "contents_metadata_links_preserved": true, "cargo_not_called": true, "output_and_target_not_created": true}
{"filesystem": "tmpfs_mount_root", "case": "filler_file_only", "result": "PASS", "exit": 1, "contents_metadata_links_preserved": true, "cargo_not_called": true, "output_and_target_not_created": true}
{"filesystem": "tmpfs_mount_root", "case": "root_regular_file_only", "result": "PASS", "exit": 1, "contents_metadata_links_preserved": true, "cargo_not_called": true, "output_and_target_not_created": true}
{"filesystem": "tmpfs_mount_root", "case": "filler_directory_only", "result": "PASS", "exit": 1, "contents_metadata_links_preserved": true, "cargo_not_called": true, "output_and_target_not_created": true}
{"filesystem": "tmpfs_mount_root", "case": "root_live_symlink_only", "result": "PASS", "exit": 1, "contents_metadata_links_preserved": true, "cargo_not_called": true, "output_and_target_not_created": true}
{"filesystem": "tmpfs_mount_root", "case": "root_dangling_symlink_only", "result": "PASS", "exit": 1, "contents_metadata_links_preserved": true, "cargo_not_called": true, "output_and_target_not_created": true}
{"filesystem": "tmpfs_mount_root", "case": "filler_live_symlink_only", "result": "PASS", "exit": 1, "contents_metadata_links_preserved": true, "cargo_not_called": true, "output_and_target_not_created": true}
{"filesystem": "tmpfs_mount_root", "case": "filler_dangling_symlink_only", "result": "PASS", "exit": 1, "contents_metadata_links_preserved": true, "cargo_not_called": true, "output_and_target_not_created": true}
{"filesystem": "tmpfs_mount_root", "case": "both_live_symlinks", "result": "PASS", "exit": 1, "contents_metadata_links_preserved": true, "cargo_not_called": true, "output_and_target_not_created": true}
{"filesystem": "tmpfs_mount_root", "case": "both_dangling_symlinks", "result": "PASS", "exit": 1, "contents_metadata_links_preserved": true, "cargo_not_called": true, "output_and_target_not_created": true}
{"filesystem": "tmpfs_mount_root", "case": "fresh", "result": "PASS", "exit": 0, "both_profiles": true, "leftovers_absent_at_disk_and_mount": true}
{"filesystem": "tmpfs_subdirectory", "case": "fresh", "result": "PASS", "exit": 0, "both_profiles": true, "leftovers_absent_at_disk_and_mount": true}
TOTAL 25 PASS: 22 preservation/refusal cases, 3 fresh paths
```

### r4_seeded_trace.py

```python
import os
import shutil
import sys
sys.path.insert(0, "/work/tests/m2/harness")
os.environ["CHAM3_M2_RESERVE_PROBE"] = "/work/out/m2/storage/target/release/cham3_reserve_probe"
import test_m2_reserve_probe as probe_tests
case = probe_tests.seeded_wrapper_defects_t()
for disk in ("", "/full-disk"):
    probe_tests.WrapperDisk = disk
    for defect in (False, True):
        directory, script = case.seeded_copy(defect)
        try:
            status, survived, output = case.existing_paths_survive(script)
            print("filesystem", disk or "overlay", "seeded_pre_deletion", defect,
                  "wrapper_status", status, "sentinels_survive", survived,
                  "positive_path", "reserve_verified" in output, flush=True)
            assert survived == (not defect), output
            if not defect:
                assert status == 1 and "refusing:" in output
            elif disk:
                assert status == 0 and "reserve_verified" in output
            else:
                assert status == 1 and "refusing_to_fill" in output
        finally:
            shutil.rmtree(directory)
assert os.listdir("/full-disk") == []
print("PASS: seed restores data loss, pristine preserves, tmpfs empty")
```

### r4_seeded_trace.log

```text
filesystem overlay seeded_pre_deletion False wrapper_status 1 sentinels_survive True positive_path False
filesystem overlay seeded_pre_deletion True wrapper_status 1 sentinels_survive False positive_path False
filesystem /full-disk seeded_pre_deletion False wrapper_status 1 sentinels_survive True positive_path False
filesystem /full-disk seeded_pre_deletion True wrapper_status 0 sentinels_survive False positive_path True
PASS: seed restores data loss, pristine preserves, tmpfs empty
```
