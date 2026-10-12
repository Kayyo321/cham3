# Handoff: M2-001 / M2-002 — completion after owner decisions

- **State:** M2-001 and M2-002 are **DONE**. G1–G6 were owner-approved on 2026-10-12 (ADRs 0003–0008 APPROVED). M2-003 is READY.
- **Role:** IMPLEMENTER. The author approved nothing; the approvals below are the owner's answers to explicit questions.
- **Model/provider:** Claude Code (Anthropic), cloud VM. Independent reviewer: Codex.
- **Working directory / branch:** `codex/m2-cryptographic-contract-core` (pushed fast-forward).
- **Task specification:** [M2 task](../tasks/M2-cryptographic-contract-core.md), M2-001 and M2-002.
- **Baseline commit:** a4bc0c4371f2298787f0876d81c0c6fd21150ea6 (implementation baseline).
- **Approved candidate:** 28a8595e627bcd092acaff4b8161522cf5d38f7f (revision 4.1), with docs-only ed2ba0d79e60b9a476af5b93685a1b4b214573f2 and 87c9cbc9bdeed40fd90b93213c22e5b854502b29. The commit adding this handoff records the owner decisions and changes only `docs/`.

Use `PROJECT.md` and approved ADRs as product-authority references.

## Completed work

- Behavior/files changed: no code in this step. The ADRs 0003–0008 statuses and approval sections, the decision index, the M2 task file and STATUS record the owner decisions.
- Decisions made within approved scope (owner, 2026-10-12):

| Decision | Owner answer |
|---|---|
| G1–G6 (ADRs 0003–0008) | Approve all six. G1 carries the reviewer's condition: re-check the glibc loader/setgid and systemd advisory items and re-qualify changed pins before M2-007. |
| ADR 0006 §9.1 default anchor mode | Option A: `same_volume` degraded default with visible status |
| ADR 0006 §9.2 who opens the recovery ceremony | Option C1: the user, after an admin `recover_user` when quarantined |
| ADR 0007 §4 one-shot semantics | Option A: the reservation consumes |
| ADR 0007 §8 time-bound strength | Option B: observed-time start eligibility (implemented in M2-006) |
| ADR 0007 §8 forward-error recovery | Deferred; no M2 operation lowers trusted time |
| ADR 0008 §3 deletion granularity | Option A: whole-account deletion only |

## Verification

| Check | Environment/command | Result | Evidence/limits |
|---|---|---|---|
| Author amd64 qualification at 28a8595 | Clean UID-1000 clone, pinned container, `qualify_m2_rust.sh` | PASS | [m2_001_amd64_vm_28a8595](../evidence/M2-cryptographic-contract-core/m2_001_amd64_vm_28a8595/) |
| GitHub Actions at 28a8595 | m2 38312981989, m1 38312982028, m0 38312982051 | PASS | [m2_001_amd64_ci_28a8595](../evidence/M2-cryptographic-contract-core/m2_001_amd64_ci_28a8595/) |
| Reviewer's own amd64 qualification, static checks/tests, CI comparison and artifact download | Owner's desktop, Debian 13.7 amd64, UID 1001, Docker 26.1.5 | PASS | Review record below |
| Reviewer's own Debian/RustSec advisory check | Tracker 2026-10-12T22:00:46Z, 82 518 506 bytes, SHA-256 `3464ca13702eea97fc84042030476ec85efc6100aa2d7e4be20b83cc820e0848`; advisory-db 57dcc0f2192e025138907a495c1ac9a7e1e3e123 | PASS: 0 behind; 23 open not-unimportant items, author triage agreed; 0 RustSec affected | Review record below |
| Miri / sanitizers / fuzzing; real services, IPC, vault, anchors, clock, enforcement | — | NOT_RUN | Belong to M2-003..M2-008 |

## Review

- Reviewer and exact commit: Codex, `review/m2-001-002-r5-security` at bd081df26bc31636b01bf470bd9b1fea9be7d20d, `docs/reviews/M2-001-002-codex-security-rereview-r5.md`, SHA-256 `77ec6d5f2c0c86102304b2227830b21f664fd78e6518cde3af502468bdb178af` (verified; parent 9cf8825; adds only that file; not merged).
- Findings and disposition: APPROVE for M2-001 and G1–G6; I04 PASS; R13/R14/R15 and all earlier items intact; no new findings.

## Remaining

- Known issues/risks: the advisory re-check before M2-007 (ADR 0003); the deferred forward-error recovery and the unadopted authenticated time source (ADR 0007 §8); every design mechanism is still NOT_RUN until its package implements it.
- Uncommitted paths or dirty state: none after this commit.
- Required next action and owner: Claude Code implements M2-003, then submits the exact candidate for independent review. At the end of M2, the owner runs the final review with the **Codex Security plugin skill** ([task file](../tasks/M2-cryptographic-contract-core.md#final-m2-review-requirement-owner-instruction-2026-10-12)).
