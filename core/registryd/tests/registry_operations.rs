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
use cham3_identityd::outcome::service_error_t;
use cham3_identityd::service::identity_service_t;
use cham3_registryd::journal::{journal_io_t, system_io_t};
use cham3_registryd::lineage::standing_t;
use cham3_registryd::record::{
    FrameOverheadBytes, HeaderBytes, frame, genesis_chain, header, lifecycle_record_t, next_chain,
    parse_journal, placement_t, record_body_t, registry_record_t,
};
use cham3_registryd::registry::{
    registry_clock_t, registry_config_t, registry_identity_store_t, registry_start_error_t,
    registry_t, unusable_reason_t,
};

const Contractd: caller_role_t = caller_role_t::service(service_name_t::contractd);
const Policyd: caller_role_t = caller_role_t::service(service_name_t::policyd);
const Identityd: caller_role_t = caller_role_t::service(service_name_t::identityd);
const Vaultd: caller_role_t = caller_role_t::service(service_name_t::vaultd);

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

fn code(result: Result<impl std::fmt::Debug, service_error_t>) -> String {
    match result {
        Ok(value) => panic!("expected a refusal, got {value:?}"),
        Err(error) => error.to_string(),
    }
}

struct fixed_clock_t;

impl registry_clock_t for fixed_clock_t {
    fn utc_seconds(&self) -> i64 {
        // 2026-10-13T09:00:00Z
        1_791_882_000
    }
}

/// Fault injection at the journal's durability boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum fault_t {
    none,
    /// The next append's write fails before any byte is written.
    fail_write,
    /// The next append writes only this many bytes, then fails.
    torn(usize),
    /// The next append's write succeeds and its fdatasync fails.
    fail_sync,
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
            fault_t::none | fault_t::fail_sync => system_io_t.write_all(file, bytes),
        }
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

fn temp_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("cham3_registryd_{label}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("temp root");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("mode");
    root
}

fn owner_of(root: &Path) -> u32 {
    fs::metadata(root).expect("root").uid()
}

fn open_with(root: &Path, config: registry_config_t, io: Box<dyn journal_io_t>) -> Arc<registry_t> {
    Arc::new(
        registry_t::open(root, owner_of(root), config, io, Box::new(fixed_clock_t))
            .expect("registry start"),
    )
}

fn open(root: &Path) -> Arc<registry_t> {
    open_with(root, registry_config_t::default(), Box::new(system_io_t))
}

fn identity_over(registry: &Arc<registry_t>) -> identity_service_t<registry_identity_store_t> {
    identity_service_t::start(registry_identity_store_t {
        registry: Arc::clone(registry),
    })
    .expect("identityd start")
}

/// Enrolls `user` through identityd, which persists through the registry.
fn enroll(registry: &Arc<registry_t>, user: &test_user_t) {
    let mut identity = identity_over(registry);
    identity
        .register_enrollment(
            Vaultd,
            user.uid,
            &user.enrollment,
            &user.persistent_certificate,
            &user.temporary_certificate,
        )
        .expect("enrollment");
}

fn admit(
    registry: &registry_t,
    user: &test_user_t,
    commit: &commit_t,
) -> Result<placement_t, service_error_t> {
    registry
        .admit(Contractd, user.uid, &commit.envelope)
        .map(|receipt| {
            assert_eq!(receipt.commit_id, commit.commit_id);
            receipt.placement
        })
}

