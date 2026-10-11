# Internal Review: M0-001 (not the independent Codex review)

This record summarizes the internal subagent review that the task assignment required. It does not replace the independent Codex review, and it is not approval. The reviewer also wrote the M0 test suites, so this review does not independently assess those tests.

- **Reviewer / role / provider:** Claude Haiku 5.5 subagent (model ID reported as `claude-haiku-5-5`), `REVIEWER` in a read-only phase after its `TEST_ENGINEER` phase
- **Task specification:** `docs/tasks/M0-repository-build-baseline.md`
- **Base commit:** `c0fb0aaf` (coordinator setup commit; recorded base `26eada8` and task-preparation commit `d988253` precede it)
- **Reviewed commit:** implementation range `c0fb0aa..24caecb3ae3ccf47ce5924b95f4b24bff7a7ed28` (branch `task/M0-repository-build-baseline-implementer`)
- **Worktree / environment:** read-only Git inspection; throwaway `debian:trixie` linux/amd64 containers under Docker Desktop on macOS arm64 (x86-64 emulation), scratch copies only
- **Review scope:** `.gitignore`, `.yamllint.yml`, `.github/workflows/m0_checks.yml`, `build/host_packages.txt`, `build/live/**`, `build/scripts/**`, `build/vm/run_boot_test.py`, `packaging/cham3-base/debian/**`
- **Tests authored by this reviewer:** `tests/**` in commits `f9088aa`, `74b6be6`, `abfad2f` and `429528f`

## Findings

1. **F1, Medium, confirmed:** `build/scripts/scan_secrets.sh`, the per-rule `grep` invocation. Grep's stderr and exit status were discarded, so an unreadable file was silently skipped and the scan reported PASS. Reproduction: a fake PEM header in a mode-000 file, scanned as non-root, gave exit 0. This violates the task and `AGENTS.md` requirement that secret scanning must not report clean on input it did not read.
2. **F2, Low, confirmed:** `build/scripts/run_static_checks.sh`, file discovery. A `git ls-files` failure produced zero files and per-check PASS lines; the aggregate still failed through the secret scan. Reproduction: a tree copied without `.git`.
3. **F3, Low, confirmed:** `build/scripts/scan_secrets.sh`, location output. `cut -d: -f1,2` mis-reported findings in filenames that contain `:`.
4. **F4, Low, confirmed:** `build/scripts/scan_secrets.sh`, file listing. `find -type f` did not follow symlinks, so a symlinked secret was not scanned.
5. **F5, Low, confirmed (synthetic log only):** `build/vm/run_boot_test.py`, `classify_boot_log`. A rescue-mode root shell was classified PASS. The required PASS condition is a login prompt or a live-config autologin shell.
6. **Concerns / informational (not defects):**
   - the unpinned `debian:trixie` CI container tag
   - `--privileged` covers the whole `image_build` job; acceptable only on GitHub-hosted ephemeral runners
   - `CHAM3_OUT_DIR=/` was accepted by `build_image.sh`
   - plain-HTTP official mirrors; APT signature verification stays enabled
   - `rsync` is listed but apparently unused
   - only the BIOS boot path is tested
   - boot PASS is decided at the first login prompt

## Verification and limits

- **Verified without defect:**
  - the failed-build path unmounts and leaves no stale ISO
  - `rm -rf` targets are confined to a marked work directory
  - CI permissions are `contents: read`, there are no `secrets.` references and no `pull_request_target`, and checkout uses `persist-credentials: false`
  - action SHAs match the `v5.0.0` tags
  - APT authentication is never disabled
  - the image rootfs has a locked root account and no secret-pattern hits
  - `debian/copyright` license text matches `LICENSE`
  - package metadata is honest
  - the boot VM uses read-only media, no disk and no network, and QEMU is always reaped
  - naming follows `AGENTS.md`, with external-name exceptions
  - there are no out-of-scope components
- **PASS / FAIL / NOT_RUN / BLOCKED:** static checks at `24caecb`: PASS. Static test suite at `24caecb` with the reviewer's tests: PASS (59/59). Full build and boot at `24caecb`: NOT_RUN by this reviewer.
- **Unverified behavior and limitations:**
  - a real rescue-mode boot
  - GitHub Actions execution
  - the KVM path
  - interrupt handling during `lb build`
  - whether `cpio` is required

## Verdict

- **CHANGES_REQUESTED** (internal review only).
- **Required actions:** fix F1–F5 and the `CHAM3_OUT_DIR=/` guard, add regression tests, and rerun the developer flow on the final commit. The coordinator's disposition is recorded in the M0-001 handoff.
