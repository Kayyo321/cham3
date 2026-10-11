# Independent Review: M0-001 R1/R2 rework

- **Reviewer / role / provider:** Codex / independent REVIEWER, including security review / OpenAI. Author claims were context; conclusions below come from source inspection and independent execution. No subagents used.
- **Task specification:** `docs/tasks/M0-repository-build-baseline.md` at the reviewed target; `PROJECT.md` §§1, 12, 13; `AGENTS.md`; workflow, testing, orchestration and SEC-17/SEC-18 guidance.
- **Base commit:** rework base `86600a3d2be7318e55a777b98a79e17621af6a81`. Original recorded task base remains `26eada8258eca60935319520c05917a8a547cdbf`.
- **Reviewed commit:** `8029ccdde09b205c9a477d5e6745716b9242cdfb`. Built/booted implementation in the submitted evidence: `526bacccdc3c38e8d61a15cb8e50af85d4cc7557`. Tests-only sensitivity target: `ac362952b0c11828b4483458a3902b11524a1448`.
- **Worktree / environment:** `/Users/jessicabruce/cham3-worktrees/M0-001-codex-rereview`, branch `review/M0-repository-build-baseline-rework`, created from the exact review target. Execution used disposable clones in an unprivileged Debian linux/amd64 container on a macOS arm64 host. Neither the primary checkout nor the author's rework worktree was written to.
- **Review scope:** full `git diff 86600a3 8029ccd`, implementation `git diff 86600a3 526bacc`, surrounding scanner/build control flow, regression sensitivity and safety, updated documentation, committed evidence consistency and lifecycle gates. Previous review is preserved in `docs/reviews/M0-repository-build-baseline-codex.md`.
- **Review date:** 2026-10-11.

## Findings

**None: no new confirmed defects in the scoped rework.** R1 and R2 are independently verified resolved.

### R1 — resolved

- `build/scripts/scan_secrets.sh:75-79` accepts only grep statuses 0 and 1. All other statuses set the per-file failure flag; `:88-100` reports the incomplete scan and returns 2 unless a finding takes precedence with exit 1. This checks the actual invocation, so a separate dependency preflight is unnecessary for these cases.
- Re-running the exact missing-grep probe from the previous review against the target returns **2**, empty stdout and the expected value-free scan-error diagnostics. It no longer reports PASS on unscanned input.
- Non-executable grep, SIGSEGV, unexpected status 3, mixed clean/crashing files and finding/crashing-file precedence all pass independent regression execution. The earlier unreadable-file, symlink, dangling-link and path/line controls also pass as non-root. The pattern set, binary-file limitation and finding formatting are unchanged.

### R2 — resolved

- `build/scripts/build_image.sh:101-111` runs configuration and build as separately checked pipelines. A failed `cd` explicitly exits the subshell. `pipefail` preserves a failed `lb` or logging pipeline; `die` terminates the wrapper before the next step. Splitting the subshells does not change live-build's persisted work-directory configuration.
- Re-running the original configuration-failure probe against the target returns **1**, records only `config`, reports exit 42, and publishes no ISO. Source inspection and additional non-root assertions confirm configuration failure and entry failure leave the image directory containing **only `build.log`**, including removal of stale named artifacts. Entry failure invokes no live-build step. Build failure after successful configuration also stops without publishing artifacts.
- Root/host/tool guards, directory resolution, marker checks, `lb clean --purge`, cleanup boundaries and publication operations are unchanged by the diff. Diagnostic output is still appended through `tee`. The positive stub control runs both steps in the expected directory and verifies generated checksums/provenance. Synthetic outputs are not valid images or package evidence.

### Regression quality and scope

- The 8 new scanner tests and 4 new wrapper tests have positive controls and fault controls. Independent execution at `ac36295` reproduces exactly 8 failures: 6 scanner controls and 2 wrapper controls. The same 30-test command, including the existing scanner tests, passes at `526bacc`, without skips. The old `build/` tree is independently confirmed identical to `82b0cfc`.
- Wrapper tests refuse to run as root, stub `id`, `lb` and `debootstrap`, and confine writes to temporary directories. Actual process credentials remain non-root. The entry fault moves only its temporary work directory. Scanner tests use synthetic tokens assembled at runtime and restricted PATH fixtures; fault/finding checks reject fixture values in output. These tests establish control flow and scanner behavior, not privileged live-build or product security enforcement.
- Only two build scripts, two test files and documentation/evidence changed. `526bacc..8029ccd` is docs-only. No architecture, CI permission, package metadata, authentication or later-milestone functionality changed. New internal names follow the repository convention; unittest's required external method names remain at its boundary.
- Documentation accurately distinguishes synthetic regressions, author-run real VM evidence, emulation, BIOS-only boot, absent runtime checks and open CI. No new enforcement or bit-for-bit reproducibility claim was found. Previous concerns remain follow-ups: mutable container tag, privileged CI image job, host guard's OS-ID omission, metadata placeholders, and unverified UEFI/native hardware/interruption recovery. Approval here does not close those follow-ups.

