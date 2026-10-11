# Development Status

**Evidence snapshot:** 2026-10-11. Current task M0-001 evidence: `docs/handoffs/M0-repository-build-baseline-rework.md` and independent Codex rereview `docs/reviews/M0-repository-build-baseline-codex-rereview.md` (commit `59b361651a6d5cbc0ecb91c5fc5e184ebee2eb3d`). The original handoff/review are preserved as historical evidence.

## Repository

- **Project specification:** `PROJECT.md` exists and is the canonical product specification.
- **Branch:** `main` is the sole local and remote branch, tracking `origin/main`. On 2026-10-11, the owner requested branch consolidation: the former GitHub default branch `origin` was renamed to `main`, and the 29 additional local commits through `c4745ff3ec867db948519c4ff38d7c7e3c4cdabb` were preserved by a fast-forward push. M0-001 includes owner-authorized merge `2bb14924aef910d762811a32de0b1bbbf864b2af`, published and verified at `3484304e5f6b744de66375f78494df65393e7949`. Historical verification below applies to its stated commits; branch consolidation makes no new product or full-system verification claim.
- **Inspected base commit:** `26eada8258eca60935319520c05917a8a547cdbf`.
- **Task preparation commit:** `d988253caf8e1ba55b4dc45764f6ec3e51fd2803` (parent `26eada8`). It adds the M0-001 task record and this status update. The working tree was clean when implementation started.
- **Implementation evidence (branch `task/M0-repository-build-baseline`, implementation commit `5ae0dc5`):** a Debian live-build configuration for a minimal Debian 13 amd64 live development image; build, static-check and secret-scan scripts; a documentation-only `cham3-base` M0 skeleton package; a QEMU boot-test harness; `unittest` suites; a GitHub Actions workflow; and `docs/engineering/developer_setup.md`. There is no cham3 product functionality: no services, enforcement, contracts, sandboxing, networking modes, desktop or installer.
- **Current milestone/task:** M0-001 is DONE. Codex approval and exact-target GitHub Actions passed at `8029ccd`; owner-authorized merge and remote push completed; post-merge GitHub Actions passed at `3484304e5f6b744de66375f78494df65393e7949`. R1/R2 are resolved, all acceptance criteria have evidence, and M0 build/repository foundations are complete. Later product milestones remain unimplemented.

## Verified state

- Product requirements are documented in `PROJECT.md`.
- Agent/workflow documentation is present in the repository.
- **Build status (M0-001, `5ae0dc5`):**
  - PASS for the package build and the live image build, from a clean checkout in a disposable Debian 13.7 amd64 VM under QEMU TCG emulation, following `docs/engineering/developer_setup.md`.
  - ISO sha256 `060382a0daba662d6f2354ce9abcbdde77c9337743f699ffeef2dea0e0bf1478`.
  - Native-hardware build: NOT_RUN.
  - Bit-for-bit reproducibility is not claimed.
- **Test status (M0-001, `5ae0dc5`):**
  - PASS for static checks; the static (70), packaging (8) and image (17) suites in the VM; and the boot (18) suite on the host.
  - GitHub Actions: NOT_RUN, because the branch has not been pushed.
  - Independent Codex checks at `82b0cfc`: PASS for static checks, 70 static tests with zero skips, package build/lintian and 8 package tests as non-root in an unprivileged disposable Debian trixie linux/amd64 container under emulation; 14 boot-classifier unit tests PASS on the host.
  - Independent failure-path probes: FAIL for R1 (scanner reports PASS with grep missing) and R2 (build wrapper continues after configuration failure and reports PASS). Probes used only synthetic fixtures/fault injection; they are not real image or enforcement tests.
  - Independent GitHub query: workflow-specific query BLOCKED by HTTP 404; branch-wide query PASS with no runs (`[]`). No exact-target passing GitHub Actions evidence is available.
- **Author-recorded rework status (M0-001, `526bacc`):**
  - PASS, as non-root in a disposable Debian 13.3 amd64 container and in a clean Debian 13.7 amd64 VM (TCG): static checks, 82 static tests (0 skipped), package build (lintian exit 0) and 8 package tests.
  - PASS in the VM: real image build (ISO sha256 `ed1165d57926f2521d629d0b10430f4bf4e2938410ea2bdfb433849c9e3da71d`) and 17 image tests.
  - PASS: host-QEMU BIOS boot to a login prompt, and 18 boot tests.
  - R1/R2 regression controls: 8 failures before the fix (`ac36295`), all pass after it. They are synthetic fault injection, not image or enforcement tests.
  - GitHub Actions, UEFI and native hardware: NOT_RUN.
