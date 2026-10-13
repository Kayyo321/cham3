//! The M2-005 path end to end, in process: vaultd creates a user's keys (registered
//! through identityd into registryd's journal), signs an exactly approved proposal;
//! contractd submits the envelope; registryd admits it durably; policyd evaluates
//! the grant eligible. Then a restart, a stale proposal and a compromised key.
//!
//! In-process composition only: no sockets or processes (M2-007). Synthetic user and
//! passphrase; the KDF runs at its minimum calibration (a real 64 MiB Argon2id).
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
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use builders::*;
use cham3_contract_format::dcbor::encode;
use cham3_contract_format::envelope::{protected_header_bytes, to_be_signed};
use cham3_contract_format::hash::{digest_t, lineage_id, sha256};
use cham3_contract_format::object_kind_t;
use cham3_contract_format::schema::{
    commit_payload_t, commit_type_t, random_id_t, resource_category_t,
};
use cham3_contractd::submit::contract_service_t;
use cham3_identityd::caller::{caller_role_t, service_name_t};
use cham3_identityd::index::{naming_effect_t, naming_t, recovery_status_t};
use cham3_identityd::outcome::{ipc, ipc_code_t, service_result_t};
use cham3_identityd::service::identity_service_t;
use cham3_policyd::evaluate::{
    decision_t, deny_reason_t, resource_request_t, runtime_identity_t, session_attestation_t,
    session_source_t,
};
use cham3_policyd::mandatory::mandatory_policy_t;
use cham3_policyd::service::{policy_clock_t, policy_service_t};
use cham3_registryd::journal::system_io_t;
use cham3_registryd::registry::{
    registry_clock_t, registry_config_t, registry_identity_store_t, registry_t,
};
use cham3_secure_memory::secret_region_t;
use cham3_vaultd::clock::vault_clock_t;
use cham3_vaultd::kdf::kdf_params_t;
use cham3_vaultd::links::recovery_registry_t;
use cham3_vaultd::service::{created_keys_t, proposal_request_t, vault_config_t, vault_service_t};

const Contractd: caller_role_t = caller_role_t::service(service_name_t::contractd);
const Alice: caller_role_t = caller_role_t::trusted_client { uid: 1001 };
/// 2026-10-13T09:00:00Z.
const T1Utc: i64 = 1_791_882_000;

type identity_t = Rc<RefCell<identity_service_t<registry_identity_store_t>>>;

struct fixed_time_t;

impl registry_clock_t for fixed_time_t {
    fn utc_seconds(&self) -> i64 {
        T1Utc
    }
}

#[derive(Clone, Copy)]
struct vault_time_t;

impl vault_clock_t for vault_time_t {
    fn monotonic_seconds(&self) -> u64 {
        1000
    }

    fn utc_seconds(&self) -> i64 {
        T1Utc
    }
}

struct trusted_now_t {
    now: Rc<Cell<i64>>,
}

impl policy_clock_t for trusted_now_t {
    fn trusted_now(&self) -> Option<i64> {
        Some(self.now.get())
    }
}

struct no_sessions_t;

impl session_source_t for no_sessions_t {
    fn liveness(&self, _: &random_id_t) -> Option<session_attestation_t> {
        None
    }
}

/// No recovery ceremony runs in this test; the ceremony operations are M2-006.
struct no_ceremony_t;

impl recovery_registry_t for no_ceremony_t {
    fn recovery_status(&mut self, _: u32, _: &random_id_t) -> service_result_t<recovery_status_t> {
        Err(ipc(ipc_code_t::storage_unavailable))
    }

    fn name_recovery_keys(&mut self, _: u32, _: &naming_t) -> service_result_t<naming_effect_t> {
        Err(ipc(ipc_code_t::storage_unavailable))
    }
}

type vault_t = vault_service_t<
    cham3_vaultd::links::in_process_identity_t<registry_identity_store_t>,
    no_ceremony_t,
    vault_time_t,
>;

struct services_t {
    registry: Arc<registry_t>,
    identity: identity_t,
    contract: contract_service_t<
        cham3_contractd::submit::in_process_identity_t<registry_identity_store_t>,
    >,
    policy:
        policy_service_t<cham3_policyd::service::in_process_identity_t<registry_identity_store_t>>,
}

fn root(label: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("cham3_end_to_end_{label}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("registry")).expect("root");
    for directory in [root.clone(), root.join("registry")] {
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).expect("mode");
    }
    root
}

