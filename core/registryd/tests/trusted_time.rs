//! Trusted time in registryd (ADR 0007 §5, §8 option B; errata E4 and E6; M2-006
//! stage B1): the offset rule over a settable pair of clocks, its persistence in the
//! ledger and the anchor, same-boot and new-boot restarts, the end of temporary
//! branches at a new boot, the commit-timestamp rule, the observed time of an
//! admission, clock anomalies, `time_sample` and a clock that cannot be sampled.
//! Synthetic users and keys only; the store is a disposable directory.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::unwrap_used
)]

mod builders;

use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use builders::*;
use cham3_contract_format::schema::commit_type_t;
use cham3_identityd::index::utc_text;
use cham3_registryd::anchor::{SlotFileNames, anchor_map_t, decode_slot};
use cham3_registryd::clock::time_floor_t;
use cham3_registryd::journal::{journal_io_t, system_io_t};
use cham3_registryd::ledger::{entry_kind_t, parse_time_payload, previous_ledger_t, read_previous};
use cham3_registryd::lineage::{admitted_at_t, standing_t};
use cham3_registryd::record::{
    lifecycle_record_t, placement_t, record_body_t, registry_event_t, registry_record_t,
};
use cham3_registryd::registry::{
    registry_config_t, registry_start_error_t, registry_t, run_decision_t,
};

/// Ledger appends and anchor writes fail while armed.
#[derive(Clone, Default)]
struct switch_io_t {
    ledger: Arc<AtomicBool>,
}

impl journal_io_t for switch_io_t {
    fn write_all(&self, file: &mut fs::File, bytes: &[u8]) -> io::Result<()> {
        system_io_t.write_all(file, bytes)
    }

    fn sync_data(&self, file: &fs::File) -> io::Result<()> {
        system_io_t.sync_data(file)
    }

    fn sync_directory(&self, path: &Path) -> io::Result<()> {
        system_io_t.sync_directory(path)
    }

    fn append_file(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        if self.ledger.load(Ordering::SeqCst) {
            return Err(io::Error::other("injected ledger append failure"));
        }
        system_io_t.append_file(path, bytes)
    }
}

fn config(boot_id: &str) -> registry_config_t {
    registry_config_t {
        boot_id: Some(boot_id.to_owned()),
        ..registry_config_t::default()
    }
}

fn start(root: &Path, boot_id: &str, time: &test_time_t) -> Arc<registry_t> {
    start_with(root, boot_id, time, Box::new(system_io_t))
}

fn start_with(
    root: &Path,
    boot_id: &str,
    time: &test_time_t,
    io: Box<dyn journal_io_t>,
) -> Arc<registry_t> {
    Arc::new(
        registry_t::open(
            root,
            owner_of(root),
            config(boot_id),
            io,
            Box::new(time.clone()),
        )
        .expect("registry start"),
    )
}

fn trusted_now(registry: &registry_t) -> u64 {
    registry
        .rollback_status(Policyd)
        .expect("status")
        .clock
        .trusted_now
}

fn ledger_floors(root: &Path) -> Vec<time_floor_t> {
    match read_previous(&root.join("ledger"), owner_of(root)) {
        previous_ledger_t::valid { entries, .. } => entries
            .iter()
            .filter(|entry| entry.kind == entry_kind_t::time)
            .map(|entry| parse_time_payload(&entry.payload).expect("time entry"))
            .collect(),
        other => panic!("ledger not valid: {other:?}"),
    }
}

/// The map of the anchor slot with the higher valid counter.
fn newest_anchor(root: &Path) -> anchor_map_t {
    SlotFileNames
        .iter()
        .filter_map(|name| decode_slot(&fs::read(root.join("anchor").join(name)).ok()?))
        .max_by_key(|(counter, _)| *counter)
        .expect("a valid slot")
        .1
}

fn decoded(root: &Path, uid: u32) -> Vec<registry_record_t> {
    records_of(root, uid)
        .iter()
        .map(|bytes| registry_record_t::decode(bytes).expect("record"))
        .collect()
}

fn anomalies(root: &Path, uid: u32) -> Vec<(u64, u64)> {
    decoded(root, uid)
        .into_iter()
        .filter_map(|record| match record.body {
            record_body_t::event(registry_event_t::clock_anomaly {
                trusted,
                behind_seconds,
                ..
            }) => Some((trusted, behind_seconds)),
            _ => None,
        })
        .collect()
}

