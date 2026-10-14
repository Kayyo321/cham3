//! registryd over real journal files: replay, durability boundaries, tampering, roles,
//! epochs, quotas, concurrency and the identity store identityd runs over.
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
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex};

use builders::*;
use cham3_contract_format::hash::digest_t;
use cham3_contract_format::object_kind_t;
use cham3_contract_format::schema::{authority_class_t, commit_type_t, invalidation_reason_t};
use cham3_identityd::caller::{caller_role_t, service_name_t};
use cham3_identityd::directory::journal_record_t;
use cham3_registryd::journal::{ReserveBytes, ReserveFileName, journal_io_t, system_io_t};
use cham3_registryd::lineage::standing_t;
use cham3_registryd::record::{
    FrameOverheadBytes, HeaderBytes, frame, genesis_chain, header, lifecycle_record_t, next_chain,
    parse_journal, placement_t, record_body_t, registry_record_t,
};
use cham3_registryd::registry::{
    quarantine_reason_t, registry_config_t, registry_start_error_t, registry_t, unusable_reason_t,
};

/// Every role a peer can map to (service IPC §1).
fn every_role() -> Vec<caller_role_t> {
    let mut roles: Vec<caller_role_t> = service_name_t::All
        .into_iter()
        .map(caller_role_t::service)
        .collect();
    roles.extend([
        caller_role_t::trusted_client { uid: 1001 },
        caller_role_t::admin,
        caller_role_t::fixture_consumer,
    ]);
    roles
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum fault_t {
    none,
    /// The next append's write fails before any byte is written.
    fail_write,
    /// The next append writes only this many bytes, then fails.
    torn(usize),
    /// The next append's write succeeds and its fdatasync fails.
    fail_sync,
    /// The next append writes only this many bytes, fails, and the truncation that
    /// would put the journal back fails too.
    torn_rollback_fails(usize),
    /// Armed by `torn_rollback_fails`: the next truncation fails.
    rollback_fails,
}

struct faulty_io_t {
    fault: Arc<Mutex<fault_t>>,
    appends: Arc<AtomicUsize>,
}

impl journal_io_t for faulty_io_t {
    fn write_all(&self, file: &mut fs::File, bytes: &[u8]) -> io::Result<()> {
        self.appends.fetch_add(1, Ordering::SeqCst);
        let fault = *self.fault.lock().unwrap();
        match fault {
            fault_t::fail_write => {
                *self.fault.lock().unwrap() = fault_t::none;
                Err(io::Error::other("injected write failure"))
            }
            fault_t::torn(length) => {
                *self.fault.lock().unwrap() = fault_t::none;
                system_io_t.write_all(file, &bytes[..length.min(bytes.len())])?;
                Err(io::Error::other("injected torn write"))
            }
            fault_t::torn_rollback_fails(length) => {
                *self.fault.lock().unwrap() = fault_t::rollback_fails;
                system_io_t.write_all(file, &bytes[..length.min(bytes.len())])?;
                Err(io::Error::other("injected torn write"))
            }
            fault_t::none | fault_t::fail_sync | fault_t::rollback_fails => {
                system_io_t.write_all(file, bytes)
            }
        }
    }

    fn truncate(&self, file: &fs::File, length: u64) -> io::Result<()> {
        let fault = *self.fault.lock().unwrap();
        if fault == fault_t::rollback_fails {
            *self.fault.lock().unwrap() = fault_t::none;
            return Err(io::Error::other("injected truncation failure"));
        }
        system_io_t.truncate(file, length)
    }

    fn sync_data(&self, file: &fs::File) -> io::Result<()> {
        let fault = *self.fault.lock().unwrap();
        if fault == fault_t::fail_sync {
            *self.fault.lock().unwrap() = fault_t::none;
            return Err(io::Error::other("injected fdatasync failure"));
        }
        system_io_t.sync_data(file)
    }

    fn sync_directory(&self, path: &Path) -> io::Result<()> {
        system_io_t.sync_directory(path)
    }
}

/// Filesystem-space faults at the reserve's boundaries (M2-005 review R03): the next
/// write reports `ENOSPC` before any byte or after `n` bytes, or the next allocation
/// of the reserve reports `ENOSPC`. Counts truncations, releases and allocations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum space_fault_t {
    none,
    write_enospc,
    torn_then_enospc(usize),
    allocate_enospc,
}

#[derive(Clone)]
struct space_io_t {
    fault: Arc<Mutex<space_fault_t>>,
    truncations: Arc<AtomicUsize>,
    releases: Arc<AtomicUsize>,
    allocations: Arc<AtomicUsize>,
}

