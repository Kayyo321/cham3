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
use cham3_contract_format::hash::sha256;
use cham3_contract_format::object_kind_t;
use cham3_contract_format::schema::{
    authority_class_t, commit_type_t, grant_t, invalidation_reason_t,
};
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
    /// Every ledger append fails, and the validity file can neither be removed nor
    /// rewritten (the anchor can be written).
    ledger_appends_and_unlink,
    /// Every ledger append fails, the validity file cannot be removed, and the anchor
    /// slots cannot be written (the validity file can be rewritten).
    ledger_appends_unlink_and_anchor,
    /// Every write fails: journal frames, ledger appends, the removal and the
    /// rewriting of the validity file, and the anchor slots (the compound failure of
    /// M2-006-A-R2-03).
    all_writes,
}

#[derive(Clone)]
struct fault_io_t {
    fault: Arc<Mutex<fault_t>>,
    /// A fault armed by the first ledger append (the start's own appends included).
    on_first_ledger_append: Arc<Mutex<Option<fault_t>>>,
    /// Journal frame writes still allowed before they fail (`None`: unlimited).
    journal_budget: Arc<Mutex<Option<usize>>>,
    /// The next anchor slot writes that fail, whatever else is armed.
    anchor_failures: Arc<Mutex<usize>>,
}

impl fault_io_t {
    fn new() -> fault_io_t {
        fault_io_t {
            fault: Arc::new(Mutex::new(fault_t::none)),
            on_first_ledger_append: Arc::new(Mutex::new(None)),
            journal_budget: Arc::new(Mutex::new(None)),
            anchor_failures: Arc::new(Mutex::new(0)),
        }
    }

    fn set(&self, fault: fault_t) {
        *self.fault.lock().unwrap() = fault;
    }

    fn arm_on_first_ledger_append(&self, fault: fault_t) {
        *self.on_first_ledger_append.lock().unwrap() = Some(fault);
    }

    fn journal_budget(&self, budget: Option<usize>) {
        *self.journal_budget.lock().unwrap() = budget;
    }

    fn fail_next_anchor_writes(&self, count: usize) {
        *self.anchor_failures.lock().unwrap() = count;
    }

    fn armed(&self, fault: fault_t) -> bool {
        *self.fault.lock().unwrap() == fault
    }
}

