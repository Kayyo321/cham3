//! One user's ECR index and verification layer 7, admission (wire format §§6.5–6.12,
//! §7; ADR 0006 §1, ADR 0007 §1).
//!
//! The index is derived, never stored: replay rebuilds it from the journal. It holds
//! every admitted commit, each lineage's trunk head, live temporary branch heads and
//! authoritative subject, the user's grant ids and request ids, the grant ids ended
//! by narrowing lifecycle records and the session ids those records ended (an ended
//! session never becomes live again, ADR 0007 §6: no later grant may be bound to it;
//! M2-005 review R02).
//!
//! [`lineage_index_t::check_commit`] is a pure function of the index and a commit that
//! passed layers 1–6; it reports the first failure in wire format §7 order. Nothing
//! changes until [`lineage_index_t::apply_commit`] runs, which the registry calls only
//! after the commit's record is durable. The decisions mirror the independent oracle's
//! journal model (tests/m2/oracle/ecr_oracle.py `journal_t`), which the published
//! journal scenarios and admission vectors check. Rules the model does not state, and
//! that this index adds, are marked "(beyond the model)".

use std::collections::{BTreeMap, BTreeSet};

use cham3_contract_format::dcbor::encode;
use cham3_contract_format::hash::digest_t;
use cham3_contract_format::schema::{
    commit_payload_t, commit_type_t, contract_state_t, grant_t, lifetime_kind_t, random_id_t,
    subject_t,
};
use cham3_identityd::outcome::{rule, rule_code_t, service_result_t};

use cham3_identityd::index::{user_record_t, utc_seconds};

use crate::clock::timestamp_acceptable;

use crate::record::{lifecycle_record_t, placement_t};

/// Why a grant id ended: the lifecycle records of M2-005, and the user-state records
/// of ADR 0006 §§3, 4 and 7 (the oracle's reasons; consumption is M2-006 stage B).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum end_reason_t {
    expired,
    session_ended,
    restriction,
    /// Containment: admitted before an unverifiable run's restart point, permanently.
    suspended,
    /// Quarantine of the user.
    quarantined,
    /// Deletion of the user.
    deleted,
}

impl end_reason_t {
    pub fn as_str(self) -> &'static str {
        match self {
            end_reason_t::expired => "expired",
            end_reason_t::session_ended => "session_ended",
            end_reason_t::restriction => "restriction",
            end_reason_t::suspended => "suspended",
            end_reason_t::quarantined => "quarantined",
            end_reason_t::deleted => "deleted",
        }
    }
}

/// The standing of a grant id (the oracle's `standing`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum standing_t {
    /// In a live state (trunk head or live temporary head) and not ended.
    current,
    /// A lifecycle record ended it.
    ended(end_reason_t),
    /// Admitted, but removed, superseded or on an ended branch.
    inactive,
    /// Never admitted in this journal.
    unknown,
}

/// Admission status of a commit. Confirmation per enforcement domain is M2-006;
/// M2-005 admits every commit as `pending`, and policyd never uses the status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum commit_status_t {
    pending,
}

/// When a commit was admitted: the trusted time (its record's `time`) and the
/// observed-time counter (ADR 0007 §8 option B).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct admitted_at_t {
    pub trusted: i64,
    pub observed: u64,
}

/// An admitted commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct commit_entry_t {
    pub payload: Box<commit_payload_t>,
    pub envelope: Vec<u8>,
    pub sequence: u64,
    pub placement: placement_t,
    pub status: commit_status_t,
    pub admitted: admitted_at_t,
}

/// One lineage.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct lineage_t {
    pub trunk_head: Option<digest_t>,
    /// Live temporary branch heads → grant ids added on that branch. Any admitted
    /// trunk commit ends every branch (wire format §6.11).
    pub branches: BTreeMap<digest_t, BTreeSet<random_id_t>>,
    /// Subject of the genesis, replaced only by identity transitions (§6.9).
    pub authoritative_subject: Option<subject_t>,
    /// The trunk head at the last containment or quarantine (ADR 0006 §10 step 1:
    /// the `restore_reference` of a recovery commit).
    pub containment_head: Option<digest_t>,
    /// A recovery commit re-authorized this lineage in the open ceremony.
    pub recovered: bool,
}

