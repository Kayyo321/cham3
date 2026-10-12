"""M2-003 harness sensitivity: the format differential check must detect defects.

* A probe that disagrees with the oracle on one case, or drops a result line, FAILs.
* A corpus that does not reach every local layer FAILs the coverage floor.
* The seeded corpus is deterministic per seed.
* The oracle's text patterns reject a trailing newline and non-ASCII digits (the two
  oracle defects the differential check found).
* Seeded defects in a disposable copy of the Rust verifier (never the checkout) are
  each detected by the differential check, and the unmodified copy passes.
"""

import os
import shutil
import stat
import sys
import tempfile
import textwrap
import unittest

from m2_harness_support import RepoRoot, m2_harness, run

sys.path.insert(0, os.path.join(RepoRoot, "tests", "m2", "oracle"))
import ecr_oracle  # noqa: E402
import format_differential  # noqa: E402

VectorsPath = os.path.join(RepoRoot, "tests", "m2", "vectors", "ecr_v1", "vectors.json")

# Each seeded defect removes or weakens one rule of wire format §§2-6.
SeededDefects = [
    ("non_minimal_heads_accepted", "core/contract_format/src/dcbor.rs",
     "        if value < minimum {\n", "        if false && value < minimum {\n"),
    ("duplicate_map_keys_accepted", "core/contract_format/src/dcbor.rs",
     "if previous.is_some_and(|earlier| encoded_key <= earlier) {", "if previous.is_some_and(|earlier| encoded_key < earlier) {"),
    ("commit_key_id_not_bound_to_kid", "core/contract_format/src/schema.rs",
     "        if payload.key_id != *kid {\n", "        if false && payload.key_id != *kid {\n"),
    ("second_network_grant_accepted", "core/contract_format/src/intrinsic.rs",
     ".filter(|grant| grant.is_network()).count() > 1", ".filter(|grant| grant.is_network()).count() > 2"),
    ("bidirectional_controls_accepted", "core/contract_format/src/text.rs",
     "| 0x202a..=0x202e ", ""),
]


def fake_probe(directory, flip_index=None, drop_last=False):
    """An executable that answers with the oracle's outcomes, optionally wrong once."""
    path = os.path.join(directory, "fake_probe.py")
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(textwrap.dedent("""\
            #!/usr/bin/env python3
            import sys
            sys.path.insert(0, %r)
            import ecr_oracle, format_differential
            keys, cases = {}, []
            for line in sys.stdin.read().splitlines():
                parts = line.split(" ")
                if parts[0] == "key":
                    keys[bytes.fromhex(parts[1])] = bytes.fromhex(parts[2])
                else:
                    cases.append((parts[1], bytes.fromhex(parts[2])))
            resolver = ecr_oracle.resolver_t([])
            resolver.keys = keys
            answers = [format_differential.expected_outcome(data, kind, resolver) for kind, data in cases]
            flip, drop = %r, %r
            if flip is not None:
                answers[flip] = "err schema unknown_field" if answers[flip] != "err schema unknown_field" else "err schema missing_field"
            if drop:
                answers.pop()
            print("\\n".join(answers))
            """ % (os.path.join(RepoRoot, "tests", "m2", "oracle"), flip_index, drop_last)))
    os.chmod(path, os.stat(path).st_mode | stat.S_IXUSR)
    return path


