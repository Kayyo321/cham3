//! A03: the Rust verifier against the published ECR v1 vectors of the independent
//! oracle (wire format §9). Every valid object verifies and reproduces its bytes,
//! object id and deterministic signature; every invalid object reports exactly the
//! published layer and code; objects that fail only at layers 6–8 pass layers 1–5.

// Test crate only: a failed assertion or panic is how a test fails, and the vectors
// are trusted test input, so the restriction lints that guard production code are
// relaxed here. The library itself keeps every workspace lint.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::as_conversions
)]

mod support;

use cham3_contract_format::dcbor::{MaxEncodedBytes, SignedObjectTag, decode, encode, value_t};
use cham3_contract_format::envelope::{assemble_envelope, protected_header_bytes, to_be_signed};
use cham3_contract_format::hash::{key_id, object_id};
use cham3_contract_format::intrinsic::empty_state_hash;
use cham3_contract_format::{layer_t, object_kind_t, verify_object};
use ed25519_dalek::{Signer, SigningKey};

use support::{
    fixed, hex_decode, hex_encode, json_t, resolver_t, synthetic_key_t, synthetic_keys, vectors,
};

/// Layers this crate implements (wire format §1, layers 1–5).
const LocalLayers: [&str; 6] = [
    "envelope_decode",
    "envelope",
    "signature",
    "payload_decode",
    "schema",
    "intrinsic",
];

fn outcome(data: &[u8], kind: object_kind_t, resolver: &resolver_t) -> String {
    match verify_object(data, kind, resolver) {
        Ok(_) => "ok".to_owned(),
        Err(error) => error.to_string(),
    }
}

fn signer<'keys>(keys: &'keys [synthetic_key_t], kid: &[u8; 32]) -> &'keys synthetic_key_t {
    keys.iter()
        .find(|key| key.key_id == *kid)
        .unwrap_or_else(|| panic!("no synthetic key for kid {}", hex_encode(kid)))
}

#[test]
fn synthetic_keys_and_constants_match() {
    let document = vectors();
    let keys = synthetic_keys(&document);
    assert_eq!(keys.len(), 10);
    for key in &keys {
        let signing = SigningKey::from_bytes(&key.seed);
        assert_eq!(
            signing.verifying_key().to_bytes(),
            key.public_key,
            "{}",
            key.label
        );
        assert_eq!(key_id(&key.public_key), key.key_id, "{}", key.label);
        assert!(
            cham3_contract_format::envelope::public_key_is_valid(&key.public_key),
            "{}",
            key.label
        );
    }
    let constants = document.get("constants").expect("constants");
    assert_eq!(
        hex_encode(&empty_state_hash()),
        constants.field("empty_state_hash")
    );
    for kind in object_kind_t::All {
        assert_eq!(
            constants
                .get("typ")
                .and_then(|typ| typ.get(kind.name()))
                .and_then(json_t::as_text),
            Some(kind.typ())
        );
        assert_eq!(
            constants
                .get("external_aad")
                .and_then(|aad| aad.get(kind.name()))
                .and_then(json_t::as_text),
            Some(kind.external_aad())
        );
    }
    let limits = document.get("limits").expect("limits");
    assert_eq!(
        limits.get("max_envelope_bytes").and_then(json_t::as_number),
        i64::try_from(MaxEncodedBytes).ok()
    );
}

#[test]
fn valid_vectors_verify_and_reproduce_bytes() {
    let document = vectors();
    let keys = synthetic_keys(&document);
    let resolver = resolver_t::new(&keys);
    let valid = document.get("valid").map(json_t::items).unwrap_or_default();
    assert_eq!(valid.len(), 19);
    let mut largest = 0;
    for vector in valid {
        let name = vector.field("name");
        let kind = object_kind_t::from_name(vector.field("kind")).expect("kind");
        let envelope = hex_decode(vector.field("envelope"));
        let verified = verify_object(&envelope, kind, &resolver)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        // Object id (commit_id for commits) is SHA-256 of the complete envelope.
        assert_eq!(
            hex_encode(&verified.object_id),
            vector.field("object_id"),
            "{name}"
        );
        assert_eq!(verified.object_id, object_id(&envelope), "{name}");
        // Deterministic bytes: the parsed payload re-encodes to the signed bytes.
        assert_eq!(
            encode(&verified.payload.to_value()),
            verified.payload_bytes,
            "{name}"
        );
        // Deterministic signature: re-signing the rebuilt Sig_structure with the
        // synthetic key reproduces the envelope byte for byte.
        let protected = protected_header_bytes(&verified.kid, kind);
        let signing = SigningKey::from_bytes(&signer(&keys, &verified.kid).seed);
        let signature = signing
            .sign(&to_be_signed(kind, &protected, &verified.payload_bytes))
            .to_bytes();
        assert_eq!(
            assemble_envelope(&protected, &verified.payload_bytes, &signature),
            envelope,
            "{name}"
        );
        largest = largest.max(envelope.len());
    }
    assert_eq!(
        largest, MaxEncodedBytes,
        "the maximum-size commit vector is present"
    );
}