impl space_io_t {
    fn new() -> space_io_t {
        space_io_t {
            fault: Arc::new(Mutex::new(space_fault_t::none)),
            truncations: Arc::new(AtomicUsize::new(0)),
            releases: Arc::new(AtomicUsize::new(0)),
            allocations: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn set(&self, fault: space_fault_t) {
        *self.fault.lock().unwrap() = fault;
    }

    fn take(&self) -> space_fault_t {
        std::mem::replace(&mut *self.fault.lock().unwrap(), space_fault_t::none)
    }

    fn counts(&self) -> (usize, usize, usize) {
        (
            self.truncations.load(Ordering::SeqCst),
            self.releases.load(Ordering::SeqCst),
            self.allocations.load(Ordering::SeqCst),
        )
    }
}

fn enospc() -> io::Error {
    io::Error::from_raw_os_error(28)
}

impl journal_io_t for space_io_t {
    fn write_all(&self, file: &mut fs::File, bytes: &[u8]) -> io::Result<()> {
        // Copy the fault out first: the guard must not be held while `take` locks.
        let fault = *self.fault.lock().unwrap();
        match fault {
            space_fault_t::write_enospc => {
                self.take();
                Err(enospc())
            }
            space_fault_t::torn_then_enospc(length) => {
                self.take();
                system_io_t.write_all(file, &bytes[..length.min(bytes.len())])?;
                Err(enospc())
            }
            space_fault_t::none | space_fault_t::allocate_enospc => {
                system_io_t.write_all(file, bytes)
            }
        }
    }

    fn sync_data(&self, file: &fs::File) -> io::Result<()> {
        system_io_t.sync_data(file)
    }

    fn sync_directory(&self, path: &Path) -> io::Result<()> {
        system_io_t.sync_directory(path)
    }

    fn allocate(&self, file: &mut fs::File, from: u64, to: u64) -> io::Result<()> {
        self.allocations.fetch_add(1, Ordering::SeqCst);
        let fault = *self.fault.lock().unwrap();
        if fault == space_fault_t::allocate_enospc {
            self.take();
            return Err(enospc());
        }
        system_io_t.allocate(file, from, to)
    }

    fn release(&self, file: &fs::File) -> io::Result<()> {
        self.releases.fetch_add(1, Ordering::SeqCst);
        system_io_t.release(file)
    }

    fn truncate(&self, file: &fs::File, length: u64) -> io::Result<()> {
        self.truncations.fetch_add(1, Ordering::SeqCst);
        system_io_t.truncate(file, length)
    }
}

/// The next directory sync a [`sync_spy_t`] fails: none, the next sync of a directory
/// named `users`, or the next sync of one exact path.
#[derive(Clone, Debug, PartialEq, Eq)]
enum sync_fault_t {
    none,
    users_once,
    path_once(PathBuf),
}

/// Records every directory sync and can fail one of them (M2-005 review R01, R2-01: a
/// directory sync that fails on one attempt or at one start).
#[derive(Clone)]
struct sync_spy_t {
    synced: Arc<Mutex<Vec<PathBuf>>>,
    fault: Arc<Mutex<sync_fault_t>>,
}

impl sync_spy_t {
    fn new(fail_users_once: bool) -> sync_spy_t {
        sync_spy_t::with(if fail_users_once {
            sync_fault_t::users_once
        } else {
            sync_fault_t::none
        })
    }

    /// Fails the next sync of exactly `path`.
    fn failing_once(path: &Path) -> sync_spy_t {
        sync_spy_t::with(sync_fault_t::path_once(path.to_path_buf()))
    }

    fn with(fault: sync_fault_t) -> sync_spy_t {
        sync_spy_t {
            synced: Arc::new(Mutex::new(Vec::new())),
            fault: Arc::new(Mutex::new(fault)),
        }
    }

    fn synced(&self) -> Vec<PathBuf> {
        self.synced.lock().unwrap().clone()
    }
}

impl journal_io_t for sync_spy_t {
    fn write_all(&self, file: &mut fs::File, bytes: &[u8]) -> io::Result<()> {
        system_io_t.write_all(file, bytes)
    }

    fn sync_data(&self, file: &fs::File) -> io::Result<()> {
        system_io_t.sync_data(file)
    }

    fn sync_directory(&self, path: &Path) -> io::Result<()> {
        self.synced.lock().unwrap().push(path.to_path_buf());
        let mut fault = self.fault.lock().unwrap();
        let fails = match &*fault {
            sync_fault_t::none => false,
            sync_fault_t::users_once => path.file_name().is_some_and(|name| name == "users"),
            sync_fault_t::path_once(target) => path == target,
        };
        if fails {
            *fault = sync_fault_t::none;
            return Err(io::Error::other("injected directory fsync failure"));
        }
        drop(fault);
        system_io_t.sync_directory(path)
    }
}

/// Allocations that write every requested byte and then fail their sync (M2-005
/// review R2-02): the reserve has its full length and nothing about it is durable.
/// The next `failing` allocations fail that way; later ones are the real operation.
#[derive(Clone)]
struct unsynced_allocation_io_t {
    failing: Arc<AtomicUsize>,
    allocations: Arc<AtomicUsize>,
}

impl unsynced_allocation_io_t {
    fn new(failing: usize) -> unsynced_allocation_io_t {
        unsynced_allocation_io_t {
            failing: Arc::new(AtomicUsize::new(failing)),
            allocations: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn fail_next(&self, count: usize) {
        self.failing.store(count, Ordering::SeqCst);
    }

    fn allocations(&self) -> usize {
        self.allocations.load(Ordering::SeqCst)
    }
}

impl journal_io_t for unsynced_allocation_io_t {
    fn write_all(&self, file: &mut fs::File, bytes: &[u8]) -> io::Result<()> {
        system_io_t.write_all(file, bytes)
    }

    fn sync_data(&self, file: &fs::File) -> io::Result<()> {
        system_io_t.sync_data(file)
    }

    fn sync_directory(&self, path: &Path) -> io::Result<()> {
        system_io_t.sync_directory(path)
    }

    fn allocate(&self, file: &mut fs::File, from: u64, to: u64) -> io::Result<()> {
        self.allocations.fetch_add(1, Ordering::SeqCst);
        let mut remaining = to.saturating_sub(from);
        while remaining > 0 {
            let chunk = remaining.min(65_536) as usize;
            io::Write::write_all(file, &vec![0u8; chunk])?;
            remaining -= chunk as u64;
        }
        if self.failing.load(Ordering::SeqCst) > 0 {
            self.failing.fetch_sub(1, Ordering::SeqCst);
            return Err(io::Error::other(
                "injected reserve fsync failure after its writes",
            ));
        }
        file.sync_all()
    }
}

/// The directory syncs of a start (M2-006 stage A): `root` in its parent and `users` in
/// `root` (M2-005 R01), `root` again for the anchor directory it holds, each reopened
/// user directory, `root` for the ledger directory, the ledger directory after its files
/// and the anchor directory after the run marker. Every start repeats them (R2-01).
fn start_syncs(parent: &Path, root: &Path, reopened: &[PathBuf]) -> Vec<PathBuf> {
    let mut syncs = vec![parent.to_path_buf(), root.to_path_buf(), root.to_path_buf()];
    syncs.extend(reopened.iter().cloned());
    syncs.extend([root.to_path_buf(), root.join("ledger"), root.join("anchor")]);
    syncs
}

/// A small lineage: genesis, a revision, a temporary branch.
struct sample_t {
    genesis: commit_t,
    revision: commit_t,
    branch: commit_t,
}

fn sample(user: &test_user_t, app: &app_t) -> sample_t {
    let read = file_grant(
        "downloads read",
        user.uid,
        "home:Downloads",
        &["read"],
        persistent_lifetime(),
    );
    let genesis = draft(user, app, commit_type_t::genesis, "genesis")
        .add(vec![read.clone()])
        .sign(user);
    let write = file_grant(
        "documents write",
        user.uid,
        "home:Documents",
        &["read", "write"],
        persistent_lifetime(),
    );
    let revision = draft(user, app, commit_type_t::revision, "revision")
        .on(&genesis)
        .add(vec![write])
        .sign(user);
    let camera = file_grant(
        "camera session",
        user.uid,
        "home:Pictures",
        &["read"],
        session_lifetime("one"),
    );
    let branch = draft(user, app, commit_type_t::temporary_grant, "branch")
        .on(&revision)
        .add(vec![camera])
        .temporary_authority()
        .sign(user);
    sample_t {
        genesis,
        revision,
        branch,
    }
}

#[test]
fn enrollment_and_commits_replay_identically_after_restart() {
    let root = temp_root("replay");
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = sample(&alice, &editor);
    let registry = open(&root);
    enroll(&registry, &alice);
    assert_eq!(
        admit(&registry, &alice, &lineage.genesis),
        Ok(placement_t::trunk)
    );
    assert_eq!(
        admit(&registry, &alice, &lineage.revision),
        Ok(placement_t::trunk)
    );
    assert_eq!(
        admit(&registry, &alice, &lineage.branch),
        Ok(placement_t::temporary)
    );
    let lineage_id = lineage.genesis.payload.lineage_id;
    let before = registry
        .lineage_head(Policyd, alice.uid, &lineage_id)
        .expect("view")
        .expect("user");
    assert_eq!(
        before.trunk_head.as_ref().map(|head| head.commit_id),
        Some(lineage.revision.commit_id)
    );
    assert_eq!(before.temporary_heads.len(), 1);
    assert_eq!(before.journal_seq, 6);
    drop(registry);

    let registry = open(&root);
    assert_eq!(registry.unusable_users().expect("users"), Vec::new());
    let after = registry
        .lineage_head(Policyd, alice.uid, &lineage_id)
        .expect("view")
        .expect("user");
    assert_eq!(after, before);
    // identityd rebuilds its index from the replayed journal.
    let mut identity = identity_over(&registry);
    let resolution = identity
        .resolve_key(Contractd, &alice.persistent.key_id)
        .expect("resolve");
    assert_eq!(resolution.owner_uid, alice.uid);
    // Replay keeps every admission decision: the replayed commits cannot be
    // admitted again, and the next commit on the replayed head is.
    assert!(admit(&registry, &alice, &lineage.revision).is_err());
    let narrow = draft(&alice, &editor, commit_type_t::revocation, "narrow")
        .on(&lineage.revision)
        .remove(vec![synthetic_id("grant documents write")])
        .sign(&alice);
    assert_eq!(admit(&registry, &alice, &narrow), Ok(placement_t::trunk));
    assert_eq!(
        registry.standing(alice.uid, &synthetic_id("grant camera session")),
        Ok(standing_t::inactive),
        "a trunk commit ends the temporary branch"
    );
}

#[test]
fn the_journal_file_holds_exactly_the_chained_records() {
    let root = temp_root("layout");
    let alice = test_user(1001);
    let lineage = sample(&alice, &app("editor"));
    let registry = open(&root);
    enroll(&registry, &alice);
    admit(&registry, &alice, &lineage.genesis).expect("genesis");
    admit(&registry, &alice, &lineage.revision).expect("revision");
    let path = journal_path(&root, alice.uid);
    assert_eq!(fs::metadata(&path).expect("journal").mode() & 0o777, 0o600);
    assert_eq!(
        fs::metadata(path.parent().unwrap()).expect("dir").mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(root.join("users")).expect("users").mode() & 0o777,
        0o700
    );
    let bytes = fs::read(&path).expect("read");
    assert_eq!(
        &bytes[..HeaderBytes],
        header(alice.uid, &alice.identity.key_id).as_slice()
    );
    let parsed = parse_journal(&bytes, alice.uid).expect("parse");
    let kinds: Vec<String> = parsed
        .records
        .iter()
        .enumerate()
        .map(|(position, record)| {
            let record = registry_record_t::decode(record).expect("record");
            assert_eq!(record.sequence, position as u64 + 1);
            assert_eq!(record.time, "2026-10-13T09:00:00Z");
            match record.body {
                record_body_t::identity { kind, .. } => kind.name().to_owned(),
                record_body_t::commit_admitted {
                    commit_id,
                    placement,
                    envelope,
                    ..
                } => {
                    assert_eq!(cham3_contract_format::hash::sha256(&envelope), commit_id);
                    format!("commit_admitted/{}", placement.as_str())
                }
                record_body_t::lifecycle(_) => "lifecycle".to_owned(),
                record_body_t::user(_) => "user".to_owned(),
                record_body_t::event(_) => "event".to_owned(),
            }
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "identity_enrollment",
            "key_certificate",
            "key_certificate",
            "commit_admitted/trunk",
            "commit_admitted/trunk"
        ]
    );
    let expected_length: usize = HeaderBytes
        + parsed
            .records
            .iter()
            .map(|record| record.len() + FrameOverheadBytes)
            .sum::<usize>();
    assert_eq!(bytes.len(), expected_length);
}

fn write_tampered(root: &Path, uid: u32, edit: impl FnOnce(&mut Vec<u8>)) {
    let path = journal_path(root, uid);
    let mut bytes = fs::read(&path).expect("read");
    edit(&mut bytes);
    fs::write(&path, bytes).expect("write");
}

#[test]
fn a_tampered_or_torn_journal_makes_only_that_user_unusable() {
    // A rewritten or corrupted journal makes only its user unusable. A defect in the
    // bytes (a flipped byte, a cut header, a frame whose length no record can have) is a
    // store defect; a tail shorter than the anchor says is a stale journal and the user
    // is quarantined (ADR 0006 §4); neither is truncated. Only a torn frame beyond the
    // anchor — an append that died before it was acknowledged — is discarded at the
    // next start, and that user resumes (M2-006 stage A).
    let alice = test_user(1001);
    let bob = test_user(1002);
    let editor = app("editor");
    let alice_lineage = sample(&alice, &editor);
    let bob_lineage = sample(&bob, &editor);
    type edit_t = Box<dyn Fn(&mut Vec<u8>)>;
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum outcome_t {
        defect,
        stale_journal,
        truncated_and_resumed,
    }
    let cases: [(&str, edit_t, outcome_t); 5] = [
        (
            "flipped",
            Box::new(|bytes: &mut Vec<u8>| {
                let position = bytes.len() - 100;
                bytes[position] ^= 1;
            }),
            outcome_t::defect,
        ),
        (
            "torn_inside_the_anchor",
            Box::new(|bytes: &mut Vec<u8>| bytes.truncate(bytes.len() - 7)),
            outcome_t::stale_journal,
        ),
        (
            "truncated_header",
            Box::new(|bytes: &mut Vec<u8>| bytes.truncate(20)),
            outcome_t::defect,
        ),
        // A frame start whose declared length exceeds every record: a defect, never a
        // torn append.
        (
            "appended_garbage",
            Box::new(|bytes: &mut Vec<u8>| bytes.extend_from_slice(b"C3JRxxxx")),
            outcome_t::defect,
        ),
        // A frame start with a plausible length and a few bytes of body: a torn append.
        (
            "torn_beyond_the_anchor",
            Box::new(|bytes: &mut Vec<u8>| {
                bytes.extend_from_slice(b"C3JR");
                bytes.extend_from_slice(&200u32.to_be_bytes());
                bytes.extend_from_slice(&[0x5a; 11]);
            }),
            outcome_t::truncated_and_resumed,
        ),
    ];
    for (label, edit, outcome) in cases {
        let root = temp_root(&format!("tamper_{label}"));
        let registry = open(&root);
        enroll(&registry, &alice);
        enroll(&registry, &bob);
        admit(&registry, &alice, &alice_lineage.genesis).expect("alice genesis");
        admit(&registry, &bob, &bob_lineage.genesis).expect("bob genesis");
        let anchored = registry
            .anchored_user(alice.uid)
            .expect("anchor")
            .expect("anchored");
        drop(registry);
        let intact_length = fs::metadata(journal_path(&root, alice.uid)).unwrap().len();
        write_tampered(&root, alice.uid, edit);
        let tampered_length = fs::metadata(journal_path(&root, alice.uid)).unwrap().len();
        let registry = open(&root);
        let unusable = registry.unusable_users().expect("users");
        let lineage_id = alice_lineage.genesis.payload.lineage_id;
        let mut identity = identity_over(&registry);
        if outcome == outcome_t::truncated_and_resumed {
            assert_eq!(unusable, Vec::new(), "{label}");
            assert_eq!(
                fs::metadata(journal_path(&root, alice.uid)).unwrap().len(),
                intact_length,
                "{label}: the torn frame is discarded"
            );
            assert_eq!(
                registry.anchored_user(alice.uid).unwrap(),
                Some(anchored),
                "{label}: nothing to roll forward"
            );
            assert!(
                registry
                    .lineage_head(Policyd, alice.uid, &lineage_id)
                    .expect("view")
                    .is_some(),
                "{label}"
            );
            assert_eq!(
                admit(&registry, &alice, &alice_lineage.revision),
                Ok(placement_t::trunk),
                "{label}"
            );
            assert!(
                identity
                    .resolve_key(Contractd, &alice.persistent.key_id)
                    .is_ok(),
                "{label}"
            );
        } else {
            assert_eq!(unusable.len(), 1, "{label}");
            assert_eq!(unusable[0].0, alice.uid, "{label}");
            let expected_code = if outcome == outcome_t::stale_journal {
                assert_eq!(
                    unusable[0].1,
                    unusable_reason_t::quarantine(quarantine_reason_t::stale_journal),
                    "{label}"
                );
                "key_authority/quarantined"
            } else {
                assert!(
                    matches!(unusable[0].1, unusable_reason_t::defect(_)),
                    "{label}: {:?}",
                    unusable[0].1
                );
                "ipc/storage_unavailable"
            };
            // Kept as found: a defective or stale journal is evidence, never truncated.
            assert_eq!(
                fs::metadata(journal_path(&root, alice.uid)).unwrap().len(),
                tampered_length,
                "{label}"
            );
            assert_eq!(
                code(registry.lineage_head(Policyd, alice.uid, &lineage_id)),
                expected_code,
                "{label}"
            );
            assert_eq!(
                code(admit(&registry, &alice, &alice_lineage.revision)),
                expected_code,
                "{label}"
            );
            assert_eq!(
                code(registry.identity_state(Identityd, alice.uid, 0)),
                expected_code,
                "{label}"
            );
            // A freshly started identityd cannot read the user's keys: they resolve as
            // not certified (denied).
            assert_eq!(
                code(identity.resolve_key(Contractd, &alice.persistent.key_id)),
                "key_authority/key_not_certified",
                "{label}"
            );
        }
        // The other user is unaffected.
        assert_eq!(
            admit(&registry, &bob, &bob_lineage.revision),
            Ok(placement_t::trunk),
            "{label}"
        );
    }
}

/// Writes a journal of `uid` from raw records with correct chain values.
fn write_chained(root: &Path, uid: u32, user_id: &digest_t, records: &[Vec<u8>]) {
    let mut chain = genesis_chain(user_id, uid);
    let mut bytes = header(uid, user_id);
    for (index, record) in records.iter().enumerate() {
        chain = next_chain(&chain, index as u64, record);
        bytes.extend(frame(record, &chain).expect("frame"));
    }
    fs::write(journal_path(root, uid), bytes).expect("write");
}

fn record(sequence: u64, body: record_body_t) -> Vec<u8> {
    registry_record_t {
        sequence,
        time: "2026-10-13T09:00:00Z".to_owned(),
        body,
    }
    .encode()
}

#[test]
fn a_correctly_chained_record_must_still_pass_every_check_on_replay() {
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = sample(&alice, &editor);
    let root = temp_root("rechained");
    let registry = open(&root);
    enroll(&registry, &alice);
    admit(&registry, &alice, &lineage.genesis).expect("genesis");
    drop(registry);
    let original = records_of(&root, alice.uid);
    let user_id = alice.identity.key_id;
    let commit_record = |sequence: u64, commit: &commit_t, placement: placement_t| {
        record(
            sequence,
            record_body_t::commit_admitted {
                commit_id: commit.commit_id,
                lineage_id: commit.payload.lineage_id,
                placement,
                envelope: commit.envelope.clone(),
                observed: 0,
            },
        )
    };
    let unsigned_epoch = draft(&alice, &editor, commit_type_t::revision, "skipped head")
        .on(&lineage.genesis)
        .add(vec![file_grant(
            "x",
            alice.uid,
            "home:x",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&alice);
    let mut forged_signature = lineage.revision.envelope.clone();
    let last = forged_signature.len() - 1;
    forged_signature[last] ^= 1;
    // The anchor holds the chain at record 4. A rewritten or shortened prefix is caught
    // there (ADR 0006 §4: chain mismatch, stale journal) before any replay; a record
    // appended beyond the anchor extends the chain and is admitted only if it passes
    // every replay rule — a failing one quarantines the user (M2-006 stage A).
    let on_absent_parent = draft(&alice, &editor, commit_type_t::revision, "absent parent")
        .on(&lineage.revision)
        .add(vec![file_grant(
            "y",
            alice.uid,
            "home:y",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&alice);
    let replay_defect = Some(unusable_reason_t::quarantine(
        quarantine_reason_t::replay_defect,
    ));
    let chain_mismatch = Some(unusable_reason_t::quarantine(
        quarantine_reason_t::chain_mismatch,
    ));
    let stale_journal = Some(unusable_reason_t::quarantine(
        quarantine_reason_t::stale_journal,
    ));
    let extended = |fifth: Vec<u8>| {
        let mut records = original.clone();
        records.push(fifth);
        records
    };
    type case_t<'a> = (&'a str, Vec<Vec<u8>>, Option<unusable_reason_t>);
    let cases: Vec<case_t<'_>> = vec![
        // The control: the original records replay.
        ("control", original.clone(), None),
        // A commit whose recorded placement differs from the recomputed one.
        (
            "placement",
            extended(commit_record(5, &lineage.revision, placement_t::temporary)),
            replay_defect,
        ),
        // A commit admitted twice.
        (
            "twice",
            extended(commit_record(5, &lineage.genesis, placement_t::trunk)),
            replay_defect,
        ),
        // A commit whose parent is not in the journal.
        (
            "parent",
            extended(commit_record(5, &on_absent_parent, placement_t::trunk)),
            replay_defect,
        ),
        // A signature that does not verify.
        (
            "signature",
            extended(record(
                5,
                record_body_t::commit_admitted {
                    commit_id: cham3_contract_format::hash::sha256(&forged_signature),
                    lineage_id: lineage.revision.payload.lineage_id,
                    placement: placement_t::trunk,
                    envelope: forged_signature.clone(),
                    observed: 0,
                },
            )),
            replay_defect,
        ),
        // A sequence number out of order.
        (
            "sequence",
            extended(commit_record(9, &lineage.revision, placement_t::trunk)),
            replay_defect,
        ),
        // A lifecycle record that fails its rule (expiry of a grant without expiration).
        (
            "lifecycle",
            extended(record(
                5,
                record_body_t::lifecycle(lifecycle_record_t::expired {
                    grant_id: synthetic_id("grant downloads read"),
                }),
            )),
            replay_defect,
        ),
        // A rewritten prefix, correctly chained: the anchored chain disagrees.
        (
            "rewritten_prefix",
            vec![
                original[0].clone(),
                original[1].clone(),
                original[2].clone(),
                commit_record(4, &lineage.genesis, placement_t::temporary),
            ],
            chain_mismatch,
        ),
        // A shortened journal: the certificates and the commit gone.
        ("shortened", vec![original[0].clone()], stale_journal),
        // A commit before the user's certificates (also shorter than the anchor).
        (
            "before_certificates",
            vec![
                original[0].clone(),
                commit_record(2, &lineage.genesis, placement_t::trunk),
            ],
            stale_journal,
        ),
        // A second, valid commit on top: replays, and rolls the anchor forward.
        (
            "valid_extension",
            extended(commit_record(5, &unsigned_epoch, placement_t::trunk)),
            None,
        ),
    ];
    for (label, records, expected) in cases {
        write_chained(&root, alice.uid, &user_id, &records);
        let registry = open(&root);
        let unusable = registry.unusable_users().expect("users");
        match expected {
            None => {
                assert_eq!(unusable, Vec::new(), "{label}");
                assert_eq!(
                    registry
                        .anchored_user(alice.uid)
                        .unwrap()
                        .map(|entry| entry.seq),
                    Some(records.len() as u64),
                    "{label}: anchored at the last record"
                );
            }
            Some(reason) => {
                assert_eq!(unusable, vec![(alice.uid, reason)], "{label}");
            }
        }
    }
}

#[test]
fn replayed_lifecycle_records_end_grants_and_free_the_network_slot() {
    let alice = test_user(1001);
    let browser = app("browser");
    let root = temp_root("lifecycle");
    let registry = open(&root);
    enroll(&registry, &alice);
    let genesis = draft(&alice, &browser, commit_type_t::genesis, "browser genesis")
        .add(vec![file_grant(
            "cache",
            alice.uid,
            "home:cache",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&alice);
    admit(&registry, &alice, &genesis).expect("genesis");
    let direct = draft(&alice, &browser, commit_type_t::temporary_grant, "direct")
        .on(&genesis)
        .add(vec![network_grant(
            "direct",
            alice.uid,
            "direct",
            timed_lifetime("2026-10-13T10:00:00Z"),
        )])
        .temporary_authority()
        .sign(&alice);
    admit(&registry, &alice, &direct).expect("direct branch");
    let tor = draft(&alice, &browser, commit_type_t::temporary_grant, "tor")
        .on(&genesis)
        .add(vec![network_grant(
            "tor",
            alice.uid,
            "tor",
            timed_lifetime("2026-10-13T11:00:00Z"),
        )])
        .temporary_authority()
        .sign(&alice);
    assert_eq!(
        code(admit(&registry, &alice, &tor)),
        "admission/network_grant_conflict"
    );
    drop(registry);
    // A durable expiry record (written here directly; the operation that writes it is
    // M2-006) frees the slot after replay.
    let mut records = records_of(&root, alice.uid);
    records.push(record(
        records.len() as u64 + 1,
        record_body_t::lifecycle(lifecycle_record_t::expired {
            grant_id: synthetic_id("grant direct"),
        }),
    ));
    // Registry time in `record` is 09:00, before the 10:00 expiration: refused.
    write_chained(&root, alice.uid, &alice.identity.key_id, &records);
    let registry = open(&root);
    assert_eq!(registry.unusable_users().expect("users").len(), 1);
    drop(registry);
    let mut late = records.clone();
    late.pop();
    late.push(
        registry_record_t {
            sequence: late.len() as u64 + 1,
            time: "2026-10-13T10:00:00Z".to_owned(),
            body: record_body_t::lifecycle(lifecycle_record_t::expired {
                grant_id: synthetic_id("grant direct"),
            }),
        }
        .encode(),
    );
    write_chained(&root, alice.uid, &alice.identity.key_id, &late);
    let registry = open(&root);
    assert_eq!(registry.unusable_users().expect("users"), Vec::new());
    assert!(matches!(
        registry.standing(alice.uid, &synthetic_id("grant direct")),
        Ok(standing_t::ended(_))
    ));
    let view = registry
        .lineage_head(Policyd, alice.uid, &genesis.payload.lineage_id)
        .expect("view")
        .expect("user");
    assert!(view.occupying_network.is_empty());
    assert_eq!(admit(&registry, &alice, &tor), Ok(placement_t::temporary));
}

#[test]
fn every_created_directory_entry_is_durable_before_the_enrollment_is_acknowledged() {
    // M2-005 review R01. Opening a new store syncs each directory it creates in its
    // parent (`root` in its parent, `users` in `root`); the first enrollment syncs the
    // user directory's entry in `users`, then the journal's entry in the user
    // directory. The order of the syncs is the durability order.
    let parent = temp_root("directory_durability");
    let root = parent.join("registry");
    let spy = sync_spy_t::new(false);
    let registry = Arc::new(
        registry_t::open(
            &root,
            owner_of(&parent),
            registry_config_t::default(),
            Box::new(spy.clone()),
            fixed_clock(),
        )
        .expect("registry start"),
    );
    assert_eq!(spy.synced(), start_syncs(&parent, &root, &[]));
    let alice = test_user(1001);
    enroll(&registry, &alice);
    // The enrollment syncs the user directory's entry in `users`, the journal's entry
    // in the user directory, then the anchor directory once: its first write to the
    // anchor's second slot (the slot's entry; later writes to a slot this run has
    // synced do not repeat it).
    assert_eq!(
        spy.synced(),
        [
            start_syncs(&parent, &root, &[]),
            vec![
                root.join("users"),
                root.join("users").join("1001"),
                root.join("anchor"),
            ],
        ]
        .concat()
    );
    // Reopening an existing store creates nothing, but repeats the syncs the acknowledged
    // state depends on (R2-01): `root` in its parent, `users` in `root`, and each
    // replayed user's directory; a directory's existence is no proof of an earlier
    // successful sync.
    drop(registry);
    let spy = sync_spy_t::new(false);
    let registry = open_with(&root, registry_config_t::default(), Box::new(spy.clone()));
    assert_eq!(
        spy.synced(),
        start_syncs(&parent, &root, &[root.join("users").join("1001")])
    );
    assert!(registry.unusable_users().unwrap().is_empty());
}

#[test]
fn a_failed_parent_directory_sync_is_repeated_before_a_retry_is_acknowledged() {
    // M2-005 review R01: the first attempt creates the user directory and fails to
    // sync its entry in `users`; the retry finds the directory and must sync `users`
    // again before it acknowledges the enrollment. Nothing is written before the
    // parent entry is durable, so the failed attempt leaves no journal.
    let root = temp_root("parent_sync_retry");
    let parent = root.parent().unwrap().to_path_buf();
    let spy = sync_spy_t::new(true);
    let registry = open_with(&root, registry_config_t::default(), Box::new(spy.clone()));
    let alice = test_user(1001);
    assert_eq!(
        code(registry.append_identity(
            Identityd,
            alice.uid,
            object_kind_t::identity_enrollment,
            &alice.enrollment,
        )),
        "ipc/storage_unavailable"
    );
    assert!(root.join("users").join("1001").is_dir());
    assert!(!journal_path(&root, alice.uid).exists());
    // Opening the store synced `root` in its parent and the new `users` in `root`; the
    // enrollment's `users` sync failed and nothing else was synced or written.
    assert_eq!(
        spy.synced(),
        [start_syncs(&parent, &root, &[]), vec![root.join("users")]].concat()
    );
    let receipt = registry
        .append_identity(
            Identityd,
            alice.uid,
            object_kind_t::identity_enrollment,
            &alice.enrollment,
        )
        .expect("retry");
    assert_eq!(receipt.journal_seq, 1);
    assert!(!receipt.already_present);
    assert_eq!(
        spy.synced(),
        [
            start_syncs(&parent, &root, &[]),
            vec![
                root.join("users"),
                root.join("users"),
                root.join("users").join("1001"),
                root.join("anchor"),
            ],
        ]
        .concat()
    );
    // The journal replays after a restart.
    drop(registry);
    let registry = open(&root);
    assert!(registry.unusable_users().unwrap().is_empty());
    assert_eq!(
        registry
            .identity_state(Identityd, alice.uid, 0)
            .unwrap()
            .entries
            .len(),
        1
    );
}

#[test]
fn a_failed_startup_directory_sync_is_repeated_at_the_next_start() {
    // M2-005 review R2-01. A start that fails to sync a directory it created refuses
    // the store and leaves the directory visible. The next start finds it and must not
    // take its existence for a completed sync: `root` is synced in its parent and
    // `users` in `root` at every start, before any enrollment is acknowledged.
    // (a) A new root whose entry in the parent was never made durable.
    let parent = temp_root("startup_sync_new_root");
    let root = parent.join("registry");
    let spy = sync_spy_t::failing_once(&parent);
    assert!(
        registry_t::open(
            &root,
            owner_of(&parent),
            registry_config_t::default(),
            Box::new(spy.clone()),
            fixed_clock(),
        )
        .is_err()
    );
    assert!(root.is_dir(), "visible, its entry not durable");
    assert_eq!(spy.synced(), vec![parent.clone()]);
    let spy = sync_spy_t::new(false);
    let registry = Arc::new(
        registry_t::open(
            &root,
            owner_of(&parent),
            registry_config_t::default(),
            Box::new(spy.clone()),
            fixed_clock(),
        )
        .expect("second start"),
    );
    assert_eq!(spy.synced(), start_syncs(&parent, &root, &[]));
    enroll(&registry, &test_user(1001));
    // The enrollment syncs the user directory's entry in `users`, the journal's entry
    // in the user directory, then the anchor directory once: its first write to the
    // anchor's second slot (the slot's entry; later writes to a slot this run has
    // synced do not repeat it).
    assert_eq!(
        spy.synced(),
        [
            start_syncs(&parent, &root, &[]),
            vec![
                root.join("users"),
                root.join("users").join("1001"),
                root.join("anchor"),
            ],
        ]
        .concat()
    );
    // (b) An existing root whose new `users` directory was never made durable in it.
    let root = temp_root("startup_sync_users");
    let parent = root.parent().unwrap().to_path_buf();
    let spy = sync_spy_t::failing_once(&root);
    assert!(
        registry_t::open(
            &root,
            owner_of(&root),
            registry_config_t::default(),
            Box::new(spy.clone()),
            fixed_clock(),
        )
        .is_err()
    );
    assert!(root.join("users").is_dir());
    assert_eq!(spy.synced(), vec![parent.clone(), root.clone()]);
    let spy = sync_spy_t::new(false);
    let registry = open_with(&root, registry_config_t::default(), Box::new(spy.clone()));
    assert_eq!(spy.synced(), start_syncs(&parent, &root, &[]));
    enroll(&registry, &test_user(1002));
    assert!(registry.unusable_users().unwrap().is_empty());
}

#[test]
fn a_failed_creation_sync_is_repeated_when_the_journal_is_reopened() {
    // M2-005 review R2-01. (a) The journal's bytes are durable but the sync of the user
    // directory that names it fails: the enrollment is refused, the file stays
    // visible, and the next start syncs the user directory before it replays the
    // journal and acknowledges the same enrollment as already present.
    let root = temp_root("creation_sync_reopen");
    let parent = root.parent().unwrap().to_path_buf();
    let alice = test_user(1001);
    let user_directory = root.join("users").join("1001");
    let spy = sync_spy_t::failing_once(&user_directory);
    let registry = open_with(&root, registry_config_t::default(), Box::new(spy.clone()));
    assert_eq!(
        code(registry.append_identity(
            Identityd,
            alice.uid,
            object_kind_t::identity_enrollment,
            &alice.enrollment,
        )),
        "ipc/storage_unavailable"
    );
    assert!(
        journal_path(&root, alice.uid).is_file(),
        "written, its entry not durable"
    );
    drop(registry);
    let spy = sync_spy_t::new(false);
    let registry = open_with(&root, registry_config_t::default(), Box::new(spy.clone()));
    assert!(registry.unusable_users().unwrap().is_empty());
    // The journal's one durable record was never anchored (its creation failed before
    // the anchor update): the start rolls the anchor forward, one more anchor sync.
    assert_eq!(
        spy.synced(),
        [
            start_syncs(&parent, &root, &[user_directory.clone()]),
            vec![root.join("anchor")],
        ]
        .concat()
    );
    let receipt = registry
        .append_identity(
            Identityd,
            alice.uid,
            object_kind_t::identity_enrollment,
            &alice.enrollment,
        )
        .expect("the replayed enrollment");
    assert!(receipt.already_present);
    assert_eq!(receipt.journal_seq, 1);
    drop(registry);
    // (b) A reserve recreated at start whose directory sync fails: the user is unusable
    // for that run; the next start syncs the directory again before any append.
    fs::remove_file(reserve_path(&root, alice.uid)).unwrap();
    let spy = sync_spy_t::failing_once(&user_directory);
    let registry = open_with(&root, registry_config_t::default(), Box::new(spy.clone()));
    assert_eq!(
        registry.unusable_users().unwrap(),
        vec![(
            alice.uid,
            unusable_reason_t::store(cham3_registryd::journal::store_error_t::io)
        )]
    );
    assert!(
        reserve_path(&root, alice.uid).is_file(),
        "created, its entry not durable"
    );
    drop(registry);
    let spy = sync_spy_t::new(false);
    let registry = open_with(&root, registry_config_t::default(), Box::new(spy.clone()));
    assert!(registry.unusable_users().unwrap().is_empty());
    assert_eq!(
        spy.synced(),
        start_syncs(&parent, &root, &[user_directory.clone()])
    );
    assert_eq!(registry.reserve_bytes(alice.uid), Ok(0));
    let mut identity = identity_over(&registry);
    identity
        .register_certificate(
            Vaultd,
            alice.uid,
            &certificate(
                &alice.identity,
                alice.uid,
                &test_key("after the recreated reserve"),
                authority_class_t::persistent,
                1,
            ),
        )
        .expect("the first non-restrictive append refills the recreated reserve");
    assert_eq!(registry.reserve_bytes(alice.uid), Ok(ReserveBytes));
    // An append syncs no directory, except the anchor's for the first write to a slot
    // this run (the other slot, after the run marker's).
    assert_eq!(
        spy.synced(),
        [
            start_syncs(&parent, &root, &[user_directory]),
            vec![root.join("anchor")]
        ]
        .concat()
    );
}

#[test]
fn an_ended_session_refuses_every_later_grant_bound_to_it() {
    // M2-005 review R02 (ADR 0007 §6): a durable `session_ended` record ends the
    // session itself, not only the grants present at that instant. A later grant bound
    // to the same session id — a network grant included, with the slot free — is
    // `duplicate_identifier`, at admission and at replay; a fresh session is admitted.
    let alice = test_user(1001);
    let browser = app("browser");
    let root = temp_root("ended_session");
    let registry = open(&root);
    enroll(&registry, &alice);
    let genesis = draft(&alice, &browser, commit_type_t::genesis, "session genesis")
        .add(vec![file_grant(
            "cache",
            alice.uid,
            "home:cache",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&alice);
    admit(&registry, &alice, &genesis).expect("genesis");
    let first = file_grant(
        "pictures in session one",
        alice.uid,
        "home:Pictures",
        &["read"],
        session_lifetime("one"),
    );
    let session_one = first.lifetime.session_id.expect("session");
    let first_branch = draft(&alice, &browser, commit_type_t::temporary_grant, "first")
        .on(&genesis)
        .add(vec![first.clone()])
        .temporary_authority()
        .sign(&alice);
    admit(&registry, &alice, &first_branch).expect("first branch");
    drop(registry);
    let mut records = records_of(&root, alice.uid);
    records.push(record(
        records.len() as u64 + 1,
        record_body_t::lifecycle(lifecycle_record_t::session_ended {
            session_id: session_one,
        }),
    ));
    write_chained(&root, alice.uid, &alice.identity.key_id, &records);
    let registry = open(&root);
    assert!(registry.unusable_users().expect("users").is_empty());
    assert_eq!(
        registry.standing(alice.uid, &first.grant_id),
        Ok(standing_t::ended(
            cham3_registryd::lineage::end_reason_t::session_ended
        ))
    );
    // The same session on a new sibling: a file grant, then a network grant although
    // the slot is free. Neither leaves a trace in the index.
    let again = draft(&alice, &browser, commit_type_t::temporary_grant, "again")
        .on(&genesis)
        .add(vec![file_grant(
            "camera in session one",
            alice.uid,
            "camera:front",
            &["capture"],
            session_lifetime("one"),
        )])
        .temporary_authority()
        .sign(&alice);
    assert_eq!(
        code(admit(&registry, &alice, &again)),
        "admission/duplicate_identifier"
    );
    let network_again = draft(
        &alice,
        &browser,
        commit_type_t::temporary_grant,
        "tor again",
    )
    .on(&genesis)
    .add(vec![network_grant(
        "tor in session one",
        alice.uid,
        "tor",
        session_lifetime("one"),
    )])
    .temporary_authority()
    .sign(&alice);
    assert_eq!(
        code(admit(&registry, &alice, &network_again)),
        "admission/duplicate_identifier"
    );
    assert_eq!(
        registry.standing(alice.uid, &synthetic_id("grant camera in session one")),
        Ok(standing_t::unknown)
    );
    // A fresh session is admitted. The view names the ended session because the first
    // branch (whose grant it ended) is still a live temporary head; the fresh grant's
    // session is not named.
    let fresh = draft(&alice, &browser, commit_type_t::temporary_grant, "fresh")
        .on(&genesis)
        .add(vec![file_grant(
            "camera in session two",
            alice.uid,
            "camera:front",
            &["capture"],
            session_lifetime("two"),
        )])
        .temporary_authority()
        .sign(&alice);
    assert_eq!(admit(&registry, &alice, &fresh), Ok(placement_t::temporary));
    let view = registry
        .lineage_head(Policyd, alice.uid, &genesis.payload.lineage_id)
        .expect("view")
        .expect("user");
    assert_eq!(view.ended_sessions, [session_one].into_iter().collect());
    assert_eq!(view.ended.len(), 1);
    assert_eq!(view.temporary_heads.len(), 2);
    drop(registry);
    // Replay refuses a correctly chained commit that binds a grant to the ended
    // session: the whole journal is unusable (fail closed).
    let mut forged = records_of(&root, alice.uid);
    forged.push(record(
        forged.len() as u64 + 1,
        record_body_t::commit_admitted {
            commit_id: again.commit_id,
            lineage_id: again.payload.lineage_id,
            placement: placement_t::temporary,
            envelope: again.envelope.clone(),
            observed: 0,
        },
    ));
    write_chained(&root, alice.uid, &alice.identity.key_id, &forged);
    let registry = open(&root);
    assert_eq!(
        registry.unusable_users().expect("users"),
        vec![(
            alice.uid,
            unusable_reason_t::quarantine(quarantine_reason_t::replay_defect)
        )]
    );
}

fn reserve_path(root: &Path, uid: u32) -> PathBuf {
    root.join("users")
        .join(uid.to_string())
        .join(ReserveFileName)
}

fn allocated_bytes(path: &Path) -> u64 {
    fs::metadata(path).unwrap().blocks() * 512
}

#[test]
fn the_preallocated_reserve_is_used_only_by_restrictive_records() {
    // M2-005 review R03 (ADR 0006 §1, ADR 0008 §1). Enrollment preallocates the reserve
    // (written zeros, so real blocks). An admission that meets a full filesystem is
    // refused and touches no reserve; a key invalidation that meets one takes the
    // reserve's blocks and becomes durable; while the reserve cannot be refilled no
    // admission proceeds, restrictive records still do; the first admission after
    // space returns refills it.
    let root = temp_root("reserve");
    let alice = test_user(1001);
    let browser = app("browser");
    let io = space_io_t::new();
    let registry = open_with(&root, registry_config_t::default(), Box::new(io.clone()));
    enroll(&registry, &alice);
    let reserve = reserve_path(&root, alice.uid);
    assert_eq!(fs::metadata(&reserve).unwrap().len(), ReserveBytes);
    assert!(
        allocated_bytes(&reserve) >= ReserveBytes,
        "blocks, not a sparse file"
    );
    assert_eq!(fs::metadata(&reserve).unwrap().mode() & 0o777, 0o600);
    assert_eq!(registry.reserve_bytes(alice.uid), Ok(ReserveBytes));
    assert_eq!(io.counts(), (0, 0, 1));
    let genesis = draft(&alice, &browser, commit_type_t::genesis, "reserve genesis").sign(&alice);
    // An admission on a full filesystem: refused, the failed frame truncated away, the
    // reserve untouched; the user continues (stage A).
    io.set(space_fault_t::write_enospc);
    assert_eq!(
        code(admit(&registry, &alice, &genesis)),
        "ipc/storage_unavailable"
    );
    assert_eq!(io.counts(), (1, 0, 1));
    assert_eq!(fs::metadata(&reserve).unwrap().len(), ReserveBytes);
    assert_eq!(
        registry
            .lineage_head(Policyd, alice.uid, &genesis.payload.lineage_id)
            .expect("view")
            .expect("user")
            .trunk_head,
        None
    );
    drop(registry);
    let registry = open_with(&root, registry_config_t::default(), Box::new(io.clone()));
    assert!(registry.unusable_users().unwrap().is_empty());
    // The restart synced the full-length reserve before trusting it (R2-02).
    assert_eq!(io.counts(), (1, 0, 2));
    assert_eq!(registry.reserve_bytes(alice.uid), Ok(ReserveBytes));
    let before = fs::metadata(journal_path(&root, alice.uid)).unwrap().len();
    // A key invalidation on a full filesystem: truncated back, the reserve released,
    // the record durable.
    let mut identity = identity_over(&registry);
    io.set(space_fault_t::write_enospc);
    identity
        .register_invalidation(
            Vaultd,
            alice.uid,
            &invalidation(
                &alice.identity,
                &alice.temporary.key_id,
                invalidation_reason_t::compromised,
                "full disk",
            ),
        )
        .expect("restrictive record from the reserve");
    assert_eq!(io.counts(), (2, 1, 2));
    assert_eq!(registry.reserve_bytes(alice.uid), Ok(0));
    // The depleted reserve is reported (M2-006 stage A, until the status operations of
    // stage B).
    assert_eq!(
        registry.rollback_status(Policyd).unwrap().depleted_reserves,
        vec![alice.uid]
    );
    assert_eq!(fs::metadata(&reserve).unwrap().len(), 0);
    assert!(fs::metadata(journal_path(&root, alice.uid)).unwrap().len() > before);
    assert_eq!(
        identity
            .resolve_key(Contractd, &alice.temporary.key_id)
            .unwrap()
            .invalidated,
        Some(invalidation_reason_t::compromised)
    );
    // While the reserve cannot be refilled: no admission, but restrictive records go on
    // and the journal stays usable.
    let length = fs::metadata(journal_path(&root, alice.uid)).unwrap().len();
    io.set(space_fault_t::allocate_enospc);
    assert_eq!(
        code(admit(&registry, &alice, &genesis)),
        "ipc/storage_unavailable"
    );
    assert_eq!(io.counts(), (2, 1, 3));
    assert_eq!(
        fs::metadata(journal_path(&root, alice.uid)).unwrap().len(),
        length
    );
    assert_eq!(registry.reserve_bytes(alice.uid), Ok(0));
    io.set(space_fault_t::allocate_enospc);
    assert_eq!(
        code(admit(&registry, &alice, &genesis)),
        "ipc/storage_unavailable",
        "refilling failed again"
    );
    assert_eq!(io.counts(), (2, 1, 4));
    identity
        .register_invalidation(
            Vaultd,
            alice.uid,
            &invalidation(
                &alice.identity,
                &alice.persistent.key_id,
                invalidation_reason_t::retired,
                "still full",
            ),
        )
        .expect("restrictive records never wait for the reserve");
    assert!(
        registry
            .lineage_head(Policyd, alice.uid, &genesis.payload.lineage_id)
            .is_ok()
    );
    // Space is back: the admission refills the reserve first, then is admitted.
    io.set(space_fault_t::none);
    let fresh_key = test_key("reserve fresh persistent");
    identity
        .register_certificate(
            Vaultd,
            alice.uid,
            &certificate(
                &alice.identity,
                alice.uid,
                &fresh_key,
                authority_class_t::persistent,
                1,
            ),
        )
        .expect("a certificate is not restrictive: it refills the reserve");
    assert_eq!(registry.reserve_bytes(alice.uid), Ok(ReserveBytes));
    assert!(
        registry
            .rollback_status(Policyd)
            .unwrap()
            .depleted_reserves
            .is_empty()
    );
    assert_eq!(fs::metadata(&reserve).unwrap().len(), ReserveBytes);
    assert!(allocated_bytes(&reserve) >= ReserveBytes);
    let signed =
        draft(&alice, &browser, commit_type_t::genesis, "reserve genesis").sign_with(&fresh_key);
    assert_eq!(admit(&registry, &alice, &signed), Ok(placement_t::trunk));
    // A restart replays the journal and reopens the reserve.
    drop(registry);
    let registry = open(&root);
    assert!(registry.unusable_users().unwrap().is_empty());
    assert_eq!(registry.reserve_bytes(alice.uid), Ok(ReserveBytes));
    assert!(matches!(
        registry.standing(alice.uid, &[0; 16]),
        Ok(standing_t::unknown)
    ));
}

#[test]
fn a_torn_restrictive_write_on_a_full_filesystem_is_truncated_back_and_retried() {
    // R03: the write of the frame stops after a few bytes with ENOSPC. The journal is
    // put back to its last complete frame, the reserve released, the frame written
    // whole; the file holds exactly the chained records afterwards.
    let root = temp_root("reserve_torn");
    let alice = test_user(1001);
    let io = space_io_t::new();
    let registry = open_with(&root, registry_config_t::default(), Box::new(io.clone()));
    enroll(&registry, &alice);
    let before = records_of(&root, alice.uid);
    let mut identity = identity_over(&registry);
    io.set(space_fault_t::torn_then_enospc(7));
    let object = invalidation(
        &alice.identity,
        &alice.temporary.key_id,
        invalidation_reason_t::compromised,
        "torn on a full disk",
    );
    identity
        .register_invalidation(Vaultd, alice.uid, &object)
        .expect("retried from the reserve");
    assert_eq!(io.counts(), (1, 1, 1));
    let after = records_of(&root, alice.uid);
    assert_eq!(after.len(), before.len() + 1);
    assert_eq!(&after[..before.len()], &before[..]);
    let parsed = parse_journal(
        &fs::read(journal_path(&root, alice.uid)).unwrap(),
        alice.uid,
    )
    .expect("chained");
    assert_eq!(parsed.records.len(), after.len());
    drop(registry);
    let registry = open(&root);
    assert!(registry.unusable_users().unwrap().is_empty());
    assert_eq!(registry.reserve_bytes(alice.uid), Ok(0));
    let mut identity = identity_over(&registry);
    assert_eq!(
        identity
            .resolve_key(Contractd, &alice.temporary.key_id)
            .unwrap()
            .invalidated,
        Some(invalidation_reason_t::compromised)
    );
}

#[test]
fn reserve_files_are_checked_like_journals_and_recreated_when_missing() {
    // R03: a reserve that is a symbolic link makes the user unusable; a missing reserve
    // (a store written before the reserve existed) is created at start and refilled by
    // the first non-restrictive append.
    let root = temp_root("reserve_paths");
    let alice = test_user(1001);
    let browser = app("browser");
    let registry = open(&root);
    enroll(&registry, &alice);
    drop(registry);
    let reserve = reserve_path(&root, alice.uid);
    fs::remove_file(&reserve).unwrap();
    std::os::unix::fs::symlink(root.join("elsewhere"), &reserve).unwrap();
    let registry = open(&root);
    assert_eq!(
        registry.unusable_users().unwrap(),
        vec![(
            alice.uid,
            unusable_reason_t::store(cham3_registryd::journal::store_error_t::unsafe_path)
        )]
    );
    drop(registry);
    fs::remove_file(&reserve).unwrap();
    let registry = open(&root);
    assert!(registry.unusable_users().unwrap().is_empty());
    assert_eq!(fs::metadata(&reserve).unwrap().len(), 0);
    assert_eq!(registry.reserve_bytes(alice.uid), Ok(0));
    let genesis = draft(&alice, &browser, commit_type_t::genesis, "paths genesis").sign(&alice);
    assert_eq!(admit(&registry, &alice, &genesis), Ok(placement_t::trunk));
    assert_eq!(registry.reserve_bytes(alice.uid), Ok(ReserveBytes));
    assert_eq!(fs::metadata(&reserve).unwrap().len(), ReserveBytes);
}

#[test]
fn a_reserve_written_without_a_completed_sync_is_never_trusted_for_its_length() {
    // M2-005 review R2-02. An allocation that writes every byte and then fails its
    // sync leaves a reserve of full length and unknown durability. The length never
    // stands in for the sync: (a) at creation, (b) when a journal is reopened and (c)
    // when a depleted reserve is replenished, the sync is repeated until it succeeds.
    let root = temp_root("reserve_unsynced");
    let alice = test_user(1001);
    let io = unsynced_allocation_io_t::new(2);
    let registry = open_with(&root, registry_config_t::default(), Box::new(io.clone()));
    let reserve = reserve_path(&root, alice.uid);
    // (a) Two enrollment attempts whose reserve sync fails: refused, no journal, the
    // reserve written in full each time but never trusted; the third is acknowledged
    // only after an allocation that synced.
    for attempt in 0..2 {
        assert_eq!(
            code(registry.append_identity(
                Identityd,
                alice.uid,
                object_kind_t::identity_enrollment,
                &alice.enrollment,
            )),
            "ipc/storage_unavailable",
            "attempt {attempt}"
        );
        assert!(!journal_path(&root, alice.uid).exists());
        assert_eq!(fs::metadata(&reserve).unwrap().len(), ReserveBytes);
    }
    assert_eq!(io.allocations(), 2);
    enroll(&registry, &alice);
    assert_eq!(io.allocations(), 3, "the retry allocated (synced) again");
    assert_eq!(registry.reserve_bytes(alice.uid), Ok(ReserveBytes));
    drop(registry);
    // (b) Reopening: a full-length reserve is synced before it counts as full. While
    // that sync fails the reserve is pending: restrictive records are unaffected and
    // the next non-restrictive append repeats the sync before it is acknowledged.
    io.fail_next(1);
    let registry = open_with(&root, registry_config_t::default(), Box::new(io.clone()));
    assert!(registry.unusable_users().unwrap().is_empty());
    assert_eq!(io.allocations(), 4);
    assert_eq!(fs::metadata(&reserve).unwrap().len(), ReserveBytes);
    assert_eq!(
        registry.reserve_bytes(alice.uid),
        Ok(0),
        "pending, whatever the file's length"
    );
    let mut identity = identity_over(&registry);
    identity
        .register_invalidation(
            Vaultd,
            alice.uid,
            &invalidation(
                &alice.identity,
                &alice.temporary.key_id,
                invalidation_reason_t::compromised,
                "pending reserve",
            ),
        )
        .expect("restrictive records never wait for the reserve");
    assert_eq!(io.allocations(), 4);
    io.fail_next(1);
    assert_eq!(
        code(identity.register_certificate(
            Vaultd,
            alice.uid,
            &certificate(
                &alice.identity,
                alice.uid,
                &test_key("unsynced certificate"),
                authority_class_t::persistent,
                1,
            ),
        )),
        "ipc/storage_unavailable"
    );
    assert_eq!(io.allocations(), 5);
    assert_eq!(registry.reserve_bytes(alice.uid), Ok(0));
    let fresh_key = test_key("synced certificate");
    identity
        .register_certificate(
            Vaultd,
            alice.uid,
            &certificate(
                &alice.identity,
                alice.uid,
                &fresh_key,
                authority_class_t::persistent,
                1,
            ),
        )
        .expect("acknowledged once the sync succeeded");
    assert_eq!(io.allocations(), 6);
    assert_eq!(registry.reserve_bytes(alice.uid), Ok(ReserveBytes));
    drop(registry);
    // (c) Replenishing a released reserve: the allocation writes it in full and fails
    // its sync; the admission is refused, the journal stays usable, and the next
    // admission allocates (syncs) again although the file already has its full length.
    fs::OpenOptions::new()
        .write(true)
        .open(&reserve)
        .unwrap()
        .set_len(0)
        .unwrap();
    let registry = open_with(&root, registry_config_t::default(), Box::new(io.clone()));
    assert_eq!(
        io.allocations(),
        6,
        "a short reserve is pending without a sync"
    );
    assert_eq!(registry.reserve_bytes(alice.uid), Ok(0));
    let browser = app("browser");
    let genesis =
        draft(&alice, &browser, commit_type_t::genesis, "unsynced genesis").sign_with(&fresh_key);
    io.fail_next(1);
    assert_eq!(
        code(admit(&registry, &alice, &genesis)),
        "ipc/storage_unavailable"
    );
    assert_eq!(io.allocations(), 7);
    assert_eq!(fs::metadata(&reserve).unwrap().len(), ReserveBytes);
    assert_eq!(registry.reserve_bytes(alice.uid), Ok(0));
    assert!(
        registry
            .lineage_head(Policyd, alice.uid, &genesis.payload.lineage_id)
            .is_ok(),
        "the journal stays usable"
    );
    io.fail_next(1);
    assert_eq!(
        code(admit(&registry, &alice, &genesis)),
        "ipc/storage_unavailable",
        "the sync is repeated, the length not trusted"
    );
    assert_eq!(io.allocations(), 8);
    assert_eq!(admit(&registry, &alice, &genesis), Ok(placement_t::trunk));
    assert_eq!(io.allocations(), 9);
    assert_eq!(registry.reserve_bytes(alice.uid), Ok(ReserveBytes));
}

#[test]
fn failed_appends_change_nothing_and_are_rolled_back() {
    // A failed admission changes nothing: the in-memory state is untouched and the
    // journal is put back to its last durable frame, so the user continues (M2-006
    // stage A; M2-005 denied the user until restart). Whether the write failed before
    // any byte, after a few bytes, or at its fdatasync, the rollback leaves no trace of
    // the record and nothing is anchored.
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = sample(&alice, &editor);
    for (label, fault) in [
        ("write", fault_t::fail_write),
        ("torn", fault_t::torn(11)),
        ("sync", fault_t::fail_sync),
    ] {
        let root = temp_root(&format!("fault_{label}"));
        let fault_state = Arc::new(Mutex::new(fault_t::none));
        let appends = Arc::new(AtomicUsize::new(0));
        let registry = open_with(
            &root,
            registry_config_t::default(),
            Box::new(faulty_io_t {
                fault: Arc::clone(&fault_state),
                appends: Arc::clone(&appends),
            }),
        );
        enroll(&registry, &alice);
        admit(&registry, &alice, &lineage.genesis).expect("genesis");
        let lineage_id = lineage.genesis.payload.lineage_id;
        let before = registry
            .lineage_head(Policyd, alice.uid, &lineage_id)
            .expect("view")
            .expect("user");
        let length_before = fs::metadata(journal_path(&root, alice.uid)).unwrap().len();
        let anchored_before = registry.anchored_user(alice.uid).unwrap();
        *fault_state.lock().unwrap() = fault;
        assert_eq!(
            code(admit(&registry, &alice, &lineage.revision)),
            "ipc/storage_unavailable",
            "{label}"
        );
        assert_eq!(
            registry
                .lineage_head(Policyd, alice.uid, &lineage_id)
                .expect("view")
                .expect("user"),
            before,
            "{label}: nothing in memory changed"
        );
        assert_eq!(
            registry.anchored_user(alice.uid).unwrap(),
            anchored_before,
            "{label}: nothing anchored"
        );
        assert_eq!(
            fs::metadata(journal_path(&root, alice.uid)).unwrap().len(),
            length_before,
            "{label}: rolled back"
        );
        // The user continues: keys resolve and the retried admission is admitted.
        let mut identity = identity_over(&registry);
        assert!(
            identity
                .resolve_key(Contractd, &alice.persistent.key_id)
                .is_ok(),
            "{label}"
        );
        drop(identity);
        assert_eq!(
            admit(&registry, &alice, &lineage.revision),
            Ok(placement_t::trunk),
            "{label}"
        );
        drop(registry);
        let registry = open(&root);
        assert!(registry.unusable_users().unwrap().is_empty(), "{label}");
        assert_eq!(
            registry
                .lineage_head(Policyd, alice.uid, &lineage_id)
                .expect("view")
                .expect("user")
                .trunk_head
                .map(|head| head.commit_id),
            Some(lineage.revision.commit_id),
            "{label}"
        );
    }
}

#[test]
fn a_failed_rollback_denies_the_user_until_restart() {
    // When the truncation that puts the journal back after a failed write fails too,
    // the file's state is unknown: the user is denied for the rest of the run (fail
    // closed) and the next start decides from the bytes (ADR 0006 §4). A torn frame
    // beyond the anchor is discarded; a complete frame beyond it is a durable but
    // unacknowledged record that rolls the anchor forward — reported failed, found
    // admitted, reconciled by the client through its request id (ADR 0006 §1).
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = sample(&alice, &editor);
    for (label, written, after_restart) in [
        ("torn", 11usize, "absent"),
        ("whole", usize::MAX, "present"),
    ] {
        let root = temp_root(&format!("rollback_{label}"));
        let fault_state = Arc::new(Mutex::new(fault_t::none));
        let registry = open_with(
            &root,
            registry_config_t::default(),
            Box::new(faulty_io_t {
                fault: Arc::clone(&fault_state),
                appends: Arc::new(AtomicUsize::new(0)),
            }),
        );
        enroll(&registry, &alice);
        admit(&registry, &alice, &lineage.genesis).expect("genesis");
        let lineage_id = lineage.genesis.payload.lineage_id;
        let before = registry
            .lineage_head(Policyd, alice.uid, &lineage_id)
            .expect("view")
            .expect("user");
        let length_before = fs::metadata(journal_path(&root, alice.uid)).unwrap().len();
        *fault_state.lock().unwrap() = fault_t::torn_rollback_fails(written);
        assert_eq!(
            code(admit(&registry, &alice, &lineage.revision)),
            "ipc/storage_unavailable",
            "{label}"
        );
        assert_eq!(
            *fault_state.lock().unwrap(),
            fault_t::none,
            "{label}: the rollback was attempted"
        );
        assert!(
            fs::metadata(journal_path(&root, alice.uid)).unwrap().len() > length_before,
            "{label}: the failed frame's bytes stay"
        );
        // Denied for the rest of the run.
        assert_eq!(
            code(registry.lineage_head(Policyd, alice.uid, &lineage_id)),
            "ipc/storage_unavailable",
            "{label}"
        );
        assert_eq!(
            code(admit(&registry, &alice, &lineage.revision)),
            "ipc/storage_unavailable",
            "{label}"
        );
        let mut identity = identity_over(&registry);
        assert_eq!(
            code(identity.resolve_key(Contractd, &alice.persistent.key_id)),
            "key_authority/key_not_certified",
            "{label}"
        );
        drop(identity);
        drop(registry);
        let registry = open(&root);
        assert!(registry.unusable_users().unwrap().is_empty(), "{label}");
        let view = registry
            .lineage_head(Policyd, alice.uid, &lineage_id)
            .expect("view")
            .expect("user");
        match after_restart {
            "absent" => {
                assert_eq!(view, before, "{label}");
                assert_eq!(
                    fs::metadata(journal_path(&root, alice.uid)).unwrap().len(),
                    length_before,
                    "{label}: the torn frame is discarded at start"
                );
                assert_eq!(
                    admit(&registry, &alice, &lineage.revision),
                    Ok(placement_t::trunk),
                    "{label}"
                );
            }
            _ => {
                assert_eq!(
                    view.trunk_head.map(|head| head.commit_id),
                    Some(lineage.revision.commit_id),
                    "{label}"
                );
                assert_eq!(
                    registry
                        .anchored_user(alice.uid)
                        .unwrap()
                        .map(|entry| entry.seq),
                    Some(view.journal_seq),
                    "{label}: the anchor rolled forward"
                );
                assert_eq!(
                    code(admit(&registry, &alice, &lineage.revision)),
                    "admission/stale_head",
                    "{label}"
                );
            }
        }
    }
}

#[test]
fn operations_are_limited_to_their_roles() {
    let root = temp_root("roles");
    let alice = test_user(1001);
    let lineage = sample(&alice, &app("editor"));
    let registry = open(&root);
    enroll(&registry, &alice);
    let lineage_id = lineage.genesis.payload.lineage_id;
    for role in every_role() {
        let admit_allowed = role == Contractd;
        let head_allowed = role == Contractd || role == Policyd;
        let identity_allowed = role == Identityd;
        let admitted = registry.admit(role, alice.uid, &lineage.genesis.envelope);
        assert!(admitted.is_ok() || !admit_allowed);
        if !admit_allowed {
            assert_eq!(code(admitted), "ipc/role_denied", "{role:?}");
        }
        let head = registry.lineage_head(role, alice.uid, &lineage_id);
        if head_allowed {
            assert!(head.is_ok(), "{role:?}");
        } else {
            assert_eq!(code(head), "ipc/role_denied", "{role:?}");
        }
        let state = registry.identity_state(role, alice.uid, 0);
        let users = registry.journal_users(role);
        let append = registry.append_identity(
            role,
            alice.uid,
            object_kind_t::identity_enrollment,
            &alice.enrollment,
        );
        if identity_allowed {
            assert!(state.is_ok() && users.is_ok());
            assert!(append.expect("idempotent").already_present);
        } else {
            assert_eq!(code(state), "ipc/role_denied", "{role:?}");
            assert_eq!(code(users), "ipc/role_denied", "{role:?}");
            assert_eq!(code(append), "ipc/role_denied", "{role:?}");
        }
    }
    // append_identity accepts identity objects only.
    assert_eq!(
        code(registry.append_identity(
            Identityd,
            alice.uid,
            object_kind_t::commit,
            &lineage.revision.envelope
        )),
        "ipc/malformed_request"
    );
}

#[test]
fn stale_epochs_lineage_limits_and_unknown_users_are_refused() {
    let root = temp_root("epochs");
    let alice = test_user(1001);
    let bob = test_user(1002);
    let config = registry_config_t {
        policy_epoch: 3,
        min_valid_policy_epoch: 2,
        max_lineages: 1,
        ..registry_config_t::default()
    };
    let registry = open_with(&root, config, Box::new(system_io_t));
    enroll(&registry, &alice);
    let editor = app("editor");
    let stale = draft(&alice, &editor, commit_type_t::genesis, "stale")
        .add(vec![file_grant(
            "a",
            alice.uid,
            "home:a",
            &["read"],
            persistent_lifetime(),
        )])
        .epoch(2)
        .sign(&alice);
    let journal_length = fs::metadata(journal_path(&root, alice.uid)).unwrap().len();
    assert_eq!(code(admit(&registry, &alice, &stale)), "ipc/stale_epoch");
    let future = draft(&alice, &editor, commit_type_t::genesis, "future")
        .add(vec![file_grant(
            "b",
            alice.uid,
            "home:b",
            &["read"],
            persistent_lifetime(),
        )])
        .epoch(4)
        .sign(&alice);
    assert_eq!(code(admit(&registry, &alice, &future)), "ipc/stale_epoch");
    assert_eq!(
        fs::metadata(journal_path(&root, alice.uid)).unwrap().len(),
        journal_length
    );
    let current = draft(&alice, &editor, commit_type_t::genesis, "current")
        .add(vec![file_grant(
            "c",
            alice.uid,
            "home:c",
            &["read"],
            persistent_lifetime(),
        )])
        .epoch(3)
        .sign(&alice);
    assert_eq!(admit(&registry, &alice, &current), Ok(placement_t::trunk));
    let second = draft(
        &alice,
        &app("viewer"),
        commit_type_t::genesis,
        "second lineage",
    )
    .add(vec![file_grant(
        "d",
        alice.uid,
        "home:d",
        &["read"],
        persistent_lifetime(),
    )])
    .epoch(3)
    .sign(&alice);
    assert_eq!(
        code(admit(&registry, &alice, &second)),
        "ipc/quota_exhausted"
    );
    // Bob has no journal: nothing of his is admitted, and Alice's journal does not
    // accept a commit naming Bob.
    let bobs = draft(&bob, &editor, commit_type_t::genesis, "bob")
        .epoch(3)
        .add(vec![file_grant(
            "e",
            bob.uid,
            "home:e",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&bob);
    assert_eq!(
        code(admit(&registry, &bob, &bobs)),
        "key_authority/key_not_certified"
    );
    assert_eq!(
        code(registry.admit(Contractd, alice.uid, &bobs.envelope)),
        "signature/unknown_key"
    );
    // A commit Alice's key signed that names Bob as actor is not Alice's authority.
    let mislabeled = draft(&alice, &app("third"), commit_type_t::genesis, "mislabeled")
        .epoch(3)
        .add(vec![file_grant(
            "f",
            alice.uid,
            "home:f",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&alice);
    assert_eq!(
        code(admit(&registry, &bob, &mislabeled)),
        "key_authority/key_not_certified"
    );
    assert!(
        registry
            .lineage_head(Policyd, bob.uid, &bobs.payload.lineage_id)
            .expect("view")
            .is_none()
    );
    // Invalid configurations are refused.
    for config in [
        registry_config_t {
            quota_bytes: 1024,
            ..registry_config_t::default()
        },
        registry_config_t {
            max_lineages: 0,
            ..registry_config_t::default()
        },
        registry_config_t {
            policy_epoch: 1,
            min_valid_policy_epoch: 2,
            ..registry_config_t::default()
        },
    ] {
        assert_eq!(
            registry_t::open(
                &root,
                owner_of(&root),
                config,
                Box::new(system_io_t),
                fixed_clock()
            )
            .err(),
            Some(registry_start_error_t::invalid_config)
        );
    }
}

#[test]
fn duplicate_request_ids_are_rejected_across_the_users_lineages() {
    // M2-004 carried obligation: admission rejects a repeated request id even in
    // another lineage, and in another user's journal it is that user's own namespace.
    let root = temp_root("requests");
    let alice = test_user(1001);
    let registry = open(&root);
    enroll(&registry, &alice);
    let first = draft(
        &alice,
        &app("editor"),
        commit_type_t::genesis,
        "same request",
    )
    .add(vec![file_grant(
        "g1",
        alice.uid,
        "home:g1",
        &["read"],
        persistent_lifetime(),
    )])
    .sign(&alice);
    admit(&registry, &alice, &first).expect("first");
    let other_lineage = draft(
        &alice,
        &app("viewer"),
        commit_type_t::genesis,
        "same request",
    )
    .add(vec![file_grant(
        "g2",
        alice.uid,
        "home:g2",
        &["read"],
        persistent_lifetime(),
    )])
    .sign(&alice);
    assert_eq!(
        other_lineage.payload.authorization.request_id,
        first.payload.authorization.request_id
    );
    assert_eq!(
        code(admit(&registry, &alice, &other_lineage)),
        "admission/duplicate_identifier"
    );
    // A reused grant id in another lineage is refused too.
    let reused_grant = draft(
        &alice,
        &app("viewer"),
        commit_type_t::genesis,
        "fresh request",
    )
    .add(vec![file_grant(
        "g1",
        alice.uid,
        "home:g1",
        &["read"],
        persistent_lifetime(),
    )])
    .sign(&alice);
    assert_eq!(
        code(admit(&registry, &alice, &reused_grant)),
        "admission/duplicate_identifier"
    );
}

#[test]
fn the_byte_quota_keeps_its_reserve_for_restrictive_records() {
    let root = temp_root("quota");
    let alice = test_user(1001);
    let config = registry_config_t {
        quota_bytes: cham3_registryd::registry::MinQuotaBytes,
        ..registry_config_t::default()
    };
    let registry = open_with(&root, config, Box::new(system_io_t));
    enroll(&registry, &alice);
    // Large genesis commits (32 grants with long resource ids, about 34 KB each) until
    // the quota minus the reserve is reached.
    let long = "r".repeat(400);
    let mut admitted = 0usize;
    let refusal = loop {
        let label = format!("bulk {admitted}");
        let grants = (0..32)
            .map(|index| {
                file_grant(
                    &format!("{label} {index}"),
                    alice.uid,
                    &format!("{long}{index}"),
                    &["read"],
                    persistent_lifetime(),
                )
            })
            .collect();
        let commit = draft(
            &alice,
            &app(&format!("bulk{admitted}")),
            commit_type_t::genesis,
            &label,
        )
        .add(grants)
        .sign(&alice);
        match admit(&registry, &alice, &commit) {
            Ok(_) => admitted += 1,
            Err(error) => break error.to_string(),
        }
        assert!(admitted < 1000, "quota never reached");
    };
    assert_eq!(refusal, "ipc/quota_exhausted");
    let length = fs::metadata(journal_path(&root, alice.uid)).unwrap().len();
    let limit = cham3_registryd::registry::MinQuotaBytes - cham3_registryd::registry::ReserveBytes;
    assert!(
        length <= limit && length > limit - 70_000,
        "length {length}"
    );
    // Certificates are not restrictive: they fill the rest of the quota minus the
    // reserve, then are refused. A key invalidation is restrictive: accepted from the
    // reserve.
    let mut identity = identity_over(&registry);
    let mut certificates = 0usize;
    loop {
        let extra = test_key(&format!("quota extra persistent {certificates}"));
        let certificate = certificate(
            &alice.identity,
            alice.uid,
            &extra,
            authority_class_t::persistent,
            1,
        );
        match identity.register_certificate(Vaultd, alice.uid, &certificate) {
            Ok(_) => certificates += 1,
            Err(error) => {
                assert_eq!(error.to_string(), "ipc/quota_exhausted");
                break;
            }
        }
        assert!(
            certificates < 200,
            "the certificate quota was never reached"
        );
    }
    let length = fs::metadata(journal_path(&root, alice.uid)).unwrap().len();
    assert!(length <= limit && length > limit - 1_000, "length {length}");
    assert_eq!(
        code(registry.append_identity(
            Identityd,
            alice.uid,
            object_kind_t::key_certificate,
            &certificate(
                &alice.identity,
                alice.uid,
                &test_key("quota direct"),
                authority_class_t::persistent,
                1
            ),
        )),
        "ipc/quota_exhausted"
    );
    // Invalidations of the extra keys are accepted until the journal is past the
    // quota minus the reserve: they are written from the reserve.
    let mut invalidated = 0usize;
    while fs::metadata(journal_path(&root, alice.uid)).unwrap().len() <= limit {
        assert!(invalidated < certificates, "the reserve was never used");
        let extra = test_key(&format!("quota extra persistent {invalidated}"));
        let invalidation = invalidation(
            &alice.identity,
            &extra.key_id,
            invalidation_reason_t::retired,
            &format!("quota {invalidated}"),
        );
        identity
            .register_invalidation(Vaultd, alice.uid, &invalidation)
            .expect("restrictive record within the reserve");
        invalidated += 1;
    }
    assert!(fs::metadata(journal_path(&root, alice.uid)).unwrap().len() > limit);
}

#[test]
fn conflicting_concurrent_admissions_serialize() {
    let root = temp_root("concurrency");
    let alice = test_user(1001);
    let bob = test_user(1002);
    let registry = open(&root);
    enroll(&registry, &alice);
    enroll(&registry, &bob);
    let editor = app("editor");
    let mut alice_head = draft(&alice, &editor, commit_type_t::genesis, "race genesis")
        .add(vec![file_grant(
            "race base",
            alice.uid,
            "home:base",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&alice);
    admit(&registry, &alice, &alice_head).expect("genesis");
    let mut bob_head = draft(&bob, &editor, commit_type_t::genesis, "bob race genesis")
        .add(vec![file_grant(
            "bob race base",
            bob.uid,
            "home:base",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&bob);
    admit(&registry, &bob, &bob_head).expect("bob genesis");
    for round in 0..16 {
        // Two revisions prepared against the same trunk head, plus one for Bob.
        let candidates: Vec<commit_t> = (0..2)
            .map(|writer| {
                draft(
                    &alice,
                    &editor,
                    commit_type_t::revision,
                    &format!("race {round} {writer}"),
                )
                .on(&alice_head)
                .add(vec![file_grant(
                    &format!("race {round} {writer}"),
                    alice.uid,
                    "home:race",
                    &["read"],
                    persistent_lifetime(),
                )])
                .sign(&alice)
            })
            .collect();
        let bob_next = draft(
            &bob,
            &editor,
            commit_type_t::revision,
            &format!("bob race {round}"),
        )
        .on(&bob_head)
        .add(vec![file_grant(
            &format!("bob race {round}"),
            bob.uid,
            "home:race",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&bob);
        let barrier = Arc::new(Barrier::new(3));
        let mut workers = Vec::new();
        for candidate in candidates.clone() {
            let registry = Arc::clone(&registry);
            let barrier = Arc::clone(&barrier);
            let uid = alice.uid;
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                registry
                    .admit(Contractd, uid, &candidate.envelope)
                    .map(|_| candidate.commit_id)
            }));
        }
        let bob_worker = {
            let registry = Arc::clone(&registry);
            let barrier = Arc::clone(&barrier);
            let envelope = bob_next.envelope.clone();
            std::thread::spawn(move || {
                barrier.wait();
                registry.admit(Contractd, 1002, &envelope)
            })
        };
        let results: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().expect("join"))
            .collect();
        bob_worker
            .join()
            .expect("join")
            .expect("bob is not serialized behind alice's conflict");
        bob_head = bob_next;
        let winners: Vec<digest_t> = results
            .iter()
            .filter_map(|result| result.as_ref().ok().copied())
            .collect();
        assert_eq!(
            winners.len(),
            1,
            "round {round}: exactly one conflicting writer wins"
        );
        let loser = results
            .iter()
            .find_map(|result| result.as_ref().err())
            .expect("one loser");
        assert_eq!(loser.to_string(), "admission/stale_head", "round {round}");
        alice_head = candidates
            .into_iter()
            .find(|candidate| candidate.commit_id == winners[0])
            .expect("winner");
    }
    // The journal replays to the same heads.
    let lineage_id = alice_head.payload.lineage_id;
    let before = registry
        .lineage_head(Policyd, alice.uid, &lineage_id)
        .expect("view");
    drop(registry);
    let registry = open(&root);
    assert_eq!(
        registry
            .lineage_head(Policyd, alice.uid, &lineage_id)
            .expect("view"),
        before
    );
    assert_eq!(
        before
            .and_then(|view| view.trunk_head)
            .map(|head| head.commit_id),
        Some(alice_head.commit_id)
    );
}

#[test]
fn identity_state_pages_identity_records_in_journal_order() {
    let root = temp_root("paging");
    let alice = test_user(1001);
    let registry = open(&root);
    enroll(&registry, &alice);
    let mut identity = identity_over(&registry);
    let lineage = sample(&alice, &app("editor"));
    admit(&registry, &alice, &lineage.genesis).expect("genesis");
    let mut expected = vec![1u64, 2, 3];
    for index in 0..20 {
        let key = test_key(&format!("paging persistent {index}"));
        let certificate = certificate(
            &alice.identity,
            alice.uid,
            &key,
            authority_class_t::persistent,
            1,
        );
        let receipt = identity
            .register_certificate(Vaultd, alice.uid, &certificate)
            .expect("certificate");
        expected.push(receipt.journal_seq);
    }
    // Sequence 4 is the commit; identity records skip it.
    assert_eq!(expected[3], 5);
    let mut cursor = 0;
    let mut seen = Vec::new();
    loop {
        let page = registry
            .identity_state(Identityd, alice.uid, cursor)
            .expect("page");
        assert!(page.entries.len() <= 16);
        for entry in &page.entries {
            assert!(matches!(entry.record, journal_record_t::identity { .. }));
            seen.push(entry.sequence);
        }
        match page.next {
            Some(next) => cursor = next,
            None => break,
        }
    }
    assert_eq!(seen, expected);
    // A restarted identityd over the restarted registry resolves every key.
    drop(identity);
    drop(registry);
    let registry = open(&root);
    let mut identity = identity_over(&registry);
    for index in 0..20 {
        let key = test_key(&format!("paging persistent {index}"));
        assert!(identity.resolve_key(Contractd, &key.key_id).is_ok());
    }
}

#[test]
fn unsafe_store_paths_are_refused() {
    let alice = test_user(1001);
    // A root readable by others.
    let root = temp_root("unsafe_root");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(
        registry_t::open(
            &root,
            owner_of(&root),
            registry_config_t::default(),
            Box::new(system_io_t),
            fixed_clock()
        ),
        Err(registry_start_error_t::store(_))
    ));
    // An entry in `users` that is not a canonical UID.
    let root = temp_root("unsafe_entry");
    let registry = open(&root);
    drop(registry);
    fs::create_dir(root.join("users").join("01001")).unwrap();
    assert!(matches!(
        registry_t::open(
            &root,
            owner_of(&root),
            registry_config_t::default(),
            Box::new(system_io_t),
            fixed_clock()
        ),
        Err(registry_start_error_t::store(_))
    ));
    // A journal that is a symbolic link, or readable by others: that user is unusable.
    for label in ["symlink", "mode"] {
        let root = temp_root(&format!("unsafe_{label}"));
        let registry = open(&root);
        enroll(&registry, &alice);
        drop(registry);
        let path = journal_path(&root, alice.uid);
        if label == "symlink" {
            let moved = root.join("elsewhere.c3j");
            fs::rename(&path, &moved).unwrap();
            std::os::unix::fs::symlink(&moved, &path).unwrap();
        } else {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        }
        let registry = open(&root);
        assert_eq!(
            registry.unusable_users().expect("users").len(),
            1,
            "{label}"
        );
    }
}

#[test]
fn delegated_grants_are_refused_until_their_admission_rules_exist() {
    // Interim, fail closed: wire format §6.4's delegation checks are M2-006.
    let root = temp_root("delegation");
    let alice = test_user(1001);
    let registry = open(&root);
    enroll(&registry, &alice);
    let editor = app("editor");
    let mut parent = file_grant(
        "delegating parent",
        alice.uid,
        "home:Shared",
        &["read", "write"],
        persistent_lifetime(),
    );
    parent.delegation.permitted = true;
    parent.delegation.max_depth = 2;
    parent.delegation.max_operations = vec!["read".to_owned()];
    let genesis = draft(
        &alice,
        &editor,
        commit_type_t::genesis,
        "delegation genesis",
    )
    .add(vec![parent.clone()])
    .sign(&alice);
    assert_eq!(admit(&registry, &alice, &genesis), Ok(placement_t::trunk));
    let mut child = file_grant(
        "delegated child",
        alice.uid,
        "home:Shared",
        &["read"],
        persistent_lifetime(),
    );
    child.delegation.parent_grant_id = Some(parent.grant_id);
    let delegated = draft(&alice, &editor, commit_type_t::revision, "delegated")
        .on(&genesis)
        .add(vec![child])
        .sign(&alice);
    assert_eq!(
        code(admit(&registry, &alice, &delegated)),
        "admission/delegation_rule"
    );
}

#[test]
fn admission_requires_the_signing_keys_window_at_registry_time() {
    // T0 = 2026-10-13T08:00:00Z (every certificate's not_before); T2 =
    // 2026-10-14T08:00:00Z (the temporary certificate's not_after). Each commit's
    // timestamp is within 300 s of trusted now (the commit-timestamp rule, checked
    // before the window), so the window at trusted now is what decides.
    const T0Utc: u64 = 1_791_878_400;
    const T2Utc: u64 = 1_791_964_800;
    let root = temp_root("key_window");
    let alice = test_user(1001);
    let time = test_time_t::at(T0Utc - 1);
    let open_at = |time: &test_time_t| {
        Arc::new(
            registry_t::open(
                &root,
                owner_of(&root),
                registry_config_t::default(),
                Box::new(system_io_t),
                Box::new(time.clone()),
            )
            .expect("registry"),
        )
    };
    let registry = open_at(&time);
    enroll(&registry, &alice);
    let editor = app("editor");
    let genesis_at = |timestamp: &str| {
        draft(&alice, &editor, commit_type_t::genesis, "window genesis")
            .add(vec![file_grant(
                "w",
                alice.uid,
                "home:W",
                &["read"],
                persistent_lifetime(),
            )])
            .timestamp(timestamp)
            .sign(&alice)
    };
    let genesis = genesis_at(T1);
    let branch = |label: &str, timestamp: &str| {
        draft(&alice, &editor, commit_type_t::temporary_grant, label)
            .on(&genesis)
            .add(vec![file_grant(
                label,
                alice.uid,
                "home:Tmp",
                &["read"],
                timed_lifetime("2026-10-20T00:00:00Z"),
            )])
            .timestamp(timestamp)
            .temporary_authority()
            .sign(&alice)
    };
    // Before every certificate's not_before: refused, even for the persistent key
    // (the commit's own timestamp, T0, is inside the window).
    assert_eq!(
        code(admit(
            &registry,
            &alice,
            &genesis_at("2026-10-13T08:00:00Z")
        )),
        "key_authority/certificate_window"
    );
    time.pass_until(T0Utc + 3600);
    assert_eq!(admit(&registry, &alice, &genesis), Ok(placement_t::trunk));
    let early = branch("window early", T1);
    assert_eq!(admit(&registry, &alice, &early), Ok(placement_t::temporary));
    // Signed at the temporary window's inclusive end: admitted at T2, refused one
    // second after the window closed although its own timestamp is inside it.
    let at_end = branch("window at end", "2026-10-14T08:00:00Z");
    let late = branch("window late", "2026-10-14T08:00:00Z");
    time.pass_until(T2Utc);
    assert_eq!(
        admit(&registry, &alice, &at_end),
        Ok(placement_t::temporary),
        "inclusive end"
    );
    time.pass_until(T2Utc + 1);
    assert_eq!(
        code(admit(&registry, &alice, &late)),
        "key_authority/certificate_window"
    );
    // The persistent key has no not_after.
    let revision = draft(&alice, &editor, commit_type_t::revision, "window revision")
        .on(&genesis)
        .add(vec![file_grant(
            "w2",
            alice.uid,
            "home:W2",
            &["read"],
            persistent_lifetime(),
        )])
        .timestamp("2026-10-14T08:00:01Z")
        .sign(&alice);
    assert_eq!(admit(&registry, &alice, &revision), Ok(placement_t::trunk));
    // Replay is not an admission: the earlier temporary commits replay after the
    // window closed.
    drop(registry);
    let registry = open_at(&time);
    assert_eq!(registry.unusable_users().expect("users"), Vec::new());
    assert_eq!(
        registry.standing(alice.uid, &synthetic_id("grant window early")),
        Ok(standing_t::inactive)
    );
}
