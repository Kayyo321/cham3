//! M2-001 provider qualification probe.
//!
//! Runs published known-answer vectors (tests/m2/vectors/provider_kat.json) through
//! the candidate Rust providers selected for G1, plus behavioral checks: Ed25519
//! strict verification and tamper rejection, XChaCha20-Poly1305 in-place AEAD with
//! tag/AAD/ciphertext tamper rejection, Argon2id with secret/associated data and
//! parameter bounds, SHA-256, and the Linux CSPRNG. The same vectors are checked
//! independently by tests/m2/oracle/provider_oracle.py. This probe is qualification
//! evidence only; it is not product code and approves no wire format.
#![forbid(unsafe_code)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

use argon2::{Algorithm, Argon2, AssociatedData, Block, ParamsBuilder, Version};
use chacha20poly1305::aead::AeadInPlace;
use chacha20poly1305::{Key, KeyInit, Tag, XChaCha20Poly1305, XNonce};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

/// Committed vector file; both this probe and the Python oracle check it.
pub const VectorFile: &str = include_str!("../../../../tests/m2/vectors/provider_kat.json");

/// Probe failure. Names the failed check only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct probe_failure_t {
    /// Check identifier.
    pub check: String,
}

fn fail(check: &str) -> probe_failure_t {
    probe_failure_t {
        check: check.to_string(),
    }
}

/// Decodes lowercase/uppercase hex. Returns None on odd length or a non-hex digit.
pub fn decode_hex(text: &str) -> Option<Vec<u8>> {
    fn nibble(digit: u8) -> Option<u8> {
        match digit {
            b'0'..=b'9' => digit.checked_sub(b'0'),
            b'a'..=b'f' => digit
                .checked_sub(b'a')
                .and_then(|value| value.checked_add(10)),
            b'A'..=b'F' => digit
                .checked_sub(b'A')
                .and_then(|value| value.checked_add(10)),
            _ => None,
        }
    }
    let bytes = text.as_bytes();
    if bytes.len() % 2 != 0 {
        return None;
    }
    bytes
        .chunks_exact(2)
        .map(|pair| match pair {
            [high, low] => {
                let high = nibble(*high)?;
                let low = nibble(*low)?;
                high.checked_mul(16)
                    .and_then(|value| value.checked_add(low))
            }
            _ => None,
        })
        .collect()
}

fn hex_array<const Length: usize>(
    text: &str,
    check: &str,
) -> Result<[u8; Length], probe_failure_t> {
    let bytes = decode_hex(text).ok_or_else(|| fail(check))?;
    <[u8; Length]>::try_from(bytes.as_slice()).map_err(|_| fail(check))
}

/// Ed25519 published vector.
pub struct ed25519_vector_t {
    /// Source citation.
    pub source: &'static str,
    /// 32-byte seed (RFC 8032 secret key).
    pub seed: &'static str,
    /// Expected public key.
    pub public_key: &'static str,
    /// Message.
    pub message: &'static str,
    /// Expected signature.
    pub signature: &'static str,
}

/// RFC 8032 §7.1 Ed25519 vectors (mirrors provider_kat.json).
pub const Ed25519Vectors: [ed25519_vector_t; 4] = [
    ed25519_vector_t {
        source: "RFC 8032 section 7.1 TEST 1",
        seed: "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
        public_key: "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
        message: "",
        signature: "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
    },
    ed25519_vector_t {
        source: "RFC 8032 section 7.1 TEST 2",
        seed: "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb",
        public_key: "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
        message: "72",
        signature: "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00",
    },
    ed25519_vector_t {
        source: "RFC 8032 section 7.1 TEST 3",
        seed: "c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7",
        public_key: "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025",
        message: "af82",
        signature: "6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac18ff9b538d16f290ae67f760984dc6594a7c15e9716ed28dc027beceea1ec40a",
    },
    ed25519_vector_t {
        source: "RFC 8032 section 7.1 TEST SHA(abc)",
        seed: "833fe62409237b9d62ec77587520911e9a759cec1d19755b7da901b96dca3d42",
        public_key: "ec172b93ad5e563bf4932c70e1245034c35467ef2efd4d64ebf819683467e2bf",
        message: "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
        signature: "dc2a4459e7369633a52b1bf277839a00201009a3efbf3ecb69bea2186c26b58909351fc9ac90b3ecfdfbc7c66431e0303dca179c138ac17ad9bef1177331a704",
    },
];

