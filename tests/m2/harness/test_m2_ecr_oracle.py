"""Sensitivity of the independent ECR v1 vector oracle (tests/m2/oracle/ecr_oracle.py).

The committed vectors must regenerate byte for byte and verify; controlled corruptions
of a copy (a flipped valid-envelope byte, a changed expected error code, a removed
invalid vector, a journal-scenario step whose expected outcome or grant standing is
changed) must make the check fail.
"""

import json
import os
import subprocess
import sys
import tempfile
import unittest

from m2_harness_support import RepoRoot

Oracle = os.path.join(RepoRoot, "tests", "m2", "oracle", "ecr_oracle.py")
Vectors = os.path.join(RepoRoot, "tests", "m2", "vectors", "ecr_v1", "vectors.json")


def check(path):
    completed = subprocess.run([sys.executable, "-I", Oracle, "check", path], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, check=False)
    return completed.returncode, completed.stdout


class ecr_oracle_t(unittest.TestCase):
    def corrupted_copy(self, change):
        with open(Vectors, "r", encoding="utf-8") as handle:
            document = json.load(handle)
        change(document)
        handle = tempfile.NamedTemporaryFile("w", suffix=".json", delete=False, encoding="utf-8")
        with handle:
            json.dump(document, handle)
        self.addCleanup(os.unlink, handle.name)
        return handle.name

    def test_committed_vectors_pass(self):
        status, output = check(Vectors)
        self.assertEqual(status, 0, output)
        self.assertIn("PASS", output)

    def test_vector_counts_are_nonzero(self):
        with open(Vectors, "r", encoding="utf-8") as handle:
            document = json.load(handle)
        self.assertGreaterEqual(len(document["valid"]), 16)
        self.assertGreaterEqual(len(document["invalid"]), 70)
        self.assertGreaterEqual(len(document["admission_invalid"]), 18)
        self.assertGreaterEqual(len(document["journal_scenarios"]), 23)
        findings = {scenario["finding"] for scenario in document["journal_scenarios"]}
        self.assertEqual(findings, {"R07-r2", "R12", "R13", "R14", "M2-005-R02"})
        expectations = [step["expect"] for scenario in document["journal_scenarios"] for step in scenario["steps"]]
        self.assertIn("admitted", expectations)
        self.assertTrue(any(isinstance(expect, dict) for expect in expectations))
        layers = {item["layer"] for item in document["invalid"]}
        self.assertEqual(layers, {"envelope_decode", "envelope", "signature", "payload_decode", "schema", "intrinsic"})

    def test_flipped_valid_envelope_fails(self):
        def change(document):
            envelope = bytearray.fromhex(document["valid"][-1]["envelope"])
            envelope[-1] ^= 1
            document["valid"][-1]["envelope"] = envelope.hex()

        status, output = check(self.corrupted_copy(change))
        self.assertEqual(status, 1, output)

    def test_changed_expected_error_fails(self):
        def change(document):
            document["invalid"][0]["error"] = "truncated" if document["invalid"][0]["error"] != "truncated" else "trailing_bytes"

        status, output = check(self.corrupted_copy(change))
        self.assertEqual(status, 1, output)

    def test_removed_vector_fails(self):
        status, output = check(self.corrupted_copy(lambda document: document["invalid"].pop()))
        self.assertEqual(status, 1, output)

    def test_changed_scenario_expectation_fails(self):
        def change(document):
            for scenario in document["journal_scenarios"]:
                for step in scenario["steps"]:
                    if isinstance(step["expect"], dict) and step["expect"]["error"] == "network_grant_conflict":
                        step["expect"] = "admitted"
                        return

        status, output = check(self.corrupted_copy(change))
        self.assertEqual(status, 1, output)

    def test_changed_grant_standing_fails(self):
        def change(document):
            for scenario in document["journal_scenarios"]:
                for step in scenario["steps"]:
                    if "ended" in step.get("check", {}):
                        step["check"]["current"] = step["check"].pop("ended")
                        return

        status, output = check(self.corrupted_copy(change))
        self.assertEqual(status, 1, output)


if __name__ == "__main__":
    unittest.main()
