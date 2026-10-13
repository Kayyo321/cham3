//! Test builders for registryd: synthetic users (identityd's builders) and signed ECR
//! commits built from scratch.
//!
//! Every key is a public synthetic test key and must never sign anything outside
//! tests.

#![allow(dead_code, unused_imports)]

#[path = "../../../identityd/tests/builders/mod.rs"]
mod identity_builders;

pub use identity_builders::*;
mod registry_support;
pub use registry_support::*;

use cham3_contract_format::hash::{digest_t, lineage_id};
use cham3_contract_format::object_kind_t;
use cham3_contract_format::schema::{
    actor_t, authority_class_t, authorization_t, commit_payload_t, commit_type_t, contract_state_t,
    delegation_t, delta_t, grant_t, identity_change_t, identity_reference_t, initiating_program_t,
    lifetime_kind_t, lifetime_t, policy_binding_t, random_id_t, resource_category_t, resource_t,
    subject_t,
};
use cham3_contract_format::text::is_valid_time;

/// One synthetic application installation.
#[derive(Clone, Debug)]
pub struct app_t {
    pub app_id: String,
    pub installation_id: random_id_t,
    pub package_identity: String,
    pub executable_sha256: digest_t,
}

pub fn app(label: &str) -> app_t {
    app_t {
        app_id: format!("org.example.{label}"),
        installation_id: synthetic_id(&format!("installation {label}")),
        package_identity: format!("deb:{label}:amd64"),
        executable_sha256: cham3_contract_format::hash::sha256(label.as_bytes()),
    }
}

pub fn subject(user: &test_user_t, app: &app_t) -> subject_t {
    subject_t {
        owner_uid: user.uid,
        owner_user_id: user.identity.key_id,
        app_id: app.app_id.clone(),
        installation_id: app.installation_id,
        package_identity: app.package_identity.clone(),
        executable_sha256: app.executable_sha256,
        runtime_scope: None,
    }
}

pub fn persistent_lifetime() -> lifetime_t {
    lifetime_t {
        kind: lifetime_kind_t::persistent,
        effective_start: T1.to_owned(),
        expiration: None,
        session_id: None,
        usage_limit: None,
    }
}

pub fn timed_lifetime(expiration: &str) -> lifetime_t {
    assert!(is_valid_time(expiration));
    lifetime_t {
        kind: lifetime_kind_t::timed,
        effective_start: T1.to_owned(),
        expiration: Some(expiration.to_owned()),
        session_id: None,
        usage_limit: None,
    }
}

pub fn session_lifetime(session: &str) -> lifetime_t {
    lifetime_t {
        kind: lifetime_kind_t::session,
        effective_start: T1.to_owned(),
        expiration: None,
        session_id: Some(synthetic_id(&format!("session {session}"))),
        usage_limit: None,
    }
}

pub fn one_shot_lifetime() -> lifetime_t {
    lifetime_t {
        kind: lifetime_kind_t::one_shot,
        effective_start: T1.to_owned(),
        expiration: None,
        session_id: None,
        usage_limit: Some(1),
    }
}

fn no_delegation() -> delegation_t {
    delegation_t {
        permitted: false,
        max_depth: 0,
        max_operations: Vec::new(),
        parent_grant_id: None,
    }
}

/// A filesystem grant of `user` over `resource_id`.
pub fn file_grant(
    label: &str,
    uid: u32,
    resource_id: &str,
    operations: &[&str],
    lifetime: lifetime_t,
) -> grant_t {
    grant_t {
        grant_id: synthetic_id(&format!("grant {label}")),
        lifetime,
        resource: resource_t {
            category: resource_category_t::filesystem,
            resource_id: resource_id.to_owned(),
            resource_owner_uid: uid,
            operations: operations
                .iter()
                .map(|operation| (*operation).to_owned())
                .collect(),
        },
        delegation: no_delegation(),
    }
}

/// A network grant (`direct` or `tor`).
pub fn network_grant(label: &str, uid: u32, mode: &str, lifetime: lifetime_t) -> grant_t {
    grant_t {
        grant_id: synthetic_id(&format!("grant {label}")),
        lifetime,
        resource: resource_t {
            category: resource_category_t::network,
            resource_id: "network_mode".to_owned(),
            resource_owner_uid: uid,
            operations: vec![mode.to_owned()],
        },
        delegation: no_delegation(),
    }
}

/// A signed commit.
#[derive(Clone, Debug)]
pub struct commit_t {
    pub envelope: Vec<u8>,
    pub commit_id: digest_t,
    pub payload: commit_payload_t,
}

impl commit_t {
    pub fn state(&self) -> &contract_state_t {
        &self.payload.result_state
    }
}

/// A commit under construction.
#[derive(Clone, Debug)]
pub struct draft_t {
    payload: commit_payload_t,
    base: contract_state_t,
    temporary: bool,
}

