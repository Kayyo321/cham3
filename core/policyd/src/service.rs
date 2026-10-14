//! policyd's operations (service IPC §3): `evaluate`, `network_mode` and
//! `policy_status`.
//!
//! Each evaluation reads one registryd `lineage_head` snapshot, asks identityd for
//! every signing key that snapshot names (so a denial that is not yet durable still
//! denies), reads trusted now and session liveness, and calls the pure functions of
//! [`crate::evaluate`]. Anything that cannot be read makes the outcome `unknown`
//! (denied) or Offline.
//!
//! **Trusted time (ADR 0007 §5 rule 4, §8 option B; M2-006 stage B1).** policyd
//! evaluates with `max(own offset, offset of the snapshot)`, from its own
//! `(realtime, boottime)` sample ([`policy_clock_t`]). A sample ahead of that is
//! reported to registryd (`time_sample`), and policyd relies on the increase only
//! after the `ok` reply. The observed counter comes from the snapshot's offset. A
//! snapshot of another boot, or a sample that cannot be taken, makes the outcome
//! `unknown` (`clock_unavailable`).
//!
//! **Lifecycle evidence (ADR 0007 §§3–5; erratum E7; stage B2).** Every evaluation
//! and network-mode query that read a snapshot reports what it observed through
//! registryd's `restrict`: the expiry latch for each current grant that trusted now
//! shows expired and no record ends yet (§5 rule 5), and an `identity_mismatch` for
//! a runtime identity whose package identity or executable hash is not the
//! lineage's subject (E7.4). A failed report changes no decision; the next
//! evaluation retries it. `reserve_use` (E7.1) evaluates the request against the
//! one-shot grant completely and only then asks registryd to reserve it, which
//! re-checks its own rules under the user lock.
//!
//! Limits: in-process composition; the transport is M2-007. Reservations reach
//! evaluation as grants that a `use_reserved` record ended (`consumed`), so the
//! snapshot's separate reserved set stays empty. The session source is a test
//! stand-in until sessiond exists.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;

use cham3_contract_format::hash::{digest_t, lineage_id};
use cham3_contract_format::schema::{random_id_t, subject_t};
use cham3_identityd::caller::{caller_role_t, service_name_t};
use cham3_identityd::index::key_resolution_t;
use cham3_identityd::journal::identity_journal_t;
use cham3_identityd::outcome::{ipc, ipc_code_t, service_error_t, service_result_t};
use cham3_identityd::service::{identity_service_t, user_identity_t};
use cham3_registryd::anchor::anchor_mode_t;
use cham3_registryd::clock::{time_source_t, time_view_t};
use cham3_registryd::record::lifecycle_record_t;
use cham3_registryd::registry::{lineage_view_t, registry_t};

use crate::evaluate::{
    decision_t, evaluate, evaluate_grant, identity_mismatch, key_state_t, network_decision_t,
    network_mode, resource_request_t, runtime_identity_t, session_source_t, snapshot_t,
    unknown_reason_t, unlatched_expirations,
};
use crate::mandatory::mandatory_policy_t;

const Policyd: caller_role_t = caller_role_t::service(service_name_t::policyd);

/// policyd's trusted time (ADR 0007 §5 rule 4): its own offset in this boot, only
/// ever raised.
pub struct policy_clock_t {
    source: Box<dyn time_source_t>,
    boot_id: String,
    offset: Option<i64>,
}

impl policy_clock_t {
    /// A clock over `source` in the boot `boot_id` (the kernel's boot id in
    /// deployment).
    pub fn new(source: Box<dyn time_source_t>, boot_id: &str) -> policy_clock_t {
        policy_clock_t {
            source,
            boot_id: boot_id.to_owned(),
            offset: None,
        }
    }

    /// policyd's own offset (diagnostics and tests).
    pub fn offset(&self) -> Option<i64> {
        self.offset
    }
}

/// Trusted and observed now for one evaluation.
struct evaluation_time_t {
    now: i64,
    observed_now: u64,
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

/// The outcome of `reserve_use` (ADR 0007 §4, erratum E7.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum reserve_outcome_t {
    /// registryd reserved the grant: it is consumed, whatever the outcome.
    reserved {
        grant_id: random_id_t,
        commit_id: digest_t,
        reservation_id: random_id_t,
        journal_seq: u64,
    },
    /// The evaluation did not find the grant eligible; nothing is reserved.
    refused(decision_t),
}

/// `policy_status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct policy_status_t {
    /// The persisted policy state (registryd's anchor; raised by configuration, never
    /// lowered).
    pub policy_epoch: u64,
    pub min_valid_policy_epoch: u64,
    /// The persisted emergency restriction of everyone, or policyd's configured
    /// overlay for everyone (`service:*`) or for the caller's own UID.
    pub emergency_restriction: bool,
    /// The anchor's rollback-protection mode (ADR 0006 §2, owner choice §9.1).
    pub rollback_protection: rollback_protection_t,
}

