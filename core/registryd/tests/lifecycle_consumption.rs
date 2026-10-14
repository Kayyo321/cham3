//! Lifecycle and consumption in registryd (ADR 0007 §§3, 4, 6, 7; ADR 0008 §2;
//! erratum E7; M2-006 stage B2): one-shot reservations and their outcomes, startup
//! reconciliation (`consumed_uncertain`), session events, enforcement outcomes and
//! their per-run status, audit gaps for lost evidence, `identity_mismatch`,
//! `history`, and replay of the new records. Synthetic users and keys only; the
//! store is a disposable directory.
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
use cham3_contract_format::hash::digest_t;
use cham3_contract_format::schema::{
    commit_type_t, grant_t, lifetime_t, random_id_t, resource_category_t, subject_t,
};
use cham3_identityd::caller::caller_role_t;
use cham3_registryd::journal::{journal_io_t, system_io_t};
use cham3_registryd::ledger::{previous_ledger_t, read_previous};
use cham3_registryd::lineage::{end_reason_t, standing_t};
use cham3_registryd::record::{
    enforcement_domain_t, enforcement_result_t, frame, genesis_chain, header, lifecycle_record_t,
    lost_evidence_t, next_chain, record_body_t, registry_event_t, registry_record_t,
    session_change_t, use_outcome_t,
};
use cham3_registryd::registry::{
    quarantine_reason_t, registry_config_t, registry_t, run_decision_t, unusable_reason_t,
};

const Consumer: caller_role_t = caller_role_t::fixture_consumer;

/// Journal appends fail while `journal` is armed, ledger appends while `ledger` is,
/// anchor slot writes while `anchor` is.
#[derive(Clone, Default)]
struct fault_io_t {
    journal: Arc<AtomicBool>,
    ledger: Arc<AtomicBool>,
    anchor: Arc<AtomicBool>,
}

impl fault_io_t {
    fn journal_fails(&self, failing: bool) {
        self.journal.store(failing, Ordering::SeqCst);
    }
}

impl journal_io_t for fault_io_t {
    fn write_all(&self, file: &mut fs::File, bytes: &[u8]) -> io::Result<()> {
        if self.journal.load(Ordering::SeqCst) {
            return Err(io::Error::other("injected journal write failure"));
        }
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

    fn replace_file(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let slot = path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("slot"));
        if slot && self.anchor.load(Ordering::SeqCst) {
            return Err(io::Error::other("injected anchor write failure"));
        }
        system_io_t.replace_file(path, bytes)
    }
}

/// The clocks at T1 + 60 s: every grant starting at T1 has started.
fn clock() -> test_time_t {
    test_time_t::at(T1Seconds + 60)
}

fn start(root: &Path, boot_id: &str, time: &test_time_t, io: &fault_io_t) -> Arc<registry_t> {
    let config = registry_config_t {
        boot_id: Some(boot_id.to_owned()),
        ..registry_config_t::default()
    };
    Arc::new(
        registry_t::open(
            root,
            owner_of(root),
            config,
            Box::new(io.clone()),
            Box::new(time.clone()),
        )
        .expect("registry start"),
    )
}

fn decoded(root: &Path, uid: u32) -> Vec<registry_record_t> {
    records_of(root, uid)
        .iter()
        .map(|bytes| registry_record_t::decode(bytes).expect("decodes"))
        .collect()
}

fn last_body(root: &Path, uid: u32) -> record_body_t {
    decoded(root, uid).pop().expect("a record").body
}

/// Bodies of the ledger entries of the previous run (a same-boot start reads them).
fn ledger_bodies(root: &Path) -> Vec<record_body_t> {
    match read_previous(&root.join("ledger"), owner_of(root)) {
        previous_ledger_t::valid { entries, .. } => entries
            .iter()
            .filter_map(|entry| registry_record_t::decode(&entry.payload).ok())
            .map(|record| record.body)
            .collect(),
        other => panic!("ledger not valid: {other:?}"),
    }
}

/// A user's lineage: a persistent genesis, and a temporary branch adding `grants`.
struct lineage_fixture_t {
    user: test_user_t,
    app: app_t,
    genesis: commit_t,
    branch: commit_t,
}

impl lineage_fixture_t {
    fn subject(&self) -> subject_t {
        subject(&self.user, &self.app)
    }

    fn lineage_id(&self) -> digest_t {
        self.genesis.payload.lineage_id
    }
}

fn with_branch(
    registry: &Arc<registry_t>,
    uid: u32,
    label: &str,
    grants: Vec<grant_t>,
) -> lineage_fixture_t {
    let user = test_user(uid);
    enroll(registry, &user);
    let app = app(label);
    let genesis = draft(
        &user,
        &app,
        commit_type_t::genesis,
        &format!("{label} genesis"),
    )
    .add(vec![file_grant(
        &format!("{label} cache"),
        user.uid,
        "home:cache",
        &["read"],
        persistent_lifetime(),
    )])
    .sign(&user);
    admit(registry, &user, &genesis).expect("genesis");
    // A temporary branch adds at least one grant.
    let grants = if grants.is_empty() {
        vec![file_grant(
            &format!("{label} week"),
            user.uid,
            "home:Week",
            &["read"],
            timed_lifetime(Week),
        )]
    } else {
        grants
    };
    let branch = draft(
        &user,
        &app,
        commit_type_t::temporary_grant,
        &format!("{label} branch"),
    )
    .on(&genesis)
    .add(grants)
    .temporary_authority()
    .sign(&user);
    admit(registry, &user, &branch).expect("branch");
    lineage_fixture_t {
        user,
        app,
        genesis,
        branch,
    }
}

