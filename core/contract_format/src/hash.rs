//! Hashes and derived identifiers (wire format §5).

use sha2::{Digest, Sha256};

use crate::dcbor::{encode, value_t};

/// A 32-byte identifier or digest.
pub type digest_t = [u8; 32];

/// Domain tag of `key_id`.
pub const KeyIdDomain: &str = "cham3/key-id/ed25519/v1";
/// Domain tag of `lineage_id`.
pub const LineageDomain: &str = "cham3/lineage/v1";
/// Domain tag of `state_hash`.
pub const StateDomain: &str = "cham3/state/v1";

/// SHA-256 of the input.
pub fn sha256(data: &[u8]) -> digest_t {
    Sha256::digest(data).into()
}

/// `domain_hash(tag, data) = SHA-256(tag || 0x00 || data)`. Every tag used by cham3
/// is ASCII without NUL, which keeps the construction prefix-free.
pub fn domain_hash(tag: &str, data: &[u8]) -> digest_t {
    let mut hasher = Sha256::new();
    hasher.update(tag.as_bytes());
    hasher.update([0u8]);
    hasher.update(data);
    hasher.finalize().into()
}

/// `key_id` of an Ed25519 public key; a user's `user_id` is the key id of its
/// identity key.
pub fn key_id(public_key: &[u8; 32]) -> digest_t {
    domain_hash(KeyIdDomain, public_key)
}

/// `lineage_id = domain_hash("cham3/lineage/v1", dcbor([owner_user_id, app_id, installation_id]))`.
pub fn lineage_id(owner_user_id: &digest_t, app_id: &str, installation_id: &[u8; 16]) -> digest_t {
    let item = value_t::array(vec![
        value_t::bytes_of(owner_user_id),
        value_t::text_of(app_id),
        value_t::bytes_of(installation_id),
    ]);
    domain_hash(LineageDomain, &encode(&item))
}

/// `state_hash = domain_hash("cham3/state/v1", dcbor(contract_state))` over the
/// deterministic encoding of a state value.
pub fn state_hash_of_encoding(encoded_state: &[u8]) -> digest_t {
    domain_hash(StateDomain, encoded_state)
}

/// `commit_id` (and the object id of every signed object): SHA-256 of the complete
/// stored envelope bytes.
pub fn object_id(envelope: &[u8]) -> digest_t {
    sha256(envelope)
}
