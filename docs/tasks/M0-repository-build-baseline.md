# Task M0-001 — Repository and build baseline

- **State:** IN_REVIEW (handed to Codex for independent review; no verdict yet)
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

Status as of the handoff. Evidence is in `docs/handoffs/M0-repository-build-baseline.md` and `docs/evidence/M0-repository-build-baseline/`.

- [x] The task implementation is on a task-specific branch based on the recorded baseline, and the exact branch, worktree and target commit are recorded.
- [x] A clean checkout can follow the documented developer instructions to build the initial M0 image and package artifacts, without undocumented host state or production credentials.
  - Evidence: PASS in a disposable Debian 13.7 amd64 VM at `5ae0dc5`.
  - Limitation: the VM's CPU is emulated (QEMU TCG); no native-hardware build was run.
- [x] The build definition uses Debian live-build, records material inputs, tool versions and provenance, and makes no unsupported bit-for-bit reproducibility claim.
- [x] The minimal package skeleton builds, and its metadata and dependencies are documented accurately. It does not advertise absent cham3 functionality.
- [ ] CI runs the applicable static checks and package/build validation without production secrets, and exact CI and local check results are recorded.
  - Local and CI-equivalent results: PASS.
  - GitHub Actions execution: NOT_RUN, because the branch was not published. **Open.**
- [x] A boot attempt of the initial image is performed in a disposable VM, recording the artifact identity, VM environment, result and limitations.
  - PASS on QEMU TCG (BIOS) to a serial login prompt. ISO sha256 `060382a0…0bf1478`.
- [x] `docs/STATUS.md` reflects only evidence from the task and distinguishes M0 work from later milestones.
- [x] A durable handoff under `docs/handoffs/` uses `docs/templates/HANDOFF.md` and identifies the exact base and target commits, verification outcomes, risks, review state and next action.
- [ ] Codex independently reviews the exact target commit. `CHANGES_REQUESTED` returns the task to implementation. M0 is complete only after Codex returns `APPROVE` and all other acceptance criteria have evidence. **Pending.**

## Security considerations

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
- **Target commit:** implementation commit `5ae0dc526336b7f3c6dec10b94ff4b78f135d593`, which was built and booted. The review target is the docs-only evidence/handoff commit at the tip of `task/M0-repository-build-baseline`; its exact SHA is given in the Codex request. Handoff: `docs/handoffs/M0-repository-build-baseline.md`.
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
