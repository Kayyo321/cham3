# Handoff: M2-003 — strict schema and COSE_Sign1 verifier (layers 1–5)

- **State:** M2-003 is **IMPLEMENTED → IN_REVIEW**. The author approves nothing; independent review of the exact candidate is required before M2-003 is DONE.
- **Role:** IMPLEMENTER.
- **Model/provider:** Claude Code (Anthropic), cloud VM. Requested reviewer: Codex (independent), on the owner's desktop.
- **Working directory / branch:** `codex/m2-cryptographic-contract-core` (pushed fast-forward).
- **Task specification:** [M2 task](../tasks/M2-cryptographic-contract-core.md), M2-003 (gates G1, G2 and the relevant G3 parts, all owner-approved 2026-10-12); acceptance A01–A03 for this package.
- **Authority:** [wire format v1](../specs/m2_wire_format_v1.md), [CDDL](../specs/m2_ecr_v1.cddl), [ADR 0004](../decisions/0004_m2_wire_schema_hash.md), [ADR 0003](../decisions/0003_m2_rust_toolchain_dependencies.md).
- **Base commit:** ecc821342a50668742d9f23f5065eeffe87a6a10 (owner decisions recorded; docs only since 87c9cbc).
- **Candidate:** **4841039d772ca923e09a55f1505f119317e73a91**.
- **Docs-only follow-up:** the commit adding this handoff, the evidence, the task progress and STATUS.

## Completed work

`core/contract_format` (crate `cham3_contract_format`, `#![forbid(unsafe_code)]`, workspace lints) implements verification layers 1–5 of wire format §1:

| Module | Content |
|---|---|
| `dcbor.rs` | `cham3_dcbor_v1` strict decoder: the §2.1 item set, shortest-form arguments, reserved/indefinite rejection, tag 18 only at the root of a signed object, the §2.3 limits checked **before** any allocation, map keys strictly ascending by encoding with one key type, trailing bytes; the first error in byte order (§2.4). Deterministic encoder. |
| `envelope.rs` | `#6.18([protected, {}, payload, signature])`, exact protected header `{1: −19, 4: kid, 16: typ}`, `unsupported_algorithm` / `wrong_object_type` / `protected_header_invalid` rules, empty unprotected map, 64-byte signature. The `kid` is resolved through a caller-supplied `key_resolver_t`, and the signature over the Sig_structure (kind `external_aad`) is checked with ed25519-dalek `verify_strict` after a canonical, non-small-order public-key check, **before** the payload is parsed. Also `protected_header_bytes`, `to_be_signed`, `assemble_envelope` (byte assembly only; nothing here signs). |
| `schema.rs` | Closed typed schemas for commits, identity enrollments, key certificates and key invalidations. Fields are walked in encoded-key order, and each is checked by type, then format and limits, then ordering. Cross-field identity rules: `key_id_mismatch`, `subject_actor_mismatch`, `lineage_mismatch`. Parsing is lossless: `to_value` re-encodes to the signed bytes. |
| `text.rs` | ASCII-exact `utc_time` with a Gregorian date check, label/operation/package patterns, and restricted resource and message text (§3). |
| `intrinsic.rs` | Layer 5: commit-type table, lifetimes, delegation, network grants, identity-transition rule, delta/result consistency, genesis base state. |
| `hash.rs` | `domain_hash`, `key_id`, `lineage_id`, `state_hash`, `object_id` / `commit_id`. |
| `lib.rs` | `verify_object(bytes, expected_kind, resolver) -> verified_object_t`. It reports exactly one `layer/code`. The documentation states that a verified object is signature validity only: not key authority (layer 6), admission (7), current authorization (8) or enforcement. |
| `src/bin/cham3_format_probe.rs` | Line-protocol probe used only by the harness differential check. |

Tests and harness:

- `tests/ecr_vectors.rs` (9 tests) runs every published vector:
  - 18 valid objects verify and reproduce their payload bytes, object id and deterministic signature, re-signed with ed25519-dalek independently of the oracle's libsodium;
  - 90 invalid objects give exactly the published `layer/code`;
  - 18 admission and 129 journal objects pass layers 1–5, or give their published local code;
  - every kind is rejected as each other kind;
  - every single-byte change (two masks) and every truncation of the valid objects is rejected (stride 97 on the 61 440-byte object);
  - 20 000 seeded random inputs are rejected without panicking.
