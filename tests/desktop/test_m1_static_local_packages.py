"""Static checks for the per-profile local package manifests (F1 staging contract).

build/live/cham3-local-packages (M0 profile) must name EXACTLY cham3-base.
build/live-desktop/cham3-local-packages (M1 profile) must name EXACTLY
cham3-desktop and cham3-full. cham3-base is not staged for M1: live-build
force-installs every .deb staged into config/packages.chroot, so a wildcard or
an extra name would silently change what the image installs.

These read the manifests as text and check their names against the packaging
sources. They do not run build_image.sh. The behavior of the staging logic is
checked by tests/container/test_m1_container_staging.py, which runs the real
script in a debian:trixie container because it needs dpkg-deb.
"""

import os
import sys
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m1_support  # noqa: E402


def profile_manifest_problems(names, expected):
    """Problems with one profile's manifest names against the exact expected set."""
    problems = []
    if len(set(names)) != len(names):
        problems.append("duplicate names in manifest: %s" % ", ".join(sorted(names)))
    unknown = sorted(set(names) - set(m1_support.LocalPackageNames))
    if unknown:
        problems.append("manifest names non-contracted package(s): " + ", ".join(unknown))
    if sorted(set(names)) != sorted(expected):
        problems.append(
            "manifest must name exactly %s, found %s"
            % (", ".join(sorted(expected)), ", ".join(sorted(names)) or "none")
        )
    return problems


def manifest_text(profile_dir):
    return m1_support.read_repo(profile_dir + "/" + m1_support.ProfileManifestName)


class local_packages_manifest_real_t(unittest.TestCase):
    def test_m0_profile_stages_exactly_cham3_base(self):
        names = m1_support.local_manifest_names(manifest_text(m1_support.M0ProfileDir))
        self.assertEqual(profile_manifest_problems(names, m1_support.M0StagedPackages), [])

    def test_m1_profile_stages_exactly_desktop_and_full(self):
        names = m1_support.local_manifest_names(manifest_text(m1_support.M1ProfileDir))
        self.assertEqual(profile_manifest_problems(names, m1_support.M1StagedPackages), [])
        self.assertNotIn(m1_support.PackageBase, names, "M1 must not stage cham3-base")

    def test_every_manifest_name_has_a_packaging_source(self):
        # A manifest may name only packages the repository actually builds.
        package_dirs = {
            m1_support.PackageBase: "packaging/cham3-base",
            m1_support.PackageDesktop: m1_support.PackageDesktopDir,
            m1_support.PackageFull: m1_support.PackageFullDir,
        }
        for profile_dir, expected in (
            (m1_support.M0ProfileDir, m1_support.M0StagedPackages),
            (m1_support.M1ProfileDir, m1_support.M1StagedPackages),
        ):
            for name in expected:
                stanza = m1_support.package_control(package_dirs[name], name)
                self.assertEqual(stanza.get("Package"), name, profile_dir + " -> " + name)


class local_packages_manifest_fixture_t(unittest.TestCase):
    """Positive and negative controls for the manifest checker and parser."""

    def test_comments_and_blank_lines_are_ignored(self):
        text = "# header comment\n\ncham3-desktop  # trailing comment\n\ncham3-full\n"
        self.assertEqual(m1_support.local_manifest_names(text), ["cham3-desktop", "cham3-full"])

    def test_exact_m1_set_is_accepted(self):
        names = ["cham3-desktop", "cham3-full"]
        self.assertEqual(profile_manifest_problems(names, m1_support.M1StagedPackages), [])

    def test_m1_set_with_cham3_base_is_rejected(self):
        names = ["cham3-base", "cham3-desktop", "cham3-full"]
        problems = profile_manifest_problems(names, m1_support.M1StagedPackages)
        self.assertTrue(any("exactly" in p for p in problems), problems)

    def test_m0_set_with_desktop_suite_is_rejected(self):
        # The bug Codex F1 reproduced: the M0 build staged the M1 desktop suite.
        names = ["cham3-base", "cham3-desktop", "cham3-full"]
        problems = profile_manifest_problems(names, m1_support.M0StagedPackages)
        self.assertTrue(any("exactly" in p for p in problems), problems)
        self.assertTrue(any("non-contracted" in p or "exactly" in p for p in problems), problems)

    def test_missing_package_is_rejected(self):
        problems = profile_manifest_problems(["cham3-desktop"], m1_support.M1StagedPackages)
        self.assertTrue(any("exactly" in p for p in problems), problems)

    def test_duplicate_name_is_rejected(self):
        problems = profile_manifest_problems(["cham3-base", "cham3-base"], m1_support.M0StagedPackages)
        self.assertTrue(any("duplicate" in p for p in problems), problems)

    def test_unknown_name_is_rejected(self):
        problems = profile_manifest_problems(["cham3-base", "librewolf"], m1_support.M0StagedPackages)
        self.assertTrue(any("non-contracted" in p for p in problems), problems)


if __name__ == "__main__":
    unittest.main()
