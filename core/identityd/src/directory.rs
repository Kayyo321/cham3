//! All enrolled users' key-authority indexes, and verification of identity objects.

use std::collections::BTreeMap;

use cham3_contract_format::dcbor::{SignedObjectTag, decode, value_t};
use cham3_contract_format::envelope::public_key_is_valid;
use cham3_contract_format::hash::{digest_t, key_id};
use cham3_contract_format::schema::commit_payload_t;
use cham3_contract_format::{
    key_resolver_t, layer_t, object_kind_t, payload_t, verified_object_t, verify_object,
};

use crate::index::{
    check_commit_authority, key_class_t, key_resolution_t, naming_t, user_index_t, user_record_t,
};
use crate::outcome::{authority, authority_code_t, ipc, ipc_code_t, service_result_t};

/// What registering a verified identity object would do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum identity_effect_t {
    enroll,
    certify,
    invalidate,
    /// The identical object is already durable at this journal sequence.
    already_present(u64),
}

/// One journal record that identityd applies (identity objects and the registry
/// records that change key authority), in journal order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum journal_record_t {
    identity {
        kind: object_kind_t,
        object: Vec<u8>,
    },
    user(user_record_t),
    recovery_keys(naming_t),
}

/// The key-authority state of every enrolled user.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct identity_directory_t {
    users: BTreeMap<u32, user_index_t>,
    key_owner: BTreeMap<digest_t, u32>,
}

impl identity_directory_t {
    pub fn new() -> identity_directory_t {
        identity_directory_t::default()
    }

    pub fn user(&self, uid: u32) -> Option<&user_index_t> {
        self.users.get(&uid)
    }

    pub fn users(&self) -> impl Iterator<Item = &user_index_t> {
        self.users.values()
    }

    pub fn owner_of(&self, key_id: &digest_t) -> Option<u32> {
        self.key_owner.get(key_id).copied()
    }

    /// Removes a user whose journal could not be applied; every key of that user then
    /// resolves as unknown (fail closed) until a rebuild succeeds.
    pub fn forget_user(&mut self, uid: u32) {
        if self.users.remove(&uid).is_some() {
            self.key_owner.retain(|_, owner| *owner != uid);
        }
    }

    /// `resolve_key`: the key with its scope, or `None` for an unknown key.
    pub fn resolve(&self, key_id: &digest_t) -> Option<key_resolution_t> {
        self.users.get(&self.owner_of(key_id)?)?.resolve(key_id)
    }

    /// Layer 6 for a commit that passed layers 1–5.
    pub fn check_commit(&self, payload: &commit_payload_t) -> service_result_t<()> {
        let resolution = self
            .resolve(&payload.key_id)
            .ok_or(authority(authority_code_t::key_not_certified))?;
        check_commit_authority(&resolution, payload)
    }

    /// Layers 1–5 for an identity object. Certificates and invalidations must verify
    /// under a registered identity key. An enrollment is self-signed: its candidate
    /// key comes from a strict decode of the envelope, and the object is then verified
    /// from scratch under that key, so only a payload that the candidate key signed is
    /// accepted (the candidate is otherwise unused).
    pub fn verify_identity_object(
        &self,
        kind: object_kind_t,
        data: &[u8],
    ) -> service_result_t<verified_object_t> {
        let verified = match kind {
            object_kind_t::commit => return Err(ipc(ipc_code_t::malformed_request)),
            object_kind_t::identity_enrollment => {
                let resolver = enrollment_resolver_t {
                    candidate: enrollment_key_candidate(data),
                };
                verify_object(data, kind, &resolver)?
            }
            object_kind_t::key_certificate | object_kind_t::key_invalidation => {
                verify_object(data, kind, &identity_key_resolver_t { directory: self })?
            }
        };
        Ok(verified)
    }

    /// Checks a verified identity object for registration under `owner_uid`.
    pub fn check_identity_object(
        &self,
        owner_uid: u32,
        verified: &verified_object_t,
    ) -> service_result_t<identity_effect_t> {
        let not_certified = authority(authority_code_t::key_not_certified);
        let existing = self.users.get(&owner_uid);
        // A deleted user registers nothing, not even an identical retry (a vaultd
        // retry must not report success for a deleted account).
        if existing.is_some_and(|user| user.state() == crate::index::user_state_t::deleted) {
            return Err(authority(authority_code_t::user_deleted));
        }
        if let Some(sequence) = existing.and_then(|user| user.object_sequence(&verified.object_id))
        {
            return Ok(identity_effect_t::already_present(sequence));
        }
        match &verified.payload {
            payload_t::identity_enrollment(enrollment) => {
                if enrollment.owner_uid != owner_uid {
                    return Err(ipc(ipc_code_t::owner_mismatch));
                }
                if existing.is_some() {
                    return Err(ipc(ipc_code_t::duplicate_identifier));
                }
                if self.key_owner.contains_key(&enrollment.user_id)
                    || !public_key_is_valid(&enrollment.public_key)
                {
                    return Err(not_certified);
                }
                Ok(identity_effect_t::enroll)
            }
            payload_t::key_certificate(certificate) => {
                let user = existing.ok_or(not_certified)?;
                user.check_certificate(certificate)?;
                if self.key_owner.contains_key(&certificate.subject_key_id) {
                    return Err(not_certified);
                }
                Ok(identity_effect_t::certify)
            }
            payload_t::key_invalidation(invalidation) => {
                existing
                    .ok_or(not_certified)?
                    .check_invalidation(invalidation)?;
                Ok(identity_effect_t::invalidate)
            }
            payload_t::commit(_) => Err(ipc(ipc_code_t::malformed_request)),
        }
    }

