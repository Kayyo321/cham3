//! Signed object envelope `cham3_cose_sign1_v1` (wire format §4): layers 1–3.
//!
//! The payload bytes are authenticated before they are parsed. A verified signature
//! is not key authority, admission or current authorization (wire format §1).

use ed25519_dalek::{Signature, VerifyingKey};

use crate::dcbor::{SignedObjectTag, decode, encode, value_t};
use crate::error::{error_code_t, format_error_t, format_result_t, layer_t};
use crate::hash::{digest_t, object_id};

/// COSE header label `alg`.
pub const LabelAlg: u64 = 1;
/// COSE header label `kid`.
pub const LabelKid: u64 = 4;
/// COSE header label `typ` (RFC 9596).
pub const LabelTyp: u64 = 16;
/// COSE algorithm Ed25519 (RFC 9864), −19; stored as the CBOR negative argument 18.
pub const AlgorithmEd25519Argument: u64 = 18;
/// Length of an Ed25519 signature.
pub const SignatureBytes: usize = 64;

/// The four signed object kinds of wire format §4.2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum object_kind_t {
    commit,
    identity_enrollment,
    key_certificate,
    key_invalidation,
}

impl object_kind_t {
    /// Every kind, in wire format §4.2 order.
    pub const All: [object_kind_t; 4] = [
        object_kind_t::commit,
        object_kind_t::identity_enrollment,
        object_kind_t::key_certificate,
        object_kind_t::key_invalidation,
    ];

    /// The signed `typ` value.
    pub fn typ(self) -> &'static str {
        match self {
            object_kind_t::commit => "application/vnd.cham3.ecr-commit.v1+cose",
            object_kind_t::identity_enrollment => {
                "application/vnd.cham3.identity-enrollment.v1+cose"
            }
            object_kind_t::key_certificate => "application/vnd.cham3.key-certificate.v1+cose",
            object_kind_t::key_invalidation => "application/vnd.cham3.key-invalidation.v1+cose",
        }
    }

    /// The verifier-supplied external_aad (ASCII domain string).
    pub fn external_aad(self) -> &'static str {
        match self {
            object_kind_t::commit => "cham3/ecr-commit/v1",
            object_kind_t::identity_enrollment => "cham3/identity-enrollment/v1",
            object_kind_t::key_certificate => "cham3/key-certificate/v1",
            object_kind_t::key_invalidation => "cham3/key-invalidation/v1",
        }
    }

    /// The kind name used by the published vectors.
    pub fn name(self) -> &'static str {
        match self {
            object_kind_t::commit => "commit",
            object_kind_t::identity_enrollment => "identity_enrollment",
            object_kind_t::key_certificate => "key_certificate",
            object_kind_t::key_invalidation => "key_invalidation",
        }
    }

    /// Parses a vector kind name.
    pub fn from_name(name: &str) -> Option<object_kind_t> {
        object_kind_t::All
            .into_iter()
            .find(|kind| kind.name() == name)
    }

    fn from_typ(typ: &str) -> Option<object_kind_t> {
        object_kind_t::All
            .into_iter()
            .find(|kind| kind.typ() == typ)
    }
}

/// Resolves a `kid` to an Ed25519 public key. Implemented by identityd (ADR 0005);
/// tests use synthetic keys. Resolution says nothing about the key's current
/// authority, which is layer 6.
pub trait key_resolver_t {
    fn resolve(&self, kid: &digest_t) -> Option<[u8; 32]>;
}

/// Result of layers 1–3: a well-formed envelope of the expected kind whose
/// signature verified under the resolved key. The payload is authenticated but
/// not yet parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct authenticated_envelope_t {
    pub kind: object_kind_t,
    pub kid: digest_t,
    pub payload: Vec<u8>,
    /// SHA-256 of the complete envelope (the `commit_id` for commits).
    pub object_id: digest_t,
}

/// The protected header bytes `dcbor({1: -19, 4: kid, 16: typ})`.
pub fn protected_header_bytes(kid: &digest_t, kind: object_kind_t) -> Vec<u8> {
    encode(&value_t::map(vec![
        (
            value_t::unsigned(LabelAlg),
            value_t::negative(AlgorithmEd25519Argument),
        ),
        (value_t::unsigned(LabelKid), value_t::bytes_of(kid)),
        (value_t::unsigned(LabelTyp), value_t::text_of(kind.typ())),
    ]))
}

/// The bytes Ed25519 signs: `dcbor(["Signature1", protected, external_aad, payload])`
/// (RFC 9052 §4.4) with the kind's external_aad.
pub fn to_be_signed(kind: object_kind_t, protected: &[u8], payload: &[u8]) -> Vec<u8> {
    encode(&value_t::array(vec![
        value_t::text_of("Signature1"),
        value_t::bytes_of(protected),
        value_t::bytes_of(kind.external_aad().as_bytes()),
        value_t::bytes_of(payload),
    ]))
}

