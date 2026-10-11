# Handoff: M0-001 rework (Codex findings R1, R2)

- **State:** IN_REVIEW. The revised change goes back to Codex for independent review. It has not been approved, and M0 is not complete.
- **Role:** Claude Code (Opus 5.5, `claude-opus-5-5`) acting as IMPLEMENTER for the rework, as assigned by the owner, plus the coordinator's record-keeping for task, status, handoff and evidence. No subagents were used.
- **Model/provider:** Claude Opus 5.5 / Anthropic. Independent reviewer: Codex / OpenAI (pending).
- **Working directory / branch:** `task/M0-repository-build-baseline-rework` in worktree `../cham3-worktrees/M0-001-rework`. The primary checkout and the reviewer's worktree `../cham3-worktrees/M0-001-codex-review` were not written to.
- **Task specification:** `docs/tasks/M0-repository-build-baseline.md`. Review being answered: `docs/reviews/M0-repository-build-baseline-codex.md`.
- **Baseline commit:**
  - Rework base: `86600a3d2be7318e55a777b98a79e17621af6a81`, the Codex task/status commit (CHANGES_REQUESTED). Its parent is the Codex review commit `fc2c342e6fdf35476016c9f676167fdbbc354eb7`, whose parent is the reviewed target `82b0cfcf255672ce58c15ad32a5aad201bbde7bd`.
  - The original recorded task base, `26eada8258eca60935319520c05917a8a547cdbf`, is kept as historical context.
- **Current/implementation commit:**
  - Tested implementation commit: `526bacccdc3c38e8d61a15cb8e50af85d4cc7557`.
  - Revised review target: the commit that adds this handoff, at the tip of `task/M0-repository-build-baseline-rework`. The exact SHA is given in the Codex request. Everything after `526bacc` changes only `docs/`.

Product authority: `PROJECT.md` §§1, 12 and 13, the task record, `AGENTS.md`, and SEC-17/SEC-18 in `docs/engineering/SECURITY_INVARIANTS.md`. There are no approved ADRs.

## Completed work

### Behavior and files changed

Rework commits, in order:

| Commit | Content |
|---|---|
| `52bef47` | Moves the task and status to IN_PROGRESS for the rework (docs only). |
| `ac36295` | Adds the regression tests only: `tests/static/test_secret_scanner.py` (new class `secret_scanner_dependency_t`, 8 tests) and the new file `tests/static/test_build_image_failure_paths.py` (class `build_image_failure_paths_t`, 4 tests). `build/` is unchanged from `82b0cfc`, so this commit shows the defects. |
| `526bacc` | The fixes, in `build/scripts/scan_secrets.sh` and `build/scripts/build_image.sh`, plus two matching bullets in `docs/engineering/developer_setup.md`. |
| (this commit) | Rework evidence, this handoff, and the task, status and original-handoff pointers (docs only). |

**R1: `build/scripts/scan_secrets.sh`**
- Only grep exit statuses 0 (match) and 1 (no match) count as a successful search. Any other status makes that file a scan error with `scan_secrets: ERROR cannot scan <path>` on stderr. This covers 2 (I/O or permission error), 126 and 127 (grep not executable or not found), and 128+N (killed by a signal).
- Unchanged:
  - A finding still takes precedence (exit 1, with the error lines also printed).
  - Unreadable files are still exit 2.
  - Symlinks are still followed.
  - Output still has no values: `FINDING <rule>: <path>:<line>`.
- The header comment and `--help` text describe the new rule.
- No separate preflight `command -v grep` check was added. Checking every invocation's status catches missing, non-executable and crashing grep at the point where scanning happens, and a preflight cannot detect a crash.

**R2: `build/scripts/build_image.sh`**
- The single `( cd; lb config; lb build ) | tee || build_status=$?` block is replaced by `run_lb_step config` followed by `run_lb_step build`.
- Each step runs `( cd -- "$work_dir" || { echo "build_image: cannot enter work dir …"; exit 1; }; lb "$1" ) 2>&1 | tee -a build.log`. It records the pipeline status, which under `pipefail` includes a failed `cd` or `lb`, and calls `die "lb <step> failed (exit N)"` on any non-zero status. Because `die` exits the script, `lb build` never runs after a failed entry or a failed `lb config`.
- Unchanged, and still effective on failure:
  - The earlier `rm -f` of the ISO, manifest, `build.log`, `provenance.json` and `SHA256SUMS`.
  - ISO and manifest publication, provenance and checksums all come after the steps. A failed attempt therefore leaves only the diagnostic `build.log` in the image dir.
- Directory guards, the work-dir marker, cleanup boundaries and logging are unchanged.

