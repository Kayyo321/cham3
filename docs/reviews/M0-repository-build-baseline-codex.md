# Independent Review: M0-001

- **Reviewer / role / provider:** Codex / independent REVIEWER, including security review / OpenAI. No delegated review or model-consensus approval.
- **Task specification:** `docs/tasks/M0-repository-build-baseline.md` at the reviewed commit; `PROJECT.md` §§1, 12, 13; `AGENTS.md`; workflow, orchestration, testing guidance and SEC-17/SEC-18.
- **Base commit:** recorded base `26eada8258eca60935319520c05917a8a547cdbf`; task setup `d988253caf8e1ba55b4dc45764f6ec3e51fd2803` and `c0fb0aafe225bff07edaf25f43f18da0912cff4b` are excluded from the implementation diff.
- **Reviewed commit:** `82b0cfcf255672ce58c15ad32a5aad201bbde7bd`, full diff `git diff c0fb0aa 82b0cfc`. Built/booted implementation: `5ae0dc526336b7f3c6dec10b94ff4b78f135d593`. Independently confirmed `5ae0dc5..82b0cfc` changes only `docs/`.
- **Worktree / environment:** `/Users/jessicabruce/cham3-worktrees/M0-001-codex-review`, initially detached at the exact target; review branch `review/M0-repository-build-baseline` created from that target. Primary checkout was inspected for status only; its unrelated dirty artwork was preserved.
- **Review scope:** all 50 changed files, build configuration and scripts, package metadata, CI, all test suites, developer guide, README, committed build/boot evidence, lifecycle and completion claims. The internal review was read as context after direct source inspection, not accepted as approval.
- **Review date:** 2026-10-11.

## Findings

### R1 — Medium, confirmed: secret scanner accepts a failed scanner invocation

- **Location:** `build/scripts/scan_secrets.sh:71-75` (especially line 73).
- **Trigger:** `grep` is missing from PATH, cannot execute, or terminates with an error status other than 2. The scanner catches the command failure but marks the file unscanned only for status 2. A command-not-found status of 127 leaves `file_failed=0` and empty `matches`.
- **Violated requirement:** fail-closed secret scanning in this review assignment; `AGENTS.md` prohibition on weakening fail-closed verification and exposing secrets; the script and developer guide's explicit promise not to report clean for input it could not scan. This affects the distribution supply-chain checks relevant to SEC-17; it is not a claim about unimplemented runtime enforcement.
- **Evidence/reproduction:** in the disposable Debian amd64 container, run the unchanged scanner with a PATH containing working `dirname`, `mktemp`, `find`, and `rm`, but no `grep`, against a synthetic private-key-header fixture. Actual result: exit **0**, stdout `scan_secrets: PASS (no matches for the configured patterns)`, empty stderr. No search command read the fixture. The probe contains no real key or credential and prints no fixture value.
- **Impact:** an unavailable or crashed search dependency can produce a successful secret-scan gate without checking the input. Normal unreadable-file regression tests pass because they exercise grep status 2, not dependency execution failures.
- **Required fix:** recognize only grep statuses 0 and 1 as successful scanning; treat all other statuses as errors. Check the scanner dependency and add a controlled missing/non-executable/crashed-search-tool regression proving nonzero, value-free output. Preserve existing unreadable-file and mixed-finding behavior.

### R2 — Medium, confirmed: configuration failure is discarded before image building

- **Location:** `build/scripts/build_image.sh:97-102`.
- **Trigger:** `lb config` fails and a subsequent `lb build` succeeds. The whole subshell/pipeline is the left operand of `||`; Bash suppresses `errexit` within that subshell. Its status is the last command's status, so `pipefail` does not preserve an earlier failed `cd` or `lb config`.
- **Violated requirement:** the assigned failure-path review and fail-closed build verification; the task requires a repeatable build from the committed live-build definition, and SEC-17 requires failed verification not to authorize a release. A successful final command cannot establish that required configuration succeeded.
- **Evidence/reproduction:** run the unchanged root build wrapper inside the disposable, **unprivileged** container with a scratch output directory, the independently built real `.deb`, and a PATH-injected `lb` fault-injection executable. It returns 42 for `config`; its `build` branch records invocation, creates synthetic `.iso`/`.packages` outputs, and returns 0. Actual invocation log: `config`, then `build`. The wrapper logs `controlled configuration failure`, subsequently logs `build_image: PASS`, exits **0**, and copies the synthetic ISO to the output image directory with provenance and checksums.
- **Impact:** the wrapper can proceed after an unsuccessful configuration operation and label the aggregate successful. The probe demonstrates wrapper control flow only; the synthetic file is not a valid ISO, and this review does not claim to have reproduced an incorrectly configured real Debian image. Existing image tests would reject this particular synthetic file, but that does not repair the wrapper or prove earlier configuration succeeded for valid outputs.
- **Required fix:** propagate each preparation/configuration failure explicitly before permitting the next command. Add a regression with config failure and a would-be successful build; require nonzero exit, no build invocation, and no newly published successful artifacts. Cover work-directory entry failure too. Keep logging and cleanup confined to the intended directories.

