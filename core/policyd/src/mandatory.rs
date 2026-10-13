//! Mandatory policy (PROJECT §3.3, ADR 0007 §1 rule 1): prohibitions that no grant
//! overrides, forbidden network modes and the emergency restriction overlay.
//!
//! This is the M2 **configuration model** of mandatory policy: the inputs evaluation
//! needs, supplied when policyd starts. It is not an administrator policy language,
//! and loading, authenticating and updating it are later work. Raising the policy
//! epoch and setting or releasing an emergency restriction are restrictive changes
//! that move to M2-006 with the restrictive write path.

use std::collections::BTreeSet;

use cham3_contract_format::schema::resource_category_t;

/// The two network modes a grant can make eligible (wire format §6.10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum network_mode_t {
    direct,
    tor,
}

impl network_mode_t {
    pub fn as_str(self) -> &'static str {
        match self {
            network_mode_t::direct => "direct",
            network_mode_t::tor => "tor",
        }
    }

    pub fn parse(text: &str) -> Option<network_mode_t> {
        match text {
            "direct" => Some(network_mode_t::direct),
            "tor" => Some(network_mode_t::tor),
            _ => None,
        }
    }
}

/// One prohibition. `None` fields match everything; a prohibition with every field
/// `None` but the category forbids the whole category.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct prohibition_t {
    pub category: resource_category_t,
    pub resource_id: Option<String>,
    pub operation: Option<String>,
    /// Only for subjects of this owner UID.
    pub owner_uid: Option<u32>,
}

impl prohibition_t {
    fn forbids(
        &self,
        owner_uid: u32,
        category: resource_category_t,
        resource_id: &str,
        operation: &str,
    ) -> bool {
        self.category == category
            && self
                .resource_id
                .as_deref()
                .is_none_or(|id| id == resource_id)
            && self.operation.as_deref().is_none_or(|op| op == operation)
            && self.owner_uid.is_none_or(|uid| uid == owner_uid)
    }
}

/// The mandatory policy evaluation applies before any grant.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct mandatory_policy_t {
    pub prohibitions: Vec<prohibition_t>,
    /// Modes no lineage may use (a Tor-only system forbids `direct`, PROJECT §3.3).
    pub forbidden_network_modes: BTreeSet<network_mode_t>,
    /// Emergency restriction of every user (ADR 0007 §2).
    pub emergency_global: bool,
    /// Emergency restriction of these users.
    pub emergency_users: BTreeSet<u32>,
}

impl mandatory_policy_t {
    /// Whether a prohibition forbids this access for a subject of `owner_uid`.
    pub fn forbids(
        &self,
        owner_uid: u32,
        category: resource_category_t,
        resource_id: &str,
        operation: &str,
    ) -> bool {
        self.prohibitions
            .iter()
            .any(|prohibition| prohibition.forbids(owner_uid, category, resource_id, operation))
            || (category == resource_category_t::network
                && network_mode_t::parse(operation)
                    .is_some_and(|mode| self.forbidden_network_modes.contains(&mode)))
    }

    /// Whether an emergency restriction covers `owner_uid`.
    pub fn emergency(&self, owner_uid: u32) -> bool {
        self.emergency_global || self.emergency_users.contains(&owner_uid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prohibitions_match_their_fields_only() {
        let policy = mandatory_policy_t {
            prohibitions: vec![
                prohibition_t {
                    category: resource_category_t::camera,
                    resource_id: None,
                    operation: None,
                    owner_uid: None,
                },
                prohibition_t {
                    category: resource_category_t::filesystem,
                    resource_id: Some("home:Secrets".to_owned()),
                    operation: Some("write".to_owned()),
                    owner_uid: Some(1001),
                },
            ],
            forbidden_network_modes: [network_mode_t::direct].into_iter().collect(),
            emergency_global: false,
            emergency_users: [1002].into_iter().collect(),
        };
        let (cameras, files, networks) = (
            resource_category_t::camera,
            resource_category_t::filesystem,
            resource_category_t::network,
        );
        assert!(policy.forbids(1001, cameras, "any", "capture"));
        assert!(policy.forbids(1001, files, "home:Secrets", "write"));
        assert!(!policy.forbids(1001, files, "home:Secrets", "read"));
        assert!(!policy.forbids(1002, files, "home:Secrets", "write"));
        assert!(!policy.forbids(1001, files, "home:Other", "write"));
        assert!(policy.forbids(1001, networks, "network_mode", "direct"));
        assert!(!policy.forbids(1001, networks, "network_mode", "tor"));
        assert!(policy.emergency(1002));
        assert!(!policy.emergency(1001));
        assert!(
            mandatory_policy_t {
                emergency_global: true,
                ..mandatory_policy_t::default()
            }
            .emergency(1001)
        );
    }
}
