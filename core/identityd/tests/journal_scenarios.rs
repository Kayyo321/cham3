//! Layer-6 differential against the independent oracle's published journal scenarios.
//!
//! Every `key_authority: true` scenario of tests/m2/vectors/ecr_v1/vectors.json is
//! replayed step by step. Identity objects, the registry records that change key
//! authority and the ceremony's key naming go through the Rust identity index; commits
//! go through layers 1–5 (`cham3_contract_format`) and then layer 6. The driver itself
//! models only what registryd adds around identityd (caller roles of registry
//! operations, closed arguments and the unanchored-tail rule of ADR 0006 §4), exactly
//! as the oracle's `registry_service_t` does. Admission (layer 7) is registryd's
//! (M2-005); a step the oracle rejects at admission must pass layer 6 here.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::as_conversions
)]

#[path = "../../contract_format/tests/support/mod.rs"]
mod support;

use std::collections::{BTreeMap, BTreeSet};

use cham3_contract_format::{object_kind_t, payload_t, verify_object};
use cham3_identityd::directory::{identity_directory_t, journal_record_t};
use cham3_identityd::index::{
    ceremony_mode_t, naming_effect_t, naming_t, record_role_t, user_record_t,
};
use cham3_identityd::outcome::service_error_t;

use support::{fixed, hex_decode, hex_encode, json_t, resolver_t, synthetic_keys, vectors};

/// The scenarios' user.
const ScenarioUid: u32 = 1001;

type outcome_t = Option<(String, String)>;

fn outcome_of(result: Result<(), service_error_t>) -> outcome_t {
    result
        .err()
        .map(|error| (error.layer().to_owned(), error.code().to_owned()))
}

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

fn kind_of(name: &str) -> object_kind_t {
    object_kind_t::from_name(name).unwrap_or_else(|| panic!("unknown kind {name}"))
}

fn number(value: &json_t, key: &str) -> i64 {
    value
        .get(key)
        .and_then(json_t::as_number)
        .unwrap_or_else(|| panic!("missing number {key}"))
}

fn user_record(record: &json_t) -> Option<user_record_t> {
    let role = |text: &str| match text {
        "admin" => record_role_t::admin,
        "trusted_client" => record_role_t::trusted_client,
        other => panic!("unknown role {other}"),
    };
    Some(match record.field("record") {
        "containment" => user_record_t::containment,
        "quarantine" => user_record_t::quarantine,
        "admin_recover" => user_record_t::admin_recover {
            role: role(record.field("role")),
        },
        "recovery_opened" => user_record_t::recovery_opened {
            recovery_id: fixed(record.field("recovery_id")),
            key_epoch: u64::try_from(number(record, "key_epoch")).expect("epoch"),
            mode: match record.field("mode") {
                "c1" => ceremony_mode_t::c1,
                "c2" => ceremony_mode_t::c2,
                other => panic!("unknown mode {other}"),
            },
            role: role(record.field("role")),
        },
        "recovery_closed" => user_record_t::recovery_closed {
            recovery_id: fixed(record.field("recovery_id")),
        },
        "deletion_pending" => user_record_t::deletion_pending,
        // Grant lifecycle records are registryd's; they do not change key authority.
        "expired" | "session_ended" | "restriction" => return None,
        other => panic!("unknown record kind {other}"),
    })
}

fn argument_names(args: &json_t) -> BTreeSet<String> {
    match args {
        json_t::object(members) => members.keys().cloned().collect(),
        _ => panic!("arguments are not a map"),
    }
}

fn names(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|name| (*name).to_owned()).collect()
}

fn ipc_error(code: &str) -> outcome_t {
    Some(("ipc".to_owned(), code.to_owned()))
}

/// One scenario run: the Rust identity index plus the registry-side driver state.
struct run_t<'run> {
    resolver: &'run resolver_t,
    directory: identity_directory_t,
    sequence: u64,
    /// A durable `recovery_keys` record whose anchor update failed (pending tail).
    pending: Option<naming_t>,
    compared: usize,
    failures: Vec<String>,
}