### Concerns and follow-ups (not confirmed defects)

- CI uses the mutable `debian:trixie` tag. Tool versions and provenance record observations, not a pinned snapshot. Pinning the container can be a later supply-chain improvement; the task specifically requires action SHA pins, which are valid.
- The entire image job is privileged, including checked-out PR code. It is restricted to GitHub-hosted ephemeral runners, has read-only repository permission, disables persisted checkout credentials, and references no secrets or production signing credentials. A future move to self-hosted infrastructure needs separate review.
- KVM, UEFI, native hardware, interruption/unmount recovery during real live-build, and long-running boot health are unverified here. The harness decides success at a prompt; this supports the stated narrow boot-attempt claim only.
- The build-host guard checks version and dpkg architecture, but not OS ID; the later image test checks Debian ID. Consider checking ID before root build operations when refining unsupported-host handling.
- Developer setup calls x86-64 emulation unverified, while the handoff records successful emulated executions. The distinction between the validated target and actual evidence should remain explicit; neither document proves native-hardware support.
- Maintainer, copyright-holder and license-selection placeholders are openly documented and require owner decisions before publication. No change to those owner decisions is made by this review.

## Verification and limits

### Independent execution environment and commands

Host: macOS arm64. Tests ran in a disposable `debian:trixie` **linux/amd64** Docker container through x86-64 user-mode emulation, not on native Debian hardware. `dpkg --print-architecture` returned `amd64`; `/etc/os-release` reported Debian 13/trixie, `DEBIAN_VERSION_FULL=13.3`. The container was not privileged and had only a read-only mount of a disposable source clone; no host protection was changed and no secrets were used.

Preparation commands:

```sh
git clone --quiet --no-hardlinks --no-checkout /Users/jessicabruce/cham3 /tmp/cham3_m0_codex_review_source
git -C /tmp/cham3_m0_codex_review_source checkout --quiet --detach 82b0cfcf255672ce58c15ad32a5aad201bbde7bd
docker run -d --name cham3_m0_codex_review --platform linux/amd64 -v /tmp/cham3_m0_codex_review_source:/source:ro debian:trixie sleep infinity
```

Inside the container as root, preparation was `apt-get update`, `apt-get install -y --no-install-recommends $(sed "/^#/d" /source/build/host_packages.txt)`, `cp -a /source /work`, `useradd --create-home --uid 1000 builder`, and `chown -R builder: /work`. All checks below ran from `/work` unless specified.

Observed tool versions: git `1:2.47.3-0+deb13u1`, Python `3.13.5-1`, shellcheck `0.10.0-1`, yamllint `1.37.1-1`, live-build `1:20250505+deb13u1`, dpkg-dev `1.22.22`, debhelper `13.24.2`, lintian `2.122.0`.

