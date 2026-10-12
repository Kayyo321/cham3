"""Seeded hostile-input corpus for verification layers 1-5 (M2-003, acceptance A02/A03).

Builds a deterministic corpus from the published ECR v1 vectors and computes the
expected outcome of every case with the independent oracle (ecr_oracle.verify). The
harness (tests/m2/lib/m2_harness.py format-differential) feeds the same cases to the
Rust probe and requires identical outcomes.

Case families, per valid object:
* envelope byte edits (flip, insert, delete, truncate, extend);
* payload byte edits, re-signed with the object's synthetic key, so the edited
  payload is authenticated and reaches the payload decoder;
* structural payload edits, re-encoded deterministically and re-signed: deleted,
  added or retyped fields, boundary integers, hostile text (control and bidi code
  points, trailing newline, non-ASCII digits, whitespace), reordered, duplicated or
  oversized lists, and swapped enumeration values, so the schema and intrinsic
  layers are exercised;
* protected-header edits, re-signed.
Plus every invalid, admission and journal object of the vector document.

All keys are the synthetic vector keys (public labels); nothing here is product data.
"""

import copy
import hashlib
import json
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import ecr_oracle  # noqa: E402

DefaultSeed = 3
DefaultCasesPerObject = 300
LocalLayers = ("envelope_decode", "envelope", "signature", "payload_decode", "schema", "intrinsic")

TextPool = [
    "", "a", "network_mode", "direct", "tor", "persistent", "timed", "session", "one_shot", "genesis", "revision",
    "revocation", "temporary_grant", "merge", "restore", "identity_transition", "temporary", "network", "filesystem",
    "resource_access", "cham3_authui", "compromised", "read", "write", "2026-10-12T12:00:00Z", "2026-10-12T11:00:00Z",
    "2026-02-30T12:00:00Z", "2026-10-12T12:00:60Z", "1999-12-31T23:59:59Z", "2026-10-12t12:00:00Z",
    "٢٠٢٦-10-12T12:00:00Z", "abc\n", "abc", "Abc", ".abc", "a b", "a‮b", "a\u0085b", "a\tb",
    " leading", "trailing ", "trailing ", "line\nline", "\n".join(["x"] * 17), "x" * 64, "x" * 65, "x" * 255,
    "x" * 256, "x" * 1024, "﻿", "café", "org.example.notes", "1abc", "a-b_c.d",
]
IntegerPool = [0, 1, 2, 7, 8, 9, 255, 256, 1001, 4294967294, 4294967295, 2**63, 2**64 - 1, -1, -19, -8, -(2**63)]


def expected_outcome(data, kind, resolver):
    """Outcome line in the probe's format."""
    try:
        ecr_oracle.verify(data, kind, resolver)
    except ecr_oracle.oracle_error_t as error:
        return "err %s %s" % (error.layer, error.code)
    return "ok %s same" % hashlib.sha256(data).hexdigest()


