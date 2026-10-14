//! Verification layer 8, current authorization (ADR 0007 §1): `evaluate` and
//! `network_mode` as pure functions of one consistent snapshot.
//!
//! The inputs are a registryd `lineage_head` view (one journal snapshot: heads,
//! states, authoritative subject, grant origins with their signing key's state,
//! lifecycle records and policy epochs), identityd's answer for each signing key (it
//! includes denials that are not yet durable), the mandatory policy, trusted now,
//! session liveness and one-shot reservations. A request is `eligible` only if every
//! rule holds; otherwise `denied` with a reason or `unknown`, which callers treat as
//! denied. Eligibility is never enforcement, and admission status (`pending`) is not
//! an input (rule 7).

use std::collections::{BTreeMap, BTreeSet};

use cham3_contract_format::hash::digest_t;
use cham3_contract_format::schema::{
    grant_t, invalidation_reason_t, lifetime_kind_t, random_id_t, resource_category_t,
};
use cham3_identityd::index::{user_state_t, utc_seconds};
use cham3_registryd::lineage::end_reason_t;
use cham3_registryd::record::lifecycle_record_t;
use cham3_registryd::registry::{grant_origin_t, lineage_view_t};

use crate::mandatory::{mandatory_policy_t, network_mode_t};

/// Longest age of a session liveness attestation (ADR 0007 §1 rule 5).
pub const MaxSessionAttestationSeconds: i64 = 5;

/// The runtime identity of the requesting application instance. In M2 it is a
/// fixture supplied by the trusted caller; M3 attributes it (ADR 0005 §1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct runtime_identity_t {
    pub owner_uid: u32,
    pub app_id: String,
    pub installation_id: random_id_t,
    pub package_identity: String,
    pub executable_sha256: digest_t,
    pub runtime_scope: Option<String>,
    /// The application session the request runs in, if any.
    pub session_id: Option<random_id_t>,
}

/// The access requested.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct resource_request_t {
    pub category: resource_category_t,
    pub resource_id: String,
    pub resource_owner_uid: u32,
    pub operation: String,
}

/// Why a request is denied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum deny_reason_t {
    /// A mandatory prohibition (higher-level prohibitions win, PROJECT §3.3).
    mandatory_policy,
    emergency_restriction,
    /// The user is contained, quarantined or deleted (ADR 0006 §§3–4, 7).
    user_state(user_state_t),
    /// The resource is owned by another user (M2 has no cross-user authorization).
    resource_ownership,
    /// The runtime identity is not the lineage's authoritative subject.
    unknown_identity,
    /// No grant of a live state covers the resource and operation.
    no_matching_grant,
    /// A lifecycle record ended the grant.
    grant_ended(end_reason_t),
    not_started,
    expired,
    /// The grant is bound to a session that is not this one or is not attested live.
    session_not_live,
    /// A one-shot grant with a reservation.
    consumed,
    /// The signing key of the grant's commit is invalidated as compromised.
    key_invalidated,
    /// The grant's commit is below the minimum valid policy epoch.
    epoch_invalid,
    /// Delegated grants are not admitted in M2-005; evaluation refuses them too.
    delegation_unsupported,
    /// A reservation was requested for a grant that is not one-shot (ADR 0007 §4).
    not_one_shot,
}

impl deny_reason_t {
    pub fn as_str(self) -> &'static str {
        match self {
            deny_reason_t::mandatory_policy => "mandatory_policy",
            deny_reason_t::emergency_restriction => "emergency_restriction",
            deny_reason_t::user_state(state) => state.as_str(),
            deny_reason_t::resource_ownership => "resource_ownership",
            deny_reason_t::unknown_identity => "unknown_identity",
            deny_reason_t::no_matching_grant => "no_matching_grant",
            deny_reason_t::grant_ended(reason) => reason.as_str(),
            deny_reason_t::not_started => "not_started",
            deny_reason_t::expired => "expired",
            deny_reason_t::session_not_live => "session_not_live",
            deny_reason_t::consumed => "consumed",
            deny_reason_t::key_invalidated => "key_invalidated",
            deny_reason_t::epoch_invalid => "epoch_invalid",
            deny_reason_t::delegation_unsupported => "delegation_unsupported",
            deny_reason_t::not_one_shot => "not_one_shot",
        }
    }
}

