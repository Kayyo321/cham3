"""Sensitivity test for build/scripts/run_static_checks.sh.

The runner must pass on a clean clone of HEAD and must fail when a controlled
defect is introduced into a disposable clone (shell syntax, Python syntax and
YAML syntax). The original repository is never modified.
"""

import os
import sys
import tempfile
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402

RunnerPath = "build/scripts/run_static_checks.sh"


def clone_head(temp):
    destination = os.path.join(temp, "clone")
    completed = m0_support.run_tool(
        ["git", "clone", "--quiet", "--no-hardlinks", m0_support.repo_root(), destination]
    )
    if completed.returncode != 0:
        raise AssertionError("git clone failed: " + completed.stderr.strip())
    return destination


def run_runner(clone):
    return m0_support.run_tool([os.path.join(clone, RunnerPath)], cwd=clone)


def inject_defect(clone, suffix, defect_text):
    candidates = [p for p in m0_support.tracked_files(clone) if p.endswith(suffix)]
    if not candidates:
        raise AssertionError("no tracked file ending in %s to mutate" % suffix)
    path = os.path.join(clone, candidates[0])
    with open(path, "a", encoding="utf-8") as handle:
        handle.write(defect_text)
    return candidates[0]


class static_check_runner_t(unittest.TestCase):
    def test_clean_clone_of_head_passes(self):
        with tempfile.TemporaryDirectory() as temp:
            clone = clone_head(temp)
            completed = run_runner(clone)
        output = completed.stdout + completed.stderr
        self.assertEqual(completed.returncode, 0, output)
        self.assertIn("PASS", output)

    def test_defective_shell_script_fails_runner(self):
        self.assert_defect_fails(".sh", "\nif then\n", "shell syntax")

    def test_defective_python_file_fails_runner(self):
        self.assert_defect_fails(".py", "\ndef broken(:\n", "python syntax")

    def test_defective_workflow_yaml_fails_runner(self):
        self.assert_defect_fails(".yml", "\nbroken: [unclosed\n", "yaml syntax")

    def assert_defect_fails(self, suffix, defect_text, label):
        with tempfile.TemporaryDirectory() as temp:
            clone = clone_head(temp)
            mutated = inject_defect(clone, suffix, defect_text)
            completed = run_runner(clone)
        output = completed.stdout + completed.stderr
        self.assertNotEqual(
            completed.returncode,
            0,
            "runner passed despite %s defect in %s" % (label, mutated),
        )
        self.assertIn("FAIL", output)


if __name__ == "__main__":
    unittest.main()
