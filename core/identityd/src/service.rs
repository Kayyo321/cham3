//! cham3-identityd operations (service IPC §3), with caller-role checks.
//!
//! The transport (socket activation, `SO_PEERCRED`, framing) is M2-007; here every
//! operation takes the caller role the transport derived from peer credentials. The
//! service is single-threaded: operations are serialized, which also serializes them
//! per user.

use std::collections::{BTreeMap, BTreeSet};

use cham3_contract_format::hash::digest_t;
use cham3_contract_format::schema::invalidation_reason_t;
use cham3_contract_format::{object_kind_t, payload_t};

use crate::caller::{caller_role_t, service_name_t};
use crate::directory::{identity_directory_t, identity_effect_t};
use crate::index::{ceremony_t, key_class_t, key_resolution_t, key_scope_t, user_state_t};
use crate::journal::{append_receipt_t, identity_journal_t};
use crate::outcome::{
    authority, authority_code_t, ipc, ipc_code_t, service_error_t, service_result_t,
};

/// Journal sequences of a registered enrollment and its two certificates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct enrollment_receipt_t {
    pub enrollment: append_receipt_t,
    pub persistent: append_receipt_t,
    pub temporary: append_receipt_t,
}

/// Answer of `user_identity`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct user_identity_t {
    pub uid: u32,
    pub user_id: digest_t,
    pub identity_epoch: u64,
    pub state: user_state_t,
    pub min_key_epoch: u64,
    /// Contract keys whose scope is not denied, by class.
    pub persistent_key_ids: Vec<digest_t>,
    pub temporary_key_ids: Vec<digest_t>,
    /// The open recovery ceremony, if any (vaultd `recover_keys` needs its epoch).
    pub ceremony: Option<ceremony_t>,
    pub journal_seq: u64,
}

/// identityd: key-authority index over registryd's journal.
pub struct identity_service_t<journal_impl_t: identity_journal_t> {
    journal: journal_impl_t,
    directory: identity_directory_t,
    applied: BTreeMap<u32, u64>,
    /// Users whose journal could not be read or applied; each is rebuilt from the
    /// start of its journal on the next refresh and denied until that succeeds.
    unusable_users: BTreeSet<u32>,
    /// Keys of unusable users: answered `storage_unavailable`, never unknown.
    unusable_keys: BTreeMap<digest_t, u32>,
    /// Invalidations accepted but not yet durable: denied in memory first (ADR 0006
    /// §§3, 5).
    pending_denials: BTreeMap<digest_t, invalidation_reason_t>,
}

/// Upper bound on `identity_state` pages read for one user in one refresh.
const MaxRefreshPages: usize = 1 << 20;

impl<journal_impl_t: identity_journal_t> identity_service_t<journal_impl_t> {
    /// Starts identityd: rebuilds the index of every user from the journal. A user
    /// whose records do not apply stays denied (`storage_unavailable`).
    pub fn start(journal: journal_impl_t) -> service_result_t<identity_service_t<journal_impl_t>> {
        let users = journal.journal_users()?;
        let mut service = identity_service_t {
            journal,
            directory: identity_directory_t::new(),
            applied: BTreeMap::new(),
            unusable_users: BTreeSet::new(),
            unusable_keys: BTreeMap::new(),
            pending_denials: BTreeMap::new(),
        };
        for uid in users {
            // A failure is recorded per user; other users still start.
            let _ = service.refresh(uid);
        }
        Ok(service)
    }

    pub fn journal(&self) -> &journal_impl_t {
        &self.journal
    }

    pub fn journal_mut(&mut self) -> &mut journal_impl_t {
        &mut self.journal
    }

    pub fn directory(&self) -> &identity_directory_t {
        &self.directory
    }

