"""Static checks for the M1 live-build definition (build/live-desktop/auto/config).

Mirrors the M0 live-config suite but asserts the M1 contract values: trixie amd64,
main/non-free/non-free-firmware archive areas, installable live debian-installer,
firmware-binary on and firmware-chroot off (F2: the image installs the explicit
cham3_firmware list, see test_m1_static_firmware.py), and cham3-full as the only
package-list entry. contrib (torbrowser-launcher only) must NOT be an archive area:
live-build's installer firmware selection reads the archive areas, and in Debian 13
contrib's only firmware candidates are the two b43 installers (Codex F7). It is supplied
by config/archives/cham3_debian_contrib.list from the official, signed Debian mirrors.
The wireshark-common debconf answer install-setuid must be preseeded false (Codex F3).
Fixture controls show that the checker rejects the M0 config (positive control for the
defect), contrib as an archive area, and an install-setuid true preseed.
"""

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

RequiredArchiveAreas = frozenset({"main", "non-free", "non-free-firmware"})
ContribSourceFile = m1_support.ArchivesDir + "/cham3_debian_contrib.list"
DebianKeyring = "/usr/share/keyrings/debian-archive-keyring.pgp"
ContribSuites = {
    "trixie": "http://deb.debian.org/debian/",
    "trixie-updates": "http://deb.debian.org/debian/",
    "trixie-security": "http://security.debian.org/debian-security",
}
# trusted=yes is NOT forbidden in the build configuration: live-build's own local package
# repository uses it transiently inside the build chroot. The final image is checked for it
# (test_m1_image_contents.py, Codex Remote), and the cham3 sources must not use it (policy in
# test_m1_static_foreign_sources.py).
ForbiddenAptFragments = (
    "allow-unauthenticated",
    "allowunauthenticated",
    "allowinsecurerepositories",
    "apt-secure false",
    "apt-secure=false",
)
RequiredBootParameters = ("boot=live", "console=tty0", "console=ttyS0,115200n8")
OfficialMirrorHosts = m0_support.OfficialMirrorHosts


def config_tokens(text):
    joined = re.sub(r"\\\r?\n", " ", text)
    kept = [line for line in joined.splitlines() if not line.lstrip().startswith("#")]
    return shlex.split("\n".join(kept), comments=True)


def option_values(tokens, name):
    values = []
    for index, token in enumerate(tokens):
        if token == name and index + 1 < len(tokens):
            values.append(tokens[index + 1])
        elif token.startswith(name + "="):
            values.append(token[len(name) + 1 :])
    return values


def package_list_names(text):
    names = []
    for line in text.splitlines():
        names.extend(line.split("#", 1)[0].split())
    return names


def live_config_problems(config_text, package_list_text):
    """Return problems with the M1 live-build config and package list (empty means accepted)."""
    problems = []
    lowered = config_text.lower()
    tokens = config_tokens(config_text)
    if not re.search(r"\blb\s+config\s+noauto\b", config_text):
        problems.append("lb config noauto not invoked")

    def single(name, expected):
        values = option_values(tokens, name)
        if values != [expected]:
            problems.append("%s must be exactly %r, found %r" % (name, expected, values))

    single("--distribution", "trixie")
    arch = option_values(tokens, "--architectures") + option_values(tokens, "--architecture")
    if arch != ["amd64"]:
        problems.append("architecture must be amd64, found %r" % arch)

    areas = option_values(tokens, "--archive-areas")
    if len(areas) == 1 and "contrib" in areas[0].split():
        problems.append("--archive-areas must not include contrib: the installer firmware selection would add "
                        "firmware-b43-installer and firmware-b43legacy-installer (F7)")
    if len(areas) != 1 or set(areas[0].split()) != RequiredArchiveAreas:
        problems.append("--archive-areas must be main non-free non-free-firmware, found %r" % areas)

    single("--debian-installer", "live")
    images = option_values(tokens, "--binary-image") + option_values(tokens, "--binary-images")
    if images != ["iso-hybrid"]:
        problems.append("binary image must be iso-hybrid, found %r" % images)
    bootloaders = option_values(tokens, "--bootloaders")
    if len(bootloaders) != 1 or not {"syslinux", "grub-efi"} <= set(bootloaders[0].split(",")):
        problems.append("bootloaders must include syslinux and grub-efi, found %r" % bootloaders)

    # F2 firmware decision: live-build does not install firmware packages itself
    # (--firmware-chroot false); cham3_firmware.list.chroot installs the explicit set.
    single("--firmware-chroot", "false")
    single("--firmware-binary", "true")
    for name in ("--security", "--updates", "--apt-secure"):
        single(name, "true")
    for fragment in ForbiddenAptFragments:
        if fragment in lowered:
            problems.append("forbidden apt setting present: " + fragment)

    if not option_values(tokens, "--apt-recommends"):
        problems.append("--apt-recommends must be set explicitly and documented")

    hosts = set()
    for token in tokens:
        match = re.match(r"^https?://([^/\s]+)", token)
        if match:
            hosts.add(match.group(1))
    if not hosts:
        problems.append("no mirror URLs configured")
    elif not hosts <= set(OfficialMirrorHosts):
        problems.append("non-official mirror host: " + ", ".join(sorted(hosts - set(OfficialMirrorHosts))))

    boot = option_values(tokens, "--bootappend-live")
    if len(boot) != 1:
        problems.append("--bootappend-live must be set exactly once")
    else:
        parameters = set(boot[0].split())
        for required in RequiredBootParameters:
            if required not in parameters:
                problems.append("missing boot parameter: " + required)

    iso_text = " ".join(
        value
        for name in ("--iso-application", "--iso-volume", "--iso-publisher")
        for value in option_values(tokens, name)
    ).lower()
    if "cham3" not in iso_text:
        problems.append("ISO metadata does not name cham3")

    names = package_list_names(package_list_text)
    if names != ["cham3-full"]:
        problems.append("package list must contain only cham3-full, found %r" % names)
    for name in names:
        for forbidden in (m1_support.DesktopConflictNames + m1_support.ExcludedDuplicates + m1_support.GatedTools):
            if name == forbidden:
                problems.append("package list pulls forbidden package: " + name)
    return problems