fn valid_envelope(document: &json_t, name: &str) -> Vec<u8> {
    let vector = document
        .get("valid")
        .map(json_t::items)
        .unwrap_or_default()
        .iter()
        .find(|vector| vector.field("name") == name)
        .unwrap_or_else(|| panic!("valid vector {name}"));
    hex_decode(vector.field("envelope"))
}

#[test]
fn protected_header_decode_precedes_other_element_kinds() {
    // Review M2-003-R01: layer 1 includes the protected-header decode, so its errors
    // precede layer-2 kind errors of the other envelope elements.
    let document = vectors();
    let resolver = resolver_t::new(&synthetic_keys(&document));
    let genesis = valid_envelope(&document, "commit_genesis");
    let Ok(value_t::tag(_, content)) = decode(&genesis, layer_t::envelope_decode, true) else {
        panic!("genesis decodes");
    };
    let value_t::array(elements) = *content else {
        panic!("genesis is an array");
    };
    let [protected, _, payload, signature] =
        <[value_t; 4]>::try_from(elements).expect("four elements");
    let build = |elements: [value_t; 4]| {
        encode(&value_t::tag(
            SignedObjectTag,
            Box::new(value_t::array(elements.to_vec())),
        ))
    };
    let cases = [
        (
            [
                value_t::bytes(vec![0x18, 0x00]),
                value_t::boolean(false),
                payload.clone(),
                signature.clone(),
            ],
            "envelope_decode/non_minimal_encoding",
        ),
        (
            [
                value_t::bytes(vec![0xa3]),
                value_t::map(Vec::new()),
                value_t::text_of("p"),
                signature.clone(),
            ],
            "envelope_decode/truncated",
        ),
        (
            [
                value_t::bytes(vec![0xa0, 0x00]),
                value_t::map(Vec::new()),
                payload.clone(),
                value_t::unsigned(0),
            ],
            "envelope_decode/trailing_bytes",
        ),
        (
            [
                protected.clone(),
                value_t::boolean(false),
                payload.clone(),
                signature.clone(),
            ],
            "envelope/not_signed_object",
        ),
        (
            [
                protected.clone(),
                value_t::map(Vec::new()),
                value_t::text_of("p"),
                signature.clone(),
            ],
            "envelope/not_signed_object",
        ),
        (
            [
                value_t::text_of("p"),
                value_t::boolean(false),
                payload,
                signature,
            ],
            "envelope/not_signed_object",
        ),
    ];
    for (elements, expected) in cases {
        assert_eq!(
            outcome(&build(elements), object_kind_t::commit, &resolver),
            expected
        );
    }
}

#[test]
fn text_typ_other_than_expected_is_wrong_object_type() {
    // Owner erratum E2 (review M2-003-R03): any text typ other than the expected
    // kind's is wrong_object_type; a non-text typ is a malformed header.
    let document = vectors();
    let keys = synthetic_keys(&document);
    let resolver = resolver_t::new(&keys);
    let genesis = valid_envelope(&document, "commit_genesis");
    let verified = verify_object(&genesis, object_kind_t::commit, &resolver).expect("valid");
    let signing = SigningKey::from_bytes(&signer(&keys, &verified.kid).seed);
    let cases = [
        (value_t::text_of("x"), "envelope/wrong_object_type"),
        (value_t::text_of(""), "envelope/wrong_object_type"),
        (
            value_t::text_of("application/cbor"),
            "envelope/wrong_object_type",
        ),
        (
            value_t::text_of("application/vnd.cham3.ecr-commit.v2+cose"),
            "envelope/wrong_object_type",
        ),
        (
            value_t::text_of(object_kind_t::key_certificate.typ()),
            "envelope/wrong_object_type",
        ),
        (
            value_t::bytes_of(object_kind_t::commit.typ().as_bytes()),
            "envelope/protected_header_invalid",
        ),
        (value_t::unsigned(16), "envelope/protected_header_invalid"),
        (value_t::text_of(object_kind_t::commit.typ()), "ok"),
    ];
    for (typ, expected) in cases {
        let protected = encode(&value_t::map(vec![
            (value_t::unsigned(1), value_t::negative(18)),
            (value_t::unsigned(4), value_t::bytes_of(&verified.kid)),
            (value_t::unsigned(16), typ.clone()),
        ]));
        let signature = signing
            .sign(&to_be_signed(
                object_kind_t::commit,
                &protected,
                &verified.payload_bytes,
            ))
            .to_bytes();
        let envelope = assemble_envelope(&protected, &verified.payload_bytes, &signature);
        assert_eq!(
            outcome(&envelope, object_kind_t::commit, &resolver),
            expected,
            "{typ:?}"
        );
    }
}

