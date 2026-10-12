//! Stateless commit semantics (wire format §§6.2–6.5, 6.9, 6.10): layer 5. Checks
//! that need the parent, the lineage or registry state are admission (layer 7).

use crate::dcbor::encode;
use crate::error::{error_code_t, format_error_t, format_result_t, layer_t};
use crate::schema::{
    authority_class_t, commit_payload_t, commit_type_t, contract_state_t, grant_t, lifetime_kind_t,
    resource_category_t,
};

/// `resource_id` of every network grant.
pub const NetworkResourceId: &str = "network_mode";
/// The network operations (exactly one per network grant).
pub const NetworkOperations: [&str; 2] = ["direct", "tor"];

fn intrinsic_error(code: error_code_t) -> format_error_t {
    format_error_t::new(layer_t::intrinsic, code)
}

/// `empty_state_hash`: the state hash of `{"grants": []}`.
pub fn empty_state_hash() -> crate::hash::digest_t {
    contract_state_t::default().state_hash()
}

fn check_commit_type(payload: &commit_payload_t) -> format_result_t<()> {
    let parents = payload.parents.len();
    let adds = &payload.delta.add_grants;
    let removes = &payload.delta.remove_grant_ids;
    let persistent_authority = payload.authority_class == authority_class_t::persistent;
    let persistent_adds = adds
        .iter()
        .all(|grant| grant.lifetime.kind == lifetime_kind_t::persistent);
    let temporary_adds = adds
        .iter()
        .all(|grant| grant.lifetime.kind != lifetime_kind_t::persistent);
    let shape = match payload.commit_type {
        commit_type_t::genesis => {
            parents == 0 && persistent_authority && persistent_adds && removes.is_empty()
        }
        commit_type_t::revision => {
            parents == 1
                && persistent_authority
                && persistent_adds
                && (!adds.is_empty() || !removes.is_empty())
        }
        commit_type_t::revocation => {
            parents == 1 && persistent_authority && adds.is_empty() && !removes.is_empty()
        }
        commit_type_t::temporary_grant => {
            parents == 1 && !adds.is_empty() && temporary_adds && removes.is_empty()
        }
        commit_type_t::merge => {
            (2..=8).contains(&parents)
                && persistent_authority
                && persistent_adds
                && payload
                    .merge_base
                    .is_some_and(|merge_base| payload.parents.contains(&merge_base))
        }
        commit_type_t::restore => parents == 1 && persistent_authority && persistent_adds,
        commit_type_t::identity_transition => {
            parents == 1 && persistent_authority && adds.is_empty() && removes.is_empty()
        }
    };
    if !shape {
        return Err(intrinsic_error(error_code_t::commit_type_rule));
    }
    if payload.merge_base.is_some() != (payload.commit_type == commit_type_t::merge)
        || payload.restore_reference.is_some() != (payload.commit_type == commit_type_t::restore)
        || payload.delta.identity_change.is_some()
            != (payload.commit_type == commit_type_t::identity_transition)
    {
        return Err(intrinsic_error(error_code_t::commit_type_rule));
    }
    Ok(())
}

/// Wire format §6.3. `utc_time` values are fixed-width, so byte order is time order.
fn check_lifetime(grant: &grant_t) -> format_result_t<()> {
    let lifetime = &grant.lifetime;
    let kind = lifetime.kind;
    let rule = || Err(intrinsic_error(error_code_t::lifetime_rule));
    if lifetime
        .expiration
        .as_ref()
        .is_some_and(|expiration| *expiration <= lifetime.effective_start)
    {
        return rule();
    }
    if kind == lifetime_kind_t::timed && lifetime.expiration.is_none() {
        return rule();
    }
    if kind == lifetime_kind_t::session && lifetime.session_id.is_none() {
        return rule();
    }
    if matches!(kind, lifetime_kind_t::persistent | lifetime_kind_t::timed)
        && lifetime.session_id.is_some()
    {
        return rule();
    }
    if (kind == lifetime_kind_t::one_shot) != lifetime.usage_limit.is_some() {
        return rule();
    }
    Ok(())
}

/// Wire format §6.4 (stateless part).
fn check_delegation(grant: &grant_t) -> format_result_t<()> {
    let delegation = &grant.delegation;
    let rule = || Err(intrinsic_error(error_code_t::delegation_rule));
    if !delegation.permitted {
        if delegation.max_depth != 0 || !delegation.max_operations.is_empty() {
            return rule();
        }
        return Ok(());
    }
    if !(1..=8).contains(&delegation.max_depth) || delegation.max_operations.is_empty() {
        return rule();
    }
    if !delegation
        .max_operations
        .iter()
        .all(|operation| grant.resource.operations.contains(operation))
    {
        return rule();
    }
    Ok(())
}

/// Wire format §6.10 (per grant).
fn check_network_grant(grant: &grant_t, owner_uid: u32) -> format_result_t<()> {
    let resource = &grant.resource;
    let rule = || Err(intrinsic_error(error_code_t::network_grant_rule));
    if resource.resource_id != NetworkResourceId {
        return rule();
    }
    match resource.operations.as_slice() {
        [operation] if NetworkOperations.contains(&operation.as_str()) => {}
        _ => return rule(),
    }
    if resource.resource_owner_uid != owner_uid
        || grant.delegation.permitted
        || grant.lifetime.kind == lifetime_kind_t::one_shot
    {
        return rule();
    }
    Ok(())
}

/// Layer 5 for a commit payload that passed the schema layer.
pub fn check_commit(payload: &commit_payload_t) -> format_result_t<()> {
    check_commit_type(payload)?;
    let adds = &payload.delta.add_grants;
    let result = &payload.result_state.grants;
    for grant in adds.iter().chain(result) {
        check_lifetime(grant)?;
    }
    for grant in adds.iter().chain(result) {
        check_delegation(grant)?;
    }
    for grant in adds
        .iter()
        .chain(result)
        .filter(|grant| grant.resource.category == resource_category_t::network)
    {
        check_network_grant(grant, payload.subject.owner_uid)?;
    }
    if result.iter().filter(|grant| grant.is_network()).count() > 1 {
        return Err(intrinsic_error(error_code_t::network_grant_rule));
    }
    if let Some(change) = &payload.delta.identity_change {
        let to = encode(&change.to.to_value());
        if to != encode(&payload.subject.identity().to_value())
            || encode(&change.from.to_value()) == to
        {
            return Err(intrinsic_error(error_code_t::identity_transition_rule));
        }
    }
    let removes = &payload.delta.remove_grant_ids;
    if adds.iter().any(|grant| removes.contains(&grant.grant_id)) {
        return Err(intrinsic_error(error_code_t::delta_rule));
    }
    for grant in adds {
        let kept = result
            .iter()
            .find(|candidate| candidate.grant_id == grant.grant_id);
        if kept.is_none_or(|kept| encode(&kept.to_value()) != encode(&grant.to_value())) {
            return Err(intrinsic_error(error_code_t::result_state_mismatch));
        }
    }
    if result.iter().any(|grant| removes.contains(&grant.grant_id)) {
        return Err(intrinsic_error(error_code_t::result_state_mismatch));
    }
    if payload.commit_type == commit_type_t::genesis
        && payload.base_state_hash != empty_state_hash()
    {
        return Err(intrinsic_error(error_code_t::base_state_mismatch));
    }
    Ok(())
}