/// Where a grant came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct grant_entry_t {
    pub grant: grant_t,
    pub lineage_id: digest_t,
    /// The commit that added it (its signed contract, wire format §6.8).
    pub commit_id: digest_t,
}

/// The decision of a successful [`lineage_index_t::check_commit`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct admission_t {
    pub commit_id: digest_t,
    pub lineage_id: digest_t,
    pub placement: placement_t,
    /// The live temporary head a branch commit extends (`None` for a new branch).
    branch_head: Option<digest_t>,
    /// Whether the commit starts a new lineage.
    pub new_lineage: bool,
}

/// One user's ECR index.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct lineage_index_t {
    commits: BTreeMap<digest_t, commit_entry_t>,
    lineages: BTreeMap<digest_t, lineage_t>,
    requests: BTreeSet<random_id_t>,
    grants: BTreeMap<random_id_t, grant_entry_t>,
    ended: BTreeMap<random_id_t, end_reason_t>,
    /// Sessions a durable `session_ended` record ended (ADR 0007 §6).
    ended_sessions: BTreeSet<random_id_t>,
}

fn reject<ok_t>(code: rule_code_t) -> service_result_t<ok_t> {
    Err(rule(code))
}

/// `base` minus the removed ids plus the added grants (wire format §6.5 items 2–3).
fn apply_delta(
    base: &contract_state_t,
    payload: &commit_payload_t,
) -> service_result_t<contract_state_t> {
    let mut grants: BTreeMap<random_id_t, grant_t> = base
        .grants
        .iter()
        .map(|grant| (grant.grant_id, grant.clone()))
        .collect();
    for removed in &payload.delta.remove_grant_ids {
        if grants.remove(removed).is_none() {
            return reject(rule_code_t::delta_rule);
        }
    }
    for grant in &payload.delta.add_grants {
        if grants.insert(grant.grant_id, grant.clone()).is_some() {
            return reject(rule_code_t::delta_rule);
        }
    }
    Ok(contract_state_t {
        grants: grants.into_values().collect(),
    })
}

/// Subject fields other than the package identity and executable hash.
fn same_binding(first: &subject_t, second: &subject_t) -> bool {
    first.owner_uid == second.owner_uid
        && first.owner_user_id == second.owner_user_id
        && first.app_id == second.app_id
        && first.installation_id == second.installation_id
        && first.runtime_scope == second.runtime_scope
}

impl lineage_index_t {
    pub fn new() -> lineage_index_t {
        lineage_index_t::default()
    }

    pub fn commit(&self, commit_id: &digest_t) -> Option<&commit_entry_t> {
        self.commits.get(commit_id)
    }

    pub fn lineage(&self, lineage_id: &digest_t) -> Option<&lineage_t> {
        self.lineages.get(lineage_id)
    }

    pub fn lineage_count(&self) -> usize {
        self.lineages.len()
    }

    pub fn grant(&self, grant_id: &random_id_t) -> Option<&grant_entry_t> {
        self.grants.get(grant_id)
    }

    pub fn ended(&self, grant_id: &random_id_t) -> Option<end_reason_t> {
        self.ended.get(grant_id).copied()
    }

    pub fn has_request(&self, request_id: &random_id_t) -> bool {
        self.requests.contains(request_id)
    }

    /// Whether a durable `session_ended` record ended this session (ADR 0007 §6).
    pub fn session_ended(&self, session_id: &random_id_t) -> bool {
        self.ended_sessions.contains(session_id)
    }

    /// States of the trunk head and every live temporary head of a lineage.
    pub fn live_states(&self, lineage: &lineage_t) -> Vec<(digest_t, &contract_state_t)> {
        lineage
            .trunk_head
            .iter()
            .chain(lineage.branches.keys())
            .filter_map(|head| {
                self.commits
                    .get(head)
                    .map(|entry| (*head, &entry.payload.result_state))
            })
            .collect()
    }

    pub fn standing(&self, grant_id: &random_id_t) -> standing_t {
        if let Some(reason) = self.ended.get(grant_id) {
            return standing_t::ended(*reason);
        }
        let Some(entry) = self.grants.get(grant_id) else {
            return standing_t::unknown;
        };
        let Some(lineage) = self.lineages.get(&entry.lineage_id) else {
            return standing_t::unknown;
        };
        let current = self
            .live_states(lineage)
            .iter()
            .any(|(_, state)| state.grants.iter().any(|grant| grant.grant_id == *grant_id));
        if current {
            standing_t::current
        } else {
            standing_t::inactive
        }
    }

