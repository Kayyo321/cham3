//! M2-006 stage A: the global anchor, the run marker and the restriction ledger
//! (ADR 0006 §§2–4, §6) over real files — restart decisions, per-user recovery,
//! containment, the restrictive write path, the persisted policy state and the
//! rollback status.
//!
//! Synthetic users and keys only. Each test uses its own directory under the system
//! temporary directory, owned by the (unprivileged) test UID.
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
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use builders::*;
use cham3_contract_format::object_kind_t;
use cham3_contract_format::schema::{authority_class_t, commit_type_t, grant_t};
use cham3_identityd::index::{key_scope_t, user_record_t, user_state_t};
use cham3_registryd::anchor::{
    SlotFileNames, anchor_mode_t, anchored_state_t, decode_slot, encode_slot, policy_state_t,
    run_state_t, tombstone_state_t, tombstone_t,
};
use cham3_registryd::journal::{journal_io_t, system_io_t};
use cham3_registryd::ledger::{
    EntriesFileName, ValidityFileName, entry_kind_t, previous_ledger_t, read_previous,
};
use cham3_registryd::lineage::{end_reason_t, standing_t};
use cham3_registryd::record::{lifecycle_record_t, placement_t, record_body_t, registry_record_t};
use cham3_registryd::registry::{
    quarantine_reason_t, registry_config_t, registry_t, run_decision_t, unusable_reason_t,
};

/// A store whose previous run used `boot_id` (tests simulate reboots).
fn booted(boot_id: &str) -> registry_config_t {
    registry_config_t {
        boot_id: Some(boot_id.to_owned()),
        ..registry_config_t::default()
    }
}

/// Faults at the three write boundaries of the restrictive path (ADR 0006 §3, §6).
/// A fault stays armed until it is cleared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum fault_t {
    none,
    /// Every journal frame write fails before any byte.
    journal_writes,
    /// Every ledger append fails.
    ledger_appends,
    /// Every anchor slot write fails.
    anchor_writes,
}

#[derive(Clone)]
struct fault_io_t {
    fault: Arc<Mutex<fault_t>>,
}

impl fault_io_t {
    fn new() -> fault_io_t {
        fault_io_t {
            fault: Arc::new(Mutex::new(fault_t::none)),
        }
    }

    fn set(&self, fault: fault_t) {
        *self.fault.lock().unwrap() = fault;
    }

    fn armed(&self, fault: fault_t) -> bool {
        *self.fault.lock().unwrap() == fault
    }
}

