"""Independent oracle for the cham3 ECR v1 wire format (docs/specs/m2_wire_format_v1.md).

This implementation shares no code with the Rust crates. It contains its own
deterministic CBOR encoder and strict decoder, the COSE_Sign1 profile, Ed25519 through
libsodium (python3-nacl), the closed schemas, intrinsic semantics and the lineage
admission rules needed to check the published chain. cbor2 is used only as a second,
independent decoder/canonical encoder cross-check for valid objects.

Usage (python3 -I):
  ecr_oracle.py generate OUT.json   write the vector set (deterministic)
  ecr_oracle.py check VECTORS.json  regenerate and compare byte for byte, then verify
                                    every vector reports exactly its expected result

All keys and identifiers are synthetic, derived from public labels, and are for tests
only. Status: PROPOSED G2 vectors (ADR 0004); not approved.
"""

import datetime
import hashlib
import json
import re
import sys

import cbor2
import nacl.exceptions
import nacl.signing

MaxInputBytes = 65536
MaxDepth = 8
MaxContainerItems = 256
MaxTextBytes = 1024
AlgorithmEd25519 = -19
AlgorithmEdDsaPolymorphic = -8
LabelAlg = 1
LabelKid = 4
LabelTyp = 16

Kinds = {
    "commit": ("application/vnd.cham3.ecr-commit.v1+cose", b"cham3/ecr-commit/v1"),
    "identity_enrollment": ("application/vnd.cham3.identity-enrollment.v1+cose", b"cham3/identity-enrollment/v1"),
    "key_certificate": ("application/vnd.cham3.key-certificate.v1+cose", b"cham3/key-certificate/v1"),
    "key_invalidation": ("application/vnd.cham3.key-invalidation.v1+cose", b"cham3/key-invalidation/v1"),
}
KnownTypes = {typ: kind for kind, (typ, _) in Kinds.items()}
GroupOrder = 2**252 + 27742317777372353535851937790883648493


class oracle_error_t(Exception):
    def __init__(self, layer, code, detail=""):
        super().__init__("%s/%s %s" % (layer, code, detail))
        self.layer = layer
        self.code = code


class tag_t:
    def __init__(self, number, value):
        self.number = number
        self.value = value


# ------------------------------------------------------------------ encoder


def head(major, argument):
    if argument < 24:
        return bytes([(major << 5) | argument])
    if argument < 2**8:
        return bytes([(major << 5) | 24, argument])
    if argument < 2**16:
        return bytes([(major << 5) | 25]) + argument.to_bytes(2, "big")
    if argument < 2**32:
        return bytes([(major << 5) | 26]) + argument.to_bytes(4, "big")
    return bytes([(major << 5) | 27]) + argument.to_bytes(8, "big")


def encode(value):
    if isinstance(value, bool):
        return b"\xf5" if value else b"\xf4"
    if isinstance(value, int):
        return head(0, value) if value >= 0 else head(1, -1 - value)
    if isinstance(value, bytes):
        return head(2, len(value)) + value
    if isinstance(value, str):
        raw = value.encode("utf-8")
        return head(3, len(raw)) + raw
    if isinstance(value, list):
        return head(4, len(value)) + b"".join(encode(item) for item in value)
    if isinstance(value, dict):
        pairs = sorted((encode(key), encode(item)) for key, item in value.items())
        for left, right in zip(pairs, pairs[1:]):
            if left[0] == right[0]:
                raise ValueError("duplicate map key")
        return head(5, len(pairs)) + b"".join(key + item for key, item in pairs)
    if isinstance(value, tag_t):
        return head(6, value.number) + encode(value.value)
    raise TypeError("unsupported type %r" % type(value))


# ------------------------------------------------------------------ strict decoder


class decoder_t:
    def __init__(self, data, layer):
        self.data = data
        self.position = 0
        self.layer = layer

    def fail(self, code):
        raise oracle_error_t(self.layer, code, "at byte %d" % self.position)

    def take(self, count):
        if self.position + count > len(self.data):
            self.fail("truncated")
        chunk = self.data[self.position:self.position + count]
        self.position += count
        return chunk

    def decode_top(self, allow_root_tag=False):
        if len(self.data) > MaxInputBytes:
            self.fail("input_too_large")
        value = self.item(0, allow_root_tag)
        if self.position != len(self.data):
            self.fail("trailing_bytes")
        return value

    def argument(self, major, additional):
        if additional < 24:
            return additional
        width = {24: 1, 25: 2, 26: 4, 27: 8}[additional]
        value = int.from_bytes(self.take(width), "big")
        minimum = {24: 24, 25: 2**8, 26: 2**16, 27: 2**32}[additional]
        if value < minimum:
            self.fail("non_minimal_encoding")
        return value

    def item(self, depth, allow_root_tag):
        initial = self.take(1)[0]
        major = initial >> 5
        additional = initial & 0x1F
        if additional in (28, 29, 30):
            self.fail("reserved_additional_info")
        if additional == 31:
            self.fail("indefinite_length")
        if major == 7:
            if additional == 20:
                return False
            if additional == 21:
                return True
            if additional in (25, 26, 27):
                self.take({25: 2, 26: 4, 27: 8}[additional])
                self.fail("float_forbidden")
            if additional == 24:
                self.take(1)
            self.fail("simple_value_forbidden")
        argument = self.argument(major, additional)
        if major == 0:
            return argument
        if major == 1:
            if argument > 2**63 - 1:
                self.fail("integer_out_of_range")
            return -1 - argument
        if major in (2, 3):
            if major == 3 and argument > MaxTextBytes:
                self.fail("text_too_long")
            if argument > MaxInputBytes:
                self.fail("input_too_large")
            raw = self.take(argument)
            if major == 2:
                return bytes(raw)
            try:
                return bytes(raw).decode("utf-8", errors="strict")
            except UnicodeDecodeError:
                self.fail("invalid_utf8")
        if major in (4, 5):
            if depth + 1 > MaxDepth:
                self.fail("depth_exceeded")
            if argument > MaxContainerItems:
                self.fail("container_too_large")
            if major == 4:
                return [self.item(depth + 1, False) for _ in range(argument)]
            result = {}
            previous = None
            key_class = None
            for _ in range(argument):
                start = self.position
                key = self.item(depth + 1, False)
                encoded_key = self.data[start:self.position]
                this_class = "int" if isinstance(key, int) and not isinstance(key, bool) else ("text" if isinstance(key, str) else None)
                if this_class is None or (key_class is not None and this_class != key_class):
                    self.fail("map_key_type")
                key_class = this_class
                if previous is not None and not encoded_key > previous:
                    self.fail("unsorted_or_duplicate_key")
                previous = encoded_key
                result[key] = self.item(depth + 1, False)
            return result
        # major 6
        if not (allow_root_tag and depth == 0 and argument == 18):
            self.fail("tag_forbidden")
        if depth + 1 > MaxDepth:
            self.fail("depth_exceeded")
        return tag_t(18, self.item(depth + 1, False))


# ------------------------------------------------------------------ hashes and keys


def domain_hash(tag, data):
    return hashlib.sha256(tag.encode("ascii") + b"\x00" + data).digest()


def key_id(public_key):
    return domain_hash("cham3/key-id/ed25519/v1", public_key)


def lineage_id(user_id, app_id, installation_id):
    return domain_hash("cham3/lineage/v1", encode([user_id, app_id, installation_id]))


def state_hash(state):
    return domain_hash("cham3/state/v1", encode(state))


EmptyState = {"grants": [], "network_mode": "offline"}


class key_t:
    def __init__(self, label):
        self.label = label
        self.seed = hashlib.sha256(b"cham3 m2 synthetic test key v1: " + label.encode()).digest()
        self.signing = nacl.signing.SigningKey(self.seed)
        self.public_key = bytes(self.signing.verify_key)
        self.key_id = key_id(self.public_key)

    def sign(self, message):
        return self.signing.sign(message).signature


def synthetic_id(label):
    return hashlib.sha256(b"cham3 m2 vector id v1: " + label.encode()).digest()[:16]


# ------------------------------------------------------------------ envelope


def protected_bytes(kid, typ, algorithm=AlgorithmEd25519):
    return encode({LabelAlg: algorithm, LabelKid: kid, LabelTyp: typ})


def to_be_signed(protected, external_aad, payload):
    return encode(["Signature1", protected, external_aad, payload])


def build_envelope(kind, signer, payload_bytes, protected=None, external_aad=None, signature=None, unprotected=None):
    typ, aad = Kinds[kind]
    protected = protected if protected is not None else protected_bytes(signer.key_id, typ)
    aad = external_aad if external_aad is not None else aad
    if signature is None:
        signature = signer.sign(to_be_signed(protected, aad, payload_bytes))
    return encode(tag_t(18, [protected, unprotected if unprotected is not None else {}, payload_bytes, signature]))