/// Checks Ed25519 key derivation, deterministic signing and strict verification
/// against one vector, then checks that message, signature and key tampering fail.
pub fn check_ed25519(vector: &ed25519_vector_t) -> Result<(), probe_failure_t> {
    let seed: [u8; 32] = hex_array(vector.seed, "ed25519.seed")?;
    let expected_public: [u8; 32] = hex_array(vector.public_key, "ed25519.public_key")?;
    let expected_signature: [u8; 64] = hex_array(vector.signature, "ed25519.signature")?;
    let message = decode_hex(vector.message).ok_or_else(|| fail("ed25519.message"))?;

    let signing_key = SigningKey::from_bytes(&seed);
    let verifying_key = signing_key.verifying_key();
    if verifying_key.to_bytes() != expected_public {
        return Err(fail("ed25519.public_key_derivation"));
    }
    let signature = signing_key.sign(&message);
    if signature.to_bytes() != expected_signature {
        return Err(fail("ed25519.deterministic_signature"));
    }
    let parsed_key =
        VerifyingKey::from_bytes(&expected_public).map_err(|_| fail("ed25519.key_parse"))?;
    let parsed_signature = Signature::from_bytes(&expected_signature);
    parsed_key
        .verify_strict(&message, &parsed_signature)
        .map_err(|_| fail("ed25519.verify_strict"))?;

    // Negative controls: any changed message, signature byte or key must fail.
    let mut changed_message = message.clone();
    changed_message.push(0);
    if parsed_key
        .verify_strict(&changed_message, &parsed_signature)
        .is_ok()
    {
        return Err(fail("ed25519.changed_message_accepted"));
    }
    for index in [0usize, 31, 32, 63] {
        let mut changed = expected_signature;
        if let Some(byte) = changed.get_mut(index) {
            *byte ^= 0x01;
        }
        if parsed_key
            .verify_strict(&message, &Signature::from_bytes(&changed))
            .is_ok()
        {
            return Err(fail("ed25519.changed_signature_accepted"));
        }
    }
    let other_key = SigningKey::from_bytes(&[0x42u8; 32]).verifying_key();
    if other_key.verify_strict(&message, &parsed_signature).is_ok() {
        return Err(fail("ed25519.substituted_key_accepted"));
    }
    Ok(())
}

/// Checks that verify_strict rejects a non-canonical S (S + L) that the RFC 8032
/// cofactorless equation would otherwise accept.
pub fn check_ed25519_rejects_noncanonical_s() -> Result<(), probe_failure_t> {
    // Group order L, little-endian.
    const GroupOrder: [u8; 32] = [
        0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde,
        0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x10,
    ];
    let signing_key = SigningKey::from_bytes(&[0x07u8; 32]);
    let message = b"cham3 noncanonical s probe";
    let signature = signing_key.sign(message).to_bytes();
    let mut changed = signature;
    let mut carry = 0u16;
    for index in 0..32usize {
        let (Some(s_byte), Some(order_byte)) = (
            signature.get(index.saturating_add(32)),
            GroupOrder.get(index),
        ) else {
            return Err(fail("ed25519.noncanonical_index"));
        };
        let sum = u16::from(*s_byte)
            .checked_add(u16::from(*order_byte))
            .and_then(|value| value.checked_add(carry))
            .ok_or_else(|| fail("ed25519.noncanonical_sum"))?;
        let low = u8::try_from(sum & 0xff).map_err(|_| fail("ed25519.noncanonical_low"))?;
        if let Some(target) = changed.get_mut(index.saturating_add(32)) {
            *target = low;
        }
        carry = sum >> 8;
    }
    if carry != 0 {
        return Err(fail("ed25519.noncanonical_carry"));
    }
    if signing_key
        .verifying_key()
        .verify_strict(message, &Signature::from_bytes(&changed))
        .is_ok()
    {
        return Err(fail("ed25519.noncanonical_s_accepted"));
    }
    Ok(())
}

/// XChaCha20-Poly1305 published vector (draft-irtf-cfrg-xchacha-03 A.3.1).
pub const XchachaVector: [&str; 6] = [
    "808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f",
    "404142434445464748494a4b4c4d4e4f5051525354555657",
    "50515253c0c1c2c3c4c5c6c7",
    "4c616469657320616e642047656e746c656d656e206f662074686520636c617373206f66202739393a204966204920636f756c64206f6666657220796f75206f6e6c79206f6e652074697020666f7220746865206675747572652c2073756e73637265656e20776f756c642062652069742e",
    "bd6d179d3e83d43b9576579493c0e939572a1700252bfaccbed2902c21396cbb731c7f1b0b4aa6440bf3a82f4eda7e39ae64c6708c54c216cb96b72e1213b4522f8c9ba40db5d945b11b69b982c1bb9e3f3fac2bc369488f76b2383565d3fff921f9664c97637da9768812f615c68b13b52e",
    "c0875924c1c7987947deafd8780acf49",
];