- `tests/schema_mutations.rs` (5 tests) applies field-level edits and re-signs them, so each edit is authenticated:
  - every removed required field gives `missing_field`;
  - an unknown field gives `unknown_field` wherever it sorts;
  - every field retyped gives `wrong_type`;
  - integer, time, text, list, identity and genesis boundaries give their specified codes.
- Unit tests in `dcbor.rs` and `text.rs` (10 new).
- `tests/m2/oracle/format_differential.py` builds a seeded hostile corpus from the vectors and computes every expected outcome with the independent oracle. The corpus contains:
  - envelope byte edits;
  - re-signed payload byte edits;
  - re-signed structural edits: fields deleted, added or retyped; boundary integers; hostile text; reordered, duplicated or oversized lists; swapped enumerations;
  - re-signed protected-header edits;
  - every vector object.
- `m2_harness.py format-differential` feeds the corpus to the probe and requires every outcome to be identical, plus a coverage floor for every local layer and at least 40 distinct outcomes. It runs as the new unit-suite steps `build_format_probe` and `format_differential`.
- `tests/m2/harness/test_m2_format_verifier.py` (8 tests) shows that the check is sensitive:
  - one wrong answer is detected;
  - a missing result line is detected;
  - a too-thin corpus is detected;
  - the corpus is deterministic per seed;
  - there is an oracle-pattern regression test;
  - **5 seeded defects** in a disposable copy of the Rust verifier are each detected, and the unmodified copy passes. The defects are: non-minimal heads accepted, duplicate map keys accepted, commit `key_id` not bound to `kid`, a second network grant accepted, and bidi controls accepted.

### Oracle defects found and fixed (test code only)

The differential check disagreed with the oracle on 4 of 5 642 cases. The Rust verifier followed the specification; the oracle did not:

- **O-1:** `re.match` with `$` accepted a trailing `"\n"` in `app_id`/`ascii_label`/`operation`/`utc_time`.
- **O-2:** `\d` accepted non-ASCII digits in `utc_time` (wire format §3.4 requires ASCII).

Two of the four mismatches were objects the old oracle accepted and the specification forbids. The oracle now uses `fullmatch` with `re.ASCII`. The published vectors are byte-identical; `ecr_oracle.py check` still passes. The reproduction is in the evidence (`oracle_before/`).

### Points for the reviewer and the owner

1. **Erratum E1 (owner decision requested, not edited):** wire format §6.1 and §7 name the list-length code `list_too_long`. The approved vectors and the oracle use **`list_length`**, for a list that is too short (for example, empty `operations`) as well as too long. The implementation follows the approved vectors, as wire format §9 requires ("must reproduce every byte and error code"), and the code is documented in `error.rs`. Recommendation: correct the two spec sentences to `list_length`. This is an editorial change to an approved text, so it needs the owner's approval.
2. **Spec readings shared by oracle and verifier** that the reviewer should judge against the text, not against each other:
   - Text lists are "strictly ascending" by deterministic encoding, so shorter strings come first, as for map keys.
   - The decode order for a string head is `text_too_long`, then `input_too_large`, then `truncated`.
   - Leading/trailing "whitespace" in `message` means Unicode White_Space; the only differences from Python's `str.strip` are forbidden controls.
   - A non-integer `alg` is `protected_header_invalid`; any other integer is `unsupported_algorithm`.
   - A `typ` that names another known kind is `wrong_object_type`.
3. **Public-key validation of enrolled and certified keys** (wire format §4.3) is exposed as `envelope::public_key_is_valid`. It is applied to every resolved key before verification, but it is **not** a schema check of `public_key`/`subject_public_key`: that remains identityd's enrollment and certification check (M2-004), as the oracle also models.
4. Cargo.lock gains only the dependency edges of `cham3_contract_format` on the already pinned `ed25519-dalek` and `sha2`. No package or version changed, the dependency set and source manifest are unchanged, and proptest/ciborium remain unused (deterministic mutators and the oracle differential instead).

## Verification