/// Why the outcome cannot be established (treated as denied).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum unknown_reason_t {
    registry_unavailable,
    identity_unavailable,
    clock_unavailable,
    /// The snapshot's parts do not agree (for example a grant without its origin).
    snapshot_inconsistent,
    /// More than one network grant is eligible (impossible after admission; ADR 0007
    /// §1 conservative evaluation).
    network_grant_conflict,
}

impl unknown_reason_t {
    pub fn as_str(self) -> &'static str {
        match self {
            unknown_reason_t::registry_unavailable => "registry_unavailable",
            unknown_reason_t::identity_unavailable => "identity_unavailable",
            unknown_reason_t::clock_unavailable => "clock_unavailable",
            unknown_reason_t::snapshot_inconsistent => "snapshot_inconsistent",
            unknown_reason_t::network_grant_conflict => "network_grant_conflict",
        }
    }
}

/// The outcome of `evaluate`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum decision_t {
    eligible {
        grant_id: random_id_t,
        commit_id: digest_t,
    },
    denied(deny_reason_t),
    unknown(unknown_reason_t),
}

/// The outcome of `network_mode` (eligibility only, never enforcement).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum network_decision_t {
    mode {
        mode: network_mode_t,
        grant_id: random_id_t,
    },
    /// No eligible network grant, or the mode is forbidden.
    offline(Option<deny_reason_t>),
    /// Offline because the outcome cannot be established.
    offline_unknown(unknown_reason_t),
}

/// A positive session-liveness attestation from the session source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct session_attestation_t {
    pub live: bool,
    /// Trusted time of the observation (UTC seconds).
    pub observed_at: i64,
}

/// Session liveness (ADR 0007 §6). Unknown liveness denies.
pub trait session_source_t {
    fn liveness(&self, session_id: &random_id_t) -> Option<session_attestation_t>;
}

/// identityd's current answer for one signing key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct key_state_t {
    pub invalidated: Option<invalidation_reason_t>,
    pub user_state: user_state_t,
}

/// Everything one evaluation reads.
pub struct snapshot_t<'inputs> {
    pub view: &'inputs lineage_view_t,
    /// identityd's answer for every origin key of the view.
    pub key_states: &'inputs BTreeMap<digest_t, key_state_t>,
    pub policy: &'inputs mandatory_policy_t,
    /// Trusted now (UTC seconds; ADR 0007 §5).
    pub now: i64,
    /// The observed-time counter now (ADR 0007 §8 option B).
    pub observed_now: u64,
    pub sessions: &'inputs dyn session_source_t,
    /// One-shot grants with a reservation (ADR 0007 §4; reservations are M2-006).
    pub reserved: &'inputs BTreeSet<random_id_t>,
}

fn user_state_denial(snapshot: &snapshot_t<'_>) -> Option<deny_reason_t> {
    let registry_state = snapshot.view.user_state;
    if registry_state != user_state_t::normal {
        return Some(deny_reason_t::user_state(registry_state));
    }
    snapshot
        .key_states
        .values()
        .map(|state| state.user_state)
        .find(|state| *state != user_state_t::normal)
        .map(deny_reason_t::user_state)
}

/// Rule 3: the runtime identity is exactly the lineage's authoritative subject.
fn subject_matches(snapshot: &snapshot_t<'_>, identity: &runtime_identity_t) -> Option<bool> {
    let subject = snapshot.view.authoritative_subject.as_ref()?;
    Some(
        subject.owner_uid == identity.owner_uid
            && subject.owner_user_id == snapshot.view.owner_user_id
            && subject.app_id == identity.app_id
            && subject.installation_id == identity.installation_id
            && subject.package_identity == identity.package_identity
            && subject.executable_sha256 == identity.executable_sha256
            && subject.runtime_scope == identity.runtime_scope,
    )
}