def parse_and_verify(data, expected_kind, resolver):
    """Layers 1-3. Returns (kid, payload bytes)."""
    root = decoder_t(data, "envelope_decode").decode_top(allow_root_tag=True)
    if not isinstance(root, tag_t) or not isinstance(root.value, list) or len(root.value) != 4:
        raise oracle_error_t("envelope", "not_signed_object")
    protected, unprotected, payload, signature = root.value
    if not isinstance(protected, bytes) or not isinstance(unprotected, dict) or not isinstance(payload, bytes) or not isinstance(signature, bytes):
        raise oracle_error_t("envelope", "not_signed_object")
    header = decoder_t(protected, "envelope_decode").decode_top()
    if not isinstance(header, dict) or set(header) != {LabelAlg, LabelKid, LabelTyp}:
        raise oracle_error_t("envelope", "protected_header_invalid")
    if header[LabelAlg] != AlgorithmEd25519:
        if isinstance(header[LabelAlg], int) and not isinstance(header[LabelAlg], bool):
            raise oracle_error_t("envelope", "unsupported_algorithm")
        raise oracle_error_t("envelope", "protected_header_invalid")
    kid = header[LabelKid]
    if not isinstance(kid, bytes) or len(kid) != 32:
        raise oracle_error_t("envelope", "protected_header_invalid")
    typ = header[LabelTyp]
    expected_typ, external_aad = Kinds[expected_kind]
    if typ != expected_typ:
        if typ in KnownTypes:
            raise oracle_error_t("envelope", "wrong_object_type")
        raise oracle_error_t("envelope", "protected_header_invalid")
    if unprotected != {}:
        raise oracle_error_t("envelope", "unprotected_not_empty")
    if len(signature) != 64:
        raise oracle_error_t("envelope", "signature_length")
    public_key = resolver.get(kid)
    if public_key is None:
        raise oracle_error_t("signature", "unknown_key")
    try:
        # libsodium crypto_sign_open: rejects S >= L and small-order A/R encodings.
        nacl.signing.VerifyKey(public_key).verify(to_be_signed(protected, external_aad, payload), signature)
    except nacl.exceptions.BadSignatureError:
        raise oracle_error_t("signature", "signature_invalid")
    if int.from_bytes(signature[32:], "little") >= GroupOrder:
        raise oracle_error_t("signature", "signature_invalid")
    return kid, payload


# ------------------------------------------------------------------ schema

AppIdPattern = re.compile(r"^[a-z0-9][a-z0-9._-]*$")
OperationPattern = re.compile(r"^[a-z][a-z0-9_]*$")
TimePattern = re.compile(r"^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})Z$")
ForbiddenText = set(range(0x00, 0x20)) | set(range(0x7F, 0xA0)) | {0x061C, 0x200E, 0x200F, 0xFEFF} | set(range(0x202A, 0x202F)) | set(range(0x2066, 0x206A))


def schema_error(code, path):
    raise oracle_error_t("schema", code, path)


def is_uint(value):
    return isinstance(value, int) and not isinstance(value, bool) and value >= 0


def check_uint(value, path, maximum=2**64 - 1, minimum=0):
    if not isinstance(value, int) or isinstance(value, bool):
        schema_error("wrong_type", path)
    if value < minimum or value > maximum:
        schema_error("value_out_of_range", path)
    return value


def check_fixed(value, path, expected):
    if type(value) is not type(expected):
        schema_error("wrong_type", path)
    if value != expected:
        schema_error("invalid_enum", path)
    return value


def check_enum(value, path, members):
    if not isinstance(value, str):
        schema_error("wrong_type", path)
    if value not in members:
        schema_error("invalid_enum", path)
    return value


def check_bytes(value, path, length):
    if not isinstance(value, bytes):
        schema_error("wrong_type", path)
    if len(value) != length:
        schema_error("value_out_of_range", path)
    return value


def check_bool(value, path):
    if not isinstance(value, bool):
        schema_error("wrong_type", path)
    return value


def check_time(value, path):
    if not isinstance(value, str):
        schema_error("wrong_type", path)
    match = TimePattern.match(value)
    if not match or len(value) != 20:
        schema_error("invalid_time", path)
    year, month, day, hour, minute, second = (int(part) for part in match.groups())
    if year < 2000 or hour > 23 or minute > 59 or second > 59:
        schema_error("invalid_time", path)
    try:
        datetime.date(year, month, day)
    except ValueError:
        schema_error("invalid_time", path)
    return value


def parse_time(value):
    return datetime.datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ")


def check_pattern_text(value, path, pattern, minimum, maximum):
    if not isinstance(value, str):
        schema_error("wrong_type", path)
    if not minimum <= len(value.encode("utf-8")) <= maximum:
        schema_error("value_out_of_range", path)
    if not pattern.match(value):
        schema_error("invalid_text", path)
    return value


def check_package_identity(value, path):
    if not isinstance(value, str):
        schema_error("wrong_type", path)
    if not 1 <= len(value.encode("utf-8")) <= 255:
        schema_error("value_out_of_range", path)
    if any(not 0x21 <= ord(char) <= 0x7E for char in value):
        schema_error("invalid_text", path)
    return value


def check_restricted_text(value, path, minimum, maximum, allow_newline):
    if not isinstance(value, str):
        schema_error("wrong_type", path)
    if not minimum <= len(value.encode("utf-8")) <= maximum:
        schema_error("value_out_of_range", path)
    for char in value:
        code = ord(char)
        if code in ForbiddenText and not (allow_newline and code == 0x0A):
            schema_error("invalid_text", path)
    if allow_newline:
        if value != value.strip() or value.count("\n") > 15:
            schema_error("invalid_text", path)
    return value


def check_list(value, path, minimum, maximum, item_check, sort_key=None):
    if not isinstance(value, list):
        schema_error("wrong_type", path)
    if not minimum <= len(value) <= maximum:
        schema_error("list_length", path)
    for index, item in enumerate(value):
        item_check(item, "%s[%d]" % (path, index))
    keys = [sort_key(item) if sort_key else encode(item) for item in value]
    for left, right in zip(keys, keys[1:]):
        if not left < right:
            schema_error("list_not_strictly_ascending", path)
    return value


def check_map(value, path, fields):
    """fields: {key: (required, check)}; checked in ascending encoded key order."""
    if not isinstance(value, dict):
        schema_error("wrong_type", path)
    for key in sorted(set(fields) | set(value), key=encode):
        if key not in fields:
            schema_error("unknown_field", "%s.%s" % (path, key))
        required, check = fields[key]
        if key not in value:
            if required:
                schema_error("missing_field", "%s.%s" % (path, key))
            continue
        check(value[key], "%s.%s" % (path, key))
    return value


def uid_check(value, path):
    return check_uint(value, path, maximum=4294967294)


def random_id_check(value, path):
    return check_bytes(value, path, 16)


def digest_check(value, path):
    return check_bytes(value, path, 32)


def label_check(value, path):
    return check_pattern_text(value, path, AppIdPattern, 1, 64)


def operation_check(value, path):
    return check_pattern_text(value, path, OperationPattern, 1, 32)


LifetimeFields = {
    "kind": (True, lambda value, path: check_enum(value, path, {"persistent", "timed", "session", "one_shot"})),
    "effective_start": (True, check_time),
    "expiration": (False, check_time),
    "session_id": (False, random_id_check),
    "usage_limit": (False, lambda value, path: check_fixed(value, path, 1)),
}
ResourceFields = {
    "category": (True, lambda value, path: check_enum(value, path, {"filesystem", "device", "audio_input", "camera", "screen_capture", "clipboard", "portal", "ipc", "notification"})),
    "resource_id": (True, lambda value, path: check_restricted_text(value, path, 1, 512, False)),
    "resource_owner_uid": (True, uid_check),
    "operations": (True, lambda value, path: check_list(value, path, 1, 32, operation_check)),
}
DelegationFields = {
    "permitted": (True, check_bool),
    "max_depth": (True, lambda value, path: check_uint(value, path, maximum=8)),
    "max_operations": (True, lambda value, path: check_list(value, path, 0, 32, operation_check)),
    "parent_grant_id": (False, random_id_check),
}
GrantFields = {
    "grant_id": (True, random_id_check),
    "contract_type": (True, lambda value, path: check_fixed(value, path, "resource_access")),
    "lifetime": (True, lambda value, path: check_map(value, path, LifetimeFields)),
    "resource": (True, lambda value, path: check_map(value, path, ResourceFields)),
    "delegation": (True, lambda value, path: check_map(value, path, DelegationFields)),
}