impl journal_io_t for fault_io_t {
    fn write_all(&self, file: &mut fs::File, bytes: &[u8]) -> io::Result<()> {
        if self.armed(fault_t::journal_writes) {
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

    fn replace_file(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        if self.armed(fault_t::anchor_writes) {
            return Err(io::Error::other("injected anchor write failure"));
        }
        system_io_t.replace_file(path, bytes)
    }

    fn append_file(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        if self.armed(fault_t::ledger_appends) {
            return Err(io::Error::other("injected ledger append failure"));
        }
        system_io_t.append_file(path, bytes)
    }
}

/// A genesis with one persistent grant, and a revision on it.
struct lineage_t {
    genesis: commit_t,
    revision: commit_t,
    grant: grant_t,
}

fn lineage(user: &test_user_t, app: &app_t) -> lineage_t {
    let grant = file_grant(
        "downloads read",
        user.uid,
        "home:Downloads",
        &["read"],
        persistent_lifetime(),
    );
    let genesis = draft(user, app, commit_type_t::genesis, "genesis")
        .add(vec![grant.clone()])
        .sign(user);
    let revision = draft(user, app, commit_type_t::revision, "revision")
        .on(&genesis)
        .add(vec![file_grant(
            "documents write",
            user.uid,
            "home:Documents",
            &["read", "write"],
            persistent_lifetime(),
        )])
        .sign(user);
    lineage_t {
        genesis,
        revision,
        grant,
    }
}

fn restriction(grant: &grant_t) -> lifecycle_record_t {
    lifecycle_record_t::restriction {
        grant_ids: vec![grant.grant_id],
    }
}

fn last_record(root: &Path, uid: u32) -> registry_record_t {
    registry_record_t::decode(records_of(root, uid).last().expect("a record")).expect("decodes")
}

fn ledger_dir(root: &Path) -> PathBuf {
    root.join("ledger")
}

fn anchor_dir(root: &Path) -> PathBuf {
    root.join("anchor")
}

fn ledger_entries(root: &Path) -> Vec<cham3_registryd::ledger::ledger_entry_t> {
    match read_previous(&ledger_dir(root), owner_of(root)) {
        previous_ledger_t::valid { entries, .. } => entries,
        other => panic!("ledger not valid: {other:?}"),
    }
}

/// Copies the regular files of one directory level.
fn copy_files(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("directory");
    for entry in fs::read_dir(from).expect("read_dir") {
        let entry = entry.expect("entry");
        if entry.file_type().expect("type").is_file() {
            fs::copy(entry.path(), to.join(entry.file_name())).expect("copy");
        }
    }
}

/// The anchor slot with the higher valid counter: (name, counter, map).
fn newest_slot(root: &Path) -> (String, u64, cham3_registryd::anchor::anchor_map_t) {
    SlotFileNames
        .iter()
        .filter_map(|name| {
            let bytes = fs::read(anchor_dir(root).join(name)).ok()?;
            let (counter, map) = decode_slot(&bytes)?;
            Some((name.to_string(), counter, map))
        })
        .max_by_key(|(_, counter, _)| *counter)
        .expect("a valid slot")
}

fn write_private(path: &Path, bytes: &[u8]) {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .expect("open");
    io::Write::write_all(&mut file, bytes).expect("write");
}

/// Quarantine denies everything of the user (ADR 0006 §4). With a readable journal the
/// user keeps a slot in the quarantined state (the verified prefix a `recover_user` of
/// stage C accepts): `lineage_head` then reports the state, with every grant ended;
/// without one, every read is refused.
fn quarantined_everywhere(
    registry: &Arc<registry_t>,
    user: &test_user_t,
    commit: &commit_t,
    readable: bool,
) {
    let lineage_id = commit.payload.lineage_id;
    if readable {
        let view = registry
            .lineage_head(Policyd, user.uid, &lineage_id)
            .expect("view")
            .expect("user");
        assert_eq!(view.user_state, user_state_t::quarantined);
        assert!(
            view.ended
                .values()
                .all(|reason| *reason == end_reason_t::quarantined)
                && !view.ended.is_empty(),
            "{:?}",
            view.ended
        );
    } else {
        assert_eq!(
            code(registry.lineage_head(Policyd, user.uid, &lineage_id)),
            "key_authority/quarantined"
        );
    }
    assert_eq!(
        code(admit(registry, user, commit)),
        "key_authority/quarantined"
    );
    assert_eq!(
        code(registry.append_identity(
            Identityd,
            user.uid,
            object_kind_t::key_certificate,
            &certificate(
                &user.identity,
                user.uid,
                &test_key("quarantined fresh"),
                authority_class_t::persistent,
                1,
            ),
        )),
        "key_authority/quarantined"
    );
    assert_eq!(
        code(registry.restrict(
            Admin,
            user.uid,
            &restriction(&file_grant(
                "x",
                user.uid,
                "home:x",
                &["read"],
                persistent_lifetime()
            ))
        )),
        "key_authority/quarantined"
    );
    assert_eq!(
        code(registry.identity_state(Identityd, user.uid, 0)),
        "key_authority/quarantined"
    );
    let mut identity = identity_over(registry);
    assert_eq!(
        code(identity.resolve_key(Contractd, &user.persistent.key_id)),
        "key_authority/key_not_certified",
        "a freshly started identityd holds no key of a quarantined user"
    );
}

#[test]
fn a_fresh_store_starts_resumed_and_an_orderly_shutdown_keeps_it_so() {
    // ADR 0006 §3: no run marker (first start) or a `clean` one resumes; the marker is
    // `running` while registryd serves and `clean` after an orderly shutdown. The
    // anchor is in degraded mode (same filesystem as the store; owner choice §9.1).
    let root = temp_root("resumed");
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = lineage(&alice, &editor);
    let registry = open(&root);
    assert_eq!(registry.run_decision(), run_decision_t::resumed);
    let status = registry.rollback_status(Policyd).expect("status");
    assert_eq!(status.anchor_mode, anchor_mode_t::degraded);
    assert_eq!(status.run_decision, run_decision_t::resumed);
    assert!(status.quarantined.is_empty());
    assert!(status.contained.is_empty());
    assert!(status.pending_restrictions.is_empty());
    assert!(!status.ledger_failed);
    assert_eq!(code(registry.rollback_status(Admin)), "ipc/role_denied");
    let (_, _, map) = newest_slot(&root);
    assert_eq!(
        map.run_marker.as_ref().map(|marker| marker.state),
        Some(run_state_t::running)
    );
    assert!(
        map.run_marker
            .as_ref()
            .is_some_and(|marker| !marker.boot_id.is_empty())
    );
    enroll(&registry, &alice);
    assert_eq!(
        admit(&registry, &alice, &lineage.genesis),
        Ok(placement_t::trunk)
    );
    // Every record is anchored before its reply.
    let anchored = registry
        .anchored_user(alice.uid)
        .unwrap()
        .expect("anchored");
    assert_eq!(anchored.seq, records_of(&root, alice.uid).len() as u64);
    assert_eq!(anchored.state, anchored_state_t::normal);
    assert_eq!(anchored.identity_seq, 3);
    assert_eq!(anchored.user_id, alice.identity.key_id);
    registry.shutdown().expect("clean shutdown");
    let (_, counter, map) = newest_slot(&root);
    assert_eq!(
        map.run_marker.as_ref().map(|marker| marker.state),
        Some(run_state_t::clean)
    );
    drop(registry);
    let registry = open(&root);
    assert_eq!(registry.run_decision(), run_decision_t::resumed);
    assert!(registry.unusable_users().unwrap().is_empty());
    assert_eq!(registry.anchor_counter(), Ok(counter + 1));
    assert_eq!(
        registry.standing(alice.uid, &lineage.grant.grant_id),
        Ok(standing_t::current)
    );
    assert_eq!(
        admit(&registry, &alice, &lineage.revision),
        Ok(placement_t::trunk)
    );
    // The ledger of the previous run is cleared by a start: this run's is empty.
    assert!(ledger_entries(&root).is_empty());
}

#[test]
fn a_restriction_is_denied_in_memory_and_ledgered_before_any_disk_write() {
    // ADR 0006 §3 restrictive path: in-memory denial → ledger entry → journal record →
    // anchor update → reply. With the journal write failing, the caller gets
    // `restriction_not_durable`, the denial is in force at once, the ledger holds the
    // entry, admissions wait, shutdown refuses, and the next operation retries the
    // write once it can succeed.
    let root = temp_root("restrict_retry");
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = lineage(&alice, &editor);
    let io = fault_io_t::new();
    let registry = open_with(&root, registry_config_t::default(), Box::new(io.clone()));
    enroll(&registry, &alice);
    assert_eq!(
        admit(&registry, &alice, &lineage.genesis),
        Ok(placement_t::trunk)
    );
    let records_before = records_of(&root, alice.uid).len();
    assert_eq!(
        code(registry.restrict(
            trusted_client(alice.uid),
            alice.uid,
            &restriction(&lineage.grant)
        )),
        "ipc/role_denied"
    );
    io.set(fault_t::journal_writes);
    assert_eq!(
        code(registry.restrict(Admin, alice.uid, &restriction(&lineage.grant))),
        "ipc/restriction_not_durable"
    );
    assert_eq!(
        registry.standing(alice.uid, &lineage.grant.grant_id),
        Ok(standing_t::ended(end_reason_t::restriction)),
        "denied in memory"
    );
    let entries = ledger_entries(&root);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].kind, entry_kind_t::journal_record);
    assert_eq!(entries[0].uid, alice.uid);
    assert_eq!(
        registry_record_t::decode(&entries[0].payload)
            .expect("record")
            .body,
        record_body_t::lifecycle(restriction(&lineage.grant))
    );
    assert_eq!(
        records_of(&root, alice.uid).len(),
        records_before,
        "nothing on disk"
    );
    let status = registry.rollback_status(Policyd).expect("status");
    assert_eq!(status.pending_restrictions, vec![alice.uid]);
    assert!(!status.ledger_failed);
    // Nothing is admitted and nothing is reported clean while the write is pending.
    assert_eq!(
        code(admit(&registry, &alice, &lineage.revision)),
        "ipc/restriction_not_durable"
    );
    assert_eq!(code(registry.shutdown()), "ipc/restriction_not_durable");
    assert_eq!(
        newest_slot(&root).2.run_marker.map(|marker| marker.state),
        Some(run_state_t::running)
    );
    // The write succeeds now: the next operation flushes it first, in order.
    io.set(fault_t::none);
    assert_eq!(
        admit(&registry, &alice, &lineage.revision),
        Ok(placement_t::trunk)
    );
    let records = records_of(&root, alice.uid);
    assert_eq!(records.len(), records_before + 2);
    assert_eq!(
        registry_record_t::decode(&records[records_before])
            .unwrap()
            .body,
        record_body_t::lifecycle(restriction(&lineage.grant))
    );
    assert!(matches!(
        registry_record_t::decode(&records[records_before + 1])
            .unwrap()
            .body,
        record_body_t::commit_admitted { .. }
    ));
    assert!(
        registry
            .rollback_status(Policyd)
            .unwrap()
            .pending_restrictions
            .is_empty()
    );
    assert_eq!(
        registry
            .anchored_user(alice.uid)
            .unwrap()
            .map(|entry| entry.seq),
        Some(records.len() as u64)
    );
    registry.shutdown().expect("clean now");
}

