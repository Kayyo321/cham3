"""Positive and negative controls for build/scripts/scan_secrets.sh.

Every fake secret is assembled at runtime from fragments, so no literal
secret pattern appears in this source file. The scanner scans tracked files,
which includes this file, so a literal would make the repository scan fail.
"""

import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402

ScannerPath = "build/scripts/scan_secrets.sh"
PemKinds = (
    "RSA PRIVATE KEY",
    "EC PRIVATE KEY",
    "DSA PRIVATE KEY",
    "OPENSSH PRIVATE KEY",
    "PGP PRIVATE KEY BLOCK",
    "PRIVATE KEY",
)
PayloadAlphabet = "A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8"


def pem_header(kind):
    return "-----BEGIN " + kind + "-----"


def github_token(prefix, length):
    return prefix + (PayloadAlphabet * 3)[:length]


def aws_key_id():
    return "AK" + "IA" + "ABCD2345EFGH6789"


def slack_token(letter):
    return "xox" + letter + "-" + "1234567890-1234567890-abcdefghijklmnopqrstuvwx"


def scanner():
    path = os.path.join(m0_support.repo_root(), ScannerPath)
    m0_support.require_file(path, "secret scanner entry point")
    return path


def scan(*paths):
    return m0_support.run_tool([scanner()] + list(paths), cwd=m0_support.repo_root())


def write_file(directory, name, content):
    path = os.path.join(directory, name)
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(content)
    return path


class secret_scanner_t(unittest.TestCase):
    def assert_detected(self, completed, filename):
        self.assertEqual(completed.returncode, 1, "scanner did not report a finding")
        output = completed.stdout + completed.stderr
        self.assertRegex(output, re.escape(filename) + r":\d+", "no file:line in output")
        return output

    def test_clean_file_exits_zero(self):
        with tempfile.TemporaryDirectory() as temp:
            path = write_file(temp, "clean.txt", "nothing secret here\nline two\n")
            completed = scan(path)
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)

    def test_pem_private_key_blocks_are_detected(self):
        for kind in PemKinds:
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temp:
                content = pem_header(kind) + "\nMIIBOgIBAAJBAKj34GkxFhD90vcNYfMu\n"
                path = write_file(temp, "key.txt", content)
                completed = scan(path)
                self.assert_detected(completed, "key.txt")

    def test_github_tokens_are_detected_and_not_printed_in_full(self):
        cases = (
            ("ghp_", 36),
            ("gho_", 36),
            ("ghs_", 36),
            ("github_pat_", 82),
        )
        for prefix, length in cases:
            with self.subTest(prefix=prefix), tempfile.TemporaryDirectory() as temp:
                token = github_token(prefix, length)
                path = write_file(temp, "tokens.env", "x=1\nTOKEN=" + token + "\n")
                completed = scan(path)
                output = self.assert_detected(completed, "tokens.env")
                self.assertNotIn(token, output, "scanner printed the full secret value")

    def test_aws_access_key_id_is_detected(self):
        with tempfile.TemporaryDirectory() as temp:
            path = write_file(temp, "aws.cfg", "key_id = " + aws_key_id() + "\n")
            self.assert_detected(scan(path), "aws.cfg")

    def test_slack_tokens_are_detected(self):
        for letter in ("b", "p"):
            with self.subTest(letter=letter), tempfile.TemporaryDirectory() as temp:
                path = write_file(temp, "slack.txt", slack_token(letter) + "\n")
                self.assert_detected(scan(path), "slack.txt")

    def test_recursive_directory_scan_finds_nested_secret(self):
        with tempfile.TemporaryDirectory() as temp:
            nested = os.path.join(temp, "a", "b")
            os.makedirs(nested)
            write_file(nested, "deep.txt", pem_header("RSA PRIVATE KEY") + "\n")
            self.assert_detected(scan(temp), "deep.txt")

    def test_public_key_block_is_not_flagged(self):
        with tempfile.TemporaryDirectory() as temp:
            content = pem_header("PUBLIC KEY") + "\nMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE\n"
            path = write_file(temp, "public.txt", content)
            completed = scan(path)
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)

    def test_truncated_aws_prefix_is_not_flagged(self):
        with tempfile.TemporaryDirectory() as temp:
            path = write_file(temp, "short.txt", "AK" + "IA" + "ABCD2345\n")
            completed = scan(path)
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)

    def test_usage_error_exits_two(self):
        with tempfile.TemporaryDirectory() as temp:
            missing = os.path.join(temp, "does-not-exist")
            completed = scan(missing)
        self.assertEqual(completed.returncode, 2, completed.stdout + completed.stderr)

    def test_scanner_does_not_flag_itself(self):
        completed = scan(scanner())
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)

    def test_repository_default_scan_is_clean(self):
        completed = m0_support.run_tool([scanner()], cwd=m0_support.repo_root())
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)


