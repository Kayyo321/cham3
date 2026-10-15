//! Current authorization (ADR 0007 §1) over the real registry, identityd and
//! contractd composed in process: every rule of `evaluate`, `network_mode`, roles and
//! the unknown outcomes.
//!
//! Commits are submitted through contractd `submit_signed` and admitted by registryd
//! into journal files; policyd reads `lineage_head` snapshots and identityd's key
//! answers. Synthetic users and keys only.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::unwrap_used
)]

#[path = "../../registryd/tests/builders/mod.rs"]
mod builders;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use builders::*;
use cham3_contract_format::hash::digest_t;
use cham3_contract_format::object_kind_t;
use cham3_contract_format::schema::{
    commit_type_t, invalidation_reason_t, random_id_t, resource_category_t,
};
use cham3_contractd::submit::contract_service_t;
use cham3_identityd::caller::{caller_role_t, service_name_t};
use cham3_identityd::index::user_state_t;
use cham3_identityd::service::identity_service_t;
use cham3_policyd::evaluate::{
    decision_t, deny_reason_t, evaluate, key_state_t, network_decision_t, network_mode,
    resource_request_t, runtime_identity_t, session_attestation_t, session_source_t, snapshot_t,
    unknown_reason_t,
};
use cham3_policyd::mandatory::{mandatory_policy_t, network_mode_t, prohibition_t};
use cham3_policyd::service::{
    policy_clock_t, policy_service_t, reserve_outcome_t, rollback_protection_t,
};
use cham3_registryd::journal::{journal_io_t, system_io_t};
use cham3_registryd::ledger::{boot_id_source_t, kernel_boot_id_t};
use cham3_registryd::record::{
    frame, lifecycle_record_t, next_chain, parse_journal, placement_t, record_body_t,
    registry_record_t,
};
use cham3_registryd::registry::{registry_config_t, registry_identity_store_t, registry_t};

const Contractd: caller_role_t = caller_role_t::service(service_name_t::contractd);
const Launcher: caller_role_t = caller_role_t::fixture_consumer;
const Vaultd: caller_role_t = caller_role_t::service(service_name_t::vaultd);

/// 2026-10-13T09:00:00Z (`T1`, every grant's effective start).
const T1Utc: i64 = 1_791_882_000;

type identity_t = Rc<RefCell<identity_service_t<registry_identity_store_t>>>;

#[derive(Clone, Default)]
struct sessions_t {
    attested: Rc<RefCell<BTreeMap<random_id_t, session_attestation_t>>>,
}

impl session_source_t for sessions_t {
    fn liveness(&self, session_id: &random_id_t) -> Option<session_attestation_t> {
        self.attested.borrow().get(session_id).copied()
    }
}

struct world_t {
    root: PathBuf,
    registry: Arc<registry_t>,
    identity: identity_t,
    contract: contract_service_t<
        cham3_contractd::submit::in_process_identity_t<registry_identity_store_t>,
    >,
    policy:
        policy_service_t<cham3_policyd::service::in_process_identity_t<registry_identity_store_t>>,
    /// The clocks every service of the world samples (ADR 0007 §5).
    time: test_time_t,
    sessions: sessions_t,
}

fn temp_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("cham3_policyd_{label}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("root");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("mode");
    root
}

fn world_at(root: &Path, config: registry_config_t, policy: mandatory_policy_t) -> world_t {
    world_at_with(root, config, policy, Box::new(system_io_t))
}

/// Every anchor slot write fails while armed; the journal and the ledger are fine.
#[derive(Clone)]
struct anchor_fault_io_t {
    armed: Arc<AtomicBool>,
}

impl journal_io_t for anchor_fault_io_t {
    fn write_all(&self, file: &mut fs::File, bytes: &[u8]) -> io::Result<()> {
        system_io_t.write_all(file, bytes)
    }

    fn sync_data(&self, file: &fs::File) -> io::Result<()> {
        system_io_t.sync_data(file)
    }

    fn sync_directory(&self, path: &Path) -> io::Result<()> {
        system_io_t.sync_directory(path)
    }

    fn replace_file(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        if self.armed.load(Ordering::SeqCst) {
            return Err(io::Error::other("injected anchor write failure"));
        }
        system_io_t.replace_file(path, bytes)
    }
}

fn world_at_with(
    root: &Path,
    config: registry_config_t,
    policy: mandatory_policy_t,
    io: Box<dyn journal_io_t>,
) -> world_t {
    let owner = fs::metadata(root).expect("root").uid();
    let time = test_time_t::at(T1Seconds + 60);
    let boot_id = config
        .boot_id
        .clone()
        .or_else(|| kernel_boot_id_t.boot_id())
        .expect("boot id");
    let registry = Arc::new(
        registry_t::open(root, owner, config, io, Box::new(time.clone())).expect("registry"),
    );
    let identity: identity_t = Rc::new(RefCell::new(
        identity_service_t::start(registry_identity_store_t {
            registry: Arc::clone(&registry),
        })
        .expect("identityd"),
    ));
    let sessions = sessions_t::default();
    world_t {
        root: root.to_path_buf(),
        contract: contract_service_t::new(
            cham3_contractd::submit::in_process_identity_t {
                service: Rc::clone(&identity),
            },
            Arc::clone(&registry),
        ),
        policy: policy_service_t::new(
            Arc::clone(&registry),
            cham3_policyd::service::in_process_identity_t {
                service: Rc::clone(&identity),
            },
            policy,
            policy_clock_t::new(Box::new(time.clone()), &boot_id),
            Box::new(sessions.clone()),
        ),
        registry,
        identity,
        time,
        sessions,
    }
}

fn world(label: &str) -> world_t {
    world_at(
        &temp_root(label),
        registry_config_t::default(),
        mandatory_policy_t::default(),
    )
}

impl world_t {
    fn enroll(&self, user: &test_user_t) {
        self.identity
            .borrow_mut()
            .register_enrollment(
                Vaultd,
                user.uid,
                &user.enrollment,
                &user.persistent_certificate,
                &user.temporary_certificate,
            )
            .expect("enrollment");
    }

    fn submit(&mut self, user: &test_user_t, commit: &commit_t) -> placement_t {
        self.contract
            .submit_signed(
                caller_role_t::trusted_client { uid: user.uid },
                &commit.envelope,
            )
            .expect("admitted")
            .placement
    }

    fn evaluate(
        &mut self,
        identity: &runtime_identity_t,
        request: &resource_request_t,
    ) -> decision_t {
        self.policy
            .evaluate(Launcher, identity, request)
            .expect("evaluate")
    }
}

fn runtime(user: &test_user_t, app: &app_t) -> runtime_identity_t {
    runtime_identity_t {
        owner_uid: user.uid,
        app_id: app.app_id.clone(),
        installation_id: app.installation_id,
        package_identity: app.package_identity.clone(),
        executable_sha256: app.executable_sha256,
        runtime_scope: None,
        session_id: None,
    }
}

fn files(uid: u32, resource_id: &str, operation: &str) -> resource_request_t {
    resource_request_t {
        category: resource_category_t::filesystem,
        resource_id: resource_id.to_owned(),
        resource_owner_uid: uid,
        operation: operation.to_owned(),
    }
}

fn denied(reason: deny_reason_t) -> decision_t {
    decision_t::denied(reason)
}

fn is_eligible(decision: decision_t) -> bool {
    matches!(decision, decision_t::eligible { .. })
}