fn trusted_client(uid: u32) -> cham3_identityd::caller::caller_role_t {
    cham3_identityd::caller::caller_role_t::trusted_client { uid }
}

#[test]
fn an_unclean_run_in_the_same_boot_replays_the_ledger() {
    // ADR 0006 §3 row 2: marker `running`, same boot, a valid ledger for that run: the
    // restriction whose journal write never succeeded is re-journaled from the ledger
    // before anything is served.
    let root = temp_root("ledger_replay");
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = lineage(&alice, &editor);
    let io = fault_io_t::new();
    let registry = open_with(&root, booted("boot-1"), Box::new(io.clone()));
    enroll(&registry, &alice);
    assert_eq!(
        admit(&registry, &alice, &lineage.genesis),
        Ok(placement_t::trunk)
    );
    let records_before = records_of(&root, alice.uid).len();
    io.set(fault_t::journal_writes);
    assert_eq!(
        code(registry.restrict(Admin, alice.uid, &restriction(&lineage.grant))),
        "ipc/restriction_not_durable"
    );
    // The process dies (no shutdown) with the write still pending.
    drop(registry);
    let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
    assert_eq!(registry.run_decision(), run_decision_t::ledger_replayed);
    assert!(registry.unusable_users().unwrap().is_empty());
    assert_eq!(
        registry.standing(alice.uid, &lineage.grant.grant_id),
        Ok(standing_t::ended(end_reason_t::restriction))
    );
    let records = records_of(&root, alice.uid);
    assert_eq!(records.len(), records_before + 1);
    assert_eq!(
        last_record(&root, alice.uid).body,
        record_body_t::lifecycle(restriction(&lineage.grant))
    );
    assert_eq!(
        registry
            .anchored_user(alice.uid)
            .unwrap()
            .map(|entry| entry.seq),
        Some(records.len() as u64)
    );
    let status = registry.rollback_status(Policyd).expect("status");
    assert_eq!(status.run_decision, run_decision_t::ledger_replayed);
    assert!(status.pending_restrictions.is_empty());
    assert!(status.contained.is_empty());
    let view = registry
        .lineage_head(Policyd, alice.uid, &lineage.genesis.payload.lineage_id)
        .expect("view")
        .expect("user");
    assert_eq!(view.user_state, user_state_t::normal);
    assert_eq!(
        view.ended.get(&lineage.grant.grant_id),
        Some(&end_reason_t::restriction)
    );
    // The re-journaled record is not written twice by a further restart.
    drop(registry);
    let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
    assert_eq!(registry.run_decision(), run_decision_t::ledger_replayed);
    assert_eq!(records_of(&root, alice.uid).len(), records_before + 1);
    assert_eq!(
        admit(&registry, &alice, &lineage.revision),
        Ok(placement_t::trunk)
    );
}

