//! Test fixture for vaultd: temporary vault roots, a manual clock, a shared identity
//! service over the journal fixture, a labeled stand-in for registryd's ceremony
//! operations, an identity link that interleaves another service's change with a
//! vault operation, and commit payloads re-bound from the published vectors.
//!
//! Synthetic users and passphrases only; nothing here is a production secret.

#![allow(dead_code)]

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use cham3_contract_format::dcbor::encode;
use cham3_contract_format::hash::{digest_t, lineage_id};
use cham3_contract_format::schema::{authority_class_t, commit_payload_t, random_id_t};
use cham3_contract_format::{object_kind_t, payload_t, verify_object};
use cham3_identityd::index::{
    key_resolution_t, naming_effect_t, naming_t, recovery_status_t, user_record_t,
};
use cham3_identityd::journal::{append_receipt_t, memory_journal_t};
use cham3_identityd::outcome::{ipc_code_t, service_error_t, service_result_t};
use cham3_identityd::service::{enrollment_receipt_t, identity_service_t, user_identity_t};
use cham3_secure_memory::secret_region_t;
use cham3_vaultd::clock::vault_clock_t;
use cham3_vaultd::kdf::kdf_params_t;
use cham3_vaultd::links::{identity_client_t, in_process_identity_t, recovery_registry_t};
use cham3_vaultd::service::{vault_config_t, vault_service_t};

#[path = "../../../contract_format/tests/support/mod.rs"]
pub mod support;

/// 2026-10-13T08:00:00Z.
pub const StartUtc: i64 = 1_791_878_400;

pub type identity_t = Rc<RefCell<identity_service_t<memory_journal_t>>>;
pub type vault_t =
    vault_service_t<in_process_identity_t<memory_journal_t>, fixture_registry_t, test_clock_t>;

/// A manually advanced clock.
#[derive(Clone, Copy, Debug)]
pub struct test_clock_t {
    pub monotonic: u64,
    pub utc: i64,
}

impl vault_clock_t for test_clock_t {
    fn monotonic_seconds(&self) -> u64 {
        self.monotonic
    }

    fn utc_seconds(&self) -> i64 {
        self.utc
    }
}

/// **M2-004 fixture** standing in for registryd's `recovery_status` and
/// `name_recovery_keys` (ADR 0006 §10; service IPC §3), with the same validation as
/// the identity index and the unanchored-tail rule: a naming record whose anchor
/// update fails is durable but has no effect until a later call completes it. Not
/// the registry: no files, ledger or crash points.
pub struct fixture_registry_t {
    pub identity: identity_t,
    /// The naming record's anchor update fails (and blocks every later call while a
    /// tail is pending).
    pub anchor_fault: bool,
    /// `recovery_status` cannot be answered.
    pub status_unavailable: bool,
    /// `name_recovery_keys` fails before anything is durable (timeout or crash).
    pub naming_unavailable: bool,
    /// `name_recovery_keys` refuses with this definitive error before anything is
    /// durable (for example an access or configuration fault).
    pub naming_error: Option<service_error_t>,
    pub pending: Option<(u32, naming_t)>,
    pub naming_calls: usize,
}

impl fixture_registry_t {
    fn settle(&mut self) -> service_result_t<()> {
        let Some((uid, naming)) = self.pending else {
            return Ok(());
        };
        if self.anchor_fault {
            return Err(service_error_t::ipc(ipc_code_t::storage_unavailable));
        }
        self.pending = None;
        self.identity
            .borrow_mut()
            .journal_mut()
            .push_naming(uid, naming)?;
        Ok(())
    }
}

impl recovery_registry_t for fixture_registry_t {
    fn recovery_status(
        &mut self,
        owner_uid: u32,
        recovery_id: &random_id_t,
    ) -> service_result_t<recovery_status_t> {
        if self.status_unavailable {
            return Err(service_error_t::ipc(ipc_code_t::storage_unavailable));
        }
        self.settle()?;
        let mut identity = self.identity.borrow_mut();
        identity.refresh(owner_uid)?;
        let user = identity
            .directory()
            .user(owner_uid)
            .ok_or(service_error_t::admission(
                cham3_identityd::outcome::rule_code_t::recovery_rule,
            ))?;
        Ok(user.recovery_status(recovery_id))
    }

