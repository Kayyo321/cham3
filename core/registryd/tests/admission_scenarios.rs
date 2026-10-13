//! Layer-7 differential against the independent oracle's published vectors.
//!
//! - Every `key_authority: false` journal scenario of
//!   tests/m2/vectors/ecr_v1/vectors.json is replayed step by step through layers 1–5
//!   (`cham3_contract_format`) and the Rust lineage index. Each commit, each lifecycle
//!   record and each standing check after a step is compared with the oracle's
//!   expectation. (The `key_authority: true` scenarios are the recovery ceremony's,
//!   M2-006; identityd's layer-6 differential replays them.)
//! - Every admission-invalid vector is checked after admitting its lineage prefix.
//! - The published lineage is admitted in full.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::as_conversions
)]

#[path = "../../contract_format/tests/support/mod.rs"]
mod support;

use std::collections::BTreeMap;

use cham3_contract_format::{object_kind_t, payload_t, verify_object};
use cham3_identityd::outcome::service_error_t;
use cham3_registryd::lineage::{lineage_index_t, standing_t};
use cham3_registryd::record::lifecycle_record_t;

use support::{fixed, hex_decode, json_t, resolver_t, synthetic_keys, vectors};

type outcome_t = Option<(String, String)>;

fn expected_of(step: &json_t) -> outcome_t {
    match step.get("expect") {
        Some(json_t::text(text)) if text == "admitted" => None,
        Some(expect) => Some((
            expect.field("layer").to_owned(),
            expect.field("error").to_owned(),
        )),
        None => panic!("step without expect"),
    }
}

fn outcome_of(error: &service_error_t) -> outcome_t {
    Some((error.layer().to_owned(), error.code().to_owned()))
}

/// Layers 1–5, then layer 7; applies the commit when admitted.
fn admit(index: &mut lineage_index_t, resolver: &resolver_t, data: &[u8]) -> outcome_t {
    let verified = match verify_object(data, object_kind_t::commit, resolver) {
        Ok(verified) => verified,
        Err(error) => return outcome_of(&service_error_t::format(error)),
    };
    let payload_t::commit(payload) = verified.payload else {
        panic!("commit payload expected");
    };
    match index.check_commit(&verified.object_id, &payload) {
        Ok(admission) => {
            index.apply_commit(&admission, payload, data.to_vec(), 0);
            None
        }
        Err(error) => outcome_of(&error),
    }
}

fn lifecycle_record(record: &json_t) -> (lifecycle_record_t, String) {
    let time = record
        .get("time")
        .and_then(json_t::as_text)
        .unwrap_or("")
        .to_owned();
    let record = match record.field("record") {
        "expired" => lifecycle_record_t::expired {
            grant_id: fixed(record.field("grant_id")),
        },
        "session_ended" => lifecycle_record_t::session_ended {
            session_id: fixed(record.field("session_id")),
        },
        "restriction" => lifecycle_record_t::restriction {
            grant_ids: record
                .get("grant_ids")
                .map(json_t::items)
                .unwrap_or_default()
                .iter()
                .map(|item| fixed(item.as_text().expect("grant id")))
                .collect(),
        },
        other => panic!("record kind {other} is outside M2-005"),
    };
    (record, time)
}

fn standing_name(standing: standing_t) -> &'static str {
    match standing {
        standing_t::current => "current",
        standing_t::ended(_) => "ended",
        standing_t::inactive => "inactive",
        standing_t::unknown => "unknown",
    }
}

#[derive(Default)]
struct totals_t {
    scenarios: usize,
    steps: usize,
    comparisons: usize,
    disagreements: Vec<String>,
}

fn run_scenarios() -> totals_t {
    let document = vectors();
    let keys = synthetic_keys(&document);
    let resolver = resolver_t::new(&keys);
    let mut totals = totals_t::default();
    for scenario in document
        .get("journal_scenarios")
        .map(json_t::items)
        .unwrap_or_default()
    {
        if scenario.get("key_authority") == Some(&json_t::boolean(true)) {
            continue;
        }
        totals.scenarios += 1;
        let name = scenario.field("name");
        let mut index = lineage_index_t::new();
        for (position, step) in scenario.items_of("steps").iter().enumerate() {
            totals.steps += 1;
            let expected = expected_of(step);
            let actual = if let Some(commit) = step.get("commit").and_then(json_t::as_text) {
                admit(&mut index, &resolver, &hex_decode(commit))
            } else if let Some(record) = step.get("record") {
                let (record, time) = lifecycle_record(record);
                match index.check_record(&record, &time) {
                    Ok(()) => {
                        index.apply_record(&record);
                        None
                    }
                    Err(error) => outcome_of(&error),
                }
            } else {
                panic!("{name} step {position}: unknown step kind");
            };
            totals.comparisons += 1;
            if actual != expected {
                totals.disagreements.push(format!(
                    "{name} step {position}: expected {expected:?}, got {actual:?}"
                ));
            }
            if let Some(json_t::object(checks)) = step.get("check") {
                for (standing, grant_ids) in checks {
                    for grant_id in grant_ids.items() {
                        let grant_id = fixed(grant_id.as_text().expect("grant id"));
                        totals.comparisons += 1;
                        let actual = standing_name(index.standing(&grant_id));
                        if actual != standing {
                            totals.disagreements.push(format!(
                                "{name} step {position}: grant expected {standing}, got {actual}"
                            ));
                        }
                    }
                }
            }
        }
    }
    totals
}

