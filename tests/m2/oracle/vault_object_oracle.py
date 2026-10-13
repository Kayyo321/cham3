"""Independent oracle for the vault's encrypted key objects (ADR 0005 section 4).

Builds and opens key objects with implementations that share no code with the Rust
vault: the reference libargon2 (python3-argon2) for Argon2id, libsodium
(python3-nacl) for XChaCha20-Poly1305 and Ed25519 public keys, hashlib for SHA-256
and the ECR oracle's own deterministic CBOR encoder. It drives the Rust probe
(core/vaultd/src/bin/cham3_vault_probe.rs) over stdin and checks:

* every object the Rust vault seals is byte-identical to the oracle's own
  construction, and the oracle opens it with its own KEK and AEAD;
* the Rust vault opens objects the oracle seals and reports the right key id;
* a wrong passphrase, every authenticated field changed, structural changes, KDF
  parameters outside the bounds or above the locked budget, and a substituted seed
  are all refused with the expected code.

Seeds, passphrases, salts and nonces are synthetic (derived from public labels).
Prints one "PASS <check>" or "FAIL <check>" line per check and exits 0 only if every
check passes and at least MinimumChecks ran.
Usage: python3 -I vault_object_oracle.py --probe PATH
"""

import argparse
import hashlib
import os
import subprocess
import sys

import argon2.low_level
import nacl.bindings
import nacl.signing

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ecr_oracle  # noqa: E402

MinimumChecks = 30
KeyIdDomain = b"cham3/key-id/ed25519/v1"
Created = "2026-10-13T08:00:00Z"
# The vault's bounds and default locked budget (ADR 0005 section 4).
MinMemoryKib, MaxMemoryKib = 65536, 1048576
LockedBudgetKib = 512 * 1024


def synthetic(label, length):
    output = b""
    counter = 0
    while len(output) < length:
        output += hashlib.sha256(("cham3 m2-004 vault oracle %s %d" % (label, counter)).encode()).digest()
        counter += 1
    return output[:length]


def key_id(public_key):
    return hashlib.sha256(KeyIdDomain + b"\x00" + public_key).digest()


def public_key(seed):
    return bytes(nacl.signing.SigningKey(seed).verify_key)


def kek(passphrase, salt, memory_kib, passes, lanes):
    return argon2.low_level.hash_secret_raw(
        secret=passphrase, salt=salt, time_cost=passes, memory_cost=memory_kib,
        parallelism=lanes, hash_len=32, type=argon2.low_level.Type.ID, version=19)


def metadata_map(fields):
    return {
        "format_version": 1,
        "key_type": "ed25519_seed",
        "authority_class": fields["class"],
        "owner_uid": fields["uid"],
        "owner_user_id": fields["owner_user_id"],
        "key_id": fields["key_id"],
        "created": fields["created"],
        "kdf": {"algorithm": "argon2id", "version": 19, "memory_kib": fields["memory_kib"],
                "passes": fields["passes"], "lanes": fields["lanes"]},
        "algorithm": "xchacha20poly1305",
        "salt": fields["salt"],
        "nonce": fields["nonce"],
    }


def seal(fields, seed, passphrase):
    """The oracle's own key object: AAD = encoding of the map without ciphertext/tag."""
    metadata = metadata_map(fields)
    aad = ecr_oracle.encode(metadata)
    key = kek(passphrase, fields["salt"], fields["memory_kib"], fields["passes"], fields["lanes"])
    sealed = nacl.bindings.crypto_aead_xchacha20poly1305_ietf_encrypt(seed, aad, fields["nonce"], key)
    document = dict(metadata, ciphertext=sealed[:32], tag=sealed[32:])
    return ecr_oracle.encode(document)


def oracle_open(document, passphrase):
    """Returns the seed, or None if the AEAD rejects the object."""
    metadata = {name: value for name, value in document.items() if name not in ("ciphertext", "tag")}
    kdf = metadata["kdf"]
    key = kek(passphrase, metadata["salt"], kdf["memory_kib"], kdf["passes"], kdf["lanes"])
    try:
        return nacl.bindings.crypto_aead_xchacha20poly1305_ietf_decrypt(
            document["ciphertext"] + document["tag"], ecr_oracle.encode(metadata), metadata["nonce"], key)
    except Exception:  # noqa: BLE001 - any AEAD failure is a rejection
        return None