def grant_check(value, path):
    return check_map(value, path, GrantFields)


def grant_list_check(maximum):
    return lambda value, path: check_list(value, path, 0, maximum, grant_check, sort_key=lambda grant: grant["grant_id"])


NetworkModes = {"offline", "tor", "direct"}
StateFields = {
    "network_mode": (True, lambda value, path: check_enum(value, path, NetworkModes)),
    "grants": (True, grant_list_check(256)),
}
DeltaFields = {
    "add_grants": (True, grant_list_check(64)),
    "remove_grant_ids": (True, lambda value, path: check_list(value, path, 0, 64, random_id_check)),
    "network_mode": (False, lambda value, path: check_map(value, path, {
        "from": (True, lambda item, item_path: check_enum(item, item_path, NetworkModes)),
        "to": (True, lambda item, item_path: check_enum(item, item_path, NetworkModes)),
    })),
}
CommitFields = {
    "schema_version": (True, lambda value, path: check_fixed(value, path, 1)),
    "commit_type": (True, lambda value, path: check_enum(value, path, {"genesis", "revision", "revocation", "temporary_grant", "merge", "restore", "identity_transition"})),
    "timestamp": (True, check_time),
    "actor": (True, lambda value, path: check_map(value, path, {"uid": (True, uid_check), "user_id": (True, digest_check)})),
    "key_id": (True, digest_check),
    "authority_class": (True, lambda value, path: check_enum(value, path, {"persistent", "temporary"})),
    "authorization": (True, lambda value, path: check_map(value, path, {"request_id": (True, random_id_check), "session_id": (True, random_id_check)})),
    "policy": (True, lambda value, path: check_map(value, path, {
        "epoch": (True, check_uint),
        "version": (True, label_check),
        "security_profile": (True, label_check),
        "mandatory_constraints": (True, lambda item, item_path: check_list(item, item_path, 0, 32, label_check)),
    })),
    "subject": (True, lambda value, path: check_map(value, path, {
        "owner_uid": (True, uid_check),
        "owner_user_id": (True, digest_check),
        "app_id": (True, lambda item, item_path: check_pattern_text(item, item_path, AppIdPattern, 1, 255)),
        "installation_id": (True, random_id_check),
        "package_identity": (True, check_package_identity),
        "executable_sha256": (True, digest_check),
        "runtime_scope": (False, label_check),
    })),
    "initiating_program": (True, lambda value, path: check_enum(value, path, {"cham3_authui", "cham3_inspector", "cham3_portal_broker", "cham3_dev_cli"})),
    "lineage_id": (True, digest_check),
    "parents": (True, lambda value, path: check_list(value, path, 0, 8, digest_check)),
    "base_state_hash": (True, digest_check),
    "merge_base": (False, digest_check),
    "restore_reference": (False, digest_check),
    "message": (True, lambda value, path: check_restricted_text(value, path, 0, 1024, True)),
    "delta": (True, lambda value, path: check_map(value, path, DeltaFields)),
    "result_state": (True, lambda value, path: check_map(value, path, StateFields)),
}
EnrollmentFields = {
    "schema_version": (True, lambda value, path: check_fixed(value, path, 1)),
    "user_id": (True, digest_check),
    "public_key": (True, digest_check),
    "owner_uid": (True, uid_check),
    "enrolled": (True, check_time),
    "identity_epoch": (True, check_uint),
}
CertificateFields = {
    "schema_version": (True, lambda value, path: check_fixed(value, path, 1)),
    "certificate_id": (True, random_id_check),
    "issued": (True, check_time),
    "owner_uid": (True, uid_check),
    "owner_user_id": (True, digest_check),
    "subject_key_id": (True, digest_check),
    "subject_public_key": (True, digest_check),
    "authority_class": (True, lambda value, path: check_enum(value, path, {"persistent", "temporary"})),
    "key_epoch": (True, check_uint),
    "not_before": (True, check_time),
    "not_after": (False, check_time),
    "supersedes_key_id": (False, digest_check),
}
InvalidationFields = {
    "schema_version": (True, lambda value, path: check_fixed(value, path, 1)),
    "invalidation_id": (True, random_id_check),
    "issued": (True, check_time),
    "owner_user_id": (True, digest_check),
    "target_key_id": (True, digest_check),
    "reason": (True, lambda value, path: check_enum(value, path, {"compromised", "rotated", "retired"})),
}


def check_payload(kind, kid, payload_bytes):
    """Layer 4: payload decode, schema and cross-field identity rules."""
    payload = decoder_t(payload_bytes, "payload_decode").decode_top()
    if kind == "commit":
        check_map(payload, "commit", CommitFields)
        if payload["key_id"] != kid:
            schema_error("key_id_mismatch", "commit.key_id")
        subject = payload["subject"]
        if subject["owner_uid"] != payload["actor"]["uid"] or subject["owner_user_id"] != payload["actor"]["user_id"]:
            schema_error("subject_actor_mismatch", "commit.subject")
        if payload["lineage_id"] != lineage_id(subject["owner_user_id"], subject["app_id"], subject["installation_id"]):
            schema_error("lineage_mismatch", "commit.lineage_id")
    elif kind == "identity_enrollment":
        check_map(payload, "identity_enrollment", EnrollmentFields)
        if payload["user_id"] != kid or payload["user_id"] != key_id(payload["public_key"]):
            schema_error("key_id_mismatch", "identity_enrollment.user_id")
    elif kind == "key_certificate":
        check_map(payload, "key_certificate", CertificateFields)
        if payload["owner_user_id"] != kid:
            schema_error("key_id_mismatch", "key_certificate.owner_user_id")
        if payload["subject_key_id"] != key_id(payload["subject_public_key"]):
            schema_error("key_id_mismatch", "key_certificate.subject_key_id")
    else:
        check_map(payload, "key_invalidation", InvalidationFields)
        if payload["owner_user_id"] != kid:
            schema_error("key_id_mismatch", "key_invalidation.owner_user_id")
    return payload


# ------------------------------------------------------------------ intrinsic semantics

TemporaryKinds = {"timed", "session", "one_shot"}


def intrinsic_error(code, detail=""):
    raise oracle_error_t("intrinsic", code, detail)


def check_lifetime(grant):
    lifetime = grant["lifetime"]
    kind = lifetime["kind"]
    start = parse_time(lifetime["effective_start"])
    if "expiration" in lifetime and parse_time(lifetime["expiration"]) <= start:
        intrinsic_error("lifetime_rule", "expiration not after start")
    if kind == "timed" and "expiration" not in lifetime:
        intrinsic_error("lifetime_rule", "timed without expiration")
    if kind == "session" and "session_id" not in lifetime:
        intrinsic_error("lifetime_rule", "session without session_id")
    if kind in ("persistent", "timed") and "session_id" in lifetime:
        intrinsic_error("lifetime_rule", "unexpected session_id")
    if (kind == "one_shot") != ("usage_limit" in lifetime):
        intrinsic_error("lifetime_rule", "usage_limit only and always for one_shot")


def check_delegation(grant):
    delegation = grant["delegation"]
    operations = set(grant["resource"]["operations"])
    if not delegation["permitted"]:
        if delegation["max_depth"] != 0 or delegation["max_operations"]:
            intrinsic_error("delegation_rule", "non-delegable grant with delegation scope")
    else:
        if not 1 <= delegation["max_depth"] <= 8 or not delegation["max_operations"]:
            intrinsic_error("delegation_rule", "delegable grant without scope")
        if not set(delegation["max_operations"]) <= operations:
            intrinsic_error("delegation_rule", "max_operations exceeds operations")