const Hour: u64 = 3600;
const NoLineage: [u8; 32] = [0; 32];

#[test]
fn trusted_time_rebases_on_forward_samples_and_never_goes_back() {
    let root = temp_root("time_rebase");
    let time = test_time_t::at(T1Seconds);
    let registry = start(&root, "boot-1", &time);
    assert_eq!(trusted_now(&registry), T1Seconds);
    // A forward step is accepted at once and ledgered.
    time.step_wall_clock(T1Seconds + Hour);
    assert_eq!(trusted_now(&registry), T1Seconds + Hour);
    assert!(
        ledger_floors(&root)
            .iter()
            .any(|floor| floor.trusted == T1Seconds + Hour && floor.boot_id == "boot-1"),
        "the increase is in the ledger"
    );
    // A correction back changes nothing: trusted time continues at BOOTTIME pace.
    time.step_wall_clock(T1Seconds);
    time.advance(10);
    assert_eq!(trusted_now(&registry), T1Seconds + Hour + 10);
    let status = registry.rollback_status(Policyd).expect("status");
    assert!(status.clock.anomaly.open, "a sample an hour behind");
    assert_eq!(status.clock.anomaly.behind, Hour);
    time.advance(100);
    assert_eq!(trusted_now(&registry), T1Seconds + Hour + 110);
}

#[test]
fn an_offset_increase_that_cannot_be_ledgered_is_not_relied_on() {
    let root = temp_root("time_unledgered");
    let time = test_time_t::at(T1Seconds);
    let io = switch_io_t::default();
    let registry = start_with(&root, "boot-1", &time, Box::new(io.clone()));
    io.ledger.store(true, Ordering::SeqCst);
    time.step_wall_clock(T1Seconds + Hour);
    assert_eq!(trusted_now(&registry), T1Seconds, "not adopted");
    let status = registry.rollback_status(Policyd).expect("status");
    assert!(
        status.ledger_failed,
        "the failed append makes the run unverifiable"
    );
}

#[test]
fn a_same_boot_restart_resumes_from_the_ledger_and_from_the_anchor_floor() {
    let root = temp_root("time_same_boot");
    let time = test_time_t::at(T1Seconds);
    let registry = start(&root, "boot-1", &time);
    time.step_wall_clock(T1Seconds + Hour);
    assert_eq!(trusted_now(&registry), T1Seconds + Hour);
    // A crash: the ledger holds the increase. The wall clock is set back meanwhile.
    drop(registry);
    time.step_wall_clock(T1Seconds);
    let registry = start(&root, "boot-1", &time);
    assert_eq!(registry.run_decision(), run_decision_t::ledger_replayed);
    assert_eq!(trusted_now(&registry), T1Seconds + Hour);
    // A clean shutdown publishes the floor; the next start reads it from the anchor.
    time.advance(30);
    registry.shutdown().expect("clean shutdown");
    drop(registry);
    let registry = start(&root, "boot-1", &time);
    assert_eq!(registry.run_decision(), run_decision_t::resumed);
    assert_eq!(trusted_now(&registry), T1Seconds + Hour + 30);
}

#[test]
fn a_new_boot_starts_no_earlier_than_the_last_floor() {
    let root = temp_root("time_new_boot");
    let time = test_time_t::at(T1Seconds);
    let registry = start(&root, "boot-1", &time);
    time.advance(2 * Hour);
    registry.shutdown().expect("clean shutdown");
    drop(registry);
    let floor = newest_anchor(&root).time_floor.expect("a floor");
    assert_eq!(floor.trusted, T1Seconds + 2 * Hour);
    assert_eq!(floor.observed, 2 * Hour);
    // The next boot's wall clock starts an hour behind: trusted time does not.
    let rebooted = test_time_t::at(T1Seconds - Hour);
    let registry = start(&root, "boot-2", &rebooted);
    assert_eq!(trusted_now(&registry), T1Seconds + 2 * Hour);
    let status = registry.rollback_status(Policyd).expect("status");
    assert_eq!(status.clock.boot_id, "boot-2");
    // Powered-off time never counts: observed time continues from the floor.
    assert_eq!(status.clock.observed_now, 2 * Hour);
}