def fields_for(label, seed, klass, uid, owner_user_id=None, memory_kib=MinMemoryKib, passes=1, lanes=1):
    identifier = key_id(public_key(seed))
    return {
        "class": klass, "uid": uid, "owner_user_id": owner_user_id or identifier, "key_id": identifier,
        "created": Created, "memory_kib": memory_kib, "passes": passes, "lanes": lanes,
        "salt": synthetic(label + " salt", 16), "nonce": synthetic(label + " nonce", 24),
    }


class probe_t:
    def __init__(self, path):
        self.path = path
        self.requests = []

    def run(self, requests):
        completed = subprocess.run([self.path], input="\n".join(requests) + "\n", capture_output=True,
                                   text=True, check=False, timeout=1800)
        if completed.returncode != 0:
            raise RuntimeError("probe exited %d: %s" % (completed.returncode, completed.stderr[-500:]))
        answers = completed.stdout.splitlines()
        if len(answers) != len(requests):
            raise RuntimeError("probe answered %d of %d requests" % (len(answers), len(requests)))
        return answers


def seal_request(fields, seed, passphrase):
    owner = "self" if fields["owner_user_id"] == fields["key_id"] else fields["owner_user_id"].hex()
    return " ".join(["seal", seed.hex(), passphrase.hex(), fields["salt"].hex(), fields["nonce"].hex(),
                     fields["class"], str(fields["uid"]), owner, fields["created"],
                     str(fields["memory_kib"]), str(fields["passes"]), str(fields["lanes"])])


def open_request(document_bytes, passphrase):
    return "open %s %s" % (document_bytes.hex(), passphrase.hex())


def decode(data):
    return ecr_oracle.decoder_t(data, "payload_decode").decode_top()


