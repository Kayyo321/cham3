//! Caller roles (service IPC §1).
//!
//! A role is derived only from kernel-reported peer credentials (`SO_PEERCRED`) and
//! the deployment's fixed account configuration. A caller-supplied UID, user id, key
//! id, program name or role field is never authentication.

use cham3_os_adapter::peer_credentials_t;

/// The five core services of PROJECT §14.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum service_name_t {
    identityd,
    vaultd,
    contractd,
    policyd,
    registryd,
}

impl service_name_t {
    pub const All: [service_name_t; 5] = [
        service_name_t::identityd,
        service_name_t::vaultd,
        service_name_t::contractd,
        service_name_t::policyd,
        service_name_t::registryd,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            service_name_t::identityd => "identityd",
            service_name_t::vaultd => "vaultd",
            service_name_t::contractd => "contractd",
            service_name_t::policyd => "policyd",
            service_name_t::registryd => "registryd",
        }
    }
}

/// The role a connection acts in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum caller_role_t {
    /// A core service, identified by its own system user.
    service(service_name_t),
    /// The setgid trusted development client, acting for the human UID `uid`.
    trusted_client { uid: u32 },
    /// UID 0: administrative operations only, never signing.
    admin,
    /// The labeled M2 stand-in for future consumers (launcherd, netd, brokers,
    /// sessiond); installed only by the test-only fixture package.
    fixture_consumer,
}

impl caller_role_t {
    pub fn is_service(self) -> bool {
        matches!(self, caller_role_t::service(_))
    }

    /// The human UID a trusted client acts for.
    pub fn trusted_uid(self) -> Option<u32> {
        match self {
            caller_role_t::trusted_client { uid } => Some(uid),
            _ => None,
        }
    }
}

/// Default regular-user UID range (Debian login.defs UID_MIN..UID_MAX).
pub const DefaultUidMin: u32 = 1000;
pub const DefaultUidMax: u32 = 60000;

/// Why a role configuration was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum role_config_error_t {
    /// The regular range is empty, starts at 0 or overlaps a system account.
    invalid_uid_range,
    /// Two accounts share a UID, or an account is UID 0 or a regular UID.
    account_conflict,
    /// The IPC group is GID 0.
    invalid_ipc_group,
}

/// The deployment's fixed account configuration (system users, the `_cham3-ipc`
/// group, the optional fixture account and the regular-user range).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct role_map_t {
    service_uids: [(service_name_t, u32); 5],
    ipc_gid: u32,
    fixture_uid: Option<u32>,
    uid_min: u32,
    uid_max: u32,
}

impl role_map_t {
    /// Validates the configuration; an inconsistent one is refused (fail closed).
    pub fn new(
        service_uids: [(service_name_t, u32); 5],
        ipc_gid: u32,
        fixture_uid: Option<u32>,
        uid_min: u32,
        uid_max: u32,
    ) -> Result<role_map_t, role_config_error_t> {
        if uid_min == 0 || uid_min > uid_max {
            return Err(role_config_error_t::invalid_uid_range);
        }
        if ipc_gid == 0 {
            return Err(role_config_error_t::invalid_ipc_group);
        }
        let mut accounts: Vec<u32> = service_uids.iter().map(|(_, uid)| *uid).collect();
        accounts.extend(fixture_uid);
        let names_in_order = service_uids
            .iter()
            .map(|(name, _)| *name)
            .eq(service_name_t::All);
        if !names_in_order {
            return Err(role_config_error_t::account_conflict);
        }
        for (position, uid) in accounts.iter().enumerate() {
            let regular = (uid_min..=uid_max).contains(uid);
            let repeated = accounts
                .iter()
                .skip(position.saturating_add(1))
                .any(|other| other == uid);
            if *uid == 0 || regular || repeated {
                return Err(role_config_error_t::account_conflict);
            }
        }
        Ok(role_map_t {
            service_uids,
            ipc_gid,
            fixture_uid,
            uid_min,
            uid_max,
        })
    }

