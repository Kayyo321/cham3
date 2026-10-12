"""Checks on the built M1 ISO's installer payload and the image's capture policy (F7, F3).

F7 (Codex re-review of 0fec3a0): live-build's installer stage (--firmware-binary true)
selects its own firmware packages from the configured archive areas and puts them in
the ISO pool, linked under /firmware for the installer's hardware detection. The
explicit live-root firmware list does not limit that selection. This suite reads the
built ISO with xorriso (read-only, nothing mounted) and requires:
  - no firmware-b43-installer or firmware-b43legacy-installer anywhere in the ISO pool,
    under /firmware or in /firmware/Contents-firmware;
  - every reviewed self-contained firmware package (42 names, including
    firmware-brcm80211) present in the pool and linked under /firmware (no firmware is
    silently dropped);
  - the live root file system manifest holding the same 42 names and no b43 installer.

F3: inside the live root file system, /usr/bin/dumpcap carries no set-id bit and is not
executable by others, the dpkg-statoverride root root 750 is registered, and the
wireshark-common debconf answer install-setuid is false. File capabilities are not
visible in a squashfs listing; they are checked on the booted live and installed
systems (build/vm/run_m1_vm_checks.py).

Real-ISO tests need out/image/cham3-m1-desktop-amd64.iso and its .packages manifest
(missing artifacts FAIL). Fixture controls run without an ISO.
"""

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

PoolRoot = "/pool"
FirmwareRoot = "/firmware"
ContentsFirmware = FirmwareRoot + "/Contents-firmware"
SquashfsMember = "/live/filesystem.squashfs"
DumpcapMember = "usr/bin/dumpcap"
QuotedPath = re.compile(r"'([^']+)'")
DebName = re.compile(r"([^/_]+)_[^/_]+_[^/_]+\.deb$")


def iso_path():
    path = m1_support.image_iso_path()
    m0_support.require_file(path, "run build/scripts/build_image.sh with the M1 profile first")
    return path


def iso_find(iso, root):
    """Paths under root in the ISO (xorriso -find); an absent root gives an empty list."""
    completed = m0_support.run_tool(["xorriso", "-indev", iso, "-find", root])
    if completed.returncode != 0:
        if "not found" in (completed.stderr + completed.stdout).lower():
            return []
        raise AssertionError("xorriso -find %s failed: %s" % (root, completed.stderr.strip()))
    return QuotedPath.findall(completed.stdout)


def deb_names(paths):
    names = set()
    for path in paths:
        match = DebName.search(path)
        if match:
            names.add(match.group(1))
    return names


def iso_extract(iso, member, target):
    completed = m0_support.run_tool(["xorriso", "-osirrox", "on", "-indev", iso, "-extract", member, target])
    if completed.returncode != 0 or not os.path.exists(target):
        raise AssertionError("could not extract %s: %s" % (member, completed.stderr.strip()))
    return target


def payload_problems(pool_names, firmware_names, contents_text):
    """Problems with the installer payload (empty means accepted)."""
    problems = []
    for name in m1_support.FirmwareInstallerPackages:
        if name in pool_names:
            problems.append("install-time fetch package in the ISO pool: " + name)
        if name in firmware_names:
            problems.append("install-time fetch package linked under /firmware: " + name)
        if re.search(r"(?m)\b%s_" % re.escape(name), contents_text):
            problems.append("install-time fetch package in Contents-firmware: " + name)
    for name in sorted(n for n in pool_names | firmware_names if n.endswith("-installer")
                       and n not in m1_support.FirmwareInstallerPackages):
        problems.append("other *-installer firmware package in the payload: " + name)
    for name in m1_support.FirmwareSelfContainedSet:
        if name not in pool_names:
            problems.append("reviewed firmware package missing from the ISO pool: " + name)
        if name not in firmware_names:
            problems.append("reviewed firmware package not linked under /firmware: " + name)
    return problems


def manifest_firmware_problems(names):
    problems = ["install-time fetch package installed in the live root: " + name
                for name in m1_support.FirmwareInstallerPackages if name in names]
    problems += ["reviewed firmware package missing from the live root: " + name
                 for name in m1_support.FirmwareSelfContainedSet if name not in names]
    return problems


