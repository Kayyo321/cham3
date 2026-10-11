# Engineering Workflow

## Start and scope

Read `AGENTS.md`, `PROJECT.md`, current `docs/STATUS.md`, and only the role/task-specific references needed. Check branch, commit, and dirty state. A task must state objective, approved requirements, exclusions, dependencies, interfaces, acceptance criteria, security impact, tests, baseline, and owner before implementation.

All agents follow the naming rules in `AGENTS.md`: `snake_case` by default, `PascalCase` only for constants and preprocessor definitions/macros, `_t` suffix for type names, and no `camelCase`. If a language/tool requires a different external spelling, isolate that exception at the boundary and explain it in the handoff.

## Change cycle

1. Confirm task state is READY and dependencies/interfaces are settled.
2. Assign one owner and a dedicated worktree for concurrent work.
3. Implement only the approved scope. Add tests for changed observable behavior and negative security cases where relevant.
4. Run checks appropriate to the changed layer. Record exact environment and commands; classify each result PASS, FAIL, NOT_RUN, or BLOCKED.
5. Create a reviewable commit or exact immutable review reference; hand off using the template.
6. Independent review examines the exact change and relevant context. Security-sensitive changes require security review.
7. Address findings and repeat affected checks and review.
8. Integrator verifies required gates and dependencies, then merges using the repository's established protected process.
9. Run applicable post-merge regression checks, update status, and record remaining work.

## Evidence and integrity

Tests must establish the environment is functional (positive control) and the restriction works (negative control). A mock, static configuration inspection, or VM simulation has only the scope it actually exercises. Report limitations. Never change security policy solely to satisfy a test. If test design seems defective, demonstrate that with a controlled defect in a disposable environment.

## Git and interruption

Preserve unrelated changes. Capture baseline and target commits. Never discard unknown dirty work. On interruption, leave a handoff with current state, uncommitted paths, checks completed, and a specific next action. Do not claim ownership of another agent's worktree without coordination.

## Privileged operations

Keep development, integration testing, security testing, and release environments separate. Use disposable environments for elevated tests. Coding agents get no production signing keys, broad host administration, or release publishing rights by default. User-facing prompt instructions do not enforce OS-level controls.
