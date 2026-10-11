# Handoff: M0-001

- **State:** Superseded for the current state by `docs/handoffs/M0-repository-build-baseline-rework.md`. Codex reviewed `82b0cfc` and returned CHANGES_REQUESTED (R1, R2), and the rework is in IN_REVIEW. This record is kept unchanged below as the history of the original submission.
- **Role:** Claude Code as COORDINATOR and INTEGRATOR. Subagents:
  - IMPLEMENTER
  - TEST_ENGINEER, then a read-only REVIEWER
  - TRIAGE with assigned documentation edits
- **Model/provider:**
  - Coordinator/integrator: Claude Opus 5.5 (`claude-opus-5-5`).
  - IMPLEMENTER: Claude Sonnet 5.5, effort high, reported model ID `claude-sonnet-5-5`.
  - TEST_ENGINEER/REVIEWER: Claude Haiku 5.5, effort high, reported `claude-haiku-5-5`.
  - TRIAGE: Claude Haiku 5.5, effort medium, reported `claude-haiku-5-5`.
  - The subagents could see their model IDs but not their effort levels. Effort was requested through the Agent tool.
  - Independent reviewer: Codex (pending).
- **Working directory / branch:**
  - Integration branch `task/M0-repository-build-baseline` in worktree `../cham3-worktrees/M0-001-integration`.
  - Agent branches:
    - `task/M0-repository-build-baseline-implementer` (`../cham3-worktrees/M0-001-implementer`)
    - `test/M0-repository-build-baseline` (`../cham3-worktrees/M0-001-test`)
    - `task/M0-repository-build-baseline-docs` (`../cham3-worktrees/M0-001-docs`)
- **Task specification:** `docs/tasks/M0-repository-build-baseline.md`
- **Baseline commit:** `26eada8258eca60935319520c05917a8a547cdbf` (recorded base). Setup commits that are not implementation:
  - `d988253caf8e1ba55b4dc45764f6ec3e51fd2803`: owner task preparation.
  - `c0fb0aafe225bff07edaf25f43f18da0912cff4b`: the coordinator moved the task to IN_PROGRESS and recorded the assignments.
- **Current/implementation commit:** `5ae0dc526336b7f3c6dec10b94ff4b78f135d593` is the implementation target that was built and booted. The review target is the evidence/handoff commit that adds this file, at the tip of `task/M0-repository-build-baseline`; its exact SHA is given in the Codex request. That commit changes only `docs/` (task, status, handoff, evidence), not implementation.

Product authority is `PROJECT.md` §§1, 12 and 13, together with the task record. There are no approved ADRs.

## Completed work

### Behavior and files changed

The diff from `c0fb0aa` to `5ae0dc5` touches 39 files.

**Build definition:**
- `build/live/auto/{config,build,clean}` contains the Debian live-build configuration. It targets trixie amd64, uses `main` only, and sets `--debian-installer none`, `--binary-image iso-hybrid`, and bootloaders `syslinux,grub-efi`.
- Firmware is off. Security and updates are on, with `--apt-secure true`.
- Mirrors are the official ones over HTTP. APT verifies the signed InRelease files.
- The boot parameters include a serial console.
- `build/live/config/package-lists/m0_base.list.chroot` lists only `cham3-base`.

**Build entry points:**
- `build/host_packages.txt`
- `build/scripts/run_static_checks.sh`
- `build/scripts/scan_secrets.sh`
- `build/scripts/build_packages.sh`
- `build/scripts/build_image.sh`
- `build/scripts/write_provenance.py`
- `build/vm/run_boot_test.py`

**Package skeleton:** `packaging/cham3-base/debian/` is a native, architecture-`all` package that contains documentation only. It has no maintainer scripts, services or configuration, and its description says so.