/// Rollback-protection mode reported by `policy_status` (ADR 0006 §2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum rollback_protection_t {
    /// The anchor shares the store's filesystem: partial restores, torn tails and
    /// journal substitution are detected, a whole-volume restore is not.
    degraded,
    /// The anchor is on another filesystem.
    independent,
}

/// policyd.
pub struct policy_service_t<identity_link_impl_t: policy_identity_link_t> {
    registry: Arc<registry_t>,
    identity: identity_link_impl_t,
    policy: mandatory_policy_t,
    clock: policy_clock_t,
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
    observed_now: u64,
}

impl<identity_link_impl_t: policy_identity_link_t> policy_service_t<identity_link_impl_t> {
    pub fn new(
        registry: Arc<registry_t>,
        identity: identity_link_impl_t,
        policy: mandatory_policy_t,
        clock: policy_clock_t,
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

    pub fn clock(&self) -> &policy_clock_t {
        &self.clock
    }

    /// Trusted and observed now against one snapshot's shared offsets (ADR 0007 §5
    /// rule 4, §8 option B).
    fn evaluation_time(
        &mut self,
        shared: &time_view_t,
    ) -> Result<evaluation_time_t, unknown_reason_t> {
        let unavailable = unknown_reason_t::clock_unavailable;
        if shared.boot_id != self.clock.boot_id {
            return Err(unavailable);
        }
        let sample = self.clock.source.sample().ok_or(unavailable)?;
        let mut offset = self
            .clock
            .offset
            .map_or(shared.offset, |own| own.max(shared.offset));
        let sample_offset = i64::try_from(sample.realtime)
            .ok()
            .zip(i64::try_from(sample.boottime).ok())
            .and_then(|(realtime, boottime)| realtime.checked_sub(boottime))
            .ok_or(unavailable)?;
        if sample_offset > offset {
            // Reported first; relied on only after registryd ledgered it. A refused
            // report decides nothing from the snapshot read before it (M2-006-B1-R01):
            // a ledger that failed meanwhile serves no eligible decision (ADR 0006 §3),
            // and a sample registryd cannot take means the services disagree on the
            // clock. Either way the outcome is unknown (denied; Offline).
            let reply = self
                .registry
                .time_sample(
                    Policyd,
                    &self.clock.boot_id,
                    sample.boottime,
                    sample.realtime,
                )
                .map_err(|error| {
                    if error.code() == ipc_code_t::malformed_request.as_str() {
                        unavailable
                    } else {
                        unknown_reason_t::registry_unavailable
                    }
                })?;
            offset = offset.max(sample_offset).max(reply.offset);
        }
        self.clock.offset = Some(offset);
        let now = i64::try_from(sample.boottime)
            .ok()
            .and_then(|boottime| boottime.checked_add(offset))
            .ok_or(unavailable)?;
        let observed_now = shared.observed_at(sample.boottime).ok_or(unavailable)?;
        Ok(evaluation_time_t { now, observed_now })
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
        let time = self.evaluation_time(&view.time)?;
        Ok(inputs_t {
            view,
            key_states,
            now: time.now,
            observed_now: time.observed_now,
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
            observed_now: inputs.observed_now,
            sessions: self.sessions.as_ref(),
            reserved: &reserved,
        };
        let decision = evaluate(&snapshot, identity, request);
        self.report(&snapshot, identity);
        Ok(decision)
    }

    /// The evidence one evaluation owes (ADR 0007 §5 rule 5; erratum E7.4), through
    /// registryd's `restrict`: the expiry latch for every current grant that trusted
    /// now shows expired and no record ends yet, and an identity mismatch. A failed
    /// report changes no decision (the grant is denied as expired either way); the
    /// next evaluation retries it.
    fn report(&self, snapshot: &snapshot_t<'_>, identity: &runtime_identity_t) {
        for grant_id in unlatched_expirations(snapshot) {
            let _ = self.registry.restrict(
                Policyd,
                identity.owner_uid,
                &lifecycle_record_t::expired { grant_id },
            );
        }
        if let Some(record) = identity_mismatch(snapshot, identity) {
            let _ = self.registry.restrict(Policyd, identity.owner_uid, &record);
        }
    }

    /// `reserve_use` (`fixture_consumer`, later brokers; ADR 0007 §4 option A,
    /// erratum E7.1): evaluates the request completely against the one-shot grant
    /// `grant_id` (mandatory policy, subject, session liveness and every other rule
    /// of §1); only an eligible grant is reserved, by registryd, which re-checks the
    /// rules its own snapshot decides under the user lock and writes `use_reserved`
    /// before it replies. registryd's refusal is the error: `restriction_not_durable`
    /// means the grant is consumed and the use denied.
    pub fn reserve_use(
        &mut self,
        caller: caller_role_t,
        identity: &runtime_identity_t,
        request: &resource_request_t,
        grant_id: &random_id_t,
        operation_id: &random_id_t,
    ) -> service_result_t<reserve_outcome_t> {
        if caller != caller_role_t::fixture_consumer {
            return Err(ipc(ipc_code_t::role_denied));
        }
        let refused = |decision| Ok(reserve_outcome_t::refused(decision));
        if self.policy.forbids(
            identity.owner_uid,
            request.category,
            &request.resource_id,
            &request.operation,
        ) {
            return refused(decision_t::denied(
                crate::evaluate::deny_reason_t::mandatory_policy,
            ));
        }
        if self.policy.emergency(identity.owner_uid) {
            return refused(decision_t::denied(
                crate::evaluate::deny_reason_t::emergency_restriction,
            ));
        }
        let inputs = match self.inputs(identity) {
            Ok(inputs) => inputs,
            Err(reason) => return refused(decision_t::unknown(reason)),
        };
        let reserved = BTreeSet::new();
        let snapshot = snapshot_t {
            view: &inputs.view,
            key_states: &inputs.key_states,
            policy: &self.policy,
            now: inputs.now,
            observed_now: inputs.observed_now,
            sessions: self.sessions.as_ref(),
            reserved: &reserved,
        };
        let decision = evaluate_grant(&snapshot, identity, request, grant_id);
        self.report(&snapshot, identity);
        let decision_t::eligible {
            grant_id: eligible,
            commit_id,
        } = decision
        else {
            return refused(decision);
        };
        let subject = subject_t {
            owner_uid: identity.owner_uid,
            owner_user_id: inputs.view.owner_user_id,
            app_id: identity.app_id.clone(),
            installation_id: identity.installation_id,
            package_identity: identity.package_identity.clone(),
            executable_sha256: identity.executable_sha256,
            runtime_scope: identity.runtime_scope.clone(),
        };
        let receipt = self.registry.reserve_use(
            Policyd,
            identity.owner_uid,
            &eligible,
            &subject,
            operation_id,
        )?;
        Ok(reserve_outcome_t::reserved {
            grant_id: eligible,
            commit_id,
            reservation_id: receipt.reservation_id,
            journal_seq: receipt.journal_seq,
        })
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
            observed_now: inputs.observed_now,
            sessions: self.sessions.as_ref(),
            reserved: &reserved,
        };
        let decision = network_mode(&snapshot, identity);
        self.report(&snapshot, identity);
        Ok(decision)
    }

    /// `policy_status` (`service:*`, `trusted_client` for its own UID).
    pub fn policy_status(&self, caller: caller_role_t) -> service_result_t<policy_status_t> {
        let overlay = match caller {
            caller_role_t::service(_) => self.policy.emergency_global,
            caller_role_t::trusted_client { uid } => self.policy.emergency(uid),
            _ => return Err(service_error_t::ipc(ipc_code_t::role_denied)),
        };
        let state = self.registry.policy_state()?;
        let anchor_mode = self.registry.rollback_status(Policyd)?.anchor_mode;
        Ok(policy_status_t {
            policy_epoch: state.epoch,
            min_valid_policy_epoch: state.min_valid_epoch,
            emergency_restriction: overlay || state.emergency_restriction,
            rollback_protection: match anchor_mode {
                anchor_mode_t::degraded => rollback_protection_t::degraded,
                anchor_mode_t::independent => rollback_protection_t::independent,
            },
        })
    }

    /// `set_policy_epoch` (`admin`): raises the policy epoch and the minimum valid
    /// epoch through registryd's restrictive path (ADR 0006 §3; ADR 0007 §1 rule 4).
    /// Neither value ever goes down.
    pub fn set_policy_epoch(
        &self,
        caller: caller_role_t,
        epoch: u64,
        min_valid_epoch: u64,
    ) -> service_result_t<policy_status_t> {
        if caller != caller_role_t::admin {
            return Err(ipc(ipc_code_t::role_denied));
        }
        self.registry
            .set_policy_epoch(caller, epoch, min_valid_epoch)?;
        self.policy_status(Policyd)
    }

    /// `set_emergency_restriction` (`admin`): sets or releases the persisted emergency
    /// restriction of everyone (ADR 0007 §1 rule 1) through registryd. Releasing it
    /// restores no grant by itself.
    pub fn set_emergency_restriction(
        &self,
        caller: caller_role_t,
        restricted: bool,
    ) -> service_result_t<policy_status_t> {
        if caller != caller_role_t::admin {
            return Err(ipc(ipc_code_t::role_denied));
        }
        self.registry
            .set_emergency_restriction(caller, restricted)?;
        self.policy_status(Policyd)
    }
}
