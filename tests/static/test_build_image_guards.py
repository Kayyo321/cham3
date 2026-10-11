"""Regression tests for review finding I6: build_image.sh must refuse unsafe
output and work directories before any other check, including the root check.

These run the script WITHOUT root. A refusal must come from the path guard, not
from the root check, and must not touch the filesystem. The tests skip when run
as root: a broken script would then be able to start a live build into "/".
"""

import os
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402

ScriptPath = "build/scripts/build_image.sh"


def run_build_image(overrides):
    script = os.path.join(m0_support.repo_root(), ScriptPath)
    m0_support.require_file(script, "build_image entry point")
    env = {name: value for name, value in os.environ.items() if not name.startswith("CHAM3_")}
    env.update(overrides)
    try:
        completed = subprocess.run(
            [script],
            cwd=m0_support.repo_root(),
            env=env,
            capture_output=True,
            text=True,
            check=False,
            timeout=60,
        )
    except subprocess.TimeoutExpired:
        raise AssertionError("build_image.sh did not refuse promptly")
    return completed


class build_image_guards_t(unittest.TestCase):
    def require_non_root(self):
        if os.geteuid() == 0:
            self.skipTest(
                "running as root: a non-refusing build_image.sh could start a live "
                "build into '/', so the refusal guards are only tested as non-root"
            )

    def assert_refused_by_path_guard(self, completed):
        output = completed.stdout + completed.stderr
        self.assertEqual(completed.returncode, 1, output)
        self.assertIn("refusing", output)
        self.assertNotIn("must run as root", output, "refusal came after the root check")

    def test_out_dir_root_is_refused(self):
        self.require_non_root()
        completed = run_build_image({"CHAM3_OUT_DIR": "/"})
        self.assert_refused_by_path_guard(completed)

    def test_work_dir_root_is_refused(self):
        self.require_non_root()
        completed = run_build_image({"CHAM3_WORK_DIR": "/"})
        self.assert_refused_by_path_guard(completed)

    def test_work_dir_equal_to_out_dir_is_refused_and_nothing_removed(self):
        self.require_non_root()
        with tempfile.TemporaryDirectory() as temp:
            sentinel = os.path.join(temp, "keep-me.txt")
            with open(sentinel, "w", encoding="utf-8") as handle:
                handle.write("must survive a refused run\n")
            completed = run_build_image({"CHAM3_OUT_DIR": temp, "CHAM3_WORK_DIR": temp})
            self.assert_refused_by_path_guard(completed)
            self.assertTrue(os.path.isfile(sentinel), "refused run removed a file")


if __name__ == "__main__":
    unittest.main()
