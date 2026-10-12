"""Regression controls for the independent review's reproduced G2 holes (R06-R08).

Each case from the reviewer's schema_holes.py (review 9cf8825, evidence bundle) is
replayed separately against the repaired oracle and must now be rejected with the
designated layer/code; positive controls show the unmodified objects still pass.
"""

import copy
import importlib.util
import json
import os
import unittest

from m2_harness_support import RepoRoot

OraclePath = os.path.join(RepoRoot, "tests", "m2", "oracle", "ecr_oracle.py")
VectorsPath = os.path.join(RepoRoot, "tests", "m2", "vectors", "ecr_v1", "vectors.json")


def load_oracle():
    spec = importlib.util.spec_from_file_location("cham3_ecr_oracle", OraclePath)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class review_regressions_t(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.oracle = load_oracle()
        with open(VectorsPath, "r", encoding="utf-8") as handle:
            cls.vectors = json.load(handle)
        cls.keys = {item["label"]: cls.oracle.key_t(item["label"]) for item in cls.vectors["keys"]}
        cls.resolver = cls.oracle.resolver_t(list(cls.keys.values()))
        cls.valid = {item["name"]: bytes.fromhex(item["envelope"]) for item in cls.vectors["valid"]}
        cls.signer = cls.keys["user_1001_persistent"]

    def payload(self, name):
        return self.oracle.verify(self.valid[name], "commit", self.resolver)

    def assert_rejected(self, function, layer, code):
        with self.assertRaises(self.oracle.oracle_error_t) as caught:
            function()
        self.assertEqual((caught.exception.layer, caught.exception.code), (layer, code))

    def test_positive_control_lineage_admits(self):
        chain = [self.valid[name] for name in self.vectors["lineage"]["commits"]]
        self.assertEqual(len(self.oracle.admit_chain(chain, self.resolver)), len(chain))

    def test_r06_ordinary_revision_cannot_substitute_identity(self):
        revision = copy.deepcopy(self.payload("commit_revision"))
        revision["subject"]["package_identity"] = "deb:attacker:amd64"
        revision["subject"]["executable_sha256"] = b"\x42" * 32
        envelope, _ = self.oracle.sign_commit(self.signer, revision)
        self.assert_rejected(lambda: self.oracle.admit_chain([self.valid["commit_genesis"], envelope], self.resolver),
                             "admission", "subject_binding_mismatch")

    def test_r07_top_level_network_mode_is_not_representable(self):
        genesis = copy.deepcopy(self.payload("commit_genesis"))
        genesis["delta"] = {"add_grants": [], "remove_grant_ids": [], "network_mode": {"from": "offline", "to": "direct"}}
        genesis["result_state"] = {"grants": [], "network_mode": "direct"}
        envelope, _ = self.oracle.sign_commit(self.signer, genesis)
        self.assert_rejected(lambda: self.oracle.verify(envelope, "commit", self.resolver), "schema", "unknown_field")

    def test_r07_restore_cannot_introduce_network_absent_from_snapshot(self):
        names = self.vectors["lineage"]["commits"]
        case = next(item for item in self.vectors["admission_invalid"] if item["name"] == "restore_network_not_in_snapshot")
        chain = [self.valid[name] for name in names[: case["prefix"]]]
        self.assert_rejected(lambda: self.oracle.admit_chain(chain + [bytes.fromhex(case["envelope"])], self.resolver),
                             "admission", "restore_rule")

    def test_r08_maximum_object_fits_wrappers_and_one_more_byte_fails(self):
        limits = self.vectors["limits"]
        boundary = self.valid["commit_boundary_max_size"]
        self.assertEqual(len(boundary), limits["max_envelope_bytes"])
        self.oracle.admit_chain([boundary], self.resolver)
        wrapped = self.oracle.encode({"op": "submit_signed", "request_id": b"\x00" * 16,
                                      "args": {"proposal_id": b"\x00" * 16, "envelope": boundary}})
        self.assertLessEqual(len(wrapped), limits["max_wrapper_bytes"])
        self.assertLessEqual(len(wrapped) - len(boundary), limits["max_wrapper_metadata_bytes"])
        over = next(item for item in self.vectors["invalid"] if item["name"] == "boundary_one_byte_over")
        self.assertEqual(len(bytes.fromhex(over["envelope"])), limits["max_envelope_bytes"] + 1)
        self.assert_rejected(lambda: self.oracle.verify(bytes.fromhex(over["envelope"]), "commit", self.resolver),
                             "envelope_decode", "input_too_large")


if __name__ == "__main__":
    unittest.main()