**Documentation:** `docs/engineering/developer_setup.md` now describes the grep-status rule and the stop-on-step-failure behavior. README is unchanged.

### Decisions made within approved scope

- **R2 tests use non-root fault injection.**
  - Stub `id`, which reports uid 0 so the wrapper passes its root check; stub `lb`, which records calls and the working directory and writes synthetic outputs only inside the expected work dir; and stub `debootstrap`.
  - The work-dir entry failure is triggered by a stub `dpkg-query`. The wrapper calls it once, just before live-build, and the stub moves the work dir away.
  - These tests skip when run as root, consistent with the existing guard tests. CI runs the static suite as non-root.
- **R1 tests use a restricted `PATH`** that contains only the tools the scanner needs. Fixtures are synthetic tokens assembled at runtime, and every test asserts that the value is never printed.
- No architecture, scope, CI, packaging or metadata decisions were changed.

## Verification

Environment labels (details are in `docs/evidence/M0-repository-build-baseline/rework_526bacc/README.md`):
- **Container:** unprivileged `debian:trixie` linux/amd64 (Debian 13.3), Docker Desktop 29.2.1 with x86-64 emulation on macOS arm64. Checks ran as non-root `builder`. Tool versions: grep 3.11-4, bash 5.2.37-2+b7, git 1:2.47.3-0+deb13u1, python3 3.13.5-1, shellcheck 0.10.0-1, yamllint 1.37.1-1, live-build 1:20250505+deb13u1, dpkg-dev 1.22.22, debhelper 13.24.2, lintian 2.122.0.
- **VM:** a disposable Debian 13.7 amd64 genericcloud VM (SHA512-verified image, fresh overlay), QEMU 10.2.1 TCG on macOS 26.3.1 Apple M2 Pro. The CPU is emulated.
- **Host-QEMU:** Homebrew QEMU 10.2.1 TCG, SeaBIOS, 1024 MiB, 2 CPUs, no disk, no network.

None of these is a native Debian 13 x86-64 machine.

| Check | Environment/command | Result | Evidence/limits |
|---|---|---|---|
| Regression sensitivity before the fix | Container, non-root, at `ac36295` (`build/` identical to `82b0cfc`): `python3 -m unittest -v tests/static/test_secret_scanner.py tests/static/test_build_image_failure_paths.py` | FAIL as intended: 30 run, 8 failures | Failing (6 for R1): `missing_grep`, `non_executable_grep`, `crashing_grep`, `unexpected_grep_status` and `crash_on_one_file_with_clean_other` all got exit 0 / PASS; `finding_takes_precedence_over_crash_on_other_file` had no scan-error line. Failing (2 for R2): `config_failure_stops_before_build…` (wrapper exit 0, PASS), `work_dir_entry_failure_runs_no_live_build` (`lb config` and `lb build` ran outside the work dir). Positive controls passing: `clean_input_passes_with_real_grep`, `finding_is_reported_with_real_grep`, `successful_stub_build_publishes_artifacts`, `build_failure_after_config_publishes_nothing`. File: `regression_sensitivity_before_fix_ac36295.txt` |
| Regression tests after the fix | Same command at `526bacc` | PASS (30/30) | `regression_sensitivity_after_fix_526bacc.txt` |
| Static checks | Container, non-root: `build/scripts/run_static_checks.sh` | PASS | shellcheck 8 files, py_compile 18, yamllint 2, scan_secrets. `container_checks.txt` |
| Static tests | Container, non-root: `tests/run_tests.sh static` | PASS (82/82, 0 skipped) | 70 earlier tests plus 12 new. The first attempt never started because of a harness quoting error (recorded) and was rerun correctly |
| Package build + lintian | Container, non-root: `build/scripts/build_packages.sh` | PASS | lintian exit 0. `.deb` sha256 `19b33e909c7f6f3d7f8e8955a9cbff6fdeb1c716508702059ea7dc7db3fa5fbc` |
| Package tests | Container, non-root: `tests/run_tests.sh packages` | PASS (8/8, 0 skipped) | Same harness note as static tests |
| Clean checkout + host packages | VM: clone of the bundle, `git checkout --detach 526bacc…`, `sudo apt-get install --no-install-recommends $(grep -v '^#' build/host_packages.txt)` | PASS | `vm_build_transcript.txt` |
| Static checks / static tests (VM) | VM, non-root: `run_static_checks.sh`; `tests/run_tests.sh static` | PASS; PASS (82/82, 0 skipped) | `vm_step_outputs.txt` |
| Package build / package tests (VM) | VM, non-root: `build_packages.sh`; `tests/run_tests.sh packages` | PASS; PASS (8/8) | `vm_step_outputs.txt` |
| Real image build | VM: `sudo build/scripts/build_image.sh` | PASS (1321 s) | ISO sha256 `ed1165d57926f2521d629d0b10430f4bf4e2938410ea2bdfb433849c9e3da71d`, 313,524,224 bytes. Provenance `git_commit` is `526bacc…`, `git_tree_dirty` false, `in_container` false, `reproducibility` `not_claimed`. Manifest: 159 packages, names and versions identical to `5ae0dc5` |
| Image tests | VM: `tests/run_tests.sh image` | PASS (17/17) | `vm_step_outputs.txt` |
| Artifact secret scan | VM: `build/scripts/scan_secrets.sh out/packages out/image` (the fixed scanner) | PASS | Pattern scan only. Binary files are skipped |
| Source tree unchanged | VM: `git status --porcelain` → 0 | PASS | `--ignored` shows only `out/` and `__pycache__/` |
| VM boot of the produced ISO | Host-QEMU: `python3 -I build/vm/run_boot_test.py --iso <ISO ed1165d5…> --qemu /opt/homebrew/bin/qemu-system-x86_64 --accel tcg --timeout-seconds 1200 --out-dir <scratch>/boot_test` | PASS (44.6 s) | `boot_result.json`, `boot_serial_console.txt`: `debian login:` on ttyS0. BIOS only. No login was performed, so no runtime checks were run |
| Boot tests | Host python3: `CHAM3_OUT_DIR=<evidence out> tests/run_tests.sh boot` | PASS (18/18) | Binds the result to the ISO hash |
| Nested boot inside the build VM | VM: `run_boot_test.py --accel auto` | NOT_RUN | Nested TCG is too slow. The host-QEMU boot of the same ISO is the evidence |
| UEFI boot; native x86-64 hardware build | n/a | NOT_RUN | No suitable environment |
| R2 fault injection as root | n/a | NOT_RUN by design | The tests skip as root so a wrapper defect cannot reach a privileged real build |
| GitHub Actions `m0_checks.yml` | GitHub | NOT_RUN | Not pushed, as instructed. **Open gate** |
| Rework diff/artifact secret inspection | Coordinator: `scan_secrets.sh` over the new evidence directory; manual review of the diff | PASS | Fixtures are synthetic. No values are printed. Scratch paths are redacted |

