# cham3 ECR wire format, version 1

- **Status:** PROPOSED under G2 ([ADR 0004](../decisions/0004_m2_wire_schema_hash.md)), revised after the independent review published at 9cf882503848f8c6cc4938a588cc9fc062a9392d (findings R06–R08; resolutions in [docs/reviews/M2-001-002-finding-resolutions.md](../reviews/M2-001-002-finding-resolutions.md)). Not approved. M2-003 implementation is gated on independent re-review and owner approval.
- **Schema:** [m2_ecr_v1.cddl](m2_ecr_v1.cddl). This document is normative where the CDDL cannot express a rule (ordering, limits, text restrictions, semantics, validation order and error codes).
- **Vectors:** [tests/m2/vectors/ecr_v1/](../../tests/m2/vectors/ecr_v1/vectors_readme.md), generated and checked by the independent oracle [tests/m2/oracle/ecr_oracle.py](../../tests/m2/oracle/ecr_oracle.py).
- **Authority:** PROJECT.md §§4.2, 5, 5.7; ADR 0001. Required primitives are unchanged: Ed25519 (RFC 8032), COSE_Sign1 (RFC 9052), deterministic CBOR (RFC 8949 §4.2), SHA-256.

Names in this document use the repository convention; COSE/CBOR terms (COSE_Sign1, Sig_structure, kid, alg, typ) are external spellings kept at the standards boundary.

## 1. Layers and separation of facts

Verification is layered. Each layer reports a distinct fact and never implies the next:

1. **Envelope decode** — the input is a well-formed `cham3_dcbor_v1` item within limits, and the protected header byte string decodes the same way.
2. **Envelope** — the item is a `signed_object` with the exact protected header for the expected object kind.
3. **Signature** — `kid` resolves through the caller-supplied key resolver to an Ed25519 public key, and the signature over the Sig_structure verifies with strict verification. Payload bytes are authenticated before they are parsed.
4. **Payload decode and schema** — the authenticated payload is a well-formed `cham3_dcbor_v1` item matching the closed schema for the kind (field set, types, formats, lengths, ordering, cross-field identity rules).
5. **Intrinsic semantics** — stateless rules: commit type, lifetime, delegation, delta/result consistency that need no registry state.
6. **Key authority** (G3, identityd) — the resolved key is authenticated and its certificate, owner, authority class and epoch permit this object.
7. **Admission** (G4/G5, registryd) — parents exist in the lineage, expected head and policy epoch are current, base state matches, identifiers are unused, delegation parents are valid.
8. **Current authorization** (policyd) — evaluated per request; signature validity and admission are not current authorization.
9. **Enforcement confirmation** — outside M2's core; a commit is never "active" because it verified.

Layers 1–5 are pure functions of the bytes and a key resolver, and are fully specified here. Layers 6–8 are specified in ADRs 0005–0007. A resolver that knows a key says nothing about that key's current authority (layer 6).

## 2. Deterministic CBOR profile `cham3_dcbor_v1`

A strict subset of RFC 8949 core deterministic encoding (§4.2.1).

### 2.1 Allowed data items

| Major type | Allowed | Notes |
|---|---|---|
| 0 unsigned integer | yes | 0 ..= 2^64−1 |
| 1 negative integer | yes | −2^63 ..= −1 only (argument ≤ 2^63−1) |
| 2 byte string | yes | definite length only |
| 3 text string | yes | definite length, valid UTF-8 (RFC 3629) |
| 4 array | yes | definite length only |
| 5 map | yes | definite length only; key order below |
| 6 tag | tag 18 only, and only as the outermost item of a `signed_object` | all other tags rejected |
| 7 simple/float | `false` (0xf4) and `true` (0xf5) only | null, undefined, other simple values and all floats rejected |

### 2.2 Encoding rules

- Every integer argument (values, lengths, tags) uses the shortest form (RFC 8949 §4.2.1 preferred serialization). Any longer form is `non_minimal_encoding`.
- Additional information 28–30 is `reserved_additional_info`; 31 (indefinite length or break) is `indefinite_length`.
- Map keys are compared as their complete deterministic encodings, bytewise lexicographically; keys must be strictly increasing. Equal keys are therefore also rejected (`unsorted_or_duplicate_key`). In every cham3 map all keys share one major type (text for payload maps, unsigned/negative integers for COSE headers); a map whose keys mix types or use any other type is `map_key_type`.
- The input must contain exactly one top-level item; remaining bytes are `trailing_bytes`.
- Optional fields are absent when unused; `null` is never used.

