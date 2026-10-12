# ECR v1 published vectors

- **Status:** APPROVED with G2 (owner, 2026-10-12, after independent re-review bd081df; vectors at 28a8595). History: PROPOSED G2 vectors, revision 3 after independent review 9cf8825 (R06–R08) and re-review 336f13b (R07-r2, R12, R13); revision 4 after re-review f451f2e changes only `journal_scenarios` (R14 key-naming interface) ([ADR 0004](../../../../docs/decisions/0004_m2_wire_schema_hash.md)). Regenerate only together with a reviewed change to [the wire format](../../../../docs/specs/m2_wire_format_v1.md).
- **Producer/oracle:** [tests/m2/oracle/ecr_oracle.py](../../oracle/ecr_oracle.py), an independent Python implementation (own deterministic CBOR encoder and strict decoder, libsodium Ed25519 via python3-nacl, cbor2 as a second canonical cross-check). It shares no code with the Rust crates.
- **Synthetic material only:** every key seed is `SHA-256("cham3 m2 synthetic test key v1: " || label)` and every identifier is derived from a public label. These keys are published and must never sign anything outside tests.

## Contents of `vectors.json`

| Section | Meaning |
|---|---|
| `constants` | `empty_state_hash`, per-kind `typ` and external_aad strings |
| `keys` | label, seed, public key and `key_id` of each synthetic key |
| `valid` | envelopes that must verify for their `kind`: identity enrollments, persistent/temporary key certificates, a retired-key certificate and invalidation for user 1001 and 1002; one commit of every type for user 1001's synthetic Kate lineage; a Tor genesis (network grant) for user 1002; a maximum-size 61 440-byte commit. `object_id` is SHA-256 of the envelope (`commit_id` for commits); commits also carry `result_state_hash` |
| `lineage` | the valid user 1001 commits in admission order: genesis, revision (Direct network grant, expiring clipboard grant), session microphone branch, one-shot screenshot branch, merge (new persistent microphone grant), clipboard revocation, forward restore of clipboard with a new grant id and the historical expiration, identity transition (`identity_change`) to an updated executable |
| `invalid` | single-object failures, each with `layer` (`envelope_decode`, `envelope`, `signature`, `payload_decode`, `schema`, `intrinsic`) and the exact expected `error` code; vectors past the signature layer are correctly signed so the intended rule is what fails |
| `admission_invalid` | lineage-context failures: admit the first `prefix` lineage commits, then this envelope must fail with `error` (`base_state_mismatch`, `result_state_mismatch`, `parent_unknown`, `duplicate_identifier`, `restore_rule`, `subject_binding_mismatch`, `stale_head`); revision 3 adds the re-review's ancestral-session restore, a non-ancestor reference and two stale placements |
| `journal_scenarios` | revision 3: signed multi-step journals run in a fresh `journal_t` each; from revision 4 every step passes through the registry interface model `registry_service_t` (R14). A step is `{"valid": name}`, `{"commit": hex}` (contractd `admit`), `{"identity": hex, "kind": …}` (identityd `append_identity`), `{"record": {…}}` (an unsigned narrowing record registryd writes itself: `expired`, `session_ended`, `restriction`, `containment`, `quarantine`, `admin_recover`, `recovery_opened`, `recovery_closed`, `deletion_pending`; byte fields hex), `{"call": {caller, operation, args, caller_uid?}}` (a role-checked registryd operation: `append_identity`, `name_recovery_keys` — the only path of a `recovery_keys` record — or `recovery_status`) or `{"restart": true}` (startup roll-forward of a durable, unanchored tail), with `expect` (`"admitted"` or `{layer, error}`; interface errors use layer `ipc`), optional `anchor_fault` (the anchor write fails during the step), optional `result` (the call's result), optional `records` (durable journal records after the step), optional `keys` (revision 4.1, I04: the key-authority state after the step — `named`, `min_key_epoch`, sorted `retired` key ids — observed directly, independent of the response) and optional `check` (`current` / `ended` / `inactive` grant ids after the step). `key_authority: true` scenarios also apply layer 6 from the journal's own identity records. 9 network-slot scenarios (wire §6.12), 1 restore lifetime-kind scenario (§6.7), 9 recovery-ceremony scenarios and 3 key-naming interface scenarios (ADR 0006 §10); `finding` names the review item |
| `limits` | `max_envelope_bytes` (61 440), container limit and wrapper-metadata budget, and the measured sizes of the maximum object inside a minimal `submit_signed` message and a worst-case `commit_admitted` journal record (R08) |

## Commands

From the repository root inside the pinned Debian container:

~~~sh
python3 -I tests/m2/oracle/ecr_oracle.py check tests/m2/vectors/ecr_v1/vectors.json
~~~

`check` regenerates the whole set, requires byte-for-byte equality with the committed file, then verifies every entry reports exactly its expected result, including every journal-scenario step and grant standing. The journal and interface models decide admission, key authority, caller roles and journal/anchor ordering only; they do not evaluate time, sessions, mandatory policy or policy epochs, and they are not the registry (no sockets, files or crash points). `tests/m2/run_m2_tests.sh unit` runs it; `tests/m2/harness/test_m2_ecr_oracle.py` proves corrupted copies fail.

The Rust implementation (gated M2-003) must accept every valid envelope, reproduce every `object_id`/`result_state_hash`, and report the same layer and error for every invalid vector.
