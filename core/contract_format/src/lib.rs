//! Canonical cham3 contract schema, deterministic CBOR profile and COSE_Sign1
//! envelope handling (M2-003; wire format `docs/specs/m2_wire_format_v1.md`,
//! ADR 0004 / G2).
//!
//! [`verify_object`] implements verification layers 1–5 of wire format §1: envelope
//! decode, envelope, strict Ed25519 signature, payload decode and closed schema, and
//! stateless commit semantics. Each layer reports one stable error code (§7).
//!
//! A verified object is only that: its bytes are well formed and signed by the key
//! the resolver named. It is **not** key authority (layer 6, identityd), admission
//! (layer 7, registryd), current authorization (layer 8, policyd) or enforcement.
//! This crate signs nothing; signing belongs to vaultd (ADR 0005).
#![forbid(unsafe_code)]

pub mod dcbor;
pub mod envelope;
pub mod error;
pub mod hash;
pub mod intrinsic;
pub mod schema;
pub mod text;

pub use envelope::{key_resolver_t, object_kind_t};
pub use error::{error_code_t, format_error_t, format_result_t, layer_t};

use dcbor::{decode, value_t};
use hash::digest_t;
use schema::{commit_payload_t, identity_enrollment_t, key_certificate_t, key_invalidation_t};

/// Canonical crate name, reported by qualification probes.
pub const CrateName: &str = "cham3_contract_format";

/// Returns the canonical crate name.
pub fn crate_name() -> &'static str {
    CrateName
}

/// The parsed payload of a verified object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum payload_t {
    commit(Box<commit_payload_t>),
    identity_enrollment(identity_enrollment_t),
    key_certificate(key_certificate_t),
    key_invalidation(key_invalidation_t),
}

impl payload_t {
    /// The payload as a value; its deterministic encoding is the signed payload.
    pub fn to_value(&self) -> value_t {
        match self {
            payload_t::commit(payload) => payload.to_value(),
            payload_t::identity_enrollment(payload) => payload.to_value(),
            payload_t::key_certificate(payload) => payload.to_value(),
            payload_t::key_invalidation(payload) => payload.to_value(),
        }
    }
}

/// An object that passed layers 1–5. Signature validity only: not key authority,
/// admission, current authorization or enforcement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct verified_object_t {
    pub kind: object_kind_t,
    /// The signing key named by the protected header.
    pub kid: digest_t,
    /// SHA-256 of the complete envelope (`commit_id` for commits).
    pub object_id: digest_t,
    /// The authenticated payload bytes.
    pub payload_bytes: Vec<u8>,
    pub payload: payload_t,
}

/// Verification layers 1–5 for an object of `expected_kind`. Returns the first
/// failure in layer order (wire format §7).
pub fn verify_object(
    data: &[u8],
    expected_kind: object_kind_t,
    resolver: &dyn key_resolver_t,
) -> format_result_t<verified_object_t> {
    let envelope = envelope::verify_envelope(data, expected_kind, resolver)?;
    let value = decode(&envelope.payload, layer_t::payload_decode, false)?;
    let payload = match expected_kind {
        object_kind_t::commit => {
            let commit = commit_payload_t::parse(&value, &envelope.kid)?;
            intrinsic::check_commit(&commit)?;
            payload_t::commit(Box::new(commit))
        }
        object_kind_t::identity_enrollment => {
            payload_t::identity_enrollment(identity_enrollment_t::parse(&value, &envelope.kid)?)
        }
        object_kind_t::key_certificate => {
            payload_t::key_certificate(key_certificate_t::parse(&value, &envelope.kid)?)
        }
        object_kind_t::key_invalidation => {
            payload_t::key_invalidation(key_invalidation_t::parse(&value, &envelope.kid)?)
        }
    };
    Ok(verified_object_t {
        kind: expected_kind,
        kid: envelope.kid,
        object_id: envelope.object_id,
        payload_bytes: envelope.payload,
        payload,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::crate_name(), "cham3_contract_format");
    }
}