## Verification and limits

### Independent environment and exact commands

Container `cham3_m0_codex_rereview`: `debian:trixie`, `--platform linux/amd64`, Debian 13.3, `dpkg --print-architecture` = `amd64`, through x86-64 user-mode emulation on macOS arm64. No `--privileged`, no production secrets, no host protection changes. Only a disposable clone was mounted read-only; checks ran in its copied `/work` tree as `builder` uid 1000.

Preparation:

```sh
git clone --quiet --no-hardlinks --no-checkout /Users/jessicabruce/cham3 /tmp/cham3_m0_codex_rereview_source
git -C /tmp/cham3_m0_codex_rereview_source checkout --quiet --detach 8029ccdde09b205c9a477d5e6745716b9242cdfb
docker run -d --name cham3_m0_codex_rereview --platform linux/amd64 -v /tmp/cham3_m0_codex_rereview_source:/source:ro debian:trixie sleep infinity
```

Inside the disposable container, root preparation was `apt-get update`, `apt-get install -y --no-install-recommends $(sed "/^#/d" /source/build/host_packages.txt)`, `cp -a /source /work`, `useradd --create-home --uid 1000 builder`, and `chown -R builder: /work`.

Observed versions: bash `5.2.37-2+b7`, grep `3.11-4`, git `1:2.47.3-0+deb13u1`, Python `3.13.5-1`, shellcheck `0.10.0-1`, yamllint `1.37.1-1`, live-build `1:20250505+deb13u1`, dpkg-dev `1.22.22`, debhelper `13.24.2`, lintian `2.122.0`.

Each primary check used `docker exec cham3_m0_codex_rereview bash -c 'cd /work && runuser -u builder -- <command>'`:

| Exact command in `/work` | Result | Independent evidence |
|---|---|---|
| `runuser -u builder -- build/scripts/run_static_checks.sh` | PASS | Exit 0; shellcheck 8 files, py_compile 18, yamllint 2, scanner; all summary entries PASS. |
| `runuser -u builder -- tests/run_tests.sh static` | PASS | Exit 0, 82 tests, zero skips, 72.252 s. Includes existing guards and defect-injection checks. |
| `runuser -u builder -- build/scripts/build_packages.sh` | PASS | Exit 0; real unsigned `.deb` build, lintian exit 0. |
| `runuser -u builder -- tests/run_tests.sh packages` | PASS | Exit 0, 8 tests, zero skips, 1.085 s; actual package plus positive/negative package fixtures. |

Sensitivity clones were created with `mkdir -p /before /fixed`, `chown builder: /before /fixed`, then as `builder`: `git clone --quiet --no-hardlinks /work /before`, `git -C /before checkout --quiet --detach ac362952b0c11828b4483458a3902b11524a1448`, and the corresponding clone/checkout of `/fixed` at `526bacccdc3c38e8d61a15cb8e50af85d4cc7557`. Two initial root clone attempts were BLOCKED by Git's ownership check before tests started; running clone as the source owner resolved preparation without relaxing that check.

| Command / environment | Result | Evidence and limits |
|---|---|---|
| In `/before`: `runuser -u builder -- python3 -m unittest -v tests/static/test_secret_scanner.py tests/static/test_build_image_failure_paths.py` | FAIL as expected; sensitivity PASS | Exit 1; 30 tests, exactly 8 failures, zero skips, 21.064 s. All positive controls pass. |
| Same command in `/fixed` | PASS | Exit 0; 30 tests, zero skips, 20.465 s. |
| `docker exec cham3_m0_codex_rereview python3 /tmp/cham3_m0_review_probe.py` | PASS for R1/R2 requirements | Exact previous-review scratch probe, copied unchanged into the container after real package build. Driver exit 0; scanner exit 2, wrapper exit 1/config exit 42, calls `config` only, published ISO false. Root is confined to the unprivileged disposable container, and stub `lb` does no mounts or real build. |
| `docker exec -i -u builder -w /work cham3_m0_codex_rereview python3 -` with assertions below | PASS | Both config and entry failures return 1 and leave exactly `build.log`; calls respectively `['config']` and `[]`. Non-root, synthetic. |
| `docker exec cham3_m0_codex_rereview runuser -u builder -- git -C /work status --porcelain` | PASS | Empty output after execution; implementation unchanged. |
| Host Python `importlib.util` load of `tests/boot/test_boot_classifier.py`, `unittest.defaultTestLoader.loadTestsFromModule`, `unittest.TextTestRunner(verbosity=1).run` | PASS | 14 synthetic classifier controls, 0.003 s. No new VM boot. |
| `gh run list --branch task/M0-repository-build-baseline-rework --json databaseId,headSha,conclusion,status,url,workflowName` | PASS query; CI NOT_RUN | Exit 0, `[]`. No passing run of the exact reviewed commit exists in the returned evidence. |
| `git diff --exit-code 82b0cfc ac36295 -- build/` | PASS | Exit 0; tests-only sensitivity baseline has the old scripts. |
| `git merge-base --is-ancestor 86600a3 8029ccd`; `git diff --name-only 526bacc 8029ccd` | PASS | Base ancestry verified; post-implementation changes only docs. |
| `git diff --check 86600a3 8029ccd` | FAIL, informational | Five trailing-whitespace lines in captured serial/step output. Preserved evidence formatting, as documented; no required fix. |
| Independent full `build/scripts/build_image.sh`, `tests/run_tests.sh image`, real VM boot and `tests/run_tests.sh boot` | NOT_RUN | This rereview did not provision a privileged build VM; raw ISO/build log are not committed. Submitted real-run records were inspected for consistency, not counted as independent execution. |
| Independent UEFI/native hardware or runtime enforcement verification | NOT_RUN | No suitable environment provisioned; runtime enforcement is outside M0 scope. |

