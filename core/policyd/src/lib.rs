//! Current-authorization and mandatory-policy evaluation (cham3-policyd).
//!
//! M2-001 qualification skeleton. Domain logic for this crate is gated on the
//! G2-G5 design decision(s) recorded under docs/decisions/ and is not implemented
//! here. This crate exists so the safe-core lint controls in
//! build/scripts/qualify_m2_rust.sh have a real target.
#![forbid(unsafe_code)]

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
