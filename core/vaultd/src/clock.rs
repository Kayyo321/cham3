//! Time sources of the vault.
//!
//! Proposal expiry (≤ 120 s, ADR 0005 §3) is checked against **both** a monotonic
//! clock and the UTC clock, so neither suspend (which the monotonic clock may not
//! count) nor a wall-clock change extends a proposal. Object times (`enrolled`,
//! `issued`, `not_before`) come from the UTC clock. ADR 0007's trusted-time offset
//! belongs to policyd/registryd; in M2-004 the vault reads the system clock directly
//! (limit: trusted-time integration is M2-006/M2-007).

use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// A time source.
pub trait vault_clock_t {
    /// Seconds of a monotonic clock (arbitrary origin).
    fn monotonic_seconds(&self) -> u64;
    /// Seconds since 1970-01-01T00:00:00Z.
    fn utc_seconds(&self) -> i64;
}

/// The system clocks.
pub struct system_clock_t {
    origin: Instant,
}

impl system_clock_t {
    pub fn new() -> system_clock_t {
        system_clock_t {
            origin: Instant::now(),
        }
    }
}

impl Default for system_clock_t {
    fn default() -> system_clock_t {
        system_clock_t::new()
    }
}

impl vault_clock_t for system_clock_t {
    fn monotonic_seconds(&self) -> u64 {
        self.origin.elapsed().as_secs()
    }

    fn utc_seconds(&self) -> i64 {
        // A clock before 1970 reads as 0, which no valid utc_time accepts, so object
        // creation then fails closed.
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|elapsed| i64::try_from(elapsed.as_secs()).ok())
            .unwrap_or(0)
    }
}