fn start(root: &Path) -> services_t {
    let registry_root = root.join("registry");
    let owner = fs::metadata(&registry_root).expect("root").uid();
    let registry = Arc::new(
        registry_t::open(
            &registry_root,
            owner,
            registry_config_t::default(),
            Box::new(system_io_t),
            Box::new(fixed_time_t),
        )
        .expect("registryd"),
    );
    let identity: identity_t = Rc::new(RefCell::new(
        identity_service_t::start(registry_identity_store_t {
            registry: Arc::clone(&registry),
        })
        .expect("identityd"),
    ));
    let now = Rc::new(Cell::new(T1Utc + 60));
    services_t {
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
            mandatory_policy_t::default(),
            Box::new(trusted_now_t { now }),
            Box::new(no_sessions_t),
        ),
        registry,
        identity,
    }
}

fn vault(root: &Path, services: &services_t) -> vault_t {
    let mut config = vault_config_t::new(root.join("vault"));
    config.kdf = kdf_params_t::Minimum;
    vault_service_t::start(
        config,
        cham3_vaultd::links::in_process_identity_t {
            service: Rc::clone(&services.identity),
        },
        no_ceremony_t,
        vault_time_t,
    )
    .expect("vaultd")
}

fn passphrase() -> secret_region_t {
    let text = b"synthetic end-to-end passphrase";
    let mut region = secret_region_t::new(text.len()).expect("region");
    region.as_mut_slice().copy_from_slice(text);
    region
}

/// A payload for the vault-created keys of uid 1001 (the synthetic builder user's
/// keys are replaced by the vault's).
fn payload_for(keys: &created_keys_t, app: &app_t, draft: draft_t) -> commit_payload_t {
    let builder_user = test_user(1001);
    let mut payload = draft.sign(&builder_user).payload;
    payload.actor.user_id = keys.user_id;
    payload.subject.owner_user_id = keys.user_id;
    payload.key_id = keys.persistent_key_id;
    payload.lineage_id = lineage_id(&keys.user_id, &app.app_id, &app.installation_id);
    payload
}

/// Registers, approves (the client recomputes the digest from the bytes) and signs.
fn sign_through_vault(vault: &mut vault_t, payload: &commit_payload_t, label: &str) -> Vec<u8> {
    let bytes = encode(&payload.to_value());
    let registered = vault
        .register_proposal(
            Contractd,
            proposal_request_t {
                proposal_id: synthetic_id(&format!("proposal {label}")),
                owner_uid: 1001,
                kind: object_kind_t::commit,
                key_id: payload.key_id,
                expected_head: payload.parents.first().copied(),
                policy_epoch: payload.policy.epoch,
                expires_in_seconds: 120,
                payload: bytes.clone(),
            },
        )
        .expect("proposal");
    let protected = protected_header_bytes(&payload.key_id, object_kind_t::commit);
    let digest: digest_t = sha256(&to_be_signed(object_kind_t::commit, &protected, &bytes));
    assert_eq!(digest, registered.approval_digest);
    vault
        .approve_and_sign(Alice, &registered.proposal_id, &digest, &passphrase())
        .expect("signature")
}

