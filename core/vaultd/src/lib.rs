//! Encrypted signing vault: key objects, authorized unlock and exact-proposal signing (cham3-vaultd).
//!
//! ADR 0005 §§2–5 and service IPC §3: encrypted key objects ([`key_object`]) under
//! Argon2id-derived keys ([`kdf`]) in a restricted file store ([`store`]); fresh
//! passphrase approval of exactly one registered proposal, bound to its digest, with
//! attempt limiting ([`service`]); restartable key creation, rotation, invalidation
//! and recovery-ceremony keys ([`records`]); process protection at start
//! ([`protection`]). Seeds, key-encryption keys and passphrases live only in
//! `secret_region_t` regions.
//!
//! Limits (M2-004): the socket transport, systemd confinement and the vaultd binary
//! are M2-007; registryd's ceremony operations are reached through
//! [`links::recovery_registry_t`] (implemented by registryd in M2-006); object times
//! use the system clock until trusted-time integration. Passphrase knowledge with
//! exact-digest binding authenticates a party who knows the passphrase; it is not a
//! trusted display, trusted input path or proof of human presence (ADR 0005 §8).
#![forbid(unsafe_code)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::arithmetic_side_effects,
        clippy::as_conversions
    )
)]

pub mod clock;
pub mod kdf;
pub mod key_object;
pub mod links;
pub mod protection;
pub mod records;
pub mod service;
pub mod store;

/// Canonical crate name, reported by qualification probes.
pub const CrateName: &str = "cham3_vaultd";

/// Returns the canonical crate name.
pub fn crate_name() -> &'static str {
    CrateName
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::crate_name(), "cham3_vaultd");
    }
}
