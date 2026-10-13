//! Per-user identity registry: authenticated signing keys, authority classes, certificates and key epochs (cham3-identityd).
//!
//! Verification layer 6 (wire format §1; ADR 0005 §2, ADR 0006 §§5, 10): which
//! registered key may sign which object for which user, with a signing **scope**.
//! identityd keeps no store; its index is rebuilt from registryd's journal
//! ([`journal`]). Caller roles come only from kernel-reported peer credentials
//! ([`caller`]). A verified signature is not key authority, and key authority is not
//! admission or current authorization.
//!
//! Limits (M2-004): application/installation/session identities are M2 fixtures
//! supplied by the trusted client (ADR 0005 §1); the socket transport is M2-007; the
//! journal used by the tests is a labeled fixture until registryd exists
//! (M2-005/006). Identity replacement (`replace_identity`) is not implemented here.
#![forbid(unsafe_code)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::arithmetic_side_effects
    )
)]

pub mod caller;
pub mod directory;
pub mod index;
pub mod journal;
pub mod outcome;
pub mod service;

/// Canonical crate name, reported by qualification probes.
pub const CrateName: &str = "cham3_identityd";

/// Returns the canonical crate name.
pub fn crate_name() -> &'static str {
    CrateName
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::crate_name(), "cham3_identityd");
    }
}
