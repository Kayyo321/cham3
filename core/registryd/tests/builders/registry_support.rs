//! Shared registryd test support: a fixed clock, roles, a disposable store root and
//! the in-process services over it. Used by every registryd integration test.

use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cham3_identityd::caller::{caller_role_t, service_name_t};
use cham3_identityd::outcome::service_error_t;
use cham3_identityd::service::identity_service_t;
use cham3_registryd::journal::{journal_io_t, system_io_t};
use cham3_registryd::record::{parse_journal, placement_t};
use cham3_registryd::registry::{
    registry_clock_t, registry_config_t, registry_identity_store_t, registry_t,
};

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

/// 2026-10-13T09:00:00Z.
pub struct fixed_clock_t;

impl registry_clock_t for fixed_clock_t {
    fn utc_seconds(&self) -> i64 {
        1_791_882_000
    }
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
        registry_t::open(root, owner_of(root), config, io, Box::new(fixed_clock_t))
            .expect("registry start"),
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