class corpus_builder_t:
    def __init__(self, document, seed, cases_per_object):
        self.random = random.Random(seed)
        self.cases_per_object = cases_per_object
        self.keys = {bytes.fromhex(key["key_id"]): ecr_oracle.key_t(key["label"]) for key in document["keys"]}
        for kid, key in self.keys.items():
            if key.key_id != kid:
                raise ValueError("synthetic key %s does not reproduce its key id" % key.label)
        self.resolver = ecr_oracle.resolver_t(self.keys.values())
        self.document = document
        self.cases = []

    def add(self, family, kind, data):
        self.cases.append({"family": family, "kind": kind, "data": bytes(data)})

    # -------------------------------------------------------------- edits

    def edit_bytes(self, data):
        data = bytearray(data)
        operation = self.random.choice(["flip", "set", "insert", "delete", "truncate", "extend"])
        position = self.random.randrange(len(data)) if data else 0
        if operation == "flip" and data:
            data[position] ^= 1 << self.random.randrange(8)
        elif operation == "set" and data:
            data[position] = self.random.randrange(256)
        elif operation == "insert":
            data.insert(position, self.random.randrange(256))
        elif operation == "delete" and data:
            del data[position]
        elif operation == "truncate":
            del data[position:]
        else:
            data.extend(bytes(self.random.randrange(256) for _ in range(self.random.randint(1, 3))))
        return bytes(data)

    def random_value(self, depth=0):
        choice = self.random.randrange(9 if depth < 2 else 6)
        if choice == 0:
            return self.random.choice(IntegerPool)
        if choice == 1:
            return bytes(self.random.randrange(256) for _ in range(self.random.choice([0, 1, 15, 16, 17, 31, 32, 33, 64])))
        if choice == 2:
            return self.random.choice(TextPool)
        if choice == 3:
            return self.random.choice([True, False])
        if choice == 4:
            return self.random.choice(TextPool[:30])
        if choice == 5:
            return []
        if choice == 6:
            return [self.random_value(depth + 1) for _ in range(self.random.randint(1, 3))]
        if choice == 7:
            return {self.random.choice(TextPool[1:20]): self.random_value(depth + 1)}
        return {}

    def nodes(self, value, path=()):
        """Every (container path) in a decoded payload."""
        found = []
        if isinstance(value, dict):
            found.append(path)
            for key, item in value.items():
                found.extend(self.nodes(item, path + (key,)))
        elif isinstance(value, list):
            found.append(path)
            for index, item in enumerate(value):
                found.extend(self.nodes(item, path + (index,)))
        return found

    def edit_structure(self, payload):
        payload = copy.deepcopy(payload)
        containers = self.nodes(payload)
        path = self.random.choice(containers)
        node = payload
        for step in path:
            node = node[step]
        if isinstance(node, dict):
            operation = self.random.choice(["delete", "add", "retype", "retype", "retype", "enum", "boundary", "text"])
            keys = sorted(node, key=ecr_oracle.encode)
            if operation == "delete" and keys:
                del node[self.random.choice(keys)]
            elif operation == "add":
                node[self.random.choice(["zz_unknown", "a", "kind", "parents", "session_id", "usage_limit", "expiration",
                                         "merge_base", "restore_reference", "identity_change", "parent_grant_id",
                                         "runtime_scope", "not_after"])] = self.random_value()
            elif keys:
                key = self.random.choice(keys)
                if operation == "retype":
                    node[key] = self.random_value()
                elif operation == "enum":
                    node[key] = self.random.choice(TextPool[1:30])
                elif operation == "boundary":
                    node[key] = self.random.choice(IntegerPool)
                else:
                    node[key] = self.random.choice(TextPool)
        else:
            operation = self.random.choice(["swap", "duplicate", "delete", "append", "clear", "grow"])
            if operation == "swap" and len(node) >= 2:
                left, right = self.random.sample(range(len(node)), 2)
                node[left], node[right] = node[right], node[left]
            elif operation == "duplicate" and node:
                node.append(node[self.random.randrange(len(node))])
            elif operation == "delete" and node:
                del node[self.random.randrange(len(node))]
            elif operation == "append":
                node.append(self.random_value())
            elif operation == "clear":
                node.clear()
            elif node:
                node.extend([node[-1]] * self.random.choice([8, 32, 64, 256]))
        return payload

    # -------------------------------------------------------------- envelope helpers

    def split(self, data):
        root = ecr_oracle.decoder_t(data, "envelope_decode").decode_top(allow_root_tag=True)
        protected, _, payload, _ = root.value
        kid = ecr_oracle.decoder_t(protected, "envelope_decode").decode_top()[ecr_oracle.LabelKid]
        return protected, payload, self.keys[kid]

    def signed(self, kind, signer, payload_bytes, protected=None):
        return ecr_oracle.build_envelope(kind, signer, payload_bytes, protected=protected)

    # -------------------------------------------------------------- corpus

    def build(self):
        for vector in self.document["valid"]:
            kind = vector["kind"]
            data = bytes.fromhex(vector["envelope"])
            self.add("valid", kind, data)
            protected, payload_bytes, signer = self.split(data)
            payload = ecr_oracle.decoder_t(payload_bytes, "payload_decode").decode_top()
            typ = ecr_oracle.Kinds[kind][0]
            for _ in range(self.cases_per_object):
                family = self.random.choice(["envelope_bytes", "payload_bytes", "structure", "structure", "structure",
                                             "structure", "header"])
                if family == "envelope_bytes":
                    self.add(family, kind, self.edit_bytes(data))
                elif family == "payload_bytes":
                    self.add(family, kind, self.signed(kind, signer, self.edit_bytes(payload_bytes)))
                elif family == "structure":
                    edited = self.edit_structure(payload)
                    try:
                        encoded = ecr_oracle.encode(edited)
                    except (TypeError, ValueError):
                        continue
                    if len(encoded) > ecr_oracle.MaxEnvelopeBytes:
                        continue
                    self.add(family, kind, self.signed(kind, signer, encoded))
                else:
                    header = {ecr_oracle.LabelAlg: ecr_oracle.AlgorithmEd25519, ecr_oracle.LabelKid: signer.key_id,
                              ecr_oracle.LabelTyp: typ}
                    choice = self.random.randrange(5)
                    if choice == 0:
                        header[ecr_oracle.LabelAlg] = self.random.choice([-8, -7, 0, 1, "EdDSA"])
                    elif choice == 1:
                        header[ecr_oracle.LabelTyp] = self.random.choice([t for t, _ in ecr_oracle.Kinds.values()] + ["x", 16])
                    elif choice == 2:
                        header[self.random.choice([2, 3, 5, 15, 17, -1])] = self.random_value()
                    elif choice == 3:
                        header[ecr_oracle.LabelKid] = self.random.choice([b"", signer.key_id[:31], signer.key_id + b"\x00", "kid"])
                    else:
                        del header[self.random.choice(list(header))]
                    self.add("header", kind, self.signed(kind, signer, payload_bytes, protected=ecr_oracle.encode(header)))
        for vector in self.document["invalid"]:
            self.add("invalid_vector", vector["kind"], bytes.fromhex(vector["envelope"]))
        for vector in self.document["admission_invalid"]:
            self.add("admission_vector", "commit", bytes.fromhex(vector["envelope"]))
        for scenario in self.document["journal_scenarios"]:
            for step in scenario["steps"]:
                if "commit" in step:
                    self.add("journal_object", "commit", bytes.fromhex(step["commit"]))
                elif "identity" in step:
                    self.add("journal_object", step["kind"], bytes.fromhex(step["identity"]))
        for case in self.cases:
            case["expected"] = expected_outcome(case["data"], case["kind"], self.resolver)
        return self.cases