impl run_t<'_> {
    fn next_sequence(&mut self) -> u64 {
        self.sequence += 1;
        self.sequence
    }

    /// The pending tail takes effect once its anchor update succeeds; with the
    /// step's anchor fault it blocks the step (`storage_unavailable`).
    fn settle(&mut self, anchor_fault: bool) -> Result<(), outcome_t> {
        let Some(naming) = self.pending else {
            return Ok(());
        };
        if anchor_fault {
            return Err(ipc_error("storage_unavailable"));
        }
        self.pending = None;
        let sequence = self.next_sequence();
        self.directory
            .apply_journal_record(
                ScenarioUid,
                &journal_record_t::recovery_keys(naming),
                sequence,
            )
            .expect("an anchored naming record applies");
        Ok(())
    }

    fn identity(&mut self, kind: object_kind_t, data: &[u8]) -> outcome_t {
        let verified = match self.directory.verify_identity_object(kind, data) {
            Ok(verified) => verified,
            Err(error) => return outcome_of(Err(error)),
        };
        if let Err(error) = self.directory.check_identity_object(ScenarioUid, &verified) {
            return outcome_of(Err(error));
        }
        let sequence = self.next_sequence();
        outcome_of(
            self.directory
                .apply_identity_object(ScenarioUid, &verified, sequence),
        )
    }

    fn commit(&mut self, data: &[u8]) -> outcome_t {
        let verified = match verify_object(data, object_kind_t::commit, self.resolver) {
            Ok(verified) => verified,
            Err(error) => return outcome_of(Err(service_error_t::format(error))),
        };
        let payload_t::commit(payload) = &verified.payload else {
            panic!("commit payload expected");
        };
        outcome_of(self.directory.check_commit(payload))
    }

    fn record(&mut self, record: &user_record_t) -> outcome_t {
        let Some(user) = self.directory.user(ScenarioUid) else {
            return Some(("key_authority".to_owned(), "key_not_certified".to_owned()));
        };
        if let Err(error) = user.check_record(record) {
            return outcome_of(Err(error));
        }
        let sequence = self.next_sequence();
        outcome_of(self.directory.apply_journal_record(
            ScenarioUid,
            &journal_record_t::user(record.clone()),
            sequence,
        ))
    }

    /// A role-checked registry operation; returns the outcome and the result map.
    fn call(&mut self, call: &json_t, anchor_fault: bool) -> (outcome_t, Option<json_t>) {
        let caller = call.field("caller");
        let operation = call.field("operation");
        let args = call.get("args").expect("args");
        let (roles, arguments): (&[&str], &[&str]) = match operation {
            "append_identity" => (&["service:identityd"], &["kind", "object"]),
            "name_recovery_keys" => (
                &["service:vaultd"],
                &[
                    "owner_uid",
                    "persistent_key_id",
                    "recovery_id",
                    "temporary_key_id",
                ],
            ),
            "recovery_status" => (
                &["trusted_client", "admin", "service:vaultd"],
                &["owner_uid", "recovery_id"],
            ),
            _ => return (ipc_error("unknown_operation"), None),
        };
        if !roles.contains(&caller) {
            return (ipc_error("role_denied"), None);
        }
        if argument_names(args) != names(arguments) {
            return (ipc_error("malformed_request"), None);
        }
        let owner_uid = args.get("owner_uid").and_then(json_t::as_number);
        if caller == "trusted_client"
            && owner_uid != call.get("caller_uid").and_then(json_t::as_number)
        {
            return (ipc_error("owner_mismatch"), None);
        }
        if let Err(outcome) = self.settle(anchor_fault) {
            return (outcome, None);
        }
        match operation {
            "append_identity" => match object_kind_t::from_name(args.field("kind")) {
                // append_identity accepts identity objects only (R14): no record kind,
                // in particular no recovery_keys record, enters through it.
                Some(kind) if kind != object_kind_t::commit => {
                    (self.identity(kind, &hex_decode(args.field("object"))), None)
                }
                _ => (ipc_error("malformed_request"), None),
            },
            "name_recovery_keys" => self.name_keys(args, anchor_fault),
            _ => self.recovery_status(args),
        }
    }

    fn name_keys(&mut self, args: &json_t, anchor_fault: bool) -> (outcome_t, Option<json_t>) {
        let recovery_rule = Some(("admission".to_owned(), "recovery_rule".to_owned()));
        let owner_uid = u32::try_from(number(args, "owner_uid")).expect("uid");
        let naming = naming_t {
            recovery_id: fixed(args.field("recovery_id")),
            persistent_key_id: fixed(args.field("persistent_key_id")),
            temporary_key_id: fixed(args.field("temporary_key_id")),
        };
        let Some(user) = self.directory.user(ScenarioUid) else {
            return (recovery_rule, None);
        };
        let result = |already_named: bool| {
            json_t::object(BTreeMap::from([
                ("already_named".to_owned(), json_t::boolean(already_named)),
                ("named".to_owned(), json_t::boolean(true)),
            ]))
        };
        match user.check_naming(owner_uid, &naming) {
            Err(error) => (outcome_of(Err(error)), None),
            Ok(naming_effect_t::already_named) => (None, Some(result(true))),
            Ok(naming_effect_t::name) if anchor_fault => {
                self.pending = Some(naming);
                (ipc_error("storage_unavailable"), None)
            }
            Ok(naming_effect_t::name) => {
                let sequence = self.next_sequence();
                let applied = self.directory.apply_journal_record(
                    ScenarioUid,
                    &journal_record_t::recovery_keys(naming),
                    sequence,
                );
                (outcome_of(applied), Some(result(false)))
            }
        }
    }

    fn recovery_status(&mut self, args: &json_t) -> (outcome_t, Option<json_t>) {
        let owner_uid = u32::try_from(number(args, "owner_uid")).expect("uid");
        let user = match self.directory.user(ScenarioUid) {
            Some(user) if user.uid() == owner_uid => user,
            _ => {
                return (
                    Some(("admission".to_owned(), "recovery_rule".to_owned())),
                    None,
                );
            }
        };
        let status = user.recovery_status(&fixed(args.field("recovery_id")));
        let key = |key_id: Option<[u8; 32]>| match key_id {
            Some(key_id) => json_t::text(hex_encode(&key_id)),
            None => json_t::null,
        };
        let result = json_t::object(BTreeMap::from([
            (
                "ceremony_open".to_owned(),
                json_t::boolean(status.ceremony_open),
            ),
            (
                "persistent_key_id".to_owned(),
                key(status.named.map(|named| named.0)),
            ),
            (
                "temporary_key_id".to_owned(),
                key(status.named.map(|named| named.1)),
            ),
            (
                "user_state".to_owned(),
                json_t::text(status.user_state.as_str().to_owned()),
            ),
        ]));
        (None, Some(result))
    }

    fn check_keys(&mut self, label: &str, expected: &json_t) {
        let (named, min_key_epoch, retired) = self
            .directory
            .user(ScenarioUid)
            .map_or((false, 0, Vec::new()), |user| user.key_observation());
        let retired: Vec<String> = retired.iter().map(|key_id| hex_encode(key_id)).collect();
        let want_retired: Vec<String> = expected
            .get("retired")
            .map(json_t::items)
            .unwrap_or_default()
            .iter()
            .map(|item| item.as_text().expect("retired id").to_owned())
            .collect();
        let want_named = expected.get("named") == Some(&json_t::boolean(true));
        let want_epoch = u64::try_from(number(expected, "min_key_epoch")).expect("epoch");
        self.compared += 1;
        if (named, min_key_epoch, &retired) != (want_named, want_epoch, &want_retired) {
            self.failures.push(format!(
                "{label}: key state named={named} min_key_epoch={min_key_epoch} retired={retired:?}, expected named={want_named} min_key_epoch={want_epoch} retired={want_retired:?}"
            ));
        }
    }
}

