//! Outcome vocabulary of the M2 services (service IPC §4, wire format §7).
//!
//! Every refused operation reports exactly one stable `layer/code`. Byte-layer codes
//! (layers 1–5) come from `cham3_contract_format`; this module adds the key-authority
//! codes of layer 6, the registry-rule codes that published vectors report under the
//! `admission` layer, and the IPC/service vocabulary.

use std::fmt;

use cham3_contract_format::format_error_t;

/// Layer-6 key-authority codes (wire format §7; ADR 0005 §2, ADR 0006 §10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum authority_code_t {
    key_not_certified,
    key_invalidated,
    key_epoch_stale,
    key_retired,
    authority_class_denied,
    certificate_window,
    user_deleted,
    quarantined,
    contained,
    recovery_key_required,
}

impl authority_code_t {
    pub fn as_str(self) -> &'static str {
        match self {
            authority_code_t::key_not_certified => "key_not_certified",
            authority_code_t::key_invalidated => "key_invalidated",
            authority_code_t::key_epoch_stale => "key_epoch_stale",
            authority_code_t::key_retired => "key_retired",
            authority_code_t::authority_class_denied => "authority_class_denied",
            authority_code_t::certificate_window => "certificate_window",
            authority_code_t::user_deleted => "user_deleted",
            authority_code_t::quarantined => "quarantined",
            authority_code_t::contained => "contained",
            authority_code_t::recovery_key_required => "recovery_key_required",
        }
    }
}

/// Registry-rule codes reported under the `admission` layer (wire format §7 registry
/// records; ADR 0006 §10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum rule_code_t {
    recovery_rule,
    lifecycle_rule,
}

impl rule_code_t {
    pub fn as_str(self) -> &'static str {
        match self {
            rule_code_t::recovery_rule => "recovery_rule",
            rule_code_t::lifecycle_rule => "lifecycle_rule",
        }
    }
}

/// IPC/service codes (service IPC §4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ipc_code_t {
    malformed_request,
    unknown_operation,
    role_denied,
    owner_mismatch,
    proposal_unknown,
    proposal_expired,
    proposal_used,
    approval_mismatch,
    authentication_failed,
    attempts_exhausted,
    protection_unavailable,
    stale_head,
    stale_epoch,
    duplicate_identifier,
    object_too_large,
    quota_exhausted,
    storage_unavailable,
    restriction_not_durable,
    deletion_pending,
    deadline_exceeded,
}

impl ipc_code_t {
    pub fn as_str(self) -> &'static str {
        match self {
            ipc_code_t::malformed_request => "malformed_request",
            ipc_code_t::unknown_operation => "unknown_operation",
            ipc_code_t::role_denied => "role_denied",
            ipc_code_t::owner_mismatch => "owner_mismatch",
            ipc_code_t::proposal_unknown => "proposal_unknown",
            ipc_code_t::proposal_expired => "proposal_expired",
            ipc_code_t::proposal_used => "proposal_used",
            ipc_code_t::approval_mismatch => "approval_mismatch",
            ipc_code_t::authentication_failed => "authentication_failed",
            ipc_code_t::attempts_exhausted => "attempts_exhausted",
            ipc_code_t::protection_unavailable => "protection_unavailable",
            ipc_code_t::stale_head => "stale_head",
            ipc_code_t::stale_epoch => "stale_epoch",
            ipc_code_t::duplicate_identifier => "duplicate_identifier",
            ipc_code_t::object_too_large => "object_too_large",
            ipc_code_t::quota_exhausted => "quota_exhausted",
            ipc_code_t::storage_unavailable => "storage_unavailable",
            ipc_code_t::restriction_not_durable => "restriction_not_durable",
            ipc_code_t::deletion_pending => "deletion_pending",
            ipc_code_t::deadline_exceeded => "deadline_exceeded",
        }
    }
}

/// Response status of a refused operation (service IPC §2): `denied` is a policy or
/// authorization outcome, `error` a malformed or failed operation. Both are final.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum refusal_status_t {
    denied,
    error,
}

/// One refused operation: exactly one layer and code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum service_error_t {
    /// Layers 1–5 (wire format §1).
    format(format_error_t),
    /// Layer 6.
    key_authority(authority_code_t),
    /// Registry rules.
    admission(rule_code_t),
    /// IPC/service.
    ipc(ipc_code_t),
}

impl service_error_t {
    pub fn layer(&self) -> &'static str {
        match self {
            service_error_t::format(error) => error.layer.as_str(),
            service_error_t::key_authority(_) => "key_authority",
            service_error_t::admission(_) => "admission",
            service_error_t::ipc(_) => "ipc",
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            service_error_t::format(error) => error.code.as_str(),
            service_error_t::key_authority(code) => code.as_str(),
            service_error_t::admission(code) => code.as_str(),
            service_error_t::ipc(code) => code.as_str(),
        }
    }

    pub fn status(&self) -> refusal_status_t {
        match self {
            service_error_t::key_authority(_) | service_error_t::admission(_) => {
                refusal_status_t::denied
            }
            service_error_t::format(_) => refusal_status_t::error,
            service_error_t::ipc(code) => match code {
                ipc_code_t::malformed_request
                | ipc_code_t::unknown_operation
                | ipc_code_t::object_too_large
                | ipc_code_t::storage_unavailable
                | ipc_code_t::restriction_not_durable
                | ipc_code_t::protection_unavailable
                | ipc_code_t::deadline_exceeded => refusal_status_t::error,
                _ => refusal_status_t::denied,
            },
        }
    }
}

impl From<format_error_t> for service_error_t {
    fn from(error: format_error_t) -> service_error_t {
        service_error_t::format(error)
    }
}

impl fmt::Display for service_error_t {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.layer(), self.code())
    }
}

impl std::error::Error for service_error_t {}

/// Result of a service operation.
pub type service_result_t<ok_t> = Result<ok_t, service_error_t>;

pub(crate) fn authority(code: authority_code_t) -> service_error_t {
    service_error_t::key_authority(code)
}

pub(crate) fn rule(code: rule_code_t) -> service_error_t {
    service_error_t::admission(code)
}

pub(crate) fn ipc(code: ipc_code_t) -> service_error_t {
    service_error_t::ipc(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cham3_contract_format::{error_code_t, layer_t};

    #[test]
    fn layers_and_codes_are_stable_strings() {
        let cases = [
            (
                service_error_t::format(format_error_t::new(
                    layer_t::signature,
                    error_code_t::unknown_key,
                )),
                "signature/unknown_key",
                refusal_status_t::error,
            ),
            (
                authority(authority_code_t::recovery_key_required),
                "key_authority/recovery_key_required",
                refusal_status_t::denied,
            ),
            (
                rule(rule_code_t::recovery_rule),
                "admission/recovery_rule",
                refusal_status_t::denied,
            ),
            (
                ipc(ipc_code_t::role_denied),
                "ipc/role_denied",
                refusal_status_t::denied,
            ),
            (
                ipc(ipc_code_t::storage_unavailable),
                "ipc/storage_unavailable",
                refusal_status_t::error,
            ),
        ];
        for (error, text, status) in cases {
            assert_eq!(error.to_string(), text);
            assert_eq!(error.status(), status);
        }
    }
}