def build_corpus(vectors_path, seed=DefaultSeed, cases_per_object=DefaultCasesPerObject):
    """Returns (cases, resolver keys) for the vector document at vectors_path."""
    with open(vectors_path, encoding="utf-8") as handle:
        document = json.load(handle)
    builder = corpus_builder_t(document, seed, cases_per_object)
    cases = builder.build()
    keys = [(key.key_id, key.public_key) for key in builder.keys.values()]
    return cases, keys


def probe_input(cases, keys):
    lines = ["key %s %s" % (kid.hex(), public.hex()) for kid, public in keys]
    lines.extend("verify %s %s" % (case["kind"], case["data"].hex()) for case in cases)
    return "\n".join(lines) + "\n"


def coverage(cases):
    """{outcome class: count}, where the class is "ok" or the layer of the error."""
    counts = {}
    for case in cases:
        parts = case["expected"].split(" ")
        label = "ok" if parts[0] == "ok" else parts[1]
        counts[label] = counts.get(label, 0) + 1
    return counts


def main(arguments):
    if len(arguments) != 1:
        print("usage: format_differential.py VECTORS_JSON  (prints the probe input)", file=sys.stderr)
        return 2
    cases, keys = build_corpus(arguments[0])
    sys.stdout.write(probe_input(cases, keys))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
