//! vaultd's peers: identityd (key status, scope and registration) and registryd (the
//! recovery ceremony's key naming and status).
//!
//! The traits are the operations of service IPC §3 that vaultd calls, always in the
//! role `service:vaultd`. M2-007 implements them over the authenticated sockets; the
//! in-process adapter here connects vaultd to an `identity_service_t` for M2-004
//! tests and composition.

use std::cell::RefCell;
use std::rc::Rc;

use cham3_contract_format::hash::digest_t;
use cham3_contract_format::schema::random_id_t;
use cham3_identityd::caller::{caller_role_t, service_name_t};
use cham3_identityd::index::{key_resolution_t, naming_effect_t, naming_t, recovery_status_t};
use cham3_identityd::journal::{append_receipt_t, identity_journal_t};
use cham3_identityd::outcome::{ipc_code_t, service_error_t, service_result_t};
use cham3_identityd::service::{enrollment_receipt_t, identity_service_t, user_identity_t};

/// identityd as vaultd uses it.
pub trait identity_client_t {
    fn register_enrollment(
        &mut self,
        owner_uid: u32,
        enrollment: &[u8],
        persistent_certificate: &[u8],
        temporary_certificate: &[u8],
    ) -> service_result_t<enrollment_receipt_t>;
    fn register_certificate(
        &mut self,
        owner_uid: u32,
        certificate: &[u8],
    ) -> service_result_t<append_receipt_t>;
    fn register_invalidation(
        &mut self,
        owner_uid: u32,
        invalidation: &[u8],
    ) -> service_result_t<append_receipt_t>;
    fn resolve_key(&mut self, key_id: &digest_t) -> service_result_t<key_resolution_t>;
    fn user_identity(&mut self, uid: u32) -> service_result_t<user_identity_t>;
}

/// registryd's ceremony operations as vaultd uses them (ADR 0006 §10 step 3).
pub trait recovery_registry_t {
    fn recovery_status(
        &mut self,
        owner_uid: u32,
        recovery_id: &random_id_t,
    ) -> service_result_t<recovery_status_t>;
    fn name_recovery_keys(
        &mut self,
        owner_uid: u32,
        naming: &naming_t,
    ) -> service_result_t<naming_effect_t>;
}

const Vaultd: caller_role_t = caller_role_t::service(service_name_t::vaultd);

/// In-process link to an identity service shared with its other callers.
pub struct in_process_identity_t<journal_impl_t: identity_journal_t> {
    pub service: Rc<RefCell<identity_service_t<journal_impl_t>>>,
}

impl<journal_impl_t: identity_journal_t> in_process_identity_t<journal_impl_t> {
    fn with<result_t>(
        &self,
        operation: impl FnOnce(&mut identity_service_t<journal_impl_t>) -> service_result_t<result_t>,
    ) -> service_result_t<result_t> {
        let mut service = self
            .service
            .try_borrow_mut()
            .map_err(|_| service_error_t::ipc(ipc_code_t::storage_unavailable))?;
        operation(&mut service)
    }
}

impl<journal_impl_t: identity_journal_t> identity_client_t
    for in_process_identity_t<journal_impl_t>
{
    fn register_enrollment(
        &mut self,
        owner_uid: u32,
        enrollment: &[u8],
        persistent_certificate: &[u8],
        temporary_certificate: &[u8],
    ) -> service_result_t<enrollment_receipt_t> {
        self.with(|service| {
            service.register_enrollment(
                Vaultd,
                owner_uid,
                enrollment,
                persistent_certificate,
                temporary_certificate,
            )
        })
    }

    fn register_certificate(
        &mut self,
        owner_uid: u32,
        certificate: &[u8],
    ) -> service_result_t<append_receipt_t> {
        self.with(|service| service.register_certificate(Vaultd, owner_uid, certificate))
    }

    fn register_invalidation(
        &mut self,
        owner_uid: u32,
        invalidation: &[u8],
    ) -> service_result_t<append_receipt_t> {
        self.with(|service| service.register_invalidation(Vaultd, owner_uid, invalidation))
    }

    fn resolve_key(&mut self, key_id: &digest_t) -> service_result_t<key_resolution_t> {
        self.with(|service| service.resolve_key(Vaultd, key_id))
    }

    fn user_identity(&mut self, uid: u32) -> service_result_t<user_identity_t> {
        self.with(|service| service.user_identity(Vaultd, uid))
    }
}
