//! Vault process-protection probe (ADR 0005 §5). **Qualification tool only; never
//! packaged or deployed.**
//!
//! Applies the vault's start-up protection exactly as vaultd will
//! ([`cham3_vaultd::protection::apply_vault_protection`] with the default locked
//! budget), then runs one Argon2id derivation at the release calibration into a
//! secret region and reports, from the kernel's own view of the process:
//!
//! * `dumpable 0` and `core_limit 0` (`/proc/self/limits`);
//! * `vm_locked_kib`, and `unlocked_mappings 0`: every resident mapping in
//!   `/proc/self/smaps` is fully locked (`Locked == Rss`, so nothing can be swapped);
//! * `dontdump_mappings` ≥ 1: the secret region is excluded from core dumps;
//! * `derivation_ms`: the calibration's cost on this host.
//!
//! Exit status: 0 when every check holds; 3 with `protection_unavailable <reason>`
//! when protection cannot be applied (the vault then serves nothing); 4 when a check
//! fails after protection.

use std::time::Instant;

use cham3_secure_memory::secret_region_t;
use cham3_vaultd::kdf::{DefaultLockedBudgetBytes, derive_kek, kdf_params_t};
use cham3_vaultd::protection::{apply_vault_protection, protection_error_t};

fn field_kib(status: &str, name: &str) -> Option<u64> {
    status
        .lines()
        .find(|line| line.starts_with(name))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

struct smaps_summary_t {
    unlocked: u64,
    dontdump: u64,
}

fn smaps_summary(smaps: &str) -> smaps_summary_t {
    let mut summary = smaps_summary_t {
        unlocked: 0,
        dontdump: 0,
    };
    let mut rss = 0u64;
    let mut locked = 0u64;
    for line in smaps.lines() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("Rss:") => {
                rss = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0)
            }
            Some("Locked:") => {
                locked = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0)
            }
            Some("VmFlags:") => {
                let flags: Vec<&str> = parts.collect();
                if rss > locked {
                    summary.unlocked = summary.unlocked.saturating_add(1);
                }
                if flags.contains(&"dd") {
                    summary.dontdump = summary.dontdump.saturating_add(1);
                }
                rss = 0;
                locked = 0;
            }
            _ => {}
        }
    }
    summary
}

fn fail(code: i32, message: &str) -> ! {
    println!("{message}");
    std::process::exit(code);
}

fn main() {
    let report = match apply_vault_protection(DefaultLockedBudgetBytes) {
        Ok(report) => report,
        Err(error) => {
            let reason = match error {
                protection_error_t::hardening_failed => "hardening_failed",
                protection_error_t::memlock_too_low => "memlock_too_low",
                protection_error_t::lock_failed => "lock_failed",
            };
            fail(3, &format!("protection_unavailable {reason}"));
        }
    };
    println!("required_memlock_bytes {}", report.required_memlock_bytes);
    let started = Instant::now();
    let kek = match derive_kek(
        &kdf_params_t::Default,
        DefaultLockedBudgetBytes,
        b"synthetic protection probe passphrase",
        &[7; 16],
    ) {
        Ok(kek) => kek,
        Err(error) => fail(4, &format!("derivation_failed {error:?}")),
    };
    println!("derivation_ms {}", started.elapsed().as_millis());
    let Ok(region) = secret_region_t::new(64) else {
        fail(4, "secret_region_failed");
    };
    let read = |path: &str| std::fs::read_to_string(path).unwrap_or_default();
    let limits = read("/proc/self/limits");
    let core_limit = limits
        .lines()
        .find(|line| line.starts_with("Max core file size"))
        .and_then(|line| line.split_whitespace().nth(4))
        .unwrap_or("unknown")
        .to_owned();
    let dumpable = match cham3_os_adapter::is_dumpable() {
        Ok(false) => "0",
        Ok(true) => "1",
        Err(_) => "unknown",
    };
    let status = read("/proc/self/status");
    let summary = smaps_summary(&read("/proc/self/smaps"));
    println!("dumpable {dumpable}");
    println!("core_limit {core_limit}");
    println!(
        "vm_locked_kib {}",
        field_kib(&status, "VmLck:").unwrap_or(0)
    );
    println!("unlocked_mappings {}", summary.unlocked);
    println!("dontdump_mappings {}", summary.dontdump);
    drop((kek, region));
    let held =
        dumpable == "0" && core_limit == "0" && summary.unlocked == 0 && summary.dontdump >= 1;
    if held {
        println!("protection_verified");
    } else {
        fail(4, "protection_check_failed");
    }
}
