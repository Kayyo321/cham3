# Development Status

**Evidence snapshot:** 2026-10-11. This file records evidence from task M0-001 (handoff `docs/handoffs/M0-repository-build-baseline.md`) and the independent Codex review `docs/reviews/M0-repository-build-baseline-codex.md` (commit `fc2c342e6fdf35476016c9f676167fdbbc354eb7`).

## Repository

- **Project specification:** `PROJECT.md` exists and is the canonical product specification.
- **Branch:** the default branch `origin` (tracking `origin/origin`) does not yet contain M0-001. M0-001 work is on the unmerged branch `task/M0-repository-build-baseline`.
- **Inspected base commit:** `26eada8258eca60935319520c05917a8a547cdbf`.
- **Task preparation commit:** `d988253caf8e1ba55b4dc45764f6ec3e51fd2803` (parent `26eada8`). It adds the M0-001 task record and this status update. The working tree was clean when implementation started.
- **Implementation evidence (branch `task/M0-repository-build-baseline`, implementation commit `5ae0dc5`):** a Debian live-build configuration for a minimal Debian 13 amd64 live development image; build, static-check and secret-scan scripts; a documentation-only `cham3-base` M0 skeleton package; a QEMU boot-test harness; `unittest` suites; a GitHub Actions workflow; and `docs/engineering/developer_setup.md`. There is no cham3 product functionality: no services, enforcement, contracts, sandboxing, networking modes, desktop or installer.
- **Current milestone/task:** M0-001 is CHANGES_REQUESTED. Codex reviewed exact target `82b0cfcf255672ce58c15ad32a5aad201bbde7bd`; two confirmed defects require implementation fixes and renewed independent review. GitHub Actions execution remains open. M0 is not complete.

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
- **Boot/image status (M0-001):**
  - PASS: the image booted to a serial login prompt in a disposable QEMU TCG VM (SeaBIOS, no disk, no network) on the macOS arm64 host.
  - UEFI and hardware boot: NOT_RUN.
  - A boot result demonstrates no security property.

## Implementation, defects, reviews

- **Implemented functionality:** M0 build and repository foundations only, on an unmerged branch under review. No cham3 product functionality from `PROJECT.md` is implemented.
- **Partial functionality:** Unknown.
- **Known defects:** Two independent Codex findings are open at `82b0cfc`: R1 (Medium), scanner accepts search-tool execution failures other than grep status 2 (`build/scripts/scan_secrets.sh:71-75`); R2 (Medium), root image wrapper loses configuration failure inside its guarded pipeline (`build/scripts/build_image.sh:97-102`). Both have independent reproductions in `docs/reviews/M0-repository-build-baseline-codex.md`. Earlier internal-review fixes do not resolve these findings.
- **Active tasks / pending reviews:** M0-001 (`docs/tasks/M0-repository-build-baseline.md`) is CHANGES_REQUESTED. Next owner is IMPLEMENTER for R1/R2 fixes and sensitive regression tests, then coordinator for a revised handoff and renewed independent review. Owner retains the open GitHub Actions gate; approval, integration and post-merge checks remain required.
- **Architectural blockers:** Details left open by `PROJECT.md` require ADRs before dependent implementation, including concrete app identity/attribution mechanism, resource-specific mediation boundaries, network backend interfaces, and operational key recovery details.

## Recommended next work

1. Repair M0-001 R1/R2, rerun affected checks and obtain renewed independent approval for an exact revised commit. Owner then records passing `m0_checks.yml` on an exact reviewed commit; integrator merges through the owner-approved process and records post-merge checks. Resolve package metadata and license placeholders before publication. M0 remains incomplete throughout these pending gates.
2. Before cryptographic or privileged implementation, split specification details into owner-approved ADRs and define testable interfaces.

Do not update this status based on plans, generated code claims, or chat summaries alone. Link commits, files, and test artifacts for any new implementation claim.