**CI:** `.github/workflows/m0_checks.yml` defines three jobs: `static_checks` (runs as a non-root user), `package_build`, and `image_build` (privileged; builds, runs the image tests, boots in QEMU, runs the boot tests).
- Permissions are `contents: read`. The workflow uses no secrets and checks out with `persist-credentials: false`.
- Actions are pinned by full SHA.
- Supporting files: `.gitignore` and `.yamllint.yml`.

**Tests (`tests/`):** stdlib `unittest` suites run through `tests/run_tests.sh static|packages|image|boot|all`.

| Suite | Tests | Coverage |
|---|---|---|
| static | 70 | Live-build config, CI text, repository hygiene, host packages, package source, secret scanner with positive and negative controls, the static-check runner, and `build_image.sh` path guards |
| packaging | 8 | Includes a deliberately defective `.deb` as a negative control |
| image | 17 | ISO 9660 identification, checksums, provenance, manifest, squashfs dpkg status, out-of-scope packages |
| boot | 18 | Classifier positive and negative controls, `result.json` binding |

**Docs:**
- `docs/engineering/developer_setup.md`: sourced setup and build guide for Debian 13 x86-64, including limitations.
- `README.md`: a "Development" section.
- `docs/reviews/M0-repository-build-baseline-internal.md`: the internal review record.
- `docs/evidence/M0-repository-build-baseline/`: curated evidence.

### Decisions made within approved scope

These are local implementation choices, not architecture.

- **Host target.** The only documented and validated build host is Debian 13 x86-64. `CHAM3_ALLOW_UNVERIFIED_HOST=1` lets the build run elsewhere and records the override in provenance.
- **Package.** `cham3-base` (name from `PROJECT.md` §12.2) is the only package. Its version is `0.0.0~m0`, distribution `UNRELEASED`.
- **Package metadata placeholders:**
  - Maintainer `cham3 developers <maintainers@cham3.invalid>`, an RFC 2606 reserved domain.
  - Copyright holder "cham3 contributors".
  - `debian/copyright` reproduces the repository `LICENSE` (AGPL-3.0) with no "or later" grant, and notes that `PROJECT.md` says the license is still to be selected.
- **Image naming.** The image is `cham3-m0-dev-amd64.iso`, with ISO metadata saying it is a development build and not a release.
- **Boot test.** It presses Enter through the QEMU monitor because the syslinux menu has no timeout. It never types credentials.
- **Live image defaults.** The image keeps the live-config defaults: user `user`, password `live`, and no `sudo` package installed. This is documented as a development-only limitation.
- **Reproducibility.** `SOURCE_DATE_EPOCH` is the HEAD commit time. Bit-for-bit reproducibility is not claimed.
- **External naming exceptions.** These names are kept as their tools require: Debian package and `debian/` file names, live-build `auto/*` and `*.list.chroot`, `.github/workflows`, `README.md`, `SHA256SUMS`, the `CHAM3_*` environment variables, and unittest method names (`setUp`, `assert*`).

### Process notes

- **Concurrent writer in the primary checkout.** At 12:55:05 another session switched the primary checkout (`/Users/jessicabruce/cham3`) to `codex/cham3-logo` and committed `bdb7ea8`. The coordinator's first setup commit then landed on that branch as `d3c225f`.
  - The setup commit was re-created on the correct base as `c0fb0aa`, and all M0 work moved to dedicated worktrees.
  - `d3c225f` remains on `codex/cham3-logo`, which the coordinator does not own. It was left untouched. The owner decides whether to drop it.
- **Agent isolation.** Each writing agent had its own worktree and branch with disjoint file ownership. Every agent diff was inspected before it was merged into the integration branch.

## Verification

Environment labels:
- **VM:** disposable Debian 13.7 amd64 VM. Official genericcloud image (SHA512 verified against cloud.debian.org), kernel `6.12.111+deb13-cloud-amd64`, QEMU 10.2.1 TCG on macOS 26.3.1, Apple M2 Pro. The CPU is emulated.
- **Host-QEMU:** Homebrew QEMU 10.2.1 TCG on the macOS arm64 host. SeaBIOS, 1024 MiB, 2 CPUs, no disk, no network.
- **Container:** `debian:trixie` linux/amd64 under Docker Desktop 29.2.1 on macOS arm64, with x86-64 user-mode emulation.

