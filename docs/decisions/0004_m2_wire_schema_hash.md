# ADR 0004: M2 wire schema, deterministic encoding and hashes (G2)

- **Status:** PROPOSED — awaiting independent review and owner approval. Nothing here is approved.
- **Date / decision owner:** Proposed 2026-10-12 by Claude Code (M2 implementer) / cham3 project owner.
- **Related task/spec sections:** [M2 task](../tasks/M2-cryptographic-contract-core.md) G2, M2-003, A03, A07; PROJECT.md §§4.2, 5.1–5.7, 6.1, 15.1; [ADR 0001](0001_eternal_contract_repository.md) canonical commit; SEC-03, SEC-04, SEC-15.

`PROJECT.md` is the canonical product specification. This ADR records a scoped decision or proposal and does not silently replace that document.

## Problem and context

ADR 0001 approved canonical signed ECR transactions in a fixed versioned deterministic CBOR profile inside COSE_Sign1 with Ed25519, a SHA-256 commit identifier, and metadata binding the exact state, delta, identity, lifetime, policy epoch and parents. It left the exact fields, encoding, header/domain separation, hash preimages, parent order, genesis/merge/base-state semantics, evolution and vectors to this gate.

## Decision (proposed)

Adopt the normative [wire format v1](../specs/m2_wire_format_v1.md) and [CDDL schema](../specs/m2_ecr_v1.cddl), with [published vectors](../../tests/m2/vectors/ecr_v1/vectors_readme.md). Summary of the choices:

