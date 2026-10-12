# Development Status

**Evidence snapshot:** 2026-10-11. M0-001 evidence: `docs/handoffs/M0-repository-build-baseline-rework.md` and independent Codex rereview `docs/reviews/M0-repository-build-baseline-codex-rereview.md` (commit `59b361651a6d5cbc0ecb91c5fc5e184ebee2eb3d`). M1 (READY_TO_MERGE; implementation target `01bf7a1` independently approved by Codex on 2026-10-12, evidence snapshot 2026-10-12) evidence: `docs/handoffs/M1-bootable-desktop-distribution.md`, `docs/evidence/M1-bootable-desktop-distribution/` (acceptance matrix, author native evidence `native_01bf7a1/`, independent evidence `codex_01bf7a1/`, verification recipe); Codex reviews `docs/reviews/M1-bootable-desktop-distribution-codex.md` (`b82919c`), `-codex-rereview.md` (`0fec3a0`) and `-codex-01bf7a1.md` (APPROVE). The original M0 handoff/review are preserved as historical evidence.

## Repository

- **Project specification:** `PROJECT.md` exists and is the canonical product specification.
- **Branch:** `main` is the sole local and remote branch, tracking `origin/main`. On 2026-10-11, the owner requested branch consolidation: the former GitHub default branch `origin` was renamed to `main`, and the 29 additional local commits through `c4745ff3ec867db948519c4ff38d7c7e3c4cdabb` were preserved by a fast-forward push. M0-001 includes owner-authorized merge `2bb14924aef910d762811a32de0b1bbbf864b2af`, published and verified at `3484304e5f6b744de66375f78494df65393e7949`. Historical verification below applies to its stated commits; branch consolidation makes no new product or full-system verification claim.
- **Inspected base commit:** `26eada8258eca60935319520c05917a8a547cdbf`.
- **Task preparation commit:** `d988253caf8e1ba55b4dc45764f6ec3e51fd2803` (parent `26eada8`). It adds the M0-001 task record and this status update. The working tree was clean when implementation started.
- **Implementation evidence (branch `task/M0-repository-build-baseline`, implementation commit `5ae0dc5`):** a Debian live-build configuration for a minimal Debian 13 amd64 live development image; build, static-check and secret-scan scripts; a documentation-only `cham3-base` M0 skeleton package; a QEMU boot-test harness; `unittest` suites; a GitHub Actions workflow; and `docs/engineering/developer_setup.md`. There is no cham3 product functionality: no services, enforcement, contracts, sandboxing, networking modes, desktop or installer.
- **Current milestone/task:** M0-001 is DONE (evidence below). M1 (bootable desktop distribution) is READY_TO_MERGE on `task/M1-bootable-desktop`, baseline `401c6cf`. Codex independently **APPROVED** implementation target `01bf7a1f1066fae15ab51a56a5212c0a69780ab8`, with F2 and F3 security approval GRANTED (F6, F3, F7, F8 closed). The owner authorized the merge; M1 is DONE only after the merge and passing post-merge checks. Later product milestones (M2+) remain unimplemented.

## M1 — bootable desktop distribution (READY_TO_MERGE — independently approved)

