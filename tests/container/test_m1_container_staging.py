"""Regression test for F1: per-profile local package staging (container stage).

Runs the REAL build/scripts/build_image.sh with CHAM3_STAGE_DRYRUN=1. In that mode
the script selects packages from the profile's cham3-local-packages manifest,
prints them one per line, and exits 0 before the root check and before any work
directory or ISO is created. Packages are matched by each .deb's real Package
field, not its filename. Inputs are placeholder .deb files built with dpkg-deb.

Expected sets (contract, corrected by the coordinator):
  M0 profile (build/live):          exactly cham3-base
  M1 profile (build/live-desktop):  exactly cham3-desktop and cham3-full

Fail-closed controls: a missing manifest, a manifest naming an unbuilt package,
an empty manifest, a deb whose filename matches but whose Package field does not,
and two built debs providing the same package must all make the script exit
nonzero without staging or publishing anything.

Needs dpkg-deb, so run it in the debian:trixie container (see
tests/container/run_container_checks.sh). On a host without dpkg-deb the suite
reports BLOCKED as a FAIL. It is not a live-build run and proves nothing about the
packages' contents.
"""

import os
import shutil
import sys
import tempfile
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402
import m1_support  # noqa: E402

UnlistedFixture = "cham3-fixture-unlisted"
LiveWorkName = "live-build-work"
SyntheticTrees = ("build/scripts", "build/live", "build/live-desktop")


def staged_names(completed):
    return [line.strip() for line in completed.stdout.splitlines() if line.strip()]


def stage_problems(completed, expected):
    """Problems with one dry-run result against the exact expected package set."""
    problems = []
    if completed.returncode != 0:
        problems.append("exit %d: %s" % (completed.returncode, completed.stderr.strip()))
        return problems
    names = staged_names(completed)
    if len(set(names)) != len(names):
        problems.append("duplicate names printed: %s" % " ".join(names))
    if sorted(set(names)) != sorted(expected):
        problems.append(
            "staged set must be exactly %s, found %s"
            % (" ".join(sorted(expected)), " ".join(sorted(names)) or "none")
        )
    return problems