#[test]
fn a_new_boot_ends_every_live_temporary_branch_durably() {
    let root = temp_root("time_branches");
    let time = test_time_t::at(T1Seconds);
    let alice = test_user(1001);
    let editor = app("editor");
    let persistent = file_grant(
        "kept",
        alice.uid,
        "home:Kept",
        &["read"],
        persistent_lifetime(),
    );
    let temporary = file_grant(
        "until reboot",
        alice.uid,
        "home:Tmp",
        &["read"],
        timed_lifetime("2026-10-20T00:00:00Z"),
    );
    let genesis = draft(&alice, &editor, commit_type_t::genesis, "boot genesis")
        .add(vec![persistent.clone()])
        .sign(&alice);
    let branch = draft(
        &alice,
        &editor,
        commit_type_t::temporary_grant,
        "boot branch",
    )
    .on(&genesis)
    .add(vec![temporary.clone()])
    .temporary_authority()
    .sign(&alice);
    let registry = start(&root, "boot-1", &time);
    enroll(&registry, &alice);
    admit(&registry, &alice, &genesis).expect("genesis");
    assert_eq!(
        admit(&registry, &alice, &branch),
        Ok(placement_t::temporary)
    );
    registry.shutdown().expect("shutdown");
    drop(registry);
    // A restart in the same boot keeps the branch.
    let registry = start(&root, "boot-1", &time);
    assert_eq!(
        registry.standing(alice.uid, &temporary.grant_id),
        Ok(standing_t::current)
    );
    registry.shutdown().expect("shutdown");
    drop(registry);
    let before = records_of(&root, alice.uid).len();
    // A new boot ends it, durably; the trunk is untouched.
    let registry = start(&root, "boot-2", &time);
    assert_eq!(
        registry.standing(alice.uid, &temporary.grant_id),
        Ok(standing_t::inactive)
    );
    assert_eq!(
        registry.standing(alice.uid, &persistent.grant_id),
        Ok(standing_t::current)
    );
    let records = decoded(&root, alice.uid);
    assert_eq!(records.len(), before + 1);
    assert_eq!(
        records.last().map(|record| record.body.clone()),
        Some(record_body_t::event(registry_event_t::branch_terminated {
            boot_id: "boot-2".to_owned()
        }))
    );
    registry.shutdown().expect("shutdown");
    drop(registry);
    // Replay keeps it ended; the same boot writes no second record.
    let registry = start(&root, "boot-2", &time);
    assert_eq!(
        registry.standing(alice.uid, &temporary.grant_id),
        Ok(standing_t::inactive)
    );
    assert_eq!(records_of(&root, alice.uid).len(), before + 1);
}

#[test]
fn the_anchor_floor_is_refreshed_at_least_every_sixty_seconds() {
    let root = temp_root("time_floor_refresh");
    let time = test_time_t::at(T1Seconds);
    let registry = start(&root, "boot-1", &time);
    let first = newest_anchor(&root).time_floor.expect("start floor");
    assert_eq!(first.boottime, StartBoottime);
    time.advance(59);
    registry.policy_state().expect("policy");
    assert_eq!(newest_anchor(&root).time_floor, Some(first));
    time.advance(1);
    registry.policy_state().expect("policy");
    let refreshed = newest_anchor(&root).time_floor.expect("floor");
    assert_eq!(refreshed.boottime, StartBoottime + 60);
    assert_eq!(refreshed.trusted, T1Seconds + 60);
}

#[test]
fn the_commit_timestamp_rule_is_checked_against_trusted_now() {
    let root = temp_root("time_timestamp_rule");
    let alice = test_user(1001);
    let editor = app("editor");
    let time = test_time_t::at(T1Seconds + 301);
    let registry = start(&root, "boot-1", &time);
    enroll(&registry, &alice);
    let genesis_at = |label: &str, timestamp: u64| {
        draft(&alice, &editor, commit_type_t::genesis, label)
            .add(vec![file_grant(
                label,
                alice.uid,
                "home:Stamp",
                &["read"],
                persistent_lifetime(),
            )])
            .timestamp(&utc_text(timestamp as i64).expect("time"))
            .sign(&alice)
    };
    // 301 s behind trusted now: refused, nothing written.
    let records = records_of(&root, alice.uid).len();
    assert_eq!(
        code(admit(
            &registry,
            &alice,
            &genesis_at("stale stamp", T1Seconds)
        )),
        "admission/timestamp_rule"
    );
    assert_eq!(records_of(&root, alice.uid).len(), records);
    // 300 s ahead of trusted now: admitted.
    let genesis = genesis_at("future stamp", T1Seconds + 601);
    admit(&registry, &alice, &genesis).expect("within 300 s");
    // The parent clause: not earlier than the parent's timestamp minus 300 s.
    let child_at = |label: &str, timestamp: u64| {
        draft(&alice, &editor, commit_type_t::revision, label)
            .on(&genesis)
            .add(vec![file_grant(
                label,
                alice.uid,
                "home:Child",
                &["read"],
                persistent_lifetime(),
            )])
            .timestamp(&utc_text(timestamp as i64).expect("time"))
            .sign(&alice)
    };
    assert_eq!(
        code(admit(
            &registry,
            &alice,
            &child_at("early child", T1Seconds + 300)
        )),
        "admission/timestamp_rule"
    );
    admit(&registry, &alice, &child_at("edge child", T1Seconds + 301)).expect("edge");
}