None of these is a native Debian 13 x86-64 machine.

| Check | Environment/command | Result (PASS/FAIL/NOT_RUN/BLOCKED) | Evidence/limits |
|---|---|---|---|
| Clean checkout of target | VM: `git clone target.bundle cham3 && git checkout --detach 5ae0dc5…` after `apt-get install git ca-certificates` | PASS | `vm_build_transcript.txt`, `vm_step_outputs.txt` |
| Install documented host packages | VM: `sudo apt-get install --no-install-recommends $(grep -v '^#' build/host_packages.txt)` | PASS | Tool versions are in `vm_step_outputs.txt`. live-build `1:20250505+deb13u1` |
| Static checks | VM: `build/scripts/run_static_checks.sh` (non-root) | PASS | shellcheck 8 files, py_compile 17, yamllint 2, scan_secrets |
| Static tests | VM: `tests/run_tests.sh static` (non-root) | PASS (70/70, 0 skipped) | Includes the unreadable-file and path-guard tests, which only run as non-root |
| Package build + lintian | VM: `build/scripts/build_packages.sh` | PASS | lintian exit 0 with no tags. `.deb` sha256 `19b33e90…3fa5fbc` |
| Package tests | VM: `tests/run_tests.sh packages` | PASS (8/8) | Includes the defective-package negative control |
| Image build | VM: `sudo build/scripts/build_image.sh` | PASS (1724 s) | ISO sha256 `060382a0daba662d6f2354ce9abcbdde77c9337743f699ffeef2dea0e0bf1478`, 313,524,224 bytes. Provenance `git_commit` is `5ae0dc5…`, `git_tree_dirty` false, `in_container` false, `reproducibility` `not_claimed` |
| Image tests | VM: `tests/run_tests.sh image` | PASS (17/17) | The manifest and the squashfs dpkg status both contain `cham3-base 0.0.0~m0`. No out-of-scope packages |
| Artifact secret scan | VM: `build/scripts/scan_secrets.sh out/packages out/image` | PASS | Pattern scan only. Binary ISO and squashfs are skipped |
| Source tree unchanged by build | VM: `git status --porcelain` → 0 entries; `--ignored` shows only `out/` and `__pycache__/` | PASS | `vm_step_outputs.txt` / transcript |
| VM boot of the produced ISO | Host-QEMU: `python3 -I build/vm/run_boot_test.py --iso <ISO 060382a0…> --qemu /opt/homebrew/bin/qemu-system-x86_64 --accel tcg --timeout-seconds 1200` | PASS (57.6 s) | `boot_result.json`, `boot_serial_console.txt`. The console shows `multi-user.target` and then `debian login:` on ttyS0. BIOS path only. No login was performed, so `runtime_checks` were not run |
| Boot tests | Host python3: `CHAM3_OUT_DIR=<evidence out> tests/run_tests.sh boot` | PASS (18/18) | Binds `result.json` to the ISO hash. Classifier controls |
| Documented boot test inside the build VM (nested TCG) | VM: `build/vm/run_boot_test.py --accel auto` | NOT_RUN | Stopped by the coordinator: emulation inside emulation would take over an hour. The host-QEMU boot of the same ISO is the evidence |
| UEFI boot | n/a | NOT_RUN | `grub-efi` is included but untested |
| Native Debian 13 x86-64 hardware build | n/a | NOT_RUN | Only an emulated-CPU VM and containers were available |
| CI static_checks job, simulated | Container: the workflow `run:` lines executed in order, including useradd/runuser, at `bbcc920` with tests `9d889d3` | PASS (70/70, 0 skipped) | Implementer simulation. It runs outside GitHub Actions and does not exercise the runner, `actions/checkout` or `upload-artifact` |
| CI package and image steps, equivalent | Container (privileged only for `build_image.sh`) at `62a8c6e` (test engineer) and `5cd5ea3` (implementer): build_packages, build_image, image tests, host boot | PASS | ISOs `f4d989dd…` (62a8c6e) and `8007f009…` (5cd5ea3), both boot PASS. Equivalent commands only, not GitHub Actions |
| GitHub Actions execution of `m0_checks.yml` | GitHub | NOT_RUN | The branch was not pushed: the coordinator was told not to publish. This acceptance criterion stays open until the workflow runs |
| Final diff/artifact inspection for secrets and misleading claims | Coordinator: `scan_secrets.sh` on the repo and on the evidence, plus review of package text and docs | PASS | No secrets found. Docs and package text disclaim functionality and reproducibility |
| Earlier dry run of the full VM flow | VM at `cf6fe30` | Static tests FAIL (2 test defects, later fixed in `429528f`); all other steps PASS | Historical. Superseded by the `5ae0dc5` run |

