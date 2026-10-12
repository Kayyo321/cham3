"""Behavior checks for the shared build-script extensions (M1 contract, section 'Shared build-script extensions').

These run the REAL build/scripts/build_packages.sh and build/scripts/build_image.sh
in disposable directories with host tools replaced by stubs (dpkg-buildpackage,
lintian, lb, id, dpkg, dpkg-query, realpath). Stubs record what the script
staged or configured. This is a mocked host-tool check: it proves the script's
logic (staging, path refusal, profile selection, M0 defaults). It is NOT a
package build, an ISO build, or live-build evidence. Those are VM/container stage.

Nothing is written under the repository: every output directory is a temp dir.
"""

import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402
import m1_support  # noqa: E402

PackagesScript = "build/scripts/build_packages.sh"
ImageScript = "build/scripts/build_image.sh"
StagingPackage = "fixture_desktop"
PlainPackage = "fixture_base"
DpkgStub = """#!/bin/sh
# Mock dpkg-buildpackage: record the staged build directory, then emit placeholder outputs.
name=$(basename "$(pwd)")
log="$STUB_LOG_DIR/$name.tree"
find . -type f | sort > "$log"
printf 'mock deb\\n' > "../${name}_0.0.0_all.deb"
printf 'mock buildinfo\\n' > "../${name}_0.0.0_all.buildinfo"
printf 'mock changes\\n' > "../${name}_0.0.0_all.changes"
exit 0
"""
LintianStub = "#!/bin/sh\nexit 0\n"
Sha256Wrapper = "#!/bin/sh\nexec shasum -a 256 \"$@\"\n"
IdStub = "#!/bin/sh\nif [ \"$1\" = \"-u\" ]; then echo 0; else exec /usr/bin/id \"$@\"; fi\n"
DpkgArchStub = "#!/bin/sh\nif [ \"$1\" = \"--print-architecture\" ]; then echo amd64; exit 0; fi\nexit 1\n"
DpkgQueryStub = "#!/bin/sh\necho 1.0\nexit 0\n"
DebootstrapStub = "#!/bin/sh\nexit 0\n"
RealpathStub = (
    "#!/bin/sh\n"
    "last=''\n"
    "for arg in \"$@\"; do case \"$arg\" in -*) ;; *) last=\"$arg\" ;; esac; done\n"
    "exec python3 -c 'import os,sys; print(os.path.realpath(sys.argv[1]))' \"$last\"\n"
)
LbStub = """#!/bin/sh
# Mock live-build: 'config' records the auto/config it was given; 'build' writes one ISO and one manifest.
case "$1" in
  config) cp auto/config "$STUB_LOG_DIR/lb_auto_config.txt" ;;
  build) printf 'mock iso\\n' > cham3-stub.iso; printf 'pkg\\t1.0\\n' > cham3-stub.packages ;;
  *) exit 1 ;;
esac
exit 0
"""


def write_executable(path, text):
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(text)
    os.chmod(path, os.stat(path).st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)


def make_stub_bin(bin_dir, names_to_stubs):
    os.makedirs(bin_dir, exist_ok=True)
    for name, text in names_to_stubs.items():
        write_executable(os.path.join(bin_dir, name), text)


def base_stubs(include_sha256_wrapper):
    stubs = {
        "dpkg-buildpackage": DpkgStub,
        "lintian": LintianStub,
        "lb": LbStub,
        "id": IdStub,
        "dpkg": DpkgArchStub,
        "dpkg-query": DpkgQueryStub,
        "debootstrap": DebootstrapStub,
        "realpath": RealpathStub,
    }
    if include_sha256_wrapper:
        stubs["sha256sum"] = Sha256Wrapper
    return stubs


def assert_stubs_resolve_first(bin_dir, path_value):
    """Refuse to run unless every stub in bin_dir is what PATH resolves to (never a real lb or debootstrap)."""
    for name in sorted(os.listdir(bin_dir)):
        resolved = shutil.which(name, path=path_value)
        if resolved != os.path.join(bin_dir, name):
            raise AssertionError("refusing to run: %s would resolve to %s, not the stub" % (name, resolved))


def run_script(script_rel, repo, env_extra, bin_dir, cwd=None):
    env = {key: value for key, value in os.environ.items() if not key.startswith("CHAM3_")}
    env["PATH"] = bin_dir + os.pathsep + os.environ.get("PATH", "/usr/bin:/bin")
    assert_stubs_resolve_first(bin_dir, env["PATH"])
    env.update(env_extra)
    return subprocess.run(
        ["/bin/bash", os.path.join(repo, script_rel)],
        cwd=cwd or repo,
        env=env,
        capture_output=True,
        text=True,
        check=False,
        timeout=180,
    )


def sha256_needs_wrapper():
    return shutil.which("sha256sum") is None