/// A new commit of `commit_type` in `app`'s lineage of `user`. Defaults: no parents,
/// the empty base state, persistent authority, policy epoch 1, timestamp `T1`.
pub fn draft(user: &test_user_t, app: &app_t, commit_type: commit_type_t, label: &str) -> draft_t {
    let subject = subject(user, app);
    draft_t {
        payload: commit_payload_t {
            commit_type,
            timestamp: T1.to_owned(),
            actor: actor_t {
                uid: user.uid,
                user_id: user.identity.key_id,
            },
            key_id: user.persistent.key_id,
            authority_class: authority_class_t::persistent,
            authorization: authorization_t {
                request_id: synthetic_id(&format!("request {label}")),
                session_id: synthetic_id(&format!("authorization session {label}")),
            },
            policy: policy_binding_t {
                epoch: 1,
                version: "m2-test".to_owned(),
                security_profile: "default".to_owned(),
                mandatory_constraints: Vec::new(),
            },
            lineage_id: lineage_id(
                &subject.owner_user_id,
                &subject.app_id,
                &subject.installation_id,
            ),
            subject,
            initiating_program: initiating_program_t::cham3_dev_cli,
            parents: Vec::new(),
            base_state_hash: contract_state_t::default().state_hash(),
            merge_base: None,
            restore_reference: None,
            message: format!("test commit {label}"),
            delta: delta_t {
                add_grants: Vec::new(),
                remove_grant_ids: Vec::new(),
                identity_change: None,
            },
            result_state: contract_state_t::default(),
        },
        base: contract_state_t::default(),
        temporary: false,
    }
}

impl draft_t {
    /// Single parent; the base is the parent's result state.
    pub fn on(mut self, parent: &commit_t) -> draft_t {
        self.payload.parents = vec![parent.commit_id];
        self.base = parent.state().clone();
        self
    }

    /// A merge of `base` (the trunk head) with temporary heads `others`.
    pub fn merge(mut self, base: &commit_t, others: &[&commit_t]) -> draft_t {
        let mut parents: Vec<digest_t> = others.iter().map(|other| other.commit_id).collect();
        parents.push(base.commit_id);
        parents.sort_unstable();
        self.payload.parents = parents;
        self.payload.merge_base = Some(base.commit_id);
        self.base = base.state().clone();
        self
    }

    pub fn restore_from(mut self, reference: &commit_t) -> draft_t {
        self.payload.restore_reference = Some(reference.commit_id);
        self
    }

    pub fn add(mut self, mut grants: Vec<grant_t>) -> draft_t {
        grants.sort_by(|first, second| first.grant_id.cmp(&second.grant_id));
        self.payload.delta.add_grants = grants;
        self
    }

    pub fn remove(mut self, mut ids: Vec<random_id_t>) -> draft_t {
        ids.sort_unstable();
        self.payload.delta.remove_grant_ids = ids;
        self
    }

    pub fn epoch(mut self, epoch: u64) -> draft_t {
        self.payload.policy.epoch = epoch;
        self
    }

    pub fn timestamp(mut self, timestamp: &str) -> draft_t {
        self.payload.timestamp = timestamp.to_owned();
        self
    }

    pub fn message(mut self, message: &str) -> draft_t {
        self.payload.message = message.to_owned();
        self
    }

    /// Signed by the user's temporary key under temporary authority.
    pub fn temporary_authority(mut self) -> draft_t {
        self.temporary = true;
        self
    }

    /// Replaces the subject's package identity and executable (identity transition).
    pub fn transition_to(mut self, package_identity: &str, executable: &digest_t) -> draft_t {
        let from = identity_reference_t {
            package_identity: self.payload.subject.package_identity.clone(),
            executable_sha256: self.payload.subject.executable_sha256,
        };
        self.payload.subject.package_identity = package_identity.to_owned();
        self.payload.subject.executable_sha256 = *executable;
        self.payload.delta.identity_change = Some(identity_change_t {
            from,
            to: identity_reference_t {
                package_identity: package_identity.to_owned(),
                executable_sha256: *executable,
            },
        });
        self
    }

    /// Edits the payload before signing (for negative cases).
    pub fn edit(mut self, edit: impl FnOnce(&mut commit_payload_t)) -> draft_t {
        edit(&mut self.payload);
        self
    }

    /// Computes the base hash and result state and signs.
    pub fn sign(mut self, user: &test_user_t) -> commit_t {
        let mut grants: Vec<grant_t> = self
            .base
            .grants
            .iter()
            .filter(|grant| {
                !self
                    .payload
                    .delta
                    .remove_grant_ids
                    .contains(&grant.grant_id)
            })
            .cloned()
            .collect();
        grants.extend(self.payload.delta.add_grants.iter().cloned());
        grants.sort_by(|first, second| first.grant_id.cmp(&second.grant_id));
        self.payload.base_state_hash = self.base.state_hash();
        self.payload.result_state = contract_state_t { grants };
        self.sign_as_is(user)
    }

    /// Computes the result state and signs with `key` under the payload's own
    /// `key_id` and authority class (for negative cases).
    pub fn sign_with(mut self, key: &test_key_t) -> commit_t {
        let mut grants = self.base.grants.clone();
        grants.extend(self.payload.delta.add_grants.iter().cloned());
        grants.sort_by(|first, second| first.grant_id.cmp(&second.grant_id));
        self.payload.base_state_hash = self.base.state_hash();
        self.payload.result_state = contract_state_t { grants };
        self.payload.key_id = key.key_id;
        let envelope = sign_object(object_kind_t::commit, key, &self.payload.to_value());
        commit_t {
            commit_id: cham3_contract_format::hash::sha256(&envelope),
            envelope,
            payload: self.payload,
        }
    }

    /// Signs the payload exactly as it stands.
    pub fn sign_as_is(mut self, user: &test_user_t) -> commit_t {
        let signer = if self.temporary {
            self.payload.key_id = user.temporary.key_id;
            self.payload.authority_class = authority_class_t::temporary;
            &user.temporary
        } else {
            &user.persistent
        };
        let envelope = sign_object(object_kind_t::commit, signer, &self.payload.to_value());
        commit_t {
            commit_id: cham3_contract_format::hash::sha256(&envelope),
            envelope,
            payload: self.payload,
        }
    }
}
