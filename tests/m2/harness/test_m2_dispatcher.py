"""Failure behavior of tests/m2/run_m2_tests.sh itself."""

import os
import shutil
import tempfile
import unittest

from m2_harness_support import RepoRoot, run

Dispatcher = os.path.join(RepoRoot, "tests", "m2", "run_m2_tests.sh")


class dispatcher_t(unittest.TestCase):
    def test_unknown_suite_is_usage_error(self):
        status, output = run(["sh", Dispatcher, "everything"])
        self.assertEqual(status, 2, output)

    def test_missing_argument_is_usage_error(self):
        status, output = run(["sh", Dispatcher])
        self.assertEqual(status, 2, output)

    def test_unimplemented_suites_fail_not_pass(self):
        for suite in ("integration", "recovery", "packaging"):
            with self.subTest(suite=suite):
                status, output = run(["sh", Dispatcher, suite])
                self.assertEqual(status, 1, output)
                self.assertIn("NOT_RUN", output)
                self.assertIn("FAIL", output)

    def test_missing_cargo_is_failure(self):
        tool_dir = tempfile.mkdtemp(prefix="cham3_m2_tools_")
        self.addCleanup(shutil.rmtree, tool_dir, True)
        for tool in ("sh", "dirname", "id", "git", "rustc", "python3", "date", "mkdir", "cat"):
            located = shutil.which(tool)
            self.assertIsNotNone(located, tool)
            os.symlink(located, os.path.join(tool_dir, tool))
        out_dir = tempfile.mkdtemp(prefix="cham3_m2_out_")
        self.addCleanup(shutil.rmtree, out_dir, True)
        environment = {"PATH": tool_dir, "CHAM3_REPO_ROOT": RepoRoot, "CHAM3_OUT_DIR": out_dir, "HOME": out_dir}
        status, output = run([os.path.join(tool_dir, "sh"), Dispatcher, "unit"], env=environment)
        self.assertEqual(status, 1, output)
        self.assertIn("missing prerequisite: cargo", output)


if __name__ == "__main__":
    unittest.main()