/// The grants of the view's current state (rule 2): persistent grants of the trunk
/// head, temporary-lifetime grants of the live temporary heads. Ascending grant id,
/// each once.
fn current_grants(view: &lineage_view_t) -> Vec<&grant_t> {
    let mut grants: BTreeMap<random_id_t, &grant_t> = BTreeMap::new();
    if let Some(trunk) = &view.trunk_head {
        for grant in &trunk.state.grants {
            if grant.lifetime.kind == lifetime_kind_t::persistent {
                grants.insert(grant.grant_id, grant);
            }
        }
    }
    for head in &view.temporary_heads {
        for grant in &head.state.grants {
            if grant.lifetime.kind != lifetime_kind_t::persistent {
                grants.insert(grant.grant_id, grant);
            }
        }
    }
    grants.into_values().collect()
}

/// Rules 4–6 for one grant that matches the request.
fn check_grant(
    snapshot: &snapshot_t<'_>,
    identity: &runtime_identity_t,
    grant: &grant_t,
) -> Result<digest_t, decision_t> {
    let denied = |reason| Err(decision_t::denied(reason));
    if let Some(reason) = snapshot.view.ended.get(&grant.grant_id) {
        return denied(deny_reason_t::grant_ended(*reason));
    }
    let Some(origin) = snapshot.view.origins.get(&grant.grant_id) else {
        return Err(decision_t::unknown(unknown_reason_t::snapshot_inconsistent));
    };
    check_origin(snapshot, origin)?;
    if grant.delegation.parent_grant_id.is_some() {
        return denied(deny_reason_t::delegation_unsupported);
    }
    let lifetime = &grant.lifetime;
    let start = utc_seconds(&lifetime.effective_start)
        .ok_or(decision_t::unknown(unknown_reason_t::snapshot_inconsistent))?;
    if snapshot.now < start {
        return denied(deny_reason_t::not_started);
    }
    // ADR 0007 §8 option B: a start later than the admission is reached only once at
    // least that much observed time has passed since the admission, so a forward
    // clock error accepted after admission cannot advance it.
    if let Some(needed) = start
        .checked_sub(origin.admitted.trusted)
        .filter(|needed| *needed > 0)
    {
        let elapsed = snapshot
            .observed_now
            .checked_sub(origin.admitted.observed)
            .and_then(|elapsed| i64::try_from(elapsed).ok());
        if elapsed.is_none_or(|elapsed| elapsed < needed) {
            return denied(deny_reason_t::not_started);
        }
    }
    if let Some(expiration) = &lifetime.expiration {
        let end = utc_seconds(expiration)
            .ok_or(decision_t::unknown(unknown_reason_t::snapshot_inconsistent))?;
        if snapshot.now >= end {
            return denied(deny_reason_t::expired);
        }
    }
    if let Some(session_id) = &lifetime.session_id {
        // A durable session end dominates every attestation, however fresh (ADR 0007
        // §6; M2-005 review R02).
        if snapshot.view.ended_sessions.contains(session_id) {
            return denied(deny_reason_t::grant_ended(end_reason_t::session_ended));
        }
        let bound = identity.session_id.as_ref() == Some(session_id);
        let live = snapshot
            .sessions
            .liveness(session_id)
            .is_some_and(|attestation| {
                attestation.live
                    && attestation.observed_at <= snapshot.now
                    && snapshot
                        .now
                        .checked_sub(attestation.observed_at)
                        .is_some_and(|age| age <= MaxSessionAttestationSeconds)
            });
        if !bound || !live {
            return denied(deny_reason_t::session_not_live);
        }
    }
    if lifetime.kind == lifetime_kind_t::one_shot && snapshot.reserved.contains(&grant.grant_id) {
        return denied(deny_reason_t::consumed);
    }
    Ok(origin.commit_id)
}

