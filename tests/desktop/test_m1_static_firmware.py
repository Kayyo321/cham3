"""Static checks for the M1 firmware package list (research firmware blocker, F2 follow-up).

live-build with --firmware-chroot true schedules firmware packages on its own, including
firmware-b43-installer and firmware-b43legacy-installer. Their postinsts download fixed
URLs (GitHub, downloads.openwrt.org) and verify an embedded SHA-512, so they fail in any
environment without those hosts. The owner decision sets --firmware-chroot false, and the
image installs the explicit self-contained list in
build/live-desktop/config/package-lists/cham3_firmware.list.chroot instead.

This test checks the list: it must be exactly the self-contained set, and it must not
contain the b43 installers or any other *-installer package. The --firmware-chroot false
setting itself is checked by test_m1_static_live_desktop_config.py.
"""

import os
import sys
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m1_support  # noqa: E402


def firmware_list_problems(text):
    """Problems with a firmware package list (empty means it is exactly the self-contained set)."""
    names = m1_support.local_manifest_names(text)
    problems = []
    if len(set(names)) != len(names):
        problems.append("duplicate names in the firmware list")
    expected = set(m1_support.FirmwareSelfContainedSet)
    for name in sorted(set(names)):
        if name in m1_support.FirmwareInstallerPackages or name.endswith("-installer"):
            problems.append("install-time fetch package present: " + name)
        elif name not in expected:
            problems.append("package outside the self-contained firmware set: " + name)
    for name in sorted(expected - set(names)):
        problems.append("self-contained firmware package missing: " + name)
    return problems


class firmware_list_real_t(unittest.TestCase):
    def test_firmware_list_is_exactly_the_self_contained_set(self):
        text = m1_support.read_repo(m1_support.FirmwareListPath)
        self.assertEqual(firmware_list_problems(text), [])

    def test_no_b43_installer_is_listed(self):
        names = set(m1_support.local_manifest_names(m1_support.read_repo(m1_support.FirmwareListPath)))
        for name in m1_support.FirmwareInstallerPackages:
            self.assertNotIn(name, names, "install-time fetch package listed: " + name)


class firmware_list_fixture_t(unittest.TestCase):
    """Positive and negative controls for firmware_list_problems()."""

    def good(self):
        return "# explicit firmware set (M1)\n" + "\n".join(m1_support.FirmwareSelfContainedSet) + "\n"

    def test_positive_control_accepted(self):
        self.assertEqual(firmware_list_problems(self.good()), [])

    def test_b43_installer_is_rejected(self):
        problems = firmware_list_problems(self.good() + "firmware-b43-installer\n")
        self.assertTrue(any("firmware-b43-installer" in p for p in problems), problems)

    def test_b43legacy_installer_is_rejected(self):
        problems = firmware_list_problems(self.good() + "firmware-b43legacy-installer\n")
        self.assertTrue(any("firmware-b43legacy-installer" in p for p in problems), problems)

    def test_any_other_installer_package_is_rejected(self):
        problems = firmware_list_problems(self.good() + "firmware-foo-installer\n")
        self.assertTrue(any("install-time fetch package" in p for p in problems), problems)

    def test_package_outside_the_set_is_rejected(self):
        problems = firmware_list_problems(self.good() + "firmware-unlisted-extra\n")
        self.assertTrue(any("outside the self-contained" in p for p in problems), problems)

    def test_missing_self_contained_package_is_rejected(self):
        text = "\n".join(name for name in m1_support.FirmwareSelfContainedSet if name != "wireless-regdb")
        problems = firmware_list_problems(text)
        self.assertTrue(any("wireless-regdb" in p for p in problems), problems)

    def test_duplicate_name_is_rejected(self):
        problems = firmware_list_problems(self.good() + "firmware-linux\n")
        self.assertTrue(any("duplicate" in p for p in problems), problems)


if __name__ == "__main__":
    unittest.main()
