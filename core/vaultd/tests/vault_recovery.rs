//! vaultd `recover_keys` (ADR 0006 §10 step 3; service IPC §3, revision 4 R14):
//! fresh ceremony keys, certified at the ceremony epoch and named last; the scoped
//! authority of the named key; reconciliation after a lost anchor update, a restart,
//! an unavailable status and an abandoned ceremony, never erasing a key that can
//! still be named and never generating new keys while held ones can be.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::as_conversions
)]

mod fixture;

use cham3_contract_format::hash::digest_t;
use cham3_contract_format::object_kind_t;
use cham3_contract_format::schema::authority_class_t;
use cham3_identityd::caller::{caller_role_t, service_name_t};
use cham3_identityd::index::{ceremony_mode_t, key_scope_t, record_role_t, user_record_t};
use cham3_identityd::outcome::{authority_code_t, service_error_t};
use cham3_vaultd::service::{created_keys_t, proposal_request_t};
use fixture::*;

const Contractd: caller_role_t = caller_role_t::service(service_name_t::contractd);
const Alice: caller_role_t = caller_role_t::trusted_client { uid: 1001 };
const AlicePassphrase: &str = "synthetic alice passphrase";
const CommitTime: &str = "2026-10-13T08:05:00Z";
const RecoveryId: [u8; 16] = [0x7c; 16];

fn code<value_t: std::fmt::Debug>(result: Result<value_t, service_error_t>) -> String {
    match result {
        Ok(value) => panic!("expected a refusal, got {value:?}"),
        Err(error) => error.to_string(),
    }
}

fn enrolled(label: &str) -> (std::path::PathBuf, identity_t, vault_t, created_keys_t) {
    let root = temp_root(label);
    let identity = identity_service();
    let mut vault = vault(&root, &identity);
    let alice = vault
        .create_user_keys(Alice, &passphrase(AlicePassphrase))
        .expect("enroll");
    (root, identity, vault, alice)
}

fn contain_and_open(identity: &identity_t) {
    push_record(identity, 1001, user_record_t::containment);
    push_record(
        identity,
        1001,
        user_record_t::recovery_opened {
            recovery_id: RecoveryId,
            key_epoch: 2,
            mode: ceremony_mode_t::c1,
            role: record_role_t::trusted_client,
        },
    );
}

fn scope(identity: &identity_t, key: &digest_t) -> key_scope_t {
    identity
        .borrow_mut()
        .resolve_key(Contractd, key)
        .expect("resolve")
        .scope
}

fn request(
    label: &str,
    payload: &cham3_contract_format::schema::commit_payload_t,
) -> proposal_request_t {
    proposal_request_t {
        proposal_id: proposal_id(label),
        owner_uid: 1001,
        kind: object_kind_t::commit,
        key_id: payload.key_id,
        expected_head: trunk_parent(payload),
        policy_epoch: payload.policy.epoch,
        expires_in_seconds: 120,
        payload: encoded(payload),
    }
}

fn key_files(root: &std::path::Path) -> usize {
    std::fs::read_dir(root.join("1001"))
        .unwrap()
        .filter(|entry| {
            entry
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".key")
        })
        .count()
}

