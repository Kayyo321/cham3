"""Checks inside the built M1 desktop ISO's root filesystem (VM/container stage).

The live root filesystem is read from the ISO with xorriso and listed or read
with unsquashfs. Both are read-only; nothing is mounted or installed on the
host. Missing ISO or tools FAIL (M0 convention). Runtime behavior (session type,
panel geometry, rendering) is NOT covered here; that is a VM boot-stage check.
"""

import atexit
import os
import re
import shutil
import sys
import tempfile
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402
import m1_support  # noqa: E402

SquashfsMember = "/live/filesystem.squashfs"
StatusMember = "var/lib/dpkg/status"
PlasmaSessionFile = "usr/share/wayland-sessions/plasmawayland.desktop"
ColorSchemeFile = "usr/share/color-schemes/Cham3Dark.colors"
LookAndFeelMetadata = "usr/share/plasma/look-and-feel/org.cham3.desktop/metadata.json"
WallpaperImage = "usr/share/wallpapers/Cham3/contents/images/cham3_minimal_charcoal.png"
BackendBinaryPattern = re.compile(r"^(usr/)?(s?bin|lib(exec)?)/cham3[-_]")
SystemdUnitPattern = re.compile(r"^(usr/)?lib/systemd/system/cham3[-_]")
KaliSourcePattern = re.compile(r"(?i)kali")
ImageCache = {}


def image_root():
    """Extract the live root squashfs once per test process; return its path."""
    if "squashfs" in ImageCache:
        return ImageCache["squashfs"]
    iso = m1_support.image_iso_path()
    m0_support.require_file(iso, "run build/scripts/build_image.sh with the M1 profile first")
    temp = tempfile.mkdtemp(prefix="cham3_m1_iso_")
    atexit.register(shutil.rmtree, temp, True)
    target = os.path.join(temp, "filesystem.squashfs")
    completed = m0_support.run_tool(
        ["xorriso", "-osirrox", "on", "-indev", iso, "-extract", SquashfsMember, target]
    )
    if completed.returncode != 0 or not os.path.isfile(target):
        raise AssertionError("could not extract %s: %s" % (SquashfsMember, completed.stderr.strip()))
    ImageCache["squashfs"] = target
    return target


def rootfs_listing():
    """Return the set of repo-independent paths in the root filesystem (no leading slash)."""
    if "listing" in ImageCache:
        return ImageCache["listing"]
    completed = m0_support.run_tool(["unsquashfs", "-l", image_root()])
    if completed.returncode != 0:
        raise AssertionError("unsquashfs -l failed: " + completed.stderr.strip())
    paths = set()
    for line in completed.stdout.splitlines():
        if "squashfs-root/" in line:
            paths.add(line.split("squashfs-root/", 1)[1].strip())
    ImageCache["listing"] = paths
    return paths


def read_member(member):
    completed = m0_support.run_tool(["unsquashfs", "-cat", image_root(), member])
    if completed.returncode != 0 or not completed.stdout:
        raise AssertionError("member not readable from image rootfs: " + member)
    return completed.stdout


def installed_package_names(status_text):
    names = set()
    for stanza in m0_support.parse_control(status_text):
        if stanza.get("Status") == "install ok installed" and "Package" in stanza:
            names.add(stanza["Package"])
    return names


def contents_problems(paths, installed, apt_source_texts):
    """Return problems for M1 image contents (empty means accepted)."""
    problems = []
    for name in (m1_support.PackageDesktop, m1_support.PackageFull):
        if name not in installed:
            problems.append("metapackage not installed in image: " + name)
    for component in m1_support.Cham3PlasmoidComponents:
        plugin_id = m1_support.Cham3Namespace + component
        meta = "usr/share/plasma/plasmoids/%s/metadata.json" % plugin_id
        if meta not in paths:
            problems.append("plasmoid not installed: " + meta)
    for required in (PlasmaSessionFile, ColorSchemeFile, LookAndFeelMetadata, WallpaperImage):
        if required not in paths:
            problems.append("required file missing from image: " + required)
    for path in sorted(paths):
        if BackendBinaryPattern.match(path):
            problems.append("absent backend binary shipped in image: " + path)
        if SystemdUnitPattern.match(path):
            problems.append("cham3 systemd unit shipped in image: " + path)
    for path, text in apt_source_texts.items():
        if KaliSourcePattern.search(text):
            problems.append("Kali apt source enabled in image: " + path)
    return problems


def apt_source_texts(paths):
    texts = {}
    for path in sorted(paths):
        if path.startswith(("etc/apt/sources.list", "etc/apt/sources.list.d/")) and path.endswith(
            (".list", ".sources")
        ):
            texts[path] = read_member(path)
    return texts


class image_contents_t(unittest.TestCase):
    def test_contents_match_m1_contract(self):
        paths = rootfs_listing()
        installed = installed_package_names(read_member(StatusMember))
        self.assertEqual(contents_problems(paths, installed, apt_source_texts(paths)), [])


class image_contents_fixture_t(unittest.TestCase):
    """Positive and negative controls for contents_problems()."""

    def clean_paths(self):
        paths = {PlasmaSessionFile, ColorSchemeFile, LookAndFeelMetadata, WallpaperImage}
        for component in m1_support.Cham3PlasmoidComponents:
            paths.add("usr/share/plasma/plasmoids/%s%s/metadata.json" % (m1_support.Cham3Namespace, component))
        return paths

    def clean_installed(self):
        return {m1_support.PackageDesktop, m1_support.PackageFull}

    def test_positive_control_accepted(self):
        self.assertEqual(contents_problems(self.clean_paths(), self.clean_installed(), {}), [])

    def test_shipped_backend_binary_is_caught(self):
        paths = self.clean_paths() | {"usr/bin/cham3-netd"}
        problems = contents_problems(paths, self.clean_installed(), {})
        self.assertTrue(any("absent backend binary" in p for p in problems), problems)

    def test_shipped_systemd_unit_is_caught(self):
        paths = self.clean_paths() | {"usr/lib/systemd/system/cham3-vaultd.service"}
        problems = contents_problems(paths, self.clean_installed(), {})
        self.assertTrue(any("systemd unit" in p for p in problems), problems)

    def test_kali_apt_source_is_caught(self):
        sources = {"etc/apt/sources.list.d/kali.sources": "URIs: http://http.kali.org/kali\n"}
        problems = contents_problems(self.clean_paths(), self.clean_installed(), sources)
        self.assertTrue(any("Kali apt source" in p for p in problems), problems)

    def test_missing_plasmoid_is_caught(self):
        paths = self.clean_paths() - {"usr/share/plasma/plasmoids/org.cham3.killswitch/metadata.json"}
        problems = contents_problems(paths, self.clean_installed(), {})
        self.assertTrue(any("org.cham3.killswitch" in p for p in problems), problems)

    def test_missing_metapackage_is_caught(self):
        problems = contents_problems(self.clean_paths(), {m1_support.PackageDesktop}, {})
        self.assertTrue(any("cham3-full" in p for p in problems), problems)


if __name__ == "__main__":
    unittest.main()
