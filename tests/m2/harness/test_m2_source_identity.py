"""Source-identity evidence must fail closed (review findings R01, R01-r2).

Covers build/scripts/m2_source_identity.sh (used by qualify_m2_rust.sh,
build_m2_core.sh and the dispatcher) and the harness environment collector: a tree
without Git metadata, a failing identity command, a failing or malformed Cargo.lock
hash (synthetic sha256sum first in PATH, under both sh and bash), an unreadable
Cargo.lock and a dirty tree under --require-clean must FAIL; the real checkout and a
clean temporary repository with a well-formed digest are positive controls.
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

    def fake_tool(self, name, body):
        """A directory holding one synthetic tool, to be put first in PATH."""
        tool_dir = tempfile.mkdtemp(prefix="cham3_m2_fake_tool_")
        self.addCleanup(shutil.rmtree, tool_dir, True)
        path = os.path.join(tool_dir, name)
        with open(path, "w", encoding="utf-8") as handle:
            handle.write("#!/bin/sh\n" + body + "\n")
        os.chmod(path, 0o755)
        return tool_dir + os.pathsep + os.environ.get("PATH", "")

    def identity_with_path(self, root, interpreter, path, *arguments):
        output = os.path.join(root, "identity_out.txt")
        status, log = run([interpreter, os.path.join(root, IdentityScript), output] + list(arguments),
                          env=dict(os.environ, PATH=path))
        with open(output, "r", encoding="utf-8") as handle:
            return status, handle.read() + log

    def test_failed_hash_tool_fails_under_sh_and_bash(self):
        # R01-r2 reproduction: a sha256sum that fails without output must not yield PASS.
        root = exported_tree(with_git=True)
        self.addCleanup(shutil.rmtree, root, True)
        path = self.fake_tool("sha256sum", "printf '%s\\n' 'injected sha256sum failure' >&2\nexit 7")
        for interpreter in ("sh", "bash"):
            status, text = self.identity_with_path(root, interpreter, path, "--require-clean")
            self.assertEqual(status, 1, interpreter + ": " + text)
            self.assertIn("cargo_lock_sha256 FAILED", text)
            self.assertIn("sha256sum of Cargo.lock failed", text)
            self.assertNotIn("identity PASS", text)

    def test_malformed_hash_output_fails(self):
        root = exported_tree(with_git=True)
        self.addCleanup(shutil.rmtree, root, True)
        path = self.fake_tool("sha256sum", "printf '%s  %s\\n' 'not-a-digest' \"$2\"\nexit 0")
        status, text = self.identity_with_path(root, "sh", path)
        self.assertEqual(status, 1, text)
        self.assertIn("cargo_lock_sha256 INVALID", text)
        self.assertIn("Cargo.lock digest malformed", text)

    def test_short_hex_hash_output_fails(self):
        root = exported_tree(with_git=True)
        self.addCleanup(shutil.rmtree, root, True)
        path = self.fake_tool("sha256sum", "printf '%s  %s\\n' 'abc123' \"$2\"\nexit 0")
        status, text = self.identity_with_path(root, "sh", path)
        self.assertEqual(status, 1, text)
        self.assertIn("cargo_lock_sha256 INVALID", text)

    def test_unreadable_lock_fails(self):
        root = exported_tree(with_git=True)
        self.addCleanup(shutil.rmtree, root, True)
        lock = os.path.join(root, "Cargo.lock")
        os.chmod(lock, 0)
        self.addCleanup(os.chmod, lock, 0o644)
        # A readable result here means the fixture is not exercising the defect (for
        # example when run as root, which the dispatcher refuses): fail, never skip.
        self.assertFalse(os.access(lock, os.R_OK), "cannot create an unreadable Cargo.lock (running as root?)")
        status, text = self.identity(root)
        self.assertEqual(status, 1, text)
        self.assertIn("cargo_lock_sha256 FAILED", text)
        self.assertIn("sha256sum of Cargo.lock failed", text)

    def test_failed_architecture_tool_fails(self):
        root = exported_tree(with_git=True)
        self.addCleanup(shutil.rmtree, root, True)
        path = self.fake_tool("dpkg", "exit 3")
        status, text = self.identity_with_path(root, "sh", path)
        self.assertEqual(status, 1, text)
        self.assertIn("architecture FAILED", text)
        self.assertIn("dpkg --print-architecture failed", text)

    def test_clean_identity_records_well_formed_digest(self):
        root = exported_tree(with_git=True)
        self.addCleanup(shutil.rmtree, root, True)
        status, text = self.identity(root, "--require-clean")
        self.assertEqual(status, 0, text)
        self.assertRegex(text, r"(?m)^cargo_lock_sha256 [0-9a-f]{64}$")
        self.assertRegex(text, r"(?m)^tree [0-9a-f]{40}$")
        self.assertRegex(text, r"(?m)^euid [0-9]+$")

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

    def test_environment_collector_fails_with_unreadable_lock(self):
        root = exported_tree(with_git=True)
        self.addCleanup(shutil.rmtree, root, True)
        lock = os.path.join(root, "Cargo.lock")
        os.chmod(lock, 0)
        self.addCleanup(os.chmod, lock, 0o644)
        self.assertFalse(os.access(lock, os.R_OK), "cannot create an unreadable Cargo.lock (running as root?)")
        status, record, output = self.environment(root)
        self.assertEqual(status, 1, output)
        self.assertEqual(record["result"], "FAIL")
        self.assertIn("Cargo.lock digest failed", record["error"])
        self.assertIsNone(record["environment"]["cargo_lock_sha256"])

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