#[test]
fn an_unclean_run_across_boots_contains_every_user() {
    // ADR 0006 §3 row 4: marker `running`, a different boot: the ledger is gone with
    // the boot, the run is unverifiable, every user is contained — grants admitted
    // before the restart are suspended permanently, a durable `containment` record
    // names the point, every key is denied, and nothing but the recovery ceremony
    // (stage C) lifts it.
    let root = temp_root("contained");
    let alice = test_user(1001);
    let bob = test_user(1002);
    let editor = app("editor");
    let alice_lineage = lineage(&alice, &editor);
    let bob_lineage = lineage(&bob, &editor);
    let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
    enroll(&registry, &alice);
    enroll(&registry, &bob);
    assert_eq!(
        admit(&registry, &alice, &alice_lineage.genesis),
        Ok(placement_t::trunk)
    );
    assert_eq!(
        admit(&registry, &bob, &bob_lineage.genesis),
        Ok(placement_t::trunk)
    );
    let records_before = records_of(&root, alice.uid).len();
    drop(registry);
    let registry = open_with(&root, booted("boot-2"), Box::new(system_io_t));
    assert_eq!(registry.run_decision(), run_decision_t::contained);
    assert!(registry.unusable_users().unwrap().is_empty());
    let status = registry.rollback_status(Policyd).expect("status");
    assert_eq!(status.contained, vec![alice.uid, bob.uid]);
    assert!(status.quarantined.is_empty());
    assert!(status.pending_restrictions.is_empty());
    for (user, lineage) in [(&alice, &alice_lineage), (&bob, &bob_lineage)] {
        let view = registry
            .lineage_head(Policyd, user.uid, &lineage.genesis.payload.lineage_id)
            .expect("view")
            .expect("user");
        assert_eq!(view.user_state, user_state_t::contained);
        assert_eq!(
            registry.standing(user.uid, &lineage.grant.grant_id),
            Ok(standing_t::ended(end_reason_t::suspended))
        );
        assert_eq!(
            code(admit(&registry, user, &lineage.revision)),
            "key_authority/contained"
        );
        assert_eq!(
            last_record(&root, user.uid).body,
            record_body_t::user(user_record_t::containment)
        );
        assert_eq!(records_of(&root, user.uid).len(), records_before + 1);
        let anchored = registry.anchored_user(user.uid).unwrap().expect("anchored");
        assert_eq!(anchored.state, anchored_state_t::contained);
        assert_eq!(anchored.seq, (records_before + 1) as u64);
        // identityd denies every key of a contained user (ADR 0006 §5): the resolution
        // carries the state and a denying scope; the ceremony authority is stage C.
        let mut identity = identity_over(&registry);
        let resolution = identity
            .resolve_key(Contractd, &user.persistent.key_id)
            .expect("resolved, denied");
        assert_eq!(resolution.user_state, user_state_t::contained);
        assert!(
            matches!(resolution.scope, key_scope_t::denied(_)),
            "{:?}",
            resolution.scope
        );
    }
    // Containment outlives any number of clean restarts.
    registry.shutdown().expect("clean");
    drop(registry);
    let registry = open_with(&root, booted("boot-2"), Box::new(system_io_t));
    assert_eq!(registry.run_decision(), run_decision_t::resumed);
    assert_eq!(
        registry.rollback_status(Policyd).unwrap().contained,
        vec![alice.uid, bob.uid]
    );
    assert_eq!(
        registry.standing(alice.uid, &alice_lineage.grant.grant_id),
        Ok(standing_t::ended(end_reason_t::suspended))
    );
    assert_eq!(
        code(admit(&registry, &alice, &alice_lineage.revision)),
        "key_authority/contained"
    );
    // A user enrolled after the containment is not contained.
    let carol = test_user(1003);
    let carol_lineage = lineage(&carol, &editor);
    enroll(&registry, &carol);
    assert_eq!(
        admit(&registry, &carol, &carol_lineage.genesis),
        Ok(placement_t::trunk)
    );
    assert_eq!(
        registry.rollback_status(Policyd).unwrap().contained,
        vec![alice.uid, bob.uid]
    );
}

#[test]
fn an_unclean_run_with_a_missing_or_invalid_ledger_contains_every_user() {
    // ADR 0006 §3 row 3: marker `running`, same boot, but the ledger is missing or
    // invalid (or belongs to another run): unverifiable, contained. A `clean` marker
    // resumes whatever the ledger holds.
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = lineage(&alice, &editor);
    for (label, damage) in [
        ("validity_missing", 0u8),
        ("entries_garbage", 1u8),
        ("validity_garbage", 2u8),
        ("other_run", 3u8),
    ] {
        let root = temp_root(&format!("ledger_{label}"));
        let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
        enroll(&registry, &alice);
        assert_eq!(
            admit(&registry, &alice, &lineage.genesis),
            Ok(placement_t::trunk)
        );
        drop(registry);
        match damage {
            0 => fs::remove_file(ledger_dir(&root).join(ValidityFileName)).unwrap(),
            1 => write_private(&ledger_dir(&root).join(EntriesFileName), b"C3RLxxxxxxxx"),
            2 => write_private(&ledger_dir(&root).join(ValidityFileName), b"C3RVshort"),
            _ => {
                // A valid ledger of some other run: its run id is not the marker's.
                let mut validity = Vec::new();
                validity.extend_from_slice(b"C3RV");
                validity.extend_from_slice(&[0x11; 16]);
                validity.extend_from_slice(b"boot-1");
                write_private(&ledger_dir(&root).join(ValidityFileName), &validity);
            }
        }
        let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
        assert_eq!(
            registry.run_decision(),
            run_decision_t::contained,
            "{label}"
        );
        assert_eq!(
            registry.standing(alice.uid, &lineage.grant.grant_id),
            Ok(standing_t::ended(end_reason_t::suspended)),
            "{label}"
        );
        assert_eq!(
            code(admit(&registry, &alice, &lineage.revision)),
            "key_authority/contained",
            "{label}"
        );
    }
    // The clean marker: the ledger is not consulted.
    let root = temp_root("ledger_clean");
    let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
    enroll(&registry, &alice);
    assert_eq!(
        admit(&registry, &alice, &lineage.genesis),
        Ok(placement_t::trunk)
    );
    registry.shutdown().expect("clean");
    drop(registry);
    fs::remove_file(ledger_dir(&root).join(ValidityFileName)).unwrap();
    let registry = open_with(&root, booted("boot-2"), Box::new(system_io_t));
    assert_eq!(registry.run_decision(), run_decision_t::resumed);
    assert_eq!(
        admit(&registry, &alice, &lineage.revision),
        Ok(placement_t::trunk)
    );
}

