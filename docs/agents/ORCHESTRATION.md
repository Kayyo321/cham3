# Multi-Agent Orchestration

## Shared authority

`PROJECT.md`, approved ADRs, and task acceptance criteria are shared sources of truth. Chat history is supporting context only. Every provider receives the same task, baseline, relevant requirements, and interface decisions.

## Default provider allocation

- ChatGPT: requirements clarification, architecture discussion, task decomposition, difficult planning, and owner decision support.
- Claude Code: initial default focused implementer, tests, fixes, and handoffs.
- Codex: initial default independent reviewer, secondary implementer, regression investigation, and security review.

These are starting assignments, not claims of provider superiority. Switch roles based on demonstrated project results and current availability. For sensitive work, prefer one implementer and an independent reviewer from the other provider.

## Work lifecycle

Allowed path: `DRAFT → READY → ASSIGNED → IN_PROGRESS → IMPLEMENTED → IN_REVIEW → CHANGES_REQUESTED → IN_PROGRESS`, or `IN_REVIEW → IN_VERIFICATION → READY_TO_MERGE → MERGED → DONE`. Any active state may move to `BLOCKED`; a task may move to `CANCELLED` with a recorded reason. Rework after review returns to implementation and then repeats review and verification. Skip no required gate. DONE means evidence-backed acceptance, not an agent's assertion.

1. Coordinator defines objective, scope, exclusions, acceptance, risk, dependencies, and owner.
2. Architect resolves interfaces or records unresolved decisions before dependent implementation.
3. Implementer gets a dedicated branch/worktree and exact baseline.
4. Implementer changes code/tests, runs relevant checks, records exact results, and supplies a reviewable commit.
5. Independent reviewer inspects the task and exact commit, not just the implementation summary.
6. Confirmed defects return to implementation; material fixes repeat review and tests.
7. Integrator checks dependencies and gates, merges through approved process, and records post-merge checks.
8. Coordinator/status owner updates status and next actions; owner retains architecture, risk, and release authority.

Review disagreements are resolved with source inspection, a minimal reproduction, or a discriminating test. Repeated model votes are not evidence.

## Concurrency and worktrees

Default to one implementer plus one independent reviewer/tester. Parallel implementation is justified only with stable interfaces, distinct file ownership, low overlap, independent worktrees, and available integration/review capacity. One owner per writable worktree. Suggested branches: `task/<ID>-<slug>`, `review/<ID>-<slug>`, `test/<ID>-<slug>`, `integration/<milestone>`. Record base and target commit IDs. A reviewer may use a detached/read-only checkout.

A Git worktree separates source edits, not operating-system processes or credentials. Do not place secrets in worktrees or assume they prevent another same-account process from reading files. Preserve dirty work; never reset/clean another owner's state. Resolve conflicts by reviewing both changes and rerunning affected checks, not by choosing a side mechanically. Resume from task record, Git state, and handoff rather than reconstructing chat history.

## Efficient subscription use

Optimize accepted, verified progress per unit of usage. Use standard/fast reasoning for bounded mechanical work and stronger reasoning for uncertain cross-component or security-boundary work. Preserve capacity for independent review, repairs, and debugging. Reuse concise task packets and recorded repository findings; don't ask both providers to repeat broad discovery. Use scripts and tests for deterministic repeated checks. Delegate bounded work once; the coordinator owns delegation and prevents recursive agent chains. Subscription limits and model availability change; check current product surfaces when needed and do not assume allowances transfer between providers.

## Handoff minimum

Include task ID and state; role/provider; task/spec revision; baseline and target commit; worktree; changed behavior; commands/results with PASS/FAIL/NOT_RUN/BLOCKED; reviewer findings; known risks and limits; next owner/action. Use `docs/templates/HANDOFF.md`.