    fn name_recovery_keys(
        &mut self,
        owner_uid: u32,
        naming: &naming_t,
    ) -> service_result_t<naming_effect_t> {
        self.naming_calls += 1;
        if self.naming_unavailable {
            return Err(service_error_t::ipc(ipc_code_t::storage_unavailable));
        }
        if let Some(error) = self.naming_error {
            return Err(error);
        }
        self.settle()?;
        let effect = {
            let mut identity = self.identity.borrow_mut();
            identity.refresh(owner_uid)?;
            let user = identity
                .directory()
                .user(owner_uid)
                .ok_or(service_error_t::admission(
                    cham3_identityd::outcome::rule_code_t::recovery_rule,
                ))?;
            user.check_naming(owner_uid, naming)?
        };
        match effect {
            naming_effect_t::already_named => Ok(effect),
            naming_effect_t::name if self.anchor_fault => {
                self.pending = Some((owner_uid, *naming));
                Err(service_error_t::ipc(ipc_code_t::storage_unavailable))
            }
            naming_effect_t::name => {
                self.identity
                    .borrow_mut()
                    .journal_mut()
                    .push_naming(owner_uid, *naming)?;
                Ok(effect)
            }
        }
    }
}

/// A fresh `0700` directory for one test's vault.
pub fn temp_root(label: &str) -> PathBuf {
    let base =
        std::env::temp_dir().join(format!("cham3_vaultd_test_{}_{label}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).expect("temp root");
    base.join("vault")
}

pub fn passphrase(text: &str) -> secret_region_t {
    let mut region = secret_region_t::new(text.len()).expect("secret region");
    region.as_mut_slice().copy_from_slice(text.as_bytes());
    region
}

pub fn identity_service() -> identity_t {
    Rc::new(RefCell::new(
        identity_service_t::start(memory_journal_t::new()).expect("identityd"),
    ))
}

pub fn config(root: &Path) -> vault_config_t {
    let mut config = vault_config_t::new(root.to_path_buf());
    // The cheapest calibration inside the bounds: a real 64 MiB Argon2id derivation.
    config.kdf = kdf_params_t::Minimum;
    config
}

pub fn registry(identity: &identity_t) -> fixture_registry_t {
    fixture_registry_t {
        identity: identity.clone(),
        anchor_fault: false,
        status_unavailable: false,
        naming_unavailable: false,
        naming_error: None,
        pending: None,
        naming_calls: 0,
    }
}

pub fn vault_with(root: &Path, identity: &identity_t, registry: fixture_registry_t) -> vault_t {
    vault_service_t::start(
        config(root),
        in_process_identity_t {
            service: identity.clone(),
        },
        registry,
        test_clock_t {
            monotonic: 1000,
            utc: StartUtc,
        },
    )
    .expect("vault")
}

pub fn vault(root: &Path, identity: &identity_t) -> vault_t {
    vault_with(root, identity, registry(identity))
}

pub fn push_record(identity: &identity_t, uid: u32, record: user_record_t) {
    identity
        .borrow_mut()
        .journal_mut()
        .push_user_record(uid, record)
        .expect("record");
}

/// A published valid commit re-bound to `uid`/`user_id`, signed by `key_id` of
/// `class`, at `timestamp`: actor, subject owner, lineage and grant owners change;
/// everything else (including the result state's grant ids) is the vector's.
pub fn commit_payload(
    name: &str,
    uid: u32,
    user_id: &digest_t,
    key_id: &digest_t,
    class: authority_class_t,
    timestamp: &str,
) -> commit_payload_t {
    let document = support::vectors();
    let keys = support::synthetic_keys(&document);
    let resolver = support::resolver_t::new(&keys);
    let item = document
        .get("valid")
        .map(support::json_t::items)
        .unwrap_or_default()
        .iter()
        .find(|item| item.field("name") == name)
        .expect("vector")
        .clone();
    let verified = verify_object(
        &support::hex_decode(item.field("envelope")),
        object_kind_t::commit,
        &resolver,
    )
    .expect("valid vector");
    let payload_t::commit(payload) = verified.payload else {
        panic!("commit payload");
    };
    let mut payload = *payload;
    payload.actor.uid = uid;
    payload.actor.user_id = *user_id;
    payload.subject.owner_uid = uid;
    payload.subject.owner_user_id = *user_id;
    payload.lineage_id = lineage_id(
        user_id,
        &payload.subject.app_id,
        &payload.subject.installation_id,
    );
    payload.key_id = *key_id;
    payload.authority_class = class;
    payload.timestamp = timestamp.to_owned();
    for grant in payload
        .delta
        .add_grants
        .iter_mut()
        .chain(payload.result_state.grants.iter_mut())
    {
        grant.resource.resource_owner_uid = uid;
    }
    payload
}

pub fn encoded(payload: &commit_payload_t) -> Vec<u8> {
    encode(&payload.to_value())
}

/// The trunk parent of a payload (`None` for a genesis).
pub fn trunk_parent(payload: &commit_payload_t) -> Option<digest_t> {
    payload
        .merge_base
        .or_else(|| payload.parents.first().copied())
}

pub fn proposal_id(label: &str) -> random_id_t {
    let digest = cham3_contract_format::hash::sha256(label.as_bytes());
    let mut id = [0u8; 16];
    id.copy_from_slice(&digest[..16]);
    id
}

/// An identity link that runs `change` once, right after the `resolve_key` answer
/// that `after_resolves` counts down to, standing in for another service's change
/// that lands while vaultd derives a key. An in-process interleaving, not a socket
/// race.
pub struct interleaving_identity_t {
    pub link: in_process_identity_t<memory_journal_t>,
    pub after_resolves: usize,
    pub change: Option<Box<dyn FnOnce()>>,
    pub resolves: usize,
}

impl interleaving_identity_t {
    /// Runs `change` after the `count`-th `resolve_key` answer from now.
    pub fn arm(&mut self, count: usize, change: impl FnOnce() + 'static) {
        self.after_resolves = count;
        self.change = Some(Box::new(change));
    }
}

impl identity_client_t for interleaving_identity_t {
    fn register_enrollment(
        &mut self,
        owner_uid: u32,
        enrollment: &[u8],
        persistent_certificate: &[u8],
        temporary_certificate: &[u8],
    ) -> service_result_t<enrollment_receipt_t> {
        self.link.register_enrollment(
            owner_uid,
            enrollment,
            persistent_certificate,
            temporary_certificate,
        )
    }

    fn register_certificate(
        &mut self,
        owner_uid: u32,
        certificate: &[u8],
    ) -> service_result_t<append_receipt_t> {
        self.link.register_certificate(owner_uid, certificate)
    }

    fn register_invalidation(
        &mut self,
        owner_uid: u32,
        invalidation: &[u8],
    ) -> service_result_t<append_receipt_t> {
        self.link.register_invalidation(owner_uid, invalidation)
    }

    fn resolve_key(&mut self, key_id: &digest_t) -> service_result_t<key_resolution_t> {
        let answer = self.link.resolve_key(key_id);
        self.resolves += 1;
        if self.change.is_some() {
            self.after_resolves = self.after_resolves.saturating_sub(1);
            if self.after_resolves == 0 {
                if let Some(change) = self.change.take() {
                    change();
                }
            }
        }
        answer
    }

    fn user_identity(&mut self, uid: u32) -> service_result_t<user_identity_t> {
        self.link.user_identity(uid)
    }
}

pub type interleaved_vault_t =
    vault_service_t<interleaving_identity_t, fixture_registry_t, test_clock_t>;

/// A vault whose identity link can interleave a change (see
/// [`interleaving_identity_t`]).
pub fn interleaved_vault(root: &Path, identity: &identity_t) -> interleaved_vault_t {
    vault_service_t::start(
        config(root),
        interleaving_identity_t {
            link: in_process_identity_t {
                service: identity.clone(),
            },
            after_resolves: 0,
            change: None,
            resolves: 0,
        },
        registry(identity),
        test_clock_t {
            monotonic: 1000,
            utc: StartUtc,
        },
    )
    .expect("vault")
}
