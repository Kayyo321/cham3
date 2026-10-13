//! Narrow safe Linux adapter for cham3 core services.
//!
//! Exposes only the operating-system facilities the M2 services need, through
//! rustix's safe APIs: the Linux CSPRNG (`getrandom(2)`, blocking until the pool is
//! initialized, no `/dev/urandom` fallback), process hardening (no core dumps,
//! non-dumpable/non-ptraceable by same-UID processes), process-wide memory locking,
//! and kernel-reported Unix socket peer credentials.
//!
//! Peer credentials identify the connecting process's effective UID/GID and PID at
//! connect time. They are an input to cham3 role checks, never by themselves proof of
//! a trusted role or of human approval.
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

use std::fmt;
use std::os::fd::AsFd;

use rustix::io::Errno;

/// Operating-system call that failed. Contains no secret data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum os_error_t {
    /// `getrandom(2)` failed.
    random_failed(Errno),
    /// `getrandom(2)` returned no bytes repeatedly.
    random_stalled,
    /// `setrlimit(RLIMIT_CORE)` failed.
    core_limit_failed(Errno),
    /// `prctl(PR_SET_DUMPABLE)` failed or did not take effect.
    dumpable_failed(Errno),
    /// `mlockall(2)` failed.
    lock_all_failed(Errno),
    /// `SO_PEERCRED` lookup failed.
    peer_credentials_failed(Errno),
    /// `getrlimit`/`setrlimit(RLIMIT_MEMLOCK)` failed.
    memlock_limit_failed(Errno),
}

impl fmt::Display for os_error_t {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::random_failed(errno) => write!(formatter, "getrandom failed: {errno}"),
            Self::random_stalled => formatter.write_str("getrandom returned no data"),
            Self::core_limit_failed(errno) => write!(formatter, "RLIMIT_CORE failed: {errno}"),
            Self::dumpable_failed(errno) => write!(formatter, "PR_SET_DUMPABLE failed: {errno}"),
            Self::lock_all_failed(errno) => write!(formatter, "mlockall failed: {errno}"),
            Self::peer_credentials_failed(errno) => {
                write!(formatter, "SO_PEERCRED failed: {errno}")
            }
            Self::memlock_limit_failed(errno) => {
                write!(formatter, "RLIMIT_MEMLOCK failed: {errno}")
            }
        }
    }
}

impl std::error::Error for os_error_t {}

/// Maximum consecutive zero-length getrandom results tolerated before failing.
const MaxEmptyRandomReads: u32 = 8;

/// Fills `buffer` from the Linux kernel CSPRNG.
///
/// Uses `getrandom(2)` with no flags: it blocks until the kernel pool is initialized
/// and never falls back to a file. `EINTR` and short reads are retried.
pub fn fill_random(buffer: &mut [u8]) -> Result<(), os_error_t> {
    let mut remaining = buffer;
    let mut empty_reads = 0u32;
    while !remaining.is_empty() {
        match rustix::rand::getrandom(remaining, rustix::rand::GetRandomFlags::empty()) {
            Ok(0) => {
                empty_reads = empty_reads.saturating_add(1);
                if empty_reads >= MaxEmptyRandomReads {
                    return Err(os_error_t::random_stalled);
                }
            }
            Ok(count) => {
                let taken = core::mem::take(&mut remaining);
                remaining = taken.get_mut(count..).ok_or(os_error_t::random_stalled)?;
            }
            Err(Errno::INTR) => {}
            Err(errno) => return Err(os_error_t::random_failed(errno)),
        }
    }
    Ok(())
}

