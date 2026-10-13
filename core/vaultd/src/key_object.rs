//! Encrypted key objects (ADR 0005 §4, PROJECT §4.3).
//!
//! One object per key, a `cham3_dcbor_v1` map:
//! `format_version` 1, `key_type` `"ed25519_seed"`, `authority_class`, `owner_uid`,
//! `owner_user_id`, `key_id`, `created`, `kdf` {`algorithm` `"argon2id"`, `version`
//! 19, `memory_kib`, `passes`, `lanes`}, `algorithm` `"xchacha20poly1305"`, `salt`
//! (16 bytes, unique per object), `nonce` (24 bytes), `ciphertext` (32-byte encrypted
//! seed) and `tag` (16-byte Poly1305 tag).
//!
//! The AEAD associated data is the deterministic encoding of the map without
//! `ciphertext` and `tag`, so every metadata field is authenticated. After decryption
//! the public key is recomputed and `key_id(public) == key_id` is required
//! (substitution detection). Plaintext seeds exist only in secret regions.

use std::collections::BTreeMap;

use chacha20poly1305::aead::AeadInPlace;
use chacha20poly1305::{Key, KeyInit, Tag, XChaCha20Poly1305, XNonce};
use cham3_contract_format::dcbor::{decode, encode, value_t};
use cham3_contract_format::hash::{digest_t, key_id};
use cham3_contract_format::layer_t;
use cham3_contract_format::text::is_valid_time;
use cham3_identityd::index::key_class_t;
use cham3_secure_memory::secret_region_t;
use ed25519_dalek::SigningKey;

use crate::kdf::{ArgonVersion, SaltBytes, kdf_params_t};

pub const FormatVersion: u64 = 1;
pub const KeyType: &str = "ed25519_seed";
pub const KdfAlgorithm: &str = "argon2id";
pub const AeadAlgorithm: &str = "xchacha20poly1305";
pub const NonceBytes: usize = 24;
pub const TagBytes: usize = 16;
pub const SeedBytes: usize = 32;
/// Largest key object read from disk (the encoding is about 400 bytes).
pub const MaxObjectBytes: usize = 1024;

/// Why a key object could not be decoded, sealed or opened. Contains no secret data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum key_object_error_t {
    /// Not a well-formed key object of this format.
    malformed,
    /// The AEAD rejected the object (wrong passphrase or tampered object).
    authentication_failed,
    /// The decrypted seed is not the key the object names.
    key_id_mismatch,
    /// A secret region could not be allocated.
    secret_memory,
    /// Encryption failed.
    seal_failed,
}

/// Everything in a key object except the encrypted seed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct key_metadata_t {
    pub authority_class: key_class_t,
    pub owner_uid: u32,
    pub owner_user_id: digest_t,
    pub key_id: digest_t,
    pub created: String,
    pub kdf: kdf_params_t,
    pub salt: [u8; SaltBytes],
    pub nonce: [u8; NonceBytes],
}

/// One encrypted key object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct key_object_t {
    pub metadata: key_metadata_t,
    pub ciphertext: [u8; SeedBytes],
    pub tag: [u8; TagBytes],
}

fn text(value: &str) -> value_t {
    value_t::text_of(value)
}

fn class_text(class: key_class_t) -> &'static str {
    class.as_str()
}

fn class_of(text: &str) -> Option<key_class_t> {
    match text {
        "identity" => Some(key_class_t::identity),
        "persistent" => Some(key_class_t::persistent),
        "temporary" => Some(key_class_t::temporary),
        _ => None,
    }
}

impl key_metadata_t {
    fn pairs(&self) -> Vec<(value_t, value_t)> {
        let kdf = value_t::map(vec![
            (text("algorithm"), text(KdfAlgorithm)),
            (text("version"), value_t::unsigned(ArgonVersion)),
            (
                text("memory_kib"),
                value_t::unsigned(u64::from(self.kdf.memory_kib)),
            ),
            (
                text("passes"),
                value_t::unsigned(u64::from(self.kdf.passes)),
            ),
            (text("lanes"), value_t::unsigned(u64::from(self.kdf.lanes))),
        ]);
        vec![
            (text("format_version"), value_t::unsigned(FormatVersion)),
            (text("key_type"), text(KeyType)),
            (
                text("authority_class"),
                text(class_text(self.authority_class)),
            ),
            (
                text("owner_uid"),
                value_t::unsigned(u64::from(self.owner_uid)),
            ),
            (
                text("owner_user_id"),
                value_t::bytes_of(&self.owner_user_id),
            ),
            (text("key_id"), value_t::bytes_of(&self.key_id)),
            (text("created"), text(&self.created)),
            (text("kdf"), kdf),
            (text("algorithm"), text(AeadAlgorithm)),
            (text("salt"), value_t::bytes_of(&self.salt)),
            (text("nonce"), value_t::bytes_of(&self.nonce)),
        ]
    }

