"""Checks inside the built ISO: installed cham3-base and no desktop/Tor packages.

The root filesystem is read from the ISO with xorriso and inspected with
unsquashfs. Both are read-only operations on the image; nothing is mounted
and nothing is installed on the host.
"""

import atexit
import os
import shutil
import sys
import tempfile
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402

SquashfsMember = "/live/filesystem.squashfs"
StatusMember = "var/lib/dpkg/status"
DocDir = "usr/share/doc/cham3-base"
OutOfScopeStems = ("kde", "plasma", "kwin", "sddm", "tor", "torsocks", "cham3-core",
                   "cham3-sandbox", "cham3-networking", "cham3-desktop",
                   "cham3-security", "cham3-full")
SquashfsCache = {}


def stanzas_from_status(text):
    stanzas = []
    current = {}
    for line in text.splitlines():
        if not line.strip():
            if current:
                stanzas.append(current)
            current = {}
            continue
        if line[0] in " \t" or ":" not in line:
            continue
        key, value = line.split(":", 1)
        current[key.strip()] = value.strip()
    if current:
        stanzas.append(current)
    return stanzas


def iso_file():
    path = os.path.join(m0_support.out_dir(), "image", m0_support.ImageIsoName)
    m0_support.require_file(path, "run build/scripts/build_image.sh as root")
    return path


def extracted_squashfs():
    """Extract the live root filesystem once per test process."""
    if "path" in SquashfsCache:
        return SquashfsCache["path"]
    temp = tempfile.mkdtemp(prefix="cham3_m0_iso_")
    atexit.register(shutil.rmtree, temp, True)
    target = os.path.join(temp, "filesystem.squashfs")
    completed = m0_support.run_tool(
        ["xorriso", "-osirrox", "on", "-indev", iso_file(), "-extract", SquashfsMember, target]
    )
    if completed.returncode != 0 or not os.path.isfile(target):
        raise AssertionError(
            "could not extract %s from ISO: %s" % (SquashfsMember, completed.stderr.strip())
        )
    SquashfsCache["path"] = target
    return target


def read_member(member):
    completed = m0_support.run_tool(["unsquashfs", "-cat", extracted_squashfs(), member])
    if completed.returncode != 0 or not completed.stdout:
        raise AssertionError("member not readable from image rootfs: " + member)
    return completed.stdout


def installed_packages():
    stanzas = stanzas_from_status(read_member(StatusMember))
    return [s for s in stanzas if s.get("Status") == "install ok installed"]


class image_contents_t(unittest.TestCase):
    def test_cham3_base_is_installed_at_expected_version(self):
        matches = [s for s in installed_packages() if s.get("Package") == m0_support.ExpectedPackage]
        self.assertEqual(len(matches), 1, "cham3-base not installed in image rootfs")
        self.assertEqual(matches[0].get("Version"), m0_support.ExpectedVersion)

    def test_cham3_base_documentation_present_in_image(self):
        for name in ("copyright", "README.Debian"):
            read_member(DocDir + "/" + name)

    def test_no_desktop_or_tor_packages_installed(self):
        problems = []
        for stanza in installed_packages():
            name = stanza.get("Package", "")
            for stem in OutOfScopeStems:
                if name == stem or name.startswith(stem + "-") or (
                    stem in ("kde", "plasma", "kwin", "sddm") and name.startswith(stem)
                ):
                    problems.append(name)
        self.assertEqual(problems, [], "out-of-scope packages installed in image")


if __name__ == "__main__":
    unittest.main()
