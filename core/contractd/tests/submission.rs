//! contractd `submit_signed` and `verify_object` over the real registry and
//! identityd: roles, owner binding, every byte layer and layer 6 before admission.
//! Synthetic users and keys only.
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
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::rc::Rc;
use std::sync::Arc;

use builders::*;
use cham3_contract_format::hash::digest_t;
use cham3_contract_format::object_kind_t;
use cham3_contract_format::schema::{commit_type_t, invalidation_reason_t};
use cham3_contractd::submit::{
    contract_identity_link_t, contract_service_t, in_process_identity_t,
};
use cham3_identityd::caller::{caller_role_t, service_name_t};
use cham3_identityd::index::key_resolution_t;
use cham3_identityd::outcome::{ipc, ipc_code_t, service_result_t};
use cham3_identityd::service::identity_service_t;
use cham3_registryd::journal::system_io_t;
use cham3_registryd::record::placement_t;
use cham3_registryd::registry::{
    registry_clock_t, registry_config_t, registry_identity_store_t, registry_t,
};

const Vaultd: caller_role_t = caller_role_t::service(service_name_t::vaultd);

struct fixed_time_t;

impl registry_clock_t for fixed_time_t {
    fn utc_seconds(&self) -> i64 {
        1_791_882_000
    }
}

type identity_t = Rc<RefCell<identity_service_t<registry_identity_store_t>>>;

