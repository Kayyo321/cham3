"""Image-level checks on the built M1 desktop image's package manifest (VM/container stage).

Requires out/image/cham3-m1-desktop-amd64.packages and provenance from
build/scripts/build_image.sh (CHAM3_LIVE_DIR=build/live-desktop,
CHAM3_IMAGE_NAME=cham3-m1-desktop-amd64). Missing artifacts FAIL (M0 convention).

This is the TRANSITIVE exclusion check: the manifest lists every installed
package, so a duplicate utility pulled in by a dependency or recommendation is
caught here, not only by the static control field check.
"""

import os
import sys
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402
import m1_support  # noqa: E402


def manifest_names():
    path = m1_support.image_manifest_path()
    m0_support.require_file(path, "run build/scripts/build_image.sh with the M1 profile first")
    with open(path, "r", encoding="utf-8", errors="replace") as handle:
        return m1_support.manifest_package_names(handle.read())


def manifest_problems(names):
    """Return problems for the M1 manifest: required present, forbidden absent (empty means accepted)."""
    problems = []
    for required in (m1_support.PackageDesktop, m1_support.PackageFull):
        if required not in names:
            problems.append("installed package missing from image: " + required)
    for name in m1_support.KdeCoreRequired:
        if name not in names:
            problems.append("KDE core utility missing from image: " + name)
    for name in m1_support.CuratedRequired:
        if name not in names:
            problems.append("curated tool missing from image: " + name)
    for category, forbidden in (
        ("duplicate utility", m1_support.ExcludedDuplicates),
        ("gated tool", m1_support.GatedTools),
        ("desktop conflict", m1_support.DesktopConflictNames),
    ):
        for name in sorted(names & set(forbidden)):
            problems.append("%s present in image (transitive): %s" % (category, name))
    return problems


class image_manifest_t(unittest.TestCase):
    def test_manifest_has_required_and_no_forbidden_packages(self):
        self.assertEqual(manifest_problems(manifest_names()), [])

    def test_image_checksums_verify(self):
        image_dir = os.path.join(m0_support.out_dir(), "image")
        m0_support.require_file(os.path.join(image_dir, "SHA256SUMS"), "build_image.sh output")
        self.assertEqual(m0_support.sha256sums_problems(image_dir), [])

    def test_provenance_records_m1_profile(self):
        path = os.path.join(m0_support.out_dir(), "image", "provenance.json")
        m0_support.require_file(path, "build_image.sh output")
        with open(path, "r", encoding="utf-8") as handle:
            text = handle.read()
        self.assertIn("build/live-desktop", text, "provenance does not record the M1 live tree")
        self.assertIn(m1_support.ImageDesktopName, text, "provenance does not record the M1 image name")


class image_manifest_fixture_t(unittest.TestCase):
    """Positive and negative controls for manifest_problems()."""

    def clean(self):
        names = {m1_support.PackageDesktop, m1_support.PackageFull}
        names |= set(m1_support.KdeCoreRequired) | set(m1_support.CuratedRequired)
        return names

    def test_positive_control_clean_manifest_accepted(self):
        self.assertEqual(manifest_problems(self.clean()), [])

    def test_transitive_thunar_is_caught(self):
        names = self.clean() | {"thunar"}
        self.assertIn("duplicate utility present in image (transitive): thunar", manifest_problems(names))

    def test_transitive_librewolf_is_caught(self):
        names = self.clean() | {"librewolf"}
        self.assertIn("gated tool present in image (transitive): librewolf", manifest_problems(names))

    def test_missing_cham3_full_is_caught(self):
        names = self.clean() - {m1_support.PackageFull}
        self.assertIn("installed package missing from image: cham3-full", manifest_problems(names))

    def test_manifest_name_parsing_strips_versions(self):
        text = "nmap\t7.95-1\nbind9-dnsutils:amd64\t9.20\n"
        self.assertEqual(m1_support.manifest_package_names(text), {"nmap", "bind9-dnsutils"})


if __name__ == "__main__":
    unittest.main()
