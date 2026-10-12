# ECR v1 published vectors

- **Status:** PROPOSED G2 vectors ([ADR 0004](../../../../docs/decisions/0004_m2_wire_schema_hash.md)); not approved. Regenerate only together with a reviewed change to [the wire format](../../../../docs/specs/m2_wire_format_v1.md).
- **Producer/oracle:** [tests/m2/oracle/ecr_oracle.py](../../oracle/ecr_oracle.py), an independent Python implementation (own deterministic CBOR encoder and strict decoder, libsodium Ed25519 via python3-nacl, cbor2 as a second canonical cross-check). It shares no code with the Rust crates.
- **Synthetic material only:** every key seed is `SHA-256("cham3 m2 synthetic test key v1: " || label)` and every identifier is derived from a public label. These keys are published and must never sign anything outside tests.

## Contents of `vectors.json`

| Section | Meaning |
|---|---|
| `constants` | `empty_state_hash`, per-kind `typ` and external_aad strings |
| `keys` | label, seed, public key and `key_id` of each synthetic key |
| `valid` | envelopes that must verify for their `kind`: identity enrollments, persistent/temporary key certificates, a retired-key certificate and invalidation for user 1001 and 1002; one commit of every type for user 1001's synthetic Kate lineage; a Tor genesis for user 1002. `object_id` is SHA-256 of the envelope (`commit_id` for commits); commits also carry `result_state_hash` |
| `lineage` | the valid user 1001 commits in admission order: genesis, revision (Offline→Direct, clipboard), session microphone branch, one-shot screenshot branch, merge (new persistent microphone grant), clipboard revocation, forward restore of clipboard with a new grant id, identity transition to an updated executable |
| `invalid` | single-object failures, each with `layer` (`envelope_decode`, `envelope`, `signature`, `payload_decode`, `schema`, `intrinsic`) and the exact expected `error` code; vectors past the signature layer are correctly signed so the intended rule is what fails |
| `admission_invalid` | lineage-context failures: admit the first `prefix` lineage commits, then this envelope must fail with `error` (`base_state_mismatch`, `result_state_mismatch`, `parent_unknown`, `duplicate_identifier`, `restore_rule`, `identity_transition_rule`) |

## Commands

From the repository root inside the pinned Debian container:

~~~sh
python3 -I tests/m2/oracle/ecr_oracle.py check tests/m2/vectors/ecr_v1/vectors.json
~~~

`check` regenerates the whole set, requires byte-for-byte equality with the committed file, then verifies every entry reports exactly its expected result. `tests/m2/run_m2_tests.sh unit` runs it; `tests/m2/harness/test_m2_ecr_oracle.py` proves corrupted copies fail.

The Rust implementation (gated M2-003) must accept every valid envelope, reproduce every `object_id`/`result_state_hash`, and report the same layer and error for every invalid vector.
