//! Eternal Contract Repository: immutable signed DAG, admission, indexes, lifecycle and recovery (cham3-registryd).
//!
//! M2-001 qualification skeleton. Domain logic for this crate is gated on the
//! G2-G6 design decision(s) recorded under docs/decisions/ and is not implemented
//! here. This crate exists so the safe-core lint controls in
//! build/scripts/qualify_m2_rust.sh have a real target.
#![forbid(unsafe_code)]

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
