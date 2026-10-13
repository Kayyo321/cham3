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

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

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
use cham3_policyd::service::{policy_clock_t, policy_service_t, rollback_protection_t};
use cham3_registryd::journal::system_io_t;
use cham3_registryd::record::placement_t;
use cham3_registryd::registry::{
    registry_clock_t, registry_config_t, registry_identity_store_t, registry_t,
};

const Contractd: caller_role_t = caller_role_t::service(service_name_t::contractd);
const Launcher: caller_role_t = caller_role_t::fixture_consumer;
const Vaultd: caller_role_t = caller_role_t::service(service_name_t::vaultd);

/// 2026-10-13T09:00:00Z (`T1`, every grant's effective start).
const T1Utc: i64 = 1_791_882_000;

type identity_t = Rc<RefCell<identity_service_t<registry_identity_store_t>>>;

struct registry_clock_fixed_t;

impl registry_clock_t for registry_clock_fixed_t {
    fn utc_seconds(&self) -> i64 {
        T1Utc
    }
}

#[derive(Clone)]
struct clock_t {
    now: Rc<Cell<Option<i64>>>,
}

impl policy_clock_t for clock_t {
    fn trusted_now(&self) -> Option<i64> {
        self.now.get()
    }
}

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
    now: Rc<Cell<Option<i64>>>,
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
    let owner = fs::metadata(root).expect("root").uid();
    let registry = Arc::new(
        registry_t::open(
            root,
            owner,
            config,
            Box::new(system_io_t),
            Box::new(registry_clock_fixed_t),
        )
        .expect("registry"),
    );
    let identity: identity_t = Rc::new(RefCell::new(
        identity_service_t::start(registry_identity_store_t {
            registry: Arc::clone(&registry),
        })
        .expect("identityd"),
    ));
    let now = Rc::new(Cell::new(Some(T1Utc + 60)));
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
            Box::new(clock_t {
                now: Rc::clone(&now),
            }),
            Box::new(sessions.clone()),
        ),
        registry,
        identity,
        now,
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
    let timed = file_grant(
        "timed",
        alice.uid,
        "home:Timed",
        &["read"],
        timed_lifetime("2026-10-13T10:00:00Z"),
    );
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
    world.now.set(Some(T1Utc - 1));
    assert_eq!(
        world.evaluate(&identity, &timed_request),
        denied(deny_reason_t::not_started)
    );
    world.now.set(Some(T1Utc));
    assert!(is_eligible(world.evaluate(&identity, &timed_request)));
    world.now.set(Some(T1Utc + 3599));
    assert!(is_eligible(world.evaluate(&identity, &timed_request)));
    world.now.set(Some(T1Utc + 3600));
    assert_eq!(
        world.evaluate(&identity, &timed_request),
        denied(deny_reason_t::expired)
    );
    world.now.set(None);
    assert_eq!(
        world.evaluate(&identity, &timed_request),
        decision_t::unknown(unknown_reason_t::clock_unavailable)
    );
    world.now.set(Some(T1Utc + 60));
    // Revocation: a removed grant is no longer in the current state, and the trunk
    // commit ends the temporary branch with its timed grant.
    let revocation = draft(&alice, &editor, commit_type_t::revocation, "rules revoke")
        .on(&genesis)
        .remove(vec![read.grant_id])
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
    assert_eq!(status.rollback_protection, rollback_protection_t::none);
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
        world.now.set(Some(T1Utc + 3600));
        assert_eq!(
            world
                .policy
                .network_mode(Launcher, &identity)
                .expect("mode"),
            network_decision_t::offline(None)
        );
        world.now.set(Some(T1Utc + 60));
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