class secret_scanner_regression_t(unittest.TestCase):
    """Regression tests for review findings F1, F3 and F4 (fail-closed reading,
    exact path:line reporting, and symlink handling)."""

    def require_non_root(self):
        if os.geteuid() == 0:
            self.skipTest(
                "running as root: chmod cannot make a file unreadable, so the "
                "unreadable-file check cannot be exercised"
            )

    def test_unreadable_file_is_error_exit_two(self):
        self.require_non_root()
        with tempfile.TemporaryDirectory() as temp:
            path = write_file(temp, "unreadable.txt", pem_header("RSA PRIVATE KEY") + "\n")
            os.chmod(path, 0)
            try:
                completed = scan(path)
            finally:
                os.chmod(path, 0o600)
        output = completed.stdout + completed.stderr
        self.assertEqual(completed.returncode, 2, output)
        self.assertIn("scan_secrets: ERROR cannot scan", completed.stderr)
        self.assertIn(path, completed.stderr)
        self.assertNotIn("no matches", output, "unscanned file reported as clean")

    def test_unreadable_file_inside_scanned_directory_is_error(self):
        self.require_non_root()
        with tempfile.TemporaryDirectory() as temp:
            path = write_file(temp, "hidden.txt", "plain text\n")
            os.chmod(path, 0)
            try:
                completed = scan(temp)
            finally:
                os.chmod(path, 0o600)
        self.assertNotEqual(completed.returncode, 0, completed.stdout + completed.stderr)
        self.assertIn("scan_secrets: ERROR cannot scan", completed.stderr)
        self.assertIn(path, completed.stderr)

    def test_unreadable_file_plus_finding_exits_one_and_reports_both(self):
        self.require_non_root()
        with tempfile.TemporaryDirectory() as temp:
            hidden = write_file(temp, "hidden.txt", "plain text\n")
            write_file(temp, "found.txt", pem_header("RSA PRIVATE KEY") + "\n")
            os.chmod(hidden, 0)
            try:
                completed = scan(temp)
            finally:
                os.chmod(hidden, 0o600)
        output = completed.stdout + completed.stderr
        self.assertEqual(completed.returncode, 1, output)
        self.assertIn("found.txt:1", output)
        self.assertIn("scan_secrets: ERROR cannot scan", completed.stderr)

    def test_finding_in_colon_named_file_reports_path_and_line(self):
        with tempfile.TemporaryDirectory() as temp:
            path = write_file(temp, "colon:name.txt", "x\n" + pem_header("RSA PRIVATE KEY") + "\n")
            completed = scan(path)
        output = completed.stdout + completed.stderr
        self.assertEqual(completed.returncode, 1, output)
        self.assertIn("colon:name.txt:2", output)

    def test_finding_in_file_with_space_reports_path_and_line(self):
        with tempfile.TemporaryDirectory() as temp:
            path = write_file(temp, "with space.txt", "x\n" + pem_header("RSA PRIVATE KEY") + "\n")
            completed = scan(path)
        output = completed.stdout + completed.stderr
        self.assertEqual(completed.returncode, 1, output)
        self.assertIn("with space.txt:2", output)

    def test_symlink_inside_scanned_dir_to_outside_secret_is_detected(self):
        with tempfile.TemporaryDirectory() as temp:
            scanned = os.path.join(temp, "scanned")
            outside = os.path.join(temp, "outside")
            os.makedirs(scanned)
            os.makedirs(outside)
            target = write_file(outside, "target.key", pem_header("RSA PRIVATE KEY") + "\n")
            os.symlink(target, os.path.join(scanned, "link.txt"))
            completed = scan(scanned)
        self.assertEqual(completed.returncode, 1, completed.stdout + completed.stderr)

    def test_lone_dangling_symlink_exits_zero(self):
        with tempfile.TemporaryDirectory() as temp:
            scanned = os.path.join(temp, "scanned")
            os.makedirs(scanned)
            os.symlink(os.path.join(temp, "missing.key"), os.path.join(scanned, "dangling.txt"))
            completed = scan(scanned)
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)


# Tools the scanner needs in PATH when it is given PATH arguments, apart from grep.
# The scanner runs through its absolute #!/bin/bash shebang, so bash is not looked up.
ScannerSupportTools = ("dirname", "mktemp", "find", "rm", "cut")
CrashingGrep = "#!/bin/sh\nkill -s SEGV $$\n"
UnexpectedStatusGrep = "#!/bin/sh\nexit 3\n"
# Crashes only for files whose name contains "broken"; otherwise runs the real grep.
SelectiveCrashGrep = """#!/bin/sh
for last_arg in "$@"; do :; done
case "${last_arg}" in
    *broken*) kill -s SEGV $$ ;;
esac
exec %s "$@"
"""


