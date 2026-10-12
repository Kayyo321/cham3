//! Canonical cham3 contract schema, deterministic CBOR profile and COSE_Sign1 envelope handling.
//!
//! M2-001 qualification skeleton. Domain logic for this crate is gated on the
//! G2 (wire/schema/hash) design decision(s) recorded under docs/decisions/ and is not implemented
//! here. This crate exists so the safe-core lint controls in
//! build/scripts/qualify_m2_rust.sh have a real target.
#![forbid(unsafe_code)]

/// Canonical crate name, reported by qualification probes.
pub const CrateName: &str = "cham3_contract_format";

/// Returns the canonical crate name.
pub fn crate_name() -> &'static str {
    CrateName
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::crate_name(), "cham3_contract_format");
    }
}