def dumpcap_listing_problems(listing_line, group_lines):
    """Problems with the unsquashfs -lln line of usr/bin/dumpcap (numeric owner/group)."""
    fields = listing_line.split()
    if len(fields) < 2 or not fields[0].startswith("-"):
        return ["dumpcap is not a regular file in the image: %r" % listing_line]
    permissions, owner = fields[0], fields[1]
    problems = []
    if "s" in permissions[1:7].lower():
        problems.append("dumpcap has a set-id bit in the image: " + permissions)
    if permissions[7:10] != "---":
        problems.append("dumpcap is accessible to others in the image: " + permissions)
    uid, _, gid = owner.partition("/")
    wireshark = [line.split(":")[2] for line in group_lines if line.startswith(m1_support.CaptureGroup + ":")]
    if uid != "0":
        problems.append("dumpcap is not owned by root in the image: " + owner)
    if not wireshark or gid != wireshark[0]:
        problems.append("dumpcap group is not wireshark in the image: %s (wireshark gid %r)" % (owner, wireshark))
    return problems


def debconf_answer(config_text, question):
    for block in re.split(r"\n\s*\n", config_text):
        if re.search(r"(?m)^Name:\s*%s\s*$" % re.escape(question), block):
            match = re.search(r"(?m)^Value:\s*(\S*)\s*$", block)
            return match.group(1) if match else None
    return None


class image_payload_t(unittest.TestCase):
    """Real-ISO gates (heavy stage). Missing ISO or tools FAIL."""

    @classmethod
    def setUpClass(cls):
        cls.scratch = tempfile.mkdtemp(prefix="cham3_m1_payload_")

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.scratch, ignore_errors=True)

    def test_installer_payload_has_no_b43_installer_and_all_reviewed_firmware(self):
        iso = iso_path()
        pool = deb_names(iso_find(iso, PoolRoot))
        firmware_paths = iso_find(iso, FirmwareRoot)
        firmware = deb_names(firmware_paths)
        contents = ""
        if ContentsFirmware in firmware_paths:
            target = iso_extract(iso, ContentsFirmware, os.path.join(self.scratch, "Contents-firmware"))
            with open(target, "r", encoding="utf-8", errors="replace") as handle:
                contents = handle.read()
        self.assertTrue(pool, "the ISO has no package pool")
        self.assertEqual(payload_problems(pool, firmware, contents), [])

    def test_installer_has_the_network_mirror_udeb(self):
        # live-build excludes apt-mirror-setup by default; without it installed systems get no
        # Debian sources. auto/build adds it as a local udeb.
        udebs = set()
        for path in iso_find(iso_path(), "/pool-udeb"):
            match = re.search(r"([^/_]+)_[^/_]+_[^/_]+\.udeb$", path)
            if match:
                udebs.add(match.group(1))
        self.assertIn("apt-mirror-setup", udebs, sorted(udebs)[:20])
        self.assertIn("choose-mirror-bin", udebs)

    def test_live_root_manifest_firmware(self):
        path = m1_support.image_manifest_path()
        m0_support.require_file(path, "build_image.sh M1 output")
        with open(path, "r", encoding="utf-8", errors="replace") as handle:
            names = m1_support.manifest_package_names(handle.read())
        self.assertEqual(manifest_firmware_problems(names), [])

    def test_live_root_has_the_live_account_tooling(self):
        # live-config creates the live account only when user-setup is installed.
        path = m1_support.image_manifest_path()
        m0_support.require_file(path, "build_image.sh M1 output")
        with open(path, "r", encoding="utf-8", errors="replace") as handle:
            names = m1_support.manifest_package_names(handle.read())
        self.assertTrue({"user-setup", "live-config", "sudo"} <= names,
                        sorted({"user-setup", "live-config", "sudo"} - names))

    def squashfs(self):
        target = os.path.join(self.scratch, "filesystem.squashfs")
        if not os.path.exists(target):
            iso_extract(iso_path(), SquashfsMember, target)
        return target

    def member(self, name):
        completed = m0_support.run_tool(["unsquashfs", "-cat", self.squashfs(), name])
        if completed.returncode != 0:
            raise AssertionError("cannot read %s from the image: %s" % (name, completed.stderr.strip()))
        return completed.stdout

    def test_image_dumpcap_has_no_set_id_and_is_group_restricted(self):
        completed = m0_support.run_tool(["unsquashfs", "-lln", self.squashfs(), DumpcapMember])
        lines = [line for line in completed.stdout.splitlines() if line.rstrip().endswith("/" + DumpcapMember)]
        self.assertEqual(len(lines), 1, completed.stdout[-400:])
        groups = self.member("etc/group").splitlines()
        self.assertEqual(dumpcap_listing_problems(lines[0], groups), [])

    def test_image_registers_the_dumpcap_statoverride(self):
        self.assertIn("root root 750 /usr/bin/dumpcap", self.member("var/lib/dpkg/statoverride").splitlines())

    def test_image_preseeds_install_setuid_false(self):
        answer = debconf_answer(self.member("var/cache/debconf/config.dat"), "wireshark-common/install-setuid")
        self.assertEqual(answer, "false")