def mock_packaging_root(temp, extra_sources_lines):
    """Build a temp repo tree holding the real build_packages.sh and two fixture packages."""
    root = os.path.join(temp, "root")
    scripts = os.path.join(root, "build", "scripts")
    os.makedirs(scripts)
    shutil.copy(os.path.join(m0_support.repo_root(), PackagesScript), os.path.join(scripts, "build_packages.sh"))
    os.chmod(os.path.join(scripts, "build_packages.sh"), 0o755)
    staging_debian = os.path.join(root, "packaging", StagingPackage, "debian")
    os.makedirs(staging_debian)
    with open(os.path.join(staging_debian, "cham3-extra-sources"), "w", encoding="utf-8") as handle:
        handle.write(extra_sources_lines)
    for source in ("desktop", "assets"):
        os.makedirs(os.path.join(root, source))
        with open(os.path.join(root, source, "marker.txt"), "w", encoding="utf-8") as handle:
            handle.write(source + "\n")
    plain_debian = os.path.join(root, "packaging", PlainPackage, "debian")
    os.makedirs(plain_debian)
    return root


def run_packages(root, temp):
    out_dir = os.path.join(temp, "out")
    log_dir = os.path.join(temp, "stublog")
    bin_dir = os.path.join(temp, "bin")
    os.makedirs(log_dir)
    make_stub_bin(bin_dir, base_stubs(sha256_needs_wrapper()))
    completed = run_script(
        PackagesScript,
        root,
        {"CHAM3_OUT_DIR": out_dir, "STUB_LOG_DIR": log_dir},
        bin_dir,
    )
    return completed, out_dir, log_dir


def tree_listing(log_dir, package):
    path = os.path.join(log_dir, package + ".tree")
    if not os.path.isfile(path):
        return None
    with open(path, "r", encoding="utf-8") as handle:
        return handle.read()


def staged_package_names(work_dir):
    """Sorted Package fields of the .deb files in <work>/config/packages.chroot."""
    chroot = os.path.join(work_dir, "config", "packages.chroot")
    if not os.path.isdir(chroot):
        raise AssertionError("live-build work tree has no config/packages.chroot: " + work_dir)
    names = []
    for filename in sorted(os.listdir(chroot)):
        if not filename.endswith(".deb"):
            continue
        completed = m0_support.run_tool(["dpkg-deb", "-f", os.path.join(chroot, filename), "Package"])
        if completed.returncode != 0:
            raise AssertionError("staged file is not a readable deb: " + filename)
        names.append(completed.stdout.strip())
    return sorted(names)


class build_packages_static_t(unittest.TestCase):
    def script_text(self):
        return m1_support.read_repo(PackagesScript)

    def test_extra_sources_mechanism_present(self):
        self.assertIn("cham3-extra-sources", self.script_text())

    def test_m0_safety_behavior_preserved(self):
        text = self.script_text()
        for needle in ("set -eu -o pipefail", "dpkg-buildpackage -us -uc -b",
                       "lintian --fail-on error", 'cp -a "${package_dir}."', "rm -rf \"${work_dir}\""):
            self.assertIn(needle, text, "M0 behavior line missing: " + needle)

    def test_extra_sources_refuses_unsafe_paths(self):
        self.assertRegex(self.script_text(), r"/\*\|\*\.\.\*")


class build_packages_behavior_t(unittest.TestCase):
    def test_staging_copies_listed_sources_and_leaves_plain_package_alone(self):
        with tempfile.TemporaryDirectory() as temp:
            root = mock_packaging_root(temp, "# desktop tree\ndesktop\nassets\n")
            completed, out_dir, log_dir = run_packages(root, temp)
            self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)
            staged = tree_listing(log_dir, StagingPackage)
            self.assertIsNotNone(staged, "staging package was not built")
            self.assertIn("./desktop/marker.txt", staged)
            self.assertIn("./assets/marker.txt", staged)
            plain = tree_listing(log_dir, PlainPackage)
            self.assertIsNotNone(plain, "plain package was not built")
            self.assertNotIn("desktop/", plain)
            self.assertNotIn("assets/", plain)
            for name in ("%s_0.0.0_all.deb" % StagingPackage, "%s_0.0.0_all.deb" % PlainPackage):
                self.assertTrue(os.path.isfile(os.path.join(out_dir, "packages", name)), name)

    def test_absolute_path_in_extra_sources_is_refused(self):
        with tempfile.TemporaryDirectory() as temp:
            root = mock_packaging_root(temp, "desktop\nassets\n/etc\n")
            completed, _, _ = run_packages(root, temp)
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("unsafe path", completed.stderr)

    def test_parent_traversal_in_extra_sources_is_refused(self):
        with tempfile.TemporaryDirectory() as temp:
            root = mock_packaging_root(temp, "desktop\nassets\n../secret\n")
            completed, _, _ = run_packages(root, temp)
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("unsafe path", completed.stderr)

    def test_missing_extra_source_fails_closed(self):
        with tempfile.TemporaryDirectory() as temp:
            root = mock_packaging_root(temp, "desktop\nassets\ndocs\n")
            completed, _, _ = run_packages(root, temp)
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("missing extra source docs", completed.stderr)