    /// The AEAD associated data: the encoding of every metadata field.
    pub fn associated_data(&self) -> Vec<u8> {
        encode(&value_t::map(self.pairs()))
    }
}

impl key_object_t {
    /// Encrypts `seed` under `kek` (an Argon2id output for this object's salt).
    pub fn seal(
        metadata: key_metadata_t,
        kek: &secret_region_t,
        seed: &secret_region_t,
    ) -> Result<key_object_t, key_object_error_t> {
        if kek.len() != 32 || seed.len() != SeedBytes {
            return Err(key_object_error_t::seal_failed);
        }
        let aad = metadata.associated_data();
        // Encrypt in a secret region; only the ciphertext leaves it.
        let mut buffer =
            secret_region_t::new(SeedBytes).map_err(|_| key_object_error_t::secret_memory)?;
        buffer.as_mut_slice().copy_from_slice(seed.as_slice());
        let cipher = XChaCha20Poly1305::new(Key::from_slice(kek.as_slice()));
        let tag = cipher
            .encrypt_in_place_detached(
                XNonce::from_slice(&metadata.nonce),
                &aad,
                buffer.as_mut_slice(),
            )
            .map_err(|_| key_object_error_t::seal_failed)?;
        let ciphertext = <[u8; SeedBytes]>::try_from(buffer.as_slice())
            .map_err(|_| key_object_error_t::seal_failed)?;
        let tag = <[u8; TagBytes]>::try_from(tag.as_slice())
            .map_err(|_| key_object_error_t::seal_failed)?;
        Ok(key_object_t {
            metadata,
            ciphertext,
            tag,
        })
    }

    /// Decrypts the seed into a secret region and checks it is the named key.
    pub fn open(&self, kek: &secret_region_t) -> Result<secret_region_t, key_object_error_t> {
        if kek.len() != 32 {
            return Err(key_object_error_t::authentication_failed);
        }
        let aad = self.metadata.associated_data();
        let mut seed =
            secret_region_t::new(SeedBytes).map_err(|_| key_object_error_t::secret_memory)?;
        seed.as_mut_slice().copy_from_slice(&self.ciphertext);
        let cipher = XChaCha20Poly1305::new(Key::from_slice(kek.as_slice()));
        cipher
            .decrypt_in_place_detached(
                XNonce::from_slice(&self.metadata.nonce),
                &aad,
                seed.as_mut_slice(),
                Tag::from_slice(&self.tag),
            )
            .map_err(|_| key_object_error_t::authentication_failed)?;
        if public_key_of(&seed)
            .map(|public| key_id(&public))
            .ok_or(key_object_error_t::key_id_mismatch)?
            != self.metadata.key_id
        {
            return Err(key_object_error_t::key_id_mismatch);
        }
        Ok(seed)
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut pairs = self.metadata.pairs();
        pairs.push((text("ciphertext"), value_t::bytes_of(&self.ciphertext)));
        pairs.push((text("tag"), value_t::bytes_of(&self.tag)));
        encode(&value_t::map(pairs))
    }