#[test]
fn a_vault_signed_commit_is_admitted_durably_and_becomes_eligible() {
    let root = root("path");
    let editor = app("editor");
    let builder_user = test_user(1001);
    let read = file_grant(
        "e2e downloads",
        1001,
        "home:Downloads",
        &["read"],
        persistent_lifetime(),
    );
    let request = resource_request_t {
        category: resource_category_t::filesystem,
        resource_id: "home:Downloads".to_owned(),
        resource_owner_uid: 1001,
        operation: "read".to_owned(),
    };
    let identity = runtime_identity_t {
        owner_uid: 1001,
        app_id: editor.app_id.clone(),
        installation_id: editor.installation_id,
        package_identity: editor.package_identity.clone(),
        executable_sha256: editor.executable_sha256,
        runtime_scope: None,
        session_id: None,
    };
    let (keys, genesis_envelope) = {
        let mut services = start(&root);
        let mut vault = vault(&root, &services);
        let keys = vault.create_user_keys(Alice, &passphrase()).expect("keys");
        let genesis = payload_for(
            &keys,
            &editor,
            draft(
                &builder_user,
                &editor,
                commit_type_t::genesis,
                "e2e genesis",
            )
            .add(vec![read.clone()]),
        );
        let envelope = sign_through_vault(&mut vault, &genesis, "genesis");
        // Before admission nothing is eligible.
        assert_eq!(
            services
                .policy
                .evaluate(caller_role_t::fixture_consumer, &identity, &request)
                .expect("evaluate"),
            decision_t::denied(deny_reason_t::no_matching_grant)
        );
        let receipt = services
            .contract
            .submit_signed(Alice, &envelope)
            .expect("admitted");
        assert_eq!(receipt.commit_id, sha256(&envelope));
        assert_eq!(
            receipt.status,
            cham3_registryd::lineage::commit_status_t::pending
        );
        assert_eq!(
            services
                .policy
                .evaluate(caller_role_t::fixture_consumer, &identity, &request)
                .expect("evaluate"),
            decision_t::eligible {
                grant_id: read.grant_id,
                commit_id: receipt.commit_id
            }
        );
        // Another UID cannot submit Alice's commit; the same bytes cannot be admitted
        // twice.
        assert_eq!(
            services
                .contract
                .submit_signed(caller_role_t::trusted_client { uid: 1002 }, &envelope)
                .err()
                .map(|error| error.to_string()),
            Some("ipc/owner_mismatch".to_owned())
        );
        assert!(services.contract.submit_signed(Alice, &envelope).is_err());
        assert!(
            services
                .registry
                .unusable_users()
                .expect("users")
                .is_empty()
        );
        (keys, envelope)
    };
    // Restart every service: the journal alone carries identity and authority.
    let mut services = start(&root);
    assert_eq!(
        services
            .policy
            .evaluate(caller_role_t::fixture_consumer, &identity, &request)
            .expect("evaluate"),
        decision_t::eligible {
            grant_id: read.grant_id,
            commit_id: sha256(&genesis_envelope)
        }
    );
    // Two proposals prepared against the same head: the first admitted wins, the
    // other is stale and must be re-proposed (ADR 0005 §3).
    let mut vault = vault(&root, &services);
    let genesis = cham3_contract_format::verify_object(
        &genesis_envelope,
        object_kind_t::commit,
        &single_key_t {
            key_id: keys.persistent_key_id,
            identity: Rc::clone(&services.identity),
        },
    )
    .expect("genesis verifies");
    let genesis = commit_t {
        commit_id: genesis.object_id,
        envelope: genesis_envelope.clone(),
        payload: match genesis.payload {
            cham3_contract_format::payload_t::commit(payload) => *payload,
            _ => panic!("commit"),
        },
    };
    let first = payload_for(
        &keys,
        &editor,
        draft(&builder_user, &editor, commit_type_t::revision, "e2e first")
            .on(&genesis)
            .add(vec![file_grant(
                "e2e first",
                1001,
                "home:First",
                &["read"],
                persistent_lifetime(),
            )]),
    );
    let second = payload_for(
        &keys,
        &editor,
        draft(
            &builder_user,
            &editor,
            commit_type_t::revision,
            "e2e second",
        )
        .on(&genesis)
        .add(vec![file_grant(
            "e2e second",
            1001,
            "home:Second",
            &["read"],
            persistent_lifetime(),
        )]),
    );
    let first_envelope = sign_through_vault(&mut vault, &first, "first");
    let second_envelope = sign_through_vault(&mut vault, &second, "second");
    services
        .contract
        .submit_signed(Alice, &first_envelope)
        .expect("first admitted");
    assert_eq!(
        services
            .contract
            .submit_signed(Alice, &second_envelope)
            .err()
            .map(|error| error.to_string()),
        Some("admission/stale_head".to_owned())
    );
    // A compromise reported through the vault ends the grants that key signed.
    vault
        .invalidate_key(
            Alice,
            &keys.persistent_key_id,
            cham3_contract_format::schema::invalidation_reason_t::compromised,
            &passphrase(),
        )
        .expect("invalidation");
    assert_eq!(
        services
            .policy
            .evaluate(caller_role_t::fixture_consumer, &identity, &request)
            .expect("evaluate"),
        decision_t::denied(deny_reason_t::key_invalidated)
    );
}

/// Resolves the vault-created persistent key through identityd (test helper).
struct single_key_t {
    key_id: digest_t,
    identity: identity_t,
}

impl cham3_contract_format::key_resolver_t for single_key_t {
    fn resolve(&self, kid: &digest_t) -> Option<[u8; 32]> {
        if *kid != self.key_id {
            return None;
        }
        self.identity
            .borrow_mut()
            .resolve_key(Contractd, kid)
            .ok()
            .map(|resolution| resolution.public_key)
    }
}
