//! A03 field-level controls: each valid object with one field removed, retyped or
//! joined by an unknown field, re-signed with its synthetic key so the change is
//! authenticated, must fail at the schema layer with the specified code; selected
//! semantic edits must fail at the intrinsic layer.

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

use cham3_contract_format::dcbor::{decode, encode, value_t};
use cham3_contract_format::envelope::{assemble_envelope, protected_header_bytes, to_be_signed};
use cham3_contract_format::{layer_t, object_kind_t, verify_object};
use ed25519_dalek::{Signer, SigningKey};

use support::{hex_decode, json_t, resolver_t, synthetic_key_t, synthetic_keys, vectors};

struct fixture_t {
    keys: Vec<synthetic_key_t>,
    resolver: resolver_t,
    /// (name, kind, kid, payload value)
    objects: Vec<(String, object_kind_t, [u8; 32], value_t)>,
}

fn fixture() -> fixture_t {
    let document = vectors();
    let keys = synthetic_keys(&document);
    let resolver = resolver_t::new(&keys);
    let mut objects = Vec::new();
    for vector in document.get("valid").map(json_t::items).unwrap_or_default() {
        let kind = object_kind_t::from_name(vector.field("kind")).expect("kind");
        let envelope = hex_decode(vector.field("envelope"));
        if envelope.len() > 4_096 {
            continue;
        }
        let verified = verify_object(&envelope, kind, &resolver).expect("valid vector");
        let value =
            decode(&verified.payload_bytes, layer_t::payload_decode, false).expect("payload");
        objects.push((vector.field("name").to_owned(), kind, verified.kid, value));
    }
    fixture_t {
        keys,
        resolver,
        objects,
    }
}

impl fixture_t {
    fn sign(&self, kind: object_kind_t, kid: &[u8; 32], payload: &value_t) -> Vec<u8> {
        let key = self
            .keys
            .iter()
            .find(|key| key.key_id == *kid)
            .expect("synthetic key");
        let protected = protected_header_bytes(kid, kind);
        let payload = encode(payload);
        let signature = SigningKey::from_bytes(&key.seed)
            .sign(&to_be_signed(kind, &protected, &payload))
            .to_bytes();
        assemble_envelope(&protected, &payload, &signature)
    }

    fn outcome(&self, kind: object_kind_t, kid: &[u8; 32], payload: &value_t) -> String {
        match verify_object(&self.sign(kind, kid, payload), kind, &self.resolver) {
            Ok(_) => "ok".to_owned(),
            Err(error) => error.to_string(),
        }
    }
}

fn pairs(value: &value_t) -> Vec<(value_t, value_t)> {
    match value {
        value_t::map(pairs) => pairs.clone(),
        _ => panic!("payload is not a map"),
    }
}

fn with_pairs(pairs: Vec<(value_t, value_t)>) -> value_t {
    value_t::map(pairs)
}

#[test]
fn unchanged_resigned_payloads_verify() {
    let fixture = fixture();
    assert_eq!(fixture.objects.len(), 18);
    for (name, kind, kid, payload) in &fixture.objects {
        assert_eq!(fixture.outcome(*kind, kid, payload), "ok", "{name}");
    }
}

#[test]
fn every_removed_required_field_is_missing_field() {
    let fixture = fixture();
    let mut checked = 0;
    for (name, kind, kid, payload) in &fixture.objects {
        let all = pairs(payload);
        for index in 0..all.len() {
            let mut fewer = all.clone();
            let (key, _) = fewer.remove(index);
            let value_t::text(key) = key else {
                panic!("text key")
            };
            // Optional fields: their absence is a schema-valid object that the
            // intrinsic layer (merge and restore) or key authority (certificates)
            // judges.
            let expected = match key.as_str() {
                "merge_base" | "restore_reference" => "intrinsic/commit_type_rule",
                "not_after" | "supersedes_key_id" => "ok",
                _ => {
                    checked += 1;
                    "schema/missing_field"
                }
            };
            assert_eq!(
                fixture.outcome(*kind, kid, &with_pairs(fewer)),
                expected,
                "{name} without {key}"
            );
        }
    }
    assert!(checked > 150, "{checked}");
}

