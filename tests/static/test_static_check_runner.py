"""Sensitivity test for build/scripts/run_static_checks.sh.

The runner must pass on a clean clone of HEAD and must fail when a controlled
defect is introduced into a disposable clone (shell syntax, Python syntax and
YAML syntax). The original repository is never modified.
"""

import os
import shutil
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


class static_check_file_discovery_t(unittest.TestCase):
    """Regression test for review finding F2: a tree without git metadata must
    fail file discovery, not report per-check PASS over zero files."""

    def test_tree_without_git_fails_file_discovery(self):
        root = m0_support.repo_root()
        with tempfile.TemporaryDirectory() as temp:
            copy = os.path.join(temp, "tree")
            os.makedirs(copy)
            for relative in m0_support.tracked_files():
                source = os.path.join(root, relative)
                destination = os.path.join(copy, relative)
                os.makedirs(os.path.dirname(destination), exist_ok=True)
                shutil.copy2(source, destination)
            self.assertFalse(os.path.exists(os.path.join(copy, ".git")))
            completed = run_runner(copy)
        output = completed.stdout + completed.stderr
        self.assertEqual(completed.returncode, 1, output)
        self.assertIn("FAIL file_discovery", output)
        for line in ("PASS shellcheck", "PASS py_compile", "PASS yamllint"):
            self.assertNotIn(line, output, "per-check PASS reported over zero files")


if __name__ == "__main__":
    unittest.main()