### 2.3 Decoder limits

| Limit | Value | Error |
|---|---|---|
| Complete signed object (envelope) | 61 440 bytes | `input_too_large` |
| Nesting depth (array/map/tag levels, per decode) | 8 | `depth_exceeded` |
| Items in one array or pairs in one map | 256 | `container_too_large` |
| Text string length | 1 024 bytes | `text_too_long` |
| Byte string length | 61 440 bytes | `input_too_large` |

Schema limits (§5) are stricter for specific fields. Payload and protected-header byte strings are decoded as separate top-level items, each with its own depth count; both lie inside the envelope and so inherit its size bound. A producer (contractd) must refuse to propose an object whose envelope would exceed 61 440 bytes; the schema's list limits alone do not guarantee that.

**Container budget (R08).** Every IPC message and every journal record is a separate `cham3_dcbor_v1` item of at most 65 536 bytes. A message or record may embed at most one signed object, and its other content is at most 4 096 bytes, so the largest valid object always fits: the published maximum vector (exactly 61 440 bytes) wrapped as a minimal `submit_signed` request is 61 533 bytes and as a worst-case `commit_admitted` record is 61 619 bytes (`limits` in the vectors). Responses that could exceed a container (history, export) are paginated ([IPC §2](m2_service_ipc_v1.md)).

### 2.4 Decode order and first error

Decoding is a single depth-first pass in byte order. For each item the decoder checks, in order: (a) bytes remain (`truncated`); (b) additional information is not reserved or indefinite; (c) the argument is fully present (`truncated`) and minimal; (d) type rules — float/simple/tag restrictions, negative range, string length against remaining input and limits, UTF-8 validity, depth, container size; (e) for maps, each key's order relative to the previous key immediately after that key is decoded. The first violation encountered is the reported error. After the top-level item, `trailing_bytes` is checked. Implementations must not allocate according to an untrusted length before checking it against the remaining input and limits.

## 3. Scalar formats

### 3.1 Identifiers

- `random_id`: 16 bytes from the Linux CSPRNG (`getrandom(2)`); generated by the trusted service that creates the object. Uniqueness is enforced at admission (§6.6), not assumed.
- `key_id`, `user_id`, `commit_id`, `state_hash`, `lineage_id`, `sha256_digest`: 32 bytes.

### 3.2 `app_id`, `ascii_label`, `operation`, `package_identity`

- `app_id`, `ascii_label`: `[a-z0-9][a-z0-9._-]*`, lengths per CDDL.
- `operation`: `[a-z][a-z0-9_]*`, 1–32 bytes.
- `package_identity`: printable ASCII 0x21–0x7e, 1–255 bytes.

### 3.3 Restricted resource text

`resource_id`: UTF-8, 1–512 bytes, no code point in U+0000–U+001F, U+007F–U+009F, the bidirectional controls U+061C, U+200E, U+200F, U+202A–U+202E, U+2066–U+2069, or U+FEFF.

### 3.4 Time

`utc_time` is exactly `YYYY-MM-DDTHH:MM:SSZ` (20 ASCII bytes): year 2000–9999, a valid Gregorian calendar date, hour 00–23, minute 00–59, second 00–59 (no leap second 60), uppercase `T` and `Z`. Values are compared as UTC instants. Time values are claims by the trusted service that created them; they never establish ordering, freshness or current validity on their own (ADR 0001).

### 3.5 Display text (`message`)

`message` is the bounded human-readable description and rationale (ADR 0001 `commit_message`). UTF-8, 0–1 024 bytes, at most 16 lines; the only permitted control is LF (U+000A); every code point excluded in §3.3 other than LF is rejected; no leading or trailing whitespace on the whole value. It is not authority: the structured delta controls. Renderers must still escape it. Secrets and unnecessary sensitive resource details must not be placed in it (G6).

## 4. Signed object envelope (`cham3_cose_sign1_v1`)

### 4.1 Structure

`signed_object = #6.18([protected, unprotected, payload, signature])`, exactly four elements:

- `protected`: byte string whose content is the `cham3_dcbor_v1` encoding of exactly `{1: -19, 4: kid, 16: typ}` (keys ascending: 0x01, 0x04, 0x10). Any other label, value or encoding is `protected_header_invalid`; an `alg` other than −19 (including −8, polymorphic EdDSA, deprecated by RFC 9864) is `unsupported_algorithm`; `typ` not equal to the expected kind is `wrong_object_type`.
- `unprotected`: exactly the empty map `0xa0`; otherwise `unprotected_not_empty`.
- `payload`: attached byte string (never `nil`/detached) decoding to the kind's payload schema.
- `signature`: byte string of exactly 64 bytes; otherwise `signature_length`.

