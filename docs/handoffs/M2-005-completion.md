# Handoff: M2-005 — completion after independent re-review

- **State:** M2-005 is **DONE**. Codex's bounded independent re-review approved revision 4 for the staged scope recorded in the task file. M2-004 is DONE. M2-006 continues (stage A in progress under the owner's standing instruction).
- **Role:** IMPLEMENTER. The author approved nothing. The approval is the independent reviewer's.
- **Model/provider:** Claude Code (Anthropic), cloud VM. Independent reviewer: Codex (owner's home computer, unrestricted).
- **Working directory / branch:** `codex/m2-cryptographic-contract-core` (pushed fast-forward).
- **Task specification:** [M2 task](../tasks/M2-cryptographic-contract-core.md), M2-005 (scope recorded before the work on 2026-10-13).
- **Base commit:** 6968bd6 (the M2-005 scope record).
- **Approved candidate:** 5860f276ab593da34771f704691ea35649324d6e (revision 4). The implementation is the sequence b06d7cb (revision 1) → 051b4c8 (revision 2) → be24152 (revision 3) → 5860f27 (revision 4), each with its docs-only evidence commit (the last: aeaee2dcf106a19c7607b0051b3ba4616e4b1816). The commit adding this handoff records the approval and changes only `docs/`.

Use `PROJECT.md` and approved ADRs as product-authority references.

## Completed work

- **Delivered:** `cham3_registryd` (per-user chained journal with durable appends, directory durability repeated at every start and reopen, a preallocated 1 MiB restrictive reserve whose durable size is tracked apart from its length, fail-closed replay through layers 1–7, layer-7 admission — placement, state, uniqueness including ended sessions, restore, subject binding, branch scope, network slot — `admit` under a per-user lock with the signing-key window, epoch and quotas, the identity store identityd runs over, `lineage_head`), `cham3_policyd` (`evaluate` / `network_mode` per ADR 0007 §1 and service operations) and `cham3_contractd` (`submit_signed`, `verify_object`), in safe Rust; the layer-7 differential against the published scenarios; the full-filesystem probe and its wrapper, which own only what they create. Details: the [M2-005 handoff](M2-005-admission-authorization.md) and the revision handoffs [r2](M2-005-rereview-r2.md), [r3](M2-005-rereview-r3.md), [r4](M2-005-rereview-r4.md).
- **Findings answered:** R01, R02, R03 of the first review ([resolutions](../reviews/M2-005-finding-resolutions.md)); R2-01, R2-02, R2-03 of the revision-2 re-review ([resolutions r3](../reviews/M2-005-finding-resolutions-r3.md)); R3-01 of the revision-3 re-review ([resolutions r4](../reviews/M2-005-finding-resolutions-r4.md)). Every reviewer test was reproduced failing at the old candidate and passing at the new one before each revision was offered.
- **Owner decisions used:** `duplicate_identifier` for ended sessions (R02, as the reviewer proposed and the owner's wire-format authority allowed); the ±300 s commit-timestamp code `timestamp_rule` (erratum E4, 2026-10-13) — recorded in docs only, implemented by M2-006 stage B.
- **What the approval means:** the staged scope recorded in the task file, at commit 5860f27. It is not milestone completion and not an approval of M2-006 (the reviewer says so explicitly); in-process composition only (real IPC and full-system enforcement are M2-007).

## Verification

| Check | Environment/command | Result | Evidence/limits |
|---|---|---|---|
| Author amd64 qualification at 5860f27 | Clean UID-1000 clone, pinned container, `qualify_m2_rust.sh` with `CHAM3_M2_REQUIRE_CLEAN=1` | PASS: unit 169+169; provider 7/7; ECR 19/96/18 (23 scenarios / 352 steps); differential 5 955 / 0; vault-object oracle 38; harness 147 (41 registry/policy, 21 identity/vault seeded defects, 12 reserve-probe tests); layer-7 differential 11 scenarios / 59 steps / 105 comparisons / 0 disagreements | [m2_005_amd64_vm_5860f27](../evidence/M2-cryptographic-contract-core/m2_005_amd64_vm_5860f27/) |
| Static checks; static tests | same clone and container | PASS; 82 OK | same directory, `qualification_console.txt` |
| Full-filesystem check and the five wrapper cases on a 2 MiB tmpfs | same clone, `--tmpfs /full-disk:rw,size=2m,…` | PASS; 5/5, no leftovers | same directory, `storage/` |
| Reviewer's wrapper scenario | author: clean clones at be24152 (both paths deleted) and 5860f27 (refused, both preserved) | PASS as recorded | same directory, `reviewer_reproductions/` |
| GitHub Actions at 5860f27 | m2 38444270032, m1 38444270097, m0 38444270083 | PASS, every step, including the vault protection positive path and the new wrapper preservation step | [m2_005_amd64_ci_5860f27](../evidence/M2-cryptographic-contract-core/m2_005_amd64_ci_5860f27/) |
| Reviewer's own qualification, static gates, CI comparison, artifact digest | Codex: Debian 13.7 x86_64, UID 1001, Docker 26.1.5, `nofile 1024:1024` | PASS (same counts; static 82; artifact 44 files / 103 622 bytes, `3b9cb3b9…0ba1` verified) | Review record below |
| Reviewer's own tests | the original wrapper scenario plus 25 extended preservation checks on the overlay and at the 2 MiB tmpfs mount root; the seeded wrapper defect discriminates | PASS 25/25 | Review record below |
| Earlier reviewer suites (ten `edge_`, nine `own_`, seven journal tests, `filesystem_full_reserve`, 34 direct-probe cases) | not re-run in revision 4 (no Rust changed); revision-3 evidence carried forward by the reviewer | PASS at be24152, intact | Revision-3 record `review/m2-005-r3-security` @ 4865d7b |
| Codex Security plugin | `codex-security:security-diff-scan` 0.1.32 on ad18d8b..5860f27 | Ran; zero findings | Review record below; required again at M2-008 |
| Vault protection positive path in this VM | — | BLOCKED (memlock cannot be raised); PASS in CI and in the reviewer's revision-3 run | — |
| Miri / sanitizers / fuzzing; power loss; real IPC, service processes, full-system enforcement | — | NOT_RUN | Tooling not in the pinned toolchain; M2-006 and M2-007 |

## Review

- **Reviewer and exact record:** Codex, `review/m2-005-r4-security` at 165e149d91e7ddaf167c3a37177275af70ae14e8, `docs/reviews/M2-005-codex-security-rereview-r4.md`, SHA-256 `87a61a90e36e768b4f782e28c2543b57c125afb6f656da3c5b77540449127817`. Verified by the author: parent 9cf8825 (`main`), adds only that file (553 lines), not merged or edited.
- **Verdict:** **APPROVE** for M2-005 at 5860f27 (recorded staged scope). R3-01 PASS, resolved; every earlier item intact; no new findings; no owner decision requested.
- **Reviewer's documented test-coverage limit:** the five harness wrapper cases run in per-case subdirectories of the disk, so they alone do not prove filler-only or dangling-filler refusal or mount-root cleanup; the reviewer's own matrix covered those at the actual mount root (25/25). Recorded as a coverage limit, not a defect. The tested contract assumes disposable mounts and trusted build paths, not a hostile same-account process replacing ancestors concurrently.
- **Earlier records:** `review/m2-005-security` at f9f3b82 (CHANGES_REQUESTED, R01–R03); `review/m2-005-r2-security` at d300b60 (CHANGES_REQUESTED, R2-01–R2-03); `review/m2-005-r3-security` at 4865d7b (CHANGES_REQUESTED, R3-01).

## Remaining

- **Carried obligations (recorded in the scope and accepted by the reviews):** M2-006 — the global anchor, restriction ledger and crash/rollback recovery (stage A); lifecycle-record writers, trusted time and `timestamp_rule` (stage B); ceremony and deletion (stage C); delegation admission (lifting the interim `delegation_rule` refusal), `prepare_proposal`, one-shot reservations, confirmation, restore (stage D); reserve status reporting (the lazy replenishment is not yet visible through a status operation). M2-007 — transport, binaries, the setgid client, systemd units and the cross-service serialization of restrictions with signatures.
- **Known limits:** in-process composition; after a restart `reserve_bytes` is 0 until the start-time sync or the next ordinary append succeeds; compressing filesystems are outside the reserve's guarantee; policy state is start-time configuration until stage A persists it in the anchor; the system clock is trusted until stage B; the harness wrapper coverage limit above.
- **Uncommitted paths or dirty state:** none after this commit on the branch. The author's M2-006 stage A work in progress is held locally (not on the branch) and is committed as stage A's candidate after this handoff.
- **Next action and owner:** Claude Code continues M2-006 stage A on top of 5860f27 (scope record in the task file); Codex reviews stage A when its handoff is offered. The owner's final M2 review requirement (Codex Security plugin at M2-008) stands.