#[test]
fn a_failed_ledger_stops_eligible_decisions_and_the_next_start_contains() {
    // ADR 0006 §3 mechanism 2: when the ledger cannot be written, its validity file
    // is removed and no eligible decision is served for the rest of the run; the
    // restriction itself still reaches the journal; the next start sees an
    // unverifiable run.
    let root = temp_root("ledger_failed");
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = lineage(&alice, &editor);
    let io = fault_io_t::new();
    let registry = open_with(&root, booted("boot-1"), Box::new(io.clone()));
    enroll(&registry, &alice);
    assert_eq!(
        admit(&registry, &alice, &lineage.genesis),
        Ok(placement_t::trunk)
    );
    let lineage_id = lineage.genesis.payload.lineage_id;
    io.set(fault_t::ledger_appends);
    registry
        .restrict(Admin, alice.uid, &restriction(&lineage.grant))
        .expect("durable in the journal");
    assert_eq!(
        last_record(&root, alice.uid).body,
        record_body_t::lifecycle(restriction(&lineage.grant))
    );
    assert!(!ledger_dir(&root).join(ValidityFileName).exists());
    let status = registry.rollback_status(Policyd).expect("status");
    assert!(status.ledger_failed);
    assert!(status.pending_restrictions.is_empty());
    assert_eq!(
        code(registry.lineage_head(Policyd, alice.uid, &lineage_id)),
        "ipc/storage_unavailable"
    );
    assert_eq!(
        code(admit(&registry, &alice, &lineage.revision)),
        "ipc/storage_unavailable"
    );
    assert_eq!(code(registry.shutdown()), "ipc/restriction_not_durable");
    io.set(fault_t::none);
    // Recovered io does not revive the run's ledger.
    assert_eq!(
        code(registry.lineage_head(Policyd, alice.uid, &lineage_id)),
        "ipc/storage_unavailable"
    );
    drop(registry);
    let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
    assert_eq!(registry.run_decision(), run_decision_t::contained);
    assert_eq!(
        registry.standing(alice.uid, &lineage.grant.grant_id),
        Ok(standing_t::ended(end_reason_t::restriction)),
        "the earlier restriction keeps its own reason"
    );
    assert_eq!(
        code(admit(&registry, &alice, &lineage.revision)),
        "key_authority/contained"
    );
}

#[test]
fn a_failed_anchor_write_after_a_durable_record_denies_until_the_next_start_rolls_forward() {
    // ADR 0006 §2, §4: a slot write that fails after a durable record leaves the
    // change non-durable in the anchor; the user is denied for the rest of the run
    // (anchor and journal disagree) and the next start finds `m == a + 1` and rolls
    // the anchor forward. A restrictive change in that state is reported not durable.
    let root = temp_root("anchor_failed");
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = lineage(&alice, &editor);
    let io = fault_io_t::new();
    let registry = open_with(&root, registry_config_t::default(), Box::new(io.clone()));
    enroll(&registry, &alice);
    let anchored_before = registry
        .anchored_user(alice.uid)
        .unwrap()
        .expect("anchored");
    io.set(fault_t::anchor_writes);
    assert_eq!(
        code(admit(&registry, &alice, &lineage.genesis)),
        "ipc/storage_unavailable"
    );
    // The record is durable, the anchor is not.
    assert!(matches!(
        last_record(&root, alice.uid).body,
        record_body_t::commit_admitted { .. }
    ));
    assert_eq!(
        registry.anchored_user(alice.uid).unwrap(),
        Some(anchored_before)
    );
    io.set(fault_t::none);
    assert_eq!(
        code(admit(&registry, &alice, &lineage.revision)),
        "ipc/storage_unavailable",
        "denied until restart"
    );
    assert_eq!(
        code(registry.restrict(Admin, alice.uid, &restriction(&lineage.grant))),
        "ipc/storage_unavailable"
    );
    assert_eq!(
        registry
            .rollback_status(Policyd)
            .unwrap()
            .pending_restrictions,
        vec![alice.uid]
    );
    assert_eq!(code(registry.shutdown()), "ipc/restriction_not_durable");
    drop(registry);
    let registry = open(&root);
    assert!(registry.unusable_users().unwrap().is_empty());
    let records = records_of(&root, alice.uid).len() as u64;
    assert_eq!(
        registry
            .anchored_user(alice.uid)
            .unwrap()
            .map(|entry| entry.seq),
        Some(records),
        "rolled forward"
    );
    assert_eq!(
        registry.standing(alice.uid, &lineage.grant.grant_id),
        Ok(standing_t::current),
        "the admission was durable: reported failed, found admitted (ADR 0006 §1)"
    );
    assert_eq!(
        admit(&registry, &alice, &lineage.revision),
        Ok(placement_t::trunk)
    );
}