/// Rule 4: the signing key of the commit that added the grant, and its epoch.
fn check_origin(snapshot: &snapshot_t<'_>, origin: &grant_origin_t) -> Result<(), decision_t> {
    if origin.key_unknown {
        return Err(decision_t::unknown(unknown_reason_t::snapshot_inconsistent));
    }
    let Some(current) = snapshot.key_states.get(&origin.key_id) else {
        return Err(decision_t::unknown(unknown_reason_t::identity_unavailable));
    };
    let compromised = Some(invalidation_reason_t::compromised);
    if origin.key_invalidated == compromised || current.invalidated == compromised {
        return Err(decision_t::denied(deny_reason_t::key_invalidated));
    }
    if origin.policy_epoch < snapshot.view.min_valid_policy_epoch {
        return Err(decision_t::denied(deny_reason_t::epoch_invalid));
    }
    Ok(())
}

/// Rules shared by `evaluate` and `network_mode` before any grant: user state,
/// emergency restriction, the lineage and its subject.
fn check_subject(
    snapshot: &snapshot_t<'_>,
    identity: &runtime_identity_t,
) -> Result<(), decision_t> {
    if let Some(reason) = user_state_denial(snapshot) {
        return Err(decision_t::denied(reason));
    }
    // policyd's own overlay (configuration) and registryd's persisted restriction of
    // everyone (ADR 0007 §1 rule 1; set through the restrictive path, M2-006 stage A).
    if snapshot.policy.emergency(identity.owner_uid) || snapshot.view.emergency_restriction {
        return Err(decision_t::denied(deny_reason_t::emergency_restriction));
    }
    if snapshot.view.owner_uid != identity.owner_uid {
        return Err(decision_t::unknown(unknown_reason_t::snapshot_inconsistent));
    }
    match subject_matches(snapshot, identity) {
        None => Err(decision_t::denied(deny_reason_t::no_matching_grant)),
        Some(false) => Err(decision_t::denied(deny_reason_t::unknown_identity)),
        Some(true) => Ok(()),
    }
}

/// `evaluate` (ADR 0007 §1).
pub fn evaluate(
    snapshot: &snapshot_t<'_>,
    identity: &runtime_identity_t,
    request: &resource_request_t,
) -> decision_t {
    evaluate_matching(snapshot, identity, request, None)
}

/// `evaluate` restricted to the grant `grant_id`, for a one-shot reservation (ADR
/// 0007 §4, erratum E7.1): every rule of §1 for that grant alone, and the grant must
/// be one-shot (`not_one_shot` otherwise).
pub fn evaluate_grant(
    snapshot: &snapshot_t<'_>,
    identity: &runtime_identity_t,
    request: &resource_request_t,
    grant_id: &random_id_t,
) -> decision_t {
    let one_shot = current_grants(snapshot.view)
        .into_iter()
        .find(|grant| grant.grant_id == *grant_id)
        .map(|grant| grant.lifetime.kind == lifetime_kind_t::one_shot);
    match evaluate_matching(snapshot, identity, request, Some(grant_id)) {
        decision_t::eligible { .. } if one_shot != Some(true) => {
            decision_t::denied(deny_reason_t::not_one_shot)
        }
        decision => decision,
    }
}

/// Current grants of the view that trusted now shows expired and that no record
/// ends yet: the expiry latch (ADR 0007 §5 rule 5) records each of them.
pub fn unlatched_expirations(snapshot: &snapshot_t<'_>) -> Vec<random_id_t> {
    current_grants(snapshot.view)
        .into_iter()
        .filter(|grant| !snapshot.view.ended.contains_key(&grant.grant_id))
        .filter(|grant| {
            grant
                .lifetime
                .expiration
                .as_ref()
                .and_then(|expiration| utc_seconds(expiration))
                .is_some_and(|end| snapshot.now >= end)
        })
        .map(|grant| grant.grant_id)
        .collect()
}

/// A runtime identity bound to the lineage (same owner, application and
/// installation) whose package identity or executable hash is not the authoritative
/// subject's (ADR 0007 §3, erratum E7.4; ADR 0008 §7): the `identity_mismatch`
/// evidence to report. `None` when the identity matches or is not this lineage's.
pub fn identity_mismatch(
    snapshot: &snapshot_t<'_>,
    identity: &runtime_identity_t,
) -> Option<lifecycle_record_t> {
    let subject = snapshot.view.authoritative_subject.as_ref()?;
    let bound = subject.owner_uid == identity.owner_uid
        && subject.owner_user_id == snapshot.view.owner_user_id
        && subject.app_id == identity.app_id
        && subject.installation_id == identity.installation_id;
    let changed = subject.package_identity != identity.package_identity
        || subject.executable_sha256 != identity.executable_sha256;
    (bound && changed).then(|| lifecycle_record_t::identity_mismatch {
        lineage_id: snapshot.view.lineage_id,
        package_identity: identity.package_identity.clone(),
        executable_sha256: identity.executable_sha256,
    })
}