    /// Maps kernel-reported credentials to exactly one role, or `None` (the
    /// connection is closed without a response).
    pub fn role_of(&self, peer: &peer_credentials_t) -> Option<caller_role_t> {
        if peer.uid == 0 {
            return Some(caller_role_t::admin);
        }
        if let Some((name, _)) = self.service_uids.iter().find(|(_, uid)| *uid == peer.uid) {
            return Some(caller_role_t::service(*name));
        }
        if self.fixture_uid == Some(peer.uid) {
            return Some(caller_role_t::fixture_consumer);
        }
        if (self.uid_min..=self.uid_max).contains(&peer.uid) && peer.gid == self.ipc_gid {
            return Some(caller_role_t::trusted_client { uid: peer.uid });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const Services: [(service_name_t, u32); 5] = [
        (service_name_t::identityd, 991),
        (service_name_t::vaultd, 992),
        (service_name_t::contractd, 993),
        (service_name_t::policyd, 994),
        (service_name_t::registryd, 995),
    ];
    const IpcGid: u32 = 990;

    fn peer(uid: u32, gid: u32) -> peer_credentials_t {
        peer_credentials_t {
            pid: 4242,
            uid,
            gid,
        }
    }

    fn map() -> role_map_t {
        role_map_t::new(Services, IpcGid, Some(989), DefaultUidMin, DefaultUidMax).unwrap()
    }

    #[test]
    fn roles_come_only_from_peer_credentials() {
        let map = map();
        assert_eq!(map.role_of(&peer(0, 0)), Some(caller_role_t::admin));
        for (name, uid) in Services {
            assert_eq!(
                map.role_of(&peer(uid, uid)),
                Some(caller_role_t::service(name))
            );
        }
        assert_eq!(
            map.role_of(&peer(989, 989)),
            Some(caller_role_t::fixture_consumer)
        );
        assert_eq!(
            map.role_of(&peer(1001, IpcGid)),
            Some(caller_role_t::trusted_client { uid: 1001 })
        );
        // An ordinary same-UID process (its own primary group) gets no role.
        assert_eq!(map.role_of(&peer(1001, 1001)), None);
        // A regular UID outside the range, or a system UID with the IPC group, gets none.
        assert_eq!(map.role_of(&peer(60001, IpcGid)), None);
        assert_eq!(map.role_of(&peer(999, IpcGid)), None);
        assert_eq!(map.role_of(&peer(65534, IpcGid)), None);
    }

    #[test]
    fn inconsistent_configurations_are_refused() {
        let mut duplicated = Services;
        duplicated[1].1 = 991;
        assert_eq!(
            role_map_t::new(duplicated, IpcGid, None, DefaultUidMin, DefaultUidMax),
            Err(role_config_error_t::account_conflict)
        );
        let mut regular = Services;
        regular[0].1 = 1500;
        assert_eq!(
            role_map_t::new(regular, IpcGid, None, DefaultUidMin, DefaultUidMax),
            Err(role_config_error_t::account_conflict)
        );
        let mut root = Services;
        root[4].1 = 0;
        assert_eq!(
            role_map_t::new(root, IpcGid, None, DefaultUidMin, DefaultUidMax),
            Err(role_config_error_t::account_conflict)
        );
        let mut reordered = Services;
        reordered.swap(0, 1);
        assert_eq!(
            role_map_t::new(reordered, IpcGid, None, DefaultUidMin, DefaultUidMax),
            Err(role_config_error_t::account_conflict)
        );
        assert_eq!(
            role_map_t::new(Services, IpcGid, Some(991), DefaultUidMin, DefaultUidMax),
            Err(role_config_error_t::account_conflict)
        );
        assert_eq!(
            role_map_t::new(Services, 0, None, DefaultUidMin, DefaultUidMax),
            Err(role_config_error_t::invalid_ipc_group)
        );
        assert_eq!(
            role_map_t::new(Services, IpcGid, None, 0, DefaultUidMax),
            Err(role_config_error_t::invalid_uid_range)
        );
        assert_eq!(
            role_map_t::new(Services, IpcGid, None, 2000, 1000),
            Err(role_config_error_t::invalid_uid_range)
        );
    }
}