def read_live_inputs(config_path, list_path):
    return m1_support.read_repo(config_path), m1_support.read_repo(list_path)


PreseedPattern = re.compile(r"wireshark-common/install-setuid\s+boolean\s+false")
SetuidPreseedPattern = re.compile(r"wireshark-common/install-setuid\s+boolean\s+true")
GroupGrantPattern = re.compile(r"(usermod\s[^\n]*-[a-zA-Z]*G|gpasswd\s+-a|adduser\s+\S+\s+(wireshark|sudo|root))")


def live_tree_texts():
    """Text of every file under build/live-desktop (missing tree raises FAIL)."""
    root = os.path.join(m0_support.repo_root(), m1_support.LiveDesktopDir)
    if not os.path.isdir(root):
        raise AssertionError("required directory missing: " + m1_support.LiveDesktopDir)
    texts = {}
    for dirpath, _, filenames in os.walk(root):
        for filename in filenames:
            path = os.path.join(dirpath, filename)
            with open(path, "r", encoding="utf-8", errors="replace") as handle:
                texts[os.path.relpath(path, m0_support.repo_root())] = handle.read()
    return texts


def wireshark_capture_problems(tree_texts):
    """Capture must use the wireshark group via debconf, never auto-granted membership or root."""
    problems = []
    if not any(PreseedPattern.search(text) for text in tree_texts.values()):
        problems.append("no wireshark-common/install-setuid false preseed (F3: wireshark-common must not choose "
                        "the set-user-id layout during the image build)")
    for path, text in tree_texts.items():
        if SetuidPreseedPattern.search(text):
            problems.append("install-setuid true preseed (set-user-id fallback layout): " + path)
        if GroupGrantPattern.search(text):
            problems.append("build config grants a privileged group membership: " + path)
        if re.search(r"(?i)NOPASSWD", text):
            problems.append("build config grants passwordless sudo: " + path)
    return problems


def contrib_source_problems(text):
    """The contrib source must give exactly Debian's trixie, -updates and -security contrib, signed."""
    entries = []
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        match = re.match(r"^deb\s+\[([^\]]*)\]\s+(\S+)\s+(\S+)\s+(.+)$", stripped)
        if not match:
            return ["malformed or unsigned contrib source line: " + stripped]
        entries.append((match.group(1).split(), match.group(2), match.group(3), match.group(4).split()))
    problems = []
    seen = {}
    for options, uri, suite, components in entries:
        if options != ["signed-by=" + DebianKeyring]:
            problems.append("contrib source must name only signed-by=%s, found %r" % (DebianKeyring, options))
        if components != ["contrib"]:
            problems.append("contrib source must carry only contrib, found %r" % components)
        if ContribSuites.get(suite) != uri:
            problems.append("unexpected contrib source %s %s" % (uri, suite))
        seen[suite] = uri
    for suite in sorted(set(ContribSuites) - set(seen)):
        problems.append("contrib source lacks %s" % suite)
    return problems