def check_intrinsic(payload):
    """Layer 5 for commits."""
    commit_type = payload["commit_type"]
    parents = payload["parents"]
    delta = payload["delta"]
    adds = delta["add_grants"]
    removes = delta["remove_grant_ids"]
    network = delta.get("network_mode")
    authority = payload["authority_class"]
    add_kinds = {grant["lifetime"]["kind"] for grant in adds}
    rules = {
        "genesis": (len(parents) == 0, authority == "persistent", add_kinds <= {"persistent"} and not removes),
        "revision": (len(parents) == 1, authority == "persistent", add_kinds <= {"persistent"} and bool(adds or removes or network)),
        "revocation": (len(parents) == 1, authority == "persistent", not adds and bool(removes) and network is None),
        "temporary_grant": (len(parents) == 1, True, bool(adds) and add_kinds <= TemporaryKinds and not removes and network is None),
        "merge": (2 <= len(parents) <= 8, authority == "persistent", add_kinds <= {"persistent"} and payload.get("merge_base") in parents),
        "restore": (len(parents) == 1, authority == "persistent", add_kinds <= {"persistent"}),
        "identity_transition": (len(parents) == 1, authority == "persistent", not adds and not removes and network is None),
    }
    if not all(rules[commit_type]):
        intrinsic_error("commit_type_rule", commit_type)
    if ("merge_base" in payload) != (commit_type == "merge") or ("restore_reference" in payload) != (commit_type == "restore"):
        intrinsic_error("commit_type_rule", "merge_base/restore_reference placement")
    for grant in adds + payload["result_state"]["grants"]:
        check_lifetime(grant)
    for grant in adds + payload["result_state"]["grants"]:
        check_delegation(grant)
    result = payload["result_state"]
    if network is not None:
        if network["from"] == network["to"] or network["to"] != result["network_mode"]:
            intrinsic_error("network_change_rule")
    add_ids = {grant["grant_id"] for grant in adds}
    if add_ids & set(removes):
        intrinsic_error("delta_rule", "grant both added and removed")
    result_by_id = {grant["grant_id"]: grant for grant in result["grants"]}
    for grant in adds:
        if encode(result_by_id.get(grant["grant_id"], {})) != encode(grant):
            intrinsic_error("result_state_mismatch", "added grant absent or changed")
    if set(removes) & set(result_by_id):
        intrinsic_error("result_state_mismatch", "removed grant still present")
    if commit_type == "genesis" and payload["base_state_hash"] != state_hash(EmptyState):
        intrinsic_error("base_state_mismatch")


def verify(data, expected_kind, resolver):
    kid, payload_bytes = parse_and_verify(data, expected_kind, resolver)
    payload = check_payload(expected_kind, kid, payload_bytes)
    if expected_kind == "commit":
        check_intrinsic(payload)
    return payload


# ------------------------------------------------------------------ lineage admission (subset)


def admission_error(code, detail=""):
    raise oracle_error_t("admission", code, detail)


def apply_delta(base, delta):
    grants = {grant["grant_id"]: grant for grant in base["grants"]}
    for removed in delta["remove_grant_ids"]:
        if removed not in grants:
            admission_error("delta_rule", "removed grant not in base")
        del grants[removed]
    for grant in delta["add_grants"]:
        if grant["grant_id"] in grants:
            admission_error("delta_rule", "added grant already in base")
        grants[grant["grant_id"]] = grant
    mode = base["network_mode"]
    if "network_mode" in delta:
        if delta["network_mode"]["from"] != mode:
            admission_error("network_change_rule", "from differs from base")
        mode = delta["network_mode"]["to"]
    return {"grants": [grants[key] for key in sorted(grants)], "network_mode": mode}


def admit_chain(envelopes, resolver):
    """Admits commits in order into one lineage; returns {commit_id: payload}."""
    admitted = {}
    seen_grants = set()
    seen_requests = set()
    for data in envelopes:
        payload = verify(data, "commit", resolver)
        commit_id = hashlib.sha256(data).digest()
        for parent in payload["parents"]:
            if parent not in admitted:
                admission_error("parent_unknown")
            if admitted[parent]["lineage_id"] != payload["lineage_id"]:
                admission_error("lineage_mismatch", "cross-lineage parent")
        if payload["commit_type"] == "genesis":
            base = EmptyState
        else:
            base_id = payload.get("merge_base", payload["parents"][0])
            base = admitted[base_id]["result_state"]
        if state_hash(base) != payload["base_state_hash"]:
            admission_error("base_state_mismatch")
        if encode(apply_delta(base, payload["delta"])) != encode(payload["result_state"]):
            admission_error("result_state_mismatch")
        for grant in payload["delta"]["add_grants"]:
            if grant["grant_id"] in seen_grants:
                admission_error("duplicate_identifier", "grant_id reused")
        if payload["authorization"]["request_id"] in seen_requests:
            admission_error("duplicate_identifier", "request_id reused")
        if payload["commit_type"] == "restore":
            reference = admitted.get(payload["restore_reference"])
            if reference is None or reference["lineage_id"] != payload["lineage_id"]:
                admission_error("restore_rule", "reference not in lineage")
            historical = [(encode(grant["resource"]), encode(grant["delegation"]), grant["contract_type"]) for grant in reference["result_state"]["grants"]]
            for grant in payload["delta"]["add_grants"]:
                if (encode(grant["resource"]), encode(grant["delegation"]), grant["contract_type"]) not in historical:
                    admission_error("restore_rule", "restored grant not in referenced state")
        if payload["commit_type"] == "identity_transition":
            parent_subject = admitted[payload["parents"][0]]["subject"]
            old = (parent_subject["package_identity"], parent_subject["executable_sha256"])
            new = (payload["subject"]["package_identity"], payload["subject"]["executable_sha256"])
            if old == new:
                admission_error("identity_transition_rule", "package/executable identity unchanged")
        seen_grants.update(grant["grant_id"] for grant in payload["delta"]["add_grants"])
        seen_requests.add(payload["authorization"]["request_id"])
        admitted[commit_id] = payload
    return admitted


# ------------------------------------------------------------------ vector construction


class resolver_t:
    def __init__(self, keys):
        self.keys = {key.key_id: key.public_key for key in keys}

    def get(self, kid):
        return self.keys.get(kid)


Times = ["2026-10-12T12:%02d:00Z" % minute for minute in range(0, 60, 5)]
KeyLabels = [
    "user_1001_identity", "user_1001_persistent", "user_1001_temporary", "user_1001_retired",
    "user_1002_identity", "user_1002_persistent", "user_1002_temporary",
]


def make_grant(label, category, resource_id, operations, kind="persistent", start=Times[0], owner_uid=1001,
               expiration=None, session=None, delegable=False):
    lifetime = {"kind": kind, "effective_start": start}
    if expiration:
        lifetime["expiration"] = expiration
    if session:
        lifetime["session_id"] = session
    if kind == "one_shot":
        lifetime["usage_limit"] = 1
    delegation = {"permitted": delegable, "max_depth": 1 if delegable else 0, "max_operations": sorted(operations)[:1] if delegable else []}
    return {
        "grant_id": synthetic_id("grant " + label),
        "contract_type": "resource_access",
        "lifetime": lifetime,
        "resource": {"category": category, "resource_id": resource_id, "resource_owner_uid": owner_uid, "operations": sorted(operations)},
        "delegation": delegation,
    }


class lineage_builder_t:
    def __init__(self, keys, uid, user, app_id, package_identity):
        self.keys = keys
        self.uid = uid
        self.user = keys[user]
        self.app_id = app_id
        self.installation_id = synthetic_id("installation %s %s" % (uid, app_id))
        self.package_identity = package_identity
        self.executable = hashlib.sha256(("synthetic executable " + app_id).encode()).digest()
        self.lineage = lineage_id(self.user.key_id, app_id, self.installation_id)
        self.states = {}

    def payload(self, label, commit_type, signer, parents, base_state, delta, time, message, extra=None, authority="persistent"):
        result = apply_delta(base_state, delta)
        payload = {
            "schema_version": 1,
            "commit_type": commit_type,
            "timestamp": time,
            "actor": {"uid": self.uid, "user_id": self.user.key_id},
            "key_id": signer.key_id,
            "authority_class": authority,
            "authorization": {"request_id": synthetic_id("request " + label), "session_id": synthetic_id("auth session %s" % self.uid)},
            "policy": {"epoch": 7, "version": "cham3-policy-2026.10", "security_profile": "desktop_default", "mandatory_constraints": ["no_signing_keys", "offline_by_default"]},
            "subject": {
                "owner_uid": self.uid,
                "owner_user_id": self.user.key_id,
                "app_id": self.app_id,
                "installation_id": self.installation_id,
                "package_identity": self.package_identity,
                "executable_sha256": self.executable,
            },
            "initiating_program": "cham3_dev_cli",
            "lineage_id": self.lineage,
            "parents": sorted(parents),
            "base_state_hash": state_hash(base_state),
            "message": message,
            "delta": delta,
            "result_state": result,
        }
        payload.update(extra or {})
        return payload


def sign_commit(signer, payload):
    payload_bytes = encode(payload)
    data = build_envelope("commit", signer, payload_bytes)
    return data, hashlib.sha256(data).digest()


