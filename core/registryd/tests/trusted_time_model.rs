//! registryd's trusted-time clock against the reference model (ADR 0007 §5, §8
//! option B).
//!
//! tests/m2/vectors/trusted_time_v1/vectors.json is generated from
//! tests/m2/oracle/trusted_time_model.py by tests/m2/oracle/trusted_time_vectors.py:
//! the model's named reproductions and seeded random sequences of samples, adopted
//! offsets, floors and restarts in the same boot and in a new boot. Every case is
//! replayed through `cham3_registryd::clock::trusted_clock_t`, and every trusted and
//! observed value, floor and anomaly is compared with the model's.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::as_conversions
)]

#[path = "../../contract_format/tests/support/mod.rs"]
mod support;

use cham3_registryd::clock::{time_floor_t, time_sample_t, trusted_clock_t};

use support::{json_t, parse_json};

fn document() -> json_t {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/m2/vectors/trusted_time_v1/vectors.json"
    );
    let text = std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path}: {error}"));
    parse_json(&text).unwrap_or_else(|error| panic!("parse {path}: {error}"))
}

fn number(value: &json_t, key: &str) -> u64 {
    let number = value
        .get(key)
        .and_then(json_t::as_number)
        .unwrap_or_else(|| panic!("{key} missing"));
    u64::try_from(number).unwrap_or_else(|_| panic!("{key} is negative"))
}

fn signed(value: &json_t, key: &str) -> i64 {
    value
        .get(key)
        .and_then(json_t::as_number)
        .unwrap_or_else(|| panic!("{key} missing"))
}

fn floor_of(value: &json_t) -> time_floor_t {
    time_floor_t {
        boot_id: value.field("boot_id").to_owned(),
        boottime: number(value, "boottime"),
        trusted: number(value, "trusted"),
        observed: number(value, "observed"),
    }
}

fn sample(realtime: u64, boottime: u64) -> time_sample_t {
    time_sample_t { realtime, boottime }
}

#[derive(Default)]
struct totals_t {
    cases: usize,
    steps: usize,
    disagreements: Vec<String>,
}

fn replay() -> totals_t {
    let document = document();
    let mut totals = totals_t::default();
    for case in document.get("cases").map(json_t::items).unwrap_or_default() {
        totals.cases += 1;
        let name = case.field("name");
        let mut floors: Vec<time_floor_t> = case
            .get("floors")
            .map(json_t::items)
            .unwrap_or_default()
            .iter()
            .map(floor_of)
            .collect();
        let start = case.get("start").expect("start");
        let mut clock = trusted_clock_t::start(
            case.field("boot_id"),
            sample(number(start, "realtime"), number(start, "boottime")),
            &floors,
        )
        .expect("clock start");
        let mut found: Vec<String> = Vec::new();
        let disagree =
            |found: &mut Vec<String>, step: usize, what: &str, have: Option<u64>, want: u64| {
                if have != Some(want) {
                    found.push(format!("{name} step {step}: {what} {have:?}, model {want}"));
                }
            };
        let expected = case.get("expect_start").expect("expect_start");
        disagree(
            &mut found,
            0,
            "start trusted",
            clock.now(),
            number(expected, "trusted"),
        );
        disagree(
            &mut found,
            0,
            "start observed",
            clock.observed_now(),
            number(expected, "observed"),
        );
        for (index, step) in case
            .get("steps")
            .map(json_t::items)
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            totals.steps += 1;
            let position = index + 1;
            match step.field("op") {
                "sample" => {
                    let observation = sample(number(step, "realtime"), number(step, "boottime"));
                    let anomalous = clock.observe(observation, true).is_some();
                    disagree(
                        &mut found,
                        position,
                        "trusted",
                        clock.now(),
                        number(step, "trusted"),
                    );
                    disagree(
                        &mut found,
                        position,
                        "observed",
                        clock.observed_now(),
                        number(step, "observed"),
                    );
                    let model_anomaly = step.get("anomaly") == Some(&json_t::boolean(true));
                    if anomalous != model_anomaly {
                        found.push(format!(
                            "{name} step {position}: anomaly {anomalous}, model {model_anomaly}"
                        ));
                    }
                }
                "adopt" => {
                    clock.adopt(signed(step, "offset"));
                    disagree(
                        &mut found,
                        position,
                        "trusted",
                        clock.now(),
                        number(step, "trusted"),
                    );
                }
                "floor" => {
                    let floor = clock.floor().expect("floor");
                    let model = floor_of(step);
                    if floor != model {
                        found.push(format!(
                            "{name} step {position}: floor {floor:?}, model {model:?}"
                        ));
                    }
                    floors.push(model);
                }
                "restart" => {
                    clock = trusted_clock_t::start(
                        step.field("boot_id"),
                        sample(number(step, "realtime"), number(step, "boottime")),
                        &floors,
                    )
                    .expect("clock restart");
                    disagree(
                        &mut found,
                        position,
                        "restart trusted",
                        clock.now(),
                        number(step, "trusted"),
                    );
                    disagree(
                        &mut found,
                        position,
                        "restart observed",
                        clock.observed_now(),
                        number(step, "observed"),
                    );
                }
                other => panic!("{name} step {position}: unknown op {other}"),
            }
        }
        totals.disagreements.extend(found);
    }
    totals
}

#[test]
fn the_clock_agrees_with_the_reference_model_on_every_case() {
    let totals = replay();
    assert!(totals.cases >= 165, "only {} cases", totals.cases);
    assert!(totals.steps >= 4_000, "only {} steps", totals.steps);
    assert!(
        totals.disagreements.is_empty(),
        "{} disagreements, first: {:?}",
        totals.disagreements.len(),
        totals.disagreements.iter().take(5).collect::<Vec<_>>()
    );
    println!(
        "trusted-time differential: {} cases, {} steps, 0 disagreements",
        totals.cases, totals.steps
    );
}