#[test]
fn an_unknown_field_is_rejected_wherever_it_sorts() {
    let fixture = fixture();
    for (name, kind, kid, payload) in &fixture.objects {
        for unknown in ["a", "zzzzzzzzzzzzzzzzzzzz_unknown", "schema_versioo"] {
            let mut more = pairs(payload);
            more.push((value_t::text_of(unknown), value_t::unsigned(0)));
            assert_eq!(
                fixture.outcome(*kind, kid, &with_pairs(more)),
                "schema/unknown_field",
                "{name} with {unknown}"
            );
        }
    }
}

#[test]
fn every_retyped_field_is_wrong_type() {
    let fixture = fixture();
    for (name, kind, kid, payload) in &fixture.objects {
        let all = pairs(payload);
        for index in 0..all.len() {
            let mut retyped = all.clone();
            retyped[index].1 = value_t::boolean(true);
            assert_eq!(
                fixture.outcome(*kind, kid, &with_pairs(retyped)),
                "schema/wrong_type",
                "{name} field {index}"
            );
        }
    }
}

#[test]
fn integer_and_text_boundaries() {
    let fixture = fixture();
    let (_, kind, kid, payload) = fixture
        .objects
        .iter()
        .find(|(name, ..)| name == "commit_genesis")
        .expect("genesis vector")
        .clone();
    let replace =
        |path: &[&str], replacement: value_t| -> value_t { replaced(&payload, path, replacement) };
    let cases = [
        (
            vec!["actor", "uid"],
            value_t::unsigned(4_294_967_295),
            "schema/value_out_of_range",
        ),
        (
            vec!["actor", "uid"],
            value_t::negative(0),
            "schema/value_out_of_range",
        ),
        (
            vec!["schema_version"],
            value_t::unsigned(2),
            "schema/invalid_enum",
        ),
        (
            vec!["commit_type"],
            value_t::text_of("Genesis"),
            "schema/invalid_enum",
        ),
        (
            vec!["timestamp"],
            value_t::text_of("2026-02-29T12:00:00Z"),
            "schema/invalid_time",
        ),
        (
            vec!["timestamp"],
            value_t::text_of("2026-10-12T12:00:00Z\n"),
            "schema/invalid_time",
        ),
        (
            vec!["subject", "app_id"],
            value_t::text_of("abc\n"),
            "schema/invalid_text",
        ),
        (
            vec!["subject", "app_id"],
            value_t::text_of(""),
            "schema/value_out_of_range",
        ),
        (
            vec!["subject", "package_identity"],
            value_t::text_of("pkg name"),
            "schema/invalid_text",
        ),
        (
            vec!["message"],
            value_t::text_of(" padded"),
            "schema/invalid_text",
        ),
        (
            vec!["message"],
            value_t::text_of("rtl\u{202e}override"),
            "schema/invalid_text",
        ),
        (
            vec!["key_id"],
            value_t::bytes(vec![0; 31]),
            "schema/value_out_of_range",
        ),
        (
            vec!["lineage_id"],
            value_t::bytes(vec![0; 32]),
            "schema/lineage_mismatch",
        ),
        (
            vec!["key_id"],
            value_t::bytes(vec![0; 32]),
            "schema/key_id_mismatch",
        ),
        (
            vec!["actor", "uid"],
            value_t::unsigned(1_002),
            "schema/subject_actor_mismatch",
        ),
        (
            vec!["commit_type"],
            value_t::text_of("revision"),
            "intrinsic/commit_type_rule",
        ),
        (
            vec!["base_state_hash"],
            value_t::bytes(vec![0; 32]),
            "intrinsic/base_state_mismatch",
        ),
    ];
    for (path, replacement, expected) in cases {
        assert_eq!(
            fixture.outcome(kind, &kid, &replace(&path, replacement.clone())),
            expected,
            "{path:?} = {replacement:?}"
        );
    }
}