class build_image_static_t(unittest.TestCase):
    def script_text(self):
        return m1_support.read_repo(ImageScript)

    def test_profile_defaults_are_m0(self):
        text = self.script_text()
        self.assertRegex(text, r"CHAM3_LIVE_DIR:-build/live\}")
        self.assertRegex(text, r"CHAM3_IMAGE_NAME:-cham3-m0-dev-amd64\}")

    def test_live_dir_is_refused_when_unsafe(self):
        self.assertRegex(self.script_text(), r"/\*\|\*\.\.\*\)\s*die")


class build_image_behavior_t(unittest.TestCase):
    """Run the real build_image.sh with stubbed host tools in temp output dirs."""

    def run_image(self, extra_env, temp):
        # build_image.sh reads /etc/os-release under set -e. Without that file (for
        # example on macOS) the script exits 1 silently. Report it as BLOCKED instead.
        if not os.path.isfile("/etc/os-release"):
            raise AssertionError(
                "BLOCKED (host environment): /etc/os-release is absent; run this suite "
                "in a debian:trixie container (amd64)"
            )
        repo = m0_support.repo_root()
        out_dir = os.path.join(temp, "out")
        packages_dir = os.path.join(out_dir, "packages")
        os.makedirs(packages_dir)
        # build_image.sh reads each .deb's Package field (dpkg-deb -f), so every
        # fixture must be a real archive: all three cham3 packages plus one unlisted.
        for package in m1_support.LocalPackageNames + ("cham3-fixture-unlisted",):
            m1_support.build_fixture_deb(packages_dir, package, package + "_0.0.0~fixture_all.deb")
        log_dir = os.path.join(temp, "stublog")
        os.makedirs(log_dir)
        bin_dir = os.path.join(temp, "bin")
        make_stub_bin(bin_dir, base_stubs(sha256_needs_wrapper()))
        env = {
            "CHAM3_OUT_DIR": out_dir,
            "CHAM3_WORK_DIR": os.path.join(out_dir, "live-build-work"),
            "CHAM3_ALLOW_UNVERIFIED_HOST": "1",
            "STUB_LOG_DIR": log_dir,
            # Fixed, so the run does not depend on the repository being a git checkout.
            "SOURCE_DATE_EPOCH": "1700000000",
        }
        env.update(extra_env)
        completed = run_script(ImageScript, repo, env, bin_dir)
        return completed, out_dir, log_dir

    def test_default_invocation_builds_m0_image_from_m0_config(self):
        with tempfile.TemporaryDirectory() as temp:
            completed, out_dir, log_dir = self.run_image({}, temp)
            self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)
            image = os.path.join(out_dir, "image")
            self.assertTrue(os.path.isfile(os.path.join(image, m0_support.ImageIsoName)))
            self.assertTrue(os.path.isfile(os.path.join(image, m0_support.ImageManifestName)))
            seen = open(os.path.join(log_dir, "lb_auto_config.txt"), encoding="utf-8").read()
            expected = m1_support.read_repo("build/live/auto/config")
            self.assertEqual(seen, expected, "default run did not use build/live/auto/config")
            # F1: the M0 build stages exactly cham3-base into the live-build work tree.
            staged = staged_package_names(os.path.join(out_dir, "live-build-work"))
            self.assertEqual(staged, sorted(m1_support.M0StagedPackages))

    def test_m1_profile_builds_desktop_image_from_desktop_config(self):
        with tempfile.TemporaryDirectory() as temp:
            completed, out_dir, log_dir = self.run_image(
                {"CHAM3_LIVE_DIR": "build/live-desktop", "CHAM3_IMAGE_NAME": m1_support.ImageDesktopName},
                temp,
            )
            self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)
            self.assertTrue(os.path.isfile(os.path.join(out_dir, "image", m1_support.ImageDesktopIso)))
            seen = open(os.path.join(log_dir, "lb_auto_config.txt"), encoding="utf-8").read()
            self.assertEqual(seen, m1_support.read_repo(m1_support.LiveDesktopConfig))
            # F1: the M1 desktop build stages exactly cham3-desktop and cham3-full.
            staged = staged_package_names(os.path.join(out_dir, "live-build-work"))
            self.assertEqual(staged, sorted(m1_support.M1StagedPackages))

    def test_live_dir_outside_repository_is_refused(self):
        with tempfile.TemporaryDirectory() as temp:
            completed, out_dir, _ = self.run_image({"CHAM3_LIVE_DIR": "/etc"}, temp)
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("unsafe CHAM3_LIVE_DIR", completed.stderr)
            self.assertFalse(os.path.isdir(os.path.join(out_dir, "image")), "refused run wrote an image")

    def test_live_dir_traversal_is_refused(self):
        with tempfile.TemporaryDirectory() as temp:
            completed, _, _ = self.run_image({"CHAM3_LIVE_DIR": "../outside"}, temp)
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("unsafe CHAM3_LIVE_DIR", completed.stderr)

    def test_missing_live_dir_is_refused(self):
        with tempfile.TemporaryDirectory() as temp:
            completed, _, _ = self.run_image({"CHAM3_LIVE_DIR": "build/does-not-exist"}, temp)
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("CHAM3_LIVE_DIR not found", completed.stderr)


if __name__ == "__main__":
    unittest.main()