    /// Network grants occupying the lineage's single network slot in `states`
    /// (wire format §6.12): network grants no lifecycle record has ended.
    pub fn occupying_network_grants<'state>(
        &self,
        states: impl IntoIterator<Item = &'state contract_state_t>,
    ) -> BTreeSet<random_id_t> {
        states
            .into_iter()
            .flat_map(|state| state.grants.iter())
            .filter(|grant| grant.is_network() && !self.ended.contains_key(&grant.grant_id))
            .map(|grant| grant.grant_id)
            .collect()
    }

    fn ancestors(&self, commit_id: &digest_t) -> BTreeSet<digest_t> {
        let mut seen = BTreeSet::new();
        let mut pending = vec![*commit_id];
        while let Some(next) = pending.pop() {
            if let Some(entry) = self.commits.get(&next) {
                for parent in &entry.payload.parents {
                    if seen.insert(*parent) {
                        pending.push(*parent);
                    }
                }
            }
        }
        seen
    }

    /// Wire format §6.11: trunk, or branch with the head it extends.
    fn placement(
        &self,
        lineage: &lineage_t,
        payload: &commit_payload_t,
    ) -> service_result_t<(placement_t, Option<digest_t>)> {
        let commit_type = payload.commit_type;
        if commit_type == commit_type_t::genesis {
            if lineage.trunk_head.is_some() {
                return reject(rule_code_t::stale_head);
            }
            return Ok((placement_t::trunk, None));
        }
        if commit_type == commit_type_t::merge {
            let merge_base = payload.merge_base;
            let others_live = payload
                .parents
                .iter()
                .filter(|parent| Some(**parent) != merge_base)
                .all(|parent| lineage.branches.contains_key(parent));
            if merge_base.is_none() || merge_base != lineage.trunk_head || !others_live {
                return reject(rule_code_t::stale_head);
            }
            return Ok((placement_t::trunk, None));
        }
        let Some(parent) = payload.parents.first() else {
            return reject(rule_code_t::stale_head);
        };
        if Some(*parent) == lineage.trunk_head {
            if commit_type == commit_type_t::temporary_grant {
                return Ok((placement_t::temporary, None));
            }
            return Ok((placement_t::trunk, None));
        }
        let extends = matches!(
            commit_type,
            commit_type_t::temporary_grant | commit_type_t::revocation
        );
        if lineage.branches.contains_key(parent) && extends {
            return Ok((placement_t::temporary, Some(*parent)));
        }
        reject(rule_code_t::stale_head)
    }

    /// Layer 7 for a commit that passed layers 1–6, in wire format §7 order. Pure:
    /// the index is unchanged. `trusted_now` is the admission's trusted time: the
    /// commit-timestamp rule (ADR 0007 §5, erratum E4: `timestamp_rule`) is checked
    /// against it once the parents and the placement are known and before the state
    /// checks. Replay passes `None`: the rule is admission-only, and replay reproduces
    /// historical admissions.
    pub fn check_commit(
        &self,
        commit_id: &digest_t,
        payload: &commit_payload_t,
        trusted_now: Option<i64>,
    ) -> service_result_t<admission_t> {
        for parent in &payload.parents {
            let Some(entry) = self.commits.get(parent) else {
                return reject(rule_code_t::parent_unknown);
            };
            if entry.payload.lineage_id != payload.lineage_id {
                return reject(rule_code_t::lineage_mismatch);
            }
        }
        let existing = self.lineages.get(&payload.lineage_id);
        let empty_lineage = lineage_t::default();
        let lineage = existing.unwrap_or(&empty_lineage);
        let (placement, branch_head) = self.placement(lineage, payload)?;
        if let Some(now) = trusted_now {
            let timestamp =
                utc_seconds(&payload.timestamp).ok_or(rule(rule_code_t::timestamp_rule))?;
            let parents = payload
                .parents
                .iter()
                .map(|parent| {
                    self.commits
                        .get(parent)
                        .and_then(|entry| utc_seconds(&entry.payload.timestamp))
                })
                .collect::<Option<Vec<i64>>>()
                .ok_or(rule(rule_code_t::timestamp_rule))?;
            if !timestamp_acceptable(timestamp, now, &parents) {
                return reject(rule_code_t::timestamp_rule);
            }
        }
        let empty_state = contract_state_t::default();
        let base = if payload.commit_type == commit_type_t::genesis {
            &empty_state
        } else {
            let base_id = payload
                .merge_base
                .or_else(|| payload.parents.first().copied())
                .ok_or(rule(rule_code_t::base_state_mismatch))?;
            &self
                .commits
                .get(&base_id)
                .ok_or(rule(rule_code_t::parent_unknown))?
                .payload
                .result_state
        };
        if base.state_hash() != payload.base_state_hash {
            return reject(rule_code_t::base_state_mismatch);
        }
        let result = apply_delta(base, payload)?;
        if encode(&result.to_value()) != encode(&payload.result_state.to_value()) {
            return reject(rule_code_t::result_state_mismatch);
        }
        // Wire format §6.6: grant ids are never reused within the user's journal (the
        // oracle checks the user's every grant, which includes the lineage's); request
        // ids are unique per user.
        if payload
            .delta
            .add_grants
            .iter()
            .any(|grant| self.grants.contains_key(&grant.grant_id))
        {
            return reject(rule_code_t::duplicate_identifier);
        }
        // A byte-identical envelope repeats its request id, so a commit id is never
        // admitted twice.
        if self.requests.contains(&payload.authorization.request_id) {
            return reject(rule_code_t::duplicate_identifier);
        }
        // ADR 0007 §6: session ids are single use and an ended session never becomes
        // live again. A grant bound to a session that a durable `session_ended` record
        // ended reuses a consumed identifier, like a grant or request id (M2-005
        // review R02); evaluation denies such a grant too, should one ever be present.
        if payload.delta.add_grants.iter().any(|grant| {
            grant
                .lifetime
                .session_id
                .as_ref()
                .is_some_and(|session_id| self.ended_sessions.contains(session_id))
        }) {
            return reject(rule_code_t::duplicate_identifier);
        }
        if payload.commit_type == commit_type_t::restore {
            self.check_restore(payload)?;
        }
        self.check_subject(lineage, payload)?;
        if placement == placement_t::temporary && payload.commit_type == commit_type_t::revocation {
            // A branch revocation removes only temporary grants added on that branch.
            let added = branch_head.and_then(|head| lineage.branches.get(&head));
            let allowed = payload
                .delta
                .remove_grant_ids
                .iter()
                .all(|removed| added.is_some_and(|added| added.contains(removed)));
            if !allowed {
                return reject(rule_code_t::branch_rule);
            }
        }
        // Wire format §6.12: at most one occupying network grant across the trunk head
        // and every live temporary head after this admission (durable records only).
        let occupying = match placement {
            placement_t::trunk => self.occupying_network_grants([&payload.result_state]),
            placement_t::temporary => {
                let others = self
                    .live_states(lineage)
                    .into_iter()
                    .filter(|(head, _)| Some(*head) != branch_head)
                    .map(|(_, state)| state);
                self.occupying_network_grants(others.chain([&payload.result_state]))
            }
        };
        if occupying.len() > 1 {
            return reject(rule_code_t::network_grant_conflict);
        }
        // (beyond the model) Interim, fail closed: delegated grants wait for the
        // delegation checks of wire format §6.4 (M2-006).
        if payload
            .delta
            .add_grants
            .iter()
            .any(|grant| grant.delegation.parent_grant_id.is_some())
        {
            return reject(rule_code_t::delegation_rule);
        }
        Ok(admission_t {
            commit_id: *commit_id,
            lineage_id: payload.lineage_id,
            placement,
            branch_head,
            new_lineage: existing.is_none(),
        })
    }

    /// Wire format §6.7 (R12): the reference is the parent or one of its ancestors in
    /// the same lineage; an added grant must match a **persistent** grant of the
    /// referenced state in contract type, resource and delegation, and may not
    /// outlive it.
    fn check_restore(&self, payload: &commit_payload_t) -> service_result_t<()> {
        let restore_rule = rule(rule_code_t::restore_rule);
        let reference_id = payload.restore_reference.ok_or(restore_rule)?;
        let reference = self.commits.get(&reference_id).ok_or(restore_rule)?;
        if reference.payload.lineage_id != payload.lineage_id {
            return Err(restore_rule);
        }
        let parent = payload.parents.first().ok_or(restore_rule)?;
        if reference_id != *parent && !self.ancestors(parent).contains(&reference_id) {
            return Err(restore_rule);
        }
        let historical: Vec<&grant_t> = reference
            .payload
            .result_state
            .grants
            .iter()
            .filter(|old| old.lifetime.kind == lifetime_kind_t::persistent)
            .collect();
        for grant in &payload.delta.add_grants {
            let resource = encode(&grant.resource.to_value());
            let delegation = encode(&grant.delegation.to_value());
            let matches: Vec<&&grant_t> = historical
                .iter()
                .filter(|old| {
                    encode(&old.resource.to_value()) == resource
                        && encode(&old.delegation.to_value()) == delegation
                })
                .collect();
            if matches.is_empty() {
                return Err(restore_rule);
            }
            // utc_time is fixed-width, so text order is time order.
            let within = matches.iter().any(|old| match &old.lifetime.expiration {
                None => true,
                Some(old_expiration) => grant
                    .lifetime
                    .expiration
                    .as_ref()
                    .is_some_and(|expiration| expiration.as_str() <= old_expiration.as_str()),
            });
            if !within {
                return Err(restore_rule);
            }
        }
        Ok(())
    }

    /// Wire format §6.9 (R06).
    fn check_subject(
        &self,
        lineage: &lineage_t,
        payload: &commit_payload_t,
    ) -> service_result_t<()> {
        if payload.commit_type == commit_type_t::genesis {
            return Ok(());
        }
        let mismatch = rule(rule_code_t::subject_binding_mismatch);
        let authoritative = lineage.authoritative_subject.as_ref().ok_or(mismatch)?;
        if payload.commit_type == commit_type_t::identity_transition {
            let from_matches = payload
                .delta
                .identity_change
                .as_ref()
                .is_some_and(|change| change.from == authoritative.identity());
            if !same_binding(&payload.subject, authoritative) || !from_matches {
                return Err(mismatch);
            }
        } else if payload.subject != *authoritative {
            return Err(mismatch);
        }
        Ok(())
    }

    /// Applies a commit that [`lineage_index_t::check_commit`] accepted, once its
    /// record is durable at `sequence`.
    pub fn apply_commit(
        &mut self,
        admission: &admission_t,
        payload: Box<commit_payload_t>,
        envelope: Vec<u8>,
        sequence: u64,
        admitted: admitted_at_t,
    ) {
        let lineage = self.lineages.entry(admission.lineage_id).or_default();
        if matches!(
            payload.commit_type,
            commit_type_t::genesis | commit_type_t::identity_transition
        ) {
            lineage.authoritative_subject = Some(payload.subject.clone());
        }
        match admission.placement {
            placement_t::trunk => {
                lineage.trunk_head = Some(admission.commit_id);
                lineage.branches.clear();
            }
            placement_t::temporary => {
                let mut added = admission
                    .branch_head
                    .and_then(|head| lineage.branches.remove(&head))
                    .unwrap_or_default();
                added.extend(payload.delta.add_grants.iter().map(|grant| grant.grant_id));
                for removed in &payload.delta.remove_grant_ids {
                    added.remove(removed);
                }
                lineage.branches.insert(admission.commit_id, added);
            }
        }
        for grant in &payload.delta.add_grants {
            self.grants.insert(
                grant.grant_id,
                grant_entry_t {
                    grant: grant.clone(),
                    lineage_id: admission.lineage_id,
                    commit_id: admission.commit_id,
                },
            );
        }
        self.requests.insert(payload.authorization.request_id);
        self.commits.insert(
            admission.commit_id,
            commit_entry_t {
                payload,
                envelope,
                sequence,
                placement: admission.placement,
                status: commit_status_t::pending,
                admitted,
            },
        );
    }

    /// Whether any lineage has a live temporary branch.
    pub fn has_live_branches(&self) -> bool {
        self.lineages
            .values()
            .any(|lineage| !lineage.branches.is_empty())
    }

    /// A new boot (ADR 0007 §5 rule 6, `branch_terminated`): every live temporary
    /// branch ends, and with it every temporary grant (timed, session, one-shot) and
    /// any network slot such a grant occupied. Trunk heads are unchanged.
    pub fn end_branches(&mut self) {
        for lineage in self.lineages.values_mut() {
            lineage.branches.clear();
        }
    }

    /// Validates a lifecycle record observed at `time` (`lifecycle_rule`).
    pub fn check_record(&self, record: &lifecycle_record_t, time: &str) -> service_result_t<()> {
        let lifecycle_rule = rule(rule_code_t::lifecycle_rule);
        match record {
            lifecycle_record_t::expired { grant_id } => {
                let expiration = self
                    .grants
                    .get(grant_id)
                    .and_then(|entry| entry.grant.lifetime.expiration.as_ref())
                    .ok_or(lifecycle_rule)?;
                // utc_time is fixed-width, so text order is time order.
                if time < expiration.as_str() {
                    return Err(lifecycle_rule);
                }
            }
            lifecycle_record_t::session_ended { session_id } => {
                if self.session_grants(session_id).is_empty() {
                    return Err(lifecycle_rule);
                }
            }
            lifecycle_record_t::restriction { grant_ids } => {
                if grant_ids.is_empty() || grant_ids.iter().any(|id| !self.grants.contains_key(id))
                {
                    return Err(lifecycle_rule);
                }
            }
        }
        Ok(())
    }

    fn session_grants(&self, session_id: &random_id_t) -> Vec<random_id_t> {
        self.grants
            .iter()
            .filter(|(_, entry)| entry.grant.lifetime.session_id.as_ref() == Some(session_id))
            .map(|(grant_id, _)| *grant_id)
            .collect()
    }

    /// Containment or quarantine of the user (ADR 0006 §§3–4, §10 step 1; the oracle's
    /// `contain`): every grant of a live state ends with `reason`, permanently; every
    /// live temporary branch ends; each lineage records its contained trunk head.
    pub fn contain(&mut self, reason: end_reason_t) {
        let mut ended = Vec::new();
        for lineage in self.lineages.values() {
            for (_, state) in self.live_states(lineage) {
                ended.extend(state.grants.iter().map(|grant| grant.grant_id));
            }
        }
        for grant_id in ended {
            self.ended.entry(grant_id).or_insert(reason);
        }
        for lineage in self.lineages.values_mut() {
            lineage.branches.clear();
            lineage.containment_head = lineage.trunk_head;
            lineage.recovered = false;
        }
    }

    /// Applies a user-state record that identityd's index accepted (ADR 0006 §§3, 4,
    /// 7, 10): containment and quarantine contain; deletion ends every grant the user
    /// ever had; the ceremony records change key authority only.
    pub fn apply_user_record(&mut self, record: &user_record_t) {
        match record {
            user_record_t::containment => self.contain(end_reason_t::suspended),
            user_record_t::quarantine => self.contain(end_reason_t::quarantined),
            user_record_t::deletion_pending => {
                let grants: Vec<random_id_t> = self.grants.keys().copied().collect();
                for grant_id in grants {
                    self.ended.entry(grant_id).or_insert(end_reason_t::deleted);
                }
            }
            user_record_t::admin_recover { .. }
            | user_record_t::recovery_opened { .. }
            | user_record_t::recovery_closed { .. } => {}
        }
    }

    /// Applies a lifecycle record that [`lineage_index_t::check_record`] accepted. A
    /// grant keeps the first reason that ended it; an ended session stays ended.
    pub fn apply_record(&mut self, record: &lifecycle_record_t) {
        let (ended, reason) = match record {
            lifecycle_record_t::expired { grant_id } => (vec![*grant_id], end_reason_t::expired),
            lifecycle_record_t::session_ended { session_id } => {
                self.ended_sessions.insert(*session_id);
                (self.session_grants(session_id), end_reason_t::session_ended)
            }
            lifecycle_record_t::restriction { grant_ids } => {
                (grant_ids.clone(), end_reason_t::restriction)
            }
        };
        for grant_id in ended {
            self.ended.entry(grant_id).or_insert(reason);
        }
    }
}