## Review

- **Reviewer and exact commit:** Codex reviewed `82b0cfcf255672ce58c15ad32a5aad201bbde7bd` and returned CHANGES_REQUESTED (`docs/reviews/M0-repository-build-baseline-codex.md`, commit `fc2c342`). The review record is preserved unchanged.
- **Findings and disposition (proposed; for Codex to verify):**
  - **R1 (Medium):** fixed in `526bacc`. Regression controls are in `ac36295`; they fail before the fix and pass after it.
  - **R2 (Medium):** fixed in `526bacc`. Regression controls are in `ac36295`; they fail before the fix and pass after it. The before-fix entry-failure run also showed that the old wrapper ran `lb config` and `lb build` in the caller's working directory when the work dir could not be entered.
  - **Codex's concerns (not defects):** not addressed in this rework, because they are out of R1/R2 scope. They are the unpinned CI container, the privileged image job, the missing OS ID check in the host guard, the emulation wording, and the metadata placeholders. They remain open follow-ups.
- **Independent review of this rework:** pending. The author does not approve its own work.

## Remaining

### Known issues and risks
- GitHub Actions has not run.
- All execution was emulated, using TCG and container user-mode emulation. The boot covers BIOS only and stops at the login prompt.
- The R2 tests check the wrapper's control flow with stubs; they are not real image builds.
- Package maintainer, copyright holder and license placeholders still need owner decisions.
- Committed console and step logs contain trailing whitespace, preserved as captured. Codex treated this as informational.

### Uncommitted paths or dirty state
- The rework worktree is clean after the final commit.
- The primary checkout `/Users/jessicabruce/cham3` belongs to another session (branch `codex/cham3_minimal_wallpaper`, with modified artwork). It was not touched.
- The disposable container `cham3_m0_rework`, the VM overlay, the ISO and raw logs are in coordinator scratch, outside the repository.

### Required next action and owner
1. **Codex (REVIEWER):** independently review the revised target at the tip of `task/M0-repository-build-baseline-rework`, focusing on R1/R2 and the new tests, and record the verdict.
2. **Owner:** run `m0_checks.yml` on GitHub for an exact reviewed commit and record the result. Decide the metadata placeholders before publication.
3. **After APPROVE and passing CI:** the integrator merges through the owner-approved process and runs post-merge checks. M0-001 moves to DONE only then.