1. **Encoding — `cham3_dcbor_v1`:** RFC 8949 §4.2.1 core deterministic encoding restricted to unsigned/negative integers (negative ≥ −2^63), byte and text strings, arrays, maps, `false`/`true`, and tag 18 only at the envelope root. Floats, null/undefined, other simple values, other tags, indefinite lengths and non-minimal arguments are rejected; map keys strictly ascending by encoded bytes (duplicates rejected) and of one type per map; no trailing bytes; limits 64 KiB input, depth 8, 256 items, 1 KiB text. Closed schemas: unknown fields rejected, optional fields omitted (never null).
2. **Envelope — `cham3_cose_sign1_v1`:** tagged COSE_Sign1 with attached payload; protected header exactly `{1: -19, 4: kid, 16: typ}`; unprotected header empty; 64-byte signature. `alg` is RFC 9864 fully-specified `Ed25519` (−19); polymorphic `EdDSA` (−8) is rejected. `typ` (RFC 9596) names the object kind and version.
3. **Domain separation:** every object kind has its own `typ` (signed) and its own external_aad (`cham3/ecr-commit/v1`, `cham3/identity-enrollment/v1`, `cham3/key-certificate/v1`, `cham3/key-invalidation/v1`). Ed25519 signs the RFC 9052 Sig_structure; verification is strict (canonical `S`, no small-order points).
4. **Layer order:** envelope decode → envelope → signature (kid resolved through a caller-supplied resolver) → payload decode + schema → intrinsic semantics → key authority → admission → current authorization. Payload bytes are authenticated before they are parsed. Exactly one stable error code is reported: the first failure in layer order.
5. **Hashes:** `commit_id = SHA-256(envelope)` (ADR 0001's identifier, unchanged); all derived identifiers use `domain_hash(tag, data) = SHA-256(tag || 0x00 || data)`: `key_id` (`cham3/key-id/ed25519/v1` over the public key), `lineage_id` (`cham3/lineage/v1` over `[owner_user_id, app_id, installation_id]`), `state_hash` (`cham3/state/v1` over the deterministic state).
6. **Commit payload:** schema version, commit type, UTC timestamp (`YYYY-MM-DDTHH:MM:SSZ`), actor (uid and identity `user_id`), `key_id` (= kid), authority class, authorization request and session references, policy binding (epoch, version, security profile, mandatory constraints), verified subject (owner, `app_id`, installation, package identity, executable SHA-256, optional runtime scope), trusted `initiating_program` (set by contractd from the authenticated caller role, never from client text), `lineage_id`, sorted `parents`, `base_state_hash`, optional `merge_base`/`restore_reference`, bounded restricted `message`, `delta` (sorted adds/removes, optional network `from`/`to`) and the complete `result_state` (network mode + sorted grants). Each grant carries id, contract type, lifetime (persistent/timed/session/one_shot with start, expiration, session binding, usage limit), resource (category, id, owner, sorted operations) and delegation (permitted, depth, delegable operations, parent grant). This covers every PROJECT §5.1 field (wire format §6.8).
7. **Commit types and rules:** `genesis`, `revision`, `revocation`, `temporary_grant`, `merge`, `restore`, `identity_transition`, with per-type parent counts, authority classes and delta shapes (wire format §6.2). Temporary authority cannot add persistent grants and nothing copies temporary grants into persistent state. A merge applies its delta to an explicit `merge_base`; other parents contribute nothing implicitly. Restore adds only grants equal in scope to the referenced historical state, with new ids and current policy. `identity_transition` explicitly rebinds a lineage to an updated package/executable (PROJECT §6.1).
8. **Anti-replay at the schema level:** a `grant_id` can be added once per lineage, ever; `request_id` is unique per user. Old bytes, head resets and merge unions therefore cannot revive authority (enforced at admission, ADR 0006).
9. **Genesis:** no parents, base is the empty Offline state (`empty_state_hash` published), persistent authority only.
10. **Evolution:** `schema_version`, `typ` and external_aad are versioned together; no unknown-version or unknown-field tolerance; new versions are new forward commits under fresh approval; verifiers retain old versions for history; limit changes are version changes.

The independent oracle ([tests/m2/oracle/ecr_oracle.py](../../tests/m2/oracle/ecr_oracle.py)) implements this profile without the Rust code and publishes 17 valid objects (two users' identity enrollments and certificates, a key invalidation, one commit of every type in a complete lineage and a second user's genesis), 80 single-object invalid vectors across all five byte-level layers with exact expected codes, and 7 lineage-admission failures. A second independent library (cbor2) confirms every valid object is canonical.

## Alternatives considered

- **Integer map keys for payload fields:** smaller encodings; rejected because readable snake_case keys make retained audit history self-describing and both choices are equally deterministic. COSE header labels remain integers as the standard requires.
- **COSE content type (label 3) or no explicit type:** content type describes the payload, not the object; `typ` (RFC 9596) is the standardized defense against cross-protocol confusion. Combined with distinct external_aad.
- **`alg` −8 (EdDSA):** deprecated as polymorphic by RFC 9864; rejected in favor of −19.
- **Decode-then-reencode with a general CBOR library (ciborium):** relies on library behavior for duplicates, tags, floats and limits; rejected for the verifier. A dedicated strict decoder for this small subset is simpler to audit; ciborium/cbor2 serve as differential oracles in tests.
- **Parse before signature verification:** simpler control flow; rejected so unauthenticated payload bytes never reach schema code.
- **Integer epoch timestamps:** simpler comparisons; ADR 0001 specifies ISO-8601 UTC, so a single fixed 20-byte form is used and parsed strictly.
- **State by reference (hash only) instead of embedded `result_state`:** smaller commits; rejected for v1 because self-contained commits let any authorized verifier recompute delta/state consistency from history alone. State size is bounded (256 grants, 64 KiB envelope).
- **Revocation as a revision without a distinct type:** possible, but a distinct type makes explicit revocations auditable and lets the lifecycle design restrict before the commit (ADR 0007).
- **Domain-separated commit id:** ADR 0001 proposed plain SHA-256 of the envelope; the tag-18 envelope with its signed `typ` is already self-delimiting, so the ADR's identifier is kept.

## Tradeoffs and consequences

- Self-contained commits repeat state; bounded limits keep this small. History viewers can reconstruct every state without trusting indexes.
- The strict profile rejects valid-but-noncanonical CBOR from other producers; only cham3's own encoder produces signed objects.
- `message` is restricted (no bidi or control characters except bounded newlines) to reduce display spoofing; renderers still escape it.

## Security implications

- A valid signature establishes only issuer and integrity of exact bytes. Key authority, admission, current authorization and enforcement remain separate layers (SEC-03).
- Strict verification and deterministic encoding prevent signature and envelope malleability from producing a second `commit_id` for the same content.
- Cross-kind confusion is blocked twice (signed `typ` and verifier-chosen external_aad). Cross-user reuse is blocked by `actor`/`subject`/`lineage_id` binding and identityd key ownership (SEC-15).
- Replay and stale restoration are blocked by lineage-unique grant ids, per-user request ids, explicit parents/base state and admission-time head/epoch checks (SEC-04).

## Verification

| Check | Environment / command | Result | Evidence / limits |
|---|---|---|---|
| Vector generation is deterministic and self-consistent; corrupted copies fail | arm64 Debian 13.7 container, `python3 -I tests/m2/oracle/ecr_oracle.py check tests/m2/vectors/ecr_v1/vectors.json`; `tests/m2/harness/test_m2_ecr_oracle.py` | PASS (development) | Oracle only; no Rust implementation exists yet |
| Rust verifier reproduces every byte, hash and error code | M2-003 | NOT_RUN | Gated on approval of this ADR |
| Fuzzing of the decoder/verifier | M2-003 | NOT_RUN | Bounded property/mutation testing per ADR 0003 |

## Approval and supersession

- **Owner approval evidence:** none yet. Requires independent review of the exact proposal, specification and vectors, then an explicit owner decision.
- **Supersedes / is superseded by:** none. Refines ADR 0001's proposed schema details within its approved scope.
