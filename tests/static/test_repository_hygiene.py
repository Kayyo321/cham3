"""Repository hygiene checks: no tracked build outputs, ignore rules, modes, names."""

import os
import re
import sys
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402

ForbiddenSuffixes = (
    ".deb",
    ".udeb",
    ".iso",
    ".buildinfo",
    ".changes",
    ".squashfs",
    ".img",
    ".qcow2",
    ".log",
)
ForbiddenComponents = frozenset({"out", ".build", "live-build-work", "chroot", "binary"})
ShellInterpreters = ("#!/bin/sh", "#!/bin/bash")
SnakeCaseStem = re.compile(r"^[a-z][a-z0-9_]*$")
SnakeCheckedRoots = ("build/", "tests/", ".github/workflows/")
SnakeCheckExemptRoots = ("build/live/",)
StrictModePattern = re.compile(r"(?m)^\s*set\s+-[a-zA-Z]*e")


def tracked_text_head(path, root):
    absolute = os.path.join(root, path)
    try:
        with open(absolute, "rb") as handle:
            return handle.read(256)
    except OSError:
        return b""


class repository_hygiene_t(unittest.TestCase):
    def test_no_build_outputs_are_tracked(self):
        problems = []
        for path in m0_support.tracked_files():
            parts = path.split("/")
            if path.endswith(ForbiddenSuffixes) or path.endswith(".DS_Store"):
                problems.append("tracked build output or junk: " + path)
            elif any(part in ForbiddenComponents for part in parts):
                problems.append("tracked build output directory: " + path)
        self.assertEqual(problems, [])

    def test_gitignore_ignores_out_directory(self):
        root = m0_support.repo_root()
        for sample in ("out/example.iso", "out/packages/example.deb"):
            completed = m0_support.run_tool(
                ["git", "-C", root, "check-ignore", "-q", sample]
            )
            self.assertEqual(
                completed.returncode, 0, "git does not ignore %s" % sample
            )

    def test_scripts_and_shebang_files_are_executable(self):
        root = m0_support.repo_root()
        modes = m0_support.tracked_modes()
        problems = []
        for path in m0_support.tracked_files():
            head = tracked_text_head(path, root)
            if path.endswith(".sh") or head.startswith(b"#!"):
                if modes.get(path) != "100755":
                    problems.append("not executable in git index: %s (%s)" % (path, modes.get(path)))
        self.assertEqual(problems, [])

    def test_shell_scripts_have_shebang_and_strict_mode(self):
        root = m0_support.repo_root()
        problems = []
        for path in m0_support.tracked_files():
            is_shell = (
                path.endswith(".sh")
                or path == "tests/run_tests.sh"
                or path.startswith("build/live/auto/")
            )
            if not is_shell:
                continue
            text = m0_support.read_text(path, root)
            first_line = text.split("\n", 1)[0].rstrip()
            if first_line not in ShellInterpreters:
                problems.append("missing POSIX/bash shebang: " + path)
            if not StrictModePattern.search(text):
                problems.append("no 'set -e' strict mode: " + path)
        self.assertEqual(problems, [])

    def test_new_build_and_test_names_are_snake_case(self):
        problems = []
        for path in m0_support.tracked_files():
            if not path.startswith(SnakeCheckedRoots):
                continue
            if path.startswith(SnakeCheckExemptRoots):
                continue
            for part in path.split("/"):
                if part.startswith("."):
                    continue
                stem = part.split(".", 1)[0]
                if not SnakeCaseStem.match(stem):
                    problems.append("non snake_case name: " + path)
                    break
        self.assertEqual(problems, [])

    def test_snake_case_rule_detects_camel_case(self):
        self.assertIsNone(SnakeCaseStem.match("scanSecrets"))
        self.assertIsNotNone(SnakeCaseStem.match("scan_secrets"))


if __name__ == "__main__":
    unittest.main()