| Check / exact command | Result | Evidence and limits |
|---|---|---|
| `runuser -u builder -- build/scripts/run_static_checks.sh` | PASS | Exit 0; shellcheck 8 files, py_compile 17, yamllint 2, scanner. All four summary entries PASS. |
| `runuser -u builder -- tests/run_tests.sh static` | PASS | Exit 0, 70 tests, zero skips, 78.643 s. Includes non-root unreadable-file/path guards and actual mutations in disposable clones. |
| `runuser -u builder -- build/scripts/build_packages.sh` | PASS | Exit 0; real unsigned package built, lintian exit 0. No image build involved. |
| `runuser -u builder -- tests/run_tests.sh packages` | PASS | Exit 0, 8 tests, 1.311 s; real package and constructed positive/negative `.deb` controls. |
| `docker exec cham3_m0_codex_review python3 /tmp/cham3_m0_review_probe.py` | FAIL for R1/R2 requirements | Probe driver exits 0 after recording observations; both commands under test incorrectly exit 0. Reproducible probe source follows below. This is controlled fault injection, not full-system verification. |
| `docker exec cham3_m0_codex_review git -c safe.directory=/work -C /work status --porcelain` | PASS | Empty output after checks/probes: implementation unchanged. |
| Host Python: load `tests/boot/test_boot_classifier.py` with `importlib.util`, then `unittest.defaultTestLoader.loadTestsFromModule(module)` and `unittest.TextTestRunner(verbosity=1).run(...)` | PASS | 14 classifier controls pass. This is synthetic-log unit coverage, not a VM boot. |
| `git ls-remote https://github.com/actions/checkout.git refs/tags/v5.0.0` | PASS | `08c6903cd8c0fde910a37f88322edcfb5dd907a8`, exact workflow pin. |
| `git ls-remote https://github.com/actions/upload-artifact.git refs/tags/v5.0.0` | PASS | `330a01c490aca151604b8cf639adc76d48f6c5d4`, exact workflow pin. |
| `gh run list --workflow m0_checks.yml --branch task/M0-repository-build-baseline --json databaseId,headSha,conclusion,status,url` | BLOCKED query | Exit 1: HTTP 404, workflow not found on default branch. This is not a successful CI result. |
| `gh run list --branch task/M0-repository-build-baseline --json databaseId,headSha,conclusion,status,url,workflowName` | PASS query; CI NOT_RUN | Exit 0, `[]`. No run evidence for the branch is available; CI remains open. |
| `git merge-base --is-ancestor 26eada8258eca60935319520c05917a8a547cdbf 82b0cfc` | PASS | Exit 0, recorded base is an ancestor. |
| `git diff --name-only 5ae0dc5 82b0cfc` | PASS | Only docs/status/task/handoff/evidence paths. |
| `git diff --check c0fb0aa 82b0cfc` | FAIL, informational | Exit 2: five trailing-whitespace lines in captured console/step output, including the login prompt. Preserved evidence formatting; not a product defect or required fix. |
| Independent full `build/scripts/build_image.sh`, `tests/run_tests.sh image`, real QEMU boot and `tests/run_tests.sh boot` | NOT_RUN | Full ISO not delivered in the repository; no new privileged build/VM provisioned. Committed prior results were inspected and checked for consistency, not relabeled as independent execution. |

### Artifact/source inspection and acceptance evidence

Independent Python assertions over the committed files establish:

- `provenance.json.source.git_commit` is exactly `5ae0dc526336b7f3c6dec10b94ff4b78f135d593` and recorded `git_tree_dirty` is false.
- ISO digest is identical in `boot_result.json`, `artifact_sha256sums.txt` and the provenance output entry: `060382a0daba662d6f2354ce9abcbdde77c9337743f699ffeef2dea0e0bf1478`.
- Actual committed provenance and package-manifest bytes hash to their recorded checksums. All four live-config file hashes match reviewed source; SOURCE_DATE_EPOCH equals the built commit's timestamp.
- The committed serial console independently classifies PASS at `debian login:`. Runtime checks were not performed. The ISO itself and complete build log were unavailable for independent hashing/inspection; matching recorded hashes is consistency evidence, not verification of absent bytes.
- Normalized `debian/copyright` license body matches repository `LICENSE`. Package source and independently built package contain documentation only; no services, maintainer scripts, configuration, runtime enforcement or M1+ metapackages are added.
- `build/live/auto/config` uses official mirrors, `--apt-secure true`, security and updates enabled, Debian main, amd64/trixie, no desktop or installer. No authentication-disabling option was found in the reviewed change. Root deletion targets are quoted and limited to work `auto`/`config` and named output artifacts; destructive root/equal/repository paths are guarded and unmarked existing work directories refused. That inspection does not establish mount recovery on interruption or defend against another privileged process changing paths concurrently.
- Provenance labels reproducibility `not_claimed`, records versions, mirror metadata and output digests, and explicitly records unavailable APT metadata if necessary. No bit-for-bit claim was found. R2 prevents trusting successful wrapper status when prerequisite configuration failed.
- README and developer guide accurately disclaim complete distribution, security enforcement and UEFI/hardware coverage; scanner and aggregate build success claims require the R1/R2 fixes.
- Naming follows snake_case and `_t` type conventions, with documented required Debian/live-build/GitHub/environment/unittest boundary spellings preserved. Normal code identifiers were inspected in addition to filename tests.