fn start(label: &str) -> (Arc<registry_t>, identity_t) {
    let root = std::env::temp_dir().join(format!("cham3_contractd_{label}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("root");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("mode");
    let owner = fs::metadata(&root).expect("root").uid();
    let registry = Arc::new(
        registry_t::open(
            &root,
            owner,
            registry_config_t::default(),
            Box::new(system_io_t),
            Box::new(fixed_time_t),
        )
        .expect("registry"),
    );
    let identity = Rc::new(RefCell::new(
        identity_service_t::start(registry_identity_store_t {
            registry: Arc::clone(&registry),
        })
        .expect("identityd"),
    ));
    (registry, identity)
}

fn enroll(identity: &identity_t, user: &test_user_t) {
    identity
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

fn contract(
    registry: &Arc<registry_t>,
    identity: &identity_t,
) -> contract_service_t<in_process_identity_t<registry_identity_store_t>> {
    contract_service_t::new(
        in_process_identity_t {
            service: Rc::clone(identity),
        },
        Arc::clone(registry),
    )
}

fn refusal<value_t: std::fmt::Debug>(result: service_result_t<value_t>) -> String {
    match result {
        Ok(value) => panic!("expected a refusal, got {value:?}"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn submission_verifies_every_layer_before_admission() {
    let (registry, identity) = start("layers");
    let alice = test_user(1001);
    let bob = test_user(1002);
    enroll(&identity, &alice);
    let mut contract = contract(&registry, &identity);
    let alice_client = caller_role_t::trusted_client { uid: alice.uid };
    let editor = app("editor");
    let genesis = draft(&alice, &editor, commit_type_t::genesis, "submit genesis")
        .add(vec![file_grant(
            "submit",
            alice.uid,
            "home:S",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&alice);
    // Only the owner's trusted client submits.
    for role in [
        caller_role_t::service(service_name_t::contractd),
        caller_role_t::service(service_name_t::vaultd),
        caller_role_t::admin,
        caller_role_t::fixture_consumer,
    ] {
        assert_eq!(
            refusal(contract.submit_signed(role, &genesis.envelope)),
            "ipc/role_denied",
            "{role:?}"
        );
    }
    assert_eq!(
        refusal(contract.submit_signed(
            caller_role_t::trusted_client { uid: bob.uid },
            &genesis.envelope
        )),
        "ipc/owner_mismatch"
    );
    // Byte layers.
    let mut forged = genesis.envelope.clone();
    let last = forged.len() - 1;
    forged[last] ^= 1;
    assert_eq!(
        refusal(contract.submit_signed(alice_client, &forged)),
        "signature/signature_invalid"
    );
    assert_eq!(
        refusal(contract.submit_signed(alice_client, &forged[..10])),
        "envelope_decode/truncated"
    );
    assert_eq!(
        refusal(contract.submit_signed(alice_client, &alice.persistent_certificate)),
        "envelope/wrong_object_type"
    );
    let bobs = draft(&bob, &editor, commit_type_t::genesis, "bob genesis")
        .add(vec![file_grant(
            "bob",
            bob.uid,
            "home:B",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&bob);
    assert_eq!(
        refusal(contract.submit_signed(
            caller_role_t::trusted_client { uid: bob.uid },
            &bobs.envelope
        )),
        "signature/unknown_key"
    );
    // Layer 6 before admission: the temporary key signing persistent authority.
    let wrong_class = draft(
        &alice,
        &editor,
        commit_type_t::genesis,
        "temporary key genesis",
    )
    .add(vec![file_grant(
        "t",
        alice.uid,
        "home:T",
        &["read"],
        persistent_lifetime(),
    )])
    .sign_with(&alice.temporary);
    assert_eq!(
        refusal(contract.submit_signed(alice_client, &wrong_class.envelope)),
        "key_authority/authority_class_denied"
    );
    // The valid commit is admitted, pending, on the trunk.
    let receipt = contract
        .submit_signed(alice_client, &genesis.envelope)
        .expect("admitted");
    assert_eq!(receipt.placement, placement_t::trunk);
    assert_eq!(receipt.commit_id, genesis.commit_id);
    // Admission errors come through unchanged.
    assert_eq!(
        refusal(contract.submit_signed(alice_client, &genesis.envelope)),
        "admission/stale_head"
    );
    // After an invalidation the key signs nothing new.
    identity
        .borrow_mut()
        .register_invalidation(
            Vaultd,
            alice.uid,
            &invalidation(
                &alice.identity,
                &alice.persistent.key_id,
                invalidation_reason_t::rotated,
                "submit",
            ),
        )
        .expect("invalidation");
    let after = draft(&alice, &editor, commit_type_t::revision, "after rotation")
        .on(&genesis)
        .add(vec![file_grant(
            "after",
            alice.uid,
            "home:A",
            &["read"],
            persistent_lifetime(),
        )])
        .sign(&alice);
    assert_eq!(
        refusal(contract.submit_signed(alice_client, &after.envelope)),
        "key_authority/key_invalidated"
    );
}

#[test]
fn verify_object_reports_validity_only() {
    let (registry, identity) = start("verify");
    let alice = test_user(1001);
    enroll(&identity, &alice);
    let mut contract = contract(&registry, &identity);
    let genesis = draft(
        &alice,
        &app("editor"),
        commit_type_t::genesis,
        "verify genesis",
    )
    .add(vec![file_grant(
        "v",
        alice.uid,
        "home:V",
        &["read"],
        persistent_lifetime(),
    )])
    .sign(&alice);
    let report = contract
        .verify_object(
            caller_role_t::service(service_name_t::policyd),
            object_kind_t::commit,
            &genesis.envelope,
        )
        .expect("commit verifies");
    assert!(report.key_authority_checked);
    assert_eq!(report.object_id, genesis.commit_id);
    // Verification admits nothing.
    assert!(
        registry
            .lineage_head(
                caller_role_t::service(service_name_t::policyd),
                alice.uid,
                &genesis.payload.lineage_id
            )
            .expect("view")
            .expect("user")
            .trunk_head
            .is_none()
    );
    for (kind, object) in [
        (object_kind_t::identity_enrollment, &alice.enrollment),
        (
            object_kind_t::key_certificate,
            &alice.persistent_certificate,
        ),
    ] {
        let report = contract
            .verify_object(
                caller_role_t::trusted_client { uid: alice.uid },
                kind,
                object,
            )
            .expect("identity object verifies");
        assert!(!report.key_authority_checked);
    }
    for role in [caller_role_t::admin, caller_role_t::fixture_consumer] {
        assert_eq!(
            refusal(contract.verify_object(role, object_kind_t::commit, &genesis.envelope)),
            "ipc/role_denied"
        );
    }
}

/// identityd that cannot answer.
struct unavailable_t;

impl contract_identity_link_t for unavailable_t {
    fn resolve_key(&mut self, _: &digest_t) -> service_result_t<key_resolution_t> {
        Err(ipc(ipc_code_t::storage_unavailable))
    }
}

#[test]
fn an_unavailable_identityd_fails_closed() {
    let (registry, identity) = start("unavailable");
    let alice = test_user(1001);
    enroll(&identity, &alice);
    let mut contract = contract_service_t::new(unavailable_t, Arc::clone(&registry));
    let genesis = draft(
        &alice,
        &app("editor"),
        commit_type_t::genesis,
        "unavailable genesis",
    )
    .add(vec![file_grant(
        "u",
        alice.uid,
        "home:U",
        &["read"],
        persistent_lifetime(),
    )])
    .sign(&alice);
    assert_eq!(
        refusal(contract.submit_signed(
            caller_role_t::trusted_client { uid: alice.uid },
            &genesis.envelope
        )),
        "ipc/storage_unavailable"
    );
}