/// Layer-6 projection of the oracle's expectation for a commit: admission and
/// registry-interface refusals happen after layer 6, so layer 6 must pass.
fn layer_six_projection(expected: &outcome_t) -> outcome_t {
    match expected {
        Some((layer, _)) if layer == "admission" || layer == "ipc" => None,
        other => other.clone(),
    }
}

struct totals_t {
    scenarios: usize,
    compared: usize,
    failures: Vec<String>,
}

fn run_scenarios() -> totals_t {
    let document = vectors();
    let keys = synthetic_keys(&document);
    let resolver = resolver_t::new(&keys);
    let valid: BTreeMap<String, (object_kind_t, Vec<u8>)> = document
        .get("valid")
        .map(json_t::items)
        .unwrap_or_default()
        .iter()
        .map(|item| {
            (
                item.field("name").to_owned(),
                (
                    kind_of(item.field("kind")),
                    hex_decode(item.field("envelope")),
                ),
            )
        })
        .collect();
    let mut totals = totals_t {
        scenarios: 0,
        compared: 0,
        failures: Vec::new(),
    };
    for scenario in document
        .get("journal_scenarios")
        .map(json_t::items)
        .unwrap_or_default()
    {
        if scenario.get("key_authority") != Some(&json_t::boolean(true)) {
            continue;
        }
        totals.scenarios += 1;
        let name = scenario.field("name");
        let mut run = run_t {
            resolver: &resolver,
            directory: identity_directory_t::new(),
            sequence: 0,
            pending: None,
            compared: 0,
            failures: Vec::new(),
        };
        for (index, step) in scenario.items_of("steps").iter().enumerate() {
            let label = format!("{name} step {index}");
            let expected = expected_of(step);
            let fault = step.get("anchor_fault") == Some(&json_t::boolean(true));
            let mut result = None;
            let observed = if let Some(valid_name) = step.get("valid").and_then(json_t::as_text) {
                let (kind, data) = valid[valid_name].clone();
                match run.settle(fault) {
                    Err(outcome) => outcome,
                    Ok(()) if kind == object_kind_t::commit => {
                        let outcome = run.commit(&data);
                        if layer_six_projection(&expected) != outcome {
                            run.failures.push(format!(
                                "{label}: commit layer 6 {outcome:?}, expected {expected:?}"
                            ));
                        }
                        run.compared += 1;
                        continue_after(&mut run, &label, step);
                        continue;
                    }
                    Ok(()) => run.identity(kind, &data),
                }
            } else if let Some(commit) = step.get("commit").and_then(json_t::as_text) {
                let outcome = match run.settle(fault) {
                    Err(outcome) => outcome,
                    Ok(()) => run.commit(&hex_decode(commit)),
                };
                let want = if outcome == ipc_error("storage_unavailable") {
                    expected.clone()
                } else {
                    layer_six_projection(&expected)
                };
                if want != outcome {
                    run.failures.push(format!(
                        "{label}: commit layer 6 {outcome:?}, expected {expected:?}"
                    ));
                }
                run.compared += 1;
                continue_after(&mut run, &label, step);
                continue;
            } else if let Some(identity) = step.get("identity").and_then(json_t::as_text) {
                match run.settle(fault) {
                    Err(outcome) => outcome,
                    Ok(()) => run.identity(kind_of(step.field("kind")), &hex_decode(identity)),
                }
            } else if let Some(record) = step.get("record") {
                match run.settle(fault) {
                    Err(outcome) => outcome,
                    Ok(()) => match user_record(record) {
                        Some(record) => run.record(&record),
                        // Registry-only record: nothing for layer 6 to compare.
                        None => {
                            continue_after(&mut run, &label, step);
                            continue;
                        }
                    },
                }
            } else if let Some(call) = step.get("call") {
                let (outcome, call_result) = run.call(call, fault);
                result = call_result;
                outcome
            } else {
                run.settle(false).expect("restart rolls the tail forward");
                None
            };
            run.compared += 1;
            if observed != expected {
                run.failures
                    .push(format!("{label}: {observed:?}, expected {expected:?}"));
            }
            if let Some(want) = step.get("result") {
                // append_identity results carry registry journal sequences, which
                // count commits too; only the identity-index results are compared.
                let operation = step.get("call").map(|call| call.field("operation"));
                if operation != Some("append_identity") {
                    run.compared += 1;
                    if result.as_ref() != Some(want) {
                        run.failures
                            .push(format!("{label}: result {result:?}, expected {want:?}"));
                    }
                }
            }
            continue_after(&mut run, &label, step);
        }
        totals.compared += run.compared;
        totals.failures.extend(run.failures);
    }
    totals
}

