# Handoff: M2-003 — completion after independent re-review

- **State:** M2-003 is **DONE**. Codex's independent re-review approved revision 2. M2-004 is READY and starts next.
- **Role:** IMPLEMENTER. The author approved nothing. The approval is the independent reviewer's; the errata used are the owner's answers to explicit questions (2026-10-13, ADR 0004).
- **Model/provider:** Claude Code (Anthropic), cloud VM. Independent reviewer: Codex.
- **Working directory / branch:** `codex/m2-cryptographic-contract-core` (pushed fast-forward).
- **Task specification:** [M2 task](../tasks/M2-cryptographic-contract-core.md), M2-003.
- **Base commit:** ecc8213 (M2-003 start); revision-2 base f87111d021a743b6937dcc5a6c0efab4310f135b.
- **Approved candidate:** 8b0ccf37f2437cae0a9668e012ddf55e9952e2aa (revision 2), with docs-only ce83f28a19d1fd46e4ad1c4f61b4c2922ceed6e0. The commit adding this handoff records the approval and changes only `docs/`.

Use `PROJECT.md` and approved ADRs as product-authority references.

## Completed work

- **Delivered:** `cham3_contract_format` implements verification layers 1–5 of the [wire format](../specs/m2_wire_format_v1.md):
  - strict `cham3_dcbor_v1` decoding and encoding;
  - the COSE_Sign1 envelope with strict Ed25519 verification before the payload is parsed;
  - closed typed schemas with lossless re-encoding;
  - stateless commit semantics and the object hashes.

  Details: the [M2-003 handoff](M2-003-format-verifier.md) and the [revision-2 handoff](M2-003-rereview-r2.md).
- **Owner errata (2026-10-13):** E1 `list_length`; E2 any text `typ` other than the expected one is `wrong_object_type`; E3 strictly ascending lists use deterministic-encoding order. Recorded in [ADR 0004](../decisions/0004_m2_wire_schema_hash.md).
- **What a verified object means:** signature validity and format only. Key authority, admission, current authorization and the services are M2-004..M2-007.

## Verification

| Check | Environment/command | Result | Evidence/limits |
|---|---|---|---|
| Author amd64 qualification at 8b0ccf3 | Clean UID-1000 clone, pinned container, `qualify_m2_rust.sh` with `CHAM3_M2_REQUIRE_CLEAN=1` | PASS: unit 64+64; ECR 19/96/18 (22 scenarios / 346 steps); format differential 5 950 cases / 0 mismatches; harness 130 | [m2_003_amd64_vm_8b0ccf3](../evidence/M2-cryptographic-contract-core/m2_003_amd64_vm_8b0ccf3/) |
| GitHub Actions at 8b0ccf3 | m2 38327808145, m1 38327808083, m0 38327808082 | PASS | [m2_003_amd64_ci_8b0ccf3](../evidence/M2-cryptographic-contract-core/m2_003_amd64_ci_8b0ccf3/) |
| Reviewer's own amd64 qualification, static checks and tests, CI comparison | Codex: Debian 13.6 x86_64, UID 1000, Docker 28.4.0 | PASS (same counts; static 82; downloaded artifact digest verified) | Review record below |
| Reviewer's own scripts | `adversarial.py` 270 cases plus 45 combined-error cases; `allocations.py`; `defects.py` | PASS: 0 Rust/oracle and 0 amended-spec mismatches; no declared-capacity reservation; all eight seeded defects and the reviewer's own equality defect detected | Review record below |
| Codex Security plugin skill | `codex-security:security-diff-scan` | BLOCKED in the reviewer's environment (version, preflight resource and scan tools unavailable) | Required again at the final M2-008 review |
| Miri / sanitizers / fuzzing; services, IPC, vault, admission | — | NOT_RUN | Tooling not in the pinned toolchain; services belong to M2-004..M2-007 |

## Review

- **Reviewer and exact record:** Codex, `review/m2-003-r2-security` at 87e9b2a888671ab9b7cd86272d34e8a1c48cd5ac, `docs/reviews/M2-003-codex-security-rereview-r2.md`, SHA-256 `55605db4af3daf30703fc705e363b31055909ccc48aae75fdf16043e2d852518`. Verified: parent 9cf8825, adds only that file, not merged or edited.
- **Verdict:** **APPROVE** for M2-003. R01, R02, R03, R04 and I01 PASS; earlier PASS items intact; errata E1–E3 recorded and applied consistently; no new findings.
- **Earlier review:** `review/m2-003-security` at 1cb93c0 (CHANGES_REQUESTED), answered by the [finding resolutions](../reviews/M2-003-finding-resolutions.md).

## Remaining

- **M2-004 obligation (accepted by the review):** identityd must call `envelope::public_key_is_valid` on the enrolled `public_key` and the certified `subject_public_key` before it registers an enrollment or certificate. Bad subject points pass layers 1–5 by design.
- **Final M2 review:** the owner runs it with the **Codex Security plugin skill** ([task file](../tasks/M2-cryptographic-contract-core.md#final-m2-review-requirement-owner-instruction-2026-10-12)). The plugin was BLOCKED in both M2-003 reviews, so the final review must record whether it ran.
- **Uncommitted paths or dirty state:** none after this commit.
- **Next action and owner:** Claude Code implements M2-004 (identity and authority registry, encrypted vault, exact-proposal signing) on this branch, then submits the exact candidate for independent review.