    /// Applies every journal record of `uid` newer than the index.
    pub fn refresh(&mut self, uid: u32) -> service_result_t<()> {
        match self.apply_new_records(uid) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.mark_unusable(uid);
                Err(match error {
                    service_error_t::ipc(ipc_code_t::storage_unavailable) => error,
                    _ => ipc(ipc_code_t::storage_unavailable),
                })
            }
        }
    }

    fn apply_new_records(&mut self, uid: u32) -> service_result_t<()> {
        if self.unusable_users.contains(&uid) {
            // Rebuild from the start of the journal.
            self.directory.forget_user(uid);
            self.applied.remove(&uid);
        }
        let mut cursor = self.applied.get(&uid).copied().unwrap_or(0);
        for _ in 0..MaxRefreshPages {
            let page = self.journal.identity_state(uid, cursor)?;
            for entry in &page.entries {
                if entry.sequence <= cursor {
                    return Err(ipc(ipc_code_t::storage_unavailable));
                }
                self.directory
                    .apply_journal_record(uid, &entry.record, entry.sequence)?;
                cursor = entry.sequence;
                self.applied.insert(uid, cursor);
            }
            match page.next {
                Some(next) if next >= cursor && !page.entries.is_empty() => cursor = next,
                Some(_) => return Err(ipc(ipc_code_t::storage_unavailable)),
                None => {
                    self.unusable_users.remove(&uid);
                    self.unusable_keys.retain(|_, owner| *owner != uid);
                    return Ok(());
                }
            }
        }
        Err(ipc(ipc_code_t::storage_unavailable))
    }

    fn mark_unusable(&mut self, uid: u32) {
        if let Some(user) = self.directory.user(uid) {
            for (key_id, _) in user.keys() {
                self.unusable_keys.insert(*key_id, uid);
            }
        }
        self.unusable_users.insert(uid);
        self.directory.forget_user(uid);
        self.applied.remove(&uid);
    }

    // ---------------------------------------------------------------- registration

    /// `register_enrollment` (`service:vaultd`): a self-signed identity enrollment and
    /// its persistent and temporary certificates, validated together and persisted in
    /// order. First enrollment per UID only; identical objects already durable are
    /// accepted without a second record (vaultd retry).
    pub fn register_enrollment(
        &mut self,
        caller: caller_role_t,
        owner_uid: u32,
        enrollment: &[u8],
        persistent_certificate: &[u8],
        temporary_certificate: &[u8],
    ) -> service_result_t<enrollment_receipt_t> {
        require_vaultd(caller)?;
        self.refresh(owner_uid)?;
        let verified = self
            .directory
            .verify_identity_object(object_kind_t::identity_enrollment, enrollment)?;
        let mut provisional = self.directory.clone();
        let effect = provisional.check_identity_object(owner_uid, &verified)?;
        if effect == identity_effect_t::enroll {
            provisional.apply_identity_object(owner_uid, &verified, u64::MAX)?;
        }
        for (certificate, class) in [
            (persistent_certificate, key_class_t::persistent),
            (temporary_certificate, key_class_t::temporary),
        ] {
            let verified =
                provisional.verify_identity_object(object_kind_t::key_certificate, certificate)?;
            let payload_t::key_certificate(payload) = &verified.payload else {
                return Err(ipc(ipc_code_t::malformed_request));
            };
            let expected = match class {
                key_class_t::persistent => {
                    cham3_contract_format::schema::authority_class_t::persistent
                }
                _ => cham3_contract_format::schema::authority_class_t::temporary,
            };
            if payload.authority_class != expected {
                return Err(authority(authority_code_t::authority_class_denied));
            }
            if provisional.check_identity_object(owner_uid, &verified)?
                == identity_effect_t::certify
            {
                provisional.apply_identity_object(owner_uid, &verified, u64::MAX)?;
            }
        }
        let enrollment = self.append(owner_uid, object_kind_t::identity_enrollment, enrollment)?;
        let persistent = self.append(
            owner_uid,
            object_kind_t::key_certificate,
            persistent_certificate,
        )?;
        let temporary = self.append(
            owner_uid,
            object_kind_t::key_certificate,
            temporary_certificate,
        )?;
        Ok(enrollment_receipt_t {
            enrollment,
            persistent,
            temporary,
        })
    }

    /// `register_certificate` (`service:vaultd`): rotation or ceremony certificate
    /// signed by the user's identity key.
    pub fn register_certificate(
        &mut self,
        caller: caller_role_t,
        owner_uid: u32,
        certificate: &[u8],
    ) -> service_result_t<append_receipt_t> {
        require_vaultd(caller)?;
        self.refresh(owner_uid)?;
        let verified = self
            .directory
            .verify_identity_object(object_kind_t::key_certificate, certificate)?;
        match self.directory.check_identity_object(owner_uid, &verified)? {
            identity_effect_t::already_present(sequence) => Ok(append_receipt_t {
                journal_seq: sequence,
                already_present: true,
            }),
            identity_effect_t::certify => {
                self.append(owner_uid, object_kind_t::key_certificate, certificate)
            }
            _ => Err(ipc(ipc_code_t::malformed_request)),
        }
    }

    /// `register_invalidation` (`service:vaultd`): restrictive. A verified
    /// invalidation of a key this index knows denies that key in memory before any
    /// journal access; if the journal cannot be read or written the denial stays and
    /// the answer is `restriction_not_durable` (a retry completes it).
    pub fn register_invalidation(
        &mut self,
        caller: caller_role_t,
        owner_uid: u32,
        invalidation: &[u8],
    ) -> service_result_t<append_receipt_t> {
        require_vaultd(caller)?;
        let mut denied = false;
        if let Ok(verified) = self
            .directory
            .verify_identity_object(object_kind_t::key_invalidation, invalidation)
        {
            if let (payload_t::key_invalidation(payload), Ok(identity_effect_t::invalidate)) = (
                &verified.payload,
                self.directory.check_identity_object(owner_uid, &verified),
            ) {
                self.deny_in_memory(&payload.target_key_id, payload.reason);
                denied = true;
            }
        }
        let not_durable = |error: service_error_t| {
            if denied {
                ipc(ipc_code_t::restriction_not_durable)
            } else {
                error
            }
        };
        self.refresh(owner_uid).map_err(not_durable)?;
        let verified = self
            .directory
            .verify_identity_object(object_kind_t::key_invalidation, invalidation)?;
        let payload_t::key_invalidation(payload) = &verified.payload else {
            return Err(ipc(ipc_code_t::malformed_request));
        };
        let target = payload.target_key_id;
        match self.directory.check_identity_object(owner_uid, &verified)? {
            identity_effect_t::already_present(sequence) => {
                self.pending_denials.remove(&target);
                Ok(append_receipt_t {
                    journal_seq: sequence,
                    already_present: true,
                })
            }
            identity_effect_t::invalidate => {
                self.deny_in_memory(&target, payload.reason);
                let receipt = self
                    .append(owner_uid, object_kind_t::key_invalidation, invalidation)
                    .map_err(|_| ipc(ipc_code_t::restriction_not_durable))?;
                self.pending_denials.remove(&target);
                Ok(receipt)
            }
            _ => Err(ipc(ipc_code_t::malformed_request)),
        }
    }

    fn deny_in_memory(&mut self, key_id: &digest_t, reason: invalidation_reason_t) {
        let strongest = match self.pending_denials.get(key_id) {
            Some(invalidation_reason_t::compromised) => invalidation_reason_t::compromised,
            _ => reason,
        };
        self.pending_denials.insert(*key_id, strongest);
    }

    fn append(
        &mut self,
        owner_uid: u32,
        kind: object_kind_t,
        object: &[u8],
    ) -> service_result_t<append_receipt_t> {
        let receipt = self.journal.append_identity(owner_uid, kind, object)?;
        self.refresh(owner_uid)?;
        // The record must now be in the index (fail closed otherwise).
        let present = self
            .directory
            .user(owner_uid)
            .and_then(|user| user.object_sequence(&cham3_contract_format::hash::object_id(object)));
        if present != Some(receipt.journal_seq) {
            self.mark_unusable(owner_uid);
            return Err(ipc(ipc_code_t::storage_unavailable));
        }
        Ok(receipt)
    }

    // ---------------------------------------------------------------- queries

    /// `resolve_key` (`service:*`, `fixture_consumer`). Unknown keys are
    /// `key_not_certified`; a user whose journal cannot be read is
    /// `storage_unavailable`. A key with an invalidation not yet durable resolves
    /// with that invalidation. Callers enforce the returned scope.
    pub fn resolve_key(
        &mut self,
        caller: caller_role_t,
        key_id: &digest_t,
    ) -> service_result_t<key_resolution_t> {
        match caller {
            caller_role_t::service(_) | caller_role_t::fixture_consumer => {}
            _ => return Err(ipc(ipc_code_t::role_denied)),
        }
        let owner = match (
            self.directory.owner_of(key_id),
            self.unusable_keys.get(key_id),
        ) {
            (Some(owner), _) | (None, Some(&owner)) => owner,
            (None, None) => return Err(authority(authority_code_t::key_not_certified)),
        };
        self.refresh(owner)?;
        let mut resolution = self
            .directory
            .resolve(key_id)
            .ok_or(authority(authority_code_t::key_not_certified))?;
        if let Some(reason) = self.pending_denials.get(key_id) {
            resolution.invalidated = Some(match resolution.invalidated {
                Some(invalidation_reason_t::compromised) => invalidation_reason_t::compromised,
                _ => *reason,
            });
            resolution.scope = key_scope_t::denied(authority_code_t::key_invalidated);
        }
        Ok(resolution)
    }

    /// `user_identity` (`service:*`; `trusted_client` for its own UID).
    pub fn user_identity(
        &mut self,
        caller: caller_role_t,
        uid: u32,
    ) -> service_result_t<user_identity_t> {
        match caller {
            caller_role_t::service(_) => {}
            caller_role_t::trusted_client { uid: own } if own == uid => {}
            caller_role_t::trusted_client { .. } => return Err(ipc(ipc_code_t::owner_mismatch)),
            _ => return Err(ipc(ipc_code_t::role_denied)),
        }
        self.refresh(uid)?;
        let user = self
            .directory
            .user(uid)
            .ok_or(authority(authority_code_t::key_not_certified))?;
        let mut identity = user_identity_t {
            uid,
            user_id: user.user_id(),
            identity_epoch: user.identity_epoch(),
            state: user.state(),
            min_key_epoch: user.min_key_epoch(),
            persistent_key_ids: Vec::new(),
            temporary_key_ids: Vec::new(),
            ceremony: user.ceremony().cloned(),
            journal_seq: user.journal_seq(),
        };
        let usable: BTreeSet<digest_t> = user
            .keys()
            .filter(|(key_id, _)| !self.pending_denials.contains_key(*key_id))
            .filter_map(|(key_id, _)| {
                let resolution = user.resolve(key_id)?;
                (!matches!(resolution.scope, key_scope_t::denied(_))).then_some(*key_id)
            })
            .collect();
        for (key_id, key) in user.keys() {
            if !usable.contains(key_id) {
                continue;
            }
            match key.class {
                key_class_t::persistent => identity.persistent_key_ids.push(*key_id),
                key_class_t::temporary => identity.temporary_key_ids.push(*key_id),
                key_class_t::identity => {}
            }
        }
        Ok(identity)
    }
}

fn require_vaultd(caller: caller_role_t) -> service_result_t<()> {
    match caller {
        caller_role_t::service(service_name_t::vaultd) => Ok(()),
        _ => Err(ipc(ipc_code_t::role_denied)),
    }
}
