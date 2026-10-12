# Task: ECR-ADR-001-ADOPTION — Review and adopt ECR as an M2 pillar

- **State:** DONE — scoped architecture approval and local integration
- **Owner / role / provider:** Codex coordinator, architecture documentation implementer, integrator; project owner retains adoption authority.
- **Created / updated:** 2026-10-12.
- **Base commit:** `50e0ee2c16b2db619d106f969df90c84b88e67ae`.

## Objective
Complete the approval workflow for ADR 0001 and make ECR a key M2 pillar with explicit design dependencies and acceptance evidence.

## Approved requirements
- Owner request: “go through the approval motions for ADR 0001, once you're done make it a key pilar of M2 in the roadmap”.
- `PROJECT.md` §§3–5, 6.1, 8.3–8.5, 11, 14–18; SEC-01–06, SEC-11–16, SEC-18; ADR 0002 emergency deny boundary.
- Owner approved the reviewed scoped architecture, lifecycle rule, canonical-spec amendments and local integration on 2026-10-12: “Approve this scope and integrate locally”. Evidence: ADR 0001 approval section and independent adoption review.

## Scope and exclusions
- In scope: revised scoped adoption candidate; explicit denial/lifecycle boundary; canonical-spec amendments; independent architecture/security documentation review; exact owner decision record; decision index, M2/M5/M6 roadmap, testing/status, structured handoff and local delivery.
- Scope recorded before edits: approve architectural direction separately from detailed implementation design. No backend, wire format, database, state-anchor mechanism, retention policy, service signing key, authentication mechanism, or update identity policy is silently selected.
- Out of scope: product implementation, privileged operations, production keys, runtime claims, unrelated M1 status repair, remote publication.

## Dependencies and interfaces
- Existing proposal review approves proposal landing only; obtain a new independent adoption review.
- Preserve canonical service boundaries, fresh approval for signed transactions, mandatory denial, and confirmed enforcement.
- Remaining design records gate affected M2 implementation; M5/M6 use approved M2 interfaces.

## Acceptance criteria
- [x] Adoption candidate reconciles mandatory denial and human-authorized commits without background user-key signing.
- [x] Independent architecture/security reviewer approves the exact candidate; findings resolved.
- [x] Owner approves the scoped architecture, lifecycle rule, and canonical-spec amendments; evidence recorded accurately.
- [x] ADR/index/spec/roadmap/testing/status consistently represent the owner decision; ECR is a core M2 deliverable and exit gate.
- [x] Formatting/link checks pass; exact base/review/implementation commits and handoff recorded.

## Security considerations
- Default-deny and mandatory restrictions dominate history availability; lifecycle evidence is not grant authority; audit gaps cannot imply completeness. No prompt text is an enforcement boundary.
- Security review required: YES — adoption changes the contract architecture specification.

## Required verification
- macOS documentation checks: `git diff --check`; relative Markdown link target inspection for changed files; independent source/spec/diff review of immutable candidate.
- Controls: reviewer checks both valid signed authority and failure/denial paths; documentation inspection only, no simulated runtime claim.
- Runtime/cryptographic/IPC/KDE enforcement tests: NOT_RUN, no implementation in this task.

## Assignment and Git
- Implementer: primary Codex agent, task-specific worktree only.
- Reviewer/test owner: separate read-only Codex architecture/security reviewer per ORCHESTRATION.md.
- Branch/worktree: `codex/adr-0001-approval`, `/Users/jessicabruce/.codex/worktrees/adr-0001-approval/cham3`.
- Reviewed adoption candidate: `b0fdbc2f16bd4d5574d29532be9ef7bfd1051644`; independent review: `../reviews/ecr_adr_0001_adoption_review.md`.

## Decisions / blockers
- Final scoped owner approval GRANTED after concrete candidate and independent review; finalization `8c3ab8028c7d7255debee58af988218d0eaa35bf` independently APPROVED and locally integrated with passing documentation checks. See [handoff](../handoffs/ecr_adr_0001_adoption_handoff.md).
- Remaining detailed design choices are gates for future implementation, not a claim that M2 is READY or complete.
