"""Positive and negative controls for build/scripts/scan_secrets.sh.

Every fake secret is assembled at runtime from fragments, so no literal
secret pattern appears in this source file. The scanner scans tracked files,
which includes this file, so a literal would make the repository scan fail.
"""

import os
import re
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


if __name__ == "__main__":
    unittest.main()