impl journal_io_t for fault_io_t {
    fn write_all(&self, file: &mut fs::File, bytes: &[u8]) -> io::Result<()> {
        if self.armed(fault_t::journal_writes) || self.armed(fault_t::all_writes) {
            return Err(io::Error::other("injected journal write failure"));
        }
        let mut budget = self.journal_budget.lock().unwrap();
        if let Some(remaining) = budget.as_mut() {
            if *remaining == 0 {
                return Err(io::Error::other("injected journal write budget exhausted"));
            }
            *remaining -= 1;
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
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        let refused = match *self.fault.lock().unwrap() {
            fault_t::anchor_writes | fault_t::all_writes => true,
            fault_t::ledger_appends_and_unlink => name == ValidityFileName,
            fault_t::ledger_appends_unlink_and_anchor => SlotFileNames.contains(&name),
            fault_t::none | fault_t::journal_writes | fault_t::ledger_appends => false,
        };
        if refused {
            return Err(io::Error::other("injected replace failure"));
        }
        if SlotFileNames.contains(&name) {
            let mut failures = self.anchor_failures.lock().unwrap();
            if *failures > 0 {
                *failures -= 1;
                return Err(io::Error::other("injected anchor write failure"));
            }
        }
        system_io_t.replace_file(path, bytes)
    }

    fn append_file(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        if let Some(fault) = self.on_first_ledger_append.lock().unwrap().take() {
            self.set(fault);
        }
        if matches!(
            *self.fault.lock().unwrap(),
            fault_t::ledger_appends
                | fault_t::ledger_appends_and_unlink
                | fault_t::ledger_appends_unlink_and_anchor
                | fault_t::all_writes
        ) {
            return Err(io::Error::other("injected ledger append failure"));
        }
        system_io_t.append_file(path, bytes)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        if matches!(
            *self.fault.lock().unwrap(),
            fault_t::ledger_appends_and_unlink
                | fault_t::ledger_appends_unlink_and_anchor
                | fault_t::all_writes
        ) {
            return Err(io::Error::other("injected unlink failure"));
        }
        system_io_t.remove_file(path)
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
    // invalid (or belongs to another run), or its entries file is gone while its
    // validity file survives (M2-006-A-R06): unverifiable, contained. A `clean` marker
    // resumes whatever the ledger holds.
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = lineage(&alice, &editor);
    for (label, damage) in [
        ("validity_missing", 0u8),
        ("entries_garbage", 1u8),
        ("validity_garbage", 2u8),
        ("other_run", 3u8),
        ("entries_missing", 4u8),
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
            3 => {
                // A valid ledger of some other run: its run id is not the marker's.
                let mut validity = Vec::new();
                validity.extend_from_slice(b"C3RV");
                validity.extend_from_slice(&[0x11; 16]);
                validity.extend_from_slice(b"boot-1");
                write_private(&ledger_dir(&root).join(ValidityFileName), &validity);
            }
            // The entries file is written before the validity file: a validity file
            // without it is no complete ledger.
            _ => fs::remove_file(ledger_dir(&root).join(EntriesFileName)).unwrap(),
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
    // The run marker records the failure too (the validity file is not the only
    // evidence the next start reads).
    assert_eq!(
        newest_slot(&root).2.run_marker.map(|marker| marker.state),
        Some(run_state_t::unverifiable)
    );
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
fn a_failed_anchor_write_denies_the_user_until_the_record_is_anchored() {
    // ADR 0006 §§2, 4, 6: a slot write that fails after a durable record leaves the
    // journal one record ahead of the anchor. Until that record is anchored — by the
    // retry every later operation of the user makes first, or by the roll-forward of
    // the next start — the user is denied everywhere: admissions and identity appends
    // refuse, nothing is appended beyond the record (the next start then finds
    // exactly `m == a + 1`), and no read serves the unanchored state, so that policyd
    // and identityd never see it (M2-006-A-R01).
    let root = temp_root("anchor_failed");
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = lineage(&alice, &editor);
    let lineage_id = lineage.genesis.payload.lineage_id;
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
    let records = records_of(&root, alice.uid).len();
    assert_eq!(
        registry.anchored_user(alice.uid).unwrap(),
        Some(anchored_before)
    );
    // No read serves the unanchored state, and nothing is appended beyond it.
    assert_eq!(
        code(registry.lineage_head(Policyd, alice.uid, &lineage_id)),
        "ipc/storage_unavailable"
    );
    assert_eq!(
        code(registry.identity_state(Identityd, alice.uid, 0)),
        "ipc/storage_unavailable"
    );
    assert_eq!(
        code(admit(&registry, &alice, &lineage.revision)),
        "ipc/storage_unavailable"
    );
    assert_eq!(
        code(registry.append_identity(
            Identityd,
            alice.uid,
            object_kind_t::key_certificate,
            &certificate(
                &alice.identity,
                alice.uid,
                &test_key("unanchored fresh"),
                authority_class_t::persistent,
                1,
            ),
        )),
        "ipc/storage_unavailable"
    );
    assert_eq!(records_of(&root, alice.uid).len(), records);
    assert_eq!(
        registry
            .rollback_status(Policyd)
            .unwrap()
            .pending_restrictions,
        vec![alice.uid]
    );
    assert_eq!(code(registry.shutdown()), "ipc/restriction_not_durable");
    // A freshly started identityd cannot read the user (its refresh is refused) and
    // so holds no key of the user: every key is denied as uncertified, as for a
    // quarantined user.
    let mut identity = identity_over(&registry);
    assert_eq!(
        code(identity.resolve_key(Contractd, &alice.persistent.key_id)),
        "key_authority/key_not_certified"
    );
    drop(identity);
    // The anchor can be written again: the next operation — a read included — anchors
    // the record first, then serves it.
    io.set(fault_t::none);
    let view = registry
        .lineage_head(Policyd, alice.uid, &lineage_id)
        .expect("view")
        .expect("user");
    assert_eq!(view.user_state, user_state_t::normal);
    assert_eq!(
        view.trunk_head.map(|head| head.commit_id),
        Some(lineage.genesis.commit_id)
    );
    assert_eq!(
        registry
            .anchored_user(alice.uid)
            .unwrap()
            .map(|entry| entry.seq),
        Some(records as u64)
    );
    assert!(
        registry
            .rollback_status(Policyd)
            .unwrap()
            .pending_restrictions
            .is_empty()
    );
    assert_eq!(
        admit(&registry, &alice, &lineage.revision),
        Ok(placement_t::trunk)
    );
    registry.shutdown().expect("clean");
    drop(registry);
    // The same failure followed by process death: the next start finds `m == a + 1`
    // and rolls the anchor forward — reported failed, found admitted (ADR 0006 §1).
    let root = temp_root("anchor_failed_restart");
    let registry = open_with(&root, registry_config_t::default(), Box::new(io.clone()));
    enroll(&registry, &alice);
    io.set(fault_t::anchor_writes);
    assert_eq!(
        code(admit(&registry, &alice, &lineage.genesis)),
        "ipc/storage_unavailable"
    );
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

/// One ledger frame of a `journal_record` entry (the ledger's layout, storage formats
/// §4), for entries the tests craft.
fn ledger_frame(run_id: &[u8; 16], uid: u32, payload: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(run_id);
    body.push(1);
    body.extend_from_slice(&uid.to_be_bytes());
    body.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    body.extend_from_slice(payload);
    let mut frame = Vec::new();
    frame.extend_from_slice(b"C3RL");
    frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
    frame.extend_from_slice(&body);
    frame.extend_from_slice(&sha256(&body));
    frame
}

#[test]
fn a_restriction_after_an_anchor_failure_is_held_in_memory_and_ledger_until_journaled() {
    // M2-006-A-R02 (ADR 0006 §§2–3): a restriction requested while the user's last
    // record is still unanchored is denied in memory, ledgered and kept pending — not
    // dropped with an error, and not appended beyond the unanchored record. It is
    // journaled once the anchor can be written again: by the retry of a later
    // operation, or by the next start, which rolls the admission forward and replays
    // the ledger.
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = lineage(&alice, &editor);
    let lineage_id = lineage.genesis.payload.lineage_id;
    for recover_in_run in [false, true] {
        let root = temp_root(&format!("restrict_after_anchor_{recover_in_run}"));
        let io = fault_io_t::new();
        let registry = open_with(&root, booted("boot-1"), Box::new(io.clone()));
        enroll(&registry, &alice);
        io.set(fault_t::anchor_writes);
        assert_eq!(
            code(admit(&registry, &alice, &lineage.genesis)),
            "ipc/storage_unavailable"
        );
        let records = records_of(&root, alice.uid).len();
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
        assert_eq!(
            registry_record_t::decode(&entries[0].payload)
                .expect("record")
                .body,
            record_body_t::lifecycle(restriction(&lineage.grant))
        );
        assert_eq!(
            records_of(&root, alice.uid).len(),
            records,
            "nothing is appended beyond the unanchored record"
        );
        assert_eq!(
            registry
                .rollback_status(Policyd)
                .unwrap()
                .pending_restrictions,
            vec![alice.uid]
        );
        assert_eq!(
            code(registry.lineage_head(Policyd, alice.uid, &lineage_id)),
            "ipc/storage_unavailable"
        );
        assert_eq!(code(registry.shutdown()), "ipc/restriction_not_durable");
        if recover_in_run {
            // The anchor can be written again: the next operation anchors the
            // admission, journals the restriction and anchors that too.
            io.set(fault_t::none);
            assert_eq!(
                admit(&registry, &alice, &lineage.revision),
                Ok(placement_t::trunk)
            );
            let after = records_of(&root, alice.uid);
            assert_eq!(after.len(), records + 2);
            assert_eq!(
                registry_record_t::decode(&after[records]).unwrap().body,
                record_body_t::lifecycle(restriction(&lineage.grant))
            );
            assert_eq!(
                registry.standing(alice.uid, &lineage.grant.grant_id),
                Ok(standing_t::ended(end_reason_t::restriction))
            );
            assert!(
                registry
                    .rollback_status(Policyd)
                    .unwrap()
                    .pending_restrictions
                    .is_empty()
            );
            registry.shutdown().expect("clean");
        } else {
            // The process dies first: the next start (same boot) rolls the admission
            // forward and re-journals the restriction from the ledger.
            drop(registry);
            let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
            assert_eq!(registry.run_decision(), run_decision_t::ledger_replayed);
            assert!(registry.unusable_users().unwrap().is_empty());
            assert_eq!(records_of(&root, alice.uid).len(), records + 1);
            assert_eq!(
                last_record(&root, alice.uid).body,
                record_body_t::lifecycle(restriction(&lineage.grant))
            );
            assert_eq!(
                registry.standing(alice.uid, &lineage.grant.grant_id),
                Ok(standing_t::ended(end_reason_t::restriction))
            );
            assert!(
                registry
                    .rollback_status(Policyd)
                    .unwrap()
                    .pending_restrictions
                    .is_empty()
            );
            assert_eq!(
                admit(&registry, &alice, &lineage.revision),
                Ok(placement_t::trunk)
            );
        }
    }
}

#[test]
fn a_key_invalidation_after_an_anchor_failure_is_ledgered_pending_and_replayed() {
    // The identity path of M2-006-A-R02: a key invalidation (restrictive, ADR 0006 §5)
    // requested while the user's last record is unanchored is denied in memory and
    // ledgered, not dropped; the next start rolls the admission forward, re-journals
    // it from the ledger, and the anchor's identity count then describes the journal
    // (a further start finds nothing to quarantine and writes nothing twice).
    let root = temp_root("invalidation_after_anchor");
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = lineage(&alice, &editor);
    let io = fault_io_t::new();
    let registry = open_with(&root, booted("boot-1"), Box::new(io.clone()));
    enroll(&registry, &alice);
    io.set(fault_t::anchor_writes);
    assert_eq!(
        code(admit(&registry, &alice, &lineage.genesis)),
        "ipc/storage_unavailable"
    );
    let records = records_of(&root, alice.uid).len();
    let object = invalidation(
        &alice.identity,
        &alice.persistent.key_id,
        invalidation_reason_t::compromised,
        "after anchor failure",
    );
    assert_eq!(
        code(registry.append_identity(
            Identityd,
            alice.uid,
            object_kind_t::key_invalidation,
            &object
        )),
        "ipc/restriction_not_durable"
    );
    let entries = ledger_entries(&root);
    assert_eq!(entries.len(), 1);
    assert_eq!(
        registry_record_t::decode(&entries[0].payload).unwrap().body,
        record_body_t::identity {
            kind: object_kind_t::key_invalidation,
            object: object.clone(),
        }
    );
    assert_eq!(records_of(&root, alice.uid).len(), records);
    drop(registry);
    let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
    assert_eq!(registry.run_decision(), run_decision_t::ledger_replayed);
    assert!(registry.unusable_users().unwrap().is_empty());
    assert_eq!(records_of(&root, alice.uid).len(), records + 1);
    assert_eq!(
        last_record(&root, alice.uid).body,
        record_body_t::identity {
            kind: object_kind_t::key_invalidation,
            object: object.clone(),
        }
    );
    let anchored = registry
        .anchored_user(alice.uid)
        .unwrap()
        .expect("anchored");
    assert_eq!(anchored.seq, (records + 1) as u64);
    assert_eq!(
        anchored.identity_seq, 4,
        "the enrollment, two certificates and the invalidation"
    );
    let mut identity = identity_over(&registry);
    assert_eq!(
        identity
            .resolve_key(Contractd, &alice.persistent.key_id)
            .unwrap()
            .invalidated,
        Some(invalidation_reason_t::compromised)
    );
    drop(identity);
    drop(registry);
    let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
    assert!(registry.unusable_users().unwrap().is_empty());
    assert!(
        registry
            .rollback_status(Policyd)
            .unwrap()
            .quarantined
            .is_empty()
    );
    assert_eq!(
        records_of(&root, alice.uid).len(),
        records + 1,
        "not written twice"
    );
}

#[test]
fn a_replayed_restriction_enters_the_new_runs_ledger_before_anything_is_served() {
    // M2-006-A-R03 (ADR 0006 §3 row 2): an entry re-journaled from the previous run's
    // ledger whose journal write fails again is pending in this run — and in this
    // run's ledger, so that a further unclean restart in the same boot still finds the
    // intent; no number of failed restarts loses it, and a durable record is never
    // written twice.
    let root = temp_root("replay_twice");
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
    let records = records_of(&root, alice.uid).len();
    io.set(fault_t::journal_writes);
    assert_eq!(
        code(registry.restrict(Admin, alice.uid, &restriction(&lineage.grant))),
        "ipc/restriction_not_durable"
    );
    drop(registry);
    // Second start, the journal still failing: replayed in memory, pending, and in
    // this run's ledger.
    let registry = open_with(&root, booted("boot-1"), Box::new(io.clone()));
    assert_eq!(registry.run_decision(), run_decision_t::ledger_replayed);
    assert_eq!(
        registry.standing(alice.uid, &lineage.grant.grant_id),
        Ok(standing_t::ended(end_reason_t::restriction))
    );
    assert_eq!(
        registry
            .rollback_status(Policyd)
            .unwrap()
            .pending_restrictions,
        vec![alice.uid]
    );
    assert_eq!(records_of(&root, alice.uid).len(), records);
    let entries = ledger_entries(&root);
    assert_eq!(entries.len(), 1, "this run's ledger holds the intent");
    assert_eq!(
        registry_record_t::decode(&entries[0].payload).unwrap().body,
        record_body_t::lifecycle(restriction(&lineage.grant))
    );
    assert_eq!(
        code(admit(&registry, &alice, &lineage.revision)),
        "ipc/restriction_not_durable"
    );
    drop(registry);
    // Third start, the journal writable: the intent survives and is journaled.
    io.set(fault_t::none);
    let registry = open_with(&root, booted("boot-1"), Box::new(io.clone()));
    assert_eq!(registry.run_decision(), run_decision_t::ledger_replayed);
    assert_eq!(
        registry.standing(alice.uid, &lineage.grant.grant_id),
        Ok(standing_t::ended(end_reason_t::restriction))
    );
    assert_eq!(records_of(&root, alice.uid).len(), records + 1);
    assert_eq!(
        last_record(&root, alice.uid).body,
        record_body_t::lifecycle(restriction(&lineage.grant))
    );
    assert!(
        registry
            .rollback_status(Policyd)
            .unwrap()
            .pending_restrictions
            .is_empty()
    );
    assert_eq!(
        ledger_entries(&root).len(),
        1,
        "re-ledgered before the journal write"
    );
    drop(registry);
    // A fourth unclean start: the record is held, not written twice.
    let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
    assert_eq!(registry.run_decision(), run_decision_t::ledger_replayed);
    assert_eq!(records_of(&root, alice.uid).len(), records + 1);
    assert_eq!(
        admit(&registry, &alice, &lineage.revision),
        Ok(placement_t::trunk)
    );
}

#[test]
fn a_policy_change_whose_anchor_update_failed_stays_pending_until_published() {
    // M2-006-A-R04 (ADR 0006 §3): a policy raise whose anchor update fails is in force
    // and ledgered, reported pending, published by the next successful anchor write of
    // any user, and never lost to a clean shutdown: the shutdown publishes it with the
    // clean marker, or refuses. A same-boot death before any publication leaves it to
    // the ledger, which raises it again at the next start.
    let root = temp_root("policy_pending");
    let alice = test_user(1001);
    let bob = test_user(1002);
    let editor = app("editor");
    let lineage = lineage(&alice, &editor);
    let io = fault_io_t::new();
    let registry = open_with(&root, booted("boot-1"), Box::new(io.clone()));
    enroll(&registry, &alice);
    assert_eq!(
        admit(&registry, &alice, &lineage.genesis),
        Ok(placement_t::trunk)
    );
    let state = |epoch, min_valid_epoch, emergency_restriction| policy_state_t {
        epoch,
        min_valid_epoch,
        emergency_restriction,
    };
    io.set(fault_t::anchor_writes);
    assert_eq!(
        code(registry.set_policy_epoch(Admin, 9, 9)),
        "ipc/restriction_not_durable"
    );
    assert_eq!(registry.policy_state(), Ok(state(9, 9, false)), "in force");
    let status = registry.rollback_status(Policyd).unwrap();
    assert!(status.policy_pending);
    assert_eq!(status.policy, state(9, 9, false));
    assert_eq!(
        newest_slot(&root).2.policy,
        state(1, 1, false),
        "not yet anchored"
    );
    assert_eq!(
        code(admit(&registry, &alice, &lineage.revision)),
        "ipc/stale_epoch",
        "a commit at epoch 1 is refused at once"
    );
    let entries = ledger_entries(&root);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].kind, entry_kind_t::policy);
    // Published by the next anchor write of any user.
    io.set(fault_t::none);
    enroll(&registry, &bob);
    assert_eq!(newest_slot(&root).2.policy, state(9, 9, false));
    assert!(!registry.rollback_status(Policyd).unwrap().policy_pending);
    // A pending raise at shutdown is published with the clean marker, or the
    // shutdown refuses.
    io.set(fault_t::anchor_writes);
    assert_eq!(
        code(registry.set_emergency_restriction(Admin, true)),
        "ipc/restriction_not_durable"
    );
    assert!(registry.rollback_status(Policyd).unwrap().policy_pending);
    assert_eq!(code(registry.shutdown()), "ipc/restriction_not_durable");
    io.set(fault_t::none);
    registry.shutdown().expect("clean, with the policy");
    let (_, _, map) = newest_slot(&root);
    assert_eq!(map.policy, state(9, 9, true));
    assert_eq!(
        map.run_marker.map(|marker| marker.state),
        Some(run_state_t::clean)
    );
    drop(registry);
    let registry = open_with(&root, booted("boot-1"), Box::new(io.clone()));
    assert_eq!(registry.run_decision(), run_decision_t::resumed);
    assert_eq!(registry.policy_state(), Ok(state(9, 9, true)));
    assert!(!registry.rollback_status(Policyd).unwrap().policy_pending);
    // A same-boot death with the raise still pending: the ledger raises it again.
    io.set(fault_t::anchor_writes);
    assert_eq!(
        code(registry.set_policy_epoch(Admin, 12, 12)),
        "ipc/restriction_not_durable"
    );
    drop(registry);
    io.set(fault_t::none);
    let registry = open_with(&root, booted("boot-1"), Box::new(io.clone()));
    assert_eq!(registry.run_decision(), run_decision_t::ledger_replayed);
    assert_eq!(registry.policy_state(), Ok(state(12, 12, true)));
    assert_eq!(newest_slot(&root).2.policy, state(12, 12, true));
    assert!(!registry.rollback_status(Policyd).unwrap().policy_pending);
}

#[test]
fn an_emergency_release_takes_effect_only_once_it_is_durable() {
    // M2-006-A-R05 (ADR 0007 §1 rule 1): releasing the emergency restriction is
    // permissive, so it is in force only once the anchor holds it. A release whose
    // anchor update fails leaves the restriction in force — in memory, in every
    // snapshot and at the next start — and ledgers nothing (it is no restrictive
    // intent).
    let root = temp_root("emergency_release");
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = lineage(&alice, &editor);
    let lineage_id = lineage.genesis.payload.lineage_id;
    let io = fault_io_t::new();
    let registry = open_with(&root, booted("boot-1"), Box::new(io.clone()));
    enroll(&registry, &alice);
    assert_eq!(
        admit(&registry, &alice, &lineage.genesis),
        Ok(placement_t::trunk)
    );
    let state = |emergency_restriction| policy_state_t {
        epoch: 1,
        min_valid_epoch: 1,
        emergency_restriction,
    };
    assert_eq!(
        registry.set_emergency_restriction(Admin, true),
        Ok(state(true))
    );
    let entries_before = ledger_entries(&root).len();
    let counter = registry.anchor_counter().unwrap();
    io.set(fault_t::anchor_writes);
    assert_eq!(
        code(registry.set_emergency_restriction(Admin, false)),
        "ipc/restriction_not_durable"
    );
    assert_eq!(registry.policy_state(), Ok(state(true)), "still in force");
    let view = registry
        .lineage_head(Policyd, alice.uid, &lineage_id)
        .expect("view")
        .expect("user");
    assert!(view.emergency_restriction);
    assert_eq!(registry.anchor_counter().unwrap(), counter);
    assert!(newest_slot(&root).2.policy.emergency_restriction);
    assert_eq!(
        ledger_entries(&root).len(),
        entries_before,
        "a release is no restrictive intent"
    );
    assert!(!registry.rollback_status(Policyd).unwrap().policy_pending);
    drop(registry);
    io.set(fault_t::none);
    let registry = open_with(&root, booted("boot-1"), Box::new(io.clone()));
    assert_eq!(
        registry.policy_state(),
        Ok(state(true)),
        "at the next start"
    );
    // A durable release ends the denial.
    assert_eq!(
        registry.set_emergency_restriction(Admin, false),
        Ok(state(false))
    );
    assert!(!newest_slot(&root).2.policy.emergency_restriction);
    let view = registry
        .lineage_head(Policyd, alice.uid, &lineage_id)
        .expect("view")
        .expect("user");
    assert!(!view.emergency_restriction);
}

#[test]
fn an_anchor_whose_identity_count_disagrees_with_the_journal_quarantines() {
    // M2-006-A-R07 (ADR 0006 §2): the anchor's `identity_seq` must count exactly the
    // identity records of the anchored prefix; an entry that disagrees — in either
    // direction — is no anchor of this journal and quarantines the user
    // (`identity_mismatch`). The one unanchored record of a roll-forward may itself be
    // an identity record.
    let alice = test_user(1001);
    let bob = test_user(1002);
    let editor = app("editor");
    let alice_lineage = lineage(&alice, &editor);
    let bob_lineage = lineage(&bob, &editor);
    for (label, delta) in [("one_more", 1i64), ("one_less", -1i64)] {
        let root = temp_root(&format!("identity_seq_{label}"));
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
        let entry = map.users.get_mut(&alice.uid).expect("alice anchored");
        assert_eq!(entry.identity_seq, 3);
        entry.identity_seq = (entry.identity_seq as i64 + delta) as u64;
        let other = SlotFileNames
            .iter()
            .find(|candidate| **candidate != name)
            .expect("the other slot");
        write_private(
            &anchor_dir(&root).join(other),
            &encode_slot(counter + 1, &map).expect("encodes"),
        );
        let journal_length = fs::metadata(journal_path(&root, alice.uid)).unwrap().len();
        let registry = open(&root);
        assert_eq!(
            registry.unusable_users().unwrap(),
            vec![(
                alice.uid,
                unusable_reason_t::quarantine(quarantine_reason_t::identity_mismatch)
            )],
            "{label}"
        );
        assert_eq!(
            registry.rollback_status(Policyd).unwrap().quarantined,
            vec![(alice.uid, quarantine_reason_t::identity_mismatch)],
            "{label}"
        );
        quarantined_everywhere(&registry, &alice, &alice_lineage.revision, false);
        assert_eq!(
            fs::metadata(journal_path(&root, alice.uid)).unwrap().len(),
            journal_length,
            "{label}: left as found"
        );
        assert_eq!(
            admit(&registry, &bob, &bob_lineage.revision),
            Ok(placement_t::trunk),
            "{label}"
        );
    }
    // The justified case: the unanchored record is a certificate (the anchor counts
    // the identity records before it); the start rolls the anchor forward and counts
    // it.
    let root = temp_root("identity_seq_extension");
    let registry = open(&root);
    enroll(&registry, &alice);
    registry.shutdown().expect("clean");
    drop(registry);
    let keep = root.join("keep");
    copy_files(&anchor_dir(&root), &keep);
    let registry = open(&root);
    registry
        .append_identity(
            Identityd,
            alice.uid,
            object_kind_t::key_certificate,
            &certificate(
                &alice.identity,
                alice.uid,
                &test_key("extension"),
                authority_class_t::persistent,
                1,
            ),
        )
        .expect("certificate");
    registry.shutdown().expect("clean");
    drop(registry);
    copy_files(&keep, &anchor_dir(&root));
    let registry = open(&root);
    assert!(registry.unusable_users().unwrap().is_empty());
    let anchored = registry
        .anchored_user(alice.uid)
        .unwrap()
        .expect("anchored");
    assert_eq!(
        (anchored.seq, anchored.identity_seq),
        (4, 4),
        "rolled forward with the certificate counted"
    );
}

#[test]
fn an_anchored_quarantine_or_containment_ends_every_grant_at_start() {
    // M2-006-A-R08 (ADR 0006 §§3–4): the anchor's user state is authoritative over
    // the records'; a user the anchor records as quarantined or contained — without
    // the record in the journal — starts with every grant ended (`quarantined` /
    // `suspended`) and every branch closed, exactly as the record would end them, so
    // that no snapshot carries a current grant of a denied user.
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = lineage(&alice, &editor);
    for (label, state) in [
        ("quarantined", anchored_state_t::quarantined),
        ("contained", anchored_state_t::contained),
    ] {
        let root = temp_root(&format!("anchored_{label}"));
        let registry = open(&root);
        enroll(&registry, &alice);
        assert_eq!(
            admit(&registry, &alice, &lineage.genesis),
            Ok(placement_t::trunk)
        );
        registry.shutdown().expect("clean");
        drop(registry);
        let (name, counter, mut map) = newest_slot(&root);
        map.users.get_mut(&alice.uid).expect("anchored").state = state;
        write_private(
            &anchor_dir(&root).join(&name),
            &encode_slot(counter + 1, &map).expect("encodes"),
        );
        let registry = open(&root);
        assert!(registry.unusable_users().unwrap().is_empty(), "{label}");
        let status = registry.rollback_status(Policyd).unwrap();
        if state == anchored_state_t::quarantined {
            assert_eq!(
                status.quarantined,
                vec![(alice.uid, quarantine_reason_t::anchored)]
            );
            quarantined_everywhere(&registry, &alice, &lineage.revision, true);
            assert_eq!(
                registry.standing(alice.uid, &lineage.grant.grant_id),
                Ok(standing_t::ended(end_reason_t::quarantined))
            );
        } else {
            assert_eq!(status.contained, vec![alice.uid]);
            let view = registry
                .lineage_head(Policyd, alice.uid, &lineage.genesis.payload.lineage_id)
                .expect("view")
                .expect("user");
            assert_eq!(view.user_state, user_state_t::contained);
            assert_eq!(
                view.ended.get(&lineage.grant.grant_id),
                Some(&end_reason_t::suspended)
            );
            assert_eq!(
                registry.standing(alice.uid, &lineage.grant.grant_id),
                Ok(standing_t::ended(end_reason_t::suspended))
            );
            assert_eq!(
                code(admit(&registry, &alice, &lineage.revision)),
                "key_authority/contained"
            );
        }
    }
}

#[test]
fn a_failed_ledger_marks_the_run_unverifiable_even_if_its_validity_file_survives() {
    // ADR 0006 §3 mechanism 2, the combined failure the stage-A review left open:
    // when the ledger append fails and its validity file cannot be removed either,
    // the run marker records the run as unverifiable, so that the next start contains
    // whatever the ledger files hold; when the anchor cannot be written either, the
    // validity file is emptied instead (an empty validity file is invalid). In every
    // case the next start contains.
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = lineage(&alice, &editor);
    for (label, fault, marker_written) in [
        ("unlink_fails", fault_t::ledger_appends_and_unlink, true),
        (
            "unlink_and_anchor_fail",
            fault_t::ledger_appends_unlink_and_anchor,
            false,
        ),
    ] {
        let root = temp_root(&format!("ledger_failed_{label}"));
        let io = fault_io_t::new();
        let registry = open_with(&root, booted("boot-1"), Box::new(io.clone()));
        enroll(&registry, &alice);
        assert_eq!(
            admit(&registry, &alice, &lineage.genesis),
            Ok(placement_t::trunk)
        );
        let counter = registry.anchor_counter().unwrap();
        io.set(fault);
        let result = registry.restrict(Admin, alice.uid, &restriction(&lineage.grant));
        if marker_written {
            result.expect("durable in the journal, anchored");
        } else {
            assert_eq!(code(result), "ipc/restriction_not_durable", "{label}");
        }
        assert_eq!(
            last_record(&root, alice.uid).body,
            record_body_t::lifecycle(restriction(&lineage.grant)),
            "{label}"
        );
        let validity = ledger_dir(&root).join(ValidityFileName);
        assert!(validity.exists(), "{label}: the validity file survives");
        assert!(
            registry.rollback_status(Policyd).unwrap().ledger_failed,
            "{label}"
        );
        if marker_written {
            let (_, newest, map) = newest_slot(&root);
            assert!(newest > counter, "{label}");
            assert_eq!(
                map.run_marker.map(|marker| marker.state),
                Some(run_state_t::unverifiable),
                "{label}"
            );
            assert_eq!(
                fs::metadata(&validity).unwrap().len(),
                26,
                "{label}: the validity file is intact"
            );
        } else {
            assert_eq!(registry.anchor_counter().unwrap(), counter, "{label}");
            assert_eq!(
                fs::metadata(&validity).unwrap().len(),
                0,
                "{label}: emptied instead"
            );
            assert_eq!(
                read_previous(&ledger_dir(&root), owner_of(&root)),
                previous_ledger_t::invalid,
                "{label}"
            );
        }
        assert_eq!(
            code(registry.shutdown()),
            "ipc/restriction_not_durable",
            "{label}"
        );
        io.set(fault_t::none);
        assert_eq!(
            code(registry.shutdown()),
            "ipc/restriction_not_durable",
            "{label}: a failed ledger never shuts down clean"
        );
        drop(registry);
        let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
        assert_eq!(
            registry.run_decision(),
            run_decision_t::contained,
            "{label}"
        );
        assert_eq!(
            code(admit(&registry, &alice, &lineage.revision)),
            "key_authority/contained",
            "{label}"
        );
    }
}

#[test]
fn anchor_slots_with_equal_counters_or_of_another_store_quarantine_every_user() {
    // ADR 0006 §2: the writer alternates the slots with strictly increasing counters,
    // so two valid slots with the same counter never come from this store's writer:
    // neither is trusted and every user is quarantined (`anchor_invalid`). A valid
    // anchor of another store carries no entry that describes this store's journals:
    // every user it knows is quarantined for its lost store, every user it does not
    // know for unexplained records.
    let alice = test_user(1001);
    let bob = test_user(1002);
    let editor = app("editor");
    let alice_lineage = lineage(&alice, &editor);
    let bob_lineage = lineage(&bob, &editor);
    let root = temp_root("equal_counters");
    let registry = open(&root);
    enroll(&registry, &alice);
    assert_eq!(
        admit(&registry, &alice, &alice_lineage.genesis),
        Ok(placement_t::trunk)
    );
    registry.shutdown().expect("clean");
    drop(registry);
    let (name, counter, mut map) = newest_slot(&root);
    map.policy.epoch += 1;
    let other = SlotFileNames
        .iter()
        .find(|candidate| **candidate != name)
        .expect("the other slot");
    write_private(
        &anchor_dir(&root).join(other),
        &encode_slot(counter, &map).expect("encodes"),
    );
    let registry = open(&root);
    assert!(
        registry.unusable_users().unwrap().is_empty(),
        "readable journals keep their slots"
    );
    assert_eq!(
        registry.rollback_status(Policyd).unwrap().quarantined,
        vec![(alice.uid, quarantine_reason_t::anchor_invalid)]
    );
    quarantined_everywhere(&registry, &alice, &alice_lineage.revision, true);
    drop(registry);
    // A valid anchor of another store.
    let foreign = temp_root("foreign_store");
    let registry = open(&foreign);
    enroll(&registry, &bob);
    assert_eq!(
        admit(&registry, &bob, &bob_lineage.genesis),
        Ok(placement_t::trunk)
    );
    registry.shutdown().expect("clean");
    drop(registry);
    let root = temp_root("foreign_anchor");
    let registry = open(&root);
    enroll(&registry, &alice);
    assert_eq!(
        admit(&registry, &alice, &alice_lineage.genesis),
        Ok(placement_t::trunk)
    );
    registry.shutdown().expect("clean");
    drop(registry);
    copy_files(&anchor_dir(&foreign), &anchor_dir(&root));
    let registry = open(&root);
    assert_eq!(
        registry.unusable_users().unwrap(),
        vec![
            (
                alice.uid,
                unusable_reason_t::quarantine(quarantine_reason_t::unexplained_records)
            ),
            (
                bob.uid,
                unusable_reason_t::quarantine(quarantine_reason_t::lost_store)
            ),
        ]
    );
    quarantined_everywhere(&registry, &alice, &alice_lineage.revision, false);
}

#[test]
fn ledger_entries_of_unknown_held_or_quarantined_users_are_dropped_or_skipped() {
    // ADR 0006 §3 row 2, the entries the stage-A review asked about: an entry whose
    // record the journal already holds (the process died between the journal write
    // and the anchor update) is not written twice; an entry for a user the store does
    // not know is dropped (there is nothing to deny); an entry for a quarantined user
    // leaves that user's journal as found (every grant and key of the user is denied
    // already); and a ledger whose entries fail their checksum is no evidence at all:
    // the run is contained.
    let alice = test_user(1001);
    let bob = test_user(1002);
    let editor = app("editor");
    let alice_lineage = lineage(&alice, &editor);
    let bob_lineage = lineage(&bob, &editor);
    let root = temp_root("ledger_entries");
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
    registry.shutdown().expect("clean");
    drop(registry);
    // Bob is quarantined with a readable journal (a pending tombstone).
    let (name, counter, mut map) = newest_slot(&root);
    map.tombstones.insert(
        bob.identity.key_id,
        tombstone_t {
            uid: bob.uid,
            state: tombstone_state_t::pending,
        },
    );
    write_private(
        &anchor_dir(&root).join(&name),
        &encode_slot(counter + 1, &map).expect("encodes"),
    );
    let io = fault_io_t::new();
    let registry = open_with(&root, booted("boot-1"), Box::new(io.clone()));
    assert_eq!(
        registry.rollback_status(Policyd).unwrap().quarantined,
        vec![(bob.uid, quarantine_reason_t::deletion_pending)]
    );
    // Alice's restriction reaches the journal but not the anchor: held by the
    // journal, listed in the ledger.
    io.set(fault_t::anchor_writes);
    assert_eq!(
        code(registry.restrict(Admin, alice.uid, &restriction(&alice_lineage.grant))),
        "ipc/restriction_not_durable"
    );
    let alice_records = records_of(&root, alice.uid).len();
    assert_eq!(
        last_record(&root, alice.uid).body,
        record_body_t::lifecycle(restriction(&alice_lineage.grant))
    );
    drop(registry);
    // Two crafted entries of this run: one for an unknown user, one for bob.
    let validity = fs::read(ledger_dir(&root).join(ValidityFileName)).unwrap();
    let run_id: [u8; 16] = validity[4..20].try_into().unwrap();
    let mut entries = fs::read(ledger_dir(&root).join(EntriesFileName)).unwrap();
    for (uid, grant) in [
        (4242u32, &alice_lineage.grant),
        (bob.uid, &bob_lineage.grant),
    ] {
        let record = registry_record_t {
            sequence: 99,
            time: "2026-10-13T09:00:00Z".to_owned(),
            body: record_body_t::lifecycle(restriction(grant)),
        }
        .encode();
        entries.extend(ledger_frame(&run_id, uid, &record));
    }
    write_private(&ledger_dir(&root).join(EntriesFileName), &entries);
    let bob_length = fs::metadata(journal_path(&root, bob.uid)).unwrap().len();
    let io = fault_io_t::new();
    let registry = open_with(&root, booted("boot-1"), Box::new(io.clone()));
    assert_eq!(registry.run_decision(), run_decision_t::ledger_replayed);
    assert!(registry.unusable_users().unwrap().is_empty());
    assert_eq!(
        records_of(&root, alice.uid).len(),
        alice_records,
        "held: not written twice"
    );
    assert_eq!(
        registry.standing(alice.uid, &alice_lineage.grant.grant_id),
        Ok(standing_t::ended(end_reason_t::restriction))
    );
    assert_eq!(
        registry
            .anchored_user(alice.uid)
            .unwrap()
            .map(|entry| entry.seq),
        Some(alice_records as u64),
        "rolled forward"
    );
    assert_eq!(
        fs::metadata(journal_path(&root, bob.uid)).unwrap().len(),
        bob_length,
        "a quarantined user's journal is left as found"
    );
    assert_eq!(
        registry.rollback_status(Policyd).unwrap().quarantined,
        vec![(bob.uid, quarantine_reason_t::deletion_pending)]
    );
    assert!(
        registry
            .rollback_status(Policyd)
            .unwrap()
            .pending_restrictions
            .is_empty()
    );
    assert!(!registry.journal_users(Identityd).unwrap().contains(&4242));
    // A ledger whose entries fail their checksum: no evidence; contained.
    io.set(fault_t::journal_writes);
    assert_eq!(
        code(registry.restrict(Admin, alice.uid, &restriction(&alice_lineage.grant))),
        "ipc/restriction_not_durable"
    );
    drop(registry);
    let entries_path = ledger_dir(&root).join(EntriesFileName);
    let mut entries = fs::read(&entries_path).unwrap();
    let last = entries.len() - 1;
    entries[last] ^= 0x01;
    write_private(&entries_path, &entries);
    let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
    assert_eq!(registry.run_decision(), run_decision_t::contained);
    assert_eq!(
        code(admit(&registry, &alice, &alice_lineage.revision)),
        "key_authority/contained"
    );
}

#[test]
fn concurrent_policy_changes_are_serialized_and_the_anchor_holds_the_latest_state() {
    // ADR 0006 §2: the policy state in force is published under its lock, so that no
    // older copy overwrites a newer publication, whichever thread writes the slot:
    // after concurrent raises, emergency changes and user anchor writes, the anchor
    // holds exactly the state in force, and nothing is pending.
    let root = temp_root("policy_race");
    let registry = open(&root);
    let raiser = {
        let registry = Arc::clone(&registry);
        std::thread::spawn(move || {
            for epoch in 2..=24u64 {
                registry
                    .set_policy_epoch(Admin, epoch, epoch - 1)
                    .expect("raise");
            }
        })
    };
    let toggler = {
        let registry = Arc::clone(&registry);
        std::thread::spawn(move || {
            for _ in 0..12 {
                registry
                    .set_emergency_restriction(Admin, true)
                    .expect("set");
                registry
                    .set_emergency_restriction(Admin, false)
                    .expect("release");
            }
        })
    };
    let enroller = {
        let registry = Arc::clone(&registry);
        std::thread::spawn(move || {
            for uid in 2001..2007u32 {
                enroll(&registry, &test_user(uid));
            }
        })
    };
    raiser.join().expect("raiser");
    toggler.join().expect("toggler");
    enroller.join().expect("enroller");
    let policy = registry.policy_state().unwrap();
    assert_eq!(
        (
            policy.epoch,
            policy.min_valid_epoch,
            policy.emergency_restriction
        ),
        (24, 23, false)
    );
    assert_eq!(newest_slot(&root).2.policy, policy);
    assert!(!registry.rollback_status(Policyd).unwrap().policy_pending);
    assert_eq!(registry.journal_users(Identityd).unwrap().len(), 6);
    registry.shutdown().expect("clean");
}

#[test]
fn rejected_anchor_slots_are_outranked_and_the_quarantine_holds_at_every_later_start() {
    // M2-006-A-R2-01 (ADR 0006 §§2, 4): when both slots are rejected — two valid
    // slots with equal counters, or two invalid ones — the start continues the
    // counter above the highest one they carried, so that its own writes outrank
    // every rejected image, and anchors the quarantine of every user in the fresh
    // anchor. A rejected slot that outranked the fresh anchor at the next start would
    // revive the state it holds; a quarantine that was not anchored would be gone
    // with it. Both slot orders, an unclean restart and a clean one.
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = lineage(&alice, &editor);
    for (label, newest_first, corrupt) in [
        ("equal_a_newest", true, false),
        ("equal_b_newest", false, false),
        ("both_invalid", true, true),
    ] {
        let root = temp_root(&format!("rejected_slots_{label}"));
        let registry = open(&root);
        enroll(&registry, &alice);
        assert_eq!(
            admit(&registry, &alice, &lineage.genesis),
            Ok(placement_t::trunk)
        );
        registry.shutdown().expect("clean");
        drop(registry);
        let (_, counter, map) = newest_slot(&root);
        let equal = counter + 100;
        let (first, second) = if newest_first {
            (SlotFileNames[0], SlotFileNames[1])
        } else {
            (SlotFileNames[1], SlotFileNames[0])
        };
        let image = encode_slot(equal, &map).expect("encodes");
        if corrupt {
            let mut torn = image.clone();
            torn[8] ^= 0x40;
            write_private(&anchor_dir(&root).join(first), &torn);
            write_private(&anchor_dir(&root).join(second), &image[..image.len() - 1]);
        } else {
            write_private(&anchor_dir(&root).join(first), &image);
            write_private(&anchor_dir(&root).join(second), &image);
        }
        let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
        assert_eq!(
            registry.rollback_status(Policyd).unwrap().quarantined,
            vec![(alice.uid, quarantine_reason_t::anchor_invalid)],
            "{label}"
        );
        assert_eq!(
            registry.standing(alice.uid, &lineage.grant.grant_id),
            Ok(standing_t::ended(end_reason_t::quarantined)),
            "{label}"
        );
        // The fresh anchor outranks everything the rejected slots carried and names
        // the user quarantined; the newest image on disk is this run's.
        // (Two invalid slots carry no readable counter: the fresh anchor then starts
        // from zero, and nothing on disk can outrank it once they are removed.)
        let floor = if corrupt { 0 } else { equal };
        let fresh = registry.anchor_counter().unwrap();
        assert!(
            fresh > floor,
            "{label}: counter {fresh} continues above {floor}"
        );
        assert_eq!(
            registry
                .anchored_user(alice.uid)
                .unwrap()
                .map(|entry| entry.state),
            Some(anchored_state_t::quarantined),
            "{label}"
        );
        let (_, newest, map) = newest_slot(&root);
        assert!(newest > floor, "{label}: the newest slot is this run's");
        assert_eq!(
            map.users.get(&alice.uid).map(|entry| entry.state),
            Some(anchored_state_t::quarantined),
            "{label}"
        );
        // An unclean restart in the same boot, then a clean one: the quarantine holds.
        drop(registry);
        for clean in [false, true] {
            let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
            assert_eq!(
                registry.rollback_status(Policyd).unwrap().quarantined,
                vec![(alice.uid, quarantine_reason_t::anchored)],
                "{label}: clean={clean}"
            );
            assert_eq!(
                registry.standing(alice.uid, &lineage.grant.grant_id),
                Ok(standing_t::ended(end_reason_t::quarantined)),
                "{label}: clean={clean}"
            );
            quarantined_everywhere(&registry, &alice, &lineage.revision, true);
            if clean {
                registry.shutdown().expect("clean");
            }
            drop(registry);
        }
    }
}

#[test]
fn a_ledger_failure_during_the_start_leaves_the_run_unverifiable() {
    // M2-006-A-R2-02 (ADR 0006 §3): the start replaces the previous run's ledger
    // before it re-journals that ledger's entries; an intent that cannot enter this
    // run's ledger would otherwise be lost to a further same-boot restart, which would
    // find a complete-looking empty ledger and a `running` marker. So a ledger that
    // failed during the start makes this run unverifiable from its first moment: the
    // run marker is written `unverifiable`, never `running`, and when that write
    // fails too the start is refused. Either way the next start contains.
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = lineage(&alice, &editor);
    for (label, fault, start_succeeds, reason) in [
        // The ledger fails and keeps its validity file; the journal and the anchor
        // work: the re-journaled restriction is durable, the marker says unverifiable.
        (
            "validity_intact",
            fault_t::ledger_appends_and_unlink,
            true,
            end_reason_t::restriction,
        ),
        // Every write fails: the ledger files look complete, the restriction stays in
        // memory, the marker cannot be written, so the start is refused; the next
        // start finds a marker of another run than the ledger and contains.
        (
            "every_write_fails",
            fault_t::all_writes,
            false,
            end_reason_t::suspended,
        ),
    ] {
        let root = temp_root(&format!("start_ledger_failure_{label}"));
        let io = fault_io_t::new();
        let registry = open_with(&root, booted("boot-1"), Box::new(io.clone()));
        enroll(&registry, &alice);
        assert_eq!(
            admit(&registry, &alice, &lineage.genesis),
            Ok(placement_t::trunk)
        );
        // A restriction whose journal write failed: in memory and in the ledger only.
        io.set(fault_t::journal_writes);
        assert_eq!(
            code(registry.restrict(Admin, alice.uid, &restriction(&lineage.grant))),
            "ipc/restriction_not_durable",
            "{label}"
        );
        assert_eq!(ledger_entries(&root).len(), 1, "{label}");
        drop(registry);
        // The same boot restarts; the first append to this run's ledger (the
        // re-journaled restriction) fails, and the validity file cannot be
        // invalidated.
        let io = fault_io_t::new();
        io.arm_on_first_ledger_append(fault);
        let started = registry_t::open(
            &root,
            owner_of(&root),
            booted("boot-1"),
            Box::new(io.clone()),
            Box::new(fixed_clock_t),
        );
        match started {
            Ok(registry) => {
                assert!(start_succeeds, "{label}");
                assert_eq!(registry.run_decision(), run_decision_t::ledger_replayed);
                let status = registry.rollback_status(Policyd).unwrap();
                assert!(status.ledger_failed, "{label}");
                // The journal itself works in this run: the re-journaled restriction
                // is durable there; only the ledger lost it.
                assert_eq!(
                    last_record(&root, alice.uid).body,
                    record_body_t::lifecycle(restriction(&lineage.grant)),
                    "{label}"
                );
                assert_eq!(
                    registry.standing(alice.uid, &lineage.grant.grant_id),
                    Ok(standing_t::ended(end_reason_t::restriction)),
                    "{label}: denied in memory"
                );
                assert_eq!(
                    code(registry.lineage_head(
                        Policyd,
                        alice.uid,
                        &lineage.genesis.payload.lineage_id
                    )),
                    "ipc/storage_unavailable",
                    "{label}"
                );
                let (_, _, map) = newest_slot(&root);
                assert_eq!(
                    map.run_marker.map(|marker| marker.state),
                    Some(run_state_t::unverifiable),
                    "{label}: the marker of this run says unverifiable"
                );
                assert_eq!(
                    code(registry.shutdown()),
                    "ipc/restriction_not_durable",
                    "{label}"
                );
                drop(registry);
            }
            Err(error) => {
                assert!(!start_succeeds, "{label}: {error:?}");
            }
        }
        assert!(
            matches!(
                read_previous(&ledger_dir(&root), owner_of(&root)),
                previous_ledger_t::valid { ref entries, .. } if entries.is_empty()
            ),
            "{label}: the ledger files look complete"
        );
        // The next start of the same boot contains: the marker is unverifiable, or it
        // belongs to another run than the ledger. (The restriction itself reached the
        // journal in the failed run, so the grant keeps that first reason.)
        let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
        assert_eq!(
            registry.run_decision(),
            run_decision_t::contained,
            "{label}"
        );
        assert_eq!(
            registry.rollback_status(Policyd).unwrap().contained,
            vec![alice.uid],
            "{label}"
        );
        assert_eq!(
            registry.standing(alice.uid, &lineage.grant.grant_id),
            Ok(standing_t::ended(reason)),
            "{label}"
        );
    }
}

#[test]
fn a_queue_of_restrictions_is_journaled_and_anchored_one_record_at_a_time() {
    // M2-006-A-R2-04 (ADR 0006 §§2–4): the pending queue is drained one record at a
    // time, each anchored before the next is appended, so that at most one record is
    // ever unanchored whatever fails next: a failed anchor update leaves exactly one
    // durable unanchored record (a same-boot restart rolls it forward and replays the
    // rest from the ledger, no quarantine), and a journal failure in the middle of the
    // queue leaves the records already appended anchored and served.
    let alice = test_user(1001);
    let editor = app("editor");
    let grants = vec![
        file_grant(
            "queue first",
            alice.uid,
            "home:Downloads",
            &["read"],
            persistent_lifetime(),
        ),
        file_grant(
            "queue second",
            alice.uid,
            "home:Documents",
            &["read"],
            persistent_lifetime(),
        ),
        file_grant(
            "queue third",
            alice.uid,
            "home:Pictures",
            &["read"],
            persistent_lifetime(),
        ),
    ];
    let genesis = draft(&alice, &editor, commit_type_t::genesis, "queue genesis")
        .add(grants.clone())
        .sign(&alice);
    let lineage_id = genesis.payload.lineage_id;
    for anchor_fails in [true, false] {
        let root = temp_root(&format!("queue_drain_{anchor_fails}"));
        let io = fault_io_t::new();
        let registry = open_with(&root, booted("boot-1"), Box::new(io.clone()));
        enroll(&registry, &alice);
        assert_eq!(admit(&registry, &alice, &genesis), Ok(placement_t::trunk));
        let before = records_of(&root, alice.uid).len();
        io.set(fault_t::journal_writes);
        for grant in &grants[..2] {
            assert_eq!(
                code(registry.restrict(Admin, alice.uid, &restriction(grant))),
                "ipc/restriction_not_durable"
            );
        }
        assert_eq!(ledger_entries(&root).len(), 2);
        assert_eq!(records_of(&root, alice.uid).len(), before);
        if anchor_fails {
            // The journal recovers, the anchor does not: one record is appended and
            // stays unanchored; the second waits behind it.
            io.set(fault_t::anchor_writes);
            assert_eq!(code(registry.shutdown()), "ipc/restriction_not_durable");
            assert_eq!(records_of(&root, alice.uid).len(), before + 1);
            assert_eq!(
                registry.anchored_user(alice.uid).unwrap().unwrap().seq as usize,
                before,
                "exactly one unanchored record"
            );
            assert_eq!(
                code(registry.lineage_head(Policyd, alice.uid, &lineage_id)),
                "ipc/storage_unavailable",
                "nothing of the unanchored state is served"
            );
            assert_eq!(
                registry
                    .rollback_status(Policyd)
                    .unwrap()
                    .pending_restrictions,
                vec![alice.uid]
            );
            drop(registry);
            let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
            assert_eq!(registry.run_decision(), run_decision_t::ledger_replayed);
            assert!(
                registry
                    .rollback_status(Policyd)
                    .unwrap()
                    .quarantined
                    .is_empty()
            );
            assert_eq!(records_of(&root, alice.uid).len(), before + 2);
            for grant in &grants[..2] {
                assert_eq!(
                    registry.standing(alice.uid, &grant.grant_id),
                    Ok(standing_t::ended(end_reason_t::restriction))
                );
            }
            let view = registry
                .lineage_head(Policyd, alice.uid, &lineage_id)
                .unwrap()
                .unwrap();
            assert_eq!(view.journal_seq as usize, before + 2);
            registry.shutdown().expect("clean");
        } else {
            // The journal takes one frame, then fails again: the appended record is
            // anchored and served, the other stays pending.
            io.set(fault_t::none);
            io.journal_budget(Some(1));
            assert_eq!(code(registry.shutdown()), "ipc/restriction_not_durable");
            assert_eq!(records_of(&root, alice.uid).len(), before + 1);
            assert_eq!(
                registry.anchored_user(alice.uid).unwrap().unwrap().seq as usize,
                before + 1,
                "the appended record is anchored before the next append"
            );
            let view = registry
                .lineage_head(Policyd, alice.uid, &lineage_id)
                .unwrap()
                .unwrap();
            assert_eq!(view.journal_seq as usize, before + 1);
            assert_eq!(
                view.ended.get(&grants[0].grant_id),
                Some(&end_reason_t::restriction)
            );
            assert_eq!(
                registry.standing(alice.uid, &grants[1].grant_id),
                Ok(standing_t::ended(end_reason_t::restriction)),
                "still denied in memory"
            );
            assert_eq!(
                registry
                    .rollback_status(Policyd)
                    .unwrap()
                    .pending_restrictions,
                vec![alice.uid]
            );
            io.journal_budget(None);
            registry
                .shutdown()
                .expect("the rest of the queue, then clean");
            assert_eq!(records_of(&root, alice.uid).len(), before + 2);
            assert_eq!(
                registry.anchored_user(alice.uid).unwrap().unwrap().seq as usize,
                before + 2
            );
        }
    }
}

#[test]
fn a_compound_storage_failure_keeps_retrying_the_ledgers_invalidation_and_the_marker() {
    // M2-006-A-R2-03 (ADR 0006 §3): when the ledger append, the invalidation of its
    // validity file, the anchor and the restrictive journal write all fail, the
    // restriction is held in memory and in the pending queue, the run owes its
    // `unverifiable` marker and the ledger its invalidation, and every later
    // operation retries both until one succeeds; from then on a same-boot restart
    // contains. The window before the first successful write is the limit the owner
    // accepted as ADR 0006 §3 erratum E5 (storage formats §7): a process death inside
    // it loses the restriction to a same-boot restart (a reboot contains); this test
    // records the window exactly.
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = lineage(&alice, &editor);
    let lineage_id = lineage.genesis.payload.lineage_id;
    for recovers in [true, false] {
        let root = temp_root(&format!("compound_failure_{recovers}"));
        let io = fault_io_t::new();
        let registry = open_with(&root, booted("boot-1"), Box::new(io.clone()));
        enroll(&registry, &alice);
        assert_eq!(
            admit(&registry, &alice, &lineage.genesis),
            Ok(placement_t::trunk)
        );
        let counter = registry.anchor_counter().unwrap();
        io.set(fault_t::all_writes);
        assert_eq!(
            code(registry.restrict(Admin, alice.uid, &restriction(&lineage.grant))),
            "ipc/restriction_not_durable"
        );
        assert_eq!(
            registry.standing(alice.uid, &lineage.grant.grant_id),
            Ok(standing_t::ended(end_reason_t::restriction)),
            "denied in memory"
        );
        let status = registry.rollback_status(Policyd).unwrap();
        assert!(status.ledger_failed);
        assert!(status.unverifiable_owed, "both writes are owed");
        assert_eq!(status.pending_restrictions, vec![alice.uid]);
        assert_eq!(registry.anchor_counter().unwrap(), counter);
        let validity = ledger_dir(&root).join(ValidityFileName);
        assert_eq!(fs::metadata(&validity).unwrap().len(), 26, "intact");
        assert_eq!(
            code(registry.lineage_head(Policyd, alice.uid, &lineage_id)),
            "ipc/storage_unavailable"
        );
        assert_eq!(code(registry.shutdown()), "ipc/restriction_not_durable");
        if recovers {
            // Storage comes back before the process dies: the next operation retries
            // the invalidation and the marker, and the next start contains.
            io.set(fault_t::none);
            assert_eq!(
                code(registry.lineage_head(Policyd, alice.uid, &lineage_id)),
                "ipc/storage_unavailable",
                "a failed ledger serves nothing eligible for the rest of the run"
            );
            let status = registry.rollback_status(Policyd).unwrap();
            assert!(!status.unverifiable_owed, "retried by the next operation");
            assert_eq!(
                read_previous(&ledger_dir(&root), owner_of(&root)),
                previous_ledger_t::missing
            );
            let (_, newest, map) = newest_slot(&root);
            assert!(newest > counter);
            assert_eq!(
                map.run_marker.map(|marker| marker.state),
                Some(run_state_t::unverifiable)
            );
            drop(registry);
            let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
            assert_eq!(registry.run_decision(), run_decision_t::contained);
            assert_eq!(
                registry.standing(alice.uid, &lineage.grant.grant_id),
                Ok(standing_t::ended(end_reason_t::suspended))
            );
        } else {
            // The limit of erratum E5 (storage formats §7): the process dies before
            // any write succeeds. The ledger files look complete
            // and the marker says `running`, so the same boot resumes without the
            // restriction. A reboot contains.
            drop(registry);
            let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
            assert_eq!(registry.run_decision(), run_decision_t::ledger_replayed);
            assert_eq!(
                registry.standing(alice.uid, &lineage.grant.grant_id),
                Ok(standing_t::current),
                "the limit of erratum E5: lost to a same-boot restart"
            );
            drop(registry);
            let registry = open_with(&root, booted("boot-2"), Box::new(system_io_t));
            assert_eq!(registry.run_decision(), run_decision_t::contained);
            assert_eq!(
                registry.standing(alice.uid, &lineage.grant.grant_id),
                Ok(standing_t::ended(end_reason_t::suspended))
            );
        }
    }
}

/// The operations of the registry's IPC surface, each called once after storage
/// recovered from a compound failure (M2-006-A-R3-01). Where an operation has an
/// early return, the input takes it, so that the retry must come before it.
const RetryOperations: [&str; 16] = [
    "lineage_head",
    "identity_state",
    "journal_users",
    "policy_state",
    "unusable_users",
    "append_identity_already_present",
    "append_identity_new_user",
    "admit_unknown_user",
    "restrict",
    "set_policy_epoch_malformed",
    "set_emergency_restriction",
    "release_emergency_restriction",
    "rollback_status",
    "shutdown",
    "append_identity_commit_kind",
    "admit_wrong_caller_then_admit",
];

fn call_operation(
    registry: &Arc<registry_t>,
    root: &Path,
    operation: &str,
    alice: &test_user_t,
    lineage: &lineage_t,
) {
    let bob = test_user(1002);
    let lineage_id = lineage.genesis.payload.lineage_id;
    match operation {
        "lineage_head" => {
            let _ = registry.lineage_head(Policyd, alice.uid, &lineage_id);
        }
        "identity_state" => {
            let _ = registry.identity_state(Identityd, alice.uid, 0);
        }
        "journal_users" => {
            let _ = registry.journal_users(Identityd);
        }
        "policy_state" => {
            let _ = registry.policy_state();
        }
        "unusable_users" => {
            let _ = registry.unusable_users();
        }
        "append_identity_already_present" => {
            let receipt = registry
                .append_identity(
                    Identityd,
                    alice.uid,
                    object_kind_t::identity_enrollment,
                    &alice.enrollment,
                )
                .expect("idempotent");
            assert!(receipt.already_present);
        }
        "append_identity_new_user" => assert_eq!(
            code(registry.append_identity(
                Identityd,
                bob.uid,
                object_kind_t::identity_enrollment,
                &bob.enrollment,
            )),
            "ipc/storage_unavailable"
        ),
        "append_identity_commit_kind" => assert_eq!(
            code(registry.append_identity(Identityd, alice.uid, object_kind_t::commit, &[])),
            "ipc/malformed_request"
        ),
        "admit_unknown_user" => {
            let _ = registry.admit(Contractd, bob.uid, &lineage.revision.envelope);
        }
        "admit_wrong_caller_then_admit" => {
            // A refused caller writes nothing; the authorized call then retries.
            assert_eq!(
                code(registry.admit(Policyd, bob.uid, &[])),
                "ipc/role_denied"
            );
            assert!(
                matches!(
                    read_previous(&ledger_dir(root), owner_of(root)),
                    previous_ledger_t::valid { .. }
                ),
                "the role check comes first"
            );
            let _ = registry.admit(Contractd, bob.uid, &lineage.revision.envelope);
        }
        "restrict" => {
            let _ = registry.restrict(Admin, alice.uid, &restriction(&lineage.grant));
        }
        "set_policy_epoch_malformed" => assert_eq!(
            code(registry.set_policy_epoch(Admin, 1, 2)),
            "ipc/malformed_request"
        ),
        "set_emergency_restriction" => {
            let _ = registry.set_emergency_restriction(Admin, true);
        }
        "release_emergency_restriction" => {
            let _ = registry.set_emergency_restriction(Admin, false);
        }
        "rollback_status" => {
            let _ = registry.rollback_status(Policyd);
        }
        "shutdown" => assert_eq!(code(registry.shutdown()), "ipc/restriction_not_durable"),
        other => panic!("unknown operation {other}"),
    }
}

#[test]
fn every_operation_retries_what_a_failed_ledger_owes_once_storage_recovers() {
    // M2-006-A-R3-01 (ADR 0006 §3 erratum E5): after a compound failure (ledger
    // append, both invalidations, anchor and journal), the first IPC operation of any
    // kind after storage recovers retries both owed writes, before any early return
    // of its own: the validity file is gone, the anchor says `unverifiable`, nothing
    // eligible is served for the rest of the run, and a same-boot restart contains.
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = lineage(&alice, &editor);
    let lineage_id = lineage.genesis.payload.lineage_id;
    for operation in RetryOperations {
        let root = temp_root(&format!("retry_{operation}"));
        let io = fault_io_t::new();
        let registry = open_with(&root, booted("boot-1"), Box::new(io.clone()));
        enroll(&registry, &alice);
        assert_eq!(
            admit(&registry, &alice, &lineage.genesis),
            Ok(placement_t::trunk)
        );
        let counter = registry.anchor_counter().unwrap();
        io.set(fault_t::all_writes);
        assert_eq!(
            code(registry.restrict(Admin, alice.uid, &restriction(&lineage.grant))),
            "ipc/restriction_not_durable"
        );
        assert!(matches!(
            read_previous(&ledger_dir(&root), owner_of(&root)),
            previous_ledger_t::valid { .. }
        ));
        assert_eq!(registry.anchor_counter().unwrap(), counter);
        io.set(fault_t::none);
        call_operation(&registry, &root, operation, &alice, &lineage);
        assert_eq!(
            read_previous(&ledger_dir(&root), owner_of(&root)),
            previous_ledger_t::missing,
            "{operation}: the invalidation is retried"
        );
        let (_, newest, map) = newest_slot(&root);
        assert!(newest > counter, "{operation}: the anchor was written");
        assert_eq!(
            map.run_marker.map(|marker| marker.state),
            Some(run_state_t::unverifiable),
            "{operation}: the marker is retried"
        );
        assert_eq!(
            code(registry.lineage_head(Policyd, alice.uid, &lineage_id)),
            "ipc/storage_unavailable",
            "{operation}: nothing eligible for the rest of the run"
        );
        assert_eq!(code(registry.shutdown()), "ipc/restriction_not_durable");
        drop(registry);
        let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
        assert_eq!(
            registry.run_decision(),
            run_decision_t::contained,
            "{operation}"
        );
        assert!(
            matches!(
                registry.standing(alice.uid, &lineage.grant.grant_id),
                Ok(standing_t::ended(
                    end_reason_t::suspended | end_reason_t::restriction
                ))
            ),
            "{operation}: the grant stays ended"
        );
    }
}

#[test]
fn no_anchor_write_of_an_unverifiable_run_publishes_running() {
    // M2-006-A-R3-01: once the ledger failed, every anchor image of the run carries
    // the marker `unverifiable`, whichever write it belongs to. Here the retry's own
    // marker write fails and the release of the emergency restriction (a permissive
    // anchor write of its own) succeeds: its image must say `unverifiable` too, and a
    // later status read finds nothing owed without writing again.
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = lineage(&alice, &editor);
    let root = temp_root("sticky_unverifiable");
    let io = fault_io_t::new();
    let registry = open_with(&root, booted("boot-1"), Box::new(io.clone()));
    enroll(&registry, &alice);
    assert_eq!(
        admit(&registry, &alice, &lineage.genesis),
        Ok(placement_t::trunk)
    );
    io.set(fault_t::all_writes);
    assert_eq!(
        code(registry.restrict(Admin, alice.uid, &restriction(&lineage.grant))),
        "ipc/restriction_not_durable"
    );
    let counter = registry.anchor_counter().unwrap();
    io.set(fault_t::ledger_appends_and_unlink);
    io.fail_next_anchor_writes(1);
    registry
        .set_emergency_restriction(Admin, false)
        .expect("the release's own anchor write succeeds");
    let (_, newest, map) = newest_slot(&root);
    assert_eq!(newest, counter + 1, "one image: the release's");
    assert_eq!(
        map.run_marker.map(|marker| marker.state),
        Some(run_state_t::unverifiable)
    );
    let status = registry.rollback_status(Policyd).unwrap();
    assert!(
        status.unverifiable_owed,
        "the validity file is still intact"
    );
    assert_eq!(
        registry.anchor_counter().unwrap(),
        counter + 1,
        "the published marker is not written again"
    );
    drop(registry);
    let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
    assert_eq!(registry.run_decision(), run_decision_t::contained);
    assert_eq!(
        registry.standing(alice.uid, &lineage.grant.grant_id),
        Ok(standing_t::ended(end_reason_t::suspended))
    );
}

#[test]
fn a_journal_only_retry_keeps_the_restriction_precisely_without_containing() {
    // M2-006-A-R3-02 (ADR 0006 §3 erratum E5, corrected wording): after a compound
    // failure, a retry in which only the restriction's journal record becomes durable
    // (the ledger, both invalidations and the anchor still failing) ends the window
    // without containment: the same-boot start rolls the one unanchored record
    // forward, replays the ledger and keeps the restriction in force.
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = lineage(&alice, &editor);
    let root = temp_root("journal_only_retry");
    let io = fault_io_t::new();
    let registry = open_with(&root, booted("boot-1"), Box::new(io.clone()));
    enroll(&registry, &alice);
    assert_eq!(
        admit(&registry, &alice, &lineage.genesis),
        Ok(placement_t::trunk)
    );
    let before = records_of(&root, alice.uid).len();
    io.set(fault_t::all_writes);
    assert_eq!(
        code(registry.restrict(Admin, alice.uid, &restriction(&lineage.grant))),
        "ipc/restriction_not_durable"
    );
    io.set(fault_t::ledger_appends_and_unlink);
    io.fail_next_anchor_writes(usize::MAX);
    assert_eq!(code(registry.shutdown()), "ipc/restriction_not_durable");
    assert_eq!(
        records_of(&root, alice.uid).len(),
        before + 1,
        "the restriction reached the journal"
    );
    assert!(matches!(
        read_previous(&ledger_dir(&root), owner_of(&root)),
        previous_ledger_t::valid { .. }
    ));
    assert_eq!(
        newest_slot(&root).2.run_marker.map(|marker| marker.state),
        Some(run_state_t::running),
        "no anchor write succeeded"
    );
    drop(registry);
    let registry = open_with(&root, booted("boot-1"), Box::new(system_io_t));
    assert_eq!(registry.run_decision(), run_decision_t::ledger_replayed);
    assert_eq!(
        registry.standing(alice.uid, &lineage.grant.grant_id),
        Ok(standing_t::ended(end_reason_t::restriction)),
        "precise: the restriction itself, not a suspension"
    );
    assert!(
        registry
            .rollback_status(Policyd)
            .unwrap()
            .contained
            .is_empty()
    );
    assert_eq!(
        registry.anchored_user(alice.uid).unwrap().unwrap().seq as usize,
        before + 1,
        "the unanchored record was rolled forward"
    );
}