def build_vectors():
    keys = {label: key_t(label) for label in KeyLabels}
    resolver_keys = [keys[label] for label in KeyLabels]
    valid = []
    identity = []

    def add_identity(name, kind, signer, payload):
        data = build_envelope(kind, signer, encode(payload))
        identity.append({"name": name, "kind": kind, "envelope": data, "payload": payload})

    for uid, prefix in ((1001, "user_1001"), (1002, "user_1002")):
        identity_key = keys[prefix + "_identity"]
        add_identity(prefix + "_identity_enrollment", "identity_enrollment", identity_key, {
            "schema_version": 1, "user_id": identity_key.key_id, "public_key": identity_key.public_key,
            "owner_uid": uid, "enrolled": Times[0], "identity_epoch": 1,
        })
        for authority in ("persistent", "temporary"):
            subject = keys["%s_%s" % (prefix, authority)]
            certificate = {
                "schema_version": 1, "certificate_id": synthetic_id("certificate %s %s" % (prefix, authority)),
                "issued": Times[0], "owner_uid": uid, "owner_user_id": identity_key.key_id,
                "subject_key_id": subject.key_id, "subject_public_key": subject.public_key,
                "authority_class": authority, "key_epoch": 1, "not_before": Times[0],
            }
            if authority == "temporary":
                certificate["not_after"] = "2026-10-13T12:00:00Z"
            add_identity("%s_%s_certificate" % (prefix, authority), "key_certificate", identity_key, certificate)
    retired = keys["user_1001_retired"]
    identity_1001 = keys["user_1001_identity"]
    add_identity("user_1001_retired_certificate", "key_certificate", identity_1001, {
        "schema_version": 1, "certificate_id": synthetic_id("certificate user_1001 retired"), "issued": Times[0],
        "owner_uid": 1001, "owner_user_id": identity_1001.key_id, "subject_key_id": retired.key_id,
        "subject_public_key": retired.public_key, "authority_class": "persistent", "key_epoch": 1, "not_before": Times[0],
    })
    add_identity("user_1001_retired_invalidation", "key_invalidation", identity_1001, {
        "schema_version": 1, "invalidation_id": synthetic_id("invalidation user_1001 retired"), "issued": Times[1],
        "owner_user_id": identity_1001.key_id, "target_key_id": retired.key_id, "reason": "retired",
    })

    # Lineage for user 1001, a synthetic Kate installation.
    kate = lineage_builder_t(keys, 1001, "user_1001_identity", "org.kde.kate", "deb:kate:amd64")
    persistent = keys["user_1001_persistent"]
    temporary = keys["user_1001_temporary"]
    documents = make_grant("documents_read", "filesystem", "home:Documents/notes", ["read"], delegable=True)
    clipboard = make_grant("clipboard", "clipboard", "session_clipboard", ["read", "write"], start=Times[1])
    microphone_session = make_grant("microphone_session", "audio_input", "default_source", ["capture"], kind="session",
                                    start=Times[2], session=synthetic_id("desktop session 1001"))
    screenshot_once = make_grant("screenshot_once", "screen_capture", "focused_output", ["capture_once"], kind="one_shot", start=Times[3])
    microphone_persistent = make_grant("microphone_persistent", "audio_input", "default_source", ["capture"], start=Times[4])
    clipboard_restored = make_grant("clipboard_restored", "clipboard", "session_clipboard", ["read", "write"], start=Times[6])

    chain = []
    genesis = kate.payload("genesis", "genesis", persistent, [], EmptyState,
                           {"add_grants": [documents], "remove_grant_ids": []}, Times[0], "Allow Kate to read Documents/notes")
    genesis_data, genesis_id = sign_commit(persistent, genesis)
    chain.append(("commit_genesis", genesis_data))
    revision = kate.payload("revision", "revision", persistent, [genesis_id], genesis["result_state"],
                            {"add_grants": [clipboard], "remove_grant_ids": [], "network_mode": {"from": "offline", "to": "direct"}},
                            Times[1], "Change network mode from Offline to Direct\nAllow clipboard access")
    revision_data, revision_id = sign_commit(persistent, revision)
    chain.append(("commit_revision", revision_data))
    temporary_session = kate.payload("temporary_session", "temporary_grant", temporary, [revision_id], revision["result_state"],
                                     {"add_grants": [microphone_session], "remove_grant_ids": []}, Times[2],
                                     "Grant microphone access for this session", authority="temporary")
    temporary_session_data, temporary_session_id = sign_commit(temporary, temporary_session)
    chain.append(("commit_temporary_session", temporary_session_data))
    temporary_once = kate.payload("temporary_once", "temporary_grant", temporary, [revision_id], revision["result_state"],
                                  {"add_grants": [screenshot_once], "remove_grant_ids": []}, Times[3],
                                  "Allow one screenshot of the focused output", authority="temporary")
    temporary_once_data, _ = sign_commit(temporary, temporary_once)
    chain.append(("commit_temporary_one_shot", temporary_once_data))
    merge = kate.payload("merge", "merge", persistent, [revision_id, temporary_session_id], revision["result_state"],
                         {"add_grants": [microphone_persistent], "remove_grant_ids": []}, Times[4],
                         "Make microphone access persistent (new grant)", extra={"merge_base": revision_id})
    merge_data, merge_id = sign_commit(persistent, merge)
    chain.append(("commit_merge", merge_data))
    revocation = kate.payload("revocation", "revocation", persistent, [merge_id], merge["result_state"],
                              {"add_grants": [], "remove_grant_ids": [clipboard["grant_id"]]}, Times[5], "Revoke clipboard access")
    revocation_data, revocation_id = sign_commit(persistent, revocation)
    chain.append(("commit_revocation", revocation_data))
    restore = kate.payload("restore", "restore", persistent, [revocation_id], revocation["result_state"],
                           {"add_grants": [clipboard_restored], "remove_grant_ids": []}, Times[6],
                           "Restore clipboard access from the revision of 12:05", extra={"restore_reference": revision_id})
    restore_data, restore_id = sign_commit(persistent, restore)
    chain.append(("commit_restore", restore_data))
    kate.executable = hashlib.sha256(b"synthetic executable org.kde.kate updated").digest()
    transition = kate.payload("identity_transition", "identity_transition", persistent, [restore_id], restore["result_state"],
                              {"add_grants": [], "remove_grant_ids": []}, Times[7],
                              "Kate was updated: bind the existing grants to the new executable")
    transition_data, _ = sign_commit(persistent, transition)
    chain.append(("commit_identity_transition", transition_data))

    firefox = lineage_builder_t(keys, 1002, "user_1002_identity", "org.mozilla.firefox_esr", "deb:firefox-esr:amd64")
    firefox_genesis = firefox.payload("firefox_genesis", "genesis", keys["user_1002_persistent"], [], EmptyState,
                                      {"add_grants": [], "remove_grant_ids": [], "network_mode": {"from": "offline", "to": "tor"}},
                                      Times[0], "Change network mode from Offline to Tor")
    firefox_data, _ = sign_commit(keys["user_1002_persistent"], firefox_genesis)

    invalid = build_invalid_vectors(keys, kate, genesis, genesis_data, revision, revision_id, merge, persistent, temporary)
    admission_invalid = build_admission_vectors(keys, kate, chain, genesis, genesis_id, revision, revision_id, revocation, revocation_id, persistent,
                                                restore, restore_id)
    return keys, resolver_keys, identity, chain, ("commit_user_1002_genesis", firefox_data), invalid, admission_invalid


def resign(signer, payload):
    return build_envelope("commit", signer, encode(payload))


def mutate(payload, path, value=None, delete=False):
    copy = json.loads(json.dumps(payload, default=lambda item: {"__bytes__": item.hex()}), object_hook=_restore_bytes)
    target = copy
    for part in path[:-1]:
        target = target[part]
    if delete:
        del target[path[-1]]
    else:
        target[path[-1]] = value
    return copy


def _restore_bytes(item):
    if set(item) == {"__bytes__"}:
        return bytes.fromhex(item["__bytes__"])
    return item


def splice_payload(signer, raw_payload, sign=True):
    """Envelope around arbitrary (possibly malformed) payload bytes, correctly signed."""
    return build_envelope("commit", signer, raw_payload)


