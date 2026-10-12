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

Installer payload (Codex F7). --firmware-binary true makes live-build's installer stage
run its own selection (Firmware_List_From_Contents in live-build 20250505+deb13u1): every
package of the configured --archive-areas whose Contents index lists a path under
(usr/)lib/firmware, minus a fixed blocklist, plus firmware-linux. That selection is not
limited by the explicit list. This test replays the selection over the Debian 13 Contents
rows recorded on 2026-10-11 (docs/research/evidence/firmware/
live_build_contents_selection_raw.tsv.gz) with the archive areas of auto/config: it must
yield exactly the self-contained set, without the b43 installers. The negative control
replays it with contrib added and finds both installers. This is a replay of recorded
index data; the built ISO pool and /firmware are checked by tests/run_tests.sh image-m1.
"""

import gzip
import os
import re
import shlex
import sys
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402
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


ContentsSelectionPath = "docs/research/evidence/firmware/live_build_contents_selection_raw.tsv.gz"
# live-build 20250505+deb13u1, functions/firmwarelists.sh: always-blocked names, and the
# ARM-only names blocked when no ARM architecture is built (amd64 only here).
InstallerBlocklist = ("firmware-nvidia-gsp", "firmware-nvidia-tesla-gsp")
InstallerArmBlocklist = ("arm-trusted-firmware-tools", "crust-firmware", "firmware-qcom-soc", "firmware-samsung",
                         "firmware-ti-connectivity", "raspi-firmware")
InstallerAddedPackage = "firmware-linux"
FirmwarePathPattern = re.compile(r"^(usr/)?lib/firmware")


def contents_rows():
    """(archive area, [package ...], path) rows of the recorded Debian 13 Contents selection."""
    path = os.path.join(m0_support.repo_root(), ContentsSelectionPath)
    if not os.path.isfile(path):
        raise AssertionError("recorded Contents selection missing: " + ContentsSelectionPath)
    rows = []
    with gzip.open(path, "rt", encoding="utf-8") as handle:
        for line in handle:
            fields = line.rstrip("\n").split("\t")
            if len(fields) != 3:
                raise AssertionError("malformed Contents selection row: %r" % line)
            packages = [entry.rsplit("/", 1)[-1] for entry in fields[1].split(",") if entry]
            rows.append((fields[0], packages, fields[2]))
    return rows


def installer_firmware_selection(archive_areas, rows):
    """Names live-build's installer stage selects for amd64 from these archive areas."""
    selected = {InstallerAddedPackage}
    for area, packages, path in rows:
        if area in archive_areas and FirmwarePathPattern.match(path):
            selected.update(packages)
    return selected - set(InstallerBlocklist) - set(InstallerArmBlocklist)


def configured_archive_areas(config_text):
    joined = re.sub(r"\\\r?\n", " ", config_text)
    kept = [line for line in joined.splitlines() if not line.lstrip().startswith("#")]
    tokens = shlex.split("\n".join(kept), comments=True)
    values = [tokens[index + 1] for index, token in enumerate(tokens[:-1]) if token == "--archive-areas"]
    if len(values) != 1:
        raise AssertionError("auto/config must set --archive-areas exactly once, found %r" % values)
    return set(values[0].split())


def installer_selection_problems(selected):
    problems = ["installer payload selects an install-time fetch package: " + name
                for name in sorted(selected) if name in m1_support.FirmwareInstallerPackages
                or name.endswith("-installer")]
    expected = set(m1_support.FirmwareSelfContainedSet)
    problems += ["installer payload selects a package outside the reviewed set: " + name
                 for name in sorted(selected - expected) if not name.endswith("-installer")]
    problems += ["installer payload drops reviewed firmware: " + name for name in sorted(expected - selected)]
    return problems


class firmware_list_real_t(unittest.TestCase):
    def test_firmware_list_is_exactly_the_self_contained_set(self):
        text = m1_support.read_repo(m1_support.FirmwareListPath)
        self.assertEqual(firmware_list_problems(text), [])

    def test_no_b43_installer_is_listed(self):
        names = set(m1_support.local_manifest_names(m1_support.read_repo(m1_support.FirmwareListPath)))
        for name in m1_support.FirmwareInstallerPackages:
            self.assertNotIn(name, names, "install-time fetch package listed: " + name)


class installer_firmware_payload_t(unittest.TestCase):
    """Replay of live-build's installer firmware selection (F7) with the configured areas."""

    def test_configured_areas_select_exactly_the_reviewed_set(self):
        areas = configured_archive_areas(m1_support.read_repo(m1_support.LiveDesktopConfig))
        self.assertEqual(installer_selection_problems(installer_firmware_selection(areas, contents_rows())), [])

    def test_configured_areas_keep_brcm80211(self):
        areas = configured_archive_areas(m1_support.read_repo(m1_support.LiveDesktopConfig))
        self.assertIn("firmware-brcm80211", installer_firmware_selection(areas, contents_rows()))

    def test_control_contrib_area_selects_both_b43_installers(self):
        # Sensitivity control: the 0fec3a0 areas (with contrib) give the 44 names Codex observed.
        selected = installer_firmware_selection({"main", "contrib", "non-free", "non-free-firmware"},
                                                contents_rows())
        self.assertEqual(len(selected), 44, sorted(selected))
        problems = installer_selection_problems(selected)
        for name in m1_support.FirmwareInstallerPackages:
            self.assertTrue(any(name in p for p in problems), problems)

    def test_recorded_contrib_rows_hold_only_the_b43_installers(self):
        names = {name for area, packages, _ in contents_rows() if area == "contrib" for name in packages}
        self.assertEqual(names, set(m1_support.FirmwareInstallerPackages))

    def test_dropping_an_area_with_reviewed_firmware_is_reported(self):
        selected = installer_firmware_selection({"main", "non-free"}, contents_rows())
        self.assertTrue(any("drops reviewed firmware: firmware-brcm80211" in p
                            for p in installer_selection_problems(selected)))


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