class staging_profile_t(unittest.TestCase):
    """Dry-run selection against the real script, real manifests and fixture debs."""

    @classmethod
    def setUpClass(cls):
        if not m1_support.dpkg_deb_available():
            raise AssertionError(
                "BLOCKED (host environment): dpkg-deb is absent; run this suite in a "
                "debian:trixie container"
            )
        cls.scratch = tempfile.mkdtemp(prefix="cham3_stage_")
        cls.repo = m0_support.repo_root()
        cls.out_dir = os.path.join(cls.scratch, "out")
        cls.packages_dir = os.path.join(cls.out_dir, "packages")
        os.makedirs(cls.packages_dir)
        for package in m1_support.LocalPackageNames + (UnlistedFixture,):
            m1_support.build_fixture_deb(cls.packages_dir, package, package + "_0.0.0~fixture_all.deb")

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.scratch, ignore_errors=True)

    def assert_exact_dryrun(self, live_dir, expected):
        completed = m1_support.run_build_image_dryrun(self.repo, self.out_dir, live_dir)
        problems = stage_problems(completed, expected)
        self.assertEqual(problems, [], completed.stdout + completed.stderr)
        self.assertFalse(
            os.path.exists(os.path.join(self.out_dir, LiveWorkName)),
            "dry run created a live-build work directory",
        )
        self.assertFalse(
            os.path.exists(os.path.join(self.out_dir, "image")),
            "dry run created image output",
        )
        return staged_names(completed)

    def test_m0_profile_stages_exactly_cham3_base(self):
        self.assert_exact_dryrun(m1_support.M0ProfileDir, m1_support.M0StagedPackages)

    def test_unparameterized_run_defaults_to_m0_profile(self):
        # No CHAM3_LIVE_DIR: the default M0 build must keep its single-package set.
        completed = m1_support.run_build_image_dryrun(self.repo, self.out_dir, None)
        self.assertEqual(stage_problems(completed, m1_support.M0StagedPackages), [], completed.stderr)

    def test_m1_profile_stages_exactly_desktop_and_full(self):
        self.assert_exact_dryrun(m1_support.M1ProfileDir, m1_support.M1StagedPackages)

    def test_m1_profile_does_not_stage_cham3_base(self):
        names = self.assert_exact_dryrun(m1_support.M1ProfileDir, m1_support.M1StagedPackages)
        self.assertNotIn(m1_support.PackageBase, names)

    def test_built_package_not_in_manifest_is_not_staged(self):
        # UnlistedFixture is built and present, but no manifest names it.
        for live_dir in (m1_support.M0ProfileDir, m1_support.M1ProfileDir):
            names = staged_names(m1_support.run_build_image_dryrun(self.repo, self.out_dir, live_dir))
            self.assertNotIn(UnlistedFixture, names, live_dir)

    def test_package_missing_its_built_deb_fails_closed(self):
        # The M1 manifest names cham3-full; a packages directory without it must fail.
        partial_out = os.path.join(self.scratch, "partial-out")
        partial_packages = os.path.join(partial_out, "packages")
        os.makedirs(partial_packages)
        for package in (m1_support.PackageDesktop, m1_support.PackageBase):
            name = package + "_0.0.0~fixture_all.deb"
            shutil.copy(os.path.join(self.packages_dir, name), os.path.join(partial_packages, name))
        completed = m1_support.run_build_image_dryrun(self.repo, partial_out, m1_support.M1ProfileDir)
        self.assertNotEqual(completed.returncode, 0, completed.stdout + completed.stderr)
        self.assertEqual(staged_names(completed), [], "a failed selection printed names")
        self.assertIn(m1_support.PackageFull, completed.stderr)

    def test_filename_match_without_package_field_match_fails_closed(self):
        # The file is named cham3-base_*.deb but its Package field is cham3-basetools:
        # matching uses the Package field, so cham3-base is not provided.
        misnamed_out = os.path.join(self.scratch, "misnamed-out")
        misnamed_packages = os.path.join(misnamed_out, "packages")
        os.makedirs(misnamed_packages)
        built = m1_support.build_fixture_deb(misnamed_packages, "cham3-basetools", "x.deb")
        os.rename(built, os.path.join(misnamed_packages, "cham3-base_0.0.0~fixture_all.deb"))
        completed = m1_support.run_build_image_dryrun(self.repo, misnamed_out, m1_support.M0ProfileDir)
        self.assertNotEqual(completed.returncode, 0, completed.stdout + completed.stderr)
        self.assertIn(m1_support.PackageBase, completed.stderr)

    def test_two_built_debs_for_one_package_fail_closed(self):
        duplicate_out = os.path.join(self.scratch, "duplicate-out")
        duplicate_packages = os.path.join(duplicate_out, "packages")
        os.makedirs(duplicate_packages)
        m1_support.build_fixture_deb(duplicate_packages, m1_support.PackageBase, "one_0.0.0~fixture_all.deb")
        m1_support.build_fixture_deb(duplicate_packages, m1_support.PackageBase, "two_0.0.0~fixture_all.deb")
        completed = m1_support.run_build_image_dryrun(self.repo, duplicate_out, m1_support.M0ProfileDir)
        self.assertNotEqual(completed.returncode, 0, completed.stdout + completed.stderr)
        self.assertEqual(staged_names(completed), [], "an ambiguous selection printed names")

    def synthetic_repo_with(self, manifest_name, manifest_text):
        """Copy the real build tree into scratch and replace one profile manifest there.

        manifest_text None removes the manifest. The copy is used only for the
        fail-closed cases that would otherwise need a modified repository.
        """
        root = tempfile.mkdtemp(prefix="synthetic-", dir=self.scratch)
        for rel in SyntheticTrees:
            shutil.copytree(os.path.join(self.repo, rel), os.path.join(root, rel))
        target = os.path.join(root, manifest_name)
        if manifest_text is None:
            os.remove(target)
        else:
            with open(target, "w", encoding="utf-8") as handle:
                handle.write(manifest_text)
        return root

    def test_missing_manifest_fails_closed(self):
        root = self.synthetic_repo_with(m1_support.M1ProfileDir + "/" + m1_support.ProfileManifestName, None)
        completed = m1_support.run_build_image_dryrun(root, self.out_dir, m1_support.M1ProfileDir)
        self.assertNotEqual(completed.returncode, 0, completed.stdout + completed.stderr)
        self.assertEqual(staged_names(completed), [], "a missing manifest printed names")
        self.assertIn(m1_support.ProfileManifestName, completed.stderr)

    def test_empty_manifest_fails_closed(self):
        root = self.synthetic_repo_with(
            m1_support.M1ProfileDir + "/" + m1_support.ProfileManifestName,
            "# comments only\n\n",
        )
        completed = m1_support.run_build_image_dryrun(root, self.out_dir, m1_support.M1ProfileDir)
        self.assertNotEqual(completed.returncode, 0, completed.stdout + completed.stderr)
        self.assertEqual(staged_names(completed), [], "an empty manifest printed names")

    def test_manifest_naming_package_with_no_built_deb_fails_closed(self):
        root = self.synthetic_repo_with(
            m1_support.M0ProfileDir + "/" + m1_support.ProfileManifestName,
            "cham3-base\ncham3-not-built\n",
        )
        completed = m1_support.run_build_image_dryrun(root, self.out_dir, m1_support.M0ProfileDir)
        self.assertNotEqual(completed.returncode, 0, completed.stdout + completed.stderr)
        self.assertEqual(staged_names(completed), [], "a failed selection printed names")
        self.assertIn("cham3-not-built", completed.stderr)