/// The complete tagged envelope `#6.18([protected, {}, payload, signature])`.
/// Producing the signature is vaultd's task (ADR 0005); this only assembles bytes.
pub fn assemble_envelope(
    protected: &[u8],
    payload: &[u8],
    signature: &[u8; SignatureBytes],
) -> Vec<u8> {
    encode(&value_t::tag(
        SignedObjectTag,
        Box::new(value_t::array(vec![
            value_t::bytes_of(protected),
            value_t::map(Vec::new()),
            value_t::bytes_of(payload),
            value_t::bytes_of(signature),
        ])),
    ))
}

fn envelope_error(code: error_code_t) -> format_error_t {
    format_error_t::new(layer_t::envelope, code)
}

fn signature_error(code: error_code_t) -> format_error_t {
    format_error_t::new(layer_t::signature, code)
}

/// Layers 1–3 for an object of `expected_kind`.
pub fn verify_envelope(
    data: &[u8],
    expected_kind: object_kind_t,
    resolver: &dyn key_resolver_t,
) -> format_result_t<authenticated_envelope_t> {
    let root = decode(data, layer_t::envelope_decode, true)?;
    let elements = match root {
        value_t::tag(SignedObjectTag, content) => match *content {
            value_t::array(elements) => elements,
            _ => return Err(envelope_error(error_code_t::not_signed_object)),
        },
        _ => return Err(envelope_error(error_code_t::not_signed_object)),
    };
    let (protected, unprotected, payload, signature) = match <[value_t; 4]>::try_from(elements) {
        Ok(
            [
                value_t::bytes(protected),
                value_t::map(unprotected),
                value_t::bytes(payload),
                value_t::bytes(signature),
            ],
        ) => (protected, unprotected, payload, signature),
        _ => return Err(envelope_error(error_code_t::not_signed_object)),
    };
    let kid = check_protected_header(&protected, expected_kind)?;
    if !unprotected.is_empty() {
        return Err(envelope_error(error_code_t::unprotected_not_empty));
    }
    let signature = <[u8; SignatureBytes]>::try_from(signature.as_slice())
        .map_err(|_| envelope_error(error_code_t::signature_length))?;
    let public_key = resolver
        .resolve(&kid)
        .ok_or(signature_error(error_code_t::unknown_key))?;
    let message = to_be_signed(expected_kind, &protected, &payload);
    if !strict_verify(&public_key, &message, &signature) {
        return Err(signature_error(error_code_t::signature_invalid));
    }
    Ok(authenticated_envelope_t {
        kind: expected_kind,
        kid,
        payload,
        object_id: object_id(data),
    })
}

/// Checks the protected header and returns its `kid`.
fn check_protected_header(
    protected: &[u8],
    expected_kind: object_kind_t,
) -> format_result_t<digest_t> {
    let invalid = envelope_error(error_code_t::protected_header_invalid);
    let header = decode(protected, layer_t::envelope_decode, false)?;
    let pairs = match header {
        value_t::map(pairs) => pairs,
        _ => return Err(invalid),
    };
    // Keys are strictly ascending after the decode, so exactly {1, 4, 16} appears
    // as these three pairs in this order.
    let [(alg_label, alg), (kid_label, kid), (typ_label, typ)] =
        <[(value_t, value_t); 3]>::try_from(pairs).map_err(|_| invalid)?;
    if alg_label != value_t::unsigned(LabelAlg)
        || kid_label != value_t::unsigned(LabelKid)
        || typ_label != value_t::unsigned(LabelTyp)
    {
        return Err(invalid);
    }
    match alg {
        value_t::negative(AlgorithmEd25519Argument) => {}
        value_t::unsigned(_) | value_t::negative(_) => {
            return Err(envelope_error(error_code_t::unsupported_algorithm));
        }
        _ => return Err(invalid),
    }
    let kid = match kid {
        value_t::bytes(bytes) => digest_t::try_from(bytes.as_slice()).map_err(|_| invalid)?,
        _ => return Err(invalid),
    };
    match typ {
        value_t::text(typ) if typ == expected_kind.typ() => Ok(kid),
        value_t::text(typ) if object_kind_t::from_typ(&typ).is_some() => {
            Err(envelope_error(error_code_t::wrong_object_type))
        }
        _ => Err(invalid),
    }
}

/// Validates an Ed25519 public key as a canonical encoding of a point that is not of
/// small order (wire format §4.3; enrollment and certification apply the same rule).
pub fn public_key_is_valid(public_key: &[u8; 32]) -> bool {
    match VerifyingKey::from_bytes(public_key) {
        Ok(key) => !key.is_weak() && key.to_edwards().compress().to_bytes() == *public_key,
        Err(_) => false,
    }
}

/// Strict Ed25519 verification (RFC 8032 §5.1.7 with the strict checks of wire
/// format §4.3): canonical non-small-order public key, canonical `S` and
/// non-small-order `R` (ed25519-dalek `verify_strict`).
pub fn strict_verify(
    public_key: &[u8; 32],
    message: &[u8],
    signature: &[u8; SignatureBytes],
) -> bool {
    if !public_key_is_valid(public_key) {
        return false;
    }
    match VerifyingKey::from_bytes(public_key) {
        Ok(key) => key
            .verify_strict(message, &Signature::from_bytes(signature))
            .is_ok(),
        Err(_) => false,
    }
}
