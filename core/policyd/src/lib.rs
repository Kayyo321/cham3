//! Current-authorization and mandatory-policy evaluation (cham3-policyd).
//!
//! M2-005 implements verification layer 8 (ADR 0007 §1): [`evaluate::evaluate`] and
//! [`evaluate::network_mode`] as pure functions of one consistent registry snapshot,
//! identityd's key answers, the mandatory policy ([`mandatory`]), trusted now and
//! session liveness, and the service operations around them ([`service`]).
//! Signature validity and admission are not current authorization, and an `eligible`
//! outcome is never a claim that anything is enforced.
//!
//! Limits (M2-005): the mandatory policy is a configuration model supplied at start;
//! `set_policy_epoch`, `set_emergency_restriction`, `check_proposal`, the trusted-time
//! offset rule and one-shot reservations are M2-006; transport is M2-007.
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

pub mod evaluate;
pub mod mandatory;
pub mod service;

/// Canonical crate name, reported by qualification probes.
pub const CrateName: &str = "cham3_policyd";

/// Returns the canonical crate name.
pub fn crate_name() -> &'static str {
    CrateName
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::crate_name(), "cham3_policyd");
    }
}
