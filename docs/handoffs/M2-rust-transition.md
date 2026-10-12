# Handoff: M2 memory-safe implementation assignment

- **State:** IN_REVIEW for task/prompt/research revision and submodule removal. Full M2 remains DRAFT; M2-001/M2-002 are READY qualification/design work.
- **Role/provider:** Codex coordinator/documentation implementer; Claude Code is the intended M2 implementation author; independent reviewer assigned separately.
- **Owner scope:** On 2026-10-12 the owner requested: “Change the implementation so we use a memory safe compiled language instead, remove stasha as a submodule for now.” Rust is selected as the implementation language under that instruction; no claim that the owner named Rust or approved detailed G1–G6 designs.
- **Authority:** PROJECT.md and approved ADR 0001; language selection changes implementation scope without changing product architecture, required primitives or security boundaries.
- **Checkout/branch:** /Users/jessicabruce/cham3, codex/m2-cryptographic-contract-core.
- **Exact baseline:** c84de12d280454cba680d731f907355134a91e30; clean checkout, including prior preparation and subsequent unrelated main/branding updates.
- **Exact review target:** Recorded after the immutable candidate is committed; no product implementation commit exists.

## Changes and preserved scope

- [Full M2 task](../tasks/M2-cryptographic-contract-core.md) and [Claude prompt](../prompts/m2_claude_implementation.md) now require substantive safe Rust domain crates, isolated reviewed unsafe/FFI adapters, fixed Cargo/toolchain inputs and Rust-specific harness evidence.
- [Rust research](../research/M2_rust_readiness.md) records primary-source build/test/safety/package inputs with qualification limits.
- Removed toolchains/stasha and its .gitmodules registration using clean-submodule deinitialization and Git removal. Repository-local cached Git objects may remain recoverable; they are not an active dependency. No separate Stasha checkout was modified.
- Earlier Stasha research, receipts and preparation reviews remain historical evidence, with explicit supersession notices. No old PASS is relabeled as Rust verification.
- ECR remains a required M2 pillar; all eight packages, fifteen criteria, mandatory-denial behavior and independently reviewed owner-approved detailed gates remain in scope.

## Verification

Environment: macOS arm64, unprivileged Git/documentation operations.

| Command/check | Result | Evidence/limits |
|---|---|---|
| git status --short --branch; git rev-parse HEAD | PASS | Clean at the exact baseline before edits. |
| git -C toolchains/stasha status --short; git submodule deinit -- toolchains/stasha; git rm -- toolchains/stasha; git rm --cached -- .gitmodules; remove verified empty .gitmodules | PASS | Submodule was clean; working dependency and registration removed without forced deletion. |
| Primary Rust/Cargo/Debian documentation inspection | PASS | Research links record source claims and initial package candidate, not deployment proof. |
| Formatting, local-link and active-assignment consistency checks | NOT_RUN | Record against immutable candidate before DONE. |
| Independent exact-candidate review | NOT_RUN | Required before preparation completion. |
| Debian Rust/compiler/provider/product verification | NOT_RUN | Future READY work and gated implementation; no core code exists to migrate. |

## Next action

After preparation review, Claude follows the revised prompt, starts M2-001/M2-002, freezes and qualifies actual Rust inputs in disposable Debian, then obtains detailed design approvals before dependent implementation. Historical task preparation was independently approved at its own exact candidate; that approval cannot approve this revision. No merge, push or host deployment performed by this change.