class image_payload_fixture_t(unittest.TestCase):
    """Positive and negative controls for the payload and image-policy checkers."""

    def good(self):
        names = set(m1_support.FirmwareSelfContainedSet) | {"grub-pc", "shim-signed"}
        return names, set(m1_support.FirmwareSelfContainedSet)

    def test_positive_control_accepted(self):
        pool, firmware = self.good()
        self.assertEqual(payload_problems(pool, firmware, "/lib/firmware/brcm/x firmware-brcm80211_1_all.deb\n"), [])

    def test_b43_installer_in_pool_is_rejected(self):
        # Negative control: the payload Codex observed at 0fec3a0.
        pool, firmware = self.good()
        problems = payload_problems(pool | set(m1_support.FirmwareInstallerPackages),
                                    firmware | {"firmware-b43-installer"}, "")
        self.assertTrue(any("ISO pool: firmware-b43legacy-installer" in p for p in problems), problems)
        self.assertTrue(any("under /firmware: firmware-b43-installer" in p for p in problems), problems)

    def test_b43_in_contents_firmware_is_rejected(self):
        pool, firmware = self.good()
        problems = payload_problems(pool, firmware, "/lib/firmware/b43/x firmware-b43-installer_1_all.deb\n")
        self.assertTrue(any("Contents-firmware" in p for p in problems), problems)

    def test_dropped_reviewed_firmware_is_rejected(self):
        pool, firmware = self.good()
        problems = payload_problems(pool - {"firmware-brcm80211"}, firmware - {"firmware-brcm80211"}, "")
        self.assertTrue(any("firmware-brcm80211" in p for p in problems), problems)

    def test_deb_names_parse_pool_paths(self):
        paths = ["/pool/non-free-firmware/f/firmware-brcm80211/firmware-brcm80211_20250410-2_all.deb",
                 "/firmware/firmware-iwlwifi_20250410-2_all.deb", "/pool/main/g/grub2/"]
        self.assertEqual(deb_names(paths), {"firmware-brcm80211", "firmware-iwlwifi"})

    def test_manifest_with_b43_is_rejected(self):
        problems = manifest_firmware_problems(set(m1_support.FirmwareSelfContainedSet) | {"firmware-b43-installer"})
        self.assertEqual(problems, ["install-time fetch package installed in the live root: firmware-b43-installer"])

    def test_dumpcap_listing_controls(self):
        groups = ["root:x:0:", "wireshark:x:117:"]
        good = "-rwxr-x--- 0/117 1234 2026-10-12 00:00 squashfs-root/usr/bin/dumpcap"
        self.assertEqual(dumpcap_listing_problems(good, groups), [])
        setuid = good.replace("-rwxr-x---", "-rwsr-xr--")
        problems = dumpcap_listing_problems(setuid, groups)
        self.assertTrue(any("set-id" in p for p in problems) and any("others" in p for p in problems), problems)
        self.assertTrue(dumpcap_listing_problems(good.replace("0/117", "0/0"), groups))

    def test_debconf_answer_parsing(self):
        text = ("Name: wireshark-common/install-setuid\nTemplate: wireshark-common/install-setuid\n"
                "Value: false\nOwners: wireshark-common\n\nName: other/q\nValue: true\n")
        self.assertEqual(debconf_answer(text, "wireshark-common/install-setuid"), "false")
        self.assertIsNone(debconf_answer(text, "absent/q"))


if __name__ == "__main__":
    unittest.main()