AutoBuildPath = m1_support.LiveDesktopDir + "/auto/build"
BuildStages = ("bootstrap", "chroot", "installer", "binary", "source")
ForeignArchiveNames = ("zzz_cham3_kali.list", "zzz_cham3_librewolf.list", "cham3_kali.pref", "cham3_librewolf.pref")


def auto_build_problems(text):
    """auto/build runs the five lb build stages in order, with the pinned foreign sources and
    pins moved out of config/archives for the installer stage only (and restored)."""
    code = "\n".join(line for line in text.splitlines() if not line.lstrip().startswith("#"))
    problems = []
    stages = re.findall(r"(?m)^\s*lb\s+(\w+)\b", code)
    if tuple(stages) != BuildStages:
        problems.append("auto/build must run lb %s in this order, found %r" % (", ".join(BuildStages), stages))
    names = re.search(r'(?m)^foreign_names="([^"]*)"', code)
    if names is None or set(names.group(1).split()) != set(ForeignArchiveNames):
        problems.append("auto/build does not move exactly the four foreign source and pin files")
    installer = code.find("lb installer")
    moved = code.find('mv "config/archives/${name}" "${stash_dir}/${name}"')
    restored = code.find("restore_foreign\n", installer)
    binary = code.find("lb binary")
    if not (0 <= moved < installer < restored < binary):
        problems.append("the foreign files are not moved out before lb installer and restored before lb binary")
    if "trap restore_foreign EXIT" not in code:
        problems.append("auto/build does not restore the foreign files when the installer stage fails")
    return problems


class live_desktop_config_real_t(unittest.TestCase):
    def test_auto_build_hides_foreign_sources_only_for_the_installer_stage(self):
        self.assertEqual(auto_build_problems(m1_support.read_repo(AutoBuildPath)), [])

    def test_contrib_comes_from_the_signed_debian_contrib_source(self):
        self.assertEqual(contrib_source_problems(m1_support.read_repo(ContribSourceFile)), [])

    def test_wireshark_capture_uses_group_not_root(self):
        self.assertEqual(wireshark_capture_problems(live_tree_texts()), [])

    def test_live_desktop_config_meets_m1_contract(self):
        config, package_list = read_live_inputs(m1_support.LiveDesktopConfig, m1_support.LiveDesktopPackageList)
        self.assertEqual(live_config_problems(config, package_list), [])

    def test_live_desktop_config_is_executable_shell_with_strict_mode(self):
        text = m1_support.read_repo(m1_support.LiveDesktopConfig)
        self.assertTrue(text.startswith("#!/bin/sh") or text.startswith("#!/bin/bash"))
        self.assertRegex(text, r"(?m)^\s*set\s+-[a-zA-Z]*e")