/// Disables core dumps and makes the process non-dumpable.
///
/// Sets `RLIMIT_CORE` to zero (soft and hard) and `PR_SET_DUMPABLE` to 0, which also
/// prevents non-privileged same-UID processes from attaching with ptrace. Verifies the
/// dumpable state afterwards. Fails closed on any error.
pub fn harden_process() -> Result<(), os_error_t> {
    rustix::process::setrlimit(
        rustix::process::Resource::Core,
        rustix::process::Rlimit {
            current: Some(0),
            maximum: Some(0),
        },
    )
    .map_err(os_error_t::core_limit_failed)?;
    rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)
        .map_err(os_error_t::dumpable_failed)?;
    match rustix::process::dumpable_behavior() {
        Ok(rustix::process::DumpableBehavior::NotDumpable) => Ok(()),
        Ok(_) => Err(os_error_t::dumpable_failed(Errno::PERM)),
        Err(errno) => Err(os_error_t::dumpable_failed(errno)),
    }
}

/// Reports whether the process is currently dumpable (ptrace-attachable by its UID).
pub fn is_dumpable() -> Result<bool, os_error_t> {
    match rustix::process::dumpable_behavior() {
        Ok(rustix::process::DumpableBehavior::NotDumpable) => Ok(false),
        Ok(_) => Ok(true),
        Err(errno) => Err(os_error_t::dumpable_failed(errno)),
    }
}

/// Locks all current and future pages of the process into RAM (`mlockall`).
///
/// Used by the vault so that transient secret copies (stack frames, library
/// temporaries) cannot be written to swap. Requires an adequate `RLIMIT_MEMLOCK`;
/// fails closed otherwise.
pub fn lock_all_memory() -> Result<(), os_error_t> {
    rustix::mm::mlockall(rustix::mm::MlockAllFlags::CURRENT | rustix::mm::MlockAllFlags::FUTURE)
        .map_err(os_error_t::lock_all_failed)
}

/// `RLIMIT_MEMLOCK` of this process, `(soft, hard)` in bytes; `None` is unlimited.
pub fn memlock_limit() -> (Option<u64>, Option<u64>) {
    let limit = rustix::process::getrlimit(rustix::process::Resource::Memlock);
    (limit.current, limit.maximum)
}

/// Raises the soft `RLIMIT_MEMLOCK` to `bytes` (never above the hard limit, which an
/// unprivileged process cannot raise). Fails if the hard limit is lower.
pub fn raise_memlock_soft_limit(bytes: u64) -> Result<(), os_error_t> {
    let (current, maximum) = memlock_limit();
    if current.is_none_or(|current| current >= bytes) {
        return Ok(());
    }
    if maximum.is_some_and(|maximum| maximum < bytes) {
        return Err(os_error_t::memlock_limit_failed(Errno::PERM));
    }
    rustix::process::setrlimit(
        rustix::process::Resource::Memlock,
        rustix::process::Rlimit {
            current: Some(bytes),
            maximum,
        },
    )
    .map_err(os_error_t::memlock_limit_failed)
}

/// Kernel-reported credentials of a connected Unix socket peer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct peer_credentials_t {
    /// Peer process ID at connect time (informational; PIDs can be reused).
    pub pid: i32,
    /// Peer effective user ID at connect time.
    pub uid: u32,
    /// Peer effective group ID at connect time.
    pub gid: u32,
}

/// Returns the `SO_PEERCRED` credentials of a connected Unix socket.
pub fn peer_credentials(socket: impl AsFd) -> Result<peer_credentials_t, os_error_t> {
    let credentials = rustix::net::sockopt::get_socket_peercred(socket)
        .map_err(os_error_t::peer_credentials_failed)?;
    Ok(peer_credentials_t {
        pid: credentials.pid.as_raw_nonzero().get(),
        uid: credentials.uid.as_raw(),
        gid: credentials.gid.as_raw(),
    })
}

/// Effective user ID of this process.
pub fn effective_uid() -> u32 {
    rustix::process::geteuid().as_raw()
}