#[test]
fn an_admission_records_its_trusted_and_observed_time() {
    let root = temp_root("time_admitted_at");
    let alice = test_user(1001);
    let editor = app("editor");
    let grant = file_grant(
        "stamped",
        alice.uid,
        "home:Stamped",
        &["read"],
        persistent_lifetime(),
    );
    let genesis = draft(&alice, &editor, commit_type_t::genesis, "admitted at")
        .add(vec![grant.clone()])
        .sign(&alice);
    let time = test_time_t::at(T1Seconds);
    let registry = start(&root, "boot-1", &time);
    enroll(&registry, &alice);
    time.advance(100);
    admit(&registry, &alice, &genesis).expect("genesis");
    let expected = admitted_at_t {
        trusted: T1Seconds as i64 + 100,
        observed: 100,
    };
    let record = decoded(&root, alice.uid).pop().expect("record");
    assert_eq!(record.time, "2026-10-13T09:01:40Z");
    assert!(matches!(
        record.body,
        record_body_t::commit_admitted { observed: 100, .. }
    ));
    let lineage = genesis.payload.lineage_id;
    let view = registry
        .lineage_head(Policyd, alice.uid, &lineage)
        .expect("view")
        .expect("lineage");
    assert_eq!(view.origins[&grant.grant_id].admitted, expected);
    assert_eq!(view.time.boot_id, "boot-1");
    assert_eq!(view.time.offset, T1Seconds as i64 - StartBoottime as i64);
    assert_eq!(view.time.observed_offset, -(StartBoottime as i64));
    // Replay restores the admission's time from the journal.
    drop(registry);
    let registry = start(&root, "boot-1", &time);
    let view = registry
        .lineage_head(Policyd, alice.uid, &lineage)
        .expect("view")
        .expect("lineage");
    assert_eq!(view.origins[&grant.grant_id].admitted, expected);
}

#[test]
fn a_clock_anomaly_is_recorded_once_per_user_per_episode() {
    let root = temp_root("time_anomaly");
    let alice = test_user(1001);
    let bob = test_user(1002);
    let time = test_time_t::at(T1Seconds);
    let registry = start(&root, "boot-1", &time);
    enroll(&registry, &alice);
    enroll(&registry, &bob);
    let look = |user: &test_user_t| {
        registry
            .lineage_head(Policyd, user.uid, &NoLineage)
            .expect("view");
    };
    // 300 s behind is no anomaly; 301 s is.
    time.step_wall_clock(T1Seconds - 300);
    look(&alice);
    assert_eq!(anomalies(&root, alice.uid), Vec::new());
    time.step_wall_clock(T1Seconds - 301);
    look(&alice);
    assert_eq!(anomalies(&root, alice.uid), vec![(T1Seconds, 301)]);
    look(&alice);
    assert_eq!(anomalies(&root, alice.uid).len(), 1, "once per episode");
    look(&bob);
    assert_eq!(anomalies(&root, bob.uid), vec![(T1Seconds, 301)]);
    // Within 300 s again ends the episode; the next lag opens a new one.
    time.step_wall_clock(T1Seconds);
    look(&alice);
    time.step_wall_clock(T1Seconds - 400);
    look(&alice);
    assert_eq!(
        anomalies(&root, alice.uid),
        vec![(T1Seconds, 301), (T1Seconds, 400)]
    );
    let status = registry.rollback_status(Policyd).expect("status");
    assert_eq!(status.clock.anomaly.episode, 2);
    assert!(status.clock.anomaly.open);
    // The record narrows nothing and replays.
    drop(registry);
    let registry = start(&root, "boot-1", &time);
    assert_eq!(registry.unusable_users().expect("users"), Vec::new());
}

