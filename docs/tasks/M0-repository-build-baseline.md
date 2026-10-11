# Task M0-001 — Repository and build baseline

- **State:** MERGED (owner-authorized merge `2bb14924aef910d762811a32de0b1bbbf864b2af` into local `origin`; exact-target review/CI passed; remote push and post-merge verification follow)
- **Next owner / action:** Codex INTEGRATOR publishes `origin` and records post-merge GitHub Actions. DONE/M0 completion remains pending until post-merge checks pass.
- **Owner / role / provider:** Claude Code Opus 5.5 / COORDINATOR and INTEGRATOR; Sonnet 5.5 high / IMPLEMENTER; Haiku 5.5 high / TEST_ENGINEER and then REVIEWER; Haiku 5.5 medium / TRIAGE with assigned documentation edits
- **Reviewer:** Codex / independent REVIEWER
- **Created / updated:** 2026-10-11
- **Base commit:** `26eada8258eca60935319520c05917a8a547cdbf`

## Objective

Establish repeatable M0 development entry points and evidence by adding the initial Debian 13 build definition, a minimal buildable package skeleton, CI/static checks, and developer setup documentation. Demonstrate a clean-checkout build and record a VM boot result and limitations.

## Approved requirements

- `PROJECT.md` §§1, 12, and 13: Debian 13 (Trixie), x86-64 primary architecture, Debian live-build as the initial image-building system, Debian packaging, and the repository's intended subsystem/package boundaries.
- `docs/engineering/ROADMAP.md`, M0: Debian 13 build definition, package skeleton, CI/static checks, developer setup, clean-checkout build evidence, and VM boot result with limitations.
- `docs/engineering/WORKFLOW.md` and `docs/agents/ORCHESTRATION.md`: exact baseline and target commits, focused scope, evidence-based result labels, separate ownership for concurrent work, independent review, and structured handoff.
- `AGENTS.md`: default `snake_case` naming, preservation of existing work, no production secrets, and no unsupported security or implementation claims.
- For this M0 task, the documented and validated build-host target is Debian 13 Trixie on x86-64. Other build hosts are unsupported or unverified for this task. This is a narrow development convention, not a product architecture decision or a claim about the runtime system's supported hardware.

## Scope and exclusions

- **In scope:** Debian live-build configuration for the initial M0 artifact; only the minimum useful Debian package skeleton required to build and demonstrate package/image integration; CI and static checks; developer setup and build documentation; input/version/provenance recording; clean-checkout build and disposable-VM boot evidence; evidence-backed status and task updates.
- **Out of scope:** KDE desktop delivery, installer integration, cham3 enforcement services, contract/key implementation, sandboxing, per-app networking, production APT repository signing, release infrastructure, or any M1+ product behavior. Do not create dummy services/packages that imply unimplemented features. Do not claim M0 output is a complete or installable cham3 distribution unless it meets the applicable `PROJECT.md` requirements.
- Do not alter `PROJECT.md` or make architecture/security decisions. If a required choice is not settled by approved requirements, record it as a blocker and stop dependent work.

## Dependencies and interfaces

- **Dependencies:** Confirmed clean starting tree at the base commit above; Debian 13 Trixie x86-64 build environment for the validated path; disposable VM for boot verification.
- **Agreed interfaces:** Use Debian live-build as directed by `PROJECT.md` §12.1. Keep package definitions aligned with the responsibilities and names in §12.2 where packages are introduced. CI and development must not require production release-signing keys or user contract-signing keys.
- If the actual checkout or baseline differs from the recorded values, preserve all work and report the mismatch before implementation.

## Acceptance criteria

Status after independent rereview. Evidence is in `docs/handoffs/M0-repository-build-baseline-rework.md`, `docs/evidence/M0-repository-build-baseline/rework_526bacc/` and `docs/reviews/M0-repository-build-baseline-codex-rereview.md`.

