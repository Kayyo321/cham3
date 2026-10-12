"""Source-identity evidence must fail closed (review finding R01).

Covers build/scripts/m2_source_identity.sh (used by qualify_m2_rust.sh and the
dispatcher) and the harness environment collector: a tree without Git metadata, a
failing identity command and a dirty tree under --require-clean must FAIL; the real
checkout and a clean temporary repository are positive controls.
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

from m2_harness_support import RepoRoot, run

IdentityScript = os.path.join("build", "scripts", "m2_source_identity.sh")
Harness = os.path.join(RepoRoot, "tests", "m2", "lib", "m2_harness.py")


def exported_tree(with_git):
    root = tempfile.mkdtemp(prefix="cham3_m2_identity_")
    os.makedirs(os.path.join(root, "build", "scripts"))
    shutil.copy2(os.path.join(RepoRoot, IdentityScript), os.path.join(root, IdentityScript))
    shutil.copy2(os.path.join(RepoRoot, "Cargo.lock"), os.path.join(root, "Cargo.lock"))
    if with_git:
        git = ["git", "-C", root, "-c", "user.name=cham3 test", "-c", "user.email=test@invalid"]
        for command in (["init", "-q"], ["add", "-A"], ["commit", "-q", "-m", "fixture"]):
            status, output = run(git + command)
            assert status == 0, output
    return root


class source_identity_t(unittest.TestCase):
    def identity(self, root, *arguments):
        output = os.path.join(root, "identity_out.txt")
        status, log = run(["sh", os.path.join(root, IdentityScript), output] + list(arguments))
        with open(output, "r", encoding="utf-8") as handle:
            return status, handle.read() + log

    def test_real_checkout_identity_passes(self):
        out_dir = tempfile.mkdtemp(prefix="cham3_m2_identity_out_")
        self.addCleanup(shutil.rmtree, out_dir, True)
        status, log = run(["sh", os.path.join(RepoRoot, IdentityScript), os.path.join(out_dir, "identity.txt")])
        self.assertEqual(status, 0, log)
        with open(os.path.join(out_dir, "identity.txt"), "r", encoding="utf-8") as handle:
            text = handle.read()
        self.assertRegex(text, r"(?m)^commit [0-9a-f]{40}$")
        self.assertRegex(text, r"(?m)^identity PASS$")

    def test_tree_without_git_fails(self):
        root = exported_tree(with_git=False)
        self.addCleanup(shutil.rmtree, root, True)
        status, text = self.identity(root)
        self.assertEqual(status, 1, text)
        self.assertIn("commit MISSING", text)
        self.assertIn("identity FAIL", text)
        self.assertIn("dirty_tracked unknown", text)

    def test_clean_temporary_repository_passes_require_clean(self):
        root = exported_tree(with_git=True)
        self.addCleanup(shutil.rmtree, root, True)
        status, text = self.identity(root, "--require-clean")
        self.assertEqual(status, 0, text)
        self.assertIn("dirty_tracked 0", text)

    def test_dirty_repository_fails_require_clean_but_is_labeled_otherwise(self):
        root = exported_tree(with_git=True)
        self.addCleanup(shutil.rmtree, root, True)
        with open(os.path.join(root, "Cargo.lock"), "a", encoding="utf-8") as handle:
            handle.write("# controlled modification\n")
        status, text = self.identity(root, "--require-clean")
        self.assertEqual(status, 1, text)
        status, text = self.identity(root)
        self.assertEqual(status, 0, text)
        self.assertIn("dirty_tracked 1", text)

    def test_missing_cargo_lock_fails(self):
        root = exported_tree(with_git=True)
        self.addCleanup(shutil.rmtree, root, True)
        os.unlink(os.path.join(root, "Cargo.lock"))
        status, text = self.identity(root)
        self.assertEqual(status, 1, text)
        self.assertIn("cargo_lock_sha256 MISSING", text)

    def environment(self, repo_root, path=None):
        run_dir = tempfile.mkdtemp(prefix="cham3_m2_env_")
        self.addCleanup(shutil.rmtree, run_dir, True)
        environment = dict(os.environ, CHAM3_REPO_ROOT=repo_root)
        if path is not None:
            environment["PATH"] = path
        completed = subprocess.run(
            [sys.executable, "-I", Harness, "environment", "--run-dir", run_dir],
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, check=False, env=environment,
        )
        with open(os.path.join(run_dir, "environment.json"), "r", encoding="utf-8") as handle:
            return completed.returncode, json.load(handle), completed.stdout

    def test_environment_collector_passes_in_real_checkout(self):
        status, record, output = self.environment(RepoRoot)
        self.assertEqual(status, 0, output)
        self.assertEqual(record["result"], "PASS")

    def test_environment_collector_fails_without_git(self):
        root = exported_tree(with_git=False)
        self.addCleanup(shutil.rmtree, root, True)
        status, record, output = self.environment(root)
        self.assertEqual(status, 1, output)
        self.assertEqual(record["result"], "FAIL")
        self.assertIn("git_head", record["error"])
        self.assertIsNone(record["environment"]["git_dirty"])

    def test_environment_collector_fails_when_rustc_is_missing(self):
        tool_dir = tempfile.mkdtemp(prefix="cham3_m2_tools_")
        self.addCleanup(shutil.rmtree, tool_dir, True)
        for tool in ("git", "cargo", "uname", "cat", "dpkg"):
            located = shutil.which(tool)
            self.assertIsNotNone(located, tool)
            os.symlink(located, os.path.join(tool_dir, tool))
        status, record, output = self.environment(RepoRoot, path=tool_dir)
        self.assertEqual(status, 1, output)
        self.assertIn("rustc", record["error"])


if __name__ == "__main__":
    unittest.main()
