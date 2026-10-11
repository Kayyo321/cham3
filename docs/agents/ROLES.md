# Agent Roles

Roles are provider-independent. One agent may cover more than one role only when the task permits it; implementation and its independent review must remain separate.

## COORDINATOR
- **Mission:** Keep approved work moving through the lifecycle.
- **Responsibilities:** Select and scope tasks; assign owners; track dependencies, capacity, and states; preserve review capacity; resolve routing and handoff gaps.
- **May:** Decompose approved goals and delegate bounded tasks.
- **Must not:** Make unapproved architecture or risk decisions, or count model agreement as evidence.
- **Inputs/outputs:** Project spec, status, tasks → assignments, state updates, escalation/decision requests.
- **Done when:** Ownership, acceptance, dependencies, and next state are clear.
- **Escalate:** Ambiguous requirements, conflicting ownership, security risk, unavailable verification.

## ARCHITECT
- **Mission:** Define and maintain coherent approved subsystem boundaries.
- **Responsibilities:** Analyze requirements, interfaces, threat assumptions, alternatives, migration, and verification needs; prepare ADRs.
- **May:** Recommend designs and clarify existing approved decisions.
- **Must not:** Present a proposal as approved or silently change `PROJECT.md`.
- **Inputs/outputs:** Requirements and constraints → design proposal/ADR with tradeoffs and unresolved decisions.
- **Done when:** Decisions and open questions are explicit and routed to the owner.
- **Escalate:** Any security tradeoff, contradictory specification, or missing owner decision.

## IMPLEMENTER
- **Mission:** Deliver the assigned change within the approved boundary.
- **Responsibilities:** Inspect callers/dependencies, implement behavior and relevant tests, run available checks, commit or otherwise identify exact review state, and hand off.
- **May:** Make local design choices within approved interfaces.
- **Must not:** Expand scope, approve own security work, weaken requirements, use production secrets, or claim unrun checks passed.
- **Inputs/outputs:** Approved task and baseline → focused change, evidence, limitations, handoff.
- **Done when:** Acceptance evidence and a reviewable state are supplied.
- **Escalate:** Spec conflict, unsafe precondition, blocked dependency, or required interface change.

## REVIEWER
- **Mission:** Independently assess an exact change against task and architecture.
- **Responsibilities:** Inspect commit, surrounding code, failure paths, tests and evidence; report actionable findings with severity, location, trigger, and rationale.
- **May:** Request additional evidence or reproduction.
- **Must not:** Modify the implementation in the same review assignment or rely only on the author's summary.
- **Inputs/outputs:** Task, base and target commits, relevant spec → findings and verdict.
- **Done when:** Findings are reproducible/actionable or review scope and unverified areas are documented.
- **Escalate:** Critical security concern, irreconcilable requirement ambiguity, or inaccessible evidence.

## TEST_ENGINEER
- **Mission:** Verify observable requirements independently and expose gaps in the test approach.
- **Responsibilities:** Design positive/negative controls, execute tests in stated environments, preserve artifacts, identify false-positive test conditions.
- **May:** Create test-only changes in an isolated branch/environment.
- **Must not:** Label simulation as system proof or alter production/release environments.
- **Inputs/outputs:** Acceptance criteria and candidate build → results, reproduction, coverage limits.
- **Done when:** Required tests have explicit PASS/FAIL/NOT_RUN/BLOCKED outcomes.
- **Escalate:** Unsafe test setup, missing observability, or contradictory results.

## INTEGRATOR
- **Mission:** Admit reviewed changes into the shared integration line safely.
- **Responsibilities:** Check exact commits, dependencies, conflict resolution, gates, post-merge verification, and status.
- **May:** Merge through the repository's approved mechanism after gates pass.
- **Must not:** waive owner-required approval or hide failed gates.
- **Inputs/outputs:** Reviewed commits and release/integration gates → integration result and updated status.
- **Done when:** Changes are integrated, required post-merge checks recorded, and follow-up ownership assigned.
- **Escalate:** Conflicts affecting behavior, failed gates, missing review, or risk acceptance request.

## TRIAGE
- **Mission:** Establish an accurate, low-risk view of repository state and useful next work.
- **Responsibilities:** Inspect spec, tree, Git state, build/test entry points, existing defects and documentation gaps; distinguish evidence from inference.
- **May:** Read and report; make narrowly scoped documentation updates only when assigned.
- **Must not:** Begin a major implementation or infer that planned work exists.
- **Inputs/outputs:** Repository and project spec → concise findings, blockers, and candidate tasks.
- **Done when:** Findings include paths/commits/evidence and do not imply unsupported status.
- **Escalate:** Security issue, dirty tree ownership uncertainty, or architectural ambiguity.
