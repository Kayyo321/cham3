"""Checks for build/host_packages.txt, the single source of build-host packages."""

import os
import re
import sys
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402

HostPackagesPath = "build/host_packages.txt"
RequiredHostPackages = (
    "git",
    "live-build",
    "debootstrap",
    "squashfs-tools",
    "xorriso",
    "qemu-system-x86",
    "shellcheck",
    "yamllint",
    "python3",
    "dpkg-dev",
    "debhelper",
    "lintian",
)
DebianPackageName = re.compile(r"^[a-z0-9][a-z0-9+.\-]+$")


def host_package_entries():
    entries = []
    for raw in m0_support.read_text(HostPackagesPath).splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        entries.append(line)
    return entries


class host_packages_t(unittest.TestCase):
    def test_each_entry_is_one_debian_package_name(self):
        for entry in host_package_entries():
            self.assertRegex(entry, DebianPackageName, "invalid package entry: %r" % entry)

    def test_no_duplicate_entries(self):
        entries = host_package_entries()
        self.assertEqual(len(entries), len(set(entries)), "duplicate package entries")

    def test_required_build_host_packages_listed(self):
        entries = set(host_package_entries())
        missing = [name for name in RequiredHostPackages if name not in entries]
        self.assertEqual(missing, [], "host_packages.txt lacks required packages")


if __name__ == "__main__":
    unittest.main()