fn one_shot(label: &str, uid: u32) -> grant_t {
    file_grant(label, uid, "home:Scan", &["read"], one_shot_lifetime())
}

fn reserve(
    registry: &registry_t,
    fixture: &lineage_fixture_t,
    grant_id: &random_id_t,
) -> Result<random_id_t, cham3_identityd::outcome::service_error_t> {
    registry
        .reserve_use(
            Policyd,
            fixture.user.uid,
            grant_id,
            &fixture.subject(),
            &synthetic_id("operation"),
        )
        .map(|receipt| receipt.reservation_id)
}

#[test]
fn a_reservation_consumes_the_one_shot_grant_whatever_the_outcome() {
    // ADR 0007 §4 option A, erratum E7.1: registryd reserves for policyd only; the
    // grant is consumed from the durable `use_reserved` on, a second reservation is
    // refused, and the consumer's outcome (even a failure) returns nothing.
    let root = temp_root("reserve");
    let time = clock();
    let io = fault_io_t::default();
    let registry = start(&root, "boot-1", &time, &io);
    let scan = one_shot("scan once", 1001);
    let fixture = with_branch(&registry, 1001, "scanner", vec![scan.clone()]);
    let uid = fixture.user.uid;
    assert_eq!(
        code(registry.reserve_use(
            Consumer,
            uid,
            &scan.grant_id,
            &fixture.subject(),
            &synthetic_id("operation"),
        )),
        "ipc/role_denied",
        "the consumer goes through policyd"
    );
    assert_eq!(
        registry.standing(uid, &scan.grant_id),
        Ok(standing_t::current)
    );
    let reservation = reserve(&registry, &fixture, &scan.grant_id).expect("reserved");
    assert_eq!(
        registry.standing(uid, &scan.grant_id),
        Ok(standing_t::ended(end_reason_t::consumed))
    );
    let records = decoded(&root, uid);
    let anchored = registry.anchored_user(uid).unwrap().expect("anchored");
    assert_eq!(anchored.seq, records.len() as u64, "durable and anchored");
    assert_eq!(
        records.last().expect("record").body,
        record_body_t::event(registry_event_t::use_reserved {
            grant_id: scan.grant_id,
            reservation_id: reservation,
            operation_id: synthetic_id("operation"),
        })
    );
    // The view ends the grant for policyd.
    let view = registry
        .lineage_head(Policyd, uid, &fixture.lineage_id())
        .expect("view")
        .expect("user");
    assert_eq!(
        view.ended.get(&scan.grant_id),
        Some(&end_reason_t::consumed)
    );
    assert_eq!(
        code(reserve(&registry, &fixture, &scan.grant_id)),
        "admission/lifecycle_rule",
        "exactly one reservation"
    );
    // The outcome: the consumer only, once, never restoring the grant.
    assert_eq!(
        code(registry.complete_use(Policyd, uid, &reservation, use_outcome_t::failure)),
        "ipc/role_denied"
    );
    assert_eq!(
        code(registry.complete_use(Consumer, uid, &[7; 16], use_outcome_t::success)),
        "admission/lifecycle_rule",
        "an unknown reservation"
    );
    let completed = registry
        .complete_use(Consumer, uid, &reservation, use_outcome_t::failure)
        .expect("completed");
    assert_eq!(
        last_body(&root, uid),
        record_body_t::event(registry_event_t::use_completed {
            reservation_id: reservation,
            outcome: use_outcome_t::failure,
        })
    );
    assert_eq!(completed, decoded(&root, uid).len() as u64);
    assert_eq!(
        code(registry.complete_use(Consumer, uid, &reservation, use_outcome_t::success)),
        "admission/lifecycle_rule",
        "at most one outcome"
    );
    assert_eq!(
        registry.standing(uid, &scan.grant_id),
        Ok(standing_t::ended(end_reason_t::consumed))
    );
    // A restart replays both; a completed reservation is not uncertain.
    let count = decoded(&root, uid).len();
    registry.shutdown().expect("clean shutdown");
    drop(registry);
    let registry = start(&root, "boot-1", &time, &io);
    assert_eq!(registry.run_decision(), run_decision_t::resumed);
    assert_eq!(
        registry.standing(uid, &scan.grant_id),
        Ok(standing_t::ended(end_reason_t::consumed))
    );
    assert_eq!(decoded(&root, uid).len(), count);
}