def build_invalid_vectors(keys, kate, genesis, genesis_data, revision, revision_id, merge, persistent, temporary):
    vectors = []

    def add(name, data, layer, code, description, kind="commit"):
        vectors.append({"name": name, "kind": kind, "envelope": data, "layer": layer, "error": code, "description": description})

    genesis_bytes = encode(genesis)
    root = decoder_t(genesis_data, "x").decode_top(allow_root_tag=True)
    protected, _, payload_bytes, signature = root.value
    typ, aad = Kinds["commit"]
    kid = persistent.key_id

    # Layer 1: envelope decode.
    add("trailing_byte", genesis_data + b"\x00", "envelope_decode", "trailing_bytes", "one byte after the envelope")
    add("truncated_envelope", genesis_data[:-1], "envelope_decode", "truncated", "last signature byte missing")
    add("oversized_envelope", encode(tag_t(18, [protected, {}, b"\x00" * 65600, signature])), "envelope_decode", "input_too_large", "envelope larger than 65536 bytes")
    add("indefinite_array", b"\xd2\x9f" + encode(protected) + b"\xa0" + encode(payload_bytes) + encode(signature) + b"\xff",
        "envelope_decode", "indefinite_length", "COSE array with indefinite length")
    add("non_minimal_tag", b"\xd8\x12" + genesis_data[1:], "envelope_decode", "non_minimal_encoding", "tag 18 encoded in two bytes")
    add("cose_sign_tag_98", b"\xd8\x62" + genesis_data[1:], "envelope_decode", "tag_forbidden", "COSE_Sign (tag 98) instead of COSE_Sign1")
    add("reserved_additional_info", b"\xd2\x9c", "envelope_decode", "reserved_additional_info", "array with additional information 28")
    add("protected_header_unsorted", build_envelope("commit", persistent, payload_bytes,
        protected=head(5, 3) + encode(LabelKid) + encode(kid) + encode(LabelAlg) + encode(AlgorithmEd25519) + encode(LabelTyp) + encode(typ)),
        "envelope_decode", "unsorted_or_duplicate_key", "protected header keys out of order (signed)")
    add("protected_header_float_alg", build_envelope("commit", persistent, payload_bytes,
        protected=head(5, 3) + encode(LabelAlg) + b"\xf9\x3c\x00" + encode(LabelKid) + encode(kid) + encode(LabelTyp) + encode(typ)),
        "envelope_decode", "float_forbidden", "alg encoded as a float")
    # Layer 2: envelope.
    add("untagged_array", encode([protected, {}, payload_bytes, signature]), "envelope", "not_signed_object", "COSE_Sign1 without tag 18")
    add("three_element_array", encode(tag_t(18, [protected, {}, payload_bytes])), "envelope", "not_signed_object", "missing signature element")
    add("protected_as_map", encode(tag_t(18, [{LabelAlg: AlgorithmEd25519}, {}, payload_bytes, signature])), "envelope", "not_signed_object", "protected header not wrapped in a byte string")
    add("alg_polymorphic_eddsa", build_envelope("commit", persistent, payload_bytes, protected=protected_bytes(kid, typ, AlgorithmEdDsaPolymorphic)),
        "envelope", "unsupported_algorithm", "alg -8 (deprecated polymorphic EdDSA)")
    add("extra_protected_label", build_envelope("commit", persistent, payload_bytes, protected=encode({LabelAlg: AlgorithmEd25519, 3: "application/cbor", LabelKid: kid, LabelTyp: typ})),
        "envelope", "protected_header_invalid", "content-type label added to the protected header")
    add("kid_wrong_length", build_envelope("commit", persistent, payload_bytes, protected=encode({LabelAlg: AlgorithmEd25519, LabelKid: kid[:16], LabelTyp: typ})),
        "envelope", "protected_header_invalid", "16-byte kid")
    add("typ_key_certificate", build_envelope("commit", persistent, payload_bytes, protected=protected_bytes(kid, Kinds["key_certificate"][0])),
        "envelope", "wrong_object_type", "commit payload labeled as a key certificate")
    add("typ_unknown", build_envelope("commit", persistent, payload_bytes, protected=protected_bytes(kid, "application/cbor")),
        "envelope", "protected_header_invalid", "unregistered typ")
    add("unprotected_kid", encode(tag_t(18, [protected, {LabelKid: kid}, payload_bytes, signature])), "envelope", "unprotected_not_empty", "kid duplicated in the unprotected header")
    add("signature_63_bytes", encode(tag_t(18, [protected, {}, payload_bytes, signature[:63]])), "envelope", "signature_length", "63-byte signature")
    # Layer 3: signature.
    flipped = bytes([signature[0] ^ 1]) + signature[1:]
    add("signature_bit_flip", encode(tag_t(18, [protected, {}, payload_bytes, flipped])), "signature", "signature_invalid", "first signature bit changed")
    changed_payload = encode(mutate(genesis, ["message"], "Allow Kate to read Documents/notez"))
    add("payload_changed_after_signing", encode(tag_t(18, [protected, {}, changed_payload, signature])), "signature", "signature_invalid", "message edited, original signature kept")
    other = keys["user_1002_persistent"]
    add("signed_by_other_user", build_envelope("commit", other, payload_bytes, protected=protected), "signature", "signature_invalid", "user 1002 signs bytes naming user 1001's kid")
    add("unknown_kid", build_envelope("commit", persistent, payload_bytes, protected=protected_bytes(synthetic_id("unknown key") * 2, typ)), "signature", "unknown_key", "kid not in the key set")
    s_value = int.from_bytes(signature[32:], "little") + GroupOrder
    add("non_canonical_s", encode(tag_t(18, [protected, {}, payload_bytes, signature[:32] + s_value.to_bytes(32, "little")])), "signature", "signature_invalid", "S + L (malleated signature)")
    add("key_certificate_domain", build_envelope("commit", persistent, payload_bytes, external_aad=Kinds["key_certificate"][1]), "signature", "signature_invalid", "signed with the key-certificate external_aad")
    # Layer 4: authenticated payload decode.
    add("payload_non_minimal_integer", splice_payload(persistent, genesis_bytes.replace(b"\x6eschema_version\x01", b"\x6eschema_version\x18\x01")),
        "payload_decode", "non_minimal_encoding", "schema_version 1 encoded in two bytes (signed)")
    add("payload_trailing_byte", splice_payload(persistent, genesis_bytes + b"\x00"), "payload_decode", "trailing_bytes", "byte after the payload map (signed)")
    add("payload_indefinite_map", splice_payload(persistent, b"\xbf" + genesis_bytes[1:] + b"\xff"), "payload_decode", "indefinite_length", "indefinite-length payload map (signed)")
    duplicate = head(5, 2) + encode("message") + encode("a") + encode("message") + encode("b")
    add("payload_duplicate_key", splice_payload(persistent, duplicate), "payload_decode", "unsorted_or_duplicate_key", "duplicate message key (signed)")
    unsorted = head(5, 2) + encode("timestamp") + encode(Times[0]) + encode("actor") + encode({"uid": 1001})
    add("payload_unsorted_keys", splice_payload(persistent, unsorted), "payload_decode", "unsorted_or_duplicate_key", "keys out of deterministic order (signed)")
    add("payload_float", splice_payload(persistent, head(5, 1) + encode("schema_version") + b"\xf9\x3c\x00"), "payload_decode", "float_forbidden", "schema_version as half-float 1.0 (signed)")
    add("payload_null", splice_payload(persistent, head(5, 1) + encode("message") + b"\xf6"), "payload_decode", "simple_value_forbidden", "null value (signed)")
    add("payload_inner_tag", splice_payload(persistent, head(5, 1) + encode("timestamp") + b"\xc0" + encode(Times[0])), "payload_decode", "tag_forbidden", "tag 0 date string (signed)")
    add("payload_invalid_utf8", splice_payload(persistent, head(5, 1) + encode("message") + b"\x62\xc3\x28"), "payload_decode", "invalid_utf8", "invalid UTF-8 continuation (signed)")
    add("payload_overlong_utf8", splice_payload(persistent, head(5, 1) + encode("message") + b"\x62\xc0\xaf"), "payload_decode", "invalid_utf8", "overlong encoding of '/' (signed)")
    add("payload_surrogate_utf8", splice_payload(persistent, head(5, 1) + encode("message") + b"\x63\xed\xa0\x80"), "payload_decode", "invalid_utf8", "encoded UTF-16 surrogate (signed)")
    add("payload_depth", splice_payload(persistent, head(5, 1) + encode("message") + b"\x81" * 8 + b"\x00"), "payload_decode", "depth_exceeded", "nine nested containers (signed)")
    add("payload_container_size", splice_payload(persistent, head(5, 1) + encode("parents") + head(4, 257) + b"\x00" * 257), "payload_decode", "container_too_large", "257-element array (signed)")
    add("payload_text_too_long", splice_payload(persistent, head(5, 1) + encode("message") + encode("x" * 1025)), "payload_decode", "text_too_long", "1025-byte text (signed)")
    add("payload_integer_key", splice_payload(persistent, head(5, 2) + encode(1) + encode(1) + encode("message") + encode("a")), "payload_decode", "map_key_type", "mixed integer and text keys (signed)")
    add("payload_negative_out_of_range", splice_payload(persistent, head(5, 1) + encode("schema_version") + b"\x3b\x80\x00\x00\x00\x00\x00\x00\x00"), "payload_decode", "integer_out_of_range", "negative integer below -2^63 (signed)")
    # Layer 4: schema.
    schema_cases = [
        ("unknown_field", mutate(genesis, ["extra"], 1), "unknown_field", "unknown top-level field"),
        ("missing_message", mutate(genesis, ["message"], delete=True), "missing_field", "message absent"),
        ("schema_version_text", mutate(genesis, ["schema_version"], "1"), "wrong_type", "schema_version as text"),
        ("schema_version_2", mutate(genesis, ["schema_version"], 2), "invalid_enum", "unsupported schema version"),
        ("invalid_calendar_date", mutate(genesis, ["timestamp"], "2026-02-30T12:00:00Z"), "invalid_time", "30 February"),
        ("leap_second", mutate(genesis, ["timestamp"], "2026-06-30T23:59:60Z"), "invalid_time", "second 60"),
        ("lowercase_time", mutate(genesis, ["timestamp"], "2026-10-12t12:00:00z"), "invalid_time", "lowercase separators"),
        ("bidi_message", mutate(genesis, ["message"], "Allow Kate ‮readable"), "invalid_text", "right-to-left override in message"),
        ("message_control", mutate(genesis, ["message"], "Allow\tKate"), "invalid_text", "tab in message"),
        ("uppercase_app_id", mutate(genesis, ["subject", "app_id"], "org.KDE.kate"), "invalid_text", "uppercase application identifier"),
        ("short_installation_id", mutate(genesis, ["subject", "installation_id"], b"\x01" * 15), "value_out_of_range", "15-byte installation identifier"),
        ("commit_type_unknown", mutate(genesis, ["commit_type"], "upgrade"), "invalid_enum", "unknown commit type"),
        ("uid_out_of_range", mutate(genesis, ["actor", "uid"], 4294967295), "value_out_of_range", "uid -1 (4294967295)"),
        ("contract_type_unknown", mutate(genesis, ["delta", "add_grants", 0, "contract_type"], "anything"), "invalid_enum", "unknown contract type"),
        ("usage_limit_two", mutate(genesis, ["delta", "add_grants", 0, "lifetime", "usage_limit"], 2), "invalid_enum", "usage_limit 2"),
        ("empty_operations", mutate(genesis, ["delta", "add_grants", 0, "resource", "operations"], []), "list_length", "grant without operations"),
        ("too_many_parents", mutate(genesis, ["parents"], sorted(synthetic_id("parent %d" % index) * 2 for index in range(9))), "list_length", "nine parents"),
        ("unsorted_parents", mutate(genesis, ["parents"], [b"\x02" * 32, b"\x01" * 32]), "list_not_strictly_ascending", "parents out of canonical order"),
        ("duplicate_operations", mutate(genesis, ["delta", "add_grants", 0, "resource", "operations"], ["read", "read"]), "list_not_strictly_ascending", "duplicate operation"),
        ("key_id_mismatch", mutate(genesis, ["key_id"], keys["user_1001_temporary"].key_id), "key_id_mismatch", "payload key_id differs from kid"),
        ("subject_actor_mismatch", mutate(genesis, ["actor", "uid"], 1002), "subject_actor_mismatch", "actor uid differs from owner uid"),
        ("lineage_mismatch", mutate(genesis, ["subject", "app_id"], "org.kde.dolphin"), "lineage_mismatch", "lineage_id derived from another application"),
    ]
    for name, payload, code, description in schema_cases:
        add("schema_" + name, resign(persistent, payload), "schema", code, description + " (signed)")
    # Layer 5: intrinsic semantics (all correctly signed and schema-valid).
    temporary_persistent = mutate(genesis, ["commit_type"], "temporary_grant")
    temporary_persistent["parents"] = [revision_id]
    intrinsic_cases = [
        ("genesis_with_parent", mutate(genesis, ["parents"], [revision_id]), persistent, "commit_type_rule", "genesis with a parent"),
        ("temporary_adds_persistent", temporary_persistent, persistent, "commit_type_rule", "temporary_grant adding a persistent grant"),
        ("temporary_key_revision", mutate(mutate(revision, ["authority_class"], "temporary"), ["key_id"], temporary.key_id), temporary, "commit_type_rule", "temporary authority signs a revision"),
        ("revocation_with_add", mutate(mutate(revision, ["commit_type"], "revocation"), ["delta", "network_mode"], delete=True), persistent, "commit_type_rule", "revocation that adds a grant"),
        ("merge_without_merge_base", mutate(merge, ["merge_base"], delete=True), persistent, "commit_type_rule", "merge without merge_base"),
        ("restore_reference_on_revision", mutate(revision, ["restore_reference"], revision_id), persistent, "commit_type_rule", "restore_reference on a revision"),
        ("empty_revision", mutate(mutate(mutate(revision, ["delta", "add_grants"], []), ["delta", "network_mode"], delete=True), ["result_state"], revision["result_state"]), persistent, "commit_type_rule", "revision without any change"),
        ("timed_without_expiration", None, persistent, "lifetime_rule", "timed grant without expiration"),
        ("expiration_before_start", mutate(mutate(genesis, ["delta", "add_grants", 0, "lifetime", "expiration"], "2026-10-12T11:00:00Z"), ["result_state", "grants", 0, "lifetime", "expiration"], "2026-10-12T11:00:00Z"), persistent, "lifetime_rule", "expiration before effective start"),
        ("delegation_scope_not_permitted", mutate(mutate(genesis, ["delta", "add_grants", 0, "delegation", "permitted"], False), ["result_state", "grants", 0, "delegation", "permitted"], False), persistent, "delegation_rule", "non-delegable grant with delegation depth"),
        ("delegation_exceeds_operations", mutate(mutate(genesis, ["delta", "add_grants", 0, "delegation", "max_operations"], ["write"]), ["result_state", "grants", 0, "delegation", "max_operations"], ["write"]), persistent, "delegation_rule", "delegable operations not in the grant"),
        ("network_from_equals_to", mutate(revision, ["delta", "network_mode", "to"], "offline"), persistent, "network_change_rule", "network change from offline to offline"),
        ("added_grant_missing_from_result", mutate(genesis, ["result_state", "grants"], []), persistent, "result_state_mismatch", "added grant absent from result state"),
        ("genesis_base_not_empty", mutate(genesis, ["base_state_hash"], b"\x00" * 32), persistent, "base_state_mismatch", "genesis base is not the empty state"),
        ("identity_transition_with_add", mutate(mutate(revision, ["commit_type"], "identity_transition"), ["delta", "network_mode"], delete=True), persistent, "commit_type_rule", "identity transition that also adds a grant"),
    ]
    timed = make_grant("timed_bad", "camera", "front_camera", ["capture"], kind="timed", start=Times[3])
    timed_payload = mutate(revision, ["commit_type"], "temporary_grant")
    timed_payload["delta"] = {"add_grants": [timed], "remove_grant_ids": []}
    timed_payload["result_state"] = apply_delta(revision["result_state"], timed_payload["delta"])
    timed_payload["parents"] = [revision_id]
    timed_payload["base_state_hash"] = state_hash(revision["result_state"])
    for name, payload, signer, code, description in intrinsic_cases:
        if payload is None:
            payload = timed_payload
        add("intrinsic_" + name, resign(signer, payload), "intrinsic", code, description + " (signed)")
    # Identity objects with wrong binding.
    identity = keys["user_1001_identity"]
    enrollment = {"schema_version": 1, "user_id": identity.key_id, "public_key": keys["user_1002_identity"].public_key,
                  "owner_uid": 1001, "enrolled": Times[0], "identity_epoch": 1}
    add("enrollment_public_key_mismatch", build_envelope("identity_enrollment", identity, encode(enrollment)), "schema", "key_id_mismatch",
        "enrollment names a public key that does not hash to user_id (signed)", kind="identity_enrollment")
    certificate = {"schema_version": 1, "certificate_id": synthetic_id("bad certificate"), "issued": Times[0], "owner_uid": 1001,
                   "owner_user_id": identity.key_id, "subject_key_id": keys["user_1001_persistent"].key_id,
                   "subject_public_key": keys["user_1001_temporary"].public_key, "authority_class": "persistent",
                   "key_epoch": 1, "not_before": Times[0]}
    add("certificate_subject_mismatch", build_envelope("key_certificate", identity, encode(certificate)), "schema", "key_id_mismatch",
        "certificate subject_key_id differs from its public key (signed)", kind="key_certificate")
    return vectors