The caller always states which kind it expects; a valid object of another kind is rejected (`wrong_object_type`).

### 4.2 Kinds and domain separation

| Kind | `typ` | external_aad (ASCII) | Signing key |
|---|---|---|---|
| ECR commit | `application/vnd.cham3.ecr-commit.v1+cose` | `cham3/ecr-commit/v1` | user persistent or temporary contract key |
| Identity enrollment | `application/vnd.cham3.identity-enrollment.v1+cose` | `cham3/identity-enrollment/v1` | the enrolled identity key (proof of possession) |
| Key certificate | `application/vnd.cham3.key-certificate.v1+cose` | `cham3/key-certificate/v1` | user identity key |
| Key invalidation | `application/vnd.cham3.key-invalidation.v1+cose` | `cham3/key-invalidation/v1` | user identity key |

Two independent mechanisms separate cham3 contract signatures from every other use: the signed `typ` and the verifier-supplied external_aad. A signature produced for one kind cannot verify as another, and no cham3 key signs anything other than these Sig_structures.

### 4.3 Signed bytes

`to_be_signed = dcbor(["Signature1", protected, external_aad, payload])` — RFC 9052 §4.4 Sig_structure with `protected` and `payload` as the exact byte strings from the envelope. Ed25519 (RFC 8032 §5.1, pure, no prehash or context) signs `to_be_signed`. Verification uses strict verification: reject non-canonical `S` (≥ L), non-canonical or small-order `R`/`A` encodings (`signature_invalid`). Public keys are additionally validated as canonical, non-small-order points at enrollment/certification.

### 4.4 Key binding

`kid` in the protected header names the signing key. For commits, payload `key_id` must equal `kid` (`key_id_mismatch`). For identity enrollment, `kid` must equal payload `user_id`, which must equal `key_id(public_key)`. For certificates and invalidations, `kid` must equal payload `owner_user_id`. A `kid` that does not resolve to an authenticated key is `unknown_key`; resolution and authority checks belong to identityd (ADR 0005).

## 5. Hashes and derived identifiers