#[test]
fn time_sample_raises_through_the_ledger_and_refuses_other_boots() {
    let root = temp_root("time_sample");
    let time = test_time_t::at(T1Seconds);
    let io = switch_io_t::default();
    let registry = start_with(&root, "boot-1", &time, Box::new(io.clone()));
    let boottime = StartBoottime;
    assert_eq!(
        code(registry.time_sample(Contractd, "boot-1", boottime, T1Seconds + 10)),
        "ipc/role_denied"
    );
    assert_eq!(
        code(registry.time_sample(Policyd, "boot-0", boottime, T1Seconds + 10)),
        "ipc/malformed_request"
    );
    assert_eq!(
        code(registry.time_sample(Policyd, "boot-1", boottime + 60, T1Seconds + 10)),
        "ipc/malformed_request",
        "a CLOCK_BOOTTIME ahead of registryd's own"
    );
    let base = T1Seconds as i64 - boottime as i64;
    let lower = registry
        .time_sample(Policyd, "boot-1", boottime, T1Seconds - 10)
        .expect("lower sample");
    assert_eq!(lower.offset, base);
    let raised = registry
        .time_sample(Policyd, "boot-1", boottime, T1Seconds + 10)
        .expect("raise");
    assert_eq!(raised.offset, base + 10);
    assert!(
        ledger_floors(&root)
            .iter()
            .any(|floor| floor.trusted == T1Seconds + 10)
    );
    assert_eq!(trusted_now(&registry), T1Seconds + 10);
    // A raise that cannot be ledgered is refused and not adopted.
    io.ledger.store(true, Ordering::SeqCst);
    assert_eq!(
        code(registry.time_sample(Policyd, "boot-1", boottime, T1Seconds + 20)),
        "ipc/storage_unavailable"
    );
    assert_eq!(trusted_now(&registry), T1Seconds + 10);
}