def build_admission_vectors(keys, kate, chain, genesis, genesis_id, revision, revision_id, revocation, revocation_id, persistent,
                            restore, restore_id):
    """Lineage-context failures: each entry is a prefix of the valid chain plus one bad commit."""
    vectors = []
    prefix = [data for _, data in chain]

    def add(name, prefix_length, payload, code, description):
        vectors.append({"name": name, "prefix": prefix_length, "envelope": resign(persistent, payload), "layer": "admission", "error": code, "description": description})

    stale_base = mutate(revision, ["base_state_hash"], state_hash(EmptyState))
    add("stale_base_state", 1, stale_base, "base_state_mismatch", "revision claims the empty state as its base")
    phantom = make_grant("phantom", "camera", "front_camera", ["capture"], start=Times[1])
    wrong_result = mutate(revision, ["result_state", "grants"], sorted(revision["result_state"]["grants"] + [phantom], key=lambda grant: grant["grant_id"]))
    add("result_not_base_plus_delta", 1, wrong_result, "result_state_mismatch", "result state contains a camera grant the delta does not add")
    unknown_parent = mutate(revision, ["parents"], [b"\x07" * 32])
    add("unknown_parent", 1, unknown_parent, "parent_unknown", "parent not in the lineage")
    replayed_grant = mutate(revocation, ["commit_type"], "revision")
    replayed_grant["delta"] = {"add_grants": [g for g in revision["result_state"]["grants"] if g["grant_id"] == revision["delta"]["add_grants"][0]["grant_id"]], "remove_grant_ids": []}
    replayed_grant["parents"] = [revocation_id]
    replayed_grant["base_state_hash"] = state_hash(revocation["result_state"])
    replayed_grant["result_state"] = apply_delta(revocation["result_state"], replayed_grant["delta"])
    replayed_grant["authorization"]["request_id"] = synthetic_id("request replayed clipboard")
    add("revoked_grant_id_re_added", 6, replayed_grant, "duplicate_identifier", "revoked clipboard grant re-added with its old grant_id")
    replayed_request = mutate(revision, ["message"], "Same request identifier as genesis")
    replayed_request["authorization"]["request_id"] = genesis["authorization"]["request_id"]
    add("request_id_replayed", 1, replayed_request, "duplicate_identifier", "authorization request_id reused")
    expanded_restore = mutate(revocation, ["commit_type"], "restore")
    expanded_restore["restore_reference"] = revision_id
    clipboard_wide = make_grant("clipboard_wide", "clipboard", "session_clipboard", ["read", "write", "write_primary"], start=Times[6])
    expanded_restore["delta"] = {"add_grants": [clipboard_wide], "remove_grant_ids": []}
    expanded_restore["parents"] = [revocation_id]
    expanded_restore["base_state_hash"] = state_hash(revocation["result_state"])
    expanded_restore["result_state"] = apply_delta(revocation["result_state"], expanded_restore["delta"])
    expanded_restore["authorization"]["request_id"] = synthetic_id("request expanded restore")
    add("restore_expands_history", 6, expanded_restore, "restore_rule", "restore adds an operation absent from the referenced state")
    unchanged = mutate(restore, ["commit_type"], "identity_transition")
    del unchanged["restore_reference"]
    unchanged["delta"] = {"add_grants": [], "remove_grant_ids": []}
    unchanged["parents"] = [restore_id]
    unchanged["base_state_hash"] = state_hash(restore["result_state"])
    unchanged["result_state"] = restore["result_state"]
    unchanged["authorization"]["request_id"] = synthetic_id("request unchanged transition")
    add("identity_transition_unchanged", 7, unchanged, "identity_transition_rule", "identity transition that changes neither package nor executable")
    return vectors


