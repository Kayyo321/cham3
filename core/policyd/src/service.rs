//! policyd's operations (service IPC §3): `evaluate`, `network_mode` and
//! `policy_status`.
//!
//! Each evaluation reads one registryd `lineage_head` snapshot, asks identityd for
//! every signing key that snapshot names (so a denial that is not yet durable still
//! denies), reads trusted now and session liveness, and calls the pure functions of
//! [`crate::evaluate`]. Anything that cannot be read makes the outcome `unknown`
//! (denied) or Offline.
//!
//! Limits (M2-005): in-process composition; the transport is M2-007. Trusted now
//! comes from a [`policy_clock_t`] (ADR 0007 §5's offset rule and `time_sample` are
//! M2-006); one-shot reservations do not exist before M2-006, so the reserved set is
//! empty; the session source is a test stand-in until sessiond exists.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;

use cham3_contract_format::hash::{digest_t, lineage_id};
use cham3_identityd::caller::{caller_role_t, service_name_t};
use cham3_identityd::index::key_resolution_t;
use cham3_identityd::journal::identity_journal_t;
use cham3_identityd::outcome::{ipc, ipc_code_t, service_error_t, service_result_t};
use cham3_identityd::service::{identity_service_t, user_identity_t};
use cham3_registryd::registry::{lineage_view_t, registry_t};

use crate::evaluate::{
    decision_t, evaluate, key_state_t, network_decision_t, network_mode, resource_request_t,
    runtime_identity_t, session_source_t, snapshot_t, unknown_reason_t,
};
use crate::mandatory::mandatory_policy_t;

const Policyd: caller_role_t = caller_role_t::service(service_name_t::policyd);

/// Trusted now (UTC seconds), or `None` when it cannot be established.
pub trait policy_clock_t {
    fn trusted_now(&self) -> Option<i64>;
}

/// identityd as policyd uses it (role `service:policyd`).
pub trait policy_identity_link_t {
    fn resolve_key(&mut self, key_id: &digest_t) -> service_result_t<key_resolution_t>;
    fn user_identity(&mut self, uid: u32) -> service_result_t<user_identity_t>;
}

/// In-process link to an identity service shared with its other callers.
pub struct in_process_identity_t<journal_impl_t: identity_journal_t> {
    pub service: Rc<RefCell<identity_service_t<journal_impl_t>>>,
}

impl<journal_impl_t: identity_journal_t> policy_identity_link_t
    for in_process_identity_t<journal_impl_t>
{
    fn resolve_key(&mut self, key_id: &digest_t) -> service_result_t<key_resolution_t> {
        self.service
            .try_borrow_mut()
            .map_err(|_| ipc(ipc_code_t::storage_unavailable))?
            .resolve_key(Policyd, key_id)
    }

    fn user_identity(&mut self, uid: u32) -> service_result_t<user_identity_t> {
        self.service
            .try_borrow_mut()
            .map_err(|_| ipc(ipc_code_t::storage_unavailable))?
            .user_identity(Policyd, uid)
    }
}

/// `policy_status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct policy_status_t {
    pub policy_epoch: u64,
    pub min_valid_policy_epoch: u64,
    /// Emergency restriction of everyone, or of the caller's own UID.
    pub emergency_restriction: bool,
    /// Rollback protection is not established before the anchor exists (M2-006).
    pub rollback_protection: rollback_protection_t,
}

/// Rollback-protection mode reported by `policy_status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum rollback_protection_t {
    /// No anchor yet (M2-005): nothing detects a restored store.
    none,
}

/// policyd.
pub struct policy_service_t<identity_link_impl_t: policy_identity_link_t> {
    registry: Arc<registry_t>,
    identity: identity_link_impl_t,
    policy: mandatory_policy_t,
    clock: Box<dyn policy_clock_t>,
    sessions: Box<dyn session_source_t>,
}

fn allowed(caller: caller_role_t) -> service_result_t<()> {
    match caller {
        caller_role_t::service(_) | caller_role_t::fixture_consumer => Ok(()),
        _ => Err(ipc(ipc_code_t::role_denied)),
    }
}

/// The snapshot inputs of one lineage, or the reason they cannot be read.
struct inputs_t {
    view: lineage_view_t,
    key_states: BTreeMap<digest_t, key_state_t>,
    now: i64,
}

impl<identity_link_impl_t: policy_identity_link_t> policy_service_t<identity_link_impl_t> {
    pub fn new(
        registry: Arc<registry_t>,
        identity: identity_link_impl_t,
        policy: mandatory_policy_t,
        clock: Box<dyn policy_clock_t>,
        sessions: Box<dyn session_source_t>,
    ) -> policy_service_t<identity_link_impl_t> {
        policy_service_t {
            registry,
            identity,
            policy,
            clock,
            sessions,
        }
    }

