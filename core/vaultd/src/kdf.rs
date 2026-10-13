//! Passphrase key derivation: Argon2id with fixed bounds and an owned workspace
//! (ADR 0005 §4, ADR 0003 §7).
//!
//! Parameters are checked against the bounds and the vault's locked-memory budget
//! **before** any derivation (downgrade and denial-of-service protection). The derived
//! key-encryption key is written straight into a [`secret_region_t`].
//!
//! The pinned argon2 0.5.3 clears its internal hashes but not the caller-supplied
//! `Block` workspace. [`kdf_workspace_t`] owns that workspace: allocated once at the
//! parameters' exact block count (never grown, so no stale copies), zeroized block by
//! block after every derivation on success and on every error path, zeroized again on
//! drop, then freed. In the vault process the workspace is resident in locked memory
//! because the whole process is `mlockall`ed (ADR 0005 §5). Abort or process death
//! bypasses the zeroization; confidentiality then relies on `mlockall`, non-dumpable
//! state and `RLIMIT_CORE=0`.

use argon2::{Algorithm, Argon2, Block, Params, Version};
use cham3_secure_memory::secret_region_t;
use zeroize::Zeroize;

/// Argon2id memory bounds in KiB (64 MiB – 1 GiB).
pub const MinMemoryKib: u32 = 65_536;
pub const MaxMemoryKib: u32 = 1_048_576;
/// Pass bounds.
pub const MinPasses: u32 = 1;
pub const MaxPasses: u32 = 10;
/// Lane bounds.
pub const MinLanes: u32 = 1;
pub const MaxLanes: u32 = 4;
/// Release calibration (RFC 9106 second recommended option scaled): 256 MiB, 3
/// passes, 1 lane. Fixed per release, not per host.
pub const DefaultMemoryKib: u32 = 262_144;
pub const DefaultPasses: u32 = 3;
pub const DefaultLanes: u32 = 1;
/// Default locked-memory budget for one derivation (512 MiB).
pub const DefaultLockedBudgetBytes: u64 = 512 * 1024 * 1024;
/// `LimitMEMLOCK` headroom above the budget (256 MiB).
pub const MemlockHeadroomBytes: u64 = 256 * 1024 * 1024;
/// Argon2id version 0x13.
pub const ArgonVersion: u64 = 19;
/// Length of the derived key-encryption key.
pub const KekBytes: usize = 32;
/// Length of the per-object salt.
pub const SaltBytes: usize = 16;

/// Argon2id parameters of one key object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct kdf_params_t {
    pub memory_kib: u32,
    pub passes: u32,
    pub lanes: u32,
}

impl kdf_params_t {
    pub const Default: kdf_params_t = kdf_params_t {
        memory_kib: DefaultMemoryKib,
        passes: DefaultPasses,
        lanes: DefaultLanes,
    };

    /// The cheapest parameters inside the bounds (used by tests; still a real
    /// Argon2id derivation over 64 MiB).
    pub const Minimum: kdf_params_t = kdf_params_t {
        memory_kib: MinMemoryKib,
        passes: MinPasses,
        lanes: MinLanes,
    };

    /// Bytes of workspace a derivation needs.
    pub fn memory_bytes(&self) -> u64 {
        u64::from(self.memory_kib).saturating_mul(1024)
    }

    /// Checks the bounds and the locked budget before any derivation.
    pub fn check(&self, locked_budget_bytes: u64) -> Result<(), kdf_error_t> {
        let in_bounds = (MinMemoryKib..=MaxMemoryKib).contains(&self.memory_kib)
            && (MinPasses..=MaxPasses).contains(&self.passes)
            && (MinLanes..=MaxLanes).contains(&self.lanes);
        if !in_bounds {
            return Err(kdf_error_t::out_of_bounds);
        }
        if self.memory_bytes() > locked_budget_bytes {
            return Err(kdf_error_t::over_budget);
        }
        Ok(())
    }
}

/// Why a derivation did not run or failed. Contains no secret data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum kdf_error_t {
    out_of_bounds,
    over_budget,
    /// The workspace does not have the parameters' block count.
    workspace_size,
    /// Argon2 rejected its inputs.
    derivation,
    /// A secret region could not be allocated (memory locking failed).
    secret_memory,
}