    /// Applies a verified identity object that `check_identity_object` accepted and
    /// that is durable at journal `sequence`.
    pub fn apply_identity_object(
        &mut self,
        owner_uid: u32,
        verified: &verified_object_t,
        sequence: u64,
    ) -> service_result_t<()> {
        match self.check_identity_object(owner_uid, verified)? {
            identity_effect_t::already_present(_) => Ok(()),
            identity_effect_t::enroll => {
                let payload_t::identity_enrollment(enrollment) = &verified.payload else {
                    return Err(ipc(ipc_code_t::malformed_request));
                };
                self.users.insert(
                    owner_uid,
                    user_index_t::from_enrollment(enrollment, verified.object_id, sequence),
                );
                self.key_owner.insert(enrollment.user_id, owner_uid);
                Ok(())
            }
            identity_effect_t::certify => {
                let payload_t::key_certificate(certificate) = &verified.payload else {
                    return Err(ipc(ipc_code_t::malformed_request));
                };
                let user = self
                    .users
                    .get_mut(&owner_uid)
                    .ok_or(authority(authority_code_t::key_not_certified))?;
                user.apply_certificate(certificate, verified.object_id, sequence);
                self.key_owner.insert(certificate.subject_key_id, owner_uid);
                Ok(())
            }
            identity_effect_t::invalidate => {
                let payload_t::key_invalidation(invalidation) = &verified.payload else {
                    return Err(ipc(ipc_code_t::malformed_request));
                };
                self.users
                    .get_mut(&owner_uid)
                    .ok_or(authority(authority_code_t::key_not_certified))?
                    .apply_invalidation(invalidation, verified.object_id, sequence);
                Ok(())
            }
        }
    }

    /// Applies one journal record of `owner_uid` at `sequence` (rebuild and refresh).
    /// Every record is re-validated; a record that does not apply is reported, and the
    /// caller treats the user's journal as unusable (fail closed).
    pub fn apply_journal_record(
        &mut self,
        owner_uid: u32,
        record: &journal_record_t,
        sequence: u64,
    ) -> service_result_t<()> {
        match record {
            journal_record_t::identity { kind, object } => {
                let verified = self.verify_identity_object(*kind, object)?;
                self.apply_identity_object(owner_uid, &verified, sequence)
            }
            journal_record_t::user(user_record) => {
                let user = self
                    .users
                    .get_mut(&owner_uid)
                    .ok_or(authority(authority_code_t::key_not_certified))?;
                user.check_record(user_record)?;
                user.apply_record(user_record, sequence);
                Ok(())
            }
            journal_record_t::recovery_keys(naming) => {
                let user = self
                    .users
                    .get_mut(&owner_uid)
                    .ok_or(authority(authority_code_t::key_not_certified))?;
                if user.check_naming(owner_uid, naming)? == crate::index::naming_effect_t::name {
                    user.apply_naming(naming, sequence);
                }
                Ok(())
            }
        }
    }
}

/// Resolves only registered identity keys (the signers of certificates and
/// invalidations).
struct identity_key_resolver_t<'directory> {
    directory: &'directory identity_directory_t,
}

impl key_resolver_t for identity_key_resolver_t<'_> {
    fn resolve(&self, kid: &digest_t) -> Option<[u8; 32]> {
        let user = self.directory.users.get(&self.directory.owner_of(kid)?)?;
        let key = user.key(kid)?;
        (key.class == key_class_t::identity).then_some(key.public_key)
    }
}

/// Resolves a self-signed enrollment's `kid` to its candidate key.
struct enrollment_resolver_t {
    candidate: Option<[u8; 32]>,
}

impl key_resolver_t for enrollment_resolver_t {
    fn resolve(&self, kid: &digest_t) -> Option<[u8; 32]> {
        self.candidate.filter(|candidate| key_id(candidate) == *kid)
    }
}

/// The `public_key` field of an enrollment envelope, read with the strict decoder.
/// Returns `None` for anything that is not such an envelope; verification then fails
/// with the first error in layer order.
fn enrollment_key_candidate(data: &[u8]) -> Option<[u8; 32]> {
    let root = decode(data, layer_t::envelope_decode, true).ok()?;
    let value_t::tag(SignedObjectTag, content) = root else {
        return None;
    };
    let value_t::array(elements) = *content else {
        return None;
    };
    let Some(value_t::bytes(payload)) = elements.get(2) else {
        return None;
    };
    let value_t::map(pairs) = decode(payload, layer_t::payload_decode, false).ok()? else {
        return None;
    };
    pairs
        .into_iter()
        .find_map(|(key, value)| match (key, value) {
            (value_t::text(name), value_t::bytes(bytes)) if name == "public_key" => {
                <[u8; 32]>::try_from(bytes.as_slice()).ok()
            }
            _ => None,
        })
}