/// Checks in-place XChaCha20-Poly1305 encryption/decryption against the published
/// vector, encrypting into a guarded secret region, and checks tamper rejection.
pub fn check_xchacha20poly1305() -> Result<(), probe_failure_t> {
    let [
        key_hex,
        nonce_hex,
        aad_hex,
        plaintext_hex,
        ciphertext_hex,
        tag_hex,
    ] = XchachaVector;
    let key: [u8; 32] = hex_array(key_hex, "xchacha.key")?;
    let nonce: [u8; 24] = hex_array(nonce_hex, "xchacha.nonce")?;
    let aad = decode_hex(aad_hex).ok_or_else(|| fail("xchacha.aad"))?;
    let plaintext = decode_hex(plaintext_hex).ok_or_else(|| fail("xchacha.plaintext"))?;
    let expected_ciphertext =
        decode_hex(ciphertext_hex).ok_or_else(|| fail("xchacha.ciphertext"))?;
    let expected_tag: [u8; 16] = hex_array(tag_hex, "xchacha.tag")?;

    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    let mut region = cham3_secure_memory::secret_region_t::new(plaintext.len())
        .map_err(|_| fail("xchacha.secret_region"))?;
    region.as_mut_slice().copy_from_slice(&plaintext);
    let tag = cipher
        .encrypt_in_place_detached(XNonce::from_slice(&nonce), &aad, region.as_mut_slice())
        .map_err(|_| fail("xchacha.encrypt"))?;
    if region.as_slice() != expected_ciphertext.as_slice() {
        return Err(fail("xchacha.ciphertext_mismatch"));
    }
    if tag.as_slice() != expected_tag {
        return Err(fail("xchacha.tag_mismatch"));
    }
    cipher
        .decrypt_in_place_detached(
            XNonce::from_slice(&nonce),
            &aad,
            region.as_mut_slice(),
            Tag::from_slice(&expected_tag),
        )
        .map_err(|_| fail("xchacha.decrypt"))?;
    if region.as_slice() != plaintext.as_slice() {
        return Err(fail("xchacha.plaintext_mismatch"));
    }

    // Negative controls: changed ciphertext, tag, AAD and nonce must not decrypt.
    let attempts: [xchacha_attempt_t; 4] = [
        (
            flip_first(&expected_ciphertext),
            expected_tag,
            aad.clone(),
            nonce,
            "xchacha.changed_ciphertext_accepted",
        ),
        (
            expected_ciphertext.clone(),
            flip_array(expected_tag),
            aad.clone(),
            nonce,
            "xchacha.changed_tag_accepted",
        ),
        (
            expected_ciphertext.clone(),
            expected_tag,
            flip_first(&aad),
            nonce,
            "xchacha.changed_aad_accepted",
        ),
        (
            expected_ciphertext.clone(),
            expected_tag,
            aad.clone(),
            flip_array(nonce),
            "xchacha.changed_nonce_accepted",
        ),
    ];
    for (mut ciphertext, tag, attempt_aad, attempt_nonce, check) in attempts {
        if cipher
            .decrypt_in_place_detached(
                XNonce::from_slice(&attempt_nonce),
                &attempt_aad,
                &mut ciphertext,
                Tag::from_slice(&tag),
            )
            .is_ok()
        {
            return Err(fail(check));
        }
    }
    Ok(())
}

/// (ciphertext, tag, aad, nonce, failure check) for an XChaCha tamper attempt.
type xchacha_attempt_t = (Vec<u8>, [u8; 16], Vec<u8>, [u8; 24], &'static str);

fn flip_first(bytes: &[u8]) -> Vec<u8> {
    let mut changed = bytes.to_vec();
    if let Some(first) = changed.first_mut() {
        *first ^= 0x01;
    }
    changed
}

fn flip_array<const Length: usize>(mut bytes: [u8; Length]) -> [u8; Length] {
    if let Some(first) = bytes.first_mut() {
        *first ^= 0x01;
    }
    bytes
}

/// One Argon2id invocation.
#[derive(Clone, Copy)]
pub struct argon2_case_t<'case> {
    /// Password bytes.
    pub password: &'case [u8],
    /// Salt bytes.
    pub salt: &'case [u8],
    /// Optional secret (RFC 9106 K).
    pub secret: Option<&'case [u8]>,
    /// Optional associated data (RFC 9106 X).
    pub associated_data: Option<&'case [u8]>,
    /// Memory cost in KiB.
    pub memory_kib: u32,
    /// Number of passes.
    pub passes: u32,
    /// Number of lanes.
    pub lanes: u32,
}

