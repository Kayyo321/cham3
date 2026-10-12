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


def expected_deb_names():
    """Derive the exact set of .deb files the build must produce from the
    packaging/*/debian definitions. This keeps the original M0 guarantee that
    build_packages.sh produces nothing unexpected, while scaling to the M1
    package set (cham3-base plus cham3-desktop and cham3-full) without
    hardcoding M1 names. All current cham3 binary packages are Architecture:
    all; a non-"all" value is used verbatim and would need a build-arch lookup.
    """
    root = m0_support.repo_root()
    packaging = os.path.join(root, "packaging")
    names = []
    for entry in sorted(os.listdir(packaging)):
        debian = os.path.join(packaging, entry, "debian")
        control_path = os.path.join(debian, "control")
        changelog_path = os.path.join(debian, "changelog")
        if not (os.path.isfile(control_path) and os.path.isfile(changelog_path)):
            continue
        with open(changelog_path, "r", encoding="utf-8", errors="replace") as handle:
            first_line = handle.readline()
        version_match = re.match(r"^\S+ \(([^)]+)\)", first_line)
        if not version_match:
            raise AssertionError("cannot parse changelog version in %s" % changelog_path)
        version = version_match.group(1)
        stanzas = m0_support.parse_control(
            m0_support.read_text(os.path.join("packaging", entry, "debian", "control"))
        )
        for stanza in stanzas:
            if "Package" in stanza:
                arch = stanza.get("Architecture", "all")
                names.append("%s_%s_%s.deb" % (stanza["Package"], version, arch))
    return sorted(names)


class packages_output_t(unittest.TestCase):
    def test_exactly_the_expected_debs_are_built(self):
        names = sorted(os.path.basename(p) for p in glob.glob(os.path.join(packages_dir(), "*.deb")))
        expected = expected_deb_names()
        # cham3-base must always be among them (M0 anchor), and the set must be
        # exactly what packaging/ declares (no stray debs).
        self.assertIn(m0_support.DebFileName, expected)
        self.assertEqual(names, expected)

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