    pub fn policy(&self) -> &mandatory_policy_t {
        &self.policy
    }

    fn inputs(&mut self, identity: &runtime_identity_t) -> Result<inputs_t, unknown_reason_t> {
        let user = self
            .identity
            .user_identity(identity.owner_uid)
            .map_err(|_| unknown_reason_t::identity_unavailable)?;
        let lineage = lineage_id(&user.user_id, &identity.app_id, &identity.installation_id);
        let view = self
            .registry
            .lineage_head(Policyd, identity.owner_uid, &lineage)
            .map_err(|_| unknown_reason_t::registry_unavailable)?
            .ok_or(unknown_reason_t::snapshot_inconsistent)?;
        if view.owner_user_id != user.user_id {
            return Err(unknown_reason_t::snapshot_inconsistent);
        }
        let keys: BTreeSet<digest_t> = view.origins.values().map(|origin| origin.key_id).collect();
        let mut key_states = BTreeMap::new();
        for key_id in keys {
            let resolution = self
                .identity
                .resolve_key(&key_id)
                .map_err(|_| unknown_reason_t::identity_unavailable)?;
            key_states.insert(
                key_id,
                key_state_t {
                    invalidated: resolution.invalidated,
                    user_state: resolution.user_state,
                },
            );
        }
        let now = self
            .clock
            .trusted_now()
            .ok_or(unknown_reason_t::clock_unavailable)?;
        Ok(inputs_t {
            view,
            key_states,
            now,
        })
    }

    /// `evaluate` (`service:*`, `fixture_consumer`).
    pub fn evaluate(
        &mut self,
        caller: caller_role_t,
        identity: &runtime_identity_t,
        request: &resource_request_t,
    ) -> service_result_t<decision_t> {
        allowed(caller)?;
        // Mandatory prohibitions and the emergency overlay deny without any snapshot.
        if self.policy.forbids(
            identity.owner_uid,
            request.category,
            &request.resource_id,
            &request.operation,
        ) {
            return Ok(decision_t::denied(
                crate::evaluate::deny_reason_t::mandatory_policy,
            ));
        }
        if self.policy.emergency(identity.owner_uid) {
            return Ok(decision_t::denied(
                crate::evaluate::deny_reason_t::emergency_restriction,
            ));
        }
        let inputs = match self.inputs(identity) {
            Ok(inputs) => inputs,
            Err(reason) => return Ok(decision_t::unknown(reason)),
        };
        let reserved = BTreeSet::new();
        let snapshot = snapshot_t {
            view: &inputs.view,
            key_states: &inputs.key_states,
            policy: &self.policy,
            now: inputs.now,
            sessions: self.sessions.as_ref(),
            reserved: &reserved,
        };
        Ok(evaluate(&snapshot, identity, request))
    }

    /// `network_mode` (`service:*`, `fixture_consumer`).
    pub fn network_mode(
        &mut self,
        caller: caller_role_t,
        identity: &runtime_identity_t,
    ) -> service_result_t<network_decision_t> {
        allowed(caller)?;
        if self.policy.emergency(identity.owner_uid) {
            return Ok(network_decision_t::offline(Some(
                crate::evaluate::deny_reason_t::emergency_restriction,
            )));
        }
        let inputs = match self.inputs(identity) {
            Ok(inputs) => inputs,
            Err(reason) => return Ok(network_decision_t::offline_unknown(reason)),
        };
        let reserved = BTreeSet::new();
        let snapshot = snapshot_t {
            view: &inputs.view,
            key_states: &inputs.key_states,
            policy: &self.policy,
            now: inputs.now,
            sessions: self.sessions.as_ref(),
            reserved: &reserved,
        };
        Ok(network_mode(&snapshot, identity))
    }

    /// `policy_status` (`service:*`, `trusted_client` for its own UID).
    pub fn policy_status(&self, caller: caller_role_t) -> service_result_t<policy_status_t> {
        let emergency = match caller {
            caller_role_t::service(_) => self.policy.emergency_global,
            caller_role_t::trusted_client { uid } => self.policy.emergency(uid),
            _ => return Err(service_error_t::ipc(ipc_code_t::role_denied)),
        };
        let config = self.registry.config();
        Ok(policy_status_t {
            policy_epoch: config.policy_epoch,
            min_valid_policy_epoch: config.min_valid_policy_epoch,
            emergency_restriction: emergency,
            rollback_protection: rollback_protection_t::none,
        })
    }
}
