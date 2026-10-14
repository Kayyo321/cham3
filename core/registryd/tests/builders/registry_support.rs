//! Shared registryd test support: a settable clock, roles, a disposable store root and
//! the in-process services over it. Used by every registryd integration test.

use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use cham3_identityd::caller::{caller_role_t, service_name_t};
use cham3_identityd::outcome::service_error_t;
use cham3_identityd::service::identity_service_t;
use cham3_registryd::clock::{time_sample_t, time_source_t};
use cham3_registryd::journal::{journal_io_t, system_io_t};
use cham3_registryd::record::{parse_journal, placement_t};
use cham3_registryd::registry::{registry_config_t, registry_identity_store_t, registry_t};

use super::{commit_t, test_user_t};

pub const Contractd: caller_role_t = caller_role_t::service(service_name_t::contractd);
pub const Policyd: caller_role_t = caller_role_t::service(service_name_t::policyd);
pub const Identityd: caller_role_t = caller_role_t::service(service_name_t::identityd);
pub const Vaultd: caller_role_t = caller_role_t::service(service_name_t::vaultd);
pub const Admin: caller_role_t = caller_role_t::admin;

/// The error code of a refusal; panics on success.
pub fn code(result: Result<impl std::fmt::Debug, service_error_t>) -> String {
    match result {
        Ok(value) => panic!("expected a refusal, got {value:?}"),
        Err(error) => error.to_string(),
    }
}

/// 2026-10-13T09:00:00Z (`T1`).
pub const T1Seconds: u64 = 1_791_882_000;
/// CLOCK_BOOTTIME when a test clock starts.
pub const StartBoottime: u64 = 1_000;

/// A settable pair of clocks shared by every service of a test (ADR 0007 §5). Time
/// passing moves both clocks; a wall-clock step moves only CLOCK_REALTIME; `None`
/// makes every sample fail.
#[derive(Clone, Debug)]
pub struct test_time_t {
    pub sample: Arc<Mutex<Option<time_sample_t>>>,
}

impl test_time_t {
    /// Both clocks at `realtime` and [`StartBoottime`].
    pub fn at(realtime: u64) -> test_time_t {
        test_time_t {
            sample: Arc::new(Mutex::new(Some(time_sample_t {
                realtime,
                boottime: StartBoottime,
            }))),
        }
    }

    pub fn current(&self) -> Option<time_sample_t> {
        *self.sample.lock().expect("test clock")
    }

    /// `seconds` pass: both clocks advance.
    pub fn advance(&self, seconds: u64) {
        let mut sample = self.sample.lock().expect("test clock");
        if let Some(sample) = sample.as_mut() {
            sample.realtime += seconds;
            sample.boottime += seconds;
        }
    }

    /// Time passes until CLOCK_REALTIME reads `realtime` (never backward).
    pub fn pass_until(&self, realtime: u64) {
        let current = self.current().expect("test clock").realtime;
        self.advance(realtime.saturating_sub(current));
    }

    /// The wall clock is set to `realtime`; CLOCK_BOOTTIME does not move.
    pub fn step_wall_clock(&self, realtime: u64) {
        let mut sample = self.sample.lock().expect("test clock");
        if let Some(sample) = sample.as_mut() {
            sample.realtime = realtime;
        }
    }

    /// Samples fail (`None`) or are restored.
    pub fn set_available(&self, available: bool, realtime: u64, boottime: u64) {
        *self.sample.lock().expect("test clock") =
            available.then_some(time_sample_t { realtime, boottime });
    }
}

impl time_source_t for test_time_t {
    fn sample(&self) -> Option<time_sample_t> {
        self.current()
    }
}

/// Both clocks fixed at `T1`.
pub fn fixed_clock() -> Box<test_time_t> {
    Box::new(test_time_t::at(T1Seconds))
}

/// A fresh `0700` directory under the temporary directory, named after the test.
pub fn temp_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("cham3_registryd_{label}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("temp root");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("mode");
    root
}

pub fn owner_of(root: &Path) -> u32 {
    fs::metadata(root).expect("root").uid()
}

pub fn open_with(
    root: &Path,
    config: registry_config_t,
    io: Box<dyn journal_io_t>,
) -> Arc<registry_t> {
    Arc::new(
        registry_t::open(root, owner_of(root), config, io, fixed_clock()).expect("registry start"),
    )
}

pub fn open(root: &Path) -> Arc<registry_t> {
    open_with(root, registry_config_t::default(), Box::new(system_io_t))
}

/// A freshly started identityd over the registry.
pub fn identity_over(registry: &Arc<registry_t>) -> identity_service_t<registry_identity_store_t> {
    identity_service_t::start(registry_identity_store_t {
        registry: Arc::clone(registry),
    })
    .expect("identityd start")
}

/// Enrolls `user` through identityd, which persists through the registry.
pub fn enroll(registry: &Arc<registry_t>, user: &test_user_t) {
    let mut identity = identity_over(registry);
    identity
        .register_enrollment(
            Vaultd,
            user.uid,
            &user.enrollment,
            &user.persistent_certificate,
            &user.temporary_certificate,
        )
        .expect("enrollment");
}

pub fn admit(
    registry: &registry_t,
    user: &test_user_t,
    commit: &commit_t,
) -> Result<placement_t, service_error_t> {
    registry
        .admit(Contractd, user.uid, &commit.envelope)
        .map(|receipt| {
            assert_eq!(receipt.commit_id, commit.commit_id);
            receipt.placement
        })
}

pub fn journal_path(root: &Path, uid: u32) -> PathBuf {
    root.join("users").join(uid.to_string()).join("journal.c3j")
}

/// The chain-verified records of a user's journal.
pub fn records_of(root: &Path, uid: u32) -> Vec<Vec<u8>> {
    parse_journal(&fs::read(journal_path(root, uid)).expect("read"), uid)
        .expect("parse")
        .records
}