class secret_scanner_dependency_t(unittest.TestCase):
    """Regression tests for Codex review finding R1: a missing, non-executable,
    crashed or otherwise failing search tool (grep) must be a scan error, never
    a PASS. Each test runs the scanner with a PATH that holds only the tools it
    needs, so the fault is confined to grep. Fixtures are synthetic."""

    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.tools_dir = os.path.join(self.scratch.name, "tools")
        os.makedirs(self.tools_dir)
        for tool in ScannerSupportTools:
            os.symlink(self.require_tool(tool), os.path.join(self.tools_dir, tool))
        self.data_dir = os.path.join(self.scratch.name, "data")
        os.makedirs(self.data_dir)

    def require_tool(self, name):
        path = shutil.which(name)
        if path is None:
            raise AssertionError("required executable not found: " + name)
        return os.path.realpath(path)

    def install_real_grep(self):
        os.symlink(self.require_tool("grep"), os.path.join(self.tools_dir, "grep"))

    def install_grep_script(self, content, mode=0o755):
        path = os.path.join(self.tools_dir, "grep")
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(content)
        os.chmod(path, mode)

    def scan_with_tools(self, *paths):
        env = dict(os.environ)
        env["PATH"] = self.tools_dir
        return subprocess.run(
            [scanner()] + list(paths),
            cwd=self.scratch.name,
            env=env,
            capture_output=True,
            text=True,
            check=False,
            timeout=60,
        )

    def write_clean(self, name="clean.txt"):
        return write_file(self.data_dir, name, "nothing secret here\n")

    def write_secret(self, name):
        self.token = github_token("ghp_", 36)
        return write_file(self.data_dir, name, "TOKEN=" + self.token + "\n")

    def assert_scan_error(self, completed, path):
        output = completed.stdout + completed.stderr
        self.assertEqual(completed.returncode, 2, output)
        self.assertIn("scan_secrets: ERROR cannot scan " + path, completed.stderr)
        self.assertNotIn("PASS", output, "unscanned input reported as clean")
        self.assertNotIn(self.token, output, "scanner printed the fixture value")

    def test_clean_input_passes_with_real_grep(self):
        # Positive control: the restricted PATH itself is sufficient for a clean scan.
        self.install_real_grep()
        completed = self.scan_with_tools(self.write_clean())
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)
        self.assertIn("scan_secrets: PASS", completed.stdout)

    def test_finding_is_reported_with_real_grep(self):
        # Positive control: the restricted PATH detects a synthetic finding.
        self.install_real_grep()
        path = self.write_secret("found.txt")
        completed = self.scan_with_tools(path)
        output = completed.stdout + completed.stderr
        self.assertEqual(completed.returncode, 1, output)
        self.assertIn("found.txt:1", output)
        self.assertNotIn(self.token, output, "scanner printed the fixture value")

    def test_missing_grep_is_scan_error(self):
        path = self.write_secret("unscanned.txt")
        self.assert_scan_error(self.scan_with_tools(path), path)

    def test_non_executable_grep_is_scan_error(self):
        self.install_grep_script(CrashingGrep, mode=0o644)
        path = self.write_secret("unscanned.txt")
        self.assert_scan_error(self.scan_with_tools(path), path)

    def test_crashing_grep_is_scan_error(self):
        self.install_grep_script(CrashingGrep)
        path = self.write_secret("unscanned.txt")
        self.assert_scan_error(self.scan_with_tools(path), path)

    def test_unexpected_grep_status_is_scan_error(self):
        self.install_grep_script(UnexpectedStatusGrep)
        path = self.write_secret("unscanned.txt")
        self.assert_scan_error(self.scan_with_tools(path), path)

    def test_crash_on_one_file_with_clean_other_is_scan_error(self):
        self.install_grep_script(SelectiveCrashGrep % self.require_tool("grep"))
        self.write_clean()
        broken = self.write_secret("broken.txt")
        completed = self.scan_with_tools(self.data_dir)
        self.assert_scan_error(completed, broken)
        self.assertNotIn("clean.txt", completed.stderr, "a scanned file was reported as unscanned")

    def test_finding_takes_precedence_over_crash_on_other_file(self):
        self.install_grep_script(SelectiveCrashGrep % self.require_tool("grep"))
        broken = self.write_clean("broken.txt")
        self.write_secret("found.txt")
        completed = self.scan_with_tools(self.data_dir)
        output = completed.stdout + completed.stderr
        self.assertEqual(completed.returncode, 1, output)
        self.assertIn("found.txt:1", output)
        self.assertIn("scan_secrets: ERROR cannot scan " + broken, completed.stderr)
        self.assertNotIn(self.token, output, "scanner printed the fixture value")


if __name__ == "__main__":
    unittest.main()