class staging_guard_t(unittest.TestCase):
    """Safety controls for the dry-run helper. They need no dpkg-deb and never start a build.

    A dry run must not be able to start a live build. The guard refuses a build_image.sh that
    does not read CHAM3_STAGE_DRYRUN before the root check and the live-build steps, and the
    helper records any call to the stub lb or debootstrap that runs first on PATH.
    """

    def synthetic_tree(self, script):
        """A temporary tree whose build_image.sh is the given text (independent of the repository)."""
        root = tempfile.mkdtemp(prefix="cham3_guard_")
        self.addCleanup(shutil.rmtree, root, True)
        scripts = os.path.join(root, "build", "scripts")
        os.makedirs(scripts)
        with open(os.path.join(scripts, "build_image.sh"), "w", encoding="utf-8") as handle:
            handle.write(script)
        return root

    def test_guard_refuses_a_script_that_never_reads_the_dryrun_switch(self):
        root = self.synthetic_tree(
            "#!/bin/bash\nif [ \"${CHAM3_NO_SWITCH:-0}\" = \"1\" ]; then exit 0; fi\n"
            "[ \"$(id -u)\" -eq 0 ] || exit 1\n"
        )
        out_dir = tempfile.mkdtemp(prefix="cham3_guard_out_")
        self.addCleanup(shutil.rmtree, out_dir, True)
        with self.assertRaises(AssertionError) as raised:
            m1_support.run_build_image_dryrun(root, out_dir, m1_support.M0ProfileDir)
        self.assertIn("refusing to run build_image.sh", str(raised.exception))
        self.assertIn("does not read CHAM3_STAGE_DRYRUN", str(raised.exception))

    def test_guard_refuses_a_root_check_placed_before_the_dryrun_switch(self):
        root = self.synthetic_tree(
            "#!/bin/bash\n[ \"$(id -u)\" -eq 0 ] || exit 1\n"
            "if [ \"${CHAM3_STAGE_DRYRUN:-0}\" = \"1\" ]; then exit 0; fi\n"
        )
        out_dir = tempfile.mkdtemp(prefix="cham3_guard_out_")
        self.addCleanup(shutil.rmtree, out_dir, True)
        with self.assertRaises(AssertionError) as raised:
            m1_support.run_build_image_dryrun(root, out_dir, m1_support.M0ProfileDir)
        self.assertIn("before it reads CHAM3_STAGE_DRYRUN", str(raised.exception))

    def test_stub_live_build_invocation_is_detected(self):
        # A script that reads the switch but then runs lb anyway: the stub lb records the call.
        script = (
            "#!/bin/bash\n"
            "if [ \"${CHAM3_STAGE_DRYRUN:-0}\" = \"1\" ]; then :; fi\n"
            "lb config\n"
        )
        root = tempfile.mkdtemp(prefix="cham3_guard_stub_")
        self.addCleanup(shutil.rmtree, root, True)
        os.makedirs(os.path.join(root, "build", "scripts"))
        with open(os.path.join(root, m1_support.BuildImageScript), "w", encoding="utf-8") as handle:
            handle.write(script)
        out_dir = tempfile.mkdtemp(prefix="cham3_guard_out_")
        self.addCleanup(shutil.rmtree, out_dir, True)
        with self.assertRaises(AssertionError) as raised:
            m1_support.run_build_image_dryrun(root, out_dir, None)
        self.assertIn("invoked a live-build step", str(raised.exception))
        self.assertIn("lb config", str(raised.exception))

    def test_guard_accepts_a_script_that_reads_the_switch_first(self):
        self.assertIsNone(m1_support.dryrun_guard_problem(
            "#!/bin/bash\nif [ \"${CHAM3_STAGE_DRYRUN:-0}\" = \"1\" ]; then exit 0; fi\n"
            "[ \"$(id -u)\" -eq 0 ] || exit 1\n"))

    def test_real_build_script_passes_the_dryrun_guard(self):
        # Contract: the coordinator's build_image.sh reads the switch before any root check.
        text = m1_support.read_repo(m1_support.BuildImageScript)
        self.assertIsNone(m1_support.dryrun_guard_problem(text))


if __name__ == "__main__":
    unittest.main()
