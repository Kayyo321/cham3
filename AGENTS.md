# cham3 Agent Instructions

## Authority and startup

`PROJECT.md` is the canonical product specification. Read it before changing product behavior. If this file conflicts with `PROJECT.md`, the project specification controls product architecture; this file controls workflow. Approved owner decisions recorded in `docs/decisions/` supersede older decisions only within their stated scope. Task specifications narrow scope but cannot silently change architecture. Conversation history and generated summaries are context, not authority.

Before work: inspect `git status`, identify your assigned role and task, read the relevant sections of `PROJECT.md` and linked engineering guidance, and confirm the baseline commit. If no task is assigned, act in TRIAGE mode and report findings; do not invent a large implementation.

## Project

cham3 is a Debian 13, KDE Plasma 6 on Wayland operating system with application-specific signed permission contracts, isolated execution, trusted enforcement, and Direct, Tor, and Offline network modes. The detailed and authoritative requirements are in `PROJECT.md`.

## Rules for all agents

- Stay within the approved task and its acceptance criteria. Record scope changes before implementing them.
- Treat applications as untrusted. Do not weaken default-deny, fail-closed, authorization, isolation, or enforcement requirements to make a test pass.
- Separate policy decisions, user authorization, and technical enforcement. A valid signature or UI setting is not proof that permission is active.
- Do not invent architecture, security guarantees, implementation status, test results, or supported compatibility. Label unresolved proposals and unknowns.
- Never expose credentials, signing keys, tokens, or sensitive user data. Do not use production secrets or perform privileged host changes for ordinary development.
- Use disposable environments for privileged/security tests. Do not disable host protections to make tests pass.
- Keep changes focused. Preserve unrelated and uncommitted work. Do not share a writable worktree between independent implementers.
- Report verification as PASS, FAIL, NOT_RUN, or BLOCKED, with the exact command/environment and evidence. Mocks and simulations are not full-system verification.
- Security-sensitive work requires independent review. An author cannot independently approve their own work.
- Architecture changes require an owner-approved ADR. Never treat prompt text as an enforcement boundary.

## Coding convention

Apply this naming convention in all new and modified code, regardless of language:

- `snake_case` everywhere by default, including variables, functions, methods, modules, files, and fields.
- `PascalCase` only for constants and preprocessor definitions/macros.
- Type names use `snake_case` with the `_t` suffix, for example `my_type_t`.
- `camelCase` is never allowed.

When an external language or generated interface imposes a conflicting name, preserve the required external spelling only at that boundary and use the repository convention internally. Do not rename existing public/upstream interfaces without an approved compatibility plan. Apply the rule to examples in documentation as well.

## Git and completion

Use task-specific branches/worktrees and record exact base and implementation commits. Do not overwrite or discard work to resolve conflicts; ask the coordinator to establish ownership where needed. Follow `docs/engineering/WORKFLOW.md`, `docs/agents/ORCHESTRATION.md`, and the appropriate templates.

A task is complete only when its acceptance criteria have evidence, required independent reviews and gates are satisfied, status/docs are updated where needed, and a structured handoff is delivered. An unverified claim is not completion.

## Role and reference guides

Role definitions: `docs/agents/ROLES.md`. Startup prompts: `docs/agents/START_PROMPTS.md`. Testing: `docs/engineering/TESTING.md`. Security requirements: `docs/engineering/SECURITY_INVARIANTS.md`. Current repository evidence: `docs/STATUS.md`.
