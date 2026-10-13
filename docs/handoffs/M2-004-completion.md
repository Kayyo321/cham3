# Handoff: M2-004 — completion after independent re-review

- **State:** M2-004 is **DONE**. Codex's independent re-review approved revision 3 for its recorded staged scope. M2-005 continues (revision 3 in review); M2-006 follows under the owner's standing instruction.
- **Role:** IMPLEMENTER. The author approved nothing. The approval is the independent reviewer's.
- **Model/provider:** Claude Code (Anthropic), cloud VM. Independent reviewer: Codex (home computer, unrestricted).
- **Working directory / branch:** `codex/m2-cryptographic-contract-core` (pushed fast-forward).
- **Task specification:** [M2 task](../tasks/M2-cryptographic-contract-core.md), M2-004.
- **Base commit:** 20c849b (M2-004 start); revision-3 base 24a35c1914d787b936d14b37b500226dde51722f.
- **Approved candidate:** 051b4c8aacdeecfabb23625f93dd16615aec39a0 (revision 3; the M2-004 change is in 4f7080b091adef38d3c8b1a36fb975db53ba52b7), with the docs-only f575ac3, 4f0fe93 and 959524c. The commit adding this handoff records the approval and changes only `docs/`.

Use `PROJECT.md` and approved ADRs as product-authority references.

## Completed work

- **Delivered:** `cham3_identityd` (caller roles from peer credentials, the layer-6 key-authority index over the journal with scoped `resolve_key`, registration with the M2-003 key-validation obligation, restrictive invalidation) and `cham3_vaultd` (encrypted key objects with Argon2id and XChaCha20-Poly1305 in secret regions, the restricted store, exact-proposal approval and signing, attempt limiting, restartable creation/rotation/invalidation/recovery keys, process protection), in safe Rust. Details: the [M2-004 handoff](M2-004-identity-vault.md), the [revision-2 handoff](M2-004-rereview-r2.md) and the [revision-3 handoff](M2-004-rereview-r3.md).
- **Findings answered:** R01–R05 and I01 of the first review ([resolutions](../reviews/M2-004-finding-resolutions.md)); M2-004-R2-01 of the revision-2 re-review ([resolutions r2](../reviews/M2-004-finding-resolutions-r2.md)).
- **The two revision-2 records, reconciled by the reviewer:** both reviewed the same candidate ca40c62. The later APPROVE in the combined record (f9f3b82) neither mentioned nor disposed of the quota finding of the earlier record (fd6de58), so CHANGES_REQUESTED stood for revision 2; the author's refusal to infer approval was correct. The re-review closes M2-004-R2-01 at 051b4c8 and approves revision 3. The owner question "which revision-2 verdict governs" is therefore answered by the reviewer's reconciliation and needs no separate decision.
- **What the approval means:** the staged scope recorded in the task file. It is not a retroactive approval of revision 2 and not milestone completion; in-process composition only (real IPC and full-system enforcement are M2-007).

## Verification

| Check | Environment/command | Result | Evidence/limits |
|---|---|---|---|
| Author amd64 qualification at 051b4c8 | Clean UID-1000 clone, pinned container, `qualify_m2_rust.sh` with `CHAM3_M2_REQUIRE_CLEAN=1` | PASS: unit 166+166; provider 7/7; ECR 19/96/18 (23 scenarios / 352 steps); differential 5 955 / 0; vault-object oracle 38; harness 137 (21 identity/vault seeded defects) | [m2_005_amd64_vm_051b4c8](../evidence/M2-cryptographic-contract-core/m2_005_amd64_vm_051b4c8/) |
| Reviewer's twelve revision-2 tests and the original seventeen | author: clean clones at ca40c62 (10/2) and 051b4c8 (12/0; 16/1 with the documented I01 assertion) | PASS as recorded | [m2_004_amd64_vm_051b4c8](../evidence/M2-cryptographic-contract-core/m2_004_amd64_vm_051b4c8/) |
| GitHub Actions at 051b4c8 | m2 38422383429, m1 38422383455, m0 38422383430 | PASS, including the vault protection positive path | [m2_005_amd64_ci_051b4c8](../evidence/M2-cryptographic-contract-core/m2_005_amd64_ci_051b4c8/) |
| Reviewer's own qualification, protection positive path, static gates, CI comparison, artifact digest | Codex: Debian 13.7 x86_64, UID 1001, Docker 26.1.5 | PASS (same counts; static 82; artifact `f05dedc6…a926` verified) | Review record below |
| Reviewer's own tests | the twelve (12/0 debug and release), the original seventeen (16/1, I01 as documented), approval-transition and cancellation controls | PASS; no marker eviction, no id revival, no stranded workflow | Review record below |
| Codex Security plugin | `codex-security:security-diff-scan` 0.1.32 on bb3f1ba..ca40c62 (static) and 24a35c1..051b4c8 | Ran; zero exploit-oriented findings; the historical quota defect confirmed and closed by the tested fix | Review record below; required again at M2-008 |
| Miri / sanitizers / fuzzing; real IPC, service processes, full-system enforcement | — | NOT_RUN | Tooling not in the pinned toolchain; M2-007 and the integration packages |

## Review

- **Reviewer and exact record:** Codex, `review/m2-005-r2-security` at d300b609ccde3eaf1fe8e909f99e78707c4b6c5a, `docs/reviews/M2-005-codex-security-rereview-r2.md`, SHA-256 `28b4623121c65093d1fdc98963262b30e68da251417a868d6cea6c0f82b17435`. Verified: parent 9cf8825 (`main`), adds only that file, not merged or edited.
- **Verdict:** **APPROVE** for M2-004 revision 3 (recorded staged scope). M2-004-R2-01 PASS; R01–R05, the I01 interpretation and the carried obligations intact; no new M2-004 finding.
- **Earlier records:** `review/m2-004-security` at f2d82b7 (CHANGES_REQUESTED, R01–R05, I01); `review/m2-004-r2-security` at fd6de58 (CHANGES_REQUESTED, M2-004-R2-01); the M2-004 part of `review/m2-005-security` at f9f3b82 (APPROVE, superseded by the reconciliation above).

## Remaining

- **Carried obligations (accepted by the reviews):** the M2-006 restriction ledger (a not-yet-durable denial across an identityd restart); cross-service serialization of restrictions with signatures (M2-006/M2-007); `replace_identity` and vault-side deletion cleanup (M2-006); transport, binaries, the setgid client and systemd units (M2-007). Duplicate request ids at admission are enforced by M2-005.
- **Known limits:** completing an existing signed, durable recovery bundle does not re-authenticate the passphrase (the documented I01 behaviour; enrollment retry does); a pending denial lives in identityd's memory until the ledger exists; proposal ids are kept in memory; passphrase knowledge is not human presence.
- **Uncommitted paths or dirty state:** none after this commit.
- **Next action and owner:** Claude Code answers the M2-005 revision-2 findings (revision 3, [handoff](M2-005-rereview-r3.md)); M2-006 starts with its scope record under the owner's standing instruction.