#[test]
fn registryd_rechecks_its_own_rules_before_it_reserves() {
    // Erratum E7.1: under the user lock registryd re-checks every rule its snapshot
    // decides; each refusal leaves the grant unconsumed and writes nothing.
    let root = temp_root("reserve_rules");
    let time = clock();
    let io = fault_io_t::default();
    let registry = start(&root, "boot-1", &time, &io);
    let mut expiring = one_shot_lifetime();
    expiring.expiration = Some("2026-10-13T09:05:00Z".to_owned());
    let mut later = one_shot_lifetime();
    later.effective_start = "2026-10-13T10:00:00Z".to_owned();
    let mut soon = one_shot_lifetime();
    soon.effective_start = "2026-10-13T09:02:00Z".to_owned();
    let mut in_session = one_shot_lifetime();
    in_session.session_id = Some(synthetic_id("session scan"));
    let plain = one_shot("plain", 1001);
    let expires = file_grant("expires", 1001, "home:A", &["read"], expiring);
    let starts_later = file_grant("later", 1001, "home:B", &["read"], later);
    let session_bound = file_grant("session bound", 1001, "home:C", &["read"], in_session);
    let starts_soon = file_grant("soon", 1001, "home:D", &["read"], soon);
    let fixture = with_branch(
        &registry,
        1001,
        "rules",
        vec![
            plain.clone(),
            expires.clone(),
            starts_later.clone(),
            session_bound.clone(),
            starts_soon.clone(),
        ],
    );
    let uid = fixture.user.uid;
    let mut records = decoded(&root, uid).len();
    let persistent = synthetic_id("grant rules cache");
    // Not a one-shot grant, an unknown grant.
    for grant_id in [persistent, [9; 16]] {
        assert_eq!(
            code(reserve(&registry, &fixture, &grant_id)),
            "admission/lifecycle_rule"
        );
    }
    // Another executable, another owner's subject.
    let mut other_executable = fixture.subject();
    other_executable.executable_sha256 = [9; 32];
    let mut other_owner = fixture.subject();
    other_owner.owner_user_id = [8; 32];
    for subject in [other_executable, other_owner] {
        assert_eq!(
            code(registry.reserve_use(
                Policyd,
                uid,
                &plain.grant_id,
                &subject,
                &synthetic_id("operation"),
            )),
            "admission/subject_binding_mismatch"
        );
    }
    // Not started: admitted at T1 + 60, starting at T1 + 120 and at 10:00.
    for grant in [&starts_soon, &starts_later] {
        assert_eq!(
            code(reserve(&registry, &fixture, &grant.grant_id)),
            "admission/lifecycle_rule"
        );
    }
    // A bound session that ended.
    registry
        .session_event(
            Consumer,
            uid,
            &synthetic_id("session scan"),
            session_change_t::ended,
        )
        .expect("session ended");
    records += 1;
    assert_eq!(
        code(reserve(&registry, &fixture, &session_bound.grant_id)),
        "admission/lifecycle_rule"
    );
    // The wall clock steps an hour forward: trusted now passes both starts and the
    // expiration, but no observed time has passed since the admission (ADR 0007 §8
    // option B), so neither start is reached; the expired grant stays refused.
    time.step_wall_clock(T1Seconds + 3_660);
    for grant in [&starts_soon, &starts_later, &expires] {
        assert_eq!(
            code(reserve(&registry, &fixture, &grant.grant_id)),
            "admission/lifecycle_rule"
        );
    }
    // A minute passes: the start one minute after the admission is reached (the
    // positive control).
    time.advance(60);
    reserve(&registry, &fixture, &starts_soon.grant_id).expect("reached");
    records += 1;
    // A clock that cannot be sampled: no fresh trusted now.
    let now = time.current().expect("clock");
    time.set_available(false, now.realtime, now.boottime);
    assert_eq!(
        code(reserve(&registry, &fixture, &plain.grant_id)),
        "ipc/storage_unavailable"
    );
    time.set_available(true, now.realtime, now.boottime);
    // The persisted emergency restriction of everyone.
    registry
        .set_emergency_restriction(Admin, true)
        .expect("restricted");
    assert_eq!(
        code(reserve(&registry, &fixture, &plain.grant_id)),
        "admission/lifecycle_rule"
    );
    registry
        .set_emergency_restriction(Admin, false)
        .expect("released");
    // A minimum valid policy epoch above the commit's.
    registry.set_policy_epoch(Admin, 2, 2).expect("raised");
    assert_eq!(
        code(reserve(&registry, &fixture, &plain.grant_id)),
        "ipc/stale_epoch"
    );
    assert_eq!(decoded(&root, uid).len(), records, "nothing else written");
    for grant in [&plain, &expires, &starts_later] {
        assert_eq!(
            registry.standing(uid, &grant.grant_id),
            Ok(standing_t::current)
        );
    }
}

#[test]
fn a_contained_user_reserves_nothing() {
    let root = temp_root("reserve_contained");
    let time = clock();
    let io = fault_io_t::default();
    let registry = start(&root, "boot-1", &time, &io);
    let scan = one_shot("contained scan", 1001);
    let fixture = with_branch(&registry, 1001, "contained", vec![scan.clone()]);
    // An unclean run across boots contains every user.
    drop(registry);
    let registry = start(&root, "boot-2", &time, &io);
    assert_eq!(registry.run_decision(), run_decision_t::contained);
    assert_eq!(
        code(reserve(&registry, &fixture, &scan.grant_id)),
        "key_authority/contained"
    );
}