fn journal_path(root: &Path, uid: u32) -> PathBuf {
    root.join("users").join(uid.to_string()).join("journal.c3j")
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
    let alice = test_user(1001);
    let bob = test_user(1002);
    let editor = app("editor");
    let alice_lineage = sample(&alice, &editor);
    let bob_lineage = sample(&bob, &editor);
    type edit_t = Box<dyn Fn(&mut Vec<u8>)>;
    let cases: [(&str, edit_t); 4] = [
        (
            "flipped",
            Box::new(|bytes: &mut Vec<u8>| {
                let position = bytes.len() - 100;
                bytes[position] ^= 1;
            }),
        ),
        (
            "torn",
            Box::new(|bytes: &mut Vec<u8>| bytes.truncate(bytes.len() - 7)),
        ),
        (
            "truncated_header",
            Box::new(|bytes: &mut Vec<u8>| bytes.truncate(20)),
        ),
        (
            "appended_garbage",
            Box::new(|bytes: &mut Vec<u8>| bytes.extend_from_slice(b"C3JRxxxx")),
        ),
    ];
    for (label, edit) in cases {
        let root = temp_root(&format!("tamper_{label}"));
        let registry = open(&root);
        enroll(&registry, &alice);
        enroll(&registry, &bob);
        admit(&registry, &alice, &alice_lineage.genesis).expect("alice genesis");
        admit(&registry, &bob, &bob_lineage.genesis).expect("bob genesis");
        drop(registry);
        write_tampered(&root, alice.uid, edit);
        let registry = open(&root);
        let unusable = registry.unusable_users().expect("users");
        assert_eq!(unusable.len(), 1, "{label}");
        assert_eq!(unusable[0].0, alice.uid, "{label}");
        let lineage_id = alice_lineage.genesis.payload.lineage_id;
        assert_eq!(
            code(registry.lineage_head(Policyd, alice.uid, &lineage_id)),
            "ipc/storage_unavailable",
            "{label}"
        );
        assert_eq!(
            code(admit(&registry, &alice, &alice_lineage.revision)),
            "ipc/storage_unavailable"
        );
        assert_eq!(
            code(registry.identity_state(Identityd, alice.uid, 0)),
            "ipc/storage_unavailable"
        );
        // A freshly started identityd cannot read the user's keys: they resolve as not
        // certified (denied).
        let mut identity = identity_over(&registry);
        assert_eq!(
            code(identity.resolve_key(Contractd, &alice.persistent.key_id)),
            "key_authority/key_not_certified"
        );
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

fn records_of(root: &Path, uid: u32) -> Vec<Vec<u8>> {
    parse_journal(&fs::read(journal_path(root, uid)).expect("read"), uid)
        .expect("parse")
        .records
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
    let cases: Vec<(&str, Vec<Vec<u8>>, u64)> = vec![
        // The control: the original records replay.
        ("control", original.clone(), 0),
        // A commit whose recorded placement differs from the recomputed one.
        (
            "placement",
            vec![
                original[0].clone(),
                original[1].clone(),
                original[2].clone(),
                commit_record(4, &lineage.genesis, placement_t::temporary),
            ],
            4,
        ),
        // A commit admitted twice.
        (
            "twice",
            vec![
                original[0].clone(),
                original[1].clone(),
                original[2].clone(),
                original[3].clone(),
                commit_record(5, &lineage.genesis, placement_t::trunk),
            ],
            5,
        ),
        // A commit whose parent is not in the journal (the genesis record removed).
        (
            "parent",
            vec![
                original[0].clone(),
                original[1].clone(),
                original[2].clone(),
                commit_record(4, &lineage.revision, placement_t::trunk),
            ],
            4,
        ),
        // A signature that does not verify.
        (
            "signature",
            vec![
                original[0].clone(),
                original[1].clone(),
                original[2].clone(),
                original[3].clone(),
                record(
                    5,
                    record_body_t::commit_admitted {
                        commit_id: cham3_contract_format::hash::sha256(&forged_signature),
                        lineage_id: lineage.revision.payload.lineage_id,
                        placement: placement_t::trunk,
                        envelope: forged_signature.clone(),
                    },
                ),
            ],
            5,
        ),
        // A sequence number out of order.
        (
            "sequence",
            vec![
                original[0].clone(),
                original[1].clone(),
                original[2].clone(),
                commit_record(9, &lineage.genesis, placement_t::trunk),
            ],
            4,
        ),
        // A commit before the user's certificates.
        (
            "before_certificates",
            vec![
                original[0].clone(),
                commit_record(2, &lineage.genesis, placement_t::trunk),
            ],
            2,
        ),
        // A first record that is not the enrollment.
        ("first_not_enrollment", vec![original[1].clone()], 1),
        // A lifecycle record that fails its rule (expiry of a grant without expiration).
        (
            "lifecycle",
            vec![
                original[0].clone(),
                original[1].clone(),
                original[2].clone(),
                original[3].clone(),
                record(
                    5,
                    record_body_t::lifecycle(lifecycle_record_t::expired {
                        grant_id: synthetic_id("grant downloads read"),
                    }),
                ),
            ],
            5,
        ),
        // A second, valid commit on top: replays.
        (
            "valid_extension",
            vec![
                original[0].clone(),
                original[1].clone(),
                original[2].clone(),
                original[3].clone(),
                commit_record(5, &unsigned_epoch, placement_t::trunk),
            ],
            0,
        ),
    ];
    for (label, records, failing) in cases {
        write_chained(&root, alice.uid, &user_id, &records);
        let registry = open(&root);
        let unusable = registry.unusable_users().expect("users");
        if failing == 0 {
            assert_eq!(unusable, Vec::new(), "{label}");
        } else {
            assert_eq!(
                unusable,
                vec![(alice.uid, unusable_reason_t::replay { sequence: failing })],
                "{label}"
            );
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
fn failed_appends_change_nothing_and_deny_until_restart() {
    let alice = test_user(1001);
    let editor = app("editor");
    let lineage = sample(&alice, &editor);
    for (label, fault, after_restart) in [
        ("write", fault_t::fail_write, "absent"),
        ("torn", fault_t::torn(11), "unusable"),
        ("sync", fault_t::fail_sync, "present"),
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
        *fault_state.lock().unwrap() = fault;
        assert_eq!(
            code(admit(&registry, &alice, &lineage.revision)),
            "ipc/storage_unavailable",
            "{label}"
        );
        // Nothing in memory changed, and the user is denied for the rest of the run.
        assert_eq!(
            code(registry.lineage_head(Policyd, alice.uid, &lineage_id)),
            "ipc/storage_unavailable"
        );
        assert_eq!(
            code(admit(&registry, &alice, &lineage.revision)),
            "ipc/storage_unavailable"
        );
        let mut identity = identity_over(&registry);
        assert_eq!(
            code(identity.resolve_key(Contractd, &alice.persistent.key_id)),
            "key_authority/key_not_certified"
        );
        drop(identity);
        drop(registry);
        let registry = open(&root);
        match after_restart {
            "absent" => {
                let view = registry
                    .lineage_head(Policyd, alice.uid, &lineage_id)
                    .expect("view")
                    .expect("user");
                assert_eq!(view, before, "{label}");
                assert_eq!(
                    admit(&registry, &alice, &lineage.revision),
                    Ok(placement_t::trunk)
                );
            }
            "present" => {
                // Reported failed, found admitted after the restart: it stays pending and
                // the client reconciles by request id (ADR 0006 §1).
                let view = registry
                    .lineage_head(Policyd, alice.uid, &lineage_id)
                    .expect("view")
                    .expect("user");
                assert_eq!(
                    view.trunk_head.map(|head| head.commit_id),
                    Some(lineage.revision.commit_id)
                );
                assert_eq!(
                    code(admit(&registry, &alice, &lineage.revision)),
                    "admission/stale_head"
                );
            }
            _ => {
                assert_eq!(
                    registry.unusable_users().expect("users"),
                    vec![(
                        alice.uid,
                        unusable_reason_t::defect(
                            cham3_registryd::record::journal_defect_t::torn_tail
                        )
                    )],
                    "{label}: torn-tail recovery is M2-006"
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
                Box::new(fixed_clock_t)
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
    let invalidation = invalidation(
        &alice.identity,
        &alice.temporary.key_id,
        invalidation_reason_t::retired,
        "quota",
    );
    identity
        .register_invalidation(Vaultd, alice.uid, &invalidation)
        .expect("restrictive record within the reserve");
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
            Box::new(fixed_clock_t)
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
            Box::new(fixed_clock_t)
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
    let mut parent = file_grant("delegating parent", alice.uid, "home:Shared", &["read", "write"], persistent_lifetime());
    parent.delegation.permitted = true;
    parent.delegation.max_depth = 2;
    parent.delegation.max_operations = vec!["read".to_owned()];
    let genesis = draft(&alice, &editor, commit_type_t::genesis, "delegation genesis")
        .add(vec![parent.clone()])
        .sign(&alice);
    assert_eq!(admit(&registry, &alice, &genesis), Ok(placement_t::trunk));
    let mut child = file_grant("delegated child", alice.uid, "home:Shared", &["read"], persistent_lifetime());
    child.delegation.parent_grant_id = Some(parent.grant_id);
    let delegated = draft(&alice, &editor, commit_type_t::revision, "delegated")
        .on(&genesis)
        .add(vec![child])
        .sign(&alice);
    assert_eq!(code(admit(&registry, &alice, &delegated)), "admission/delegation_rule");
}