- `domain_hash(tag, data) = SHA-256(tag || 0x00 || data)` with an ASCII `tag` containing no NUL byte (prefix-free).
- `commit_id = SHA-256(envelope_bytes)` over the complete stored tagged envelope (ADR 0001's identifier). Determinism of the envelope (deterministic encoding, deterministic Ed25519, strict verification) gives one `commit_id` per signed content.
- `key_id = domain_hash("cham3/key-id/ed25519/v1", public_key)`; `user_id` is the `key_id` of the identity key.
- `lineage_id = domain_hash("cham3/lineage/v1", dcbor([owner_user_id, app_id, installation_id]))`.
- `state_hash = domain_hash("cham3/state/v1", dcbor(contract_state))`.
- `empty_state = {"grants": []}`; `empty_state_hash` is its `state_hash`. A state without a network grant is Offline (PROJECT §3.2).

## 6. Commit payload semantics

### 6.1 Field rules (schema layer)

- `parents`: 0–8 commit ids, strictly ascending (canonical parent order).
- `add_grants`, `result_state.grants`: strictly ascending `grant_id`; `remove_grant_ids`, `operations`, `max_operations`, `mandatory_constraints`: strictly ascending; list limits per CDDL (`list_too_long`, `list_not_strictly_ascending`).
- `subject.owner_uid == actor.uid` and `subject.owner_user_id == actor.user_id` (v1 has no cross-user authorization: `subject_actor_mismatch`).
- `lineage_id` equals §5's derivation from the subject (`lineage_mismatch`).

### 6.2 Commit types (intrinsic layer)

| Type | Parents | Authority class | Adds | Removes | Network change | Extra fields |
|---|---|---|---|---|---|---|
| `genesis` | 0 | persistent | persistent only | none | — | `base_state_hash == empty_state_hash` |
| `revision` | 1 | persistent | persistent only | allowed | — | at least one add or remove |
| `revocation` | 1 | persistent | none | ≥1 | — | — |
| `temporary_grant` | 1 | persistent or temporary | ≥1, each `timed`, `session` or `one_shot` | none | — | — |
| `merge` | 2–8 | persistent | persistent only | allowed | — | `merge_base` ∈ `parents` |
| `restore` | 1 | persistent | persistent only, ≥0 | allowed | — | `restore_reference` |
| `identity_transition` | 1 | persistent | none | none | required | `identity_change` (§6.9) |

The column "Identity change" is the `delta.identity_change` field; it is required on `identity_transition` and forbidden on every other type (`commit_type_rule`). Network changes are ordinary adds/removes of the network grant (§6.10).

`merge_base` appears only on merges and `restore_reference` only on restores (`commit_type_rule`). `identity_transition` is the explicit, user-approved rebinding of an existing lineage to an updated package or executable (PROJECT §6.1); its state is unchanged, the change is carried and displayed as `delta.identity_change`, and the new subject applies to the lineage's grants from that commit onward. Temporary authority can never add a persistent grant, and no commit type copies a temporary grant into persistent state: making a temporary permission persistent requires a new `grant_id` with a persistent lifetime under persistent authority (ADR 0001: no promotion).

### 6.3 Lifetimes (intrinsic layer, `lifetime_rule`)

| Kind | `expiration` | `session_id` | `usage_limit` |
|---|---|---|---|
| `persistent` | optional, > `effective_start` | absent | absent |
| `timed` | required, > `effective_start` | absent | absent |
| `session` | optional, > `effective_start` | required | absent |
| `one_shot` | optional, > `effective_start` | optional | required, exactly 1 |

### 6.4 Delegation (intrinsic layer, `delegation_rule`)

- `permitted = false` ⇒ `max_depth = 0` and `max_operations` empty.
- `permitted = true` ⇒ 1 ≤ `max_depth` ≤ 8 and `max_operations` is a non-empty subset of `resource.operations`.
- `parent_grant_id` marks a delegated grant; admission checks (ADR 0007) require the parent to be a currently valid, delegation-permitting grant of the same owner and resource, `operations ⊆ parent.max_operations`, `max_depth < parent.max_depth`, and a lifetime no longer than the parent's. Revoking the parent invalidates dependent grants.

### 6.5 Delta and result state (intrinsic + admission, `delta_rule`, `result_state_mismatch`, `base_state_mismatch`)

Given the base state (the `empty_state` for genesis; the parent's `result_state` for single-parent commits; the `merge_base` parent's `result_state` for merges):

1. `state_hash(base) == base_state_hash`.
2. Every `remove_grant_ids` entry exists in `base.grants`; every `add_grants` id exists in neither `base.grants` nor `remove_grant_ids`.
3. `result_state` equals, byte for byte after deterministic encoding, `{"grants": (base.grants − removed) ∪ added sorted by grant_id}`.

Intrinsically (without the base) a verifier checks: added grants appear unchanged in `result_state` and removed ids do not. Items 1–3 in full require the parent and are admission checks. A merge's other parents contribute nothing implicitly: their grants are absent unless explicitly added with fresh ids; their revocations stay effective because revoked ids can never be re-added (§6.6).

### 6.6 Lineage-wide uniqueness (admission)

Within a lineage, a `grant_id` may be added at most once, ever; a removed, revoked, expired or consumed grant id never reappears. Re-authorizing formerly granted scope (including restore) uses new ids. `request_id` is unique per user. These rules make old-byte replay, head reset and implicit merge union unable to restore authority.

### 6.7 Restore

`restore_reference` names a historical commit in the same lineage that is an ancestor of the parent. The restore target is the parent's state minus `remove_grant_ids` plus `add_grants`; every added grant — **including a network grant** — must match a grant of the referenced commit's `result_state` in `contract_type`, `resource` and `delegation`, with a new `grant_id`, a persistent lifetime, a fresh `effective_start` and, when the historical grant had an `expiration`, an expiration no later than it (no expansion beyond the historical snapshot: `restore_rule`). Removals only narrow. Temporary grants, ended sessions and consumed one-shots are never restored. Current policy, identity and key authority apply, and fresh approval of the complete displayed target is required (ADR 0005).

### 6.8 Contract-to-commit relationship

PROJECT §5.1's contract fields are carried by the commit that admits a grant: identification (`schema_version`, `grant_id`, `contract_type`, `policy.version`, `timestamp`), issuer (`actor`, `key_id`, `authority_class`, `authorization.session_id`), subject (`subject`), resource, validity (`lifetime`), delegation and authorization (`authorization.request_id`, `policy.epoch`, `policy.security_profile`, `policy.mandatory_constraints`). The signed contract for a grant is the commit envelope that added it; later commits that keep the grant in `result_state` restate but do not re-issue it.

### 6.9 Subject binding and identity transitions (R06)

The **authoritative subject** of a lineage is the `subject` of its genesis, replaced only by each admitted `identity_transition`. At admission:

- every commit other than `genesis` and `identity_transition` (revision, revocation, temporary grant, merge, restore) must carry a `subject` byte-identical to the authoritative subject (`subject_binding_mismatch`); merges, temporary branches and restores therefore cannot rebind identity;
- an `identity_transition` must keep every subject field except `package_identity` and `executable_sha256` identical, and its `identity_change.from` must equal the authoritative subject's values (`subject_binding_mismatch`); intrinsically, `identity_change.to` must equal the commit's own subject values and differ from `from` (`identity_transition_rule`).

### 6.10 Network authority (R07)

Network access is an ordinary grant with category `network`, `resource_id` `"network_mode"`, `resource_owner_uid` equal to the subject owner, exactly one operation (`direct` or `tor`), no delegation and any lifetime except `one_shot`; a state holds at most one network grant (`network_grant_rule`). A lineage with no eligible network grant is Offline. Network authority therefore has a grant id, lifetime, session/expiry bounds, explicit revocation, restriction, key/policy invalidation and restore matching exactly like other grants. A temporary network grant is only admissible on a branch whose base has no network grant (the one-network-grant rule); changing a persistent mode is a revision that removes one network grant and adds another. Mandatory policy can still forbid a mode (PROJECT §3.3); policyd then reports the lineage Offline.

## 7. Errors

Implementations report exactly one stable error code per rejected object: the first failure in layer order (§1), and within a decode the first failure in byte order (§2.4). Decode codes can therefore come from layer 1 (envelope or protected header bytes) or layer 4 (authenticated payload bytes). Codes:

- Decode: `input_too_large`, `truncated`, `trailing_bytes`, `reserved_additional_info`, `indefinite_length`, `non_minimal_encoding`, `integer_out_of_range`, `float_forbidden`, `simple_value_forbidden`, `tag_forbidden`, `invalid_utf8`, `text_too_long`, `depth_exceeded`, `container_too_large`, `unsorted_or_duplicate_key`, `map_key_type`.
- Envelope: `not_signed_object` (not a 4-element array under tag 18, or an element of the wrong major type), `protected_header_invalid`, `unsupported_algorithm`, `wrong_object_type`, `unprotected_not_empty`, `signature_length`.
- Schema: `unknown_field`, `missing_field`, `wrong_type`, `value_out_of_range`, `invalid_text`, `invalid_time`, `invalid_enum`, `list_too_long`, `list_not_strictly_ascending`, `subject_actor_mismatch`, `lineage_mismatch`, `key_id_mismatch`.
- Signature: `unknown_key`, `signature_invalid`.
- Intrinsic semantics: `commit_type_rule`, `lifetime_rule`, `delegation_rule`, `network_grant_rule`, `identity_transition_rule`, `delta_rule`, `result_state_mismatch`, `base_state_mismatch` (genesis only; for other types it is an admission check).
- Admission codes (ADR 0006/0007): `parent_unknown`, `base_state_mismatch`, `delta_rule`, `result_state_mismatch`, `duplicate_identifier`, `restore_rule`, `subject_binding_mismatch`, `stale_head`, `stale_epoch`, … The published admission vectors use the first seven.

Within the schema layer, fields are checked in ascending key order; within a field, type, then format/limits, then ordering.

## 8. Evolution

- `schema_version` and `typ` are both versioned. v1 verifiers reject any other version (`invalid_enum`/`wrong_object_type`); there is no "best effort" parsing of unknown versions and no unknown-field tolerance.
- A future schema version is a new `typ`, external_aad and payload schema. Verifiers keep every historical version needed to verify retained history. Migration never rewrites signed bytes: a migration to a new version is a new forward commit, parented on the current head, freshly approved under the new schema; any automatic migration needs its own owner-approved ADR.
- Increasing limits is a version change because it changes which byte strings are valid.

## 9. Published vectors

`tests/m2/vectors/ecr_v1/` holds valid objects (identity enrollments, key certificates, a key invalidation, one commit of every type for two synthetic users and a maximum-size 61 440-byte commit), invalid objects each labeled with the expected error code and layer (including the 61 441-byte boundary), lineage-admission failures (including identity substitution by every non-transition commit type and restore scope/network/expiration violations), and the `limits` block with worst-case wrapper sizes. All keys are synthetic, derived from public labels, and must never be used outside tests. The vectors are produced by an implementation independent of the Rust code and re-verified by it; the Rust verifier (M2-003) must reproduce every byte and error code.