class live_desktop_config_fixture_t(unittest.TestCase):
    def good_config(self):
        return (
            "#!/bin/sh\nset -eu\n"
            "lb config noauto \\\n"
            "    --mode debian --distribution trixie --architecture amd64 \\\n"
            "    --archive-areas \"main non-free non-free-firmware\" \\\n"
            "    --debian-installer live --binary-image iso-hybrid \\\n"
            "    --bootloaders syslinux,grub-efi --firmware-chroot false --firmware-binary true \\\n"
            "    --security true --updates true --apt-secure true --apt-recommends true \\\n"
            "    --mirror-bootstrap http://deb.debian.org/debian/ \\\n"
            "    --mirror-chroot http://deb.debian.org/debian/ \\\n"
            "    --mirror-chroot-security http://security.debian.org/ \\\n"
            "    --bootappend-live \"boot=live components console=tty0 console=ttyS0,115200n8\" \\\n"
            "    --iso-application \"cham3 M1 desktop image\" \\\n"
            "    --iso-volume \"CHAM3_M1_DESKTOP\" \"${@}\"\n"
        )

    def test_positive_control_accepted(self):
        self.assertEqual(live_config_problems(self.good_config(), "cham3-full\n"), [])

    def test_m0_config_is_rejected(self):
        m0 = m1_support.read_repo("build/live/auto/config")
        problems = live_config_problems(m0, "cham3-base\n")
        self.assertTrue(any("--debian-installer" in p for p in problems), problems)
        self.assertTrue(any("--archive-areas" in p for p in problems), problems)
        self.assertTrue(any("--firmware-binary" in p for p in problems), problems)
        self.assertTrue(any("package list must contain only cham3-full" in p for p in problems), problems)

    def test_contrib_archive_area_is_rejected(self):
        # Negative control for F7: the 0fec3a0 archive areas.
        config = self.good_config().replace('"main non-free non-free-firmware"',
                                            '"main contrib non-free non-free-firmware"')
        problems = live_config_problems(config, "cham3-full\n")
        self.assertTrue(any("must not include contrib" in p for p in problems), problems)

    def test_contrib_source_positive_control_accepted(self):
        text = "".join("deb [signed-by=%s] %s %s contrib\n" % (DebianKeyring, uri, suite)
                       for suite, uri in sorted(ContribSuites.items()))
        self.assertEqual(contrib_source_problems(text), [])

    def test_unsigned_or_extra_contrib_source_is_rejected(self):
        self.assertTrue(contrib_source_problems("deb http://deb.debian.org/debian/ trixie contrib\n"))
        text = "".join("deb [signed-by=%s] %s %s contrib\n" % (DebianKeyring, uri, suite)
                       for suite, uri in sorted(ContribSuites.items()))
        self.assertTrue(contrib_source_problems(text + "deb [signed-by=%s] http://deb.debian.org/debian/ trixie "
                                                       "contrib non-free\n" % DebianKeyring))
        self.assertTrue(contrib_source_problems(text.replace("trixie-security", "trixie-backports")))

    def test_firmware_chroot_true_is_rejected(self):
        # Negative control for F2: live-build firmware scheduling is what fetched the b43 installers.
        config = self.good_config().replace("--firmware-chroot false", "--firmware-chroot true")
        problems = live_config_problems(config, "cham3-full\n")
        self.assertTrue(any("--firmware-chroot" in p for p in problems), problems)

    def test_non_official_mirror_is_rejected(self):
        config = self.good_config().replace("http://deb.debian.org/debian/", "http://http.kali.org/kali/", 1)
        problems = live_config_problems(config, "cham3-full\n")
        self.assertTrue(any("non-official mirror" in p for p in problems), problems)

    def test_disabled_apt_authentication_is_rejected(self):
        config = self.good_config().replace("--apt-secure true", "--apt-secure false")
        problems = live_config_problems(config, "cham3-full\n")
        self.assertTrue(any("--apt-secure" in p for p in problems), problems)

    def test_package_list_with_gated_tool_is_rejected(self):
        problems = live_config_problems(self.good_config(), "cham3-full\nlibrewolf\n")
        self.assertTrue(any("librewolf" in p for p in problems), problems)


class auto_build_fixture_t(unittest.TestCase):
    def test_plain_lb_build_is_rejected(self):
        # Negative control: the 0fec3a0 auto/build, whose installer stage saw the Kali index.
        problems = auto_build_problems("#!/bin/sh\nset -eu\nlb build noauto \"${@}\"\n")
        self.assertTrue(any("order" in p for p in problems), problems)

    def test_restoring_after_binary_is_rejected(self):
        text = m1_support.read_repo(AutoBuildPath)
        broken = text.replace("lb installer \"${@}\"\nrestore_foreign\n", "lb installer \"${@}\"\n")
        self.assertNotEqual(broken, text)
        self.assertTrue(auto_build_problems(broken))


class wireshark_capture_fixture_t(unittest.TestCase):
    def test_preseed_is_accepted(self):
        tree = {"build/live-desktop/config/hooks/normal/wireshark.hook.chroot":
                "echo 'wireshark-common wireshark-common/install-setuid boolean false' | debconf-set-selections\n"}
        self.assertEqual(wireshark_capture_problems(tree), [])

    def test_setuid_true_preseed_is_reported(self):
        # Negative control for F3: the 0fec3a0 answer.
        tree = {"build/live-desktop/config/preseed/x.cfg.chroot":
                "wireshark-common wireshark-common/install-setuid boolean true\n"}
        problems = wireshark_capture_problems(tree)
        self.assertTrue(any("install-setuid true preseed" in p for p in problems), problems)
        self.assertTrue(any("no wireshark-common/install-setuid false" in p for p in problems), problems)

    def test_missing_preseed_is_reported(self):
        tree = {"build/live-desktop/auto/config": "lb config noauto\n"}
        self.assertTrue(any("preseed" in p for p in wireshark_capture_problems(tree)))

    def test_auto_group_grant_is_reported(self):
        tree = {"build/live-desktop/config/hooks/normal/x.hook.chroot":
                "wireshark-common/install-setuid boolean false\nusermod -aG wireshark user\n"}
        self.assertTrue(any("privileged group" in p for p in wireshark_capture_problems(tree)))

    def test_passwordless_sudo_is_reported(self):
        tree = {"build/live-desktop/config/includes.chroot/etc/sudoers.d/x":
                "wireshark-common/install-setuid boolean false\nuser ALL=(ALL) NOPASSWD: ALL\n"}
        self.assertTrue(any("passwordless sudo" in p for p in wireshark_capture_problems(tree)))


if __name__ == "__main__":
    unittest.main()