| Check | Environment / command | Result | Evidence / limits |
|---|---|---|---|
| Full M2 qualification at 4841039 | Clean UID-1000 clone, pinned container, `qualify_m2_rust.sh`, `CHAM3_M2_REQUIRE_CLEAN=1` | **PASS**: unit 60 + 60 cargo tests; provider oracle 7/7; ECR oracle 18/90/18 with 22 scenarios (346 steps); **format differential 5 642 cases, 0 mismatches, 56 distinct outcomes**; harness 128 | [m2_003_amd64_vm_4841039](../evidence/M2-cryptographic-contract-core/m2_003_amd64_vm_4841039/) |
| Static checks; static tests | same container settings | PASS; PASS (82) | same directory |
| Unchanged inputs | `git diff --exit-code ecc8213 4841039 -- Cargo.toml build .github PROJECT.md docs tests/m2/vectors` + other crates; dependency inventory diff against 28a8595 | exit 0; identical; the Cargo.lock delta is edges only | `unchanged_inputs.txt` |
| Author exploration | 10 further seeds (100–109), 300 cases per object | PASS, 0 mismatches each | `differential_seeds_100_109.txt` |
| GitHub Actions at 4841039 | m2 [38319998393](https://github.com/Kayyo321/cham3/actions/runs/38319998393), m1 [38319998397](https://github.com/Kayyo321/cham3/actions/runs/38319998397), m0 [38319998384](https://github.com/Kayyo321/cham3/actions/runs/38319998384) | **PASS** (all jobs and steps success) | [m2_003_amd64_ci_4841039](../evidence/M2-cryptographic-contract-core/m2_003_amd64_ci_4841039/) |
| CI artifact ZIP bytes | — | NOT_RUN (author `gh` unauthenticated); metadata 89 588 bytes, `sha256:17bd8023…acf9` | Owner/reviewer can download |
| Oracle-defect reproduction | Pre-fix oracle (87c9cbc) vs candidate probe, default seed and corpus | 4 mismatches, as described above; 0 with the fixed oracle | `m2_003_amd64_vm_4841039/oracle_before/` |
| Fuzzing (libFuzzer/cargo-fuzz), Miri, sanitizers | — | NOT_RUN (not available in the pinned toolchain) | Seeded mutation and differential tests are not a fuzzing PASS |
| Key authority, admission, current authorization, real services | — | NOT_RUN | M2-004..M2-007 |

## Review

- Reviewer and exact commit: requested — Codex, candidate 4841039 and the docs-only commit after it.
- Findings and disposition: none yet.

## Remaining

- Known issues/risks: Erratum E1. The verifier is a library; nothing calls it in a service yet.
- Uncommitted paths or dirty state: none after the evidence commit.
- Required next action and owner: Codex reviews M2-003; the owner decides E1. After an APPROVE and any repairs, M2-003 is DONE and M2-004 (identity/vault) starts.
- **Reminder for the end of M2:** the final review uses the Codex Security plugin skill (task file).

## Review prompt for Codex (owner's desktop)

~~~text
You are the independent REVIEWER (Codex) for cham3 M2-003 (strict schema and COSE_Sign1 verifier, verification
layers 1-5) in Kayyo321/cham3, on the owner's desktop. You did not author this work. Read AGENTS.md, PROJECT.md
sections 4-5, docs/specs/m2_wire_format_v1.md, docs/specs/m2_ecr_v1.cddl, ADR 0003 and ADR 0004 (both APPROVED),
the M2 task (M2-003, A01-A03) and docs/handoffs/M2-003-format-verifier.md. Treat author claims and evidence as
claims to verify. Judge the code against the approved specification text, not only against the author's oracle.

Targets: candidate 4841039d772ca923e09a55f1505f119317e73a91 (range ecc821342a50668742d9f23f5065eeffe87a6a10..4841039)
and the docs-only commit after it on origin/codex/m2-cryptographic-contract-core; confirm that commit changes only
docs/. Do not merge main or review branches into the implementation branch; do not edit implementation files.

1. Scope and boundaries: safe Rust only (#![forbid(unsafe_code)], no new unsafe anywhere); no new dependency or
   version (the Cargo.lock delta is only cham3_contract_format's edges to the pinned ed25519-dalek and sha2; confirm
   the source manifest and dependency inventory are unchanged); AGENTS.md naming; no secret or input bytes logged;
   nothing signs in production code; verified_object_t is documented and used as signature validity only, never as
   key authority, admission or current authorization.
2. Specification conformance (wire format sections 1-7, CDDL): decoder item set, shortest-form rule, reserved and
   indefinite rejection, tag-18-only-at-root, every 2.3 limit checked before allocation, map-key order and type,
   trailing bytes and the 2.4 first-error order; the exact protected header, algorithm, typ/external_aad domain
   separation, empty unprotected map, signature length; strict Ed25519 (S < L, non-small-order R and A, canonical A)
   applied to the authenticated payload before it is parsed; closed schemas, field order, formats and limits;
   cross-field rules; layer-5 rules. Judge the shared readings listed in the handoff ("Points for the reviewer").
3. Evidence: in a clean clone at 4841039 owned by an unprivileged UID, build the pinned image and run the
   qualification (docker run --user "$(id -u):$(id -g)" ... bash build/scripts/qualify_m2_rust.sh with
   CHAM3_M2_REQUIRE_CLEAN=1; no --privileged, no added capabilities, no --record) and confirm: unit 60+60 cargo tests,
   provider oracle 7/7, ECR oracle 18/90/18 with 22 scenarios / 346 steps, format_differential 5642 cases with 0
   mismatches, harness 128; then build/scripts/run_static_checks.sh and sh tests/run_tests.sh static. Compare GitHub
   Actions m2_checks 38319998393, m1_checks 38319998397 and m0_checks 38319998384 (you may download the m2 artifact
   with gh run download 38319998393 -n m2_qualification_amd64 and compare summary and digest).
4. Adversarial testing of your own: write hostile inputs the author's corpus may miss (for example nested
   boundaries at depth 8/9, 256/257 items, 1024/1025-byte text, 61440/61441-byte objects, non-canonical and
   small-order signature components, wrong-kind objects, unknown keys of every major type, edge times, list order
   ties) and run them through the Rust probe (src/bin/cham3_format_probe.rs) and the oracle; report any
   disagreement with the specification. Confirm the 5 seeded defects in tests/m2/harness/test_m2_format_verifier.py
   are each detected, and try at least one defect of your own.
5. Oracle change: confirm the ecr_oracle.py fix (fullmatch, re.ASCII) is correct, that vectors.json is unchanged
   and that the pre-fix reproduction in docs/evidence/M2-cryptographic-contract-core/m2_003_amd64_vm_4841039/
   oracle_before/ holds.
6. Erratum E1: give your judgement on list_length (vectors, oracle, implementation) versus list_too_long (wire
   format 6.1 and 7). Make no owner decision.

Record every check as PASS/FAIL/NOT_RUN/BLOCKED with exact commands, environment and evidence. Return APPROVE or
CHANGES_REQUESTED for M2-003; make no owner decisions. Publish only your review record on a new review branch
(review/m2-003-security), and end your reply with this block filled in:

=== CHAM3 M2-003 VERDICT ===
reviewer: Codex
environment: <machine/OS/arch; UID; Docker version; network notes>
review_record: <branch> @ <commit> : <path> (sha256 <digest>)
candidate_reviewed: 4841039d772ca923e09a55f1505f119317e73a91
docs_commits_reviewed: <shas> (docs-only: yes/no)
verdict: M2-003: <APPROVE/CHANGES_REQUESTED>
scope_and_boundaries: <PASS/FAIL> (<unsafe, dependencies, lock delta, naming, logging, signing, authority separation>)
spec_conformance: <PASS/FAIL> (<decoder / envelope+signature / schema / intrinsic; shared readings judged>)
qualification_amd64: <PASS/FAIL/BLOCKED> (<counts, format_differential cases/mismatches>)
static_checks: <..>   static_tests: <..>
own_adversarial_tests: <PASS/FAIL> (<cases, disagreements>)
seeded_defects: <all detected yes/no; own defect: ..>
oracle_change: <PASS/FAIL>
erratum_E1: <judgement; no decision>
ci_comparison: <..>
new_findings:
- id: <..> | severity: <..> | location: <file:line> | summary: <..> | required: <..>
notes_for_owner: <..>
=== END VERDICT ===
~~~