- **Scope delivered (visual/package FOUNDATION only):** `packaging/cham3-desktop` (Plasma 6 Wayland + KDE core utilities, the eight cham3 plasmoids, right-edge panel layout, look-and-feel, dark Cham3 color scheme, Cham3 wallpaper, lowest-precedence defaults, and the dumpcap no-setuid fail-closed enforcement) and `packaging/cham3-full` (metapackage: cham3-desktop + the curated security suite); `desktop/` source tree; `build/live-desktop/` live-build config (Debian 13 amd64, Plasma Wayland, `--debian-installer live`, main/non-free/non-free-firmware archive areas with contrib from a signed source file, serial console, pinned Kali/LibreWolf sources bound to their hosts, explicit firmware list, staged `auto/build`). Shared `build/scripts` extended (per-profile local-package staging, image profile env) preserving M0 behavior. All nine ADR-0002 panel components are present as inert visual foundations; backend-dependent controls report unavailable/disconnected/unverified and perform no privileged action. No M2–M4 backend, no ECR, no KDE fork, no enforcement is implemented or implied.
- **Codex findings repaired (re-review of `0fec3a0`):** F6 (foreign default-deny pins now bound to the configured source host, not to signed Release fields), F3 (preinst registers the dumpcap statoverride at unpack and fails if it cannot; postinst fails instead of warning; image preseeds `install-setuid` false), F7 (contrib out of the lb archive areas, so the installer payload has no b43 installer; contrib from a signed source file), F8 (capture harness executes after credential drop, member-owned output, no skips in privileged mode). F1, F4, F5 (closed or reasonable at `0fec3a0`) preserved. The new native gates also found and the cycle repaired: firmware licence answers (owner decision 2026-10-12), Kali index confusing live-build's installer stage, the installer stage's `trusted=yes` live-medium entry, no live account (`user-setup`), live packages kept on installed systems, and installed systems without Debian sources (`apt-mirror-setup`). Dispositions: `docs/evidence/M1-bootable-desktop-distribution/acceptance_matrix.md`.
- **Verification at `01bf7a1` — PASS, produced by the implementation owner and independently rerun by Codex** (declared environment: GitHub-hosted x86-64 runners, `debian:trixie` containers, KVM; runs `m1_image` 38207816363, `m1_checks` 38207816376, `m0_checks` 38207816428): static checks PASS; static 82, desktop 261, staging 16, apt (F6 native signed-identity) 22; packages 8 with lintian exit 0; privileged capture 77 with 0 skipped; M0 image build + boot (F1 regression); native F2 source gates 7/7 against the real Debian/Kali/LibreWolf archives; M1 ISO (sha256 `f25bf75e…fd16`) with image-m1 54/54; live Plasma 6 Wayland session 37/37; disk installation with the ISO's own debian-installer; installed system 65/65 (Wayland session, right-edge panel and order, branding, categorized launcher with search and a real launch, inert widgets, dumpcap lifecycle and capture, apt update/upgrade origin). Evidence: `docs/evidence/M1-bootable-desktop-distribution/native_01bf7a1/`. Not covered: real hardware, UEFI. No result claims any M2–M4 enforcement.
- **Independent Codex review (2026-10-12):**
  - **APPROVE** `01bf7a1` for the approved M1 development scope; F2 and F3 security approval GRANTED.
  - Independent reruns, all PASS:
    - static/desktop/staging 82/261/16;
    - F6 22;
    - packages and lintian;
    - F2 real-archive 7/7;
    - privileged capture 77 with no skips;
    - an ISO rebuilt from `01bf7a1` with image-m1 54/54;
    - VM live/install/installed 37/1/65 against the rebuilt ISO.
  - Reviewer limits: QEMU BIOS only; no M2–M4 guarantees; no reproducible-ISO claim.
  - Record: `docs/reviews/M1-bootable-desktop-distribution-codex-01bf7a1.md`; evidence `docs/evidence/M1-bootable-desktop-distribution/codex_01bf7a1/`.
- **Package sourcing (owner scope amendment, 2026-10-11):** prefer Debian 13. Six foreign roles are included via **narrowly pinned, authenticated** sources (owner-approved; F2 security approval GRANTED by Codex at `01bf7a1`): LibreWolf (its official repo) and `zaproxy`, `exploitdb` (searchsploit), `ghidra`, `amass`, `radare2` from a Kali source pinned at Pin-Priority `-1` (bound to the configured source host `http.kali.org`, F6) with an 8-package allowlist (keys trusted only via `Signed-By`; verified no base package admitted). **Deferred** to a separate reviewed task (NOT in the build, documented): `mitmproxy`, `wpscan`, `metasploit-framework` (closures pull Debian base libraries from Kali). Omitted: Cutter/rizin (radare2 chosen), Burp (ZAP chosen). Tor Browser via `torbrowser-launcher`. Detail: `docs/research/M1_package_disposition.md`, `docs/research/M1_readiness_packet.md`.

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

1. M0 is complete. M1 `01bf7a1` is independently approved (F2/F3 security approval GRANTED); next action is the owner-authorized merge into `main` with post-merge checks. The three deferred tools (mitmproxy, wpscan, metasploit-framework), the firmware licence decision and package metadata/licence placeholders remain for review before any release publication.
2. Before cryptographic or privileged implementation, split specification details into owner-approved ADRs and define testable interfaces.

Do not update this status based on plans, generated code claims, or chat summaries alone. Link commits, files, and test artifacts for any new implementation claim.

## Future contract architecture proposals

- **2026-10-11:** [ADR 0001 — Eternal Contract Repository and audit commit pipeline](decisions/0001_eternal_contract_repository.md) records the owner's requested future ECR goal as **PROPOSED**, with roadmap links at M2, M5, and M6 and proposed test specifications in `engineering/TESTING.md`.
- This entry records documentation only. ECR implementation and runtime verification are **NOT_RUN**; owner approval and lifecycle/signing, storage/schema, and anti-replay recovery decisions remain pending. It does not change the historical M0 evidence snapshot above.
