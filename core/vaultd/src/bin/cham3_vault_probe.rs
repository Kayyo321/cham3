//! Key-object differential probe for tests/m2/oracle/vault_object_oracle.py.
//!
//! **Qualification tool only; never packaged or deployed.** It exposes the key-object
//! format, the bounded Argon2id derivation and XChaCha20-Poly1305 sealing of
//! `cham3_vaultd` to an independent oracle (libargon2, libsodium and the oracle's own
//! deterministic CBOR encoder) over stdin, one request per line, with synthetic
//! seeds and passphrases chosen by the oracle:
//!
//! * `seal <seed> <passphrase> <salt> <nonce> <class> <uid> <owner_user_id|self>
//!   <created> <memory_kib> <passes> <lanes>` → `ok <object>` or `err <code>`;
//! * `open <object> <passphrase>` → `ok <key_id>` or `err <code>`.
//!
//! Byte fields are lowercase hex. The derivation enforces the vault's bounds and the
//! default locked budget, as the vault does.
#![forbid(unsafe_code)]

use std::io::{BufRead, Write};

use cham3_contract_format::hash::key_id;
use cham3_identityd::index::key_class_t;
use cham3_secure_memory::secret_region_t;
use cham3_vaultd::kdf::{DefaultLockedBudgetBytes, derive_kek, kdf_error_t, kdf_params_t};
use cham3_vaultd::key_object::{key_metadata_t, key_object_error_t, key_object_t, public_key_of};

fn hex_decode(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).ok()?;
            u8::from_str_radix(pair, 16).ok()
        })
        .collect()
}

fn hex_encode(bytes: &[u8]) -> String {
    const Digits: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::new();
    for byte in bytes {
        for nibble in [byte >> 4, byte & 0x0f] {
            if let Some(digit) = Digits.get(usize::from(nibble)) {
                text.push(char::from(*digit));
            }
        }
    }
    text
}

fn fixed<const Length: usize>(text: &str) -> Result<[u8; Length], &'static str> {
    let bytes = hex_decode(text).ok_or("malformed_request")?;
    <[u8; Length]>::try_from(bytes.as_slice()).map_err(|_| "malformed_request")
}

fn secret(text: &str) -> Result<secret_region_t, &'static str> {
    let bytes = hex_decode(text).ok_or("malformed_request")?;
    let mut region = secret_region_t::new(bytes.len()).map_err(|_| "secret_memory")?;
    region.as_mut_slice().copy_from_slice(&bytes);
    Ok(region)
}

fn kdf_code(error: kdf_error_t) -> &'static str {
    match error {
        kdf_error_t::out_of_bounds => "kdf_out_of_bounds",
        kdf_error_t::over_budget => "kdf_over_budget",
        kdf_error_t::secret_memory => "secret_memory",
        kdf_error_t::workspace_size | kdf_error_t::derivation => "kdf_failed",
    }
}

fn object_code(error: key_object_error_t) -> &'static str {
    match error {
        key_object_error_t::malformed => "malformed",
        key_object_error_t::authentication_failed => "authentication_failed",
        key_object_error_t::key_id_mismatch => "key_id_mismatch",
        key_object_error_t::secret_memory => "secret_memory",
        key_object_error_t::seal_failed => "seal_failed",
    }
}

fn number<number_t: std::str::FromStr>(text: &str) -> Result<number_t, &'static str> {
    text.parse().map_err(|_| "malformed_request")
}

fn seal(fields: &[&str]) -> Result<String, &'static str> {
    let [
        seed,
        passphrase,
        salt,
        nonce,
        class,
        uid,
        owner,
        created,
        memory,
        passes,
        lanes,
    ] = fields
    else {
        return Err("malformed_request");
    };
    let seed = secret(seed)?;
    let passphrase = secret(passphrase)?;
    let public_key = public_key_of(&seed).ok_or("malformed_request")?;
    let authority_class = match *class {
        "identity" => key_class_t::identity,
        "persistent" => key_class_t::persistent,
        "temporary" => key_class_t::temporary,
        _ => return Err("malformed_request"),
    };
    let key = key_id(&public_key);
    let kdf = kdf_params_t {
        memory_kib: number(memory)?,
        passes: number(passes)?,
        lanes: number(lanes)?,
    };
    let metadata = key_metadata_t {
        authority_class,
        owner_uid: number(uid)?,
        owner_user_id: if *owner == "self" { key } else { fixed(owner)? },
        key_id: key,
        created: (*created).to_owned(),
        kdf,
        salt: fixed(salt)?,
        nonce: fixed(nonce)?,
    };
    let kek = derive_kek(
        &kdf,
        DefaultLockedBudgetBytes,
        passphrase.as_slice(),
        &metadata.salt,
    )
    .map_err(kdf_code)?;
    let object = key_object_t::seal(metadata, &kek, &seed).map_err(object_code)?;
    Ok(hex_encode(&object.encode()))
}

fn open(fields: &[&str]) -> Result<String, &'static str> {
    let [object, passphrase] = fields else {
        return Err("malformed_request");
    };
    let bytes = hex_decode(object).ok_or("malformed_request")?;
    let passphrase = secret(passphrase)?;
    let object = key_object_t::decode(&bytes).map_err(object_code)?;
    let kek = derive_kek(
        &object.metadata.kdf,
        DefaultLockedBudgetBytes,
        passphrase.as_slice(),
        &object.metadata.salt,
    )
    .map_err(kdf_code)?;
    object.open(&kek).map_err(object_code)?;
    Ok(hex_encode(&object.metadata.key_id))
}

fn main() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            std::process::exit(2);
        };
        let fields: Vec<&str> = line.split(' ').collect();
        let answer = match fields.split_first() {
            Some((&"seal", rest)) => seal(rest),
            Some((&"open", rest)) => open(rest),
            _ => Err("malformed_request"),
        };
        let written = match answer {
            Ok(value) => writeln!(stdout, "ok {value}"),
            Err(code) => writeln!(stdout, "err {code}"),
        };
        if written.is_err() {
            std::process::exit(2);
        }
    }
}