#[test]
fn recovery_keys_replace_every_contract_key_and_sign_only_ceremony_commits() {
    let (root, identity, mut vault, alice) = enrolled("ceremony");
    let secret = passphrase(AlicePassphrase);
    push_record(&identity, 1001, user_record_t::containment);
    let old = commit_payload(
        "commit_genesis",
        1001,
        &alice.user_id,
        &alice.persistent_key_id,
        authority_class_t::persistent,
        CommitTime,
    );
    assert_eq!(
        code(vault.register_proposal(Contractd, request("contained", &old))),
        "key_authority/contained"
    );
    assert_eq!(
        code(vault.rotate_key(Alice, authority_class_t::persistent, &secret)),
        "key_authority/contained"
    );
    assert_eq!(
        code(vault.recover_keys(Alice, &RecoveryId, &secret)),
        "admission/recovery_rule",
        "no ceremony is open"
    );
    push_record(
        &identity,
        1001,
        user_record_t::recovery_opened {
            recovery_id: RecoveryId,
            key_epoch: 2,
            mode: ceremony_mode_t::c1,
            role: record_role_t::trusted_client,
        },
    );
    assert_eq!(
        code(vault.recover_keys(Alice, &[1; 16], &secret)),
        "admission/recovery_rule",
        "another ceremony"
    );
    assert_eq!(
        code(vault.recover_keys(Alice, &RecoveryId, &passphrase("wrong"))),
        "ipc/authentication_failed"
    );
    let (persistent, temporary) = vault
        .recover_keys(Alice, &RecoveryId, &secret)
        .expect("recover");
    assert_eq!(vault.registry_mut().naming_calls, 1);
    assert_eq!(
        key_files(&root),
        3,
        "identity plus the two named keys; pre-ceremony objects erased"
    );
    assert_eq!(
        scope(&identity, &persistent),
        key_scope_t::recovery {
            recovery_id: Some(RecoveryId)
        }
    );
    assert_eq!(
        scope(&identity, &temporary),
        key_scope_t::denied(authority_code_t::recovery_key_required)
    );
    assert_eq!(
        scope(&identity, &alice.persistent_key_id),
        key_scope_t::denied(authority_code_t::key_epoch_stale)
    );
    let epoch = identity
        .borrow_mut()
        .resolve_key(Contractd, &persistent)
        .unwrap()
        .key_epoch;
    assert_eq!(epoch, 2);

    // The named key signs a ceremony-bound restore, and nothing else.
    let mut restore = commit_payload(
        "commit_restore",
        1001,
        &alice.user_id,
        &persistent,
        authority_class_t::persistent,
        CommitTime,
    );
    restore.authorization.session_id = RecoveryId;
    let registered = vault
        .register_proposal(Contractd, request("restore", &restore))
        .expect("ceremony restore");
    vault
        .approve_and_sign(
            Alice,
            &registered.proposal_id,
            &registered.approval_digest,
            &secret,
        )
        .expect("signed");
    let mut unbound = restore.clone();
    unbound.authorization.session_id = [0; 16];
    assert_eq!(
        code(vault.register_proposal(Contractd, request("unbound", &unbound))),
        "admission/recovery_rule"
    );
    let mut revision = commit_payload(
        "commit_revision",
        1001,
        &alice.user_id,
        &persistent,
        authority_class_t::persistent,
        CommitTime,
    );
    revision.authorization.session_id = RecoveryId;
    assert_eq!(
        code(vault.register_proposal(Contractd, request("revision", &revision))),
        "admission/recovery_rule"
    );
    let grant = commit_payload(
        "commit_temporary_session",
        1001,
        &alice.user_id,
        &temporary,
        authority_class_t::temporary,
        CommitTime,
    );
    assert_eq!(
        code(vault.register_proposal(Contractd, request("grant", &grant))),
        "key_authority/recovery_key_required"
    );
    // The ceremony named its keys; a second recovery for it is refused.
    assert_eq!(
        code(vault.recover_keys(Alice, &RecoveryId, &secret)),
        "admission/recovery_rule"
    );

    // After close the recovery keys are the user's ordinary keys.
    push_record(
        &identity,
        1001,
        user_record_t::recovery_closed {
            recovery_id: RecoveryId,
        },
    );
    assert_eq!(scope(&identity, &persistent), key_scope_t::normal);
    let genesis = commit_payload(
        "commit_genesis",
        1001,
        &alice.user_id,
        &persistent,
        authority_class_t::persistent,
        CommitTime,
    );
    vault
        .register_proposal(Contractd, request("after close", &genesis))
        .expect("ordinary commit");
}

#[test]
fn a_lost_anchor_update_is_reconciled_with_the_held_keys_after_restart() {
    let (root, identity, mut vault, _alice) = enrolled("anchor");
    let secret = passphrase(AlicePassphrase);
    contain_and_open(&identity);
    vault.registry_mut().anchor_fault = true;
    assert_eq!(
        code(vault.recover_keys(Alice, &RecoveryId, &secret)),
        "ipc/storage_unavailable"
    );
    assert_eq!(
        key_files(&root),
        5,
        "nothing is erased on an unknown outcome"
    );
    assert!(root.join("1001").join("pending.reg").exists());
    let pending = vault
        .registry_mut()
        .pending
        .expect("durable, unanchored naming record");
    // Not anchored: no effect yet.
    let named = identity
        .borrow()
        .directory()
        .user(1001)
        .unwrap()
        .ceremony()
        .unwrap()
        .named;
    assert_eq!(named, None);

    // registryd restarts and rolls the tail forward; vaultd restarts and reconciles.
    let mut rolled = registry(&identity);
    rolled.pending = Some(pending);
    let mut restarted = vault_with(&root, &identity, rolled);
    let keys = restarted
        .recover_keys(Alice, &RecoveryId, &secret)
        .expect("reconciled");
    assert_eq!(
        keys,
        (pending.1.persistent_key_id, pending.1.temporary_key_id),
        "the held keys, not new ones"
    );
    assert_eq!(
        restarted.registry_mut().naming_calls,
        0,
        "no second naming record"
    );
    assert_eq!(key_files(&root), 3);
    assert!(!root.join("1001").join("pending.reg").exists());
    assert_eq!(
        scope(&identity, &keys.0),
        key_scope_t::recovery {
            recovery_id: Some(RecoveryId)
        }
    );
}