/// Caller-owned Argon2id workspace with explicit clearing.
pub struct kdf_workspace_t {
    blocks: Vec<Block>,
}

impl kdf_workspace_t {
    /// Allocates a zeroed workspace of exactly `block_count` blocks.
    pub fn new(block_count: usize) -> kdf_workspace_t {
        kdf_workspace_t {
            blocks: vec![Block::default(); block_count],
        }
    }

    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// Zeroizes every block (volatile writes).
    pub fn clear(&mut self) {
        for block in &mut self.blocks {
            block.zeroize();
        }
    }

    /// True when every word of every block is zero.
    pub fn is_clear(&self) -> bool {
        self.blocks
            .iter()
            .all(|block| block.as_ref().iter().all(|word| *word == 0))
    }

    /// Derives into `output`, then clears the workspace whatever the outcome.
    pub fn derive(
        &mut self,
        params: &Params,
        password: &[u8],
        salt: &[u8],
        output: &mut [u8],
    ) -> Result<(), kdf_error_t> {
        let result = if self.blocks.len() == params.block_count() {
            Argon2::new(Algorithm::Argon2id, Version::V0x13, params.clone())
                .hash_password_into_with_memory(password, salt, output, &mut self.blocks)
                .map_err(|_| kdf_error_t::derivation)
        } else {
            Err(kdf_error_t::workspace_size)
        };
        self.clear();
        if result.is_err() {
            output.zeroize();
        }
        result
    }
}

impl Drop for kdf_workspace_t {
    fn drop(&mut self) {
        self.clear();
    }
}

fn argon2_params(params: &kdf_params_t, output_length: usize) -> Result<Params, kdf_error_t> {
    Params::new(
        params.memory_kib,
        params.passes,
        params.lanes,
        Some(output_length),
    )
    .map_err(|_| kdf_error_t::out_of_bounds)
}

/// Derives the key-encryption key for one key object into a fresh secret region.
/// Bounds and budget are checked before anything is allocated or derived.
pub fn derive_kek(
    params: &kdf_params_t,
    locked_budget_bytes: u64,
    passphrase: &[u8],
    salt: &[u8; SaltBytes],
) -> Result<secret_region_t, kdf_error_t> {
    params.check(locked_budget_bytes)?;
    let argon_params = argon2_params(params, KekBytes)?;
    let mut kek = secret_region_t::new(KekBytes).map_err(|_| kdf_error_t::secret_memory)?;
    let mut workspace = kdf_workspace_t::new(argon_params.block_count());
    workspace.derive(&argon_params, passphrase, salt, kek.as_mut_slice())?;
    Ok(kek)
}

/// Test entry point below the bounds: the same workspace path with small parameters,
/// so zeroization can be observed cheaply. Never used by the vault.
#[cfg(test)]
pub(crate) fn derive_unbounded(
    params: &kdf_params_t,
    password: &[u8],
    salt: &[u8],
    output: &mut [u8],
    workspace: &mut kdf_workspace_t,
) -> Result<(), kdf_error_t> {
    let argon_params = argon2_params(params, output.len())?;
    workspace.derive(&argon_params, password, salt, output)
}

#[cfg(test)]
mod tests {
    use super::*;

    const Small: kdf_params_t = kdf_params_t {
        memory_kib: 64,
        passes: 1,
        lanes: 1,
    };

    fn small_workspace() -> kdf_workspace_t {
        kdf_workspace_t::new(argon2_params(&Small, 32).unwrap().block_count())
    }