#[test]
fn every_kind_is_rejected_as_another_kind() {
    let document = vectors();
    let resolver = resolver_t::new(&synthetic_keys(&document));
    for vector in document.get("valid").map(json_t::items).unwrap_or_default() {
        let kind = object_kind_t::from_name(vector.field("kind")).expect("kind");
        let envelope = hex_decode(vector.field("envelope"));
        for other in object_kind_t::All
            .into_iter()
            .filter(|other| *other != kind)
        {
            assert_eq!(
                outcome(&envelope, other, &resolver),
                "envelope/wrong_object_type",
                "{}",
                vector.field("name")
            );
        }
    }
}

#[test]
fn invalid_vectors_report_the_published_layer_and_code() {
    let document = vectors();
    let resolver = resolver_t::new(&synthetic_keys(&document));
    let invalid = document
        .get("invalid")
        .map(json_t::items)
        .unwrap_or_default();
    assert_eq!(invalid.len(), 96);
    let mut layers_seen = std::collections::BTreeSet::new();
    for vector in invalid {
        let kind = object_kind_t::from_name(vector.field("kind")).expect("kind");
        let expected = format!("{}/{}", vector.field("layer"), vector.field("error"));
        assert_eq!(
            outcome(&hex_decode(vector.field("envelope")), kind, &resolver),
            expected,
            "{}",
            vector.field("name")
        );
        layers_seen.insert(vector.field("layer").to_owned());
    }
    assert_eq!(
        layers_seen.len(),
        LocalLayers.len(),
        "the invalid vectors cover every local layer"
    );
}

#[test]
fn admission_vectors_pass_layers_one_to_five() {
    // Separation of facts (wire format §1): these objects fail only at admission.
    let document = vectors();
    let resolver = resolver_t::new(&synthetic_keys(&document));
    let admission = document
        .get("admission_invalid")
        .map(json_t::items)
        .unwrap_or_default();
    assert_eq!(admission.len(), 18);
    for vector in admission {
        assert_eq!(vector.field("layer"), "admission");
        assert_eq!(
            outcome(
                &hex_decode(vector.field("envelope")),
                object_kind_t::commit,
                &resolver
            ),
            "ok",
            "{}",
            vector.field("name")
        );
    }
}