#[test]
fn a_reservation_that_cannot_be_journaled_still_consumes_and_denies() {
    // §4: "a failed reservation write still consumes in memory and in the ledger,
    // and denies the operation." A same-boot restart re-journals it from the ledger
    // and reports it uncertain (§7).
    let root = temp_root("reserve_failed");
    let time = clock();
    let io = fault_io_t::default();
    let registry = start(&root, "boot-1", &time, &io);
    let scan = one_shot("scan failing", 1001);
    let fixture = with_branch(&registry, 1001, "failing", vec![scan.clone()]);
    let uid = fixture.user.uid;
    let records = decoded(&root, uid).len();
    io.journal_fails(true);
    assert_eq!(
        code(reserve(&registry, &fixture, &scan.grant_id)),
        "ipc/restriction_not_durable"
    );
    assert_eq!(
        registry.standing(uid, &scan.grant_id),
        Ok(standing_t::ended(end_reason_t::consumed)),
        "consumed in memory"
    );
    assert_eq!(decoded(&root, uid).len(), records, "not in the journal");
    let reservation = ledger_bodies(&root)
        .into_iter()
        .find_map(|body| match body {
            record_body_t::event(registry_event_t::use_reserved {
                grant_id,
                reservation_id,
                ..
            }) if grant_id == scan.grant_id => Some(reservation_id),
            _ => None,
        })
        .expect("in the ledger");
    assert_eq!(
        code(reserve(&registry, &fixture, &scan.grant_id)),
        "ipc/restriction_not_durable",
        "the pending write is retried first; the grant stays consumed"
    );
    assert_eq!(
        registry
            .rollback_status(Policyd)
            .unwrap()
            .pending_restrictions,
        vec![uid]
    );
    // The process dies; a same-boot start replays the ledger.
    drop(registry);
    io.journal_fails(false);
    let registry = start(&root, "boot-1", &time, &io);
    assert_eq!(registry.run_decision(), run_decision_t::ledger_replayed);
    assert_eq!(
        registry.standing(uid, &scan.grant_id),
        Ok(standing_t::ended(end_reason_t::consumed))
    );
    let bodies: Vec<record_body_t> = decoded(&root, uid)
        .into_iter()
        .skip(records)
        .map(|record| record.body)
        .collect();
    assert_eq!(
        bodies,
        vec![
            record_body_t::event(registry_event_t::use_reserved {
                grant_id: scan.grant_id,
                reservation_id: reservation,
                operation_id: synthetic_id("operation"),
            }),
            record_body_t::event(registry_event_t::consumed_uncertain {
                reservation_id: reservation
            }),
        ]
    );
    assert_eq!(
        code(registry.complete_use(Consumer, uid, &reservation, use_outcome_t::success)),
        "admission/lifecycle_rule",
        "an uncertain reservation takes no later outcome"
    );
}

#[test]
fn a_start_reports_every_open_reservation_as_consumed_uncertain() {
    // §7: after any restart — clean, unclean in the same boot, or a new boot — a
    // reservation without a reported outcome is `consumed_uncertain`, once; the grant
    // stays consumed.
    for (label, clean, next_boot) in [
        ("uncertain_clean", true, "boot-1"),
        ("uncertain_unclean", false, "boot-1"),
        ("uncertain_reboot", true, "boot-2"),
    ] {
        let root = temp_root(label);
        let time = clock();
        let io = fault_io_t::default();
        let registry = start(&root, "boot-1", &time, &io);
        let scan = one_shot("scan open", 1001);
        let done = file_grant(
            "scan done",
            1001,
            "home:Done",
            &["read"],
            one_shot_lifetime(),
        );
        let fixture = with_branch(&registry, 1001, label, vec![scan.clone(), done.clone()]);
        let uid = fixture.user.uid;
        let open = reserve(&registry, &fixture, &scan.grant_id).expect("reserved");
        let completed = reserve(&registry, &fixture, &done.grant_id).expect("reserved");
        registry
            .complete_use(Consumer, uid, &completed, use_outcome_t::success)
            .expect("completed");
        if clean {
            registry.shutdown().expect("shutdown");
        }
        drop(registry);
        let registry = start(&root, next_boot, &time, &io);
        let uncertain: Vec<random_id_t> = decoded(&root, uid)
            .into_iter()
            .filter_map(|record| match record.body {
                record_body_t::event(registry_event_t::consumed_uncertain { reservation_id }) => {
                    Some(reservation_id)
                }
                _ => None,
            })
            .collect();
        assert_eq!(uncertain, vec![open], "{label}");
        assert_eq!(
            registry.standing(uid, &scan.grant_id),
            Ok(standing_t::ended(end_reason_t::consumed)),
            "{label}"
        );
        // Once: the next start finds nothing open.
        let count = decoded(&root, uid).len();
        registry.shutdown().expect("shutdown");
        drop(registry);
        let _registry = start(&root, next_boot, &time, &io);
        assert_eq!(decoded(&root, uid).len(), count, "{label}");
    }
}