## Review

- **Internal review (not approval):** Haiku 5.5 REVIEWER reviewed the exact range `c0fb0aa..24caecb` and returned CHANGES_REQUESTED. The reviewer also authored the tests. Record: `docs/reviews/M0-repository-build-baseline-internal.md`.
  - F1 (Medium): the secret scanner failed open on unreadable files.
  - F2–F5 (Low): an empty static-check run on a git failure; scanner paths containing `:`; symlinks not followed; the classifier accepted a rescue-mode root shell.
  - Informational items were also reported.
  - Disposition: all were fixed in implementer commits `5cd5ea3` and `bbcc920`. The test engineer added regression tests in `9d889d3`, which failed on `909145c` before the fix and pass at `4f9e3a5` and `5ae0dc5`.
  - Items accepted as follow-ups: the unpinned `debian:trixie` CI tag, plain-HTTP mirrors (APT signatures still verified), and the whole `image_build` job running privileged (comment restricts it to GitHub-hosted runners).
- **Independent review:** Codex, REVIEWER. Review target: the commit that adds this handoff, at the tip of `task/M0-repository-build-baseline`. Verdict: **pending**.

## Remaining

### Known issues and risks
- CI has not run on GitHub.
- All execution was emulated. The build VM CPU was TCG, and the boot used host TCG.
- The boot result covers BIOS only and stops at the serial login prompt.
- The live image has public default credentials and is a development artifact only.
- Package maintainer, copyright holder and license are placeholders that need owner decisions.
- The CI container tag is unpinned.
- Mirrors use plain HTTP.
- Evidence of the mirror state is limited to InRelease dates and hashes in provenance.
- The image is not reproducible bit-for-bit.
- The live-build `.packages` manifest and provenance record what the build observed, not a pinned snapshot.

### Uncommitted paths or dirty state
- Integration worktree: clean after the evidence commit.
- The primary checkout `/Users/jessicabruce/cham3` is on another session's branch `codex/cham3-logo` and contains the stray coordinator commit `d3c225f`, left for the owner.
- Agent worktrees are clean.
- Coordinator scratch artifacts (ISOs, raw logs, the disposable VM disks, a throwaway SSH key for the disposable VM) live outside the repository and are not part of the deliverable.

### Required next action and owner
1. **Codex (REVIEWER):** independently review the exact review-target commit against the task and `PROJECT.md`, and record the verdict under `docs/reviews/`. If the verdict is CHANGES_REQUESTED, the task returns to IN_PROGRESS for the implementer.
2. **Owner:** run `m0_checks.yml` on GitHub (push the branch or dispatch the workflow) and record the result. This is required before M0-001 can be DONE.
3. **Owner:** decide the maintainer, copyright and license placeholders before any package is published, and decide what to do with `d3c225f` on `codex/cham3-logo`.
4. **After APPROVE and the CI evidence:** the integrator merges through the owner-approved process, runs post-merge checks, and moves the task to DONE.
