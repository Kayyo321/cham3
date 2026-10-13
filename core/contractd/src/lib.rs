//! Contract proposals, canonical encoding, signature verification and revisions (cham3-contractd).
//!
//! M2-005 implements the submission path ([`submit`]): `submit_signed` verifies a
//! signed commit through layers 1–6 with identityd's key answer and asks registryd to
//! admit it; `verify_object` reports byte-layer and key-authority validity only.
//!
//! Limits (M2-005): proposal preparation (`prepare_proposal`, including recovery
//! proposals and forward-restore), `check_proposal` and `request_revocation` are
//! M2-006; transport is M2-007.
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

pub mod submit;

/// Canonical crate name, reported by qualification probes.
pub const CrateName: &str = "cham3_contractd";

/// Returns the canonical crate name.
pub fn crate_name() -> &'static str {
    CrateName
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::crate_name(), "cham3_contractd");
    }
}