#[test]
fn eligible_grants_pass_and_every_rule_denies_despite_valid_signatures() {
    let mut world = world("rules");
    let alice = test_user(1001);
    world.enroll(&alice);
    let editor = app("editor");
    let read = file_grant(
        "downloads",
        alice.uid,
        "home:Downloads",
        &["read"],
        persistent_lifetime(),
    );
    // Starts two minutes after T1: admitted at T1 + 60, so a minute of observed time
    // must also pass (ADR 0007 §8 option B).
    let mut timed_window = timed_lifetime("2026-10-13T10:00:00Z");
    timed_window.effective_start = "2026-10-13T09:02:00Z".to_owned();
    let timed = file_grant("timed", alice.uid, "home:Timed", &["read"], timed_window);
    let genesis = draft(&alice, &editor, commit_type_t::genesis, "rules genesis")
        .add(vec![read.clone()])
        .sign(&alice);
    world.submit(&alice, &genesis);
    let identity = runtime(&alice, &editor);
    let decision = world.evaluate(&identity, &files(alice.uid, "home:Downloads", "read"));
    assert_eq!(
        decision,
        decision_t::eligible {
            grant_id: read.grant_id,
            commit_id: genesis.commit_id
        }
    );
    // Scope: another operation, another resource, another owner.
    assert_eq!(
        world.evaluate(&identity, &files(alice.uid, "home:Downloads", "write")),
        denied(deny_reason_t::no_matching_grant)
    );
    assert_eq!(
        world.evaluate(&identity, &files(alice.uid, "home:Documents", "read")),
        denied(deny_reason_t::no_matching_grant)
    );
    assert_eq!(
        world.evaluate(&identity, &files(1002, "home:Downloads", "read")),
        denied(deny_reason_t::resource_ownership)
    );
    // Identity: every subject field must match the authoritative subject.
    let mut other_executable = identity.clone();
    other_executable.executable_sha256 = [9; 32];
    let mut other_package = identity.clone();
    other_package.package_identity = "deb:editor-fork:amd64".to_owned();
    let mut other_scope = identity.clone();
    other_scope.runtime_scope = Some("sandbox".to_owned());
    for changed in [other_executable, other_package, other_scope] {
        assert_eq!(
            world.evaluate(&changed, &files(alice.uid, "home:Downloads", "read")),
            denied(deny_reason_t::unknown_identity)
        );
    }
    let mut other_installation = identity.clone();
    other_installation.installation_id = [7; 16];
    assert_eq!(
        world.evaluate(
            &other_installation,
            &files(alice.uid, "home:Downloads", "read")
        ),
        denied(deny_reason_t::no_matching_grant),
        "another installation is another (empty) lineage"
    );
    // Lifetime: a timed grant before its start, inside its window and at expiration.
    let branch = draft(
        &alice,
        &editor,
        commit_type_t::temporary_grant,
        "rules timed",
    )
    .on(&genesis)
    .add(vec![timed.clone()])
    .temporary_authority()
    .sign(&alice);
    assert_eq!(world.submit(&alice, &branch), placement_t::temporary);
    let timed_request = files(alice.uid, "home:Timed", "read");
    assert_eq!(
        world.evaluate(&identity, &timed_request),
        denied(deny_reason_t::not_started)
    );
    world.time.pass_until(T1Seconds + 119);
    assert_eq!(
        world.evaluate(&identity, &timed_request),
        denied(deny_reason_t::not_started)
    );
    world.time.pass_until(T1Seconds + 120);
    assert!(is_eligible(world.evaluate(&identity, &timed_request)));
    world.time.pass_until(T1Seconds + 3599);
    assert!(is_eligible(world.evaluate(&identity, &timed_request)));
    world.time.pass_until(T1Seconds + 3600);
    assert_eq!(
        world.evaluate(&identity, &timed_request),
        denied(deny_reason_t::expired)
    );
    let before = world.time.current().expect("clock");
    world.time.set_available(false, 0, 0);
    assert_eq!(
        world.evaluate(&identity, &timed_request),
        decision_t::unknown(unknown_reason_t::clock_unavailable)
    );
    world
        .time
        .set_available(true, before.realtime, before.boottime);
    // Revocation: a removed grant is no longer in the current state, and the trunk
    // commit ends the temporary branch with its timed grant. Its timestamp is trusted
    // now (the commit-timestamp rule).
    let revocation = draft(&alice, &editor, commit_type_t::revocation, "rules revoke")
        .on(&genesis)
        .remove(vec![read.grant_id])
        .timestamp("2026-10-13T10:00:00Z")
        .sign(&alice);
    world.submit(&alice, &revocation);
    assert_eq!(
        world.evaluate(&identity, &files(alice.uid, "home:Downloads", "read")),
        denied(deny_reason_t::no_matching_grant)
    );
    assert_eq!(
        world.evaluate(&identity, &timed_request),
        denied(deny_reason_t::no_matching_grant)
    );
    // Roles: consumers and services evaluate; clients and admin do not.
    for role in [
        caller_role_t::trusted_client { uid: alice.uid },
        caller_role_t::admin,
    ] {
        assert_eq!(
            world
                .policy
                .evaluate(role, &identity, &timed_request)
                .err()
                .map(|error| error.to_string()),
            Some("ipc/role_denied".to_owned())
        );
    }
    assert!(
        world
            .policy
            .evaluate(Contractd, &identity, &timed_request)
            .is_ok()
    );
    assert!(
        world
            .policy
            .evaluate(
                caller_role_t::service(service_name_t::vaultd),
                &identity,
                &timed_request
            )
            .is_ok()
    );
}

#[test]
fn mandatory_policy_and_emergency_restriction_win_over_grants() {
    let root = temp_root("mandatory");
    let alice = test_user(1001);
    let editor = app("editor");
    let grant = file_grant(
        "secrets",
        alice.uid,
        "home:Secrets",
        &["read", "write"],
        persistent_lifetime(),
    );
    let genesis = draft(&alice, &editor, commit_type_t::genesis, "mandatory genesis")
        .add(vec![grant])
        .sign(&alice);
    {
        let mut world = world_at(
            &root,
            registry_config_t::default(),
            mandatory_policy_t::default(),
        );
        world.enroll(&alice);
        world.submit(&alice, &genesis);
        assert!(is_eligible(world.evaluate(
            &runtime(&alice, &editor),
            &files(alice.uid, "home:Secrets", "write")
        )));
    }
    let prohibited = mandatory_policy_t {
        prohibitions: vec![prohibition_t {
            category: resource_category_t::filesystem,
            resource_id: Some("home:Secrets".to_owned()),
            operation: Some("write".to_owned()),
            owner_uid: None,
        }],
        ..mandatory_policy_t::default()
    };
    let mut world = world_at(&root, registry_config_t::default(), prohibited);
    let identity = runtime(&alice, &editor);
    assert_eq!(
        world.evaluate(&identity, &files(alice.uid, "home:Secrets", "write")),
        denied(deny_reason_t::mandatory_policy)
    );
    assert!(is_eligible(
        world.evaluate(&identity, &files(alice.uid, "home:Secrets", "read"))
    ));
    let emergency = mandatory_policy_t {
        emergency_users: [alice.uid].into_iter().collect(),
        ..mandatory_policy_t::default()
    };
    let mut world = world_at(&root, registry_config_t::default(), emergency);
    assert_eq!(
        world.evaluate(&identity, &files(alice.uid, "home:Secrets", "read")),
        denied(deny_reason_t::emergency_restriction)
    );
    let status = world
        .policy
        .policy_status(caller_role_t::trusted_client { uid: alice.uid })
        .expect("status");
    assert!(status.emergency_restriction);
    assert_eq!(status.rollback_protection, rollback_protection_t::degraded);
    assert!(
        !world
            .policy
            .policy_status(Contractd)
            .expect("status")
            .emergency_restriction
    );
    assert!(world.policy.policy_status(caller_role_t::admin).is_err());
}

#[test]
fn a_raised_minimum_policy_epoch_ends_older_grants() {
    let root = temp_root("epoch");
    let alice = test_user(1001);
    let editor = app("editor");
    let old = file_grant(
        "old",
        alice.uid,
        "home:Old",
        &["read"],
        persistent_lifetime(),
    );
    let genesis = draft(&alice, &editor, commit_type_t::genesis, "epoch genesis")
        .add(vec![old])
        .sign(&alice);
    {
        let mut world = world_at(
            &root,
            registry_config_t::default(),
            mandatory_policy_t::default(),
        );
        world.enroll(&alice);
        world.submit(&alice, &genesis);
    }
    let raised = registry_config_t {
        policy_epoch: 2,
        min_valid_policy_epoch: 2,
        ..registry_config_t::default()
    };
    let mut world = world_at(&root, raised, mandatory_policy_t::default());
    let identity = runtime(&alice, &editor);
    assert_eq!(
        world.evaluate(&identity, &files(alice.uid, "home:Old", "read")),
        denied(deny_reason_t::epoch_invalid)
    );
    // A fresh commit at the current epoch re-authorizes the scope with a new grant id.
    let fresh = file_grant(
        "fresh",
        alice.uid,
        "home:Old",
        &["read"],
        persistent_lifetime(),
    );
    let revision = draft(&alice, &editor, commit_type_t::revision, "epoch fresh")
        .on(&genesis)
        .add(vec![fresh.clone()])
        .epoch(2)
        .sign(&alice);
    world.submit(&alice, &revision);
    assert_eq!(
        world.evaluate(&identity, &files(alice.uid, "home:Old", "read")),
        decision_t::eligible {
            grant_id: fresh.grant_id,
            commit_id: revision.commit_id
        }
    );
}