#[test]
fn a_clock_that_cannot_be_sampled_fails_admission_but_never_a_restriction() {
    let root = temp_root("time_unavailable");
    let alice = test_user(1001);
    let editor = app("editor");
    let short = file_grant(
        "short",
        alice.uid,
        "home:Short",
        &["read"],
        timed_lifetime("2026-10-13T09:01:00Z"),
    );
    let long = file_grant(
        "long",
        alice.uid,
        "home:Long",
        &["read"],
        timed_lifetime("2026-10-13T10:00:00Z"),
    );
    let genesis = draft(&alice, &editor, commit_type_t::genesis, "unavailable")
        .add(vec![file_grant(
            "base",
            alice.uid,
            "home:Base",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&alice);
    let branch = draft(
        &alice,
        &editor,
        commit_type_t::temporary_grant,
        "short branch",
    )
    .on(&genesis)
    .add(vec![short.clone(), long.clone()])
    .temporary_authority()
    .sign(&alice);
    let revision = draft(
        &alice,
        &editor,
        commit_type_t::revision,
        "while unavailable",
    )
    .on(&genesis)
    .add(vec![file_grant(
        "later",
        alice.uid,
        "home:Later",
        &["read"],
        persistent_lifetime(),
    )])
    .sign(&alice);
    let time = test_time_t::at(T1Seconds);
    let registry = start(&root, "boot-1", &time);
    enroll(&registry, &alice);
    admit(&registry, &alice, &genesis).expect("genesis");
    admit(&registry, &alice, &branch).expect("branch");
    time.advance(60);
    assert_eq!(trusted_now(&registry), T1Seconds + 60);
    let now = time.current().expect("clock");
    time.set_available(false, 0, 0);
    // Admission needs trusted now from its own sample: a stale value could admit a
    // commit after its key's window closed.
    assert_eq!(
        code(admit(&registry, &alice, &revision)),
        "ipc/storage_unavailable"
    );
    // Restrictions use the last trusted value, a lower bound of trusted now: an expiry
    // it already shows is accepted, a later one is still premature.
    registry
        .restrict(
            Policyd,
            alice.uid,
            &lifecycle_record_t::expired {
                grant_id: short.grant_id,
            },
        )
        .expect("an expiry the last trusted value proves");
    assert_eq!(
        code(registry.restrict(
            Policyd,
            alice.uid,
            &lifecycle_record_t::expired {
                grant_id: long.grant_id
            }
        )),
        "admission/lifecycle_rule"
    );
    registry
        .restrict(
            Policyd,
            alice.uid,
            &lifecycle_record_t::restriction {
                grant_ids: vec![long.grant_id],
            },
        )
        .expect("restriction");
    assert_eq!(
        registry.standing(alice.uid, &short.grant_id),
        Ok(standing_t::ended(
            cham3_registryd::lineage::end_reason_t::expired
        ))
    );
    assert_eq!(
        registry.standing(alice.uid, &long.grant_id),
        Ok(standing_t::ended(
            cham3_registryd::lineage::end_reason_t::restriction
        ))
    );
    time.set_available(true, now.realtime, now.boottime);
    admit(&registry, &alice, &revision).expect("clock restored");
}

#[test]
fn a_registry_whose_clock_cannot_be_sampled_does_not_start() {
    let root = temp_root("time_start_unavailable");
    let time = test_time_t::at(T1Seconds);
    time.set_available(false, 0, 0);
    let refused = registry_t::open(
        &root,
        owner_of(&root),
        config("boot-1"),
        Box::new(system_io_t),
        Box::new(time),
    );
    assert!(matches!(
        refused,
        Err(registry_start_error_t::clock_unavailable)
    ));
}

/// Each `clock_anomaly` record of a user: (boot id, episode).
fn anomaly_episodes(root: &Path, uid: u32) -> Vec<(String, u64)> {
    decoded(root, uid)
        .into_iter()
        .filter_map(|record| match record.body {
            record_body_t::event(registry_event_t::clock_anomaly {
                boot_id, episode, ..
            }) => Some((boot_id, episode)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_clock_anomaly_episode_survives_same_boot_restarts() {
    // M2-006-B1-R02: an episode ends only at a sample within 300 s of trusted time,
    // never at a restart; its end is persisted too, so that a later episode is new.
    let root = temp_root("time_anomaly_restart");
    let alice = test_user(1001);
    let bob = test_user(1002);
    let time = test_time_t::at(T1Seconds);
    let registry = start(&root, "boot-1", &time);
    enroll(&registry, &alice);
    enroll(&registry, &bob);
    let look = |registry: &registry_t, user: &test_user_t| {
        registry
            .lineage_head(Policyd, user.uid, &NoLineage)
            .expect("view");
    };
    let one = |episode: u64| ("boot-1".to_owned(), episode);
    time.step_wall_clock(T1Seconds - 400);
    look(&registry, &alice);
    assert_eq!(anomaly_episodes(&root, alice.uid), vec![one(1)]);
    // A clean restart in the same boot, at the same clocks: the episode continues.
    registry.shutdown().expect("clean shutdown");
    drop(registry);
    let registry = start(&root, "boot-1", &time);
    let status = registry.rollback_status(Policyd).expect("status");
    assert_eq!(
        (status.clock.anomaly.episode, status.clock.anomaly.open),
        (1, true)
    );
    look(&registry, &alice);
    assert_eq!(anomaly_episodes(&root, alice.uid), vec![one(1)]);
    // Another user reports the resumed episode once.
    look(&registry, &bob);
    look(&registry, &bob);
    assert_eq!(anomaly_episodes(&root, bob.uid), vec![one(1)]);
    // An unclean restart (the ledger holds the episode): still the same episode.
    drop(registry);
    let registry = start(&root, "boot-1", &time);
    assert_eq!(registry.run_decision(), run_decision_t::ledger_replayed);
    look(&registry, &alice);
    assert_eq!(anomaly_episodes(&root, alice.uid), vec![one(1)]);
    // A sample within 300 s ends it; the end survives an unclean restart, and the next
    // lag is a new episode that each user reports once.
    time.step_wall_clock(T1Seconds);
    look(&registry, &alice);
    drop(registry);
    time.step_wall_clock(T1Seconds - 500);
    let registry = start(&root, "boot-1", &time);
    let status = registry.rollback_status(Policyd).expect("status");
    assert_eq!(
        (status.clock.anomaly.episode, status.clock.anomaly.open),
        (2, true)
    );
    look(&registry, &alice);
    look(&registry, &alice);
    assert_eq!(anomaly_episodes(&root, alice.uid), vec![one(1), one(2)]);
    // A new boot counts its own episodes.
    registry.shutdown().expect("shutdown");
    drop(registry);
    let registry = start(&root, "boot-2", &time);
    look(&registry, &alice);
    assert_eq!(
        anomaly_episodes(&root, alice.uid),
        vec![one(1), one(2), ("boot-2".to_owned(), 1)]
    );
}