- **Independent rereview (`8029ccdde09b205c9a477d5e6745716b9242cdfb`):**
  - PASS: static checks, 82 static tests (zero skips), real unsigned package build/lintian and 8 package tests as non-root in an unprivileged disposable Debian 13.3 `debian:trixie` linux/amd64 container under emulation.
  - PASS: original missing-grep probe returns 2; original configuration-failure probe returns 1 and calls only `config`; no ISO published. Additional non-root config/entry-failure assertions leave exactly `build.log`.
  - Sensitivity PASS: 30 scanner/wrapper tests reproduce 8 failures at tests-only `ac36295`, with positive controls passing; all 30 pass at `526bacc`. Synthetic control-flow coverage only.
  - PASS: provenance source, ISO digest binding, committed provenance/manifest checksums, config hashes, epoch and manifest consistency; committed serial-log classification; 14 synthetic boot-classifier unit tests.
  - Independent full image build/real boot rerun: NOT_RUN. Author VM records were inspected; absent raw ISO/build-log bytes were not independently hashed.
  - GitHub query PASS: `gh run list --branch task/M0-repository-build-baseline-rework --json databaseId,headSha,conclusion,status,url,workflowName` returned `[]`; GitHub Actions execution remains NOT_RUN. No exact-target passing run is evidenced.
- **Boot/image status (M0-001):**
  - PASS: the image booted to a serial login prompt in a disposable QEMU TCG VM (SeaBIOS, no disk, no network) on the macOS arm64 host.
  - UEFI and hardware boot: NOT_RUN.
  - A boot result demonstrates no security property.

## Implementation, defects, reviews

- **Exact-target GitHub Actions gate:** PASS, [run 38173172929](https://github.com/Kayyo321/cham3/actions/runs/38173172929) at `8029ccdde09b205c9a477d5e6745716b9242cdfb`. All three jobs succeeded: static checks/82 tests, unsigned package build/lintian/8 tests, real image build/17 image tests/QEMU boot/18 boot tests. Evidence: `docs/evidence/M0-repository-build-baseline/ci_8029ccd.json`. Earlier NOT_RUN observations above are historical. Privileged image work ran only on a disposable GitHub-hosted runner. No runtime enforcement or bit-for-bit reproducibility claim.
- **Post-merge GitHub Actions:** PASS, [run 38173598602](https://github.com/Kayyo321/cham3/actions/runs/38173598602) at default-branch commit `3484304e5f6b744de66375f78494df65393e7949`. All three jobs succeeded with static checks/82 tests, package build/lintian/8 tests, real image build/17 tests, QEMU boot/18 tests. Evidence: `docs/evidence/M0-repository-build-baseline/postmerge_ci_3484304.json`. Build scripts/tests/packaging/workflow remain identical to the reviewed target. `.DS_Store` is ignored at every depth and none are tracked on this branch.

- **Implemented functionality:** M0 build and repository foundations only, merged and published on `origin` with post-merge CI PASS. No cham3 product functionality from `PROJECT.md` is implemented.
- **Partial functionality:** Unknown.
- **Known defects:** R1/R2 from `82b0cfc` are closed by independent rereview at `8029ccd`; no new confirmed scoped defects. Existing concerns and verification limits remain in `docs/reviews/M0-repository-build-baseline-codex-rereview.md`.
- **Active tasks / pending gates:** M0-001 (`docs/tasks/M0-repository-build-baseline.md`) is DONE; no remaining M0 acceptance gates. Owner/coordinator selects the next milestone. Publication metadata decisions, architectural ADRs and existing review follow-ups remain outside this completed task.
- **Architectural blockers:** Details left open by `PROJECT.md` require ADRs before dependent implementation, including concrete app identity/attribution mechanism, resource-specific mediation boundaries, network backend interfaces, and operational key recovery details.

## Recommended next work

1. M0 is complete. The M1 task specification is present and remains DRAFT; assign implementation, test, and independent review owners and settle package-by-package dispositions before moving it to READY. Resolve package metadata and license placeholders before release publication.
2. Before cryptographic or privileged implementation, split specification details into owner-approved ADRs and define testable interfaces.

Do not update this status based on plans, generated code claims, or chat summaries alone. Link commits, files, and test artifacts for any new implementation claim.

## Future contract architecture proposals

- **2026-10-11:** [ADR 0001 — Eternal Contract Repository and audit commit pipeline](decisions/0001_eternal_contract_repository.md) records the owner's requested future ECR goal as **PROPOSED**, with roadmap links at M2, M5, and M6 and proposed test specifications in `engineering/TESTING.md`.
- This entry records documentation only. ECR implementation and runtime verification are **NOT_RUN**; owner approval and lifecycle/signing, storage/schema, and anti-replay recovery decisions remain pending. It does not change the historical M0 evidence snapshot above.