#[test]
fn a_compromised_signing_key_ends_its_grants_and_a_rotated_one_does_not() {
    for (reason, eligible_after) in [
        (invalidation_reason_t::compromised, false),
        (invalidation_reason_t::rotated, true),
        (invalidation_reason_t::retired, true),
    ] {
        let mut world = world(&format!("key_{}", reason.as_str()));
        let alice = test_user(1001);
        world.enroll(&alice);
        let editor = app("editor");
        let genesis = draft(&alice, &editor, commit_type_t::genesis, "key genesis")
            .add(vec![file_grant(
                "key",
                alice.uid,
                "home:Key",
                &["read"],
                persistent_lifetime(),
            )])
            .sign(&alice);
        world.submit(&alice, &genesis);
        let identity = runtime(&alice, &editor);
        let request = files(alice.uid, "home:Key", "read");
        assert!(is_eligible(world.evaluate(&identity, &request)));
        let invalidation = invalidation(
            &alice.identity,
            &alice.persistent.key_id,
            reason,
            reason.as_str(),
        );
        world
            .identity
            .borrow_mut()
            .register_invalidation(Vaultd, alice.uid, &invalidation)
            .expect("invalidation");
        let decision = world.evaluate(&identity, &request);
        if eligible_after {
            assert!(is_eligible(decision), "{reason:?}");
        } else {
            assert_eq!(
                decision,
                denied(deny_reason_t::key_invalidated),
                "{reason:?}"
            );
        }
        // Either way the key signs nothing new.
        let next = draft(
            &alice,
            &editor,
            commit_type_t::revision,
            "after invalidation",
        )
        .on(&genesis)
        .add(vec![file_grant(
            "after",
            alice.uid,
            "home:After",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&alice);
        assert_eq!(
            world
                .contract
                .submit_signed(
                    caller_role_t::trusted_client { uid: alice.uid },
                    &next.envelope
                )
                .err()
                .map(|error| error.to_string()),
            Some("key_authority/key_invalidated".to_owned())
        );
    }
}

#[test]
fn session_grants_need_this_live_session_and_end_with_their_branch() {
    let mut world = world("sessions");
    let alice = test_user(1001);
    world.enroll(&alice);
    let camera_app = app("camera");
    let genesis = draft(
        &alice,
        &camera_app,
        commit_type_t::genesis,
        "session genesis",
    )
    .add(vec![file_grant(
        "base",
        alice.uid,
        "home:Base",
        &["read"],
        persistent_lifetime(),
    )])
    .sign(&alice);
    world.submit(&alice, &genesis);
    let session_grant = file_grant(
        "session",
        alice.uid,
        "home:Pictures",
        &["read"],
        session_lifetime("call"),
    );
    let session_id = session_grant.lifetime.session_id.expect("session");
    let branch = draft(
        &alice,
        &camera_app,
        commit_type_t::temporary_grant,
        "session branch",
    )
    .on(&genesis)
    .add(vec![session_grant.clone()])
    .temporary_authority()
    .sign(&alice);
    assert_eq!(world.submit(&alice, &branch), placement_t::temporary);
    let request = files(alice.uid, "home:Pictures", "read");
    let mut identity = runtime(&alice, &camera_app);
    let now = T1Utc + 60;
    // No attestation, wrong session, stale, ended: denied. Live and recent: eligible.
    identity.session_id = Some(session_id);
    assert_eq!(
        world.evaluate(&identity, &request),
        denied(deny_reason_t::session_not_live)
    );
    world.sessions.attested.borrow_mut().insert(
        session_id,
        session_attestation_t {
            live: true,
            observed_at: now - 5,
        },
    );
    assert_eq!(
        world.evaluate(&identity, &request),
        decision_t::eligible {
            grant_id: session_grant.grant_id,
            commit_id: branch.commit_id
        }
    );
    let mut other_session = identity.clone();
    other_session.session_id = Some([3; 16]);
    assert_eq!(
        world.evaluate(&other_session, &request),
        denied(deny_reason_t::session_not_live)
    );
    world.sessions.attested.borrow_mut().insert(
        session_id,
        session_attestation_t {
            live: true,
            observed_at: now - 6,
        },
    );
    assert_eq!(
        world.evaluate(&identity, &request),
        denied(deny_reason_t::session_not_live)
    );
    world.sessions.attested.borrow_mut().insert(
        session_id,
        session_attestation_t {
            live: true,
            observed_at: now + 1,
        },
    );
    assert_eq!(
        world.evaluate(&identity, &request),
        denied(deny_reason_t::session_not_live)
    );
    world.sessions.attested.borrow_mut().insert(
        session_id,
        session_attestation_t {
            live: false,
            observed_at: now,
        },
    );
    assert_eq!(
        world.evaluate(&identity, &request),
        denied(deny_reason_t::session_not_live)
    );
    world.sessions.attested.borrow_mut().insert(
        session_id,
        session_attestation_t {
            live: true,
            observed_at: now,
        },
    );
    assert!(is_eligible(world.evaluate(&identity, &request)));
    // A trunk commit ends the branch: its grant is no longer current.
    let revision = draft(
        &alice,
        &camera_app,
        commit_type_t::revision,
        "session trunk",
    )
    .on(&genesis)
    .add(vec![file_grant(
        "trunk",
        alice.uid,
        "home:Trunk",
        &["read"],
        persistent_lifetime(),
    )])
    .sign(&alice);
    world.submit(&alice, &revision);
    assert_eq!(
        world.evaluate(&identity, &request),
        denied(deny_reason_t::no_matching_grant)
    );
}

/// Appends one registry-written lifecycle record to a user's journal file, correctly
/// chained (the operation that writes such records is M2-006; replay and the record's
/// effect are M2-005).
fn append_lifecycle_record(root: &Path, uid: u32, record: lifecycle_record_t) {
    let path = root.join("users").join(uid.to_string()).join("journal.c3j");
    let bytes = fs::read(&path).expect("journal");
    let parsed = parse_journal(&bytes, uid).expect("chained journal");
    let count = parsed.records.len() as u64;
    let encoded = registry_record_t {
        sequence: count + 1,
        time: "2026-10-13T09:00:00Z".to_owned(),
        body: record_body_t::lifecycle(record),
    }
    .encode();
    let chain = next_chain(&parsed.chain, count, &encoded);
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open");
    std::io::Write::write_all(&mut file, &frame(&encoded, &chain).expect("frame")).expect("append");
    file.sync_all().expect("sync");
}

#[test]
fn a_durably_ended_session_denies_every_grant_bound_to_it_whatever_the_attestation() {
    // M2-005 review R02 (ADR 0007 §6): once a `session_ended` record is durable, the
    // session never becomes live again. An attestation issued one second before the
    // end is still fresh by rule 5 and must not revive the session's grant; a new grant
    // bound to the same session is refused at admission; and, should the index ever
    // hold a live grant of an ended session without an ending record, the view's
    // ended sessions deny it on their own.
    let mut world = world("ended_session");
    let alice = test_user(1001);
    world.enroll(&alice);
    let browser = app("browser");
    let genesis = draft(
        &alice,
        &browser,
        commit_type_t::genesis,
        "ended session genesis",
    )
    .sign(&alice);
    world.submit(&alice, &genesis);
    let old = file_grant(
        "downloads in the ended session",
        alice.uid,
        "home:Downloads",
        &["read"],
        session_lifetime("ended"),
    );
    let session_id = old.lifetime.session_id.expect("session");
    let branch = draft(
        &alice,
        &browser,
        commit_type_t::temporary_grant,
        "old branch",
    )
    .on(&genesis)
    .add(vec![old.clone()])
    .temporary_authority()
    .sign(&alice);
    assert_eq!(world.submit(&alice, &branch), placement_t::temporary);
    let root = world.root.clone();
    drop(world);
    append_lifecycle_record(
        &root,
        alice.uid,
        lifecycle_record_t::session_ended { session_id },
    );
    let mut world = world_at(
        &root,
        registry_config_t::default(),
        mandatory_policy_t::default(),
    );
    let now = T1Utc + 60;
    let mut identity = runtime(&alice, &browser);
    identity.session_id = Some(session_id);
    // A still-fresh attestation from just before the durable end.
    world.sessions.attested.borrow_mut().insert(
        session_id,
        session_attestation_t {
            live: true,
            observed_at: now - 1,
        },
    );
    let request = files(alice.uid, "home:Downloads", "read");
    assert_eq!(
        world.evaluate(&identity, &request),
        denied(deny_reason_t::grant_ended(
            cham3_registryd::lineage::end_reason_t::session_ended
        ))
    );
    // A fresh grant id bound to the same session: refused at admission, and the
    // evaluation stays denied.
    let fresh = draft(
        &alice,
        &browser,
        commit_type_t::temporary_grant,
        "fresh branch",
    )
    .on(&genesis)
    .add(vec![file_grant(
        "downloads again in the ended session",
        alice.uid,
        "home:Downloads",
        &["read"],
        session_lifetime("ended"),
    )])
    .temporary_authority()
    .sign(&alice);
    assert_eq!(
        world
            .contract
            .submit_signed(
                caller_role_t::trusted_client { uid: alice.uid },
                &fresh.envelope
            )
            .expect_err("refused")
            .to_string(),
        "admission/duplicate_identifier"
    );
    assert!(!is_eligible(world.evaluate(&identity, &request)));
    // The view names the ended session of the live grant. Evaluated as a pure function
    // with the ending record removed from the view, the ended session alone denies;
    // without it the fresh attestation would make the grant eligible.
    let view = world
        .registry
        .lineage_head(
            caller_role_t::service(service_name_t::policyd),
            alice.uid,
            &genesis.payload.lineage_id,
        )
        .expect("view")
        .expect("user");
    assert_eq!(view.ended_sessions, [session_id].into_iter().collect());
    let key_states: BTreeMap<digest_t, key_state_t> = [(
        alice.temporary.key_id,
        key_state_t {
            invalidated: None,
            user_state: user_state_t::normal,
        },
    )]
    .into_iter()
    .collect();
    let policy = mandatory_policy_t::default();
    let reserved = BTreeSet::new();
    let evaluate_view = |view: &cham3_registryd::registry::lineage_view_t| {
        evaluate(
            &snapshot_t {
                view,
                key_states: &key_states,
                policy: &policy,
                now,
                observed_now: 0,
                sessions: &world.sessions,
                reserved: &reserved,
            },
            &identity,
            &request,
        )
    };
    let mut without_record = view.clone();
    without_record.ended.clear();
    assert_eq!(
        evaluate_view(&without_record),
        denied(deny_reason_t::grant_ended(
            cham3_registryd::lineage::end_reason_t::session_ended
        ))
    );
    without_record.ended_sessions.clear();
    assert_eq!(
        evaluate_view(&without_record),
        decision_t::eligible {
            grant_id: old.grant_id,
            commit_id: branch.commit_id
        }
    );
}

#[test]
fn network_mode_follows_the_single_eligible_network_grant() {
    let root = temp_root("network");
    let alice = test_user(1001);
    let browser = app("browser");
    let identity = runtime(&alice, &browser);
    let genesis = draft(&alice, &browser, commit_type_t::genesis, "network genesis")
        .add(vec![file_grant(
            "cache",
            alice.uid,
            "home:Cache",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&alice);
    let tor = network_grant(
        "tor",
        alice.uid,
        "tor",
        timed_lifetime("2026-10-13T10:00:00Z"),
    );
    let tor_branch = draft(
        &alice,
        &browser,
        commit_type_t::temporary_grant,
        "tor branch",
    )
    .on(&genesis)
    .add(vec![tor.clone()])
    .temporary_authority()
    .sign(&alice);
    let direct = network_grant("direct", alice.uid, "direct", persistent_lifetime());
    let direct_revision = draft(&alice, &browser, commit_type_t::revision, "direct trunk")
        .on(&genesis)
        .add(vec![direct.clone()])
        .timestamp("2026-10-13T10:00:00Z")
        .sign(&alice);
    {
        let mut world = world_at(
            &root,
            registry_config_t::default(),
            mandatory_policy_t::default(),
        );
        world.enroll(&alice);
        // No lineage yet, then a lineage without network grant: Offline.
        assert_eq!(
            world
                .policy
                .network_mode(Launcher, &identity)
                .expect("mode"),
            network_decision_t::offline(Some(deny_reason_t::no_matching_grant))
        );
        world.submit(&alice, &genesis);
        assert_eq!(
            world
                .policy
                .network_mode(Launcher, &identity)
                .expect("mode"),
            network_decision_t::offline(None)
        );
        // A temporary Tor grant: Tor until it expires.
        world.submit(&alice, &tor_branch);
        assert_eq!(
            world
                .policy
                .network_mode(Launcher, &identity)
                .expect("mode"),
            network_decision_t::mode {
                mode: network_mode_t::tor,
                grant_id: tor.grant_id
            }
        );
        world.time.pass_until(T1Seconds + 3600);
        assert_eq!(
            world
                .policy
                .network_mode(Launcher, &identity)
                .expect("mode"),
            network_decision_t::offline(None)
        );
        // A trunk commit with a persistent Direct grant ends the branch: Direct.
        world.submit(&alice, &direct_revision);
        assert_eq!(
            world
                .policy
                .network_mode(Launcher, &identity)
                .expect("mode"),
            network_decision_t::mode {
                mode: network_mode_t::direct,
                grant_id: direct.grant_id
            }
        );
        let mut changed = identity.clone();
        changed.executable_sha256 = [1; 32];
        assert_eq!(
            world.policy.network_mode(Launcher, &changed).expect("mode"),
            network_decision_t::offline(Some(deny_reason_t::unknown_identity))
        );
    }
    // Mandatory policy forbidding Direct reports Offline (a Tor-only system).
    let tor_only = mandatory_policy_t {
        forbidden_network_modes: [network_mode_t::direct].into_iter().collect(),
        ..mandatory_policy_t::default()
    };
    let mut world = world_at(&root, registry_config_t::default(), tor_only);
    assert_eq!(
        world
            .policy
            .network_mode(Launcher, &identity)
            .expect("mode"),
        network_decision_t::offline(Some(deny_reason_t::mandatory_policy))
    );
    let network_request = resource_request_t {
        category: resource_category_t::network,
        resource_id: "network_mode".to_owned(),
        resource_owner_uid: alice.uid,
        operation: "direct".to_owned(),
    };
    assert_eq!(
        world.evaluate(&identity, &network_request),
        denied(deny_reason_t::mandatory_policy)
    );
}

#[test]
fn unavailable_inputs_make_the_outcome_unknown() {
    let root = temp_root("unknown");
    let alice = test_user(1001);
    let editor = app("editor");
    let genesis = draft(&alice, &editor, commit_type_t::genesis, "unknown genesis")
        .add(vec![file_grant(
            "u",
            alice.uid,
            "home:U",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&alice);
    {
        let mut world = world_at(
            &root,
            registry_config_t::default(),
            mandatory_policy_t::default(),
        );
        world.enroll(&alice);
        world.submit(&alice, &genesis);
        // Another user identityd does not know: unknown.
        let bob = test_user(1002);
        assert_eq!(
            world.evaluate(&runtime(&bob, &editor), &files(bob.uid, "home:U", "read")),
            decision_t::unknown(unknown_reason_t::identity_unavailable)
        );
    }
    // A damaged journal: identityd and registryd both refuse; never eligible.
    let path = root.join("users").join("1001").join("journal.c3j");
    let mut bytes = fs::read(&path).expect("read");
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    fs::write(&path, bytes).expect("write");
    let mut world = world_at(
        &root,
        registry_config_t::default(),
        mandatory_policy_t::default(),
    );
    let decision = world.evaluate(
        &runtime(&alice, &editor),
        &files(alice.uid, "home:U", "read"),
    );
    assert!(matches!(decision, decision_t::unknown(_)), "{decision:?}");
    assert!(matches!(
        world
            .policy
            .network_mode(Launcher, &runtime(&alice, &editor))
            .expect("mode"),
        network_decision_t::offline_unknown(_)
    ));
}

/// The pure functions over hand-built snapshots: cases admission makes unreachable.
#[test]
fn conservative_evaluation_of_inconsistent_snapshots() {
    let mut world = world("pure");
    let alice = test_user(1001);
    world.enroll(&alice);
    let browser = app("browser");
    let genesis = draft(&alice, &browser, commit_type_t::genesis, "pure genesis")
        .add(vec![network_grant(
            "pure direct",
            alice.uid,
            "direct",
            persistent_lifetime(),
        )])
        .sign(&alice);
    world.submit(&alice, &genesis);
    let lineage_id = genesis.payload.lineage_id;
    let view = world
        .registry
        .lineage_head(
            caller_role_t::service(service_name_t::policyd),
            alice.uid,
            &lineage_id,
        )
        .expect("view")
        .expect("user");
    let key_states: BTreeMap<digest_t, key_state_t> = [(
        alice.persistent.key_id,
        key_state_t {
            invalidated: None,
            user_state: user_state_t::normal,
        },
    )]
    .into_iter()
    .collect();
    let policy = mandatory_policy_t::default();
    let reserved = BTreeSet::new();
    let sessions = sessions_t::default();
    let identity = runtime(&alice, &browser);
    let snapshot = |view: &cham3_registryd::registry::lineage_view_t,
                    key_states: &BTreeMap<digest_t, key_state_t>,
                    reserved: &BTreeSet<random_id_t>|
     -> (network_decision_t, decision_t) {
        let snapshot = snapshot_t {
            view,
            key_states,
            policy: &policy,
            now: T1Utc + 60,
            observed_now: 0,
            sessions: &sessions,
            reserved,
        };
        let request = resource_request_t {
            category: resource_category_t::network,
            resource_id: "network_mode".to_owned(),
            resource_owner_uid: alice.uid,
            operation: "direct".to_owned(),
        };
        (
            network_mode(&snapshot, &identity),
            evaluate(&snapshot, &identity, &request),
        )
    };
    let (mode, decision) = snapshot(&view, &key_states, &reserved);
    assert!(matches!(
        mode,
        network_decision_t::mode {
            mode: network_mode_t::direct,
            ..
        }
    ));
    assert!(is_eligible(decision));
    // Two eligible network grants (corruption): Offline, never by iteration order.
    let mut two = view.clone();
    let mut second = network_grant("pure second", alice.uid, "tor", persistent_lifetime());
    second.grant_id = [0xff; 16];
    let trunk = two.trunk_head.as_mut().expect("trunk");
    trunk.state.grants.push(second.clone());
    let origin = two.origins.values().next().expect("origin").clone();
    two.origins.insert(second.grant_id, origin);
    assert_eq!(
        snapshot(&two, &key_states, &reserved).0,
        network_decision_t::offline_unknown(unknown_reason_t::network_grant_conflict)
    );
    // A grant without its origin, or a key identityd did not answer for: unknown.
    let mut orphan = view.clone();
    orphan.origins.clear();
    assert_eq!(
        snapshot(&orphan, &key_states, &reserved).1,
        decision_t::unknown(unknown_reason_t::snapshot_inconsistent)
    );
    assert_eq!(
        snapshot(&view, &BTreeMap::new(), &reserved).1,
        decision_t::unknown(unknown_reason_t::identity_unavailable)
    );
    // identityd's answer denies a compromise or a user state the snapshot lacks.
    let compromised: BTreeMap<digest_t, key_state_t> = [(
        alice.persistent.key_id,
        key_state_t {
            invalidated: Some(invalidation_reason_t::compromised),
            user_state: user_state_t::normal,
        },
    )]
    .into_iter()
    .collect();
    assert_eq!(
        snapshot(&view, &compromised, &reserved).1,
        denied(deny_reason_t::key_invalidated)
    );
    let contained: BTreeMap<digest_t, key_state_t> = [(
        alice.persistent.key_id,
        key_state_t {
            invalidated: None,
            user_state: user_state_t::contained,
        },
    )]
    .into_iter()
    .collect();
    assert_eq!(
        snapshot(&view, &contained, &reserved).1,
        denied(deny_reason_t::user_state(user_state_t::contained))
    );
    // A one-shot grant with a reservation is consumed.
    let mut one_shot = view.clone();
    let mut grant = file_grant(
        "pure one shot",
        alice.uid,
        "home:Once",
        &["read"],
        one_shot_lifetime(),
    );
    grant.grant_id = [0x01; 16];
    let head = one_shot.trunk_head.clone().expect("trunk");
    one_shot
        .temporary_heads
        .push(cham3_registryd::registry::head_view_t {
            commit_id: [0x02; 32],
            placement: placement_t::temporary,
            state: cham3_contract_format::schema::contract_state_t {
                grants: vec![grant.clone()],
            },
        });
    let origin = one_shot.origins.values().next().expect("origin").clone();
    one_shot.origins.insert(grant.grant_id, origin);
    let request = files(alice.uid, "home:Once", "read");
    let evaluate_once = |reserved: &BTreeSet<random_id_t>| {
        evaluate(
            &snapshot_t {
                view: &one_shot,
                key_states: &key_states,
                policy: &policy,
                now: T1Utc + 60,
                observed_now: 0,
                sessions: &sessions,
                reserved,
            },
            &identity,
            &request,
        )
    };
    assert!(is_eligible(evaluate_once(&BTreeSet::new())));
    assert_eq!(
        evaluate_once(&[grant.grant_id].into_iter().collect()),
        denied(deny_reason_t::consumed)
    );
    let _ = head;
    // A delegated grant is refused (no delegation admission before M2-006).
    let mut delegated = view.clone();
    let trunk = delegated.trunk_head.as_mut().expect("trunk");
    trunk.state.grants[0].delegation.parent_grant_id = Some([5; 16]);
    assert_eq!(
        snapshot(&delegated, &key_states, &reserved).1,
        denied(deny_reason_t::delegation_unsupported)
    );
    let _ = (&world.root, object_kind_t::commit);
}

#[test]
fn an_emergency_restriction_set_through_registryd_denies_everyone_and_persists() {
    // M2-006 stage A: `set_emergency_restriction` is a restrictive change of registryd
    // (ADR 0007 §1 rule 1; ADR 0006 §3), in force at once for every user and persisted
    // in the anchor; releasing it restores no grant by itself but ends the denial.
    let root = temp_root("emergency_persisted");
    let alice = test_user(1001);
    let editor = app("editor");
    let identity = runtime(&alice, &editor);
    let genesis = draft(&alice, &editor, commit_type_t::genesis, "emergency genesis")
        .add(vec![file_grant(
            "secrets",
            alice.uid,
            "home:Secrets",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&alice);
    let request = files(alice.uid, "home:Secrets", "read");
    let mut world = world_at(
        &root,
        registry_config_t::default(),
        mandatory_policy_t::default(),
    );
    world.enroll(&alice);
    world.submit(&alice, &genesis);
    assert!(is_eligible(world.evaluate(&identity, &request)));
    assert_eq!(
        world
            .policy
            .set_emergency_restriction(Contractd, true)
            .unwrap_err()
            .to_string(),
        "ipc/role_denied"
    );
    let status = world
        .policy
        .set_emergency_restriction(caller_role_t::admin, true)
        .expect("set");
    assert!(status.emergency_restriction);
    assert_eq!(status.rollback_protection, rollback_protection_t::degraded);
    assert_eq!(
        world.evaluate(&identity, &request),
        denied(deny_reason_t::emergency_restriction)
    );
    // The persisted restriction shows for every caller, overlay or not.
    assert!(
        world
            .policy
            .policy_status(Contractd)
            .unwrap()
            .emergency_restriction
    );
    drop(world);
    let mut world = world_at(
        &root,
        registry_config_t::default(),
        mandatory_policy_t::default(),
    );
    assert_eq!(
        world.evaluate(&identity, &request),
        denied(deny_reason_t::emergency_restriction),
        "persisted across the restart"
    );
    let status = world
        .policy
        .set_emergency_restriction(caller_role_t::admin, false)
        .expect("released");
    assert!(!status.emergency_restriction);
    assert!(is_eligible(world.evaluate(&identity, &request)));
}

#[test]
fn a_policy_epoch_raised_through_registryd_ends_older_grants_and_persists() {
    // M2-006 stage A: `set_policy_epoch` raises the epoch and the minimum valid epoch
    // through registryd's restrictive path (ADR 0007 §1 rule 4); neither is lowered by
    // a later request or by a lower configuration at the next start.
    let root = temp_root("epoch_raised");
    let alice = test_user(1001);
    let editor = app("editor");
    let identity = runtime(&alice, &editor);
    let genesis = draft(
        &alice,
        &editor,
        commit_type_t::genesis,
        "epoch raised genesis",
    )
    .add(vec![file_grant(
        "old",
        alice.uid,
        "home:Old",
        &["read"],
        persistent_lifetime(),
    )])
    .sign(&alice);
    let request = files(alice.uid, "home:Old", "read");
    let mut world = world_at(
        &root,
        registry_config_t::default(),
        mandatory_policy_t::default(),
    );
    world.enroll(&alice);
    world.submit(&alice, &genesis);
    assert!(is_eligible(world.evaluate(&identity, &request)));
    assert_eq!(
        world
            .policy
            .set_policy_epoch(Contractd, 2, 2)
            .unwrap_err()
            .to_string(),
        "ipc/role_denied"
    );
    let status = world
        .policy
        .set_policy_epoch(caller_role_t::admin, 2, 2)
        .expect("raised");
    assert_eq!((status.policy_epoch, status.min_valid_policy_epoch), (2, 2));
    assert_eq!(
        world.evaluate(&identity, &request),
        denied(deny_reason_t::epoch_invalid)
    );
    let status = world
        .policy
        .set_policy_epoch(caller_role_t::admin, 1, 1)
        .expect("accepted, not lowered");
    assert_eq!((status.policy_epoch, status.min_valid_policy_epoch), (2, 2));
    let fresh = file_grant(
        "fresh",
        alice.uid,
        "home:Old",
        &["read"],
        persistent_lifetime(),
    );
    let revision = draft(
        &alice,
        &editor,
        commit_type_t::revision,
        "epoch raised fresh",
    )
    .on(&genesis)
    .add(vec![fresh.clone()])
    .epoch(2)
    .sign(&alice);
    world.submit(&alice, &revision);
    assert_eq!(
        world.evaluate(&identity, &request),
        decision_t::eligible {
            grant_id: fresh.grant_id,
            commit_id: revision.commit_id
        }
    );
    drop(world);
    let mut world = world_at(
        &root,
        registry_config_t::default(),
        mandatory_policy_t::default(),
    );
    let status = world.policy.policy_status(Contractd).expect("status");
    assert_eq!((status.policy_epoch, status.min_valid_policy_epoch), (2, 2));
    assert!(is_eligible(world.evaluate(&identity, &request)));
}

#[test]
fn a_contained_user_is_denied_until_recovered() {
    // M2-006 stage A: after an unverifiable run (a restart in another boot without an
    // orderly shutdown, ADR 0006 §3) every user is contained: evaluation denies with
    // the user state, and no new commit is admitted until the recovery ceremony
    // (stage C).
    let root = temp_root("contained_user");
    let alice = test_user(1001);
    let editor = app("editor");
    let identity = runtime(&alice, &editor);
    let genesis = draft(&alice, &editor, commit_type_t::genesis, "contained genesis")
        .add(vec![file_grant(
            "docs",
            alice.uid,
            "home:Docs",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&alice);
    let revision = draft(
        &alice,
        &editor,
        commit_type_t::revision,
        "contained revision",
    )
    .on(&genesis)
    .add(vec![file_grant(
        "more",
        alice.uid,
        "home:More",
        &["read"],
        persistent_lifetime(),
    )])
    .sign(&alice);
    let request = files(alice.uid, "home:Docs", "read");
    let booted = |boot: &str| registry_config_t {
        boot_id: Some(boot.to_owned()),
        ..registry_config_t::default()
    };
    let mut world = world_at(&root, booted("boot-1"), mandatory_policy_t::default());
    world.enroll(&alice);
    world.submit(&alice, &genesis);
    assert!(is_eligible(world.evaluate(&identity, &request)));
    drop(world);
    let mut world = world_at(&root, booted("boot-2"), mandatory_policy_t::default());
    assert_eq!(
        world.evaluate(&identity, &request),
        denied(deny_reason_t::user_state(user_state_t::contained))
    );
    assert_eq!(
        world
            .contract
            .submit_signed(
                caller_role_t::trusted_client { uid: alice.uid },
                &revision.envelope,
            )
            .unwrap_err()
            .to_string(),
        "key_authority/contained"
    );
}

#[test]
fn an_admission_whose_anchor_update_failed_is_not_eligible_until_it_is_anchored() {
    // M2-006-A-R01 (ADR 0006 §§2, 4): an admission reported `storage_unavailable`
    // because its anchor update failed is a durable but unanchored record; policyd
    // evaluating that lineage gets no snapshot (`unknown`, denied) until the record is
    // anchored — here by the retry the next read makes once the anchor can be written.
    // The first refusal comes from identityd, whose index refresh reads the same
    // user's identity records; registryd's `lineage_head` refuses the same way.
    let root = temp_root("unanchored_admission");
    let armed = Arc::new(AtomicBool::new(false));
    let mut world = world_at_with(
        &root,
        registry_config_t::default(),
        mandatory_policy_t::default(),
        Box::new(anchor_fault_io_t {
            armed: Arc::clone(&armed),
        }),
    );
    let alice = test_user(1001);
    let editor = app("editor");
    let identity = runtime(&alice, &editor);
    let grant = file_grant(
        "unanchored",
        alice.uid,
        "home:Unanchored",
        &["read"],
        persistent_lifetime(),
    );
    let genesis = draft(
        &alice,
        &editor,
        commit_type_t::genesis,
        "unanchored genesis",
    )
    .add(vec![grant.clone()])
    .sign(&alice);
    let request = files(alice.uid, "home:Unanchored", "read");
    world.enroll(&alice);
    assert!(!is_eligible(world.evaluate(&identity, &request)));
    let anchored_before = world.registry.anchored_user(alice.uid).unwrap();
    armed.store(true, Ordering::SeqCst);
    assert_eq!(
        world
            .contract
            .submit_signed(
                caller_role_t::trusted_client { uid: alice.uid },
                &genesis.envelope
            )
            .unwrap_err()
            .to_string(),
        "ipc/storage_unavailable"
    );
    assert_eq!(
        world.registry.anchored_user(alice.uid).unwrap(),
        anchored_before
    );
    assert_eq!(
        world.evaluate(&identity, &request),
        decision_t::unknown(unknown_reason_t::identity_unavailable),
        "nothing of the unanchored state is served"
    );
    assert_eq!(
        world.policy.network_mode(Launcher, &identity).unwrap(),
        network_decision_t::offline_unknown(unknown_reason_t::identity_unavailable)
    );
    assert_eq!(
        world
            .registry
            .lineage_head(Contractd, alice.uid, &genesis.payload.lineage_id)
            .unwrap_err()
            .to_string(),
        "ipc/storage_unavailable"
    );
    assert_eq!(
        world
            .registry
            .rollback_status(Contractd)
            .unwrap()
            .pending_restrictions,
        vec![alice.uid]
    );
    // The anchor can be written again: the next read anchors the record and serves it.
    armed.store(false, Ordering::SeqCst);
    assert_eq!(
        world.evaluate(&identity, &request),
        decision_t::eligible {
            grant_id: grant.grant_id,
            commit_id: genesis.commit_id,
        }
    );
    assert_eq!(
        world
            .registry
            .anchored_user(alice.uid)
            .unwrap()
            .map(|entry| entry.seq),
        Some(4),
        "the enrollment, two certificates and the admission"
    );
    assert!(
        world
            .registry
            .rollback_status(Contractd)
            .unwrap()
            .pending_restrictions
            .is_empty()
    );
}

#[test]
fn a_failed_emergency_release_keeps_everyone_denied() {
    // M2-006-A-R05 (ADR 0007 §1 rule 1): a release of the persisted emergency
    // restriction whose anchor update fails is reported `restriction_not_durable` and
    // changes nothing: every evaluation stays denied until a release is durable.
    let root = temp_root("failed_release");
    let armed = Arc::new(AtomicBool::new(false));
    let mut world = world_at_with(
        &root,
        registry_config_t::default(),
        mandatory_policy_t::default(),
        Box::new(anchor_fault_io_t {
            armed: Arc::clone(&armed),
        }),
    );
    let alice = test_user(1001);
    let editor = app("editor");
    let identity = runtime(&alice, &editor);
    let genesis = draft(&alice, &editor, commit_type_t::genesis, "release genesis")
        .add(vec![file_grant(
            "release",
            alice.uid,
            "home:Release",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&alice);
    let request = files(alice.uid, "home:Release", "read");
    world.enroll(&alice);
    world.submit(&alice, &genesis);
    assert!(is_eligible(world.evaluate(&identity, &request)));
    world
        .policy
        .set_emergency_restriction(caller_role_t::admin, true)
        .expect("set");
    assert_eq!(
        world.evaluate(&identity, &request),
        denied(deny_reason_t::emergency_restriction)
    );
    let counter = world.registry.anchor_counter().unwrap();
    armed.store(true, Ordering::SeqCst);
    assert_eq!(
        world
            .policy
            .set_emergency_restriction(caller_role_t::admin, false)
            .unwrap_err()
            .to_string(),
        "ipc/restriction_not_durable"
    );
    assert_eq!(world.registry.anchor_counter().unwrap(), counter);
    assert_eq!(
        world.evaluate(&identity, &request),
        denied(deny_reason_t::emergency_restriction),
        "a release that is not durable is not in force"
    );
    assert!(
        world
            .policy
            .policy_status(Contractd)
            .unwrap()
            .emergency_restriction
    );
    armed.store(false, Ordering::SeqCst);
    let status = world
        .policy
        .set_emergency_restriction(caller_role_t::admin, false)
        .expect("released");
    assert!(!status.emergency_restriction);
    assert!(is_eligible(world.evaluate(&identity, &request)));
}

// ------------------------------------------------ trusted time (M2-006 stage B1)

/// A policyd over the world's registry and identityd with its own clocks and boot id.
fn policy_with_clock(
    world: &world_t,
    time: &test_time_t,
    boot_id: &str,
) -> policy_service_t<cham3_policyd::service::in_process_identity_t<registry_identity_store_t>> {
    policy_service_t::new(
        Arc::clone(&world.registry),
        cham3_policyd::service::in_process_identity_t {
            service: Rc::clone(&world.identity),
        },
        mandatory_policy_t::default(),
        policy_clock_t::new(Box::new(time.clone()), boot_id),
        Box::new(world.sessions.clone()),
    )
}

fn kernel_boot_id() -> String {
    kernel_boot_id_t.boot_id().expect("boot id")
}

#[test]
fn a_forward_clock_error_after_admission_does_not_advance_a_future_start() {
    // ADR 0007 §8 option B: the grant starts an hour after T1 and is admitted at T1 +
    // 60, so 3 540 s of observed time must pass, whatever the wall clock says.
    let mut world = world("observed_start");
    let alice = test_user(1001);
    world.enroll(&alice);
    let editor = app("editor");
    let mut later = persistent_lifetime();
    later.effective_start = "2026-10-13T10:00:00Z".to_owned();
    let grant = file_grant("later", alice.uid, "home:Later", &["read"], later);
    let genesis = draft(&alice, &editor, commit_type_t::genesis, "observed start")
        .add(vec![grant.clone()])
        .sign(&alice);
    world.submit(&alice, &genesis);
    let identity = runtime(&alice, &editor);
    let request = files(alice.uid, "home:Later", "read");
    assert_eq!(
        world.evaluate(&identity, &request),
        denied(deny_reason_t::not_started)
    );
    // The wall clock jumps an hour forward: trusted time accepts it, but no observed
    // time has passed since the admission.
    world.time.step_wall_clock(T1Seconds + 3600);
    assert_eq!(
        world
            .registry
            .rollback_status(Policyd)
            .expect("status")
            .clock
            .trusted_now,
        T1Seconds + 3600
    );
    assert_eq!(
        world.evaluate(&identity, &request),
        denied(deny_reason_t::not_started)
    );
    world.time.advance(3539);
    assert_eq!(
        world.evaluate(&identity, &request),
        denied(deny_reason_t::not_started)
    );
    world.time.advance(1);
    assert!(is_eligible(world.evaluate(&identity, &request)));
}

#[test]
fn policyd_reports_a_forward_sample_and_relies_on_it_only_after_registryd_took_it() {
    let mut world = world("time_sample_path");
    let alice = test_user(1001);
    world.enroll(&alice);
    let editor = app("editor");
    let genesis = draft(&alice, &editor, commit_type_t::genesis, "sample path")
        .add(vec![file_grant(
            "files",
            alice.uid,
            "home:Files",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&alice);
    world.submit(&alice, &genesis);
    let identity = runtime(&alice, &editor);
    let request = files(alice.uid, "home:Files", "read");
    let base = world.time.current().expect("clock");
    let shared = (T1Seconds + 60) as i64 - base.boottime as i64;
    // policyd's wall clock reads 5 s ahead of registryd's: it reports the sample, and
    // registryd ledgers and adopts it before policyd relies on it.
    let ahead = test_time_t::at(base.realtime + 5);
    ahead.set_available(true, base.realtime + 5, base.boottime);
    let mut policy = policy_with_clock(&world, &ahead, &kernel_boot_id());
    assert!(is_eligible(
        policy
            .evaluate(Launcher, &identity, &request)
            .expect("evaluate")
    ));
    assert_eq!(policy.clock().offset(), Some(shared + 5));
    assert_eq!(
        world
            .registry
            .rollback_status(Policyd)
            .expect("status")
            .clock
            .trusted_now,
        T1Seconds + 65
    );
    // A sample registryd refuses (its CLOCK_BOOTTIME is far ahead of registryd's own)
    // is not relied on, and nothing is decided from the snapshot read before it: the
    // services disagree on the clock (M2-006-B1-R01).
    let refused = test_time_t::at(base.realtime + 500);
    refused.set_available(true, base.realtime + 500, base.boottime + 100);
    let mut policy = policy_with_clock(&world, &refused, &kernel_boot_id());
    assert_eq!(
        policy
            .evaluate(Launcher, &identity, &request)
            .expect("evaluate"),
        decision_t::unknown(unknown_reason_t::clock_unavailable)
    );
    assert_eq!(policy.clock().offset(), None, "nothing adopted");
    assert_eq!(
        world
            .registry
            .rollback_status(Policyd)
            .expect("status")
            .clock
            .trusted_now,
        T1Seconds + 65
    );
}

#[test]
fn a_snapshot_of_another_boot_is_unknown() {
    let mut world = world("other_boot");
    let alice = test_user(1001);
    world.enroll(&alice);
    let editor = app("editor");
    let genesis = draft(&alice, &editor, commit_type_t::genesis, "other boot")
        .add(vec![file_grant(
            "files",
            alice.uid,
            "home:Files",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&alice);
    world.submit(&alice, &genesis);
    let mut policy = policy_with_clock(&world, &world.time, "a-boot-that-is-not-this-one");
    assert_eq!(
        policy
            .evaluate(
                Launcher,
                &runtime(&alice, &editor),
                &files(alice.uid, "home:Files", "read")
            )
            .expect("evaluate"),
        decision_t::unknown(unknown_reason_t::clock_unavailable)
    );
}

/// Ledger appends fail while armed (the time entry of a forward sample included).
#[derive(Clone)]
struct ledger_fault_io_t {
    armed: Arc<AtomicBool>,
}

impl journal_io_t for ledger_fault_io_t {
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
        if self.armed.load(Ordering::SeqCst) {
            return Err(io::Error::other("injected ledger append failure"));
        }
        system_io_t.append_file(path, bytes)
    }
}

/// A world whose ledger can be made to fail, with one granted lineage, and a policyd
/// whose wall clock reads 140 s ahead of registryd's at the same CLOCK_BOOTTIME.
fn ahead_of_a_failing_ledger(
    label: &str,
    grant: cham3_contract_format::schema::grant_t,
) -> (
    world_t,
    Arc<AtomicBool>,
    policy_service_t<cham3_policyd::service::in_process_identity_t<registry_identity_store_t>>,
    test_user_t,
    app_t,
) {
    let armed = Arc::new(AtomicBool::new(false));
    let mut world = world_at_with(
        &temp_root(label),
        registry_config_t::default(),
        mandatory_policy_t::default(),
        Box::new(ledger_fault_io_t {
            armed: Arc::clone(&armed),
        }),
    );
    let alice = test_user(1001);
    world.enroll(&alice);
    let editor = app("editor");
    let genesis = draft(&alice, &editor, commit_type_t::genesis, label)
        .add(vec![grant])
        .sign(&alice);
    world.submit(&alice, &genesis);
    let base = world.time.current().expect("clock");
    let ahead = test_time_t::at(base.realtime + 140);
    ahead.set_available(true, base.realtime + 140, base.boottime);
    let policy = policy_with_clock(&world, &ahead, &kernel_boot_id());
    (world, armed, policy, alice, editor)
}

#[test]
fn a_time_sample_whose_ledger_append_fails_denies_the_current_evaluation() {
    // M2-006-B1-R01: the grant expires 40 s after registryd's trusted now and 100 s
    // before policyd's sample; the sample's ledger append fails.
    let mut expiring = persistent_lifetime();
    expiring.expiration = Some("2026-10-13T09:01:40Z".to_owned());
    let grant = file_grant("expiring", 1001, "home:Expiring", &["read"], expiring);
    let (world, armed, mut policy, alice, editor) =
        ahead_of_a_failing_ledger("sample_failure_evaluate", grant);
    let identity = runtime(&alice, &editor);
    let request = files(alice.uid, "home:Expiring", "read");
    armed.store(true, Ordering::SeqCst);
    assert_eq!(
        policy
            .evaluate(Launcher, &identity, &request)
            .expect("evaluate"),
        decision_t::unknown(unknown_reason_t::registry_unavailable),
        "nothing is decided from the snapshot read before the failure"
    );
    assert!(
        world
            .registry
            .rollback_status(Policyd)
            .expect("status")
            .ledger_failed
    );
    assert_eq!(
        policy.clock().offset(),
        None,
        "the refused raise is not adopted"
    );
    assert_eq!(
        policy
            .evaluate(Launcher, &identity, &request)
            .expect("evaluate"),
        decision_t::unknown(unknown_reason_t::registry_unavailable)
    );
}

#[test]
fn a_time_sample_whose_ledger_append_fails_leaves_the_network_offline() {
    for mode in ["direct", "tor"] {
        let grant = network_grant(mode, 1001, mode, persistent_lifetime());
        let (_world, armed, mut policy, alice, editor) =
            ahead_of_a_failing_ledger(&format!("sample_failure_network_{mode}"), grant);
        let identity = runtime(&alice, &editor);
        armed.store(true, Ordering::SeqCst);
        assert_eq!(
            policy.network_mode(Launcher, &identity).expect("mode"),
            network_decision_t::offline_unknown(unknown_reason_t::registry_unavailable),
            "{mode}"
        );
    }
}

#[test]
fn a_forward_sample_registryd_takes_expires_the_grant_at_once() {
    // The no-fault control of the two tests above: the same sample, ledgered.
    let mut expiring = persistent_lifetime();
    expiring.expiration = Some("2026-10-13T09:01:40Z".to_owned());
    let grant = file_grant("expiring", 1001, "home:Expiring", &["read"], expiring);
    let (world, _armed, mut policy, alice, editor) =
        ahead_of_a_failing_ledger("sample_success_expires", grant);
    assert_eq!(
        policy
            .evaluate(
                Launcher,
                &runtime(&alice, &editor),
                &files(alice.uid, "home:Expiring", "read")
            )
            .expect("evaluate"),
        denied(deny_reason_t::expired)
    );
    assert_eq!(
        world
            .registry
            .rollback_status(Policyd)
            .expect("status")
            .clock
            .trusted_now,
        T1Seconds + 200
    );
}

fn journal_bodies(root: &Path, uid: u32) -> Vec<record_body_t> {
    let bytes =
        fs::read(root.join("users").join(uid.to_string()).join("journal.c3j")).expect("journal");
    parse_journal(&bytes, uid)
        .expect("chained journal")
        .records
        .iter()
        .map(|record| registry_record_t::decode(record).expect("decodes").body)
        .collect()
}

#[test]
fn evaluation_latches_every_observed_expiry_once() {
    // ADR 0007 §5 rule 5 (M2-006 stage B2): an evaluation or network-mode query whose
    // trusted now shows a current grant expired records `expired` for it through
    // registryd's restrictive path, once; the decision does not depend on the write.
    let mut world = world("latch");
    let alice = test_user(1001);
    world.enroll(&alice);
    let browser = app("browser");
    let identity = runtime(&alice, &browser);
    let genesis = draft(&alice, &browser, commit_type_t::genesis, "latch genesis")
        .add(vec![file_grant(
            "latch cache",
            alice.uid,
            "home:Cache",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&alice);
    world.submit(&alice, &genesis);
    let timed = file_grant(
        "latched",
        alice.uid,
        "home:Timed",
        &["read"],
        timed_lifetime("2026-10-13T09:10:00Z"),
    );
    let tor = network_grant(
        "tor latched",
        alice.uid,
        "tor",
        timed_lifetime("2026-10-13T09:10:00Z"),
    );
    let branch = draft(&alice, &browser, commit_type_t::temporary_grant, "latch")
        .on(&genesis)
        .add(vec![timed.clone(), tor.clone()])
        .temporary_authority()
        .sign(&alice);
    assert_eq!(world.submit(&alice, &branch), placement_t::temporary);
    let request = files(alice.uid, "home:Timed", "read");
    assert!(is_eligible(world.evaluate(&identity, &request)));
    let before = journal_bodies(&world.root, alice.uid);
    // Ten minutes pass: both grants are expired.
    world.time.advance(600);
    assert_eq!(
        world.evaluate(&identity, &request),
        denied(deny_reason_t::expired)
    );
    let latched: Vec<record_body_t> = journal_bodies(&world.root, alice.uid)
        .into_iter()
        .skip(before.len())
        .collect();
    let mut expected = vec![
        record_body_t::lifecycle(lifecycle_record_t::expired {
            grant_id: timed.grant_id,
        }),
        record_body_t::lifecycle(lifecycle_record_t::expired {
            grant_id: tor.grant_id,
        }),
    ];
    expected.sort_by_key(|body| format!("{body:?}"));
    let mut sorted = latched.clone();
    sorted.sort_by_key(|body| format!("{body:?}"));
    assert_eq!(sorted, expected, "every expired current grant, once");
    // Latched: the next evaluation and the network mode see ended grants and write
    // nothing more.
    assert_eq!(
        world.evaluate(&identity, &request),
        denied(deny_reason_t::grant_ended(
            cham3_registryd::lineage::end_reason_t::expired
        ))
    );
    assert_eq!(
        world
            .policy
            .network_mode(Launcher, &identity)
            .expect("network mode"),
        network_decision_t::offline(None)
    );
    assert_eq!(
        journal_bodies(&world.root, alice.uid).len(),
        before.len() + 2
    );
    for grant in [&timed, &tor] {
        assert_eq!(
            world.registry.standing(alice.uid, &grant.grant_id),
            Ok(cham3_registryd::lineage::standing_t::ended(
                cham3_registryd::lineage::end_reason_t::expired
            ))
        );
    }
}

#[test]
fn network_mode_latches_an_expired_network_grant() {
    let mut world = world("latch_network");
    let alice = test_user(1001);
    world.enroll(&alice);
    let browser = app("browser");
    let identity = runtime(&alice, &browser);
    let genesis = draft(&alice, &browser, commit_type_t::genesis, "network latch")
        .add(vec![file_grant(
            "network latch cache",
            alice.uid,
            "home:Cache",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&alice);
    world.submit(&alice, &genesis);
    let tor = network_grant(
        "tor only",
        alice.uid,
        "tor",
        timed_lifetime("2026-10-13T09:10:00Z"),
    );
    let branch = draft(&alice, &browser, commit_type_t::temporary_grant, "tor only")
        .on(&genesis)
        .add(vec![tor.clone()])
        .temporary_authority()
        .sign(&alice);
    world.submit(&alice, &branch);
    assert!(matches!(
        world
            .policy
            .network_mode(Launcher, &identity)
            .expect("mode"),
        network_decision_t::mode { .. }
    ));
    world.time.advance(600);
    assert_eq!(
        world
            .policy
            .network_mode(Launcher, &identity)
            .expect("mode"),
        network_decision_t::offline(None)
    );
    assert_eq!(
        journal_bodies(&world.root, alice.uid).last(),
        Some(&record_body_t::lifecycle(lifecycle_record_t::expired {
            grant_id: tor.grant_id
        }))
    );
}

#[test]
fn an_identity_mismatch_is_reported_once_per_executable_identity() {
    // ADR 0007 erratum E7.4 (ADR 0008 §7): an updated package or executable is denied
    // `unknown_identity` and reported once per lineage, package identity and
    // executable hash; another runtime scope is denied without a record.
    let mut world = world("mismatch");
    let alice = test_user(1001);
    world.enroll(&alice);
    let editor = app("editor");
    let genesis = draft(&alice, &editor, commit_type_t::genesis, "mismatch genesis")
        .add(vec![file_grant(
            "mismatch docs",
            alice.uid,
            "home:Docs",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&alice);
    world.submit(&alice, &genesis);
    let request = files(alice.uid, "home:Docs", "read");
    let identity = runtime(&alice, &editor);
    let before = journal_bodies(&world.root, alice.uid).len();
    assert!(is_eligible(world.evaluate(&identity, &request)));
    let mut scoped = identity.clone();
    scoped.runtime_scope = Some("sandbox".to_owned());
    assert_eq!(
        world.evaluate(&scoped, &request),
        denied(deny_reason_t::unknown_identity)
    );
    assert_eq!(journal_bodies(&world.root, alice.uid).len(), before);
    let mut updated = identity.clone();
    updated.executable_sha256 = [9; 32];
    for _ in 0..3 {
        assert_eq!(
            world.evaluate(&updated, &request),
            denied(deny_reason_t::unknown_identity)
        );
        let _ = world.policy.network_mode(Launcher, &updated);
    }
    let bodies = journal_bodies(&world.root, alice.uid);
    assert_eq!(bodies.len(), before + 1, "once");
    assert_eq!(
        bodies.last(),
        Some(&record_body_t::lifecycle(
            lifecycle_record_t::identity_mismatch {
                lineage_id: genesis.payload.lineage_id,
                package_identity: editor.package_identity.clone(),
                executable_sha256: [9; 32],
            }
        ))
    );
    let mut repackaged = identity.clone();
    repackaged.package_identity = "deb:editor-fork:amd64".to_owned();
    world.evaluate(&repackaged, &request);
    assert_eq!(journal_bodies(&world.root, alice.uid).len(), before + 2);
    assert!(is_eligible(world.evaluate(&identity, &request)));
}

#[test]
fn a_package_identity_no_record_can_name_leaves_the_journal_replayable() {
    // M2-006-B2-02: a runtime package identity that is empty, longer than 255 bytes
    // or not printable ASCII is denied `unknown_identity`; registryd refuses the
    // mismatch report (`malformed_request`), so nothing is journaled and a restart
    // quarantines nobody. The longest valid package identity is reported once (the
    // positive control).
    let mut world = world("mismatch_malformed");
    let alice = test_user(1001);
    world.enroll(&alice);
    let editor = app("editor");
    let genesis = draft(&alice, &editor, commit_type_t::genesis, "malformed genesis")
        .add(vec![file_grant(
            "malformed docs",
            alice.uid,
            "home:Docs",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&alice);
    world.submit(&alice, &genesis);
    let request = files(alice.uid, "home:Docs", "read");
    let identity = runtime(&alice, &editor);
    let before = journal_bodies(&world.root, alice.uid);
    for package_identity in [
        String::new(),
        "p".repeat(256),
        "pkg\n".to_owned(),
        "deb:editor fork:amd64".to_owned(),
    ] {
        let mut malformed = identity.clone();
        malformed.package_identity = package_identity.clone();
        assert_eq!(
            world.evaluate(&malformed, &request),
            denied(deny_reason_t::unknown_identity),
            "{package_identity:?}"
        );
        let _ = world.policy.network_mode(Launcher, &malformed);
        assert_eq!(
            journal_bodies(&world.root, alice.uid),
            before,
            "{package_identity:?}: nothing journaled"
        );
    }
    let mut longest = identity.clone();
    longest.package_identity = "p".repeat(255);
    assert_eq!(
        world.evaluate(&longest, &request),
        denied(deny_reason_t::unknown_identity)
    );
    let bodies = journal_bodies(&world.root, alice.uid);
    assert_eq!(bodies.len(), before.len() + 1);
    assert_eq!(
        bodies.last(),
        Some(&record_body_t::lifecycle(
            lifecycle_record_t::identity_mismatch {
                lineage_id: genesis.payload.lineage_id,
                package_identity: "p".repeat(255),
                executable_sha256: editor.executable_sha256,
            }
        ))
    );
    assert!(is_eligible(world.evaluate(&identity, &request)));
    let root = world.root.clone();
    let time = world.time.clone();
    world.registry.shutdown().expect("shutdown");
    drop(world);
    let registry = registry_t::open(
        &root,
        fs::metadata(&root).expect("root").uid(),
        registry_config_t::default(),
        Box::new(system_io_t),
        Box::new(time),
    )
    .expect("restart");
    assert!(registry.unusable_users().expect("users").is_empty());
    assert!(
        registry
            .rollback_status(Contractd)
            .expect("status")
            .quarantined
            .is_empty()
    );
}

#[test]
fn reserve_use_evaluates_completely_before_registryd_reserves() {
    // ADR 0007 §4 option A, erratum E7.1: policyd evaluates the request against the
    // named one-shot grant (mandatory policy, subject, session liveness, every rule)
    // and only then has registryd reserve it. Nothing is consumed by a refusal.
    let root = temp_root("reserve");
    let alice = test_user(1001);
    let scanner = app("scanner");
    let identity = runtime(&alice, &scanner);
    let genesis = draft(&alice, &scanner, commit_type_t::genesis, "reserve genesis")
        .add(vec![file_grant(
            "reserve cache",
            alice.uid,
            "home:Cache",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&alice);
    let scan = file_grant(
        "scan",
        alice.uid,
        "home:Scan",
        &["read"],
        one_shot_lifetime(),
    );
    let mut in_call = one_shot_lifetime();
    in_call.session_id = Some(synthetic_id("session call"));
    let call = file_grant("call", alice.uid, "home:Call", &["read"], in_call);
    let branch = draft(&alice, &scanner, commit_type_t::temporary_grant, "reserve")
        .on(&genesis)
        .add(vec![scan.clone(), call.clone()])
        .temporary_authority()
        .sign(&alice);
    let operation = synthetic_id("scan operation");
    let standing = |world: &world_t, grant: &random_id_t| {
        world.registry.standing(alice.uid, grant).expect("standing")
    };
    let unconsumed = cham3_registryd::lineage::standing_t::current;
    {
        // Mandatory policy forbids the scan: refused, nothing reserved.
        let prohibited = mandatory_policy_t {
            prohibitions: vec![prohibition_t {
                category: resource_category_t::filesystem,
                resource_id: Some("home:Scan".to_owned()),
                operation: None,
                owner_uid: None,
            }],
            ..mandatory_policy_t::default()
        };
        let mut world = world_at(&root, registry_config_t::default(), prohibited);
        world.enroll(&alice);
        world.submit(&alice, &genesis);
        world.submit(&alice, &branch);
        assert_eq!(
            world.policy.reserve_use(
                Launcher,
                &identity,
                &files(alice.uid, "home:Scan", "read"),
                &scan.grant_id,
                &operation,
            ),
            Ok(reserve_outcome_t::refused(denied(
                deny_reason_t::mandatory_policy
            )))
        );
        assert_eq!(standing(&world, &scan.grant_id), unconsumed);
        world.registry.shutdown().expect("shutdown");
    }
    let mut world = world_at(
        &root,
        registry_config_t::default(),
        mandatory_policy_t::default(),
    );
    let scan_request = files(alice.uid, "home:Scan", "read");
    // Only the consumer role; a request outside the grant; another grant id; a
    // grant that is not one-shot; an updated executable.
    assert_eq!(
        code(world.policy.reserve_use(
            Contractd,
            &identity,
            &scan_request,
            &scan.grant_id,
            &operation
        )),
        "ipc/role_denied"
    );
    let mut refusals = vec![
        (
            files(alice.uid, "home:Scan", "write"),
            scan.grant_id,
            identity.clone(),
            denied(deny_reason_t::no_matching_grant),
        ),
        (
            scan_request.clone(),
            call.grant_id,
            identity.clone(),
            denied(deny_reason_t::no_matching_grant),
        ),
        (
            files(alice.uid, "home:Cache", "read"),
            synthetic_id("grant reserve cache"),
            identity.clone(),
            denied(deny_reason_t::not_one_shot),
        ),
    ];
    let mut updated = identity.clone();
    updated.executable_sha256 = [9; 32];
    refusals.push((
        scan_request.clone(),
        scan.grant_id,
        updated,
        denied(deny_reason_t::unknown_identity),
    ));
    // A session-bound one-shot without a live attestation.
    let mut in_session = identity.clone();
    in_session.session_id = Some(synthetic_id("session call"));
    refusals.push((
        files(alice.uid, "home:Call", "read"),
        call.grant_id,
        in_session.clone(),
        denied(deny_reason_t::session_not_live),
    ));
    for (request, grant_id, runtime_identity, decision) in refusals {
        assert_eq!(
            world
                .policy
                .reserve_use(Launcher, &runtime_identity, &request, &grant_id, &operation),
            Ok(reserve_outcome_t::refused(decision)),
            "{request:?}"
        );
    }
    assert_eq!(standing(&world, &scan.grant_id), unconsumed);
    assert_eq!(standing(&world, &call.grant_id), unconsumed);
    // Eligible: reserved and consumed; the next evaluation and reservation are
    // denied.
    let reserved = world
        .policy
        .reserve_use(
            Launcher,
            &identity,
            &scan_request,
            &scan.grant_id,
            &operation,
        )
        .expect("reserve");
    let reserve_outcome_t::reserved {
        grant_id,
        commit_id,
        reservation_id,
        ..
    } = reserved
    else {
        panic!("not reserved: {reserved:?}")
    };
    assert_eq!((grant_id, commit_id), (scan.grant_id, branch.commit_id));
    let consumed = denied(deny_reason_t::grant_ended(
        cham3_registryd::lineage::end_reason_t::consumed,
    ));
    assert_eq!(world.evaluate(&identity, &scan_request), consumed);
    assert_eq!(
        world.policy.reserve_use(
            Launcher,
            &identity,
            &scan_request,
            &scan.grant_id,
            &operation
        ),
        Ok(reserve_outcome_t::refused(consumed))
    );
    // A live attestation makes the session-bound one-shot reservable.
    world.sessions.attested.borrow_mut().insert(
        synthetic_id("session call"),
        session_attestation_t {
            live: true,
            observed_at: T1Utc + 60,
        },
    );
    assert!(matches!(
        world.policy.reserve_use(
            Launcher,
            &in_session,
            &files(alice.uid, "home:Call", "read"),
            &call.grant_id,
            &operation
        ),
        Ok(reserve_outcome_t::reserved { .. })
    ));
    // The consumer reports the outcome to registryd; the grant stays consumed.
    world
        .registry
        .complete_use(
            Launcher,
            alice.uid,
            &reservation_id,
            cham3_registryd::record::use_outcome_t::failure,
        )
        .expect("completed");
    assert_eq!(world.evaluate(&identity, &scan_request), consumed);
}
