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

# R08: signed objects are at most 61,440 bytes so that the largest valid object plus
# bounded wrapper metadata (<= 4,096 bytes) fits a 65,536-byte IPC message or journal
# record. Envelope, protected-header and payload decodes all use this limit.
MaxEnvelopeBytes = 61440
MaxWrapperBytes = 65536
MaxWrapperMetadataBytes = 4096
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
        if len(self.data) > MaxEnvelopeBytes:
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
            if argument > MaxEnvelopeBytes:
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


# R07: network authority is a grant; no network grant means Offline (PROJECT §3.2).
EmptyState = {"grants": []}


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
    "category": (True, lambda value, path: check_enum(value, path, {"filesystem", "device", "audio_input", "camera", "screen_capture", "clipboard", "portal", "ipc", "notification", "network"})),
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


NetworkOperations = {"direct", "tor"}
StateFields = {
    "grants": (True, grant_list_check(256)),
}
IdentityReferenceFields = {
    "package_identity": (True, lambda value, path: check_package_identity(value, path)),
    "executable_sha256": (True, lambda value, path: digest_check(value, path)),
}
DeltaFields = {
    "add_grants": (True, grant_list_check(64)),
    "remove_grant_ids": (True, lambda value, path: check_list(value, path, 0, 64, random_id_check)),
    "identity_change": (False, lambda value, path: check_map(value, path, {
        "from": (True, lambda item, item_path: check_map(item, item_path, IdentityReferenceFields)),
        "to": (True, lambda item, item_path: check_map(item, item_path, IdentityReferenceFields)),
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


def is_network_grant(grant):
    return grant["resource"]["category"] == "network"


def check_network_grant(grant, owner_uid):
    """R07: network authority is a grant with full lifetime and revocation semantics."""
    resource = grant["resource"]
    if resource["resource_id"] != "network_mode":
        intrinsic_error("network_grant_rule", "network resource_id must be network_mode")
    if len(resource["operations"]) != 1 or resource["operations"][0] not in NetworkOperations:
        intrinsic_error("network_grant_rule", "exactly one of direct/tor")
    if resource["resource_owner_uid"] != owner_uid:
        intrinsic_error("network_grant_rule", "network grant owner")
    if grant["delegation"]["permitted"]:
        intrinsic_error("network_grant_rule", "network grants are not delegable")
    if grant["lifetime"]["kind"] == "one_shot":
        intrinsic_error("network_grant_rule", "one-shot network grant")


def check_intrinsic(payload):
    """Layer 5 for commits."""
    commit_type = payload["commit_type"]
    parents = payload["parents"]
    delta = payload["delta"]
    adds = delta["add_grants"]
    removes = delta["remove_grant_ids"]
    identity_change = delta.get("identity_change")
    authority = payload["authority_class"]
    add_kinds = {grant["lifetime"]["kind"] for grant in adds}
    rules = {
        "genesis": (len(parents) == 0, authority == "persistent", add_kinds <= {"persistent"} and not removes),
        "revision": (len(parents) == 1, authority == "persistent", add_kinds <= {"persistent"} and bool(adds or removes)),
        "revocation": (len(parents) == 1, authority == "persistent", not adds and bool(removes)),
        "temporary_grant": (len(parents) == 1, True, bool(adds) and add_kinds <= TemporaryKinds and not removes),
        "merge": (2 <= len(parents) <= 8, authority == "persistent", add_kinds <= {"persistent"} and payload.get("merge_base") in parents),
        "restore": (len(parents) == 1, authority == "persistent", add_kinds <= {"persistent"}),
        "identity_transition": (len(parents) == 1, authority == "persistent", not adds and not removes),
    }
    if not all(rules[commit_type]):
        intrinsic_error("commit_type_rule", commit_type)
    if ("merge_base" in payload) != (commit_type == "merge") or ("restore_reference" in payload) != (commit_type == "restore"):
        intrinsic_error("commit_type_rule", "merge_base/restore_reference placement")
    if (identity_change is not None) != (commit_type == "identity_transition"):
        intrinsic_error("commit_type_rule", "identity_change only and always on identity_transition")
    for grant in adds + payload["result_state"]["grants"]:
        check_lifetime(grant)
    for grant in adds + payload["result_state"]["grants"]:
        check_delegation(grant)
    result = payload["result_state"]
    owner_uid = payload["subject"]["owner_uid"]
    for grant in adds + result["grants"]:
        if is_network_grant(grant):
            check_network_grant(grant, owner_uid)
    if sum(1 for grant in result["grants"] if is_network_grant(grant)) > 1:
        intrinsic_error("network_grant_rule", "more than one network grant in state")
    if identity_change is not None:
        new_identity = subject_identity(payload["subject"])
        if encode(identity_change["to"]) != encode(new_identity) or encode(identity_change["from"]) == encode(identity_change["to"]):
            intrinsic_error("identity_transition_rule", "identity_change must end at the new subject and differ from its start")
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


def subject_identity(subject):
    return {"package_identity": subject["package_identity"], "executable_sha256": subject["executable_sha256"]}


def verify(data, expected_kind, resolver):
    kid, payload_bytes = parse_and_verify(data, expected_kind, resolver)
    payload = check_payload(expected_kind, kid, payload_bytes)
    if expected_kind == "commit":
        check_intrinsic(payload)
    return payload


# ------------------------------------------------------------------ registry journal model (admission subset)
#
# Revision 3 (review r2: R07-r2, R12, R13). One user's journal: signed identity objects
# and commits plus the unsigned registry records that only narrow authority (lifecycle,
# containment, recovery, deletion). Wire format sections 6.6, 6.7, 6.10, 6.11 and 6.12;
# ADR 0006 sections 3, 4 and 10; ADR 0007 sections 1-3. A rejected entry changes
# nothing. This is a model of admission and key-authority decisions for vectors; it is
# not the registry, policy or identity service, and it does not evaluate time,
# sessions, mandatory policy or policy epochs.


def admission_error(code, detail=""):
    raise oracle_error_t("admission", code, detail)


def key_authority_error(code, detail=""):
    raise oracle_error_t("key_authority", code, detail)


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
    return {"grants": [grants[key] for key in sorted(grants)]}


RecoveryCommitTypes = {"restore", "revocation"}
# Unsigned registry records the model accepts; every one of them only narrows authority.
RecordKinds = {"expired", "session_ended", "restriction", "containment", "quarantine", "admin_recover",
               "recovery_opened", "recovery_keys", "recovery_closed", "deletion_pending"}
RecordByteFields = {"grant_id", "grant_ids", "session_id", "recovery_id", "persistent_key_id", "temporary_key_id"}


def record_error(code, detail=""):
    admission_error(code, detail)


class lineage_state_t:
    def __init__(self):
        self.trunk_head = None
        # Live temporary branch heads: head commit id -> grant ids added on that branch.
        # A trunk commit ends every temporary branch (ADR 0007 section 1).
        self.branches = {}
        self.authoritative_subject = None
        self.seen_grants = set()
        self.containment_head = None
        self.recovered = False


class key_record_t:
    def __init__(self, certificate):
        self.authority_class = certificate["authority_class"]
        self.key_epoch = certificate["key_epoch"]
        self.not_before = certificate["not_before"]
        self.not_after = certificate.get("not_after")
        self.invalidated = None
        self.retired = False


class journal_t:
    """Admission model of one user's journal (and, for admit_chain, of bare lineages).

    key_authority=False reproduces the revision-2 entry point admit_chain: commits only,
    no identity records and no key-authority layer. key_authority=True requires the
    user's enrollment and certificates before any commit and applies layer 6.
    """

    def __init__(self, resolver, key_authority=False):
        self.resolver = resolver
        self.key_authority = key_authority
        self.commits = {}
        self.lineages = {}
        self.seen_requests = set()
        self.grants = {}
        self.grant_lineage = {}
        self.ended = {}
        self.user_id = None
        self.uid = None
        self.keys = {}
        self.min_key_epoch = 0
        self.state = "normal"  # normal / contained / quarantined / deleted
        self.recovery = None
        self.recovery_epochs = set()

    # ------------------------------------------------------------ queries

    def live_states(self, lineage):
        states = []
        if lineage.trunk_head is not None:
            states.append(self.commits[lineage.trunk_head]["result_state"])
        states.extend(self.commits[head]["result_state"] for head in lineage.branches)
        return states

    def standing(self, grant_id):
        """current: in a live state and not ended; ended: a terminal record names it;
        inactive: removed, superseded or on an ended branch; unknown: never admitted."""
        if grant_id in self.ended:
            return "ended"
        if grant_id not in self.grants:
            return "unknown"
        lineage = self.lineages[self.grant_lineage[grant_id]]
        for state in self.live_states(lineage):
            if any(grant["grant_id"] == grant_id for grant in state["grants"]):
                return "current"
        return "inactive"

    def ancestors(self, commit_id):
        seen = set()
        pending = [commit_id]
        while pending:
            for parent in self.commits[pending.pop()]["parents"]:
                if parent not in seen:
                    seen.add(parent)
                    pending.append(parent)
        return seen

    def occupying_network_grants(self, states):
        """R07-r2: network grants that occupy the lineage's single network slot."""
        return {grant["grant_id"] for state in states for grant in state["grants"]
                if is_network_grant(grant) and grant["grant_id"] not in self.ended}

    # ------------------------------------------------------------ key authority (layer 6)

    def check_key_authority(self, payload):
        if self.user_id is None:
            key_authority_error("key_not_certified", "no enrollment in this journal")
        key = self.keys.get(payload["key_id"])
        if key is None or payload["actor"]["user_id"] != self.user_id or payload["actor"]["uid"] != self.uid:
            key_authority_error("key_not_certified", "signing key not certified for this user")
        if key.invalidated is not None:
            key_authority_error("key_invalidated", key.invalidated)
        if key.key_epoch < self.min_key_epoch:
            key_authority_error("key_epoch_stale", "key certified before the recovery key epoch")
        if key.retired:
            key_authority_error("key_retired", "key superseded by the named recovery keys")
        if key.authority_class == "identity" or payload["authority_class"] != key.authority_class:
            key_authority_error("authority_class_denied", "authority class differs from the certificate")
        if key.authority_class == "temporary" and payload["commit_type"] != "temporary_grant":
            key_authority_error("authority_class_denied", "temporary key outside temporary_grant")
        stamp = parse_time(payload["timestamp"])
        if stamp < parse_time(key.not_before) or (key.not_after and stamp > parse_time(key.not_after)):
            key_authority_error("certificate_window", "commit outside the certificate validity window")
        # User state after key validity: deletion, quarantine and containment answer
        # denied for every key; an open recovery admits only its named persistent key.
        if self.state == "deleted":
            key_authority_error("user_deleted")
        if self.state == "quarantined":
            key_authority_error("quarantined")
        if self.state == "contained":
            if self.recovery is None:
                key_authority_error("contained", "no recovery ceremony is open")
            if self.recovery.get("persistent_key_id") != payload["key_id"]:
                key_authority_error("recovery_key_required", "only the ceremony's named recovery key signs while contained")

    def admit_identity(self, data, kind):
        payload = verify(data, kind, self.resolver)
        if kind == "identity_enrollment":
            if self.user_id is not None:
                key_authority_error("duplicate_enrollment")
            self.user_id = payload["user_id"]
            self.uid = payload["owner_uid"]
            identity = key_record_t({"authority_class": "persistent", "key_epoch": 0, "not_before": payload["enrolled"]})
            identity.authority_class = "identity"
            self.keys[payload["user_id"]] = identity
            return payload
        if self.user_id is None or payload["owner_user_id"] != self.user_id:
            key_authority_error("key_not_certified", "identity object for another or unenrolled user")
        if self.state in ("deleted", "quarantined"):
            key_authority_error("user_deleted" if self.state == "deleted" else "quarantined")
        if kind == "key_invalidation":
            # Always permitted while enrolled: invalidation only narrows authority.
            target = self.keys.get(payload["target_key_id"])
            if target is None or target.authority_class == "identity":
                key_authority_error("key_not_certified", "invalidation of an unknown key")
            target.invalidated = target.invalidated or payload["reason"]
            return payload
        if payload["owner_uid"] != self.uid or payload["subject_key_id"] in self.keys:
            key_authority_error("key_not_certified", "certificate owner or duplicate key")
        if self.state == "contained":
            if self.recovery is None:
                key_authority_error("contained", "certificates while contained only inside a recovery ceremony")
            if payload["key_epoch"] != self.recovery["key_epoch"] or "persistent_key_id" in self.recovery:
                admission_error("recovery_rule", "recovery certificate epoch or ceremony step")
        elif payload["key_epoch"] < max([self.min_key_epoch, 1]):
            key_authority_error("key_epoch_stale", "certificate below the current key epoch")
        self.keys[payload["subject_key_id"]] = key_record_t(payload)
        return payload

    # ------------------------------------------------------------ commits (layer 7)

    def placement(self, lineage, payload):
        """Wire format 6.11: returns ("trunk", None) or ("branch", previous head or None)."""
        commit_type = payload["commit_type"]
        parents = payload["parents"]
        if commit_type == "genesis":
            if lineage.trunk_head is not None:
                admission_error("stale_head", "genesis for an existing lineage")
            return "trunk", None
        if commit_type == "merge":
            others = [parent for parent in parents if parent != payload["merge_base"]]
            if payload["merge_base"] != lineage.trunk_head or any(parent not in lineage.branches for parent in others):
                admission_error("stale_head", "merge base is not the trunk head or a parent is not a live temporary head")
            return "trunk", None
        parent = parents[0]
        if parent == lineage.trunk_head and commit_type != "temporary_grant":
            return "trunk", None
        if parent == lineage.trunk_head:
            return "branch", None
        if parent in lineage.branches and commit_type in ("temporary_grant", "revocation"):
            return "branch", parent
        admission_error("stale_head", "parent is not the trunk head or a live temporary head")

    def admit_commit(self, data):
        payload = verify(data, "commit", self.resolver)
        commit_id = hashlib.sha256(data).digest()
        if self.key_authority:
            self.check_key_authority(payload)
        for parent in payload["parents"]:
            if parent not in self.commits:
                admission_error("parent_unknown")
            if self.commits[parent]["lineage_id"] != payload["lineage_id"]:
                admission_error("lineage_mismatch", "cross-lineage parent")
        lineage = self.lineages.get(payload["lineage_id"]) or lineage_state_t()
        position, branch_head = self.placement(lineage, payload)
        commit_type = payload["commit_type"]
        if commit_type == "genesis":
            base = EmptyState
        else:
            base = self.commits[payload.get("merge_base", payload["parents"][0])]["result_state"]
        if state_hash(base) != payload["base_state_hash"]:
            admission_error("base_state_mismatch")
        if encode(apply_delta(base, payload["delta"])) != encode(payload["result_state"]):
            admission_error("result_state_mismatch")
        for grant in payload["delta"]["add_grants"]:
            if grant["grant_id"] in lineage.seen_grants or grant["grant_id"] in self.grants:
                admission_error("duplicate_identifier", "grant_id reused")
        if payload["authorization"]["request_id"] in self.seen_requests:
            admission_error("duplicate_identifier", "request_id reused")
        if commit_type == "restore":
            self.check_restore(payload)
        self.check_subject(lineage, payload)
        if position == "branch" and commit_type == "revocation":
            # A branch revocation removes only temporary grants added on that branch.
            if not set(payload["delta"]["remove_grant_ids"]) <= lineage.branches[branch_head]:
                admission_error("branch_rule", "branch revocation removes a grant not added on this branch")
        if self.key_authority and self.state == "contained":
            self.check_recovery_commit(lineage, payload, position, base)
        # R07-r2: at most one occupying network grant across the trunk head and every
        # live temporary head after this admission (durable records only).
        if position == "trunk":
            states_after = [payload["result_state"]]
        else:
            heads = [head for head in lineage.branches if head != branch_head]
            states_after = [self.commits[lineage.trunk_head]["result_state"]] + \
                [self.commits[head]["result_state"] for head in heads] + [payload["result_state"]]
        if len(self.occupying_network_grants(states_after)) > 1:
            admission_error("network_grant_conflict", "a second network grant would be live in this lineage")
        # Apply (nothing above mutated state).
        self.lineages[payload["lineage_id"]] = lineage
        if commit_type == "genesis":
            lineage.authoritative_subject = payload["subject"]
        elif commit_type == "identity_transition":
            lineage.authoritative_subject = payload["subject"]
        if position == "trunk":
            lineage.trunk_head = commit_id
            lineage.branches = {}
        else:
            added = set() if branch_head is None else lineage.branches.pop(branch_head)
            added |= {grant["grant_id"] for grant in payload["delta"]["add_grants"]}
            added -= set(payload["delta"]["remove_grant_ids"])
            lineage.branches[commit_id] = added
        for grant in payload["delta"]["add_grants"]:
            lineage.seen_grants.add(grant["grant_id"])
            self.grants[grant["grant_id"]] = grant
            self.grant_lineage[grant["grant_id"]] = payload["lineage_id"]
        self.seen_requests.add(payload["authorization"]["request_id"])
        if self.key_authority and self.state == "contained":
            lineage.recovered = True
        self.commits[commit_id] = payload
        return payload

    def check_restore(self, payload):
        """Wire format 6.7 (R12): reference is the parent or one of its ancestors; only
        historical grants with a persistent lifetime can be matched."""
        reference = self.commits.get(payload["restore_reference"])
        if reference is None or reference["lineage_id"] != payload["lineage_id"]:
            admission_error("restore_rule", "reference not in lineage")
        parent = payload["parents"][0]
        if payload["restore_reference"] != parent and payload["restore_reference"] not in self.ancestors(parent):
            admission_error("restore_rule", "reference is not an ancestor of the parent")
        historical = [old for old in reference["result_state"]["grants"] if old["lifetime"]["kind"] == "persistent"]
        for grant in payload["delta"]["add_grants"]:
            matches = [old for old in historical
                       if (encode(old["resource"]), encode(old["delegation"]), old["contract_type"])
                       == (encode(grant["resource"]), encode(grant["delegation"]), grant["contract_type"])]
            if not matches:
                admission_error("restore_rule", "restored grant matches no persistent grant of the referenced state")
            if not any("expiration" not in old["lifetime"]
                       or ("expiration" in grant["lifetime"] and parse_time(grant["lifetime"]["expiration"]) <= parse_time(old["lifetime"]["expiration"]))
                       for old in matches):
                admission_error("restore_rule", "restored grant outlives the referenced grant")

    def check_subject(self, lineage, payload):
        # R06: the subject changes only through identity_transition; every other commit
        # must restate the lineage's authoritative subject exactly.
        commit_type = payload["commit_type"]
        if commit_type == "genesis":
            return
        authoritative = lineage.authoritative_subject
        if commit_type == "identity_transition":
            unchanged = dict(payload["subject"], package_identity=None, executable_sha256=None)
            previous = dict(authoritative, package_identity=None, executable_sha256=None)
            if encode({k: v for k, v in unchanged.items() if v is not None}) != encode({k: v for k, v in previous.items() if v is not None}) \
                    or encode(payload["delta"]["identity_change"]["from"]) != encode(subject_identity(authoritative)):
                admission_error("subject_binding_mismatch", "identity transition does not start at the authoritative subject")
        elif encode(payload["subject"]) != encode(authoritative):
            admission_error("subject_binding_mismatch", "subject differs from the lineage's authoritative subject")

    def check_recovery_commit(self, lineage, payload, position, base):
        """ADR 0006 section 10 (R13): the only commits admitted while contained."""
        recovery = self.recovery
        if recovery is None or "persistent_key_id" not in recovery:
            admission_error("recovery_rule", "no open recovery ceremony with named keys")
        if payload["commit_type"] not in RecoveryCommitTypes or position != "trunk":
            admission_error("recovery_rule", "only trunk restore or revocation commits during recovery")
        if payload["authorization"]["session_id"] != recovery["recovery_id"]:
            admission_error("recovery_rule", "authorization session is not the open recovery ceremony")
        if lineage.containment_head is None or lineage.recovered:
            admission_error("recovery_rule", "lineage not contained or already re-authorized in this ceremony")
        if payload["commit_type"] == "restore" and payload["restore_reference"] != lineage.containment_head:
            admission_error("recovery_rule", "recovery restore must reference the contained trunk head")
        if sorted(payload["delta"]["remove_grant_ids"]) != sorted(grant["grant_id"] for grant in base["grants"]):
            admission_error("recovery_rule", "recovery commit must remove every suspended grant of the lineage")

    # ------------------------------------------------------------ registry records

    def contain(self, reason):
        for lineage in self.lineages.values():
            for state in self.live_states(lineage):
                for grant in state["grants"]:
                    self.ended.setdefault(grant["grant_id"], reason)
            lineage.branches = {}
            lineage.containment_head = lineage.trunk_head
            lineage.recovered = False
        self.recovery = None

    def admit_record(self, record):
        kind = record.get("record")
        if kind not in RecordKinds:
            record_error("lifecycle_rule", "unknown record kind")
        if kind == "expired":
            grant = self.grants.get(record["grant_id"])
            if grant is None or "expiration" not in grant["lifetime"] \
                    or parse_time(record["time"]) < parse_time(grant["lifetime"]["expiration"]):
                record_error("lifecycle_rule", "expired record before the grant's expiration")
            self.ended.setdefault(record["grant_id"], "expired")
        elif kind == "session_ended":
            bound = [grant_id for grant_id, grant in self.grants.items() if grant["lifetime"].get("session_id") == record["session_id"]]
            if not bound:
                record_error("lifecycle_rule", "no grant is bound to this session")
            for grant_id in bound:
                self.ended.setdefault(grant_id, "session_ended")
        elif kind == "restriction":
            if not record["grant_ids"] or any(grant_id not in self.grants for grant_id in record["grant_ids"]):
                record_error("lifecycle_rule", "restriction of an unknown grant")
            for grant_id in record["grant_ids"]:
                self.ended.setdefault(grant_id, "restriction")
        elif kind == "containment":
            if self.state in ("deleted", "quarantined"):
                record_error("recovery_rule", "containment of a deleted or quarantined user")
            self.contain("suspended")
            self.state = "contained"
        elif kind == "quarantine":
            if self.state == "deleted":
                record_error("recovery_rule", "quarantine of a deleted user")
            self.contain("quarantined")
            self.state = "quarantined"
        elif kind == "admin_recover":
            if self.state != "quarantined" or record.get("role") != "admin":
                record_error("recovery_rule", "admin recover_user applies only to a quarantined user, by admin")
            self.state = "contained"
        elif kind == "recovery_opened":
            epochs = [key.key_epoch for key in self.keys.values()] + list(self.recovery_epochs)
            if self.state != "contained" or self.recovery is not None:
                record_error("recovery_rule", "ceremony opens only for a contained user without an open ceremony")
            if record["mode"] not in ("c1", "c2") or record["role"] not in ("trusted_client", "admin") \
                    or (record["mode"] == "c2" and record["role"] != "admin"):
                record_error("recovery_rule", "ceremony mode/role")
            if record["key_epoch"] <= max(epochs):
                record_error("recovery_rule", "recovery key epoch must exceed every earlier key epoch")
            self.recovery = {"recovery_id": record["recovery_id"], "key_epoch": record["key_epoch"]}
            self.recovery_epochs.add(record["key_epoch"])
        elif kind == "recovery_keys":
            recovery = self.recovery
            if recovery is None or recovery["recovery_id"] != record["recovery_id"] or "persistent_key_id" in recovery:
                record_error("recovery_rule", "no open ceremony awaiting its keys")
            for field, authority in (("persistent_key_id", "persistent"), ("temporary_key_id", "temporary")):
                key = self.keys.get(record[field])
                if key is None or key.authority_class != authority or key.key_epoch != recovery["key_epoch"] or key.invalidated:
                    record_error("recovery_rule", "named recovery key not certified for this ceremony")
            for key_id, key in self.keys.items():
                if key.authority_class != "identity" and key_id not in (record["persistent_key_id"], record["temporary_key_id"]):
                    key.retired = True
            self.min_key_epoch = recovery["key_epoch"]
            recovery["persistent_key_id"] = record["persistent_key_id"]
            recovery["temporary_key_id"] = record["temporary_key_id"]
        elif kind == "recovery_closed":
            recovery = self.recovery
            if self.state != "contained" or recovery is None or recovery["recovery_id"] != record["recovery_id"] \
                    or "persistent_key_id" not in recovery:
                record_error("recovery_rule", "close of a ceremony that is not open with named keys")
            self.recovery = None
            self.state = "normal"
        else:  # deletion_pending
            for grant_id in self.grants:
                self.ended.setdefault(grant_id, "deleted")
            self.state = "deleted"
            self.recovery = None


def decode_record(record):
    decoded = {}
    for key, value in record.items():
        if key in RecordByteFields:
            decoded[key] = [bytes.fromhex(item) for item in value] if isinstance(value, list) else bytes.fromhex(value)
        else:
            decoded[key] = value
    return decoded


def encode_record(record):
    return {key: ([item.hex() for item in value] if isinstance(value, list) else value.hex()) if key in RecordByteFields else value
            for key, value in record.items()}


def admit_chain(envelopes, resolver):
    """Admits commits in order (no identity records); returns {commit_id: payload}."""
    journal = journal_t(resolver)
    for data in envelopes:
        journal.admit_commit(data)
    return journal.commits


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
    # Revision 3 (R13): keys certified inside the user 1001 recovery ceremony vectors.
    "user_1001_recovery_persistent", "user_1001_recovery_temporary", "user_1001_recovery_abandoned",
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
    operations = sorted(operations, key=encode)  # deterministic (encoded-byte) order
    delegation = {"permitted": delegable, "max_depth": 1 if delegable else 0, "max_operations": operations[:1] if delegable else []}
    return {
        "grant_id": synthetic_id("grant " + label),
        "contract_type": "resource_access",
        "lifetime": lifetime,
        "resource": {"category": category, "resource_id": resource_id, "resource_owner_uid": owner_uid, "operations": operations},
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
    clipboard = make_grant("clipboard", "clipboard", "session_clipboard", ["read", "write"], start=Times[1],
                           expiration="2027-10-12T12:05:00Z")
    network_direct = make_grant("network_direct", "network", "network_mode", ["direct"], start=Times[1])
    microphone_session = make_grant("microphone_session", "audio_input", "default_source", ["capture"], kind="session",
                                    start=Times[2], session=synthetic_id("desktop session 1001"))
    screenshot_once = make_grant("screenshot_once", "screen_capture", "focused_output", ["capture_once"], kind="one_shot", start=Times[3])
    microphone_persistent = make_grant("microphone_persistent", "audio_input", "default_source", ["capture"], start=Times[4])
    clipboard_restored = make_grant("clipboard_restored", "clipboard", "session_clipboard", ["read", "write"], start=Times[6],
                                    expiration="2027-10-12T12:05:00Z")

    chain = []
    genesis = kate.payload("genesis", "genesis", persistent, [], EmptyState,
                           {"add_grants": [documents], "remove_grant_ids": []}, Times[0], "Allow Kate to read Documents/notes")
    genesis_data, genesis_id = sign_commit(persistent, genesis)
    chain.append(("commit_genesis", genesis_data))
    revision = kate.payload("revision", "revision", persistent, [genesis_id], genesis["result_state"],
                            {"add_grants": sorted([clipboard, network_direct], key=lambda grant: grant["grant_id"]), "remove_grant_ids": []},
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
    temporary_once_data, temporary_once_id = sign_commit(temporary, temporary_once)
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
    previous_identity = subject_identity(restore["subject"])
    kate.executable = hashlib.sha256(b"synthetic executable org.kde.kate updated").digest()
    transition = kate.payload("identity_transition", "identity_transition", persistent, [restore_id], restore["result_state"],
                              {"add_grants": [], "remove_grant_ids": [],
                               "identity_change": {"from": previous_identity,
                                                   "to": {"package_identity": kate.package_identity, "executable_sha256": kate.executable}}},
                              Times[7],
                              "Kate was updated: bind the existing grants to the new executable")
    transition_data, transition_id = sign_commit(persistent, transition)
    chain.append(("commit_identity_transition", transition_data))

    firefox = lineage_builder_t(keys, 1002, "user_1002_identity", "org.mozilla.firefox_esr", "deb:firefox-esr:amd64")
    firefox_genesis = firefox.payload("firefox_genesis", "genesis", keys["user_1002_persistent"], [], EmptyState,
                                      {"add_grants": [make_grant("network_tor_1002", "network", "network_mode", ["tor"], owner_uid=1002)],
                                       "remove_grant_ids": []},
                                      Times[0], "Change network mode from Offline to Tor")
    firefox_data, _ = sign_commit(keys["user_1002_persistent"], firefox_genesis)

    boundary, boundary_over, limits = build_boundary_vectors(keys)
    invalid = build_invalid_vectors(keys, kate, genesis, genesis_data, revision, revision_id, merge, persistent, temporary, transition)
    invalid.append({"name": "boundary_one_byte_over", "kind": "commit", "envelope": boundary_over, "layer": "envelope_decode",
                    "error": "input_too_large", "description": "maximum-size commit with one more message byte (61,441 bytes)"})
    ids = {"genesis": genesis_id, "revision": revision_id, "temporary_session": temporary_session_id, "temporary_one_shot": temporary_once_id,
           "merge": merge_id, "revocation": revocation_id, "restore": restore_id, "identity_transition": transition_id}
    payloads = {"temporary_one_shot": temporary_once, "identity_transition": transition}
    admission_invalid = build_admission_vectors(keys, kate, chain, genesis, genesis_id, revision, revision_id, revocation, revocation_id, persistent,
                                                restore, restore_id, temporary, temporary_session, merge, transition, ids, payloads)
    scenarios = build_journal_scenarios(keys, kate, ids, payloads)
    extra_valid = [("commit_user_1002_genesis", firefox_data), ("commit_boundary_max_size", boundary)]
    return keys, resolver_keys, identity, chain, extra_valid, invalid, admission_invalid, limits, scenarios


def build_boundary_vectors(keys):
    """R08: a valid commit of exactly MaxEnvelopeBytes, the same plus one byte, and the
    worst-case IPC/journal wrapper sizes for the maximum object."""
    signer = keys["user_1001_persistent"]
    builder = lineage_builder_t(keys, 1001, "user_1001_identity", "org.example.boundary", "deb:boundary:amd64")

    def payload_for(grant_count, last_length, message_length):
        grants = sorted((make_grant("boundary %d" % index, "filesystem", "r" * 512, ["read"]) for index in range(grant_count)),
                        key=lambda grant: grant["grant_id"])
        grants[-1]["resource"]["resource_id"] = "r" * last_length
        return builder.payload("boundary", "genesis", signer, [], EmptyState, {"add_grants": grants, "remove_grant_ids": []},
                               Times[0], "m" * message_length)

    def size(payload):
        return len(build_envelope("commit", signer, encode(payload), signature=b"\x00" * 64))

    for grant_count in range(1, 65):
        if size(payload_for(grant_count, 512, 1024)) < MaxEnvelopeBytes:
            continue
        for last_length in range(512, 0, -1):
            probe = size(payload_for(grant_count, last_length, 300))
            message_length = MaxEnvelopeBytes - probe + 300
            if 256 <= message_length <= 1023:
                exact = payload_for(grant_count, last_length, message_length)
                over = payload_for(grant_count, last_length, message_length + 1)
                if size(exact) == MaxEnvelopeBytes and size(over) == MaxEnvelopeBytes + 1:
                    exact_data, _ = sign_commit(signer, exact)
                    over_data, _ = sign_commit(signer, over)
                    ipc = encode({"op": "submit_signed", "request_id": b"\x00" * 16,
                                  "args": {"proposal_id": b"\x00" * 16, "envelope": exact_data}})
                    record = encode({"kind": "commit_admitted", "seq": 2**64 - 1, "time": Times[0], "commit_id": b"\x00" * 32,
                                     "lineage_id": b"\x00" * 32, "branch": "temporary", "envelope": exact_data})
                    limits = {
                        "max_envelope_bytes": MaxEnvelopeBytes,
                        "max_wrapper_bytes": MaxWrapperBytes,
                        "max_wrapper_metadata_bytes": MaxWrapperMetadataBytes,
                        "max_envelope_submit_signed_message_bytes": len(ipc),
                        "max_envelope_commit_admitted_record_bytes": len(record),
                    }
                    return exact_data, over_data, limits
        break
    raise SystemExit("could not construct the boundary vector")


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


def network_variant(revision, transform, extra_tor=False):
    """Revision copy whose network grant is transformed in both delta and result."""
    payload = mutate(revision, ["message"], revision["message"])
    for grants in (payload["delta"]["add_grants"], payload["result_state"]["grants"]):
        for grant in grants:
            if grant["resource"]["category"] == "network" and transform is not None:
                transform(grant)
        if extra_tor:
            grants.append(make_grant("network_extra_tor", "network", "network_mode", ["tor"], start=Times[1]))
            grants.sort(key=lambda grant: grant["grant_id"])
    return payload


def one_shot_network(genesis, parent_id):
    payload = mutate(genesis, ["commit_type"], "temporary_grant")
    payload["authority_class"] = "temporary"
    payload["key_id"] = key_t("user_1001_temporary").key_id
    payload["parents"] = [parent_id]
    once = make_grant("network_once", "network", "network_mode", ["direct"], kind="one_shot", start=Times[3])
    payload["delta"] = {"add_grants": [once], "remove_grant_ids": []}
    payload["result_state"] = apply_delta(genesis["result_state"], payload["delta"])
    return payload


def build_invalid_vectors(keys, kate, genesis, genesis_data, revision, revision_id, merge, persistent, temporary, transition):
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
    add("oversized_envelope", encode(tag_t(18, [protected, {}, b"\x00" * 61500, signature])), "envelope_decode", "input_too_large", "envelope larger than 61440 bytes")
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
        ("revocation_with_add", mutate(revision, ["commit_type"], "revocation"), persistent, "commit_type_rule", "revocation that adds a grant"),
        ("merge_without_merge_base", mutate(merge, ["merge_base"], delete=True), persistent, "commit_type_rule", "merge without merge_base"),
        ("restore_reference_on_revision", mutate(revision, ["restore_reference"], revision_id), persistent, "commit_type_rule", "restore_reference on a revision"),
        ("empty_revision", mutate(mutate(revision, ["delta", "add_grants"], []), ["result_state"], genesis["result_state"]), persistent, "commit_type_rule", "revision without any change"),
        ("timed_without_expiration", None, persistent, "lifetime_rule", "timed grant without expiration"),
        ("expiration_before_start", mutate(mutate(genesis, ["delta", "add_grants", 0, "lifetime", "expiration"], "2026-10-12T11:00:00Z"), ["result_state", "grants", 0, "lifetime", "expiration"], "2026-10-12T11:00:00Z"), persistent, "lifetime_rule", "expiration before effective start"),
        ("delegation_scope_not_permitted", mutate(mutate(genesis, ["delta", "add_grants", 0, "delegation", "permitted"], False), ["result_state", "grants", 0, "delegation", "permitted"], False), persistent, "delegation_rule", "non-delegable grant with delegation depth"),
        ("delegation_exceeds_operations", mutate(mutate(genesis, ["delta", "add_grants", 0, "delegation", "max_operations"], ["write"]), ["result_state", "grants", 0, "delegation", "max_operations"], ["write"]), persistent, "delegation_rule", "delegable operations not in the grant"),
        ("network_two_grants", network_variant(revision, None, extra_tor=True), persistent, "network_grant_rule", "two network grants in one state"),
        ("network_both_modes", network_variant(revision, lambda grant: grant["resource"].update(operations=sorted(["direct", "tor"], key=encode))), persistent, "network_grant_rule", "network grant naming both direct and tor"),
        ("network_delegable", network_variant(revision, lambda grant: grant["delegation"].update(permitted=True, max_depth=1, max_operations=["direct"])), persistent, "network_grant_rule", "delegable network grant"),
        ("network_wrong_resource", network_variant(revision, lambda grant: grant["resource"].update(resource_id="network")), persistent, "network_grant_rule", "network grant with a resource id other than network_mode"),
        ("network_foreign_owner", network_variant(revision, lambda grant: grant["resource"].update(resource_owner_uid=1002)), persistent, "network_grant_rule", "network grant owned by another user"),
        ("network_one_shot", one_shot_network(genesis, revision_id), temporary, "network_grant_rule", "one-shot network grant on a temporary branch"),
        ("identity_change_on_revision", mutate(revision, ["delta", "identity_change"], transition["delta"]["identity_change"]), persistent, "commit_type_rule", "identity_change on an ordinary revision"),
        ("identity_transition_without_change", mutate(transition, ["delta", "identity_change"], delete=True), persistent, "commit_type_rule", "identity_transition without identity_change"),
        ("identity_change_not_subject", mutate(transition, ["delta", "identity_change", "to", "executable_sha256"], b"\x09" * 32), persistent, "identity_transition_rule", "identity_change.to differs from the signed subject"),
        ("identity_change_noop", mutate(transition, ["delta", "identity_change", "from"], transition["delta"]["identity_change"]["to"]), persistent, "identity_transition_rule", "identity_change from equals to"),
        ("added_grant_missing_from_result", mutate(genesis, ["result_state", "grants"], []), persistent, "result_state_mismatch", "added grant absent from result state"),
        ("genesis_base_not_empty", mutate(genesis, ["base_state_hash"], b"\x00" * 32), persistent, "base_state_mismatch", "genesis base is not the empty state"),
        ("identity_transition_with_add", mutate(revision, ["commit_type"], "identity_transition"), persistent, "commit_type_rule", "identity transition that also adds a grant"),
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
                            restore, restore_id, temporary, temporary_session, merge, transition, ids, payloads):
    """Lineage-context failures: each entry is a prefix of the valid chain plus one bad commit."""
    vectors = []

    def add(name, prefix_length, payload, code, description, signer=persistent):
        vectors.append({"name": name, "prefix": prefix_length, "envelope": resign(signer, payload), "layer": "admission", "error": code, "description": description})

    def successor(template, parents, base_state, delta, label, commit_type=None, extra=None):
        payload = mutate(template, ["parents"], sorted(parents))
        if commit_type:
            payload["commit_type"] = commit_type
        for key in ("merge_base", "restore_reference"):
            payload.pop(key, None)
        payload.update(extra or {})
        payload["delta"] = delta
        payload["base_state_hash"] = state_hash(base_state)
        payload["result_state"] = apply_delta(base_state, delta)
        payload["authorization"]["request_id"] = synthetic_id("request " + label)
        return payload

    def substituted(payload):
        payload = mutate(payload, ["subject", "package_identity"], "deb:attacker:amd64")
        payload["subject"]["executable_sha256"] = b"\x42" * 32
        return payload

    clipboard = next(grant for grant in revision["delta"]["add_grants"] if grant["resource"]["category"] == "clipboard")
    network_direct = next(grant for grant in revision["delta"]["add_grants"] if grant["resource"]["category"] == "network")

    add("stale_base_state", 1, mutate(revision, ["base_state_hash"], state_hash(EmptyState)), "base_state_mismatch",
        "revision claims the empty state as its base")
    phantom = make_grant("phantom", "camera", "front_camera", ["capture"], start=Times[1])
    wrong_result = mutate(revision, ["result_state", "grants"], sorted(revision["result_state"]["grants"] + [phantom], key=lambda grant: grant["grant_id"]))
    add("result_not_base_plus_delta", 1, wrong_result, "result_state_mismatch", "result state contains a camera grant the delta does not add")
    add("unknown_parent", 1, mutate(revision, ["parents"], [b"\x07" * 32]), "parent_unknown", "parent not in the lineage")
    add("revoked_grant_id_re_added", 6,
        successor(revocation, [revocation_id], revocation["result_state"], {"add_grants": [clipboard], "remove_grant_ids": []},
                  "replayed clipboard", commit_type="revision"),
        "duplicate_identifier", "revoked clipboard grant re-added with its old grant_id")
    replayed_request = mutate(revision, ["message"], "Same request identifier as genesis")
    replayed_request["authorization"]["request_id"] = genesis["authorization"]["request_id"]
    add("request_id_replayed", 1, replayed_request, "duplicate_identifier", "authorization request_id reused")
    clipboard_wide = make_grant("clipboard_wide", "clipboard", "session_clipboard", ["read", "write", "write_primary"], start=Times[6],
                                expiration="2027-10-12T12:05:00Z")
    add("restore_expands_history", 6,
        successor(revocation, [revocation_id], revocation["result_state"], {"add_grants": [clipboard_wide], "remove_grant_ids": []},
                  "expanded restore", commit_type="restore", extra={"restore_reference": revision_id}),
        "restore_rule", "restore adds an operation absent from the referenced state")
    clipboard_forever = make_grant("clipboard_forever", "clipboard", "session_clipboard", ["read", "write"], start=Times[6])
    add("restore_outlives_history", 6,
        successor(revocation, [revocation_id], revocation["result_state"], {"add_grants": [clipboard_forever], "remove_grant_ids": []},
                  "restore without expiration", commit_type="restore", extra={"restore_reference": revision_id}),
        "restore_rule", "restored clipboard grant drops the historical expiration")
    tor = make_grant("restore_tor", "network", "network_mode", ["tor"], start=Times[6])
    add("restore_network_not_in_snapshot", 6,
        successor(revocation, [revocation_id], revocation["result_state"],
                  {"add_grants": [tor], "remove_grant_ids": [network_direct["grant_id"]]},
                  "restore tor", commit_type="restore", extra={"restore_reference": genesis_id}),
        "restore_rule", "restore referencing the Offline genesis introduces a Tor network grant")
    add("revision_substitutes_identity", 1, substituted(revision), "subject_binding_mismatch",
        "ordinary revision restates the lineage under another package and executable")
    add("temporary_grant_substitutes_identity", 2, substituted(temporary_session), "subject_binding_mismatch",
        "temporary grant under another package and executable", signer=temporary)
    add("merge_substitutes_identity", 4, substituted(merge), "subject_binding_mismatch",
        "merge under another package and executable")
    add("restore_substitutes_identity", 6, substituted(restore), "subject_binding_mismatch",
        "restore under another package and executable")
    add("identity_transition_wrong_start", 7,
        mutate(transition, ["delta", "identity_change", "from", "executable_sha256"], b"\x05" * 32),
        "subject_binding_mismatch", "identity transition whose from does not match the authoritative subject")
    add("identity_transition_changes_runtime_scope", 7, mutate(transition, ["subject", "runtime_scope"], "flatpak"),
        "subject_binding_mismatch", "identity transition that also changes a non-identity subject field")
    # Revision 3, R12: restore matches only persistent historical grants of an ancestor.
    microphone_session = temporary_session["delta"]["add_grants"][0]
    session_as_persistent = mutate(microphone_session, ["grant_id"], synthetic_id("grant session restored as persistent"))
    session_as_persistent["lifetime"] = {"kind": "persistent", "effective_start": Times[6]}
    add("restore_session_grant_as_persistent", 6,
        successor(revocation, [revocation_id], revocation["result_state"], {"add_grants": [session_as_persistent], "remove_grant_ids": []},
                  "restore session as persistent", commit_type="restore", extra={"restore_reference": ids["temporary_session"]}),
        "restore_rule", "the reviewer's case: an ancestral session grant restated as persistent (reference is an ancestor through the merge)")
    clipboard_again = make_grant("clipboard_not_ancestor", "clipboard", "session_clipboard", ["read", "write"], start=Times[6],
                                 expiration="2027-10-12T12:05:00Z")
    add("restore_reference_not_ancestor", 6,
        successor(revocation, [revocation_id], revocation["result_state"], {"add_grants": [clipboard_again], "remove_grant_ids": []},
                  "restore from non-ancestor", commit_type="restore", extra={"restore_reference": ids["temporary_one_shot"]}),
        "restore_rule", "restore referencing the one-shot branch commit, which is in the lineage but not an ancestor of the parent")
    # Revision 3, R07-r2 / wire 6.11: placement relative to the trunk head and live temporary heads.
    camera = make_grant("camera_stale_parent", "camera", "front_camera", ["capture"], start=Times[2])
    add("revision_parent_not_trunk_head", 2,
        successor(revision, [genesis_id], genesis["result_state"], {"add_grants": [camera], "remove_grant_ids": []}, "stale revision"),
        "stale_head", "revision parented on the superseded genesis instead of the current trunk head")
    late = make_grant("late_temporary", "camera", "front_camera", ["capture"], kind="timed", start=Times[5], expiration=Times[6])
    add("temporary_grant_on_ended_branch", 5,
        successor(payloads["temporary_one_shot"], [ids["temporary_one_shot"]], payloads["temporary_one_shot"]["result_state"],
                  {"add_grants": [late], "remove_grant_ids": []}, "extend ended branch"),
        "stale_head", "temporary grant extending the one-shot branch after the merge ended it", signer=temporary)
    return vectors


def build_journal_scenarios(keys, kate, ids, payloads):
    """Revision 3: signed multi-step journal vectors (R07-r2 network slot, R12 restore
    lifetime kinds, R13 recovery ceremony). Each scenario runs in a fresh journal_t;
    every step states its expected outcome and, optionally, the standing of grants."""
    persistent = keys["user_1001_persistent"]
    temporary = keys["user_1001_temporary"]
    identity = keys["user_1001_identity"]
    scenarios = []

    def fails(layer, code):
        return {"layer": layer, "error": code}

    def with_check(step, expect, check, note):
        step["expect"] = expect
        if check:
            step["check"] = {standing: [grant_id.hex() for grant_id in grant_ids] for standing, grant_ids in check.items()}
        if note:
            step["note"] = note
        return step

    def valid(name, expect="admitted", check=None, note=""):
        return with_check({"valid": name}, expect, check, note)

    def commit(data, expect="admitted", check=None, note=""):
        return with_check({"commit": data.hex()}, expect, check, note)

    def identity_step(data, kind, expect="admitted", check=None, note=""):
        return with_check({"identity": data.hex(), "kind": kind}, expect, check, note)

    def record(value, expect="admitted", check=None, note=""):
        return with_check({"record": encode_record(value)}, expect, check, note)

    def scenario(name, finding, description, steps, key_authority=False):
        scenarios.append({"name": name, "finding": finding, "description": description,
                          "key_authority": key_authority, "steps": steps})

    def gid(grant):
        return grant["grant_id"]

    # ------------------------------------------------------------ R07-r2: network slot
    browser = lineage_builder_t(keys, 1001, "user_1001_identity", "org.example.browser", "deb:browser:amd64")
    downloads = make_grant("browser downloads", "filesystem", "home:Downloads", ["read", "write"])
    browser_genesis = browser.payload("browser genesis", "genesis", persistent, [], EmptyState,
                                      {"add_grants": [downloads], "remove_grant_ids": []}, Times[0], "Allow the browser to use Downloads")
    browser_genesis_data, browser_genesis_id = sign_commit(persistent, browser_genesis)
    browser_state = browser_genesis["result_state"]
    session_one = synthetic_id("browser desktop session 1")
    direct_timed = make_grant("browser direct timed", "network", "network_mode", ["direct"], kind="timed", start=Times[1], expiration=Times[3])
    tor_timed = make_grant("browser tor timed", "network", "network_mode", ["tor"], kind="timed", start=Times[1], expiration=Times[4])
    direct_short = make_grant("browser direct short", "network", "network_mode", ["direct"], kind="timed", start=Times[1], expiration=Times[2])
    tor_later = make_grant("browser tor later", "network", "network_mode", ["tor"], kind="timed", start=Times[3], expiration=Times[4])
    direct_session = make_grant("browser direct session", "network", "network_mode", ["direct"], kind="session", start=Times[1], session=session_one)
    camera_timed = make_grant("browser camera timed", "camera", "front_camera", ["capture"], kind="timed", start=Times[1], expiration=Times[3])
    clipboard_timed = make_grant("browser clipboard timed", "clipboard", "session_clipboard", ["read"], kind="timed", start=Times[1], expiration=Times[3])
    tor_extra = make_grant("browser tor extra", "network", "network_mode", ["tor"], kind="timed", start=Times[1], expiration=Times[3])
    tor_persistent = make_grant("browser tor persistent", "network", "network_mode", ["tor"], start=Times[2])
    direct_after_trunk = make_grant("browser direct after trunk", "network", "network_mode", ["direct"], kind="timed", start=Times[3], expiration=Times[4])

    def browser_temporary(label, parent_id, base_state, grants, signer=temporary, time=Times[1]):
        authority = "temporary" if signer is temporary else "persistent"
        payload = browser.payload(label, "temporary_grant", signer, [parent_id], base_state,
                                  {"add_grants": sorted(grants, key=gid), "remove_grant_ids": []}, time,
                                  "Temporary grant: " + label, authority=authority)
        data, commit_id = sign_commit(signer, payload)
        return data, commit_id, payload

    direct_data, direct_id, direct_payload = browser_temporary("browser direct", browser_genesis_id, browser_state, [direct_timed])
    # The reviewer's variant: the second sibling signed by the persistent key.
    tor_data, _, _ = browser_temporary("browser tor", browser_genesis_id, browser_state, [tor_timed], signer=persistent)
    short_data, _, _ = browser_temporary("browser direct short", browser_genesis_id, browser_state, [direct_short])
    later_data, _, _ = browser_temporary("browser tor later", browser_genesis_id, browser_state, [tor_later], time=Times[3])
    session_data, _, _ = browser_temporary("browser direct session", browser_genesis_id, browser_state, [direct_session])
    camera_data, _, _ = browser_temporary("browser camera", browser_genesis_id, browser_state, [camera_timed])
    extension_data, extension_id, extension_payload = browser_temporary("browser clipboard extension", direct_id, direct_payload["result_state"], [clipboard_timed])
    second_network_data, _, _ = browser_temporary("browser second network", extension_id, extension_payload["result_state"], [tor_extra])
    late_data, _, _ = browser_temporary("browser late extension", direct_id, direct_payload["result_state"], [clipboard_timed], time=Times[3])
    start = [commit(browser_genesis_data, check={"current": [gid(downloads)]})]

    scenario("network_sibling_branches_conflict", "R07-r2",
             "Offline trunk; a timed Direct branch is admitted; an overlapping timed Tor sibling rooted at the same current trunk head is rejected",
             start + [commit(direct_data, check={"current": [gid(direct_timed)]}),
                      commit(tor_data, fails("admission", "network_grant_conflict"), check={"current": [gid(direct_timed), gid(downloads)]},
                             note="the reviewer's case: both siblings rooted at the current Offline trunk head")])
    scenario("network_simultaneous_proposals_first_admitted_wins", "R07-r2",
             "Both proposals were prepared against the same trunk head; under the admission lock the first admitted (here Tor) wins and the other is rejected",
             start + [commit(tor_data, check={"current": [gid(tor_timed)]}),
                      commit(direct_data, fails("admission", "network_grant_conflict"), check={"current": [gid(tor_timed)]})])
    scenario("network_sibling_non_overlapping_lifetimes_conflict", "R07-r2",
             "Lifetimes that do not overlap still conflict: the slot is freed only by a durable terminal record, never by clock arithmetic at admission",
             start + [commit(short_data), commit(later_data, fails("admission", "network_grant_conflict"))])
    scenario("network_sibling_after_durable_expiry", "R07-r2",
             "A premature expiry record is rejected; after a durable expired record for the Direct grant a Tor sibling is admitted",
             start + [commit(direct_data),
                      record({"record": "expired", "grant_id": gid(direct_timed), "time": Times[2]}, fails("admission", "lifecycle_rule")),
                      commit(tor_data, fails("admission", "network_grant_conflict")),
                      record({"record": "expired", "grant_id": gid(direct_timed), "time": Times[3]}, check={"ended": [gid(direct_timed)]}),
                      commit(tor_data, check={"current": [gid(tor_timed)], "ended": [gid(direct_timed)]})])
    scenario("network_sibling_after_session_end", "R07-r2",
             "A session-bound Direct grant occupies the slot until its session_ended record is durable",
             start + [commit(session_data),
                      commit(tor_data, fails("admission", "network_grant_conflict")),
                      record({"record": "session_ended", "session_id": synthetic_id("browser desktop session 2")}, fails("admission", "lifecycle_rule"),
                             note="ending an unrelated session frees nothing"),
                      record({"record": "session_ended", "session_id": session_one}, check={"ended": [gid(direct_session)]}),
                      commit(tor_data, check={"current": [gid(tor_timed)]})])
    scenario("network_sibling_after_restriction", "R07-r2",
             "Restricting another grant frees nothing; a durable restriction (revocation intent) of the Direct grant frees the slot",
             start + [commit(direct_data),
                      record({"record": "restriction", "grant_ids": [gid(downloads)]}, check={"ended": [gid(downloads)]}),
                      commit(tor_data, fails("admission", "network_grant_conflict")),
                      record({"record": "restriction", "grant_ids": [gid(direct_timed)]}, check={"ended": [gid(direct_timed)]}),
                      commit(tor_data, check={"current": [gid(tor_timed)]})])
    bad_branch_revocation = browser.payload("browser bad branch revocation", "revocation", persistent, [direct_id], direct_payload["result_state"],
                                            {"add_grants": [], "remove_grant_ids": [gid(downloads)]}, Times[2], "Revoke Downloads on the temporary branch")
    branch_revocation = browser.payload("browser branch revocation", "revocation", persistent, [direct_id], direct_payload["result_state"],
                                        {"add_grants": [], "remove_grant_ids": [gid(direct_timed)]}, Times[2], "Revoke the temporary Direct network grant")
    scenario("network_branch_revocation_frees_slot", "R07-r2",
             "A signed branch revocation may remove only grants added on its branch; removing the Direct grant frees the slot",
             start + [commit(direct_data),
                      commit(sign_commit(persistent, bad_branch_revocation)[0], fails("admission", "branch_rule")),
                      commit(sign_commit(persistent, branch_revocation)[0], check={"inactive": [gid(direct_timed)], "current": [gid(downloads)]}),
                      commit(tor_data, check={"current": [gid(tor_timed)]})])
    scenario("network_branch_extension_and_non_network_sibling", "R07-r2",
             "Positive controls: a non-network sibling and an extension of the Direct branch are admitted; a second network grant on that branch is not",
             start + [commit(direct_data), commit(camera_data), commit(extension_data),
                      commit(second_network_data, fails("intrinsic", "network_grant_rule")),
                      record({"record": "expired", "grant_id": gid(direct_timed), "time": Times[3]},
                             check={"current": [gid(camera_timed), gid(clipboard_timed)], "ended": [gid(direct_timed)]})])
    trunk_tor = browser.payload("browser trunk tor", "revision", persistent, [browser_genesis_id], browser_state,
                                {"add_grants": [tor_persistent], "remove_grant_ids": []}, Times[2], "Change network mode from Offline to Tor")
    trunk_tor_data, trunk_tor_id = sign_commit(persistent, trunk_tor)
    direct_on_tor_trunk, _, _ = browser_temporary("browser direct on tor trunk", trunk_tor_id, trunk_tor["result_state"], [direct_after_trunk], time=Times[3])
    scenario("network_trunk_commit_ends_branches", "R07-r2",
             "A trunk revision adding a persistent Tor grant ends every live temporary branch; the old branch head is stale and a temporary Direct grant over the Tor trunk is rejected",
             start + [commit(direct_data),
                      commit(trunk_tor_data, check={"current": [gid(tor_persistent)], "inactive": [gid(direct_timed)]}),
                      commit(late_data, fails("admission", "stale_head")),
                      commit(direct_on_tor_trunk, fails("intrinsic", "network_grant_rule"))])

    # ------------------------------------------------------------ R12: restore lifetime kinds
    recorder = lineage_builder_t(keys, 1001, "user_1001_identity", "org.example.recorder", "deb:recorder:amd64")
    recordings = make_grant("recorder recordings", "filesystem", "home:Recordings", ["read", "write"])
    recorder_clipboard = make_grant("recorder clipboard", "clipboard", "session_clipboard", ["read"], expiration="2027-01-01T00:00:00Z")
    recorder_session = synthetic_id("recorder desktop session")
    microphone_session = make_grant("recorder microphone session", "audio_input", "default_source", ["capture"], kind="session", start=Times[1], session=recorder_session)
    camera_window = make_grant("recorder camera timed", "camera", "front_camera", ["capture"], kind="timed", start=Times[1], expiration=Times[5])
    screenshot_once = make_grant("recorder screenshot once", "screen_capture", "focused_output", ["capture_once"], kind="one_shot", start=Times[1])
    recorder_genesis = recorder.payload("recorder genesis", "genesis", persistent, [], EmptyState,
                                        {"add_grants": sorted([recordings, recorder_clipboard], key=gid), "remove_grant_ids": []}, Times[0],
                                        "Allow the recorder to use Recordings and read the clipboard")
    recorder_genesis_data, recorder_genesis_id = sign_commit(persistent, recorder_genesis)
    recorder_state = recorder_genesis["result_state"]
    branch_ids = []
    branch_steps = []
    for label, grant in (("session", microphone_session), ("timed", camera_window), ("one shot", screenshot_once)):
        payload = recorder.payload("recorder " + label, "temporary_grant", temporary, [recorder_genesis_id], recorder_state,
                                   {"add_grants": [grant], "remove_grant_ids": []}, Times[1], "Temporary " + label + " grant", authority="temporary")
        data, commit_id = sign_commit(temporary, payload)
        branch_ids.append(commit_id)
        branch_steps.append(commit(data))
    session_branch_id, timed_branch_id, once_branch_id = branch_ids
    recorder_merge = recorder.payload("recorder merge", "merge", persistent, [recorder_genesis_id] + branch_ids, recorder_state,
                                      {"add_grants": [], "remove_grant_ids": [gid(recorder_clipboard)]}, Times[2],
                                      "Close the temporary branches and remove clipboard access", extra={"merge_base": recorder_genesis_id})
    recorder_merge_data, recorder_merge_id = sign_commit(persistent, recorder_merge)
    merged_state = recorder_merge["result_state"]

    def recorder_restore(label, reference, grant):
        payload = recorder.payload(label, "restore", persistent, [recorder_merge_id], merged_state, {"add_grants": [grant], "remove_grant_ids": []},
                                   Times[6], "Restore: " + label, extra={"restore_reference": reference})
        return sign_commit(persistent, payload)

    restored_microphone = make_grant("recorder microphone as persistent", "audio_input", "default_source", ["capture"], start=Times[6])
    restored_camera = make_grant("recorder camera as persistent", "camera", "front_camera", ["capture"], start=Times[6], expiration=Times[5].replace("12:25", "12:59"))
    restored_screenshot = make_grant("recorder screenshot as persistent", "screen_capture", "focused_output", ["capture_once"], start=Times[6])
    restored_clipboard = make_grant("recorder clipboard restored", "clipboard", "session_clipboard", ["read"], start=Times[6], expiration="2027-01-01T00:00:00Z")
    clipboard_restore_data, clipboard_restore_id = recorder_restore("recorder clipboard via session ancestor", session_branch_id, restored_clipboard)
    fresh_microphone = make_grant("recorder microphone fresh persistent", "audio_input", "default_source", ["capture"], start=Times[7])
    fresh_revision = recorder.payload("recorder fresh microphone", "revision", persistent, [clipboard_restore_id],
                                      apply_delta(merged_state, {"add_grants": [restored_clipboard], "remove_grant_ids": []}),
                                      {"add_grants": [fresh_microphone], "remove_grant_ids": []}, Times[7],
                                      "Make microphone access persistent (new grant, fresh approval)")
    scenario("restore_matches_only_persistent_history", "R12",
             "Session, timed and one-shot grants of actual ancestors are never restored as persistent; a persistent grant of the same ancestor is; making temporary scope persistent is a separate freshly approved revision",
             [commit(recorder_genesis_data)] + branch_steps + [
                 commit(recorder_merge_data, check={"inactive": [gid(microphone_session), gid(camera_window), gid(screenshot_once), gid(recorder_clipboard)]}),
                 commit(recorder_restore("recorder session as persistent", session_branch_id, restored_microphone)[0], fails("admission", "restore_rule"),
                        note="the reviewer's case: ancestral session grant restated as persistent"),
                 commit(recorder_restore("recorder timed as persistent", timed_branch_id, restored_camera)[0], fails("admission", "restore_rule")),
                 commit(recorder_restore("recorder one shot as persistent", once_branch_id, restored_screenshot)[0], fails("admission", "restore_rule")),
                 commit(clipboard_restore_data, check={"current": [gid(restored_clipboard)]},
                        note="positive control: persistent historical grant, reference is a real ancestor through the merge"),
                 commit(sign_commit(persistent, fresh_revision)[0], check={"current": [gid(fresh_microphone)], "inactive": [gid(microphone_session)]},
                        note="fresh-approval route for making temporary scope persistent: a revision adding a new persistent grant")])

    # ------------------------------------------------------------ R13: recovery ceremony
    recovery_persistent = keys["user_1001_recovery_persistent"]
    recovery_temporary = keys["user_1001_recovery_temporary"]
    abandoned = keys["user_1001_recovery_abandoned"]
    ceremony_id = synthetic_id("recovery ceremony 1001 first")

    def certificate(subject, authority, label, epoch=2, supersedes=None):
        payload = {
            "schema_version": 1, "certificate_id": synthetic_id("certificate " + label), "issued": Times[9], "owner_uid": 1001,
            "owner_user_id": identity.key_id, "subject_key_id": subject.key_id, "subject_public_key": subject.public_key,
            "authority_class": authority, "key_epoch": epoch, "not_before": Times[9],
        }
        if authority == "temporary":
            payload["not_after"] = "2026-10-19T12:00:00Z"
        if supersedes is not None:
            payload["supersedes_key_id"] = supersedes.key_id
        return build_envelope("key_certificate", identity, encode(payload))

    recovery_persistent_certificate = certificate(recovery_persistent, "persistent", "recovery persistent", supersedes=persistent)
    recovery_temporary_certificate = certificate(recovery_temporary, "temporary", "recovery temporary", supersedes=temporary)
    abandoned_certificate = certificate(abandoned, "persistent", "recovery abandoned attempt", supersedes=persistent)
    compromised_invalidation = build_envelope("key_invalidation", identity, encode({
        "schema_version": 1, "invalidation_id": synthetic_id("invalidation recovery persistent"), "issued": Times[10],
        "owner_user_id": identity.key_id, "target_key_id": recovery_persistent.key_id, "reason": "compromised"}))

    transition = payloads["identity_transition"]
    transition_id = ids["identity_transition"]
    kate_state = transition["result_state"]
    kate_old_ids = [gid(grant) for grant in kate_state["grants"]]
    kate_screenshot = make_grant("kate screenshot before containment", "screen_capture", "focused_output", ["capture_once"], kind="one_shot", start=Times[8])
    kate_branch = kate.payload("kate branch before containment", "temporary_grant", temporary, [transition_id], kate_state,
                               {"add_grants": [kate_screenshot], "remove_grant_ids": []}, Times[8], "Allow one screenshot of the focused output",
                               authority="temporary")
    editor = lineage_builder_t(keys, 1001, "user_1001_identity", "org.example.editor", "deb:editor:amd64")
    projects = make_grant("editor projects", "filesystem", "home:Projects", ["read", "write"])
    editor_genesis = editor.payload("editor genesis", "genesis", persistent, [], EmptyState, {"add_grants": [projects], "remove_grant_ids": []},
                                    Times[8], "Allow the editor to use Projects")
    editor_genesis_data, editor_genesis_id = sign_commit(persistent, editor_genesis)
    suspended = kate_old_ids + [gid(kate_screenshot), gid(projects)]

    def regrant(grant, label):
        lifetime = grant["lifetime"]
        renewed = make_grant(label, grant["resource"]["category"], grant["resource"]["resource_id"], grant["resource"]["operations"],
                             start=Times[10], expiration=lifetime.get("expiration"), owner_uid=grant["resource"]["resource_owner_uid"],
                             delegable=grant["delegation"]["permitted"])
        renewed["delegation"] = dict(grant["delegation"])
        return renewed

    kate_by_category = {grant["resource"]["category"]: grant for grant in kate_state["grants"]}
    renewed = sorted([regrant(kate_by_category["filesystem"], "kate recovered documents"),
                      regrant(kate_by_category["network"], "kate recovered network"),
                      regrant(kate_by_category["audio_input"], "kate recovered microphone")], key=gid)
    renewed_ids = [gid(grant) for grant in renewed]

    def kate_recovery(label, signer=recovery_persistent, commit_type="restore", adds=None, removes=None, reference=None,
                      session=ceremony_id, parent=transition_id, base=None):
        base = kate_state if base is None else base
        extra = {"restore_reference": reference or transition_id} if commit_type == "restore" else None
        payload = kate.payload(label, commit_type, signer, [parent], base,
                               {"add_grants": renewed if adds is None else adds, "remove_grant_ids": sorted(kate_old_ids if removes is None else removes)},
                               Times[10], "Re-authorize Kate after containment", extra=extra)
        payload["authorization"]["session_id"] = session
        return sign_commit(signer, payload)[0]

    kate_recovered = kate_recovery("kate recovery restore")
    prefix = [valid("user_1001_identity_enrollment"), valid("user_1001_persistent_certificate"), valid("user_1001_temporary_certificate")]
    prefix += [valid(name) for name in ("commit_genesis", "commit_revision", "commit_temporary_session", "commit_temporary_one_shot",
                                        "commit_merge", "commit_revocation", "commit_restore", "commit_identity_transition")]
    prefix += [commit(sign_commit(temporary, kate_branch)[0]), commit(editor_genesis_data, check={"current": [gid(projects)] + kate_old_ids})]
    contained = prefix + [record({"record": "containment"}, check={"ended": suspended},
                                 note="unverifiable unclean run: every grant suspended, the live temporary branch ends")]
    opened = contained + [record({"record": "recovery_opened", "recovery_id": ceremony_id, "key_epoch": 2, "mode": "c1", "role": "trusted_client"})]
    keyed = opened + [identity_step(recovery_persistent_certificate, "key_certificate"),
                      identity_step(recovery_temporary_certificate, "key_certificate"),
                      record({"record": "recovery_keys", "recovery_id": ceremony_id,
                              "persistent_key_id": recovery_persistent.key_id, "temporary_key_id": recovery_temporary.key_id},
                             check={"ended": suspended})]
    editor_fresh = make_grant("editor projects re-authorized", "filesystem", "home:Projects", ["read", "write"], start=Times[11])

    editor_cleanup = editor.payload("editor remove suspended", "revocation", recovery_persistent, [editor_genesis_id], editor_genesis["result_state"],
                                    {"add_grants": [], "remove_grant_ids": [gid(projects)]}, Times[11], "Remove the suspended Projects grant")
    editor_cleanup_data, editor_cleanup_id = sign_commit(recovery_persistent, editor_cleanup)

    def editor_revision(label, signer, grant):
        payload = editor.payload(label, "revision", signer, [editor_cleanup_id], editor_cleanup["result_state"],
                                 {"add_grants": [grant], "remove_grant_ids": []}, Times[11], "Re-authorize Projects access")
        return sign_commit(signer, payload)[0]

    scenario("recovery_c1_complete", "R13",
             "Containment, then the complete C1 ceremony: open, fresh recovery keys at a new epoch, per-lineage re-authorization signed by the recovery key, close; old grants stay denied at every step",
             contained + [
                 commit(kate_recovery("kate restore before ceremony", signer=persistent), fails("key_authority", "contained"),
                        note="bypass: no ceremony open"),
                 identity_step(recovery_persistent_certificate, "key_certificate", fails("key_authority", "contained"),
                               note="certificates only inside an open ceremony"),
                 record({"record": "recovery_opened", "recovery_id": ceremony_id, "key_epoch": 1, "mode": "c1", "role": "trusted_client"},
                        fails("admission", "recovery_rule"), note="recovery epoch must exceed every earlier key epoch"),
                 record({"record": "recovery_opened", "recovery_id": ceremony_id, "key_epoch": 2, "mode": "c1", "role": "trusted_client"},
                        check={"ended": suspended}),
                 commit(kate_recovery("kate restore before keys", signer=persistent), fails("key_authority", "recovery_key_required"),
                        note="the pre-containment key cannot sign recovery commits"),
                 identity_step(recovery_persistent_certificate, "key_certificate"),
                 identity_step(recovery_temporary_certificate, "key_certificate"),
                 record({"record": "recovery_keys", "recovery_id": ceremony_id,
                         "persistent_key_id": recovery_persistent.key_id, "temporary_key_id": recovery_temporary.key_id}),
                 commit(kate_recovered, check={"current": renewed_ids, "ended": suspended},
                        note="per-lineage progress: Kate re-authorized with new grant ids; the editor lineage stays suspended"),
                 commit(kate_recovered, fails("admission", "stale_head"), note="the same recovery commit cannot be admitted twice"),
                 record({"record": "recovery_closed", "recovery_id": ceremony_id}, check={"current": renewed_ids, "ended": suspended}),
                 commit(kate_recovered, fails("admission", "stale_head"), note="replay after close"),
                 commit(editor_cleanup_data, check={"ended": [gid(projects)]}, note="ordinary revocation removing the suspended grant after close"),
                 commit(editor_revision("editor old key", persistent, editor_fresh), fails("key_authority", "key_epoch_stale"),
                        note="pre-recovery keys never sign again"),
                 commit(editor_revision("editor old id", recovery_persistent, projects), fails("admission", "duplicate_identifier"),
                        note="a suspended grant id never reappears"),
                 commit(editor_revision("editor fresh", recovery_persistent, editor_fresh), check={"current": [gid(editor_fresh)], "ended": [gid(projects)]},
                        note="unrecovered lineage re-authorized later by an ordinary freshly approved revision")],
             key_authority=True)
    scenario("recovery_rejects_stale_key", "R13", "A key certified before the recovery epoch cannot sign the recovery restore",
             keyed + [commit(kate_recovery("kate stale key", signer=persistent), fails("key_authority", "key_epoch_stale"), check={"ended": suspended})],
             key_authority=True)
    scenario("recovery_rejects_compromised_key", "R13", "A recovery key invalidated as compromised during the ceremony cannot sign; invalidation is always admitted",
             keyed + [identity_step(compromised_invalidation, "key_invalidation"),
                      commit(kate_recovered, fails("key_authority", "key_invalidated"), check={"ended": suspended})],
             key_authority=True)
    scenario("recovery_rejects_abandoned_key", "R13",
             "A key certified at the recovery epoch by an abandoned attempt (its private object erased) but not named by recovery_keys is retired",
             opened + [identity_step(abandoned_certificate, "key_certificate")] + keyed[len(opened):] +
             [commit(kate_recovery("kate abandoned key", signer=abandoned), fails("key_authority", "key_retired"), check={"ended": suspended})],
             key_authority=True)
    scenario("recovery_rejects_deleted_user", "R13", "A deletion_pending user cannot complete recovery; every key is denied",
             keyed + [record({"record": "deletion_pending"}), commit(kate_recovered, fails("key_authority", "user_deleted"), check={"ended": suspended}),
                      record({"record": "recovery_opened", "recovery_id": ceremony_id, "key_epoch": 3, "mode": "c1", "role": "trusted_client"},
                             fails("admission", "recovery_rule"))],
             key_authority=True)
    renewed_without_microphone = [grant for grant in renewed if grant["resource"]["category"] != "audio_input"]
    screenshot_persistent = make_grant("kate screenshot as persistent", "screen_capture", "focused_output", ["capture_once"], start=Times[10])
    revision_scope = make_grant("kate new camera during recovery", "camera", "front_camera", ["capture"], start=Times[10])
    temporary_scope = make_grant("kate temporary during recovery", "camera", "front_camera", ["capture"], kind="timed", start=Times[10], expiration=Times[11])
    recovery_revision = kate.payload("kate revision during recovery", "revision", recovery_persistent, [transition_id], kate_state,
                                     {"add_grants": [revision_scope], "remove_grant_ids": []}, Times[10], "Add camera access")
    recovery_revision["authorization"]["session_id"] = ceremony_id
    recovery_temporary_grant = kate.payload("kate temporary during recovery", "temporary_grant", recovery_persistent, [transition_id], kate_state,
                                            {"add_grants": [temporary_scope], "remove_grant_ids": []}, Times[10], "Temporary camera access")
    recovery_temporary_grant["authorization"]["session_id"] = ceremony_id
    scenario("recovery_rejects_bypass", "R13",
             "Inside an open ceremony only trunk restore/revocation commits bound to the ceremony, referencing the contained head and removing every suspended grant, are admitted",
             keyed + [
                 commit(kate_recovery("kate wrong session", session=synthetic_id("auth session 1001")), fails("admission", "recovery_rule")),
                 commit(sign_commit(recovery_persistent, recovery_revision)[0], fails("admission", "recovery_rule")),
                 commit(sign_commit(recovery_persistent, recovery_temporary_grant)[0], fails("admission", "recovery_rule")),
                 commit(kate_recovery("kate partial removal", removes=[grant_id for grant_id in kate_old_ids if grant_id != gid(kate_by_category["filesystem"])],
                                      adds=[grant for grant in renewed if grant["resource"]["category"] != "filesystem"]), fails("admission", "recovery_rule"),
                        note="keeps one suspended grant in the re-authorized state"),
                 commit(kate_recovery("kate older reference", reference=ids["revision"], adds=renewed_without_microphone), fails("admission", "recovery_rule")),
                 commit(kate_recovery("kate temporary scope", adds=sorted(renewed + [screenshot_persistent], key=gid)), fails("admission", "restore_rule"),
                        note="scope that existed only on a temporary branch is never restored"),
                 commit(kate_recovery("kate keep old ids", adds=[grant for grant in renewed if grant["resource"]["category"] != "network"], removes=[]),
                        fails("admission", "recovery_rule")),
                 record({"record": "recovery_closed", "recovery_id": synthetic_id("another ceremony")}, fails("admission", "recovery_rule")),
                 commit(kate_recovered, check={"current": renewed_ids, "ended": suspended}, note="positive control after the rejected attempts")],
             key_authority=True)
    scenario("recovery_c2_requires_admin", "R13", "Under option C2 only admin opens the ceremony; the user's steps are unchanged",
             contained + [record({"record": "recovery_opened", "recovery_id": ceremony_id, "key_epoch": 2, "mode": "c2", "role": "trusted_client"},
                                 fails("admission", "recovery_rule")),
                          record({"record": "recovery_opened", "recovery_id": ceremony_id, "key_epoch": 2, "mode": "c2", "role": "admin"})] +
             keyed[len(opened):] + [commit(kate_recovered, check={"current": renewed_ids})],
             key_authority=True)
    second_ceremony = synthetic_id("recovery ceremony 1001 second")
    editor_recovery = editor.payload("editor recovery in aborted ceremony", "restore", recovery_persistent, [editor_genesis_id], editor_genesis["result_state"],
                                     {"add_grants": [make_grant("editor projects recovered", "filesystem", "home:Projects", ["read", "write"], start=Times[10])],
                                      "remove_grant_ids": [gid(projects)]}, Times[10], "Re-authorize the editor", extra={"restore_reference": editor_genesis_id})
    editor_recovery["authorization"]["session_id"] = ceremony_id
    scenario("recovery_aborted_by_new_containment", "R13",
             "A new unverifiable run during an open ceremony suspends the grants it re-authorized and aborts it; a new ceremony needs a higher key epoch",
             keyed + [commit(kate_recovered, check={"current": renewed_ids}),
                      record({"record": "containment"}, check={"ended": suspended + renewed_ids}),
                      commit(sign_commit(recovery_persistent, editor_recovery)[0], fails("key_authority", "contained")),
                      record({"record": "recovery_opened", "recovery_id": second_ceremony, "key_epoch": 2, "mode": "c1", "role": "trusted_client"},
                             fails("admission", "recovery_rule")),
                      record({"record": "recovery_opened", "recovery_id": second_ceremony, "key_epoch": 3, "mode": "c1", "role": "trusted_client"},
                             check={"ended": suspended + renewed_ids})],
             key_authority=True)
    scenario("recovery_quarantine_requires_admin_first", "R13",
             "Quarantine (store integrity failure) denies everything until admin recover_user; then the same ceremony applies",
             prefix + [record({"record": "quarantine"}, check={"ended": suspended}),
                       record({"record": "recovery_opened", "recovery_id": ceremony_id, "key_epoch": 2, "mode": "c1", "role": "trusted_client"},
                              fails("admission", "recovery_rule")),
                       commit(kate_recovery("kate restore while quarantined", signer=persistent), fails("key_authority", "quarantined")),
                       record({"record": "admin_recover", "role": "trusted_client"}, fails("admission", "recovery_rule")),
                       record({"record": "admin_recover", "role": "admin"}, check={"ended": suspended}),
                       record({"record": "recovery_opened", "recovery_id": ceremony_id, "key_epoch": 2, "mode": "c1", "role": "trusted_client"})] +
             keyed[len(opened):] + [commit(kate_recovered, check={"current": renewed_ids, "ended": suspended})],
             key_authority=True)
    return scenarios


def run_scenario(scenario, valid_by_name, resolver):
    """Returns a list of failure strings for one stored journal scenario."""
    journal = journal_t(resolver, key_authority=scenario["key_authority"])
    failures = []
    for index, step in enumerate(scenario["steps"]):
        try:
            if "valid" in step:
                kind, data = valid_by_name[step["valid"]]
                if kind == "commit":
                    journal.admit_commit(data)
                else:
                    journal.admit_identity(data, kind)
            elif "commit" in step:
                journal.admit_commit(bytes.fromhex(step["commit"]))
            elif "identity" in step:
                journal.admit_identity(bytes.fromhex(step["identity"]), step["kind"])
            else:
                journal.admit_record(decode_record(step["record"]))
            outcome = "admitted"
        except oracle_error_t as error:
            outcome = {"layer": error.layer, "error": error.code}
        if outcome != step["expect"]:
            failures.append("scenario %s step %d: expected %s got %s" % (scenario["name"], index, step["expect"], outcome))
        for standing, grant_ids in step.get("check", {}).items():
            for grant_id in grant_ids:
                have = journal.standing(bytes.fromhex(grant_id))
                if have != standing:
                    failures.append("scenario %s step %d: grant %s is %s, expected %s" % (scenario["name"], index, grant_id[:8], have, standing))
    return failures


# ------------------------------------------------------------------ serialization and check


def vector_document():
    keys, resolver_keys, identity, chain, extra_valid, invalid, admission_invalid, limits, scenarios = build_vectors()
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
        "limits": limits,
        "invalid": [],
        "admission_invalid": [],
        "journal_scenarios": scenarios,
    }
    for item in identity:
        document["valid"].append({"name": item["name"], "kind": item["kind"], "envelope": item["envelope"].hex(),
                                  "object_id": hashlib.sha256(item["envelope"]).hexdigest()})
    for name, data in chain + extra_valid:
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
    limits = stored.get("limits", {})
    wrapper = limits.get("max_wrapper_bytes", 0)
    if not (0 < limits.get("max_envelope_submit_signed_message_bytes", wrapper + 1) <= wrapper
            and 0 < limits.get("max_envelope_commit_admitted_record_bytes", wrapper + 1) <= wrapper):
        failures.append("maximum envelope does not fit its IPC message or journal record wrapper")
    boundary = by_name.get("commit_boundary_max_size")
    if boundary is None or len(boundary) != limits.get("max_envelope_bytes"):
        failures.append("boundary vector is not exactly max_envelope_bytes")
    else:
        try:
            admit_chain([boundary], resolver)
        except oracle_error_t as error:
            failures.append("boundary vector not admissible: %s" % error)
    chain = [by_name[name] for name in stored["lineage"]["commits"]]
    valid_by_name = {item["name"]: (item["kind"], bytes.fromhex(item["envelope"])) for item in stored["valid"]}
    scenario_steps = 0
    for scenario in stored.get("journal_scenarios", []):
        failures.extend(run_scenario(scenario, valid_by_name, resolver))
        scenario_steps += len(scenario["steps"])
    if not stored.get("journal_scenarios"):
        failures.append("journal scenarios missing")
    for item in stored["admission_invalid"]:
        try:
            admit_chain(chain[: item["prefix"]] + [bytes.fromhex(item["envelope"])], resolver)
            failures.append("admission %s accepted" % item["name"])
        except oracle_error_t as error:
            if (error.layer, error.code) != (item["layer"], item["error"]):
                failures.append("admission %s: expected %s/%s got %s/%s" % (item["name"], item["layer"], item["error"], error.layer, error.code))
    for failure in failures:
        print("FAIL " + failure)
    print("checked %d valid, %d invalid, %d admission vectors, %d journal scenarios (%d steps): %s" % (
        len(stored["valid"]), len(stored["invalid"]), len(stored["admission_invalid"]), len(stored.get("journal_scenarios", [])),
        scenario_steps, "FAIL" if failures else "PASS"))
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
        print("wrote %d valid, %d invalid, %d admission vectors, %d journal scenarios" % (
            len(document["valid"]), len(document["invalid"]), len(document["admission_invalid"]), len(document["journal_scenarios"])))
        return 0
    return check_document(arguments[1])


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