def build_checks():
    """Returns (requests, checks): each check maps a probe answer to PASS/FAIL."""
    requests, checks = [], []

    def expect(name, request, predicate):
        requests.append(request)
        checks.append((name, predicate))

    passphrase = b"synthetic oracle passphrase"
    users = [(1001, "alice"), (1002, "bob")]
    # 1. Rust seals; the oracle reproduces the bytes and opens them independently.
    for uid, name in users:
        identity_seed = synthetic(name + " identity seed", 32)
        identity_id = key_id(public_key(identity_seed))
        for klass in ("identity", "persistent", "temporary"):
            seed = identity_seed if klass == "identity" else synthetic("%s %s seed" % (name, klass), 32)
            label = "%s %s" % (name, klass)
            fields = fields_for(label, seed, klass, uid, None if klass == "identity" else identity_id,
                                passes=2 if klass == "temporary" else 1, lanes=2 if klass == "temporary" else 1)
            expected = seal(fields, seed, passphrase)

            def rust_seal_matches(answer, expected=expected, seed=seed):
                if not answer.startswith("ok "):
                    return False
                produced = bytes.fromhex(answer[3:])
                return produced == expected and oracle_open(decode(produced), passphrase) == seed

            expect("rust_seal_matches_oracle " + label, seal_request(fields, seed, passphrase), rust_seal_matches)
    # 2. The oracle seals; Rust opens and names the key.
    seed = synthetic("carol persistent seed", 32)
    owner = key_id(public_key(synthetic("carol identity seed", 32)))
    base_fields = fields_for("carol persistent", seed, "persistent", 1003, owner)
    base = seal(base_fields, seed, passphrase)
    expect("rust_opens_oracle_object", open_request(base, passphrase),
           lambda answer: answer == "ok " + base_fields["key_id"].hex())
    expect("wrong_passphrase_refused", open_request(base, b"another passphrase"),
           lambda answer: answer == "err authentication_failed")
    # 3. Every authenticated field changed: the AEAD refuses it.
    document = decode(base)
    changes = {
        "created": lambda doc: doc.update(created="2026-10-13T08:00:01Z"),
        "owner_uid": lambda doc: doc.update(owner_uid=1004),
        "owner_user_id": lambda doc: doc.update(owner_user_id=bytes(32)),
        "key_id": lambda doc: doc.update(key_id=key_id(public_key(synthetic("other", 32)))),
        "authority_class": lambda doc: doc.update(authority_class="temporary"),
        "kdf_passes": lambda doc: doc["kdf"].update(passes=2),
        "kdf_lanes": lambda doc: doc["kdf"].update(lanes=2),
        "kdf_memory": lambda doc: doc["kdf"].update(memory_kib=MinMemoryKib + 8),
        "salt": lambda doc: doc.update(salt=bytes(16)),
        "nonce": lambda doc: doc.update(nonce=bytes(24)),
        "ciphertext": lambda doc: doc.update(ciphertext=bytes([doc["ciphertext"][0] ^ 1]) + doc["ciphertext"][1:]),
        "tag": lambda doc: doc.update(tag=bytes([doc["tag"][0] ^ 1]) + doc["tag"][1:]),
    }
    for name, change in changes.items():
        changed = decode(base)
        change(changed)
        expect("authenticated_field_change_refused " + name, open_request(ecr_oracle.encode(changed), passphrase),
               lambda answer: answer == "err authentication_failed")
    # 4. Structure and constants: refused before any derivation.
    structural = {
        "extra_field": lambda doc: doc.update(comment="x"),
        "missing_tag": lambda doc: doc.pop("tag"),
        "key_type": lambda doc: doc.update(key_type="x25519_seed"),
        "algorithm": lambda doc: doc.update(algorithm="aes256gcm"),
        "format_version": lambda doc: doc.update(format_version=2),
        "kdf_algorithm": lambda doc: doc["kdf"].update(algorithm="argon2i"),
        "kdf_version": lambda doc: doc["kdf"].update(version=16),
        "short_salt": lambda doc: doc.update(salt=bytes(15)),
        "text_owner_uid": lambda doc: doc.update(owner_uid="1003"),
        "unknown_class": lambda doc: doc.update(authority_class="admin"),
    }
    for name, change in structural.items():
        changed = decode(base)
        change(changed)
        expect("structural_change_refused " + name, open_request(ecr_oracle.encode(changed), passphrase),
               lambda answer: answer == "err malformed")
    expect("non_canonical_encoding_refused", open_request(base + b"\x00", passphrase),
           lambda answer: answer == "err malformed")
    # 5. KDF parameters outside the bounds or above the budget: refused before derivation.
    for name, memory_kib, passes, lanes, code in [
            ("memory_below_bounds", MinMemoryKib - 1, 1, 1, "kdf_out_of_bounds"),
            ("memory_above_bounds", MaxMemoryKib + 1, 1, 1, "kdf_out_of_bounds"),
            ("passes_zero", MinMemoryKib, 0, 1, "kdf_out_of_bounds"),
            ("passes_above_bounds", MinMemoryKib, 11, 1, "kdf_out_of_bounds"),
            ("lanes_above_bounds", MinMemoryKib, 1, 5, "kdf_out_of_bounds"),
            ("memory_above_budget", LockedBudgetKib + 1024, 1, 1, "kdf_over_budget")]:
        changed = decode(base)
        changed["kdf"].update(memory_kib=memory_kib, passes=passes, lanes=lanes)
        expect("kdf_parameters_refused " + name, open_request(ecr_oracle.encode(changed), passphrase),
               lambda answer, code=code: answer == "err " + code)
    # 6. Substitution: key A's metadata over key B's seed, sealed correctly.
    other_seed = synthetic("carol temporary seed", 32)
    substituted = seal(base_fields, other_seed, passphrase)
    expect("substituted_seed_refused", open_request(substituted, passphrase),
           lambda answer: answer == "err key_id_mismatch")
    return requests, checks


def main(arguments):
    parser = argparse.ArgumentParser()
    parser.add_argument("--probe", required=True)
    options = parser.parse_args(arguments[1:])
    requests, checks = build_checks()
    try:
        answers = probe_t(options.probe).run(requests)
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print("FAIL probe_run %s" % error)
        return 1
    failed = False
    executed = 0
    # Oracle-only control: its own AEAD refuses a wrong passphrase too.
    passphrase = b"synthetic oracle passphrase"
    seed = synthetic("carol persistent seed", 32)
    fields = fields_for("carol persistent", seed, "persistent", 1003)
    document = decode(seal(fields, seed, passphrase))
    oracle_control = oracle_open(document, passphrase) == seed and oracle_open(document, b"another passphrase") is None
    print(("PASS " if oracle_control else "FAIL ") + "oracle_aead_control")
    failed = not oracle_control
    for index, (name, predicate) in enumerate(checks):
        answer = answers[index] if index < len(answers) else ""
        passed = bool(predicate(answer))
        executed += 1
        print(("PASS " if passed else "FAIL ") + name + ("" if passed else " (probe: %s)" % answer[:120]))
        failed = failed or not passed
    if executed < MinimumChecks:
        print("FAIL minimum_checks %d < %d" % (executed, MinimumChecks))
        failed = True
    print("CHECKS %d" % executed)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
