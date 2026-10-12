"""Independent provider oracle for M2-001 qualification.

Checks tests/m2/vectors/provider_kat.json with implementations that share no code
with the Rust providers under qualification:

* Ed25519: OpenSSL (python3-cryptography) and libsodium (python3-nacl);
* XChaCha20-Poly1305: libsodium (python3-nacl);
* Argon2id: the reference libargon2 (python3-argon2) for the oracle-comparison case;
  libargon2's Python binding exposes no secret/associated-data inputs, so the RFC 9106
  section 5.3 vector is checked only by the Rust probe and is reported as such;
* SHA-256: hashlib (OpenSSL).

Prints one "PASS <check>" or "FAIL <check>" line per check and
"ARGON2ID_ORACLE_CASE <hex>" for comparison with the Rust probe. Exit status is 0
only if every check passes. Usage: python3 -I provider_oracle.py <vector-file>
"""

import hashlib
import json
import sys

import argon2.low_level
import nacl.bindings
import nacl.signing
from cryptography.hazmat.primitives.asymmetric import ed25519
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat

# Mirrors cham3_provider_probe::Argon2idOracleCase.
Argon2idOracleCase = (b"cham3 oracle password", b"cham3-oracle-salt", 65536, 3, 1)


def check_ed25519(vector):
    seed = bytes.fromhex(vector["seed"])
    message = bytes.fromhex(vector["message"])
    expected_public = bytes.fromhex(vector["public_key"])
    expected_signature = bytes.fromhex(vector["signature"])
    openssl_key = ed25519.Ed25519PrivateKey.from_private_bytes(seed)
    openssl_public = openssl_key.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)
    if openssl_public != expected_public:
        return False
    if openssl_key.sign(message) != expected_signature:
        return False
    openssl_key.public_key().verify(expected_signature, message)
    sodium_key = nacl.signing.SigningKey(seed)
    if bytes(sodium_key.verify_key) != expected_public:
        return False
    if sodium_key.sign(message).signature != expected_signature:
        return False
    changed = bytearray(expected_signature)
    changed[0] ^= 1
    try:
        openssl_key.public_key().verify(bytes(changed), message)
        return False
    except Exception:  # noqa: BLE001 - any verification failure is the expected result
        pass
    return True


def check_xchacha(vector):
    key = bytes.fromhex(vector["key"])
    nonce = bytes.fromhex(vector["nonce"])
    aad = bytes.fromhex(vector["aad"])
    plaintext = bytes.fromhex(vector["plaintext"])
    expected = bytes.fromhex(vector["ciphertext"]) + bytes.fromhex(vector["tag"])
    sealed = nacl.bindings.crypto_aead_xchacha20poly1305_ietf_encrypt(plaintext, aad, nonce, key)
    if sealed != expected:
        return False
    opened = nacl.bindings.crypto_aead_xchacha20poly1305_ietf_decrypt(sealed, aad, nonce, key)
    if opened != plaintext:
        return False
    changed = bytearray(sealed)
    changed[-1] ^= 1
    try:
        nacl.bindings.crypto_aead_xchacha20poly1305_ietf_decrypt(bytes(changed), aad, nonce, key)
        return False
    except Exception:  # noqa: BLE001
        return True


def check_sha256(vector):
    return hashlib.sha256(bytes.fromhex(vector["message"])).hexdigest() == vector["digest"]


def argon2id_oracle_case():
    password, salt, memory_kib, passes, lanes = Argon2idOracleCase
    return argon2.low_level.hash_secret_raw(
        secret=password,
        salt=salt,
        time_cost=passes,
        memory_cost=memory_kib,
        parallelism=lanes,
        hash_len=32,
        type=argon2.low_level.Type.ID,
        version=19,
    )


def main(arguments):
    if len(arguments) != 2:
        print("usage: provider_oracle.py <vector-file>", file=sys.stderr)
        return 2
    with open(arguments[1], "r", encoding="utf-8") as handle:
        vectors = json.load(handle)
    results = []
    for vector in vectors["ed25519"]:
        results.append(("ed25519 " + vector["source"], check_ed25519(vector)))
    for vector in vectors["xchacha20poly1305"]:
        results.append(("xchacha20poly1305 " + vector["source"], check_xchacha(vector)))
    for vector in vectors["sha256"]:
        results.append(("sha256 " + vector["source"], check_sha256(vector)))
    if not vectors["ed25519"] or not vectors["xchacha20poly1305"] or not vectors["sha256"]:
        results.append(("vector_file_nonempty", False))
    failed = False
    for name, passed in results:
        print(("PASS " if passed else "FAIL ") + name)
        failed = failed or not passed
    print("NOT_CHECKED argon2id " + vectors["argon2id"][0]["source"] + " (no secret/AD API in libargon2 binding)")
    print("ARGON2ID_ORACLE_CASE " + argon2id_oracle_case().hex())
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
