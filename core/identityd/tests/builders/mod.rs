//! Test builders: synthetic Ed25519 keys and signed identity objects.
//!
//! Every seed is `SHA-256("cham3 m2-004 synthetic test key: " || label)`; these keys
//! are public test material and must never sign anything outside tests.

#![allow(dead_code)]

use cham3_contract_format::dcbor::{encode, value_t};
use cham3_contract_format::envelope::{assemble_envelope, protected_header_bytes, to_be_signed};
use cham3_contract_format::hash::{digest_t, key_id, sha256};
use cham3_contract_format::object_kind_t;
use cham3_contract_format::schema::{
    authority_class_t, identity_enrollment_t, invalidation_reason_t, key_certificate_t,
    key_invalidation_t, random_id_t,
};
use ed25519_dalek::{Signer, SigningKey};

pub const T0: &str = "2026-10-13T08:00:00Z";
pub const T1: &str = "2026-10-13T09:00:00Z";
pub const T2: &str = "2026-10-14T08:00:00Z";
pub const Week: &str = "2026-10-20T08:00:00Z";
pub const WeekAndASecond: &str = "2026-10-20T08:00:01Z";

pub struct test_key_t {
    pub signing: SigningKey,
    pub public_key: [u8; 32],
    pub key_id: digest_t,
}

pub fn test_key(label: &str) -> test_key_t {
    let seed = sha256(format!("cham3 m2-004 synthetic test key: {label}").as_bytes());
    let signing = SigningKey::from_bytes(&seed);
    let public_key = signing.verifying_key().to_bytes();
    test_key_t {
        signing,
        public_key,
        key_id: key_id(&public_key),
    }
}

pub fn synthetic_id(label: &str) -> random_id_t {
    let digest = sha256(label.as_bytes());
    let mut id = [0u8; 16];
    id.copy_from_slice(&digest[..16]);
    id
}

/// Signs `payload` as an object of `kind` under `signer` (deterministic Ed25519).
pub fn sign_object(kind: object_kind_t, signer: &test_key_t, payload: &value_t) -> Vec<u8> {
    let payload_bytes = encode(payload);
    let protected = protected_header_bytes(&signer.key_id, kind);
    let message = to_be_signed(kind, &protected, &payload_bytes);
    let signature = signer.signing.sign(&message).to_bytes();
    assemble_envelope(&protected, &payload_bytes, &signature)
}

pub fn enrollment_payload(identity: &test_key_t, uid: u32) -> identity_enrollment_t {
    identity_enrollment_t {
        user_id: identity.key_id,
        public_key: identity.public_key,
        owner_uid: uid,
        enrolled: T0.to_owned(),
        identity_epoch: 1,
    }
}

pub fn enrollment(identity: &test_key_t, uid: u32) -> Vec<u8> {
    sign_object(
        object_kind_t::identity_enrollment,
        identity,
        &enrollment_payload(identity, uid).to_value(),
    )
}

pub fn certificate_payload(
    identity: &test_key_t,
    uid: u32,
    subject: &test_key_t,
    class: authority_class_t,
    key_epoch: u64,
) -> key_certificate_t {
    key_certificate_t {
        certificate_id: synthetic_id(&format!("certificate {:02x?}", &subject.key_id[..4])),
        issued: T0.to_owned(),
        owner_uid: uid,
        owner_user_id: identity.key_id,
        subject_key_id: subject.key_id,
        subject_public_key: subject.public_key,
        authority_class: class,
        key_epoch,
        not_before: T0.to_owned(),
        not_after: match class {
            authority_class_t::temporary => Some(T2.to_owned()),
            authority_class_t::persistent => None,
        },
        supersedes_key_id: None,
    }
}

pub fn sign_certificate(identity: &test_key_t, payload: &key_certificate_t) -> Vec<u8> {
    sign_object(
        object_kind_t::key_certificate,
        identity,
        &payload.to_value(),
    )
}

pub fn certificate(
    identity: &test_key_t,
    uid: u32,
    subject: &test_key_t,
    class: authority_class_t,
    key_epoch: u64,
) -> Vec<u8> {
    sign_certificate(
        identity,
        &certificate_payload(identity, uid, subject, class, key_epoch),
    )
}

pub fn invalidation(
    identity: &test_key_t,
    target: &digest_t,
    reason: invalidation_reason_t,
    label: &str,
) -> Vec<u8> {
    let payload = key_invalidation_t {
        invalidation_id: synthetic_id(&format!("invalidation {label}")),
        issued: T1.to_owned(),
        owner_user_id: identity.key_id,
        target_key_id: *target,
        reason,
    };
    sign_object(
        object_kind_t::key_invalidation,
        identity,
        &payload.to_value(),
    )
}

/// One synthetic user: identity, persistent and temporary keys and their objects.
pub struct test_user_t {
    pub uid: u32,
    pub identity: test_key_t,
    pub persistent: test_key_t,
    pub temporary: test_key_t,
    pub enrollment: Vec<u8>,
    pub persistent_certificate: Vec<u8>,
    pub temporary_certificate: Vec<u8>,
}

pub fn test_user(uid: u32) -> test_user_t {
    let identity = test_key(&format!("user {uid} identity"));
    let persistent = test_key(&format!("user {uid} persistent"));
    let temporary = test_key(&format!("user {uid} temporary"));
    test_user_t {
        uid,
        enrollment: enrollment(&identity, uid),
        persistent_certificate: certificate(
            &identity,
            uid,
            &persistent,
            authority_class_t::persistent,
            1,
        ),
        temporary_certificate: certificate(
            &identity,
            uid,
            &temporary,
            authority_class_t::temporary,
            1,
        ),
        identity,
        persistent,
        temporary,
    }
}

/// An Ed25519 encoding that is not a canonical, non-small-order point.
pub const BadPoints: [[u8; 32]; 3] = [
    // The identity point (order 1).
    [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ],
    // y = 0: a point of order 4.
    [0; 32],
    // y = p + 1: a non-canonical encoding of the identity point.
    [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ],
];

pub fn reason_of(text: &str) -> invalidation_reason_t {
    invalidation_reason_t::parse(text).unwrap_or_else(|| panic!("reason {text}"))
}

pub fn class_value(class: authority_class_t) -> value_t {
    value_t::text_of(class.as_str())
}