/// Replaces (or, at the last step, inserts) the item at `path`; numeric steps
/// index arrays.
fn replaced(value: &value_t, path: &[&str], replacement: value_t) -> value_t {
    match (value, path) {
        (_, []) => replacement,
        (value_t::array(items), [first, rest @ ..]) => {
            let index: usize = first.parse().expect("array index");
            value_t::array(
                items
                    .iter()
                    .enumerate()
                    .map(|(position, item)| {
                        if position == index {
                            replaced(item, rest, replacement.clone())
                        } else {
                            item.clone()
                        }
                    })
                    .collect(),
            )
        }
        (value_t::map(pairs), [first, rest @ ..]) => {
            let key = value_t::text_of(first);
            let mut pairs: Vec<(value_t, value_t)> = pairs
                .iter()
                .map(|(candidate, item)| {
                    if *candidate == key {
                        (candidate.clone(), replaced(item, rest, replacement.clone()))
                    } else {
                        (candidate.clone(), item.clone())
                    }
                })
                .collect();
            if !pairs.iter().any(|(candidate, _)| *candidate == key) {
                assert!(rest.is_empty(), "path {path:?} not found");
                pairs.push((key, replacement));
            }
            value_t::map(pairs)
        }
        _ => panic!("path {path:?} not found"),
    }
}

fn genesis_fixture(fixture: &fixture_t) -> (object_kind_t, [u8; 32], value_t) {
    let (_, kind, kid, payload) = fixture
        .objects
        .iter()
        .find(|(name, ..)| name == "commit_genesis")
        .expect("genesis vector")
        .clone();
    (kind, kid, payload)
}

#[test]
fn text_lists_ascend_in_encoding_order() {
    // Owner erratum E3 (review M2-003-R04): text lists ascend in deterministic
    // encoding order (shorter first), the map-key order of wire format §2.2.
    let fixture = fixture();
    let (kind, kid, payload) = genesis_fixture(&fixture);
    let texts =
        |items: &[&str]| value_t::array(items.iter().map(|item| value_t::text_of(item)).collect());
    let constraint_cases = [
        (texts(&["tor_only", "no_signing_keys"]), "ok"),
        (
            texts(&["no_signing_keys", "tor_only"]),
            "schema/list_not_strictly_ascending",
        ),
        (texts(&["b", "aa"]), "ok"),
        (texts(&["aa", "b"]), "schema/list_not_strictly_ascending"),
        (texts(&["aa", "ab"]), "ok"),
        (texts(&["ab", "aa"]), "schema/list_not_strictly_ascending"),
        (texts(&["aa", "aa"]), "schema/list_not_strictly_ascending"),
    ];
    for (list, expected) in constraint_cases {
        let edited = replaced(&payload, &["policy", "mandatory_constraints"], list.clone());
        assert_eq!(fixture.outcome(kind, &kid, &edited), expected, "{list:?}");
    }
    let operation_cases = [
        (texts(&["read", "write", "append"]), "ok"),
        (
            texts(&["append", "read", "write"]),
            "schema/list_not_strictly_ascending",
        ),
    ];
    for (list, expected) in operation_cases {
        let edited = replaced(
            &replaced(
                &payload,
                &["delta", "add_grants", "0", "resource", "operations"],
                list.clone(),
            ),
            &["result_state", "grants", "0", "resource", "operations"],
            list.clone(),
        );
        assert_eq!(fixture.outcome(kind, &kid, &edited), expected, "{list:?}");
    }
}

#[test]
fn expiration_equal_to_start_is_a_lifetime_rule_violation() {
    // Review M2-003-I01: wire format §6.3 requires expiration > effective_start.
    let fixture = fixture();
    let (kind, kid, payload) = genesis_fixture(&fixture);
    let start = payload
        .get("delta")
        .and_then(|delta| delta.get("add_grants"))
        .and_then(|grants| match grants {
            value_t::array(items) => items.first(),
            _ => None,
        })
        .and_then(|grant| grant.get("lifetime"))
        .and_then(|lifetime| lifetime.get("effective_start"))
        .cloned()
        .expect("effective_start");
    let with_expiration = |expiration: value_t| {
        replaced(
            &replaced(
                &payload,
                &["delta", "add_grants", "0", "lifetime", "expiration"],
                expiration.clone(),
            ),
            &["result_state", "grants", "0", "lifetime", "expiration"],
            expiration,
        )
    };
    assert_eq!(
        fixture.outcome(kind, &kid, &with_expiration(start)),
        "intrinsic/lifetime_rule"
    );
    assert_eq!(
        fixture.outcome(
            kind,
            &kid,
            &with_expiration(value_t::text_of("2026-10-12T12:00:01Z"))
        ),
        "ok"
    );
}