    #[test]
    fn bounds_and_budget_are_checked_before_derivation() {
        let budget = DefaultLockedBudgetBytes;
        kdf_params_t::Default.check(budget).unwrap();
        kdf_params_t::Minimum.check(budget).unwrap();
        let outside = [
            kdf_params_t {
                memory_kib: MinMemoryKib - 1,
                ..kdf_params_t::Default
            },
            kdf_params_t {
                memory_kib: MaxMemoryKib + 1,
                ..kdf_params_t::Default
            },
            kdf_params_t {
                passes: 0,
                ..kdf_params_t::Default
            },
            kdf_params_t {
                passes: MaxPasses + 1,
                ..kdf_params_t::Default
            },
            kdf_params_t {
                lanes: 0,
                ..kdf_params_t::Default
            },
            kdf_params_t {
                lanes: MaxLanes + 1,
                ..kdf_params_t::Default
            },
        ];
        for params in outside {
            assert_eq!(
                params.check(budget),
                Err(kdf_error_t::out_of_bounds),
                "{params:?}"
            );
            // Refused before any allocation or derivation.
            assert!(matches!(
                derive_kek(&params, budget, b"passphrase", &[1; SaltBytes]),
                Err(kdf_error_t::out_of_bounds)
            ));
        }
        let large = kdf_params_t {
            memory_kib: MaxMemoryKib,
            ..kdf_params_t::Default
        };
        assert_eq!(large.check(budget), Err(kdf_error_t::over_budget));
        assert!(matches!(
            derive_kek(&large, budget, b"passphrase", &[1; SaltBytes]),
            Err(kdf_error_t::over_budget)
        ));
        large.check(MaxMemoryKib as u64 * 1024).unwrap();
    }

    #[test]
    fn workspace_is_clear_after_success() {
        let mut workspace = small_workspace();
        let mut output = [0u8; 32];
        derive_unbounded(&Small, b"passphrase", &[2; 16], &mut output, &mut workspace).unwrap();
        assert_ne!(output, [0u8; 32]);
        assert!(workspace.is_clear());
    }

    #[test]
    fn workspace_is_clear_after_every_error_path() {
        // Argon2 rejects a salt shorter than 8 bytes after the workspace is supplied.
        let mut workspace = small_workspace();
        let mut output = [7u8; 32];
        assert_eq!(
            derive_unbounded(&Small, b"passphrase", &[2; 4], &mut output, &mut workspace),
            Err(kdf_error_t::derivation)
        );
        assert!(workspace.is_clear());
        assert_eq!(
            output, [0u8; 32],
            "a failed derivation leaves no partial output"
        );
        // A workspace of the wrong size is refused and cleared too.
        let mut wrong = small_workspace();
        Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            argon2_params(&Small, 32).unwrap(),
        )
        .hash_password_into_with_memory(b"populate", &[5; 16], &mut [0u8; 32], &mut wrong.blocks)
        .unwrap();
        wrong.blocks.push(Block::default());
        assert!(!wrong.is_clear(), "control: the workspace starts populated");
        assert_eq!(
            derive_unbounded(&Small, b"passphrase", &[2; 16], &mut output, &mut wrong),
            Err(kdf_error_t::workspace_size)
        );
        assert!(wrong.is_clear());
    }

    #[test]
    fn argon2_library_leaves_caller_workspace_populated() {
        // Regression control: without the owner's clearing the library leaves derived
        // blocks in the caller's memory, so the clearing above is necessary.
        let params = argon2_params(&Small, 32).unwrap();
        let mut blocks = vec![Block::default(); params.block_count()];
        let mut output = [0u8; 32];
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
            .hash_password_into_with_memory(b"passphrase", &[2; 16], &mut output, &mut blocks)
            .unwrap();
        assert!(
            blocks
                .iter()
                .any(|block| block.as_ref().iter().any(|word| *word != 0))
        );
    }

    #[test]
    fn derivation_matches_the_rfc9106_shape_and_is_deterministic() {
        let mut first = [0u8; 32];
        let mut second = [0u8; 32];
        let mut changed = [0u8; 32];
        derive_unbounded(
            &Small,
            b"passphrase",
            &[3; 16],
            &mut first,
            &mut small_workspace(),
        )
        .unwrap();
        derive_unbounded(
            &Small,
            b"passphrase",
            &[3; 16],
            &mut second,
            &mut small_workspace(),
        )
        .unwrap();
        derive_unbounded(
            &Small,
            b"passphrasf",
            &[3; 16],
            &mut changed,
            &mut small_workspace(),
        )
        .unwrap();
        assert_eq!(first, second);
        assert_ne!(first, changed);
    }
}