/// Runs Argon2id with explicit memory provided by the caller.
pub fn argon2id(case: &argon2_case_t<'_>, output: &mut [u8]) -> Result<(), probe_failure_t> {
    let argon2_case_t {
        password,
        salt,
        secret,
        associated_data,
        memory_kib,
        passes,
        lanes,
    } = *case;
    let mut builder = ParamsBuilder::new();
    builder
        .m_cost(memory_kib)
        .t_cost(passes)
        .p_cost(lanes)
        .output_len(output.len());
    if let Some(data) = associated_data {
        builder.data(AssociatedData::new(data).map_err(|_| fail("argon2.associated_data"))?);
    }
    let params = builder.build().map_err(|_| fail("argon2.params"))?;
    let block_count = params.block_count();
    let hasher = match secret {
        Some(secret) => {
            Argon2::new_with_secret(secret, Algorithm::Argon2id, Version::V0x13, params)
                .map_err(|_| fail("argon2.secret"))?
        }
        None => Argon2::new(Algorithm::Argon2id, Version::V0x13, params),
    };
    let mut blocks = vec![Block::default(); block_count];
    hasher
        .hash_password_into_with_memory(password, salt, output, &mut blocks)
        .map_err(|_| fail("argon2.hash"))
}

/// RFC 9106 §5.3 Argon2id vector.
pub fn check_argon2id_rfc9106() -> Result<(), probe_failure_t> {
    let expected: [u8; 32] = hex_array(
        "0d640df58d78766c08c037a34a8b53c9d01ef0452d75b65eb52520e96b01e659",
        "argon2.tag",
    )?;
    let rfc_case = argon2_case_t {
        password: &[0x01; 32],
        salt: &[0x02; 16],
        secret: Some(&[0x03; 8]),
        associated_data: Some(&[0x04; 12]),
        memory_kib: 32,
        passes: 3,
        lanes: 4,
    };
    let mut output = [0u8; 32];
    argon2id(&rfc_case, &mut output)?;
    if output != expected {
        return Err(fail("argon2.rfc9106_mismatch"));
    }
    // Negative controls: each changed input changes the output.
    let changes: [(argon2_case_t<'_>, &str); 4] = [
        (
            argon2_case_t {
                password: &[0x01; 31],
                ..rfc_case
            },
            "argon2.password_ignored",
        ),
        (
            argon2_case_t {
                salt: &[0x12; 16],
                ..rfc_case
            },
            "argon2.salt_ignored",
        ),
        (
            argon2_case_t {
                secret: Some(&[0x13; 8]),
                ..rfc_case
            },
            "argon2.secret_ignored",
        ),
        (
            argon2_case_t {
                associated_data: Some(&[0x14; 12]),
                ..rfc_case
            },
            "argon2.associated_data_ignored",
        ),
    ];
    for (case, check) in changes {
        let mut changed = [0u8; 32];
        argon2id(&case, &mut changed)?;
        if changed == expected {
            return Err(fail(check));
        }
    }
    Ok(())
}

/// Second Argon2id vector without secret/AD, compared by the libargon2 oracle.
pub const Argon2idOracleCase: (&str, &str, u32, u32, u32) =
    ("cham3 oracle password", "cham3-oracle-salt", 65536, 3, 1);

/// Computes the oracle-comparison Argon2id case (64 MiB, t=3, p=1, 32-byte output).
pub fn argon2id_oracle_case() -> Result<[u8; 32], probe_failure_t> {
    let (password, salt, memory_kib, passes, lanes) = Argon2idOracleCase;
    let case = argon2_case_t {
        password: password.as_bytes(),
        salt: salt.as_bytes(),
        secret: None,
        associated_data: None,
        memory_kib,
        passes,
        lanes,
    };
    let mut output = [0u8; 32];
    argon2id(&case, &mut output)?;
    Ok(output)
}

/// SHA-256 vectors from FIPS 180-2.
pub fn check_sha256() -> Result<(), probe_failure_t> {
    let cases = [
        (
            "616263",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            "",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
    ];
    for (message, digest) in cases {
        let message = decode_hex(message).ok_or_else(|| fail("sha256.message"))?;
        let expected: [u8; 32] = hex_array(digest, "sha256.digest")?;
        let actual: [u8; 32] = Sha256::digest(&message).into();
        if actual != expected {
            return Err(fail("sha256.mismatch"));
        }
    }
    Ok(())
}

/// Checks constant-time equality helper behavior used for tag/digest comparison.
pub fn check_constant_time_equality() -> Result<(), probe_failure_t> {
    use subtle::ConstantTimeEq;
    let left = [7u8; 32];
    let mut right = left;
    if !bool::from(left.ct_eq(&right)) {
        return Err(fail("subtle.equal_rejected"));
    }
    if let Some(last) = right.last_mut() {
        *last ^= 1;
    }
    if bool::from(left.ct_eq(&right)) {
        return Err(fail("subtle.unequal_accepted"));
    }
    Ok(())
}

/// Runs every provider check. Returns the identifiers that passed.
pub fn run_all() -> Result<Vec<&'static str>, probe_failure_t> {
    let mut passed = Vec::new();
    for vector in &Ed25519Vectors {
        check_ed25519(vector)?;
    }
    passed.push("ed25519_rfc8032_vectors");
    check_ed25519_rejects_noncanonical_s()?;
    passed.push("ed25519_strict_noncanonical_s");
    check_xchacha20poly1305()?;
    passed.push("xchacha20poly1305_draft_vector");
    check_argon2id_rfc9106()?;
    passed.push("argon2id_rfc9106_vector");
    check_sha256()?;
    passed.push("sha256_fips180_vectors");
    check_constant_time_equality()?;
    passed.push("subtle_constant_time_eq");
    let mut random = [0u8; 32];
    cham3_os_adapter::fill_random(&mut random).map_err(|_| fail("random.getrandom"))?;
    if random == [0u8; 32] {
        return Err(fail("random.all_zero"));
    }
    passed.push("linux_getrandom");
    Ok(passed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc8032_vectors_pass() {
        for vector in &Ed25519Vectors {
            check_ed25519(vector)
                .unwrap_or_else(|failure| panic!("{}: {failure:?}", vector.source));
        }
    }

    #[test]
    fn strict_verification_rejects_noncanonical_s() {
        check_ed25519_rejects_noncanonical_s().unwrap();
    }

    #[test]
    fn corrupted_vector_is_detected() {
        // Sensitivity control: a one-character change in the expected signature fails.
        let mut signature = Ed25519Vectors[0].signature.to_string();
        signature.replace_range(0..1, "f");
        let changed = ed25519_vector_t {
            signature: Box::leak(signature.into_boxed_str()),
            ..Ed25519Vectors[0]
        };
        assert_eq!(
            check_ed25519(&changed),
            Err(fail("ed25519.deterministic_signature"))
        );
    }

    #[test]
    fn xchacha_vector_passes() {
        check_xchacha20poly1305().unwrap();
    }

    #[test]
    fn argon2id_rfc9106_vector_passes() {
        check_argon2id_rfc9106().unwrap();
    }

    #[test]
    fn sha256_vectors_pass() {
        check_sha256().unwrap();
    }

    #[test]
    fn constant_time_equality_passes() {
        check_constant_time_equality().unwrap();
    }

    #[test]
    fn vectors_mirror_committed_vector_file() {
        for vector in &Ed25519Vectors {
            for value in [
                vector.seed,
                vector.public_key,
                vector.signature,
                vector.source,
            ] {
                assert!(
                    VectorFile.contains(value),
                    "{value} missing from provider_kat.json"
                );
            }
        }
        for value in XchachaVector {
            assert!(
                VectorFile.contains(value),
                "{value} missing from provider_kat.json"
            );
        }
        assert!(
            VectorFile.contains("0d640df58d78766c08c037a34a8b53c9d01ef0452d75b65eb52520e96b01e659")
        );
    }

    #[test]
    fn hex_decoder_rejects_malformed_input() {
        assert_eq!(decode_hex("0"), None);
        assert_eq!(decode_hex("zz"), None);
        assert_eq!(decode_hex("0aFF"), Some(vec![0x0a, 0xff]));
    }

    #[test]
    fn run_all_reports_every_check() {
        assert_eq!(run_all().unwrap().len(), 7);
    }
}