| Acceptance criterion | Assessment at reviewed target |
|---|---|
| Dedicated branch/base/target | PASS: ancestry and immutable commits inspected. |
| Clean documented package/image build | Prior VM evidence is recorded at `5ae0dc5`; package path independently PASS here. Full image not independently rerun; R2 requires failure-path repair. |
| live-build inputs/provenance; bounded reproducibility | Source/evidence inspection PASS on the normal recorded run; retain the R2 qualification. |
| Honest buildable package skeleton | PASS independently. |
| CI execution plus applicable checks | NOT_RUN on GitHub; independent local static/package checks PASS, R1/R2 remain defects. |
| Disposable boot attempt with artifact identity/limits | Prior committed BIOS/TCG evidence consistent; independent real boot NOT_RUN. |
| Evidence-bounded STATUS | Inspected; updated in the separate lifecycle commit to list these defects and preserve open CI. |
| Durable handoff | Present with commits, results and limits; author/internal-review claims were not used as independent approval. |
| Independent Codex approval | CHANGES_REQUESTED; acceptance gate remains open. |

### Reproduction probe

The following is the exact scratch probe run as `/tmp/cham3_m0_review_probe.py` in the prepared container. `/work` is the unchanged target clone with the independently built package. Root is confined to the disposable container; the replacement `lb` never performs a mount or real image build. Synthetic artifacts and all probe directories are removed on exit.

```python
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

root = Path('/work')

with tempfile.TemporaryDirectory(prefix='scanner_probe_') as scratch:
    scratch = Path(scratch)
    tools_dir = scratch / 'tools'
    tools_dir.mkdir()
    # Keep discovery tools, deliberately omit grep to simulate a missing scanner dependency.
    for tool in ('dirname', 'mktemp', 'find', 'rm'):
        (tools_dir / tool).symlink_to(shutil.which(tool))
    secret_path = scratch / 'synthetic.txt'
    secret_path.write_text('-----BEGIN ' + 'RSA PRIVATE KEY-----\n')
    completed = subprocess.run(
        ['/bin/bash', str(root / 'build/scripts/scan_secrets.sh'), str(secret_path)],
        env={**os.environ, 'PATH': str(tools_dir)}, capture_output=True, text=True,
    )
    print('SCANNER_MISSING_GREP', json.dumps({
        'exit': completed.returncode, 'stdout': completed.stdout, 'stderr': completed.stderr,
    }))

with tempfile.TemporaryDirectory(prefix='build_config_probe_') as scratch:
    scratch = Path(scratch)
    tools_dir = scratch / 'tools'
    tools_dir.mkdir()
    out_dir = scratch / 'out'
    (out_dir / 'packages').mkdir(parents=True)
    for deb in (root / 'out/packages').glob('*.deb'):
        shutil.copy2(deb, out_dir / 'packages' / deb.name)
    fake_lb = tools_dir / 'lb'
    fake_lb.write_text('''#!/bin/bash
printf '%s\\n' "$1" >> "$PROBE_CALLS"
case "$1" in
    config) echo 'controlled configuration failure' >&2; exit 42 ;;
    build) printf 'synthetic image\\n' > probe.iso
           printf 'cham3-base 0.0.0~m0\\n' > probe.packages
           exit 0 ;;
    *) exit 0 ;;
esac
''')
    fake_lb.chmod(0o755)
    calls = scratch / 'lb_calls.txt'
    completed = subprocess.run(
        [str(root / 'build/scripts/build_image.sh')], cwd=root,
        env={**os.environ, 'PATH': str(tools_dir) + ':' + os.environ['PATH'],
             'CHAM3_OUT_DIR': str(out_dir), 'PROBE_CALLS': str(calls)},
        capture_output=True, text=True,
    )
    print('BUILD_CONFIG_FAILURE', json.dumps({
        'exit': completed.returncode, 'stdout': completed.stdout, 'stderr': completed.stderr,
        'lb_calls': calls.read_text() if calls.exists() else None,
        'published_iso': (out_dir / 'image/cham3-m0-dev-amd64.iso').exists(),
    }))
```

## Verdict

- **CHANGES_REQUESTED.** Both R1 and R2 are independently reproduced, confirmed defects. Normal positive-path test success does not satisfy their failure-path requirements.
- **Required actions / next owner:** IMPLEMENTER repairs R1/R2 and adds sensitive regression controls in its own worktree; coordinator supplies the exact revised commit, affected verification and handoff; independent reviewer repeats review. Owner separately enables and records passing GitHub Actions for an exact reviewed commit. Integrator then follows the owner-approved merge process and post-merge checks.
- **Lifecycle:** set M0-001 to CHANGES_REQUESTED in the separate task/status commit. M0 remains incomplete; no approval criterion is ticked, no merge or push is authorized/performed, and no DONE/READY_TO_MERGE assertion is made.