Additional image-directory assertions used the committed wrapper test harness without changing it:

```python
import importlib.util
import os
spec = importlib.util.spec_from_file_location('failure_paths', 'tests/static/test_build_image_failure_paths.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
for mode in ('config_failure', 'entry_failure'):
    case = module.build_image_failure_paths_t('test_config_failure_stops_before_build_and_publishes_nothing')
    case.setUp()
    try:
        case.plant_stale_artifacts()
        if mode == 'entry_failure':
            case.write_tool('dpkg-query', module.FakeDpkgQueryMovingWorkDir)
        result = case.run_build_image(config_status=42 if mode == 'config_failure' else 0, build_status=0)
        case.assert_failed_without_publishing(result, ['config'] if mode == 'config_failure' else [])
        assert sorted(os.listdir(case.image_dir)) == ['build.log']
        print('PASS', mode, 'exit', result.returncode, 'calls', case.lb_calls(), 'image_dir contains only build.log')
    finally:
        case.doCleanups()
```

Independent raw check logs are in `/tmp/cham3_m0_codex_rereview_{static_checks,static_tests,package_build,package_tests,sensitivity_before,sensitivity_after,original_probes}.txt` on the review host. Those scratch logs are not durable repository artifacts; the exact results and observations above are the committed record. The unchanged original probe source is already preserved in the previous review.

### Submitted evidence consistency

Independent Python assertions over `docs/evidence/M0-repository-build-baseline/rework_526bacc/` establish **PASS**:

- Provenance source is exactly `526bacccdc3c38e8d61a15cb8e50af85d4cc7557`, with `git_tree_dirty=false`, `reproducibility=not_claimed`.
- ISO digest in provenance, `boot_result.json` and `artifact_sha256sums.txt` is identically `ed1165d57926f2521d629d0b10430f4bf4e2938410ea2bdfb433849c9e3da71d`; provenance size is 313,524,224 bytes.
- Actual committed provenance and package-manifest bytes match the checksum record. All four live-config hashes match reviewed source; SOURCE_DATE_EPOCH matches the implementation commit timestamp. The package manifest is byte-identical to the original evidence manifest.
- The committed serial log independently classifies PASS at `debian login:`. Boot metadata describes TCG/BIOS and explicitly records no runtime checks.

The author-run container/VM transcripts, before/after sensitivity logs, provenance, manifest, boot result and console were inspected. They consistently identify the implementation and qualified results. Absent ISO, package artifact and complete build-log bytes cannot be independently hashed from this repository. Hash agreement is evidence consistency, not proof of those absent bytes or a new boot. The existing author-recorded clean-build/boot acceptance evidence remains available within those limits.

## Verdict

- **APPROVE** the exact target `8029ccdde09b205c9a477d5e6745716b9242cdfb` for this independent R1/R2 rereview. Both findings are closed by source inspection, original probes and sensitive before/after regressions. No implementation was modified.
- **Required actions / next owner:** owner obtains and records passing GitHub Actions on the exact reviewed commit. CI is the remaining acceptance gate; local checks and author VM evidence cannot substitute for it. Any material implementation change requires renewed review.
- **Lifecycle:** tick the independent Codex approval criterion and set **IN_VERIFICATION** in a separate commit changing only the task record and `docs/STATUS.md`. Do not set READY_TO_MERGE while CI is open. DONE/M0 completion require owner-approved merge and post-merge checks. No push or merge performed.
- **Separate follow-up:** the user's subsequent `.DS_Store` housekeeping request is isolated on `codex/ignore-ds-store` from primary HEAD `b58600bfbddd6ff454a9ba64892caf1a3beb7a3e`, commit `1230d2a535883c5a82c9e344ab1c0ef98379632f`. It is outside the reviewed target and does not modify this review branch or either protected checkout.