    /// Strict decode: the exact field set, types and constant values.
    pub fn decode(bytes: &[u8]) -> Result<key_object_t, key_object_error_t> {
        if bytes.len() > MaxObjectBytes {
            return Err(key_object_error_t::malformed);
        }
        let value = decode(bytes, layer_t::payload_decode, false)
            .map_err(|_| key_object_error_t::malformed)?;
        let mut fields = closed_map(
            value,
            &[
                "format_version",
                "key_type",
                "authority_class",
                "owner_uid",
                "owner_user_id",
                "key_id",
                "created",
                "kdf",
                "algorithm",
                "salt",
                "nonce",
                "ciphertext",
                "tag",
            ],
        )?;
        let mut kdf = closed_map(
            take(&mut fields, "kdf")?,
            &["algorithm", "version", "memory_kib", "passes", "lanes"],
        )?;
        expect_unsigned(take(&mut fields, "format_version")?, FormatVersion)?;
        expect_text(take(&mut fields, "key_type")?, KeyType)?;
        expect_text(take(&mut fields, "algorithm")?, AeadAlgorithm)?;
        expect_text(take(&mut kdf, "algorithm")?, KdfAlgorithm)?;
        expect_unsigned(take(&mut kdf, "version")?, ArgonVersion)?;
        let authority_class = class_of(&text_value(take(&mut fields, "authority_class")?)?)
            .ok_or(key_object_error_t::malformed)?;
        let created = text_value(take(&mut fields, "created")?)?;
        if !is_valid_time(&created) {
            return Err(key_object_error_t::malformed);
        }
        let metadata = key_metadata_t {
            authority_class,
            owner_uid: u32_value(take(&mut fields, "owner_uid")?)?,
            owner_user_id: fixed_bytes(take(&mut fields, "owner_user_id")?)?,
            key_id: fixed_bytes(take(&mut fields, "key_id")?)?,
            created,
            kdf: kdf_params_t {
                memory_kib: u32_value(take(&mut kdf, "memory_kib")?)?,
                passes: u32_value(take(&mut kdf, "passes")?)?,
                lanes: u32_value(take(&mut kdf, "lanes")?)?,
            },
            salt: fixed_bytes(take(&mut fields, "salt")?)?,
            nonce: fixed_bytes(take(&mut fields, "nonce")?)?,
        };
        Ok(key_object_t {
            metadata,
            ciphertext: fixed_bytes(take(&mut fields, "ciphertext")?)?,
            tag: fixed_bytes(take(&mut fields, "tag")?)?,
        })
    }
}

/// The Ed25519 public key of a seed held in a secret region. The `SigningKey` copy
/// is zeroized on drop by ed25519-dalek (`zeroize` feature).
pub fn public_key_of(seed: &secret_region_t) -> Option<[u8; 32]> {
    let bytes = <&[u8; SeedBytes]>::try_from(seed.as_slice()).ok()?;
    Some(SigningKey::from_bytes(bytes).verifying_key().to_bytes())
}

fn closed_map(
    value: value_t,
    names: &[&str],
) -> Result<BTreeMap<String, value_t>, key_object_error_t> {
    let value_t::map(pairs) = value else {
        return Err(key_object_error_t::malformed);
    };
    let mut fields = BTreeMap::new();
    for (key, item) in pairs {
        let value_t::text(name) = key else {
            return Err(key_object_error_t::malformed);
        };
        if !names.contains(&name.as_str()) {
            return Err(key_object_error_t::malformed);
        }
        fields.insert(name, item);
    }
    if fields.len() != names.len() {
        return Err(key_object_error_t::malformed);
    }
    Ok(fields)
}

fn take(fields: &mut BTreeMap<String, value_t>, name: &str) -> Result<value_t, key_object_error_t> {
    fields.remove(name).ok_or(key_object_error_t::malformed)
}

fn text_value(value: value_t) -> Result<String, key_object_error_t> {
    match value {
        value_t::text(text) => Ok(text),
        _ => Err(key_object_error_t::malformed),
    }
}

fn expect_text(value: value_t, expected: &str) -> Result<(), key_object_error_t> {
    if text_value(value)? == expected {
        Ok(())
    } else {
        Err(key_object_error_t::malformed)
    }
}

fn expect_unsigned(value: value_t, expected: u64) -> Result<(), key_object_error_t> {
    match value {
        value_t::unsigned(number) if number == expected => Ok(()),
        _ => Err(key_object_error_t::malformed),
    }
}

fn u32_value(value: value_t) -> Result<u32, key_object_error_t> {
    match value {
        value_t::unsigned(number) => {
            u32::try_from(number).map_err(|_| key_object_error_t::malformed)
        }
        _ => Err(key_object_error_t::malformed),
    }
}

fn fixed_bytes<const Length: usize>(value: value_t) -> Result<[u8; Length], key_object_error_t> {
    match value {
        value_t::bytes(bytes) => {
            <[u8; Length]>::try_from(bytes.as_slice()).map_err(|_| key_object_error_t::malformed)
        }
        _ => Err(key_object_error_t::malformed),
    }
}