#[test]
fn admission_scenarios_match_the_oracle() {
    let totals = run_scenarios();
    println!(
        "admission scenarios {} steps {} comparisons {} disagreements {}",
        totals.scenarios,
        totals.steps,
        totals.comparisons,
        totals.disagreements.len()
    );
    assert!(
        totals.disagreements.is_empty(),
        "{:#?}",
        totals.disagreements
    );
    // Positive controls: the published counts (a driver that skipped steps or
    // scenarios would not reach them).
    assert_eq!(
        totals.scenarios, 10,
        "admission scenarios in the published vectors"
    );
    assert_eq!(totals.steps, 53, "steps of the admission scenarios");
    assert!(totals.comparisons > totals.steps);
}

#[test]
fn every_admission_code_of_the_scenarios_is_reproduced() {
    // Positive control for the comparison above: the scenarios reach these codes.
    let document = vectors();
    let mut codes = BTreeMap::new();
    for scenario in document
        .get("journal_scenarios")
        .map(json_t::items)
        .unwrap_or_default()
    {
        if scenario.get("key_authority") == Some(&json_t::boolean(true)) {
            continue;
        }
        for step in scenario.items_of("steps") {
            if let Some((layer, code)) = expected_of(&step) {
                *codes.entry(format!("{layer}/{code}")).or_insert(0usize) += 1;
            }
        }
    }
    let expected = [
        "admission/branch_rule",
        "admission/lifecycle_rule",
        "admission/network_grant_conflict",
        "admission/restore_rule",
        "admission/stale_head",
        "intrinsic/network_grant_rule",
    ];
    assert_eq!(
        codes.keys().map(String::as_str).collect::<Vec<_>>(),
        expected.to_vec()
    );
}

fn valid_envelopes(document: &json_t) -> BTreeMap<String, Vec<u8>> {
    document
        .get("valid")
        .map(json_t::items)
        .unwrap_or_default()
        .iter()
        .map(|vector| {
            (
                vector.field("name").to_owned(),
                hex_decode(vector.field("envelope")),
            )
        })
        .collect()
}

fn lineage_envelopes(document: &json_t) -> Vec<Vec<u8>> {
    let valid = valid_envelopes(document);
    let lineage = document.get("lineage").expect("lineage");
    lineage
        .items_of("commits")
        .iter()
        .map(|name| valid[name.as_text().expect("commit name")].clone())
        .collect()
}

#[test]
fn the_published_lineage_is_admitted() {
    let document = vectors();
    let resolver = resolver_t::new(&synthetic_keys(&document));
    let envelopes = lineage_envelopes(&document);
    assert_eq!(envelopes.len(), 8);
    let mut index = lineage_index_t::new();
    for (position, envelope) in envelopes.iter().enumerate() {
        assert_eq!(
            admit(&mut index, &resolver, envelope),
            None,
            "commit {position}"
        );
    }
    // Replaying any admitted commit again is refused (old-byte replay).
    for envelope in &envelopes {
        assert!(admit(&mut index, &resolver, envelope).is_some());
    }
}

#[test]
fn admission_invalid_vectors_match_the_oracle() {
    let document = vectors();
    let resolver = resolver_t::new(&synthetic_keys(&document));
    let envelopes = lineage_envelopes(&document);
    let vectors = document
        .get("admission_invalid")
        .map(json_t::items)
        .unwrap_or_default();
    assert_eq!(vectors.len(), 18, "admission-invalid vectors");
    for vector in vectors {
        let name = vector.field("name");
        let prefix = vector
            .get("prefix")
            .and_then(json_t::as_number)
            .expect("prefix") as usize;
        let mut index = lineage_index_t::new();
        for envelope in &envelopes[..prefix] {
            assert_eq!(
                admit(&mut index, &resolver, envelope),
                None,
                "{name} prefix"
            );
        }
        let actual = admit(&mut index, &resolver, &hex_decode(vector.field("envelope")));
        assert_eq!(
            actual,
            Some((
                vector.field("layer").to_owned(),
                vector.field("error").to_owned()
            )),
            "{name}"
        );
    }
}

trait items_of_t {
    fn items_of(&self, key: &str) -> Vec<json_t>;
}

impl items_of_t for json_t {
    fn items_of(&self, key: &str) -> Vec<json_t> {
        self.get(key)
            .map(json_t::items)
            .unwrap_or_default()
            .to_vec()
    }
}
