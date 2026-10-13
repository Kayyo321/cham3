//! Vault process protection (ADR 0005 §5, ADR 0003 §7).
//!
//! At start vaultd must apply, in this order, or refuse to serve (signing
//! unavailable, fail closed): `RLIMIT_CORE=0` and `PR_SET_DUMPABLE=0` (no core dumps;
//! same-UID processes cannot ptrace it), then a sufficient `RLIMIT_MEMLOCK` (the
//! locked KDF budget plus headroom) and `mlockall(MCL_CURRENT|MCL_FUTURE)`, so that
//! transient secret copies inside libraries are never swapped. These reduce exposure
//! only: root, the kernel and hibernation are outside them (PROJECT §3.4; disabling
//! hibernation is an open cham3-security host obligation).

use cham3_os_adapter::{harden_process, is_dumpable, lock_all_memory, raise_memlock_soft_limit};

use crate::kdf::MemlockHeadroomBytes;

/// What could not be applied. Contains no secret data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum protection_error_t {
    /// `RLIMIT_CORE=0` or `PR_SET_DUMPABLE=0` failed.
    hardening_failed,
    /// `RLIMIT_MEMLOCK` is below what the locked budget needs and cannot be raised.
    memlock_too_low,
    /// `mlockall` failed.
    lock_failed,
}

/// Evidence that protection was applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct protection_report_t {
    pub required_memlock_bytes: u64,
}

/// `RLIMIT_MEMLOCK` the vault needs for a locked KDF budget.
pub fn required_memlock_bytes(locked_budget_bytes: u64) -> u64 {
    locked_budget_bytes.saturating_add(MemlockHeadroomBytes)
}

/// Applies the vault's process protection, or reports what failed.
pub fn apply_vault_protection(
    locked_budget_bytes: u64,
) -> Result<protection_report_t, protection_error_t> {
    harden_process().map_err(|_| protection_error_t::hardening_failed)?;
    if is_dumpable().map_err(|_| protection_error_t::hardening_failed)? {
        return Err(protection_error_t::hardening_failed);
    }
    let required = required_memlock_bytes(locked_budget_bytes);
    raise_memlock_soft_limit(required).map_err(|_| protection_error_t::memlock_too_low)?;
    lock_all_memory().map_err(|_| protection_error_t::lock_failed)?;
    Ok(protection_report_t {
        required_memlock_bytes: required,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protection_fails_closed_when_memlock_is_insufficient() {
        if std::env::var("CHAM3_VAULTD_CHILD").as_deref() == Ok("protection") {
            // Model a host whose LimitMEMLOCK is too small (8 MiB, the container
            // default): lowering a limit is always permitted.
            rustix::process::setrlimit(
                rustix::process::Resource::Memlock,
                rustix::process::Rlimit {
                    current: Some(8 << 20),
                    maximum: Some(8 << 20),
                },
            )
            .unwrap();
            assert_eq!(
                apply_vault_protection(crate::kdf::DefaultLockedBudgetBytes),
                Err(protection_error_t::memlock_too_low)
            );
            // Hardening was applied first and stays.
            assert!(!is_dumpable().unwrap());
            let limits = std::fs::read_to_string("/proc/self/limits").unwrap();
            let core = limits
                .lines()
                .find(|line| line.starts_with("Max core file size"))
                .unwrap();
            assert_eq!(core.split_whitespace().nth(4), Some("0"));
            return;
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "protection::tests::protection_fails_closed_when_memlock_is_insufficient",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("CHAM3_VAULTD_CHILD", "protection")
            .output()
            .unwrap();
        assert!(output.status.success(), "child output {output:?}");
    }

    #[test]
    fn required_memlock_covers_budget_and_headroom() {
        assert_eq!(
            required_memlock_bytes(crate::kdf::DefaultLockedBudgetBytes),
            768 << 20
        );
        assert_eq!(required_memlock_bytes(u64::MAX), u64::MAX);
    }
}