#[test]
fn session_events_start_once_and_end_for_good() {
    // ADR 0007 §6: a start is evidence; an end is restrictive and final. A session
    // without bound grants ends with a `session_event` end, one with bound grants
    // with `session_ended`; admission refuses grants bound to an ended session.
    let root = temp_root("sessions");
    let time = clock();
    let io = fault_io_t::default();
    let registry = start(&root, "boot-1", &time, &io);
    let bound = file_grant(
        "bound to two",
        1001,
        "home:Two",
        &["read"],
        session_lifetime("two"),
    );
    let fixture = with_branch(&registry, 1001, "sessions", vec![bound.clone()]);
    let uid = fixture.user.uid;
    let one = synthetic_id("session one");
    let two = bound.lifetime.session_id.expect("session");
    assert_eq!(
        code(registry.session_event(Policyd, uid, &one, session_change_t::started)),
        "ipc/role_denied"
    );
    let started = registry
        .session_event(Consumer, uid, &one, session_change_t::started)
        .expect("started");
    assert_eq!(
        last_body(&root, uid),
        record_body_t::event(registry_event_t::session_event {
            session_id: one,
            change: session_change_t::started,
        })
    );
    assert_eq!(
        registry.session_event(Consumer, uid, &one, session_change_t::started),
        Ok(started),
        "a repeated start is the first one"
    );
    let ended = registry
        .session_event(Consumer, uid, &one, session_change_t::ended)
        .expect("ended");
    assert_eq!(
        last_body(&root, uid),
        record_body_t::event(registry_event_t::session_event {
            session_id: one,
            change: session_change_t::ended,
        })
    );
    assert_eq!(
        registry.session_event(Consumer, uid, &one, session_change_t::ended),
        Ok(ended),
        "a repeated end is the first one"
    );
    assert_eq!(
        code(registry.session_event(Consumer, uid, &one, session_change_t::started)),
        "admission/lifecycle_rule",
        "session ids are single-use"
    );
    // A grant bound to the ended session is never admitted (beyond the model).
    let late = draft(
        &fixture.user,
        &fixture.app,
        commit_type_t::temporary_grant,
        "late",
    )
    .on(&fixture.genesis)
    .add(vec![file_grant(
        "late in one",
        uid,
        "home:Late",
        &["read"],
        session_lifetime("one"),
    )])
    .temporary_authority()
    .sign(&fixture.user);
    assert_eq!(
        code(admit(&registry, &fixture.user, &late)),
        "admission/duplicate_identifier"
    );
    // The bound session ends its grant with `session_ended`.
    registry
        .session_event(Consumer, uid, &two, session_change_t::ended)
        .expect("ended");
    assert_eq!(
        last_body(&root, uid),
        record_body_t::lifecycle(lifecycle_record_t::session_ended { session_id: two })
    );
    assert_eq!(
        registry.standing(uid, &bound.grant_id),
        Ok(standing_t::ended(end_reason_t::session_ended))
    );
    // Replay keeps every effect.
    let count = decoded(&root, uid).len();
    registry.shutdown().expect("shutdown");
    drop(registry);
    let registry = start(&root, "boot-1", &time, &io);
    assert_eq!(decoded(&root, uid).len(), count);
    assert_eq!(
        code(registry.session_event(Consumer, uid, &one, session_change_t::started)),
        "admission/lifecycle_rule"
    );
    assert_eq!(
        code(admit(&registry, &fixture.user, &late)),
        "admission/duplicate_identifier"
    );
    assert_eq!(
        registry.standing(uid, &bound.grant_id),
        Ok(standing_t::ended(end_reason_t::session_ended))
    );
}

