//! Eternal Contract Repository: immutable signed DAG, admission, indexes, lifecycle and recovery (cham3-registryd).
//!
//! M2-005 implements the per-user append-only journal ([`journal`], [`record`]),
//! verification layer 7, admission ([`lineage`]), and the registry operations that
//! identityd, contractd and policyd use ([`registry`]): `append_identity`,
//! `identity_state`, `admit` and `lineage_head`. Admitted commits are `pending`;
//! admission is not current authorization (policyd) and never enforcement.
//!
//! Limits (M2-005): the anchor, run marker, restriction ledger, crash and torn-tail
//! recovery, quarantine, containment and the recovery ceremony operations, the
//! restrictive write path, one-shot reservations, enforcement confirmation, history
//! and delegation admission are M2-006; transport and processes are M2-007.
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

pub mod journal;
pub mod lineage;
pub mod record;
pub mod registry;

/// Canonical crate name, reported by qualification probes.
pub const CrateName: &str = "cham3_registryd";

/// Returns the canonical crate name.
pub fn crate_name() -> &'static str {
    CrateName
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::crate_name(), "cham3_registryd");
    }
}