# ------------------------------------------------------------------ serialization and check


def vector_document():
    keys, resolver_keys, identity, chain, cross_user, invalid, admission_invalid = build_vectors()
    resolver = resolver_t(resolver_keys)
    document = {
        "format": "cham3 ECR v1 vectors",
        "status": "PROPOSED G2 vectors (ADR 0004). Synthetic keys for tests only; not product data.",
        "specification": "docs/specs/m2_wire_format_v1.md",
        "constants": {
            "empty_state_hash": state_hash(EmptyState).hex(),
            "external_aad": {kind: aad.decode() for kind, (_, aad) in Kinds.items()},
            "typ": {kind: typ for kind, (typ, _) in Kinds.items()},
        },
        "keys": [
            {"label": key.label, "seed": key.seed.hex(), "public_key": key.public_key.hex(), "key_id": key.key_id.hex()}
            for key in resolver_keys
        ],
        "valid": [],
        "lineage": {"name": "user_1001_kate", "commits": [name for name, _ in chain]},
        "invalid": [],
        "admission_invalid": [],
    }
    for item in identity:
        document["valid"].append({"name": item["name"], "kind": item["kind"], "envelope": item["envelope"].hex(),
                                  "object_id": hashlib.sha256(item["envelope"]).hexdigest()})
    for name, data in chain + [cross_user]:
        payload = verify(data, "commit", resolver)
        document["valid"].append({"name": name, "kind": "commit", "envelope": data.hex(), "object_id": hashlib.sha256(data).hexdigest(),
                                  "result_state_hash": state_hash(payload["result_state"]).hex()})
    for item in invalid:
        document["invalid"].append({key: (value.hex() if isinstance(value, bytes) else value) for key, value in item.items()})
    for item in admission_invalid:
        document["admission_invalid"].append({key: (value.hex() if isinstance(value, bytes) else value) for key, value in item.items()})
    return document, resolver


def check_document(path):
    with open(path, "r", encoding="utf-8") as handle:
        stored = json.load(handle)
    regenerated, resolver = vector_document()
    failures = []
    if stored != regenerated:
        failures.append("stored vectors differ from deterministic regeneration")
    by_name = {item["name"]: bytes.fromhex(item["envelope"]) for item in stored["valid"]}
    for item in stored["valid"]:
        data = bytes.fromhex(item["envelope"])
        try:
            verify(data, item["kind"], resolver)
        except oracle_error_t as error:
            failures.append("valid %s rejected: %s" % (item["name"], error))
            continue
        try:
            decoded = cbor2.loads(data)
            if cbor2.dumps(decoded, canonical=True) != data:
                failures.append("valid %s is not cbor2 canonical" % item["name"])
            inner = cbor2.loads(decoded.value[2])
            if cbor2.dumps(inner, canonical=True) != decoded.value[2]:
                failures.append("valid %s payload is not cbor2 canonical" % item["name"])
        except Exception as error:  # noqa: BLE001 - any cbor2 failure is a cross-check failure
            failures.append("cbor2 rejected %s: %s" % (item["name"], error))
    try:
        admit_chain([by_name[name] for name in stored["lineage"]["commits"]], resolver)
    except oracle_error_t as error:
        failures.append("valid lineage rejected: %s" % error)
    for item in stored["invalid"]:
        try:
            verify(bytes.fromhex(item["envelope"]), item["kind"], resolver)
            failures.append("invalid %s accepted" % item["name"])
        except oracle_error_t as error:
            if (error.layer, error.code) != (item["layer"], item["error"]):
                failures.append("invalid %s: expected %s/%s got %s/%s" % (item["name"], item["layer"], item["error"], error.layer, error.code))
    chain = [by_name[name] for name in stored["lineage"]["commits"]]
    for item in stored["admission_invalid"]:
        try:
            admit_chain(chain[: item["prefix"]] + [bytes.fromhex(item["envelope"])], resolver)
            failures.append("admission %s accepted" % item["name"])
        except oracle_error_t as error:
            if (error.layer, error.code) != (item["layer"], item["error"]):
                failures.append("admission %s: expected %s/%s got %s/%s" % (item["name"], item["layer"], item["error"], error.layer, error.code))
    for failure in failures:
        print("FAIL " + failure)
    print("checked %d valid, %d invalid, %d admission vectors: %s" % (
        len(stored["valid"]), len(stored["invalid"]), len(stored["admission_invalid"]), "FAIL" if failures else "PASS"))
    return 1 if failures else 0


def main(arguments):
    if len(arguments) != 2 or arguments[0] not in ("generate", "check"):
        print("usage: ecr_oracle.py generate|check VECTORS.json", file=sys.stderr)
        return 2
    if arguments[0] == "generate":
        document, _ = vector_document()
        with open(arguments[1], "w", encoding="utf-8") as handle:
            json.dump(document, handle, indent=1, sort_keys=True)
            handle.write("\n")
        print("wrote %d valid, %d invalid, %d admission vectors" % (len(document["valid"]), len(document["invalid"]), len(document["admission_invalid"])))
        return 0
    return check_document(arguments[1])


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
