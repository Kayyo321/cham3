"""Checks for the build output directory $CHAM3_OUT_DIR/packages."""

import glob
import os
import re
import sys
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402

LintianErrorLine = re.compile(r"^E: ", re.MULTILINE)


def packages_dir():
    directory = os.path.join(m0_support.out_dir(), "packages")
    if not os.path.isdir(directory):
        raise AssertionError("required directory missing: %s (run build_packages.sh)" % directory)
    return directory


def changelog_version():
    text = m0_support.read_text("packaging/cham3-base/debian/changelog")
    match = re.match(r"^cham3-base \(([^)]+)\)", text)
    if not match:
        raise AssertionError("cannot parse changelog version")
    return match.group(1)


class packages_output_t(unittest.TestCase):
    def test_exactly_the_expected_deb_is_built(self):
        names = sorted(os.path.basename(p) for p in glob.glob(os.path.join(packages_dir(), "*.deb")))
        self.assertEqual(names, [m0_support.DebFileName])

    def test_deb_version_matches_changelog_version(self):
        self.assertEqual(changelog_version(), m0_support.ExpectedVersion)
        self.assertIn("_%s_" % m0_support.ExpectedVersion, m0_support.DebFileName)

    def test_sha256sums_verify_and_cover_build_outputs(self):
        directory = packages_dir()
        self.assertEqual(m0_support.sha256sums_problems(directory), [])
        listed = {name for _, name in m0_support.sha256sums_entries(os.path.join(directory, "SHA256SUMS"))}
        for pattern in ("*.deb", "*.buildinfo", "*.changes"):
            for path in glob.glob(os.path.join(directory, pattern)):
                self.assertIn(os.path.basename(path), listed)

    def test_lintian_report_has_no_errors(self):
        path = os.path.join(packages_dir(), "lintian.txt")
        m0_support.require_file(path, "build_packages.sh must save lintian output")
        with open(path, "r", encoding="utf-8", errors="replace") as handle:
            report = handle.read()
        self.assertIsNone(
            LintianErrorLine.search(report), "lintian reported errors (E:) in lintian.txt"
        )


if __name__ == "__main__":
    unittest.main()