#[test]
fn journal_scenario_objects_match_their_local_layers() {
    let document = vectors();
    let resolver = resolver_t::new(&synthetic_keys(&document));
    let mut checked = 0;
    for scenario in document
        .get("journal_scenarios")
        .map(json_t::items)
        .unwrap_or_default()
    {
        for step in scenario.get("steps").map(json_t::items).unwrap_or_default() {
            let (data, kind) = if let Some(commit) = step.get("commit").and_then(json_t::as_text) {
                (hex_decode(commit), object_kind_t::commit)
            } else if let Some(identity) = step.get("identity").and_then(json_t::as_text) {
                (
                    hex_decode(identity),
                    object_kind_t::from_name(step.field("kind")).expect("kind"),
                )
            } else {
                continue;
            };
            let expect = step.get("expect").expect("expect");
            let expected = match expect.get("layer").and_then(json_t::as_text) {
                Some(layer) if LocalLayers.contains(&layer) => {
                    format!("{layer}/{}", expect.field("error"))
                }
                // Admitted, or rejected only at key authority, admission or the IPC
                // boundary: layers 1–5 pass.
                _ => "ok".to_owned(),
            };
            assert_eq!(
                outcome(&data, kind, &resolver),
                expected,
                "scenario {}",
                scenario.field("name")
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 134, "every signed journal object was checked");
}

#[test]
fn every_single_byte_change_is_rejected() {
    // Hostile input: no one-byte change, truncation or extension of a valid object
    // verifies (the signature covers protected header and payload; everything else
    // is fixed by the deterministic profile).
    let document = vectors();
    let resolver = resolver_t::new(&synthetic_keys(&document));
    let mut mutations = 0usize;
    for vector in document.get("valid").map(json_t::items).unwrap_or_default() {
        let kind = object_kind_t::from_name(vector.field("kind")).expect("kind");
        let envelope = hex_decode(vector.field("envelope"));
        // Every position of ordinary objects; a stride over the maximum-size one.
        let stride = if envelope.len() > 4_096 { 97 } else { 1 };
        for position in (0..envelope.len()).step_by(stride) {
            for mask in [0x01u8, 0x80] {
                let mut changed = envelope.clone();
                changed[position] ^= mask;
                assert!(
                    verify_object(&changed, kind, &resolver).is_err(),
                    "{} byte {position} mask {mask:#x}",
                    vector.field("name")
                );
                mutations += 1;
            }
        }
        for length in (0..envelope.len()).step_by(stride) {
            assert!(
                verify_object(&envelope[..length], kind, &resolver).is_err(),
                "{} truncated to {length}",
                vector.field("name")
            );
        }
        let mut extended = envelope.clone();
        extended.push(0x00);
        let expected = if extended.len() > MaxEncodedBytes {
            "envelope_decode/input_too_large"
        } else {
            "envelope_decode/trailing_bytes"
        };
        assert_eq!(outcome(&extended, kind, &resolver), expected);
    }
    assert!(mutations > 20_000, "{mutations} mutations");
}

#[test]
fn seeded_random_inputs_are_rejected_without_panicking() {
    let document = vectors();
    let resolver = resolver_t::new(&synthetic_keys(&document));
    let valid: Vec<Vec<u8>> = document
        .get("valid")
        .map(json_t::items)
        .unwrap_or_default()
        .iter()
        .map(|vector| hex_decode(vector.field("envelope")))
        .collect();
    let mut generator = support::generator_t::new(0x00c4_a3e3_0003);
    for _ in 0..20_000 {
        // Random bytes, often starting like a signed object, or a valid object with
        // a random window overwritten.
        let input = if generator.below(2) == 0 {
            let length = generator.below(96);
            let mut bytes: Vec<u8> = (0..length)
                .map(|_| generator.next_u64().to_le_bytes()[0])
                .collect();
            if !bytes.is_empty() && generator.below(2) == 0 {
                bytes[0] = 0xd2;
            }
            bytes
        } else {
            let mut bytes = valid[generator.below(valid.len())].clone();
            let start = generator.below(bytes.len());
            let width = 1 + generator.below(8);
            for index in start..(start + width).min(bytes.len()) {
                bytes[index] = generator.next_u64().to_le_bytes()[0];
            }
            bytes
        };
        for kind in object_kind_t::All {
            if let Ok(verified) = verify_object(&input, kind, &resolver) {
                // Only an unchanged valid object may verify.
                assert!(
                    valid.contains(&input),
                    "random input verified as {}",
                    verified.kind.name()
                );
            }
        }
        let _ = decode(&input, layer_t::payload_decode, false);
    }
}

#[test]
fn deterministic_signature_is_independent_of_oracle_signing() {
    // The oracle signs with libsodium; ed25519-dalek reproduces the same bytes for
    // every valid object (RFC 8032 deterministic signatures).
    let document = vectors();
    let keys = synthetic_keys(&document);
    let resolver = resolver_t::new(&keys);
    for vector in document.get("valid").map(json_t::items).unwrap_or_default() {
        let kind = object_kind_t::from_name(vector.field("kind")).expect("kind");
        let envelope = hex_decode(vector.field("envelope"));
        let verified = verify_object(&envelope, kind, &resolver).expect("valid");
        let signature: [u8; 64] = fixed(&hex_encode(&envelope[envelope.len() - 64..]));
        let protected = protected_header_bytes(&verified.kid, kind);
        let message = to_be_signed(kind, &protected, &verified.payload_bytes);
        assert_eq!(
            SigningKey::from_bytes(&signer(&keys, &verified.kid).seed)
                .sign(&message)
                .to_bytes(),
            signature
        );
    }
}