#[test]
fn the_policy_state_is_restrictive_persisted_and_never_lowered() {
    // ADR 0007 §1 rules 1 and 4 through ADR 0006 §3: `set_policy_epoch` raises the
    // epoch and the minimum valid epoch (never lowers either), the emergency
    // restriction is set and released, each through the ledger and the anchor; the
    // persisted state outlives restarts and is only raised by the configuration.
    let root = temp_root("policy");
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = lineage(&alice, &editor);
    let registry = open(&root);
    enroll(&registry, &alice);
    assert_eq!(
        admit(&registry, &alice, &lineage.genesis),
        Ok(placement_t::trunk)
    );
    let lineage_id = lineage.genesis.payload.lineage_id;
    let state = |epoch, min_valid_epoch, emergency_restriction| policy_state_t {
        epoch,
        min_valid_epoch,
        emergency_restriction,
    };
    assert_eq!(registry.policy_state(), Ok(state(1, 1, false)));
    assert_eq!(
        code(registry.set_policy_epoch(Policyd, 3, 2)),
        "ipc/role_denied"
    );
    assert_eq!(
        code(registry.set_policy_epoch(Admin, 2, 3)),
        "ipc/malformed_request"
    );
    assert_eq!(
        registry.set_policy_epoch(Admin, 3, 2),
        Ok(state(3, 2, false))
    );
    assert_eq!(
        registry.set_policy_epoch(Admin, 1, 1),
        Ok(state(3, 2, false)),
        "never lowered"
    );
    assert_eq!(
        code(admit(&registry, &alice, &lineage.revision)),
        "ipc/stale_epoch",
        "a commit at epoch 1"
    );
    assert_eq!(
        code(registry.set_emergency_restriction(Contractd, true)),
        "ipc/role_denied"
    );
    assert_eq!(
        registry.set_emergency_restriction(Admin, true),
        Ok(state(3, 2, true))
    );
    let view = registry
        .lineage_head(Policyd, alice.uid, &lineage_id)
        .expect("view")
        .expect("user");
    assert_eq!(
        (
            view.policy_epoch,
            view.min_valid_policy_epoch,
            view.emergency_restriction
        ),
        (3, 2, true)
    );
    assert_eq!(
        registry.rollback_status(Policyd).unwrap().policy,
        state(3, 2, true)
    );
    // Ledger entries for each change, and the anchor holds the state.
    let policy_entries = ledger_entries(&root)
        .into_iter()
        .filter(|entry| entry.kind == entry_kind_t::policy)
        .count();
    assert_eq!(policy_entries, 3);
    assert_eq!(newest_slot(&root).2.policy, state(3, 2, true));
    drop(registry);
    // A lower configuration does not lower the persisted state; a higher one raises it.
    let registry = open(&root);
    assert_eq!(registry.policy_state(), Ok(state(3, 2, true)));
    drop(registry);
    let registry = open_with(
        &root,
        registry_config_t {
            policy_epoch: 5,
            min_valid_policy_epoch: 4,
            ..registry_config_t::default()
        },
        Box::new(system_io_t),
    );
    assert_eq!(registry.policy_state(), Ok(state(5, 4, true)));
    assert_eq!(
        registry.set_emergency_restriction(Admin, false),
        Ok(state(5, 4, false))
    );
    drop(registry);
    let registry = open(&root);
    assert_eq!(registry.policy_state(), Ok(state(5, 4, false)));
    let fresh = draft(&alice, &editor, commit_type_t::revision, "epoch five")
        .on(&lineage.genesis)
        .add(vec![file_grant(
            "fresh",
            alice.uid,
            "home:Fresh",
            &["read"],
            persistent_lifetime(),
        )])
        .epoch(5)
        .sign(&alice);
    assert_eq!(admit(&registry, &alice, &fresh), Ok(placement_t::trunk));
}

