# Handoff: M2 task preparation

Historical preparation record: the 2026-10-12 owner scope change supersedes the Stasha assignment and submodule. The [Rust transition handoff](M2-rust-transition.md) records current preparation. Checks below remain evidence only for their exact earlier candidates.

- **State:** DONE for branch/submodule/research/task/prompt preparation, independently APPROVED. Product M2 remains DRAFT; READY starting packages are M2-001 and M2-002.
- **Role / provider:** Codex coordinator/researcher/documentation implementer; intended product implementer Claude Code; independent preparation/architecture-security reviewer assigned separately.
- **Task authority:** Owner's 2026-10-12 request for M2 branch, full task, Claude prompt and Stasha core, followed by explicit submodule request; PROJECT.md and approved ADR 0001.
- **Checkout / branch:** /Users/jessicabruce/cham3, codex/m2-cryptographic-contract-core.
- **Base:** aa843f7ac19398f580f3577ba047300971ba6095; startup clean main, ahead of origin/main by the five prior approved ADR commits.
- **Exact reviewed preparation target:** c28499c88d2b2e0e73903e9ba6b2b899668d4973. Subsequent procedural evidence records completion; this is not a product implementation reference.
- **Stasha gitlink:** 5ada5de229df1255cd9b64ab0e28e94ae3d754b1 in toolchains/stasha; HTTPS upstream, detached pinned checkout, no floating branch update.

## Completed preparation

- Added Stasha as a pinned Git submodule.
- [Research](../research/M2_stasha_readiness.md) inspects pinned source/driver/ABI/runtime/qualification and native dependency candidates, separates upstream Ubuntu evidence from required Debian/M2 qualification, and records local compiler mismatch without touching the dirty local language checkout.
- [Task specification](../tasks/M2-cryptographic-contract-core.md) covers full milestone scope, Stasha/native ownership, G1–G6 design gates, eight ordered work packages, fifteen acceptance criteria, verification interfaces, exclusions and review/handoff requirements.
- [Claude prompt](../prompts/m2_claude_implementation.md) gives the execution assignment and preserves both the full M2 goal and preimplementation approval gates.
- [Planning evidence](../evidence/M2-planning/README.md) retains selected unmodified upstream receipts and API snapshots; no compiler artifact was executed.
- [Independent preparation review](../reviews/M2-task-preparation.md) approves the exact preparation target with no actionable findings; unresolved designs and product implementation remain outside that approval.

## Verification and limits

Environment: macOS arm64, unprivileged Git/source/API research operations.

| Check / command | Result | Evidence / limit |
|---|---|---|
| git status --short --branch; git rev-parse HEAD at startup | PASS | Clean main at the exact base. |
| git submodule add https://github.com/Kayyo321/stasha.git toolchains/stasha; git -C toolchains/stasha checkout --detach 5ada5de229df1255cd9b64ab0e28e94ae3d754b1 | PASS | Gitlink and .gitmodules added as requested; no nested builds/host installation. |
| gh api / source / selected qualification artifact inspection | PASS | Exact upstream run/receipts agree; research documents commands and primary links. Upstream runtime tests were not locally rerun. |
| git diff --check | PASS | Exit 0 on author preparation edits; formatting only. |
| python3 /tmp/cham3_m2_packet_check.py; git diff --check aa843f7ac19398f580f3577ba047300971ba6095 c28499c88d2b2e0e73903e9ba6b2b899668d4973 | PASS | Author and independent reviewer checked 22 relative-link targets, five JSON documents, receipt consistency, exact gitlink/URL and clean pinned checkout at the immutable candidate; formatting exit 0. |
| Independent preparation review | PASS | Read-only reviewer APPROVED c28499c88d2b2e0e73903e9ba6b2b899668d4973; source, ABI/driver, receipt comparison, GitHub run/job and Debian package availability checks are recorded in the review. |
| Pinned Debian compiler build/provider/M2 implementation tests | NOT_RUN | Future READY work and gated implementation; no new product code. |

## Remaining / ownership

- Claude starts M2-001/M2-002: disposable Debian toolchain/provider/harness qualification and concrete reviewed owner-approved G1–G6 design packages. Then admit dependent implementation; do not treat packet approval as architecture approval of unresolved choices.
- No merge, push, release or automatic message to Claude is authorized/performed by this preparation.
- An unrelated untracked docs/evidence/M1-bootable-desktop-distribution/postmerge_ci_50e0ee2.json appeared and disappeared externally during preparation. It was neither modified, removed nor staged by this task. The separate Documents/stasha dirty checkout was read-only.