fn evaluate_matching(
    snapshot: &snapshot_t<'_>,
    identity: &runtime_identity_t,
    request: &resource_request_t,
    only: Option<&random_id_t>,
) -> decision_t {
    if snapshot.policy.forbids(
        identity.owner_uid,
        request.category,
        &request.resource_id,
        &request.operation,
    ) {
        return decision_t::denied(deny_reason_t::mandatory_policy);
    }
    if let Err(decision) = check_subject(snapshot, identity) {
        return decision;
    }
    if request.resource_owner_uid != identity.owner_uid {
        return decision_t::denied(deny_reason_t::resource_ownership);
    }
    let matching: Vec<&grant_t> = current_grants(snapshot.view)
        .into_iter()
        .filter(|grant| only.is_none_or(|grant_id| grant.grant_id == *grant_id))
        .filter(|grant| {
            grant.resource.category == request.category
                && grant.resource.resource_id == request.resource_id
                && grant.resource.resource_owner_uid == request.resource_owner_uid
                && grant.resource.operations.contains(&request.operation)
        })
        .collect();
    let mut first_refusal = None;
    for grant in matching {
        match check_grant(snapshot, identity, grant) {
            Ok(commit_id) => {
                return decision_t::eligible {
                    grant_id: grant.grant_id,
                    commit_id,
                };
            }
            Err(decision) => {
                first_refusal.get_or_insert(decision);
            }
        }
    }
    first_refusal.unwrap_or(decision_t::denied(deny_reason_t::no_matching_grant))
}

/// `network_mode` (ADR 0007 §1, wire format §§6.10, 6.12): the operation of the
/// lineage's single eligible network grant; Offline with none, when the mode is
/// forbidden, or when more than one is eligible.
pub fn network_mode(
    snapshot: &snapshot_t<'_>,
    identity: &runtime_identity_t,
) -> network_decision_t {
    match check_subject(snapshot, identity) {
        Ok(()) => {}
        Err(decision_t::unknown(reason)) => return network_decision_t::offline_unknown(reason),
        Err(decision_t::denied(reason)) => return network_decision_t::offline(Some(reason)),
        Err(decision_t::eligible { .. }) => {
            return network_decision_t::offline_unknown(unknown_reason_t::snapshot_inconsistent);
        }
    }
    let mut eligible = Vec::new();
    for grant in current_grants(snapshot.view) {
        let is_network = grant.resource.category == resource_category_t::network
            && grant.resource.resource_owner_uid == identity.owner_uid;
        if !is_network {
            continue;
        }
        match check_grant(snapshot, identity, grant) {
            Ok(_) => eligible.push(grant),
            Err(decision_t::unknown(reason)) => return network_decision_t::offline_unknown(reason),
            Err(_) => {}
        }
    }
    let [grant] = eligible.as_slice() else {
        return if eligible.is_empty() {
            network_decision_t::offline(None)
        } else {
            network_decision_t::offline_unknown(unknown_reason_t::network_grant_conflict)
        };
    };
    let operation = grant.resource.operations.first().map(String::as_str);
    let Some(mode) = operation.and_then(network_mode_t::parse) else {
        return network_decision_t::offline_unknown(unknown_reason_t::snapshot_inconsistent);
    };
    if snapshot.policy.forbids(
        identity.owner_uid,
        resource_category_t::network,
        &grant.resource.resource_id,
        mode.as_str(),
    ) {
        return network_decision_t::offline(Some(deny_reason_t::mandatory_policy));
    }
    network_decision_t::mode {
        mode,
        grant_id: grant.grant_id,
    }
}
