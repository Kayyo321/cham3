//! contractd's `submit_signed` and `verify_object` (service IPC §3).
//!
//! `submit_signed` verifies a commit envelope through layers 1–5 with the key that
//! identityd resolves for its `kid`, then layer 6 with that resolution and the
//! signing scope, then asks registryd to admit it. registryd repeats layers 1–6
//! against its own journal snapshot under the user lock and applies layer 7; a
//! commit is admitted `pending`. `verify_object` reports the byte-layer and
//! key-authority result only, never an authorization decision.
//!
//! Limits (M2-005): `prepare_proposal`, `check_proposal` and `request_revocation`
//! are M2-006; transport is M2-007.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use cham3_contract_format::dcbor::{SignedObjectTag, decode, value_t};
use cham3_contract_format::envelope::LabelKid;
use cham3_contract_format::hash::digest_t;
use cham3_contract_format::{key_resolver_t, layer_t, object_kind_t, payload_t, verify_object};
use cham3_identityd::caller::{caller_role_t, service_name_t};
use cham3_identityd::index::{check_commit_scope, key_resolution_t};
use cham3_identityd::journal::identity_journal_t;
use cham3_identityd::outcome::{ipc, ipc_code_t, service_result_t};
use cham3_identityd::service::identity_service_t;
use cham3_registryd::registry::{admission_receipt_t, registry_t};

const Contractd: caller_role_t = caller_role_t::service(service_name_t::contractd);

/// identityd as contractd uses it (role `service:contractd`).
pub trait contract_identity_link_t {
    fn resolve_key(&mut self, key_id: &digest_t) -> service_result_t<key_resolution_t>;
}

/// In-process link to an identity service shared with its other callers.
pub struct in_process_identity_t<journal_impl_t: identity_journal_t> {
    pub service: Rc<RefCell<identity_service_t<journal_impl_t>>>,
}

impl<journal_impl_t: identity_journal_t> contract_identity_link_t
    for in_process_identity_t<journal_impl_t>
{
    fn resolve_key(&mut self, key_id: &digest_t) -> service_result_t<key_resolution_t> {
        self.service
            .try_borrow_mut()
            .map_err(|_| ipc(ipc_code_t::storage_unavailable))?
            .resolve_key(Contractd, key_id)
    }
}

/// The result of `verify_object`: layers 1–5, and layer 6 for commits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct verification_report_t {
    pub kind: object_kind_t,
    pub object_id: digest_t,
    /// Whether layer 6 was checked (commits only).
    pub key_authority_checked: bool,
}

/// contractd.
pub struct contract_service_t<identity_link_impl_t: contract_identity_link_t> {
    identity: identity_link_impl_t,
    registry: Arc<registry_t>,
}

/// The `kid` of an envelope, read with the strict decoder; `None` if it is not a
/// signed object with a readable protected header (verification then reports the
/// first error in layer order).
fn envelope_kid(data: &[u8]) -> Option<digest_t> {
    let value_t::tag(SignedObjectTag, content) =
        decode(data, layer_t::envelope_decode, true).ok()?
    else {
        return None;
    };
    let value_t::array(elements) = *content else {
        return None;
    };
    let Some(value_t::bytes(protected)) = elements.first() else {
        return None;
    };
    let value_t::map(pairs) = decode(protected, layer_t::envelope_decode, false).ok()? else {
        return None;
    };
    pairs
        .into_iter()
        .find_map(|(label, value)| match (label, value) {
            (value_t::unsigned(LabelKid), value_t::bytes(kid)) => <[u8; 32]>::try_from(kid).ok(),
            _ => None,
        })
}

/// Resolves exactly one key: the one identityd answered for.
struct single_resolver_t {
    key: Option<(digest_t, [u8; 32])>,
}

impl key_resolver_t for single_resolver_t {
    fn resolve(&self, kid: &digest_t) -> Option<[u8; 32]> {
        self.key
            .filter(|(key_id, _)| key_id == kid)
            .map(|(_, public_key)| public_key)
    }
}

impl<identity_link_impl_t: contract_identity_link_t> contract_service_t<identity_link_impl_t> {
    pub fn new(
        identity: identity_link_impl_t,
        registry: Arc<registry_t>,
    ) -> contract_service_t<identity_link_impl_t> {
        contract_service_t { identity, registry }
    }

    /// Layers 1–5 with identityd's key, and the resolution for layer 6.
    fn verify(
        &mut self,
        kind: object_kind_t,
        data: &[u8],
    ) -> service_result_t<(
        cham3_contract_format::verified_object_t,
        Option<key_resolution_t>,
    )> {
        let resolution = match envelope_kid(data) {
            Some(kid) => match self.identity.resolve_key(&kid) {
                Ok(resolution) => Some(resolution),
                // identityd could not answer: fail closed with its error, not as an
                // unknown key.
                Err(error) if error == ipc(ipc_code_t::storage_unavailable) => return Err(error),
                Err(_) => None,
            },
            None => None,
        };
        let resolver = single_resolver_t {
            key: resolution
                .as_ref()
                .map(|resolution| (resolution.key_id, resolution.public_key)),
        };
        let verified = verify_object(data, kind, &resolver)?;
        Ok((verified, resolution))
    }

    /// `submit_signed` (`trusted_client`, owner).
    pub fn submit_signed(
        &mut self,
        caller: caller_role_t,
        envelope: &[u8],
    ) -> service_result_t<admission_receipt_t> {
        let caller_uid = caller.trusted_uid().ok_or(ipc(ipc_code_t::role_denied))?;
        let (verified, resolution) = self.verify(object_kind_t::commit, envelope)?;
        let payload_t::commit(payload) = &verified.payload else {
            return Err(ipc(ipc_code_t::malformed_request));
        };
        if payload.actor.uid != caller_uid {
            return Err(ipc(ipc_code_t::owner_mismatch));
        }
        let resolution = resolution.ok_or(ipc(ipc_code_t::storage_unavailable))?;
        check_commit_scope(&resolution, payload)?;
        self.registry.admit(Contractd, caller_uid, envelope)
    }

    /// `verify_object` (`service:*`, `trusted_client`).
    pub fn verify_object(
        &mut self,
        caller: caller_role_t,
        kind: object_kind_t,
        data: &[u8],
    ) -> service_result_t<verification_report_t> {
        match caller {
            caller_role_t::service(_) | caller_role_t::trusted_client { .. } => {}
            _ => return Err(ipc(ipc_code_t::role_denied)),
        }
        let (verified, resolution) = self.verify(kind, data)?;
        let mut key_authority_checked = false;
        if let payload_t::commit(payload) = &verified.payload {
            let resolution = resolution.ok_or(ipc(ipc_code_t::storage_unavailable))?;
            check_commit_scope(&resolution, payload)?;
            key_authority_checked = true;
        }
        Ok(verification_report_t {
            kind,
            object_id: verified.object_id,
            key_authority_checked,
        })
    }
}
