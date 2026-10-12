"""Sensitivity of build/scripts/check_rust_naming.py (repository naming rule)."""

import os
import subprocess
import sys
import tempfile
import unittest

from m2_harness_support import RepoRoot

Checker = os.path.join(RepoRoot, "build", "scripts", "check_rust_naming.py")


def check(source):
    with tempfile.NamedTemporaryFile("w", suffix=".rs", delete=False, encoding="utf-8") as handle:
        handle.write(source)
        path = handle.name
    try:
        completed = subprocess.run(
            [sys.executable, "-I", Checker, path], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, check=False
        )
    finally:
        os.unlink(path)
    return completed.returncode, completed.stdout


class naming_checker_t(unittest.TestCase):
    def test_repository_sources_pass(self):
        completed = subprocess.run(
            [sys.executable, "-I", Checker], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, check=False,
            env=dict(os.environ, CHAM3_REPO_ROOT=RepoRoot),
        )
        self.assertEqual(completed.returncode, 0, completed.stdout)
        self.assertIn("PASS", completed.stdout)

    def test_conforming_source_passes(self):
        status, output = check(
            "pub struct record_t { value: u8 }\npub enum state_t { open, closed(u8) }\n"
            "pub const MaxItems: usize = 4;\npub fn read_value<item_t>(item: item_t) -> u8 { let local_value = 1; local_value }\n"
            "// commentCamel and \"stringCamel\" are ignored\n"
        )
        self.assertEqual(status, 0, output)

    def test_violations_are_reported(self):
        cases = {
            "struct BadType { x: u8 }": "not snake_case_t",
            "enum state_t { Open }": "enum variant 'Open'",
            "const max_items: u8 = 1;": "not PascalCase",
            "static counter: u8 = 1;": "not PascalCase",
            "fn readValue() {}": "function 'readValue'",
            "fn f() { let someValue = 1; }": "binding 'someValue'",
            "fn f<T>(item: T) {}": "generic parameter 'T'",
            "type Alias = u8;": "type alias 'Alias'",
            "trait Reader {}": "trait 'Reader'",
            "mod Inner {}": "module 'Inner'",
            "macro_rules! bad_macro { () => {} }": "macro 'bad_macro'",
            "fn f() { call(myArgument); }": "camelCase identifier 'myArgument'",
        }
        for source, expected in cases.items():
            with self.subTest(source=source):
                status, output = check(source + "\n")
                self.assertEqual(status, 1, output)
                self.assertIn(expected, output)


if __name__ == "__main__":
    unittest.main()