#[test]
fn an_unavailable_status_keeps_every_object_and_a_retry_resumes() {
    let (root, identity, mut vault, _alice) = enrolled("status");
    let secret = passphrase(AlicePassphrase);
    contain_and_open(&identity);
    vault.registry_mut().naming_unavailable = true;
    vault.registry_mut().status_unavailable = true;
    assert_eq!(
        code(vault.recover_keys(Alice, &RecoveryId, &secret)),
        "ipc/storage_unavailable"
    );
    assert_eq!(key_files(&root), 5);
    let held: Vec<digest_t> = vault.store().list_keys(1001).unwrap();
    // A later call while the status is still unavailable changes nothing.
    assert_eq!(
        code(vault.recover_keys(Alice, &RecoveryId, &secret)),
        "ipc/storage_unavailable"
    );
    assert_eq!(vault.store().list_keys(1001).unwrap(), held);
    vault.registry_mut().naming_unavailable = false;
    vault.registry_mut().status_unavailable = false;
    let keys = vault
        .recover_keys(Alice, &RecoveryId, &secret)
        .expect("resumed");
    assert!(
        held.contains(&keys.0) && held.contains(&keys.1),
        "resumed with the held keys"
    );
    assert_eq!(key_files(&root), 3);
}

fn abandon_attempt(label: &str) -> (std::path::PathBuf, identity_t, vault_t, Vec<digest_t>) {
    let (root, identity, mut vault, _alice) = enrolled(label);
    let secret = passphrase(AlicePassphrase);
    contain_and_open(&identity);
    vault.registry_mut().naming_unavailable = true;
    assert_eq!(
        code(vault.recover_keys(Alice, &RecoveryId, &secret)),
        "ipc/storage_unavailable"
    );
    let held = vault.store().list_keys(1001).unwrap();
    assert_eq!(held.len(), 5);
    // A new unverifiable run contains the user again and aborts the ceremony.
    push_record(&identity, 1001, user_record_t::containment);
    vault.registry_mut().naming_unavailable = false;
    (root, identity, vault, held)
}

#[test]
fn keys_of_an_abandoned_ceremony_are_erased_and_never_named() {
    let (root, _identity, mut vault, _held) = abandon_attempt("abandoned");
    let secret = passphrase(AlicePassphrase);
    assert_eq!(
        code(vault.recover_keys(Alice, &RecoveryId, &secret)),
        "admission/recovery_rule"
    );
    assert_eq!(
        key_files(&root),
        3,
        "the attempt's two keys are erased; the earlier keys stay"
    );
    assert!(!root.join("1001").join("pending.reg").exists());
}

#[test]
fn a_new_ceremony_proceeds_past_an_abandoned_attempt() {
    let (root, identity, mut vault, held) = abandon_attempt("new_ceremony");
    let secret = passphrase(AlicePassphrase);
    // A new ceremony needs a higher epoch; its keys are new, the abandoned attempt's
    // are erased on the way and never named.
    push_record(
        &identity,
        1001,
        user_record_t::recovery_opened {
            recovery_id: [0x7d; 16],
            key_epoch: 3,
            mode: ceremony_mode_t::c1,
            role: record_role_t::trusted_client,
        },
    );
    let (persistent, temporary) = vault
        .recover_keys(Alice, &[0x7d; 16], &secret)
        .expect("new ceremony");
    assert!(!held.contains(&persistent) && !held.contains(&temporary));
    assert_eq!(
        identity
            .borrow_mut()
            .resolve_key(Contractd, &persistent)
            .unwrap()
            .key_epoch,
        3
    );
    assert_eq!(key_files(&root), 3, "identity plus the new ceremony's keys");
    for key in held {
        let resolution = identity.borrow_mut().resolve_key(Contractd, &key).unwrap();
        if resolution.class != cham3_identityd::index::key_class_t::identity {
            assert!(matches!(resolution.scope, key_scope_t::denied(_)));
        }
    }
}