/// Effective group ID of this process.
pub fn effective_gid() -> u32 {
    rustix::process::getegid().as_raw()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream;

    #[test]
    fn random_fills_and_differs() {
        let mut first = [0u8; 64];
        let mut second = [0u8; 64];
        fill_random(&mut first).unwrap();
        fill_random(&mut second).unwrap();
        assert_ne!(first, [0u8; 64]);
        assert_ne!(first, second);
    }

    #[test]
    fn random_fills_large_buffer() {
        let mut buffer = vec![0u8; 1 << 20];
        fill_random(&mut buffer).unwrap();
        let zeros = buffer.iter().filter(|byte| **byte == 0).count();
        // Expected ~4096 zero bytes in 1 MiB of uniform data; reject gross failure.
        assert!(zeros < 8192, "too many zero bytes: {zeros}");
    }

    #[test]
    fn random_empty_buffer_is_ok() {
        fill_random(&mut []).unwrap();
    }

    #[test]
    fn peer_credentials_report_this_process() {
        let (left, right) = UnixStream::pair().unwrap();
        let credentials = peer_credentials(&left).unwrap();
        assert_eq!(credentials.uid, effective_uid());
        assert_eq!(credentials.gid, effective_gid());
        assert_eq!(credentials.pid, i32::try_from(std::process::id()).unwrap());
        drop(right);
    }

    #[test]
    fn peer_credentials_fail_on_non_socket() {
        let file = std::fs::File::open("/proc/self/status").unwrap();
        assert!(matches!(
            peer_credentials(&file),
            Err(os_error_t::peer_credentials_failed(_))
        ));
    }

    #[test]
    fn memlock_limit_can_be_lowered_but_not_raised_past_the_hard_limit() {
        // Irreversible limit changes run in a child copy of this binary.
        if std::env::var("CHAM3_OS_ADAPTER_CHILD").as_deref() == Ok("memlock") {
            // Lower the hard limit first (always permitted), so the refusal to raise
            // past it is observed whatever the environment's limit was.
            let target = 1u64 << 20;
            rustix::process::setrlimit(
                rustix::process::Resource::Memlock,
                rustix::process::Rlimit {
                    current: Some(target / 2),
                    maximum: Some(target),
                },
            )
            .unwrap();
            assert_eq!(memlock_limit(), (Some(target / 2), Some(target)));
            raise_memlock_soft_limit(target).unwrap();
            assert_eq!(memlock_limit(), (Some(target), Some(target)));
            raise_memlock_soft_limit(target / 4).unwrap();
            assert_eq!(memlock_limit().0, Some(target), "never lowers");
            assert_eq!(
                raise_memlock_soft_limit(target + 1),
                Err(os_error_t::memlock_limit_failed(Errno::PERM))
            );
            return;
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::memlock_limit_can_be_lowered_but_not_raised_past_the_hard_limit",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("CHAM3_OS_ADAPTER_CHILD", "memlock")
            .output()
            .unwrap();
        assert!(output.status.success(), "child output {output:?}");
    }

    #[test]
    fn hardening_runs_in_child_process() {
        // Hardening is irreversible, so it is exercised in a child copy of this binary.
        if std::env::var("CHAM3_OS_ADAPTER_CHILD").as_deref() == Ok("harden") {
            assert!(
                is_dumpable().unwrap(),
                "positive control: dumpable before hardening"
            );
            harden_process().unwrap();
            assert!(!is_dumpable().unwrap());
            let status = std::fs::read_to_string("/proc/self/limits").unwrap();
            let core_line = status
                .lines()
                .find(|line| line.starts_with("Max core file size"))
                .unwrap();
            let fields: Vec<&str> = core_line.split_whitespace().collect();
            assert_eq!(fields[4], "0", "soft core limit: {core_line}");
            assert_eq!(fields[5], "0", "hard core limit: {core_line}");
            return;
        }
        // Child harness output is captured so it cannot be mistaken for this suite's.
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::hardening_runs_in_child_process",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("CHAM3_OS_ADAPTER_CHILD", "harden")
            .output()
            .unwrap();
        assert!(output.status.success(), "child output {output:?}");
    }
}