#[test]
fn the_recovery_table_quarantines_rolled_back_journals_lost_stores_and_broken_anchors() {
    // ADR 0006 §4 rows: a journal behind the anchor (stale: a restored backup), a
    // journal more than one record ahead (unexplained: a restored anchor), a missing
    // journal the anchor knows (lost store), both anchor slots invalid or the anchor
    // gone while journals exist (every user). Quarantine denies everything of that
    // user and leaves the files as found; other users are unaffected.
    let root = temp_root("recovery_table");
    let alice = test_user(1001);
    let bob = test_user(1002);
    let editor = app("editor");
    let alice_lineage = lineage(&alice, &editor);
    let bob_lineage = lineage(&bob, &editor);
    let registry = open(&root);
    enroll(&registry, &alice);
    enroll(&registry, &bob);
    assert_eq!(
        admit(&registry, &alice, &alice_lineage.genesis),
        Ok(placement_t::trunk)
    );
    assert_eq!(
        admit(&registry, &bob, &bob_lineage.genesis),
        Ok(placement_t::trunk)
    );
    registry.shutdown().expect("clean");
    drop(registry);
    // State 1: four records, anchored. Keep copies.
    let keep = root.join("keep");
    copy_files(&anchor_dir(&root), &keep.join("anchor_1"));
    fs::copy(journal_path(&root, alice.uid), keep.join("journal_1")).unwrap();
    let registry = open(&root);
    assert_eq!(
        admit(&registry, &alice, &alice_lineage.revision),
        Ok(placement_t::trunk)
    );
    let later = draft(&alice, &editor, commit_type_t::revision, "later")
        .on(&alice_lineage.revision)
        .add(vec![file_grant(
            "later",
            alice.uid,
            "home:Later",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&alice);
    assert_eq!(admit(&registry, &alice, &later), Ok(placement_t::trunk));
    registry.shutdown().expect("clean");
    drop(registry);
    // State 2: six records, anchored. Keep copies.
    copy_files(&anchor_dir(&root), &keep.join("anchor_2"));
    fs::copy(journal_path(&root, alice.uid), keep.join("journal_2")).unwrap();
    let restore = |root: &Path| {
        copy_files(&keep.join("anchor_2"), &anchor_dir(root));
        let user_directory = journal_path(root, alice.uid)
            .parent()
            .unwrap()
            .to_path_buf();
        if !user_directory.exists() {
            std::os::unix::fs::DirBuilderExt::mode(&mut fs::DirBuilder::new(), 0o700)
                .create(&user_directory)
                .unwrap();
        }
        fs::copy(keep.join("journal_2"), journal_path(root, alice.uid)).unwrap();
    };
    // (label, damage, reason, every user, journals readable: slots kept)
    type damage_t<'a> = Box<dyn Fn(&Path) + 'a>;
    let cases: Vec<(&str, damage_t<'_>, Option<quarantine_reason_t>, bool, bool)> = vec![
        (
            "stale_journal",
            Box::new(|root: &Path| {
                fs::copy(keep.join("journal_1"), journal_path(root, alice.uid)).unwrap();
            }),
            Some(quarantine_reason_t::stale_journal),
            false,
            false,
        ),
        (
            "unexplained_records",
            Box::new(|root: &Path| copy_files(&keep.join("anchor_1"), &anchor_dir(root))),
            Some(quarantine_reason_t::unexplained_records),
            false,
            false,
        ),
        (
            "lost_journal_file",
            Box::new(|root: &Path| fs::remove_file(journal_path(root, alice.uid)).unwrap()),
            Some(quarantine_reason_t::lost_store),
            false,
            false,
        ),
        (
            "lost_user_directory",
            Box::new(|root: &Path| {
                fs::remove_dir_all(journal_path(root, alice.uid).parent().unwrap()).unwrap()
            }),
            Some(quarantine_reason_t::lost_store),
            false,
            false,
        ),
        (
            "both_slots_invalid",
            Box::new(|root: &Path| {
                for name in SlotFileNames {
                    write_private(&anchor_dir(root).join(name), b"C3ANgarbage");
                }
            }),
            Some(quarantine_reason_t::anchor_invalid),
            true,
            true,
        ),
        (
            "anchor_gone",
            Box::new(|root: &Path| fs::remove_dir_all(anchor_dir(root)).unwrap()),
            Some(quarantine_reason_t::anchor_invalid),
            true,
            true,
        ),
        ("restored", Box::new(|_: &Path| {}), None, false, false),
    ];
    for (label, damage, expected, everyone, readable) in cases {
        restore(&root);
        damage(&root);
        let journal_length = fs::metadata(journal_path(&root, alice.uid))
            .map(|metadata| metadata.len())
            .ok();
        let registry = open(&root);
        let unusable = registry.unusable_users().expect("users");
        match expected {
            None => {
                assert!(unusable.is_empty(), "{label}");
                assert_eq!(
                    registry.standing(alice.uid, &alice_lineage.grant.grant_id),
                    Ok(standing_t::current),
                    "{label}"
                );
            }
            Some(reason) => {
                let quarantined = |uid: u32| (uid, unusable_reason_t::quarantine(reason));
                let mut expected_unusable = vec![quarantined(alice.uid)];
                let mut expected_users = vec![(alice.uid, reason)];
                if everyone {
                    expected_unusable.push(quarantined(bob.uid));
                    expected_users.push((bob.uid, reason));
                }
                if readable {
                    assert!(unusable.is_empty(), "{label}");
                } else {
                    assert_eq!(unusable, expected_unusable, "{label}");
                }
                let status = registry.rollback_status(Policyd).unwrap();
                assert_eq!(status.quarantined, expected_users, "{label}");
                assert!(status.unusable.is_empty(), "{label}");
                quarantined_everywhere(&registry, &alice, &alice_lineage.revision, readable);
                // Left as found: evidence, never truncated or removed.
                assert_eq!(
                    fs::metadata(journal_path(&root, alice.uid))
                        .map(|metadata| metadata.len())
                        .ok(),
                    journal_length,
                    "{label}"
                );
            }
        }
        if everyone {
            assert_eq!(
                code(admit(&registry, &bob, &bob_lineage.revision)),
                "key_authority/quarantined",
                "{label}"
            );
        } else {
            // The other user is unaffected, and its admission anchors normally.
            assert_eq!(
                registry.standing(bob.uid, &bob_lineage.grant.grant_id),
                Ok(standing_t::current),
                "{label}"
            );
        }
        drop(registry);
    }
}

#[test]
fn tombstones_in_the_anchor_keep_a_deleted_users_files_out() {
    // ADR 0006 §4 tombstone rows (deletion itself is stage C): a `pending` tombstone
    // keeps the user denied until the idempotent deletion resumes; files of a
    // `deleted` user id are never loaded.
    let alice = test_user(1001);
    let bob = test_user(1002);
    let editor = app("editor");
    let alice_lineage = lineage(&alice, &editor);
    let bob_lineage = lineage(&bob, &editor);
    for (label, state, reason) in [
        (
            "pending",
            tombstone_state_t::pending,
            quarantine_reason_t::deletion_pending,
        ),
        (
            "deleted",
            tombstone_state_t::deleted,
            quarantine_reason_t::deleted_user_files,
        ),
    ] {
        let root = temp_root(&format!("tombstone_{label}"));
        let registry = open(&root);
        enroll(&registry, &alice);
        enroll(&registry, &bob);
        assert_eq!(
            admit(&registry, &alice, &alice_lineage.genesis),
            Ok(placement_t::trunk)
        );
        assert_eq!(
            admit(&registry, &bob, &bob_lineage.genesis),
            Ok(placement_t::trunk)
        );
        registry.shutdown().expect("clean");
        drop(registry);
        let (name, counter, mut map) = newest_slot(&root);
        map.tombstones.insert(
            alice.identity.key_id,
            tombstone_t {
                uid: alice.uid,
                state,
            },
        );
        let other = SlotFileNames
            .iter()
            .find(|candidate| **candidate != name)
            .expect("the other slot");
        write_private(
            &anchor_dir(&root).join(other),
            &encode_slot(counter + 1, &map).expect("encodes"),
        );
        let registry = open(&root);
        // The journal is readable, so the user keeps a slot in the quarantined state
        // (a ceremony of stage C needs it); the status lists the reason.
        assert!(registry.unusable_users().unwrap().is_empty(), "{label}");
        assert_eq!(
            registry.rollback_status(Policyd).unwrap().quarantined,
            vec![(alice.uid, reason)],
            "{label}"
        );
        quarantined_everywhere(&registry, &alice, &alice_lineage.revision, true);
        assert_eq!(
            registry.standing(alice.uid, &alice_lineage.grant.grant_id),
            Ok(standing_t::ended(end_reason_t::quarantined)),
            "{label}"
        );
        assert_eq!(
            admit(&registry, &bob, &bob_lineage.revision),
            Ok(placement_t::trunk),
            "{label}"
        );
        // The tombstone is carried by every later slot.
        assert_eq!(
            newest_slot(&root).2.tombstones.get(&alice.identity.key_id),
            Some(&tombstone_t {
                uid: alice.uid,
                state
            }),
            "{label}"
        );
    }
}

#[test]
fn the_anchor_survives_one_invalid_slot_and_rewrites_it() {
    // ADR 0006 §2: two alternating slots; startup takes the valid slot with the higher
    // counter. With the newest slot torn, the older one (one update behind: the run
    // marker still `running`) is taken, the ledger of that run replays (it is empty),
    // and the next update rewrites the torn slot.
    let root = temp_root("slot_torn");
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = lineage(&alice, &editor);
    let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
    enroll(&registry, &alice);
    assert_eq!(
        admit(&registry, &alice, &lineage.genesis),
        Ok(placement_t::trunk)
    );
    registry.shutdown().expect("clean");
    drop(registry);
    let (newest, counter, map) = newest_slot(&root);
    assert_eq!(
        map.run_marker.as_ref().map(|marker| marker.state),
        Some(run_state_t::clean)
    );
    let bytes = fs::read(anchor_dir(&root).join(&newest)).unwrap();
    write_private(&anchor_dir(&root).join(&newest), &bytes[..bytes.len() / 2]);
    let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
    assert_eq!(registry.run_decision(), run_decision_t::ledger_replayed);
    assert!(registry.unusable_users().unwrap().is_empty());
    assert_eq!(
        registry.anchor_counter(),
        Ok(counter),
        "the older slot plus the marker"
    );
    assert_eq!(
        registry.standing(alice.uid, &lineage.grant.grant_id),
        Ok(standing_t::current)
    );
    // The marker write went to the torn slot: both slots decode again.
    for name in SlotFileNames {
        assert!(
            decode_slot(&fs::read(anchor_dir(&root).join(name)).unwrap()).is_some(),
            "{name}"
        );
    }
    assert_eq!(
        admit(&registry, &alice, &lineage.revision),
        Ok(placement_t::trunk)
    );
    // An unsafe slot (group-readable) refuses the whole store at start.
    registry.shutdown().expect("clean");
    drop(registry);
    let (newest, _, _) = newest_slot(&root);
    fs::set_permissions(
        anchor_dir(&root).join(&newest),
        fs::Permissions::from_mode(0o640),
    )
    .unwrap();
    assert!(
        registry_t::open(
            &root,
            owner_of(&root),
            booted("boot-1"),
            Box::new(system_io_t),
            Box::new(fixed_clock_t),
        )
        .is_err()
    );
}

#[test]
fn an_anchor_on_another_filesystem_is_independent() {
    // ADR 0006 §2 backends: a configured anchor path whose `st_dev` differs from the
    // store's is reported `independent`; the default shares the store's filesystem
    // (`degraded`). The pinned container mounts a tmpfs at /dev/shm.
    let root = temp_root("independent");
    let shm = Path::new("/dev/shm");
    let store_device = fs::metadata(&root).unwrap().dev();
    let shm_device = fs::metadata(shm)
        .expect("/dev/shm exists in the pinned container")
        .dev();
    assert_ne!(
        store_device, shm_device,
        "/dev/shm must be a separate filesystem"
    );
    let anchor = shm.join(format!("cham3_anchor_{}", std::process::id()));
    let _ = fs::remove_dir_all(&anchor);
    let config = registry_config_t {
        anchor_path: Some(anchor.clone()),
        ..registry_config_t::default()
    };
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = lineage(&alice, &editor);
    let registry = open_with(&root, config.clone(), Box::new(system_io_t));
    assert_eq!(
        registry.rollback_status(Policyd).unwrap().anchor_mode,
        anchor_mode_t::independent
    );
    enroll(&registry, &alice);
    assert_eq!(
        admit(&registry, &alice, &lineage.genesis),
        Ok(placement_t::trunk)
    );
    assert!(anchor.join(SlotFileNames[0]).exists() || anchor.join(SlotFileNames[1]).exists());
    assert!(!anchor_dir(&root).exists(), "nothing anchored in the store");
    registry.shutdown().expect("clean");
    drop(registry);
    // The store restored without its anchor: stale, detected from the other filesystem.
    let kept = root.join("kept_journal");
    fs::copy(journal_path(&root, alice.uid), &kept).unwrap();
    let registry = open_with(&root, config.clone(), Box::new(system_io_t));
    assert_eq!(
        admit(&registry, &alice, &lineage.revision),
        Ok(placement_t::trunk)
    );
    registry.shutdown().expect("clean");
    drop(registry);
    fs::copy(&kept, journal_path(&root, alice.uid)).unwrap();
    let registry = open_with(&root, config, Box::new(system_io_t));
    assert_eq!(
        registry.unusable_users().unwrap(),
        vec![(
            alice.uid,
            unusable_reason_t::quarantine(quarantine_reason_t::stale_journal)
        )]
    );
    drop(registry);
    let _ = fs::remove_dir_all(&anchor);
}