class format_differential_t(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.mkdtemp(prefix="cham3_m2_format_")
        self.addCleanup(shutil.rmtree, self.directory, True)

    def run_dir(self, name):
        path = os.path.join(self.directory, name)
        os.makedirs(path)
        return path

    def test_faithful_probe_passes(self):
        record = m2_harness.run_format_differential(fake_probe(self.directory), self.run_dir("faithful"), 3, 40)
        self.assertEqual(record["result"], "PASS", record.get("error"))

    def test_one_wrong_outcome_is_detected(self):
        record = m2_harness.run_format_differential(fake_probe(self.directory, flip_index=200), self.run_dir("flip"), 3, 40)
        self.assertEqual(record["result"], "FAIL")
        self.assertEqual(record["mismatch_count"], 1)
        self.assertEqual(record["mismatches"][0]["index"], 200)

    def test_missing_result_line_is_detected(self):
        record = m2_harness.run_format_differential(fake_probe(self.directory, drop_last=True), self.run_dir("drop"), 3, 40)
        self.assertEqual(record["result"], "FAIL")
        self.assertIn("result lines", record["mismatches"][0]["expected"])

    def test_thin_corpus_fails_the_coverage_floor(self):
        cases, _ = format_differential.build_corpus(VectorsPath, seed=3, cases_per_object=0)
        _, _, errors = m2_harness.format_coverage_errors(cases)
        self.assertTrue(errors)

    def test_corpus_is_deterministic_per_seed(self):
        first, keys = format_differential.build_corpus(VectorsPath, seed=7, cases_per_object=20)
        second, _ = format_differential.build_corpus(VectorsPath, seed=7, cases_per_object=20)
        other, _ = format_differential.build_corpus(VectorsPath, seed=8, cases_per_object=20)
        self.assertEqual(format_differential.probe_input(first, keys), format_differential.probe_input(second, keys))
        self.assertNotEqual(format_differential.probe_input(first, keys), format_differential.probe_input(other, keys))

    def test_oracle_patterns_reject_trailing_newline_and_non_ascii_digits(self):
        for check, value in [(ecr_oracle.label_check, "abc\n"), (ecr_oracle.operation_check, "read\n"),
                             (ecr_oracle.check_time, "2026-10-12T12:00:00Z\n"),
                             (ecr_oracle.check_time, "٢٠٢٦-10-12T12:00:00Z")]:
            with self.assertRaises(ecr_oracle.oracle_error_t, msg=repr(value)):
                check(value, "field")
        ecr_oracle.label_check("abc", "field")
        ecr_oracle.check_time("2026-10-12T12:00:00Z", "field")


class seeded_verifier_defects_t(unittest.TestCase):
    """Builds the format probe from a disposable workspace copy with one defect at a time."""

    @classmethod
    def setUpClass(cls):
        cls.workspace = tempfile.mkdtemp(prefix="cham3_m2_format_defect_")
        for name in ("Cargo.toml", "Cargo.lock"):
            shutil.copy2(os.path.join(RepoRoot, name), os.path.join(cls.workspace, name))
        shutil.copytree(os.path.join(RepoRoot, ".cargo"), os.path.join(cls.workspace, ".cargo"))
        shutil.copytree(os.path.join(RepoRoot, "core"), os.path.join(cls.workspace, "core"))
        cls.environment = dict(os.environ, CARGO_TARGET_DIR=os.path.join(cls.workspace, "target"))
        cls.environment.pop("RUSTFLAGS", None)

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.workspace, ignore_errors=True)

    def build_and_compare(self, label):
        status, output = run(["cargo", "build", "--frozen", "-p", "cham3_contract_format", "--bin", "cham3_format_probe"],
                             cwd=self.workspace, env=self.environment)
        self.assertEqual(status, 0, output)
        run_dir = os.path.join(self.workspace, "runs", label)
        os.makedirs(run_dir)
        probe = os.path.join(self.workspace, "target", "debug", "cham3_format_probe")
        return m2_harness.run_format_differential(probe, run_dir, format_differential.DefaultSeed,
                                                  format_differential.DefaultCasesPerObject)

    def test_unmodified_copy_passes(self):
        record = self.build_and_compare("unmodified")
        self.assertEqual(record["result"], "PASS", record.get("error"))

    def test_each_seeded_defect_is_detected(self):
        for name, relative, original, defect in SeededDefects:
            with self.subTest(defect=name):
                path = os.path.join(self.workspace, relative)
                with open(path, encoding="utf-8") as handle:
                    source = handle.read()
                self.assertEqual(source.count(original), 1, "defect anchor for %s" % name)
                try:
                    with open(path, "w", encoding="utf-8") as handle:
                        handle.write(source.replace(original, defect))
                    record = self.build_and_compare(name)
                finally:
                    with open(path, "w", encoding="utf-8") as handle:
                        handle.write(source)
                self.assertEqual(record["result"], "FAIL", name)
                self.assertGreater(record["mismatch_count"], 0, name)


if __name__ == "__main__":
    unittest.main()