fn continue_after(run: &mut run_t<'_>, label: &str, step: &json_t) {
    if let Some(keys) = step.get("keys") {
        run.check_keys(label, keys);
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

#[test]
fn key_authority_scenarios_match_the_oracle() {
    let totals = run_scenarios();
    println!(
        "key-authority scenarios {} comparisons {} disagreements {}",
        totals.scenarios,
        totals.compared,
        totals.failures.len()
    );
    assert_eq!(
        totals.scenarios, 12,
        "key-authority scenarios in the published vectors"
    );
    assert!(
        totals.compared >= 300,
        "only {} comparisons",
        totals.compared
    );
    assert!(
        totals.failures.is_empty(),
        "{} disagreements:\n{}",
        totals.failures.len(),
        totals.failures.join("\n")
    );
}

#[test]
fn every_layer_six_code_in_the_scenarios_is_reproduced() {
    // Positive control: the scenarios exercise these layer-6 codes, so a driver that
    // compared nothing could not pass the test above.
    let document = vectors();
    let mut codes = BTreeSet::new();
    for scenario in document
        .get("journal_scenarios")
        .map(json_t::items)
        .unwrap_or_default()
    {
        for step in scenario.items_of("steps") {
            if let Some((layer, code)) = expected_of(&step) {
                if layer == "key_authority" {
                    codes.insert(code);
                }
            }
        }
    }
    let expected: BTreeSet<String> = [
        "contained",
        "key_epoch_stale",
        "key_invalidated",
        "key_retired",
        "quarantined",
        "recovery_key_required",
        "user_deleted",
    ]
    .iter()
    .map(|code| (*code).to_owned())
    .collect();
    assert_eq!(codes, expected);
}