#[test]
fn enforcement_status_counts_only_this_runs_outcomes() {
    // ADR 0006 §1, erratum E7.2: per domain, the latest outcome of this run; a
    // restart shows the commit pending again, whatever the history holds.
    let root = temp_root("enforcement");
    let time = clock();
    let io = fault_io_t::default();
    let registry = start(&root, "boot-1", &time, &io);
    let fixture = with_branch(&registry, 1001, "enforced", Vec::new());
    let uid = fixture.user.uid;
    let head = fixture.genesis.commit_id;
    let sandbox = enforcement_domain_t::launcher;
    let files = enforcement_domain_t::resource(resource_category_t::filesystem);
    let confirm = |commit: &digest_t, domain, epoch, result| {
        registry.confirm_enforcement(Consumer, uid, commit, &[3; 16], domain, epoch, result)
    };
    assert!(registry.enforcement_status(uid, &head).unwrap().is_empty());
    assert_eq!(
        code(registry.confirm_enforcement(
            Policyd,
            uid,
            &head,
            &[3; 16],
            sandbox,
            1,
            enforcement_result_t::confirmed,
        )),
        "ipc/role_denied"
    );
    confirm(&head, sandbox, 1, enforcement_result_t::confirmed).expect("sandbox");
    confirm(&head, files, 1, enforcement_result_t::confirmed).expect("files");
    confirm(&head, sandbox, 1, enforcement_result_t::failed).expect("sandbox again");
    assert_eq!(
        registry.enforcement_status(uid, &head).unwrap(),
        vec![
            (sandbox, enforcement_result_t::failed),
            (files, enforcement_result_t::confirmed)
        ],
        "the latest per domain"
    );
    // The policy epoch in force, a live head.
    assert_eq!(
        code(confirm(&head, files, 2, enforcement_result_t::confirmed)),
        "ipc/stale_epoch"
    );
    assert_eq!(
        code(confirm(&[1; 32], files, 1, enforcement_result_t::confirmed)),
        "ipc/stale_head"
    );
    confirm(
        &fixture.branch.commit_id,
        sandbox,
        1,
        enforcement_result_t::confirmed,
    )
    .expect("a live temporary head");
    let revision = draft(&fixture.user, &fixture.app, commit_type_t::revision, "next")
        .on(&fixture.genesis)
        .add(vec![file_grant(
            "next pictures",
            uid,
            "home:Pictures",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&fixture.user);
    admit(&registry, &fixture.user, &revision).expect("revision");
    assert_eq!(
        code(confirm(&head, sandbox, 1, enforcement_result_t::confirmed)),
        "ipc/stale_head",
        "a superseded commit takes no confirmation"
    );
    assert_eq!(
        code(confirm(
            &fixture.branch.commit_id,
            sandbox,
            1,
            enforcement_result_t::confirmed
        )),
        "ipc/stale_head",
        "the trunk commit ended the branch"
    );
    let outcomes = decoded(&root, uid)
        .iter()
        .filter(|record| {
            matches!(
                record.body,
                record_body_t::event(registry_event_t::enforcement_outcome { .. })
            )
        })
        .count();
    assert_eq!(outcomes, 4);
    // A restart in the same boot: every commit is pending again.
    registry.shutdown().expect("shutdown");
    drop(registry);
    let registry = start(&root, "boot-1", &time, &io);
    assert!(registry.enforcement_status(uid, &head).unwrap().is_empty());
    assert!(
        registry
            .enforcement_status(uid, &revision.commit_id)
            .unwrap()
            .is_empty()
    );
    registry
        .confirm_enforcement(
            Consumer,
            uid,
            &revision.commit_id,
            &[4; 16],
            sandbox,
            1,
            enforcement_result_t::confirmed,
        )
        .expect("fresh confirmation");
    assert_eq!(
        registry
            .enforcement_status(uid, &revision.commit_id)
            .unwrap(),
        vec![(sandbox, enforcement_result_t::confirmed)]
    );
}

#[test]
fn lost_evidence_leaves_an_audit_gap_once_storage_recovers() {
    // Erratum E7.3: an evidence record whose ordinary write fails is lost; an
    // `audit_gap` naming its kind is ledgered at once and journaled once storage
    // recovers. The evidence's effect is not applied.
    let root = temp_root("gaps");
    let time = clock();
    let io = fault_io_t::default();
    let registry = start(&root, "boot-1", &time, &io);
    let scan = one_shot("scan gap", 1001);
    let fixture = with_branch(&registry, 1001, "gaps", vec![scan.clone()]);
    let uid = fixture.user.uid;
    let reservation = reserve(&registry, &fixture, &scan.grant_id).expect("reserved");
    let records = decoded(&root, uid).len();
    io.journal_fails(true);
    assert_eq!(
        code(registry.complete_use(Consumer, uid, &reservation, use_outcome_t::success)),
        "ipc/storage_unavailable"
    );
    let gap = |lost| {
        record_body_t::event(registry_event_t::audit_gap {
            boot_id: "boot-1".to_owned(),
            lost,
        })
    };
    assert!(ledger_bodies(&root).contains(&gap(lost_evidence_t::use_completed)));
    assert_eq!(decoded(&root, uid).len(), records);
    // The session start and the enforcement outcome are lost too.
    assert!(
        registry
            .session_event(Consumer, uid, &synthetic_id("s"), session_change_t::started)
            .is_err()
    );
    assert!(
        registry
            .confirm_enforcement(
                Consumer,
                uid,
                &fixture.genesis.commit_id,
                &[3; 16],
                enforcement_domain_t::launcher,
                1,
                enforcement_result_t::confirmed,
            )
            .is_err()
    );
    assert!(
        registry
            .enforcement_status(uid, &fixture.genesis.commit_id)
            .unwrap()
            .is_empty()
    );
    // Storage recovers: the next operation of the user writes the gaps first.
    io.journal_fails(false);
    registry
        .session_event(Consumer, uid, &synthetic_id("s"), session_change_t::started)
        .expect("started");
    let bodies: Vec<record_body_t> = decoded(&root, uid)
        .into_iter()
        .skip(records)
        .map(|record| record.body)
        .collect();
    assert_eq!(
        bodies,
        vec![
            gap(lost_evidence_t::use_completed),
            gap(lost_evidence_t::session_event),
            gap(lost_evidence_t::enforcement_outcome),
            record_body_t::event(registry_event_t::session_event {
                session_id: synthetic_id("s"),
                change: session_change_t::started,
            }),
        ]
    );
    // The reservation never got its outcome: it is still open, so the outcome can be
    // reported now.
    registry
        .complete_use(Consumer, uid, &reservation, use_outcome_t::success)
        .expect("reported now");
}

#[test]
fn identity_mismatch_is_recorded_once_per_executable_identity() {
    // Erratum E7.4: policyd reports through `restrict`; one record per lineage,
    // package identity and executable hash; never for the authoritative subject.
    let root = temp_root("mismatch");
    let time = clock();
    let io = fault_io_t::default();
    let registry = start(&root, "boot-1", &time, &io);
    let fixture = with_branch(&registry, 1001, "mismatch", Vec::new());
    let uid = fixture.user.uid;
    let mismatch = |package: &str, executable: digest_t| lifecycle_record_t::identity_mismatch {
        lineage_id: fixture.lineage_id(),
        package_identity: package.to_owned(),
        executable_sha256: executable,
    };
    let updated = mismatch(&fixture.app.package_identity, [9; 32]);
    assert_eq!(
        code(registry.restrict(Contractd, uid, &updated)),
        "ipc/role_denied"
    );
    let first = registry.restrict(Policyd, uid, &updated).expect("recorded");
    let count = decoded(&root, uid).len();
    assert_eq!(
        last_body(&root, uid),
        record_body_t::lifecycle(updated.clone())
    );
    assert_eq!(registry.restrict(Policyd, uid, &updated), Ok(first));
    assert_eq!(decoded(&root, uid).len(), count, "once");
    registry
        .restrict(Policyd, uid, &mismatch("deb:other:amd64", [9; 32]))
        .expect("another package identity");
    assert_eq!(decoded(&root, uid).len(), count + 1);
    // The authoritative identity, an unknown lineage.
    assert_eq!(
        code(registry.restrict(
            Policyd,
            uid,
            &mismatch(&fixture.app.package_identity, fixture.app.executable_sha256)
        )),
        "admission/lifecycle_rule"
    );
    assert_eq!(
        code(registry.restrict(
            Policyd,
            uid,
            &lifecycle_record_t::identity_mismatch {
                lineage_id: [5; 32],
                package_identity: "deb:x:amd64".to_owned(),
                executable_sha256: [9; 32],
            }
        )),
        "admission/lifecycle_rule"
    );
    // It ends no grant; replay keeps it once.
    assert_eq!(
        registry.standing(uid, &synthetic_id("grant mismatch cache")),
        Ok(standing_t::current)
    );
    registry.shutdown().expect("shutdown");
    drop(registry);
    let registry = start(&root, "boot-1", &time, &io);
    assert_eq!(registry.restrict(Policyd, uid, &updated), Ok(first));
    assert_eq!(decoded(&root, uid).len(), count + 1);
}

#[test]
fn history_pages_the_journal_for_its_owner_and_admin() {
    // ADR 0008 §2: the owner's trusted client and admin only; pages of at most 16
    // records or one commit (service IPC §2); identity objects left out; this run's
    // enforcement status on each commit; pending restrictions reported.
    let root = temp_root("history");
    let time = clock();
    let io = fault_io_t::default();
    let registry = start(&root, "boot-1", &time, &io);
    let fixture = with_branch(&registry, 1001, "history", Vec::new());
    let uid = fixture.user.uid;
    registry
        .confirm_enforcement(
            Consumer,
            uid,
            &fixture.branch.commit_id,
            &[3; 16],
            enforcement_domain_t::launcher,
            1,
            enforcement_result_t::confirmed,
        )
        .expect("confirmed");
    for index in 0..20u8 {
        registry
            .session_event(
                Consumer,
                uid,
                &synthetic_id(&format!("history session {index}")),
                session_change_t::started,
            )
            .expect("started");
    }
    // A commit after ordinary records: it starts a page of its own.
    let revision = draft(
        &fixture.user,
        &fixture.app,
        commit_type_t::revision,
        "later",
    )
    .on(&fixture.genesis)
    .add(vec![file_grant(
        "history later",
        uid,
        "home:Later",
        &["read"],
        persistent_lifetime(),
    )])
    .sign(&fixture.user);
    admit(&registry, &fixture.user, &revision).expect("revision");
    let owner = caller_role_t::trusted_client { uid };
    assert_eq!(
        code(registry.history(caller_role_t::trusted_client { uid: 1002 }, uid, 0)),
        "ipc/owner_mismatch"
    );
    for caller in [Policyd, Contractd, Consumer] {
        assert_eq!(code(registry.history(caller, uid, 0)), "ipc/role_denied");
    }
    let journal = decoded(&root, uid);
    let identity_records = journal
        .iter()
        .filter(|record| matches!(record.body, record_body_t::identity { .. }))
        .count();
    let mut pages = Vec::new();
    let mut after = 0;
    loop {
        let page = registry.history(owner, uid, after).expect("page");
        assert_eq!(
            page,
            registry.history(Admin, uid, after).expect("admin page")
        );
        assert!(page.records.len() <= 16);
        let commits = page
            .records
            .iter()
            .filter(|record| matches!(record.body, record_body_t::commit_admitted { .. }))
            .count();
        assert!(commits == 0 || page.records.len() == 1, "a commit alone");
        pages.push(page.clone());
        match page.next {
            Some(next) => after = next,
            None => break,
        }
    }
    let served: Vec<&cham3_registryd::registry::history_record_t> =
        pages.iter().flat_map(|page| page.records.iter()).collect();
    assert_eq!(served.len(), journal.len() - identity_records);
    let expected: Vec<(u64, record_body_t)> = journal
        .iter()
        .filter(|record| !matches!(record.body, record_body_t::identity { .. }))
        .map(|record| (record.sequence, record.body.clone()))
        .collect();
    let got: Vec<(u64, record_body_t)> = served
        .iter()
        .map(|record| (record.sequence, record.body.clone()))
        .collect();
    assert_eq!(got, expected, "the journal, in order, byte for byte");
    let branch = served
        .iter()
        .find(|record| {
            matches!(&record.body, record_body_t::commit_admitted { commit_id, .. }
                if *commit_id == fixture.branch.commit_id)
        })
        .expect("the branch commit");
    assert_eq!(
        branch.enforcement,
        vec![(
            enforcement_domain_t::launcher,
            enforcement_result_t::confirmed
        )]
    );
    // A restrictive record not yet durable is reported, not served.
    io.journal_fails(true);
    let _ = registry.session_event(
        Consumer,
        uid,
        &synthetic_id("history session 0"),
        session_change_t::ended,
    );
    let page = registry.history(owner, uid, after).expect("page");
    assert_eq!(page.pending_restrictions, 1);
    // A durable record whose anchor update failed is not served until it is
    // anchored (M2-006-A-R01).
    io.journal_fails(false);
    io.anchor.store(true, Ordering::SeqCst);
    assert!(
        registry
            .session_event(
                Consumer,
                uid,
                &synthetic_id("history session 1"),
                session_change_t::ended,
            )
            .is_err()
    );
    assert_eq!(
        code(registry.history(owner, uid, 0)),
        "ipc/storage_unavailable"
    );
    io.anchor.store(false, Ordering::SeqCst);
    assert!(registry.history(owner, uid, 0).is_ok());
    // An unknown user has an empty history.
    let empty = registry
        .history(caller_role_t::trusted_client { uid: 1003 }, 1003, 0)
        .expect("empty");
    assert!(empty.records.is_empty() && empty.next.is_none());
}

fn write_chained(root: &Path, uid: u32, user_id: &digest_t, records: &[Vec<u8>]) {
    let mut chain = genesis_chain(user_id, uid);
    let mut bytes = header(uid, user_id);
    for (index, record) in records.iter().enumerate() {
        chain = next_chain(&chain, index as u64, record);
        bytes.extend(frame(record, &chain).expect("frame"));
    }
    fs::write(journal_path(root, uid), bytes).expect("write");
}

#[test]
fn replay_refuses_records_the_writers_would_not_write() {
    // A correctly chained record of a stage-B2 kind must still pass its rule on
    // replay (fail closed: the journal is unusable): a reservation of a persistent
    // grant, an outcome for an unknown reservation, a `session_event` end for a
    // session a grant is bound to, an outcome for an unknown commit, and an
    // identity mismatch naming the authoritative identity.
    let one = synthetic_id("session replay");
    let forgeries = |fixture: &lineage_fixture_t| {
        vec![
            registry_event_t::use_reserved {
                grant_id: synthetic_id("grant replay cache"),
                reservation_id: [1; 16],
                operation_id: [2; 16],
            }
            .into_body(),
            registry_event_t::use_completed {
                reservation_id: [1; 16],
                outcome: use_outcome_t::success,
            }
            .into_body(),
            registry_event_t::session_event {
                session_id: one,
                change: session_change_t::ended,
            }
            .into_body(),
            registry_event_t::enforcement_outcome {
                commit_id: [1; 32],
                runtime_id: [3; 16],
                domain: enforcement_domain_t::launcher,
                policy_epoch: 1,
                result: enforcement_result_t::confirmed,
                run_id: [4; 16],
            }
            .into_body(),
            record_body_t::lifecycle(lifecycle_record_t::identity_mismatch {
                lineage_id: fixture.lineage_id(),
                package_identity: fixture.app.package_identity.clone(),
                executable_sha256: fixture.app.executable_sha256,
            }),
        ]
    };
    let mut lifetime: lifetime_t = session_lifetime("replay");
    lifetime.session_id = Some(one);
    let probe_root = temp_root("replay_probe");
    let probe = {
        let registry = start(&probe_root, "boot-1", &clock(), &fault_io_t::default());
        with_branch(
            &registry,
            1001,
            "replay",
            vec![file_grant(
                "replay bound",
                1001,
                "home:R",
                &["read"],
                lifetime.clone(),
            )],
        )
    };
    for (index, forged) in forgeries(&probe).into_iter().enumerate() {
        let root = temp_root(&format!("replay_{index}"));
        let time = clock();
        let io = fault_io_t::default();
        let registry = start(&root, "boot-1", &time, &io);
        let fixture = with_branch(
            &registry,
            1001,
            "replay",
            vec![file_grant(
                "replay bound",
                1001,
                "home:R",
                &["read"],
                lifetime.clone(),
            )],
        );
        let uid = fixture.user.uid;
        registry.shutdown().expect("shutdown");
        drop(registry);
        let mut records = records_of(&root, uid);
        records.push(
            registry_record_t {
                sequence: records.len() as u64 + 1,
                time: "2026-10-13T09:01:00Z".to_owned(),
                body: forged.clone(),
            }
            .encode(),
        );
        write_chained(&root, uid, &fixture.user.identity.key_id, &records);
        let registry = start(&root, "boot-1", &time, &io);
        let unusable = registry.unusable_users().expect("users");
        assert_eq!(
            unusable,
            vec![(
                1001,
                unusable_reason_t::quarantine(quarantine_reason_t::replay_defect)
            )],
            "{forged:?}"
        );
        assert_eq!(
            code(registry.history(Admin, uid, 0)),
            "key_authority/quarantined",
            "{forged:?}"
        );
    }
}

trait into_body_t {
    fn into_body(self) -> record_body_t;
}

impl into_body_t for registry_event_t {
    fn into_body(self) -> record_body_t {
        record_body_t::event(self)
    }
}