- [x] The task implementation is on a task-specific branch based on the recorded baseline, and the exact branch, worktree and target commit are recorded.
- [x] A clean checkout can follow the documented developer instructions to build the initial M0 image and package artifacts, without undocumented host state or production credentials.
  - Author-recorded evidence: PASS in a disposable Debian 13.7 amd64 VM at `526bacccdc3c38e8d61a15cb8e50af85d4cc7557`; independently inspected for consistency. Independent package build PASS at the exact review target; independent full image rerun NOT_RUN.
  - Limitation: the VM's CPU is emulated (QEMU TCG); no native-hardware build was run.
- [x] The build definition uses Debian live-build, records material inputs, tool versions and provenance, and makes no unsupported bit-for-bit reproducibility claim.
- [x] The minimal package skeleton builds, and its metadata and dependencies are documented accurately. It does not advertise absent cham3 functionality.
- [x] CI runs the applicable static checks and package/build validation without production secrets, and exact CI and local check results are recorded.
  - Independent local checks at `8029ccd`: PASS; they do not satisfy GitHub Actions execution.
  - GitHub Actions PASS at exact reviewed target `8029ccdde09b205c9a477d5e6745716b9242cdfb`: [run 38173172929](https://github.com/Kayyo321/cham3/actions/runs/38173172929). Static, package and image jobs all succeeded. Durable run/step metadata and log summaries: `docs/evidence/M0-repository-build-baseline/ci_8029ccd.json`.
- [x] A boot attempt of the initial image is performed in a disposable VM, recording the artifact identity, VM environment, result and limitations.
  - Author-recorded PASS on QEMU TCG (BIOS) to a serial login prompt at `526bacc`. ISO sha256 `ed1165d57926f2521d629d0b10430f4bf4e2938410ea2bdfb433849c9e3da71d`; independently confirmed consistency across committed records and classified the serial log. Independent real boot rerun NOT_RUN.
- [x] `docs/STATUS.md` reflects only evidence from the task and distinguishes M0 work from later milestones.
- [x] A durable handoff under `docs/handoffs/` uses `docs/templates/HANDOFF.md` and identifies the exact base and target commits, verification outcomes, risks, review state and next action.
- [x] Codex independently reviews and approves the exact target commit. **APPROVE** at `8029ccdde09b205c9a477d5e6745716b9242cdfb`; R1/R2 independently verified resolved. Review: `docs/reviews/M0-repository-build-baseline-codex-rereview.md`, commit `59b361651a6d5cbc0ecb91c5fc5e184ebee2eb3d`. The earlier CHANGES_REQUESTED review at `82b0cfc` is preserved as history.

### Independent Codex review (2026-10-11)

- **Verdict / exact target:** CHANGES_REQUESTED at `82b0cfcf255672ce58c15ad32a5aad201bbde7bd`; built/booted implementation remains `5ae0dc526336b7f3c6dec10b94ff4b78f135d593`.
- **Required fix R1 (Medium):** `build/scripts/scan_secrets.sh:71-75` accepts grep execution failures other than status 2. Missing grep was independently reproduced as exit 0 / PASS on unscanned input. Accept only statuses 0/1 as successful scanning; handle all others as scan errors and add dependency-failure controls without printing fixture values.
- **Required fix R2 (Medium):** `build/scripts/build_image.sh:97-102` loses configuration failure inside the subshell/pipeline guarded by `||`. Independent fault injection returned 42 from `lb config`, yet invoked `lb build` and obtained wrapper exit 0 / PASS with synthetic outputs. Explicitly propagate work-directory/configuration failures; add a regression requiring no subsequent build or newly published successful artifacts.
- **Independent normal-path verification:** PASS for static checks, 70 static tests (zero skips), unsigned package build/lintian and 8 package tests as non-root in a disposable `debian:trixie` linux/amd64 container under emulation. PASS for 14 host classifier unit tests and committed evidence consistency. Independent real image build/boot: NOT_RUN. Full commands, evidence and limits are in the review.
- **CI gate:** workflow-specific `gh run list` query is BLOCKED by HTTP 404 (workflow absent on default branch); branch-wide run query succeeded with `[]`. GitHub Actions execution remains NOT_RUN and does not satisfy acceptance. Simulations are not CI approval.
- **Next owner / action:** IMPLEMENTER repairs R1/R2 in its assigned worktree, adds sensitive regressions and supplies an exact revised target with affected checks and handoff. Coordinator routes renewed independent review. Owner runs and records GitHub Actions on an exact reviewed commit. Integrator follows owner-approved merge and post-merge checks only after all required gates pass. No push or merge was performed by the reviewer; M0 is not complete.

### R1/R2 rework (2026-10-11)

- **Branch / base / commits:** `task/M0-repository-build-baseline-rework` from `86600a3`. Regression tests are in `ac36295`; its `build/` equals the reviewed `82b0cfc`. Fixes are in `526bacc`. The evidence and handoff commit at the branch tip is the revised review target.
- **R1:** `scan_secrets.sh` accepts only grep statuses 0 and 1 as a successful search. Any other status (missing, non-executable, crashed or failing grep) is a per-file scan error, exit 2. A finding still takes precedence, with exit 1.
- **R2:** `build_image.sh` runs the work-dir entry, `lb config` and `lb build` as separately checked steps. A failed entry or `lb config` stops the wrapper before `lb build`. A failed attempt leaves only `build.log`.
- **Regression sensitivity:** the 12 new tests at `ac36295` gave 8 failures, all fault controls; the positive controls passed. At `526bacc` all of them pass. Fault injection is non-root and synthetic, so it is not image-build evidence.
- **Affected checks at `526bacc`:** PASS, as non-root in a disposable Debian 13 amd64 container and again in a clean Debian 13.7 amd64 VM (TCG):
  - static checks
  - 82 static tests, 0 skipped
  - package build, lintian exit 0
  - 8 package tests
- **Image and boot at `526bacc`:**
  - Real image build in the VM: PASS, 1321 s. ISO sha256 `ed1165d57926f2521d629d0b10430f4bf4e2938410ea2bdfb433849c9e3da71d`.
  - 17 image tests: PASS.
  - Host-QEMU BIOS boot to `debian login:`: PASS, 44.6 s.
  - 18 boot tests: PASS.
  - UEFI, native hardware and GitHub Actions: NOT_RUN.
- **Evidence:** `docs/evidence/M0-repository-build-baseline/rework_526bacc/`.
- **Next owner at submission:** Codex, for renewed independent review. The owner holds the GitHub Actions gate. At submission, CI and Codex approval were open; the rereview below closes only the approval gate.

### Independent Codex rereview (2026-10-11)

- **Verdict / exact target:** APPROVE at `8029ccdde09b205c9a477d5e6745716b9242cdfb`, from rework base `86600a3d2be7318e55a777b98a79e17621af6a81`; tested implementation `526bacccdc3c38e8d61a15cb8e50af85d4cc7557`. No new confirmed scoped defects; R1/R2 closed.
- **Independent checks:** PASS for static checks, 82 static tests (zero skips), real unsigned package build/lintian and 8 package tests as non-root in an unprivileged disposable `debian:trixie` linux/amd64 container under emulation. Both original failure probes now satisfy the requirements. Additional non-root checks confirm failed entry/configuration leaves only `build.log`.
- **Sensitivity:** the 30-test scanner/wrapper command independently produces exactly 8 failures at `ac362952b0c11828b4483458a3902b11524a1448`, with positive controls passing, and passes all 30 at `526bacc`. Synthetic fault injection is not image-build evidence.
- **Evidence and limits:** committed provenance source, ISO digest binding, committed-byte checksums, config hashes, epoch and manifest consistency PASS. Serial-log classification and 14 classifier unit tests PASS. Independent full image/real boot rerun NOT_RUN; author VM records remain qualified evidence. Full commands and limits: `docs/reviews/M0-repository-build-baseline-codex-rereview.md`.
- **Remaining gate / next owner:** Owner supplies passing GitHub Actions for the exact reviewed commit. Independent `gh run list --branch task/M0-repository-build-baseline-rework --json databaseId,headSha,conclusion,status,url,workflowName` returned `[]` (query PASS, CI NOT_RUN). Task is IN_VERIFICATION; no READY_TO_MERGE, merge, push or M0 completion claim.

## Security considerations

### Owner-authorized integration (2026-10-11)

- Scope: approved M0 rework and review/status records, plus the requested `.DS_Store` ignore rule. Unrelated artwork/ADR branches and dirty primary files are excluded. Primary and author/reviewer worktrees are preserved.
- Integration base: `d988253caf8e1ba55b4dc45764f6ec3e51fd2803`, local `origin` tracking remote `origin/origin`. Candidate branch `codex/m0-origin-ready` in `../cham3-worktrees/M0-001-final-integration`, from reviewed-record tip `f5439cc604dae3dc6b9c4901890c4b676abd3d34`.
- Owner instruction now authorizes publishing and merging; earlier no-push/no-merge limits described in historical reviews no longer apply to this integration phase.
- Published reviewed task branch with `git push origin refs/heads/task/M0-repository-build-baseline-rework:refs/heads/task/M0-repository-build-baseline-rework`, triggering the exact-target CI run above.
- GitHub-hosted ephemeral `ubuntu-latest` runners, `debian:trixie` amd64 containers: PASS for `build/scripts/run_static_checks.sh`, 82 static tests as non-root builder, unsigned package build/lintian exit 0, 8 package tests, real live-build image, 17 image tests, disposable QEMU boot and 18 boot tests. Image job is privileged only inside that ephemeral runner; no privileged host changes or production secrets were used.
- `.DS_Store` rule ported in `827d872de9fd57ccd42feb244e0afd42f2300c4f` while preserving existing M0 ignore rules. `git check-ignore` PASS for root/nested paths; `git ls-files '*DS_Store*'` returns no tracked files in the candidate. The original cleanup commit `1230d2a` also untracks three files on the separate artwork-based branch, preserving disk files; its unrelated ancestors are not merged.
- All acceptance gates were recorded in READY_TO_MERGE commit `833c39b88ebbbb50c0eeb54776158b9d610afe9a`. `git merge --no-ff codex/m0-origin-ready` then produced merge `2bb14924aef910d762811a32de0b1bbbf864b2af`, with first parent `d988253caf8e1ba55b4dc45764f6ec3e51fd2803` and candidate parent `833c39b88ebbbb50c0eeb54776158b9d610afe9a`. No conflicts or implementation edits.
- Post-merge GitHub Actions will run on the published branch. Required post-merge checks remain pending; MERGED does not mean DONE or product security enforcement.

- Build definitions, package metadata, and CI affect the distribution supply chain and require independent review.
- Do not put production signing keys, user signing keys, tokens, or other secrets in the repository, ordinary CI, logs, or build artifacts.
- Keep claimed results bounded to what was exercised. A successful build or VM boot does not demonstrate application isolation, contract authorization, or network enforcement.
- **Security review required:** YES. Codex is the independent reviewer for this M0 change; M0 does not authorize claims that product security enforcement is implemented.

## Required verification

- From a clean checkout of the task target, run the documented build and package-validation commands; record exact command, environment, artifacts, result, and limitations.
- Run the applicable local static checks and confirm the CI workflow executes those checks; record exact results.
- Boot the produced initial image in a disposable VM and record environment, result, and limitations. Do not make privileged host changes.
- Inspect the final diff and generated artifacts for accidental secrets and misleading claims about implemented functionality.
- Use `PASS`, `FAIL`, `NOT_RUN`, or `BLOCKED` for each check. `NOT_RUN`/`BLOCKED` does not satisfy an acceptance criterion that requires successful execution.

## Assignment and Git

- **Coordinator / integrator:** Claude Code, Opus 5.5; canonical roles `COORDINATOR` and `INTEGRATOR` from `docs/agents/ROLES.md`.
- **Implementation owner:** Sonnet 5.5 high subagent; canonical role `IMPLEMENTER`.
- **Test and internal review owner:** Haiku 5.5 high subagent; canonical roles `TEST_ENGINEER` and, in a separate read-only phase, `REVIEWER`. The agent must identify tests it authored; its review is not the final Codex review.
- **Documentation/research owner:** Haiku 5.5 medium subagent; canonical role `TRIAGE`, with narrowly scoped documentation edits explicitly assigned. It must not invent architecture or alter canonical product requirements.
- **Independent reviewer:** Codex, canonical role `REVIEWER`.
- **Branch/worktree:** `task/M0-repository-build-baseline`, created from the recorded base commit. Each writing subagent must own a separate worktree/branch with disjoint file ownership. If isolation is unavailable, run write phases serially; do not permit concurrent edits to a shared writable worktree.
- **Current target commit:** `8029ccdde09b205c9a477d5e6745716b9242cdfb` on `task/M0-repository-build-baseline-rework`, worktree `../cham3-worktrees/M0-001-rework`, rework base `86600a3d2be7318e55a777b98a79e17621af6a81`. Built/booted implementation `526bacccdc3c38e8d61a15cb8e50af85d4cc7557`; changes after it are docs-only. Handoff: `docs/handoffs/M0-repository-build-baseline-rework.md`. Original implementation and assignment below are historical context.
- **Setup commits (not implementation):** `d988253caf8e1ba55b4dc45764f6ec3e51fd2803` (owner task preparation: this record and `docs/STATUS.md`, parent is the recorded base) and the coordinator commit that moved this task to `IN_PROGRESS` and recorded the assignments below.

### Coordinator assignment (2026-10-11)

The integration branch `task/M0-repository-build-baseline` is checked out in the dedicated worktree `../cham3-worktrees/M0-001-integration` and is owned by the coordinator/integrator. The primary checkout is not used for M0-001 writes. At 12:55:05 on 2026-10-11, another session switched it to the unrelated branch `codex/cham3-logo` and committed there. Each writing subagent owns one separate worktree and branch, created from the setup commit, with disjoint file ownership. Integration happens only after the coordinator inspects each agent's exact diff.

| Role | Branch | Worktree | Owned paths |
|---|---|---|---|
| COORDINATOR / INTEGRATOR | `task/M0-repository-build-baseline` | `../cham3-worktrees/M0-001-integration` | `docs/tasks/M0-repository-build-baseline.md`, `docs/STATUS.md`, `docs/handoffs/`, `docs/evidence/`, integration merges |
| IMPLEMENTER | `task/M0-repository-build-baseline-implementer` | `../cham3-worktrees/M0-001-implementer` | `.gitignore`, `.yamllint.yml`, `.github/workflows/`, `build/`, `packaging/` |
| TEST_ENGINEER, then read-only REVIEWER | `test/M0-repository-build-baseline` | `../cham3-worktrees/M0-001-test` | `tests/` (review phase: read-only, no edits) |
| TRIAGE with assigned docs | `task/M0-repository-build-baseline-docs` | `../cham3-worktrees/M0-001-docs` | `docs/engineering/developer_setup.md`, the development section of `README.md` |

The entry-point names, output layout, and provenance fields given to the subagents are local implementation choices within this task's scope. They are not architecture decisions.

### Execution environment note

The coordinator's host is macOS on Apple Silicon (arm64), so it is not a Debian 13 x86-64 build host. Builds and checks for this task run only inside disposable Debian 13 `amd64` environments (containers and/or a QEMU virtual machine) that run on that host. The verification record names each environment exactly. A result obtained under x86-64 emulation is labelled as such and is not presented as a native Debian 13 x86-64 host result.

## Decisions / blockers

- **Open owner decisions:** None within the narrow scope of this task. The Debian 13 Trixie x86-64 build-host target is limited to the initial validated development path; broadening it requires a later explicit decision.
- **BLOCKED condition and next action:** If the exact baseline/tree differs, a required build-host/architecture choice falls outside the scoped convention, an exact requested subagent model is unavailable, or safe VM verification cannot be performed, preserve work and report the blocker and the next owner/action. Do not silently substitute models or weaken acceptance criteria.
