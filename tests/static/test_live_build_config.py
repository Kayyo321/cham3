"""Static checks for the M0 live-build definition (build/live/auto/config)."""

import os
import re
import shlex
import sys
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402

AutoConfigPath = "build/live/auto/config"
PackageListsDir = "build/live/config/package-lists"
ForbiddenAptFragments = (
    "allow-unauthenticated",
    "allowunauthenticated",
    "allowinsecurerepositories",
    "trusted=yes",
)
ForbiddenPackageStems = (
    "cham3-core",
    "cham3-sandbox",
    "cham3-networking",
    "cham3-desktop",
    "cham3-security",
    "cham3-full",
    "task-kde-desktop",
    "task-desktop",
    "xorg",
    "xserver-xorg",
    "tor",
    "torsocks",
    "nftables",
    "iptables",
    "ufw",
    "firewalld",
    "apparmor",
    "firejail",
    "bubblewrap",
    "flatpak",
    "wireguard",
    "openvpn",
    "network-manager",
)
PrefixStems = ("kde", "plasma", "kwin", "sddm")
RequiredBootParameters = ("boot=live", "console=tty0", "console=ttyS0,115200n8")


def config_tokens():
    text = m0_support.read_text(AutoConfigPath)
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


def is_forbidden_package(name):
    base = name.split(":", 1)[0]
    for stem in ForbiddenPackageStems:
        if base == stem or base.startswith(stem + "-"):
            return True
    return any(base.startswith(stem) for stem in PrefixStems)


def package_list_names(path):
    names = []
    with open(path, "r", encoding="utf-8") as handle:
        for line in handle:
            stripped = line.split("#", 1)[0].strip()
            names.extend(stripped.split())
    return names


class live_build_config_t(unittest.TestCase):
    def test_lb_config_is_invoked_noauto(self):
        text = m0_support.read_text(AutoConfigPath)
        self.assertRegex(text, r"\blb\s+config\s+noauto\b")

    def test_distribution_is_trixie(self):
        values = option_values(config_tokens(), "--distribution")
        self.assertEqual(values, ["trixie"])

    def test_architecture_is_amd64(self):
        tokens = config_tokens()
        values = option_values(tokens, "--architectures") + option_values(
            tokens, "--architecture"
        )
        self.assertEqual(values, ["amd64"])

    def test_archive_areas_are_main_only(self):
        values = option_values(config_tokens(), "--archive-areas")
        self.assertEqual(len(values), 1, "--archive-areas must be set exactly once")
        self.assertEqual(values[0].split(), ["main"])

    def test_debian_installer_disabled(self):
        values = option_values(config_tokens(), "--debian-installer")
        self.assertEqual(values, ["none"])

    def test_binary_image_is_iso_hybrid(self):
        # live-build 20250505 accepts both spellings for the same option: its
        # usage text documents --binary-image and lb config also accepts
        # --binary-images as an alias (/usr/lib/live/build/config). The
        # requirement is an iso-hybrid binary image, not one spelling.
        tokens = config_tokens()
        values = option_values(tokens, "--binary-image") + option_values(tokens, "--binary-images")
        self.assertEqual(values, ["iso-hybrid"])

    def test_non_free_firmware_is_off(self):
        tokens = config_tokens()
        for name in ("--firmware-binary", "--firmware-chroot"):
            for value in option_values(tokens, name):
                self.assertEqual(value, "false", "%s must be false" % name)

    def test_security_and_updates_are_not_disabled(self):
        tokens = config_tokens()
        for name in ("--security", "--updates"):
            for value in option_values(tokens, name):
                self.assertEqual(value, "true", "%s must be true" % name)

    def test_apt_authentication_is_not_disabled(self):
        tokens = config_tokens()
        for value in option_values(tokens, "--apt-secure"):
            self.assertEqual(value, "true", "--apt-secure must stay enabled")
        lowered = m0_support.read_text(AutoConfigPath).lower()
        self.assertNotIn("apt-secure false", lowered)
        self.assertNotIn("apt-secure=false", lowered)
        for fragment in ForbiddenAptFragments:
            self.assertNotIn(fragment, lowered)

    def test_mirrors_are_official_debian_hosts(self):
        tokens = config_tokens()
        for name in (
            "--mirror-bootstrap",
            "--mirror-chroot",
            "--mirror-chroot-security",
        ):
            self.assertTrue(
                option_values(tokens, name), "%s must be set" % name
            )
        hosts = set()
        for token in tokens:
            match = re.match(r"^https?://([^/\s]+)", token)
            if match:
                hosts.add(match.group(1))
        self.assertTrue(hosts, "no mirror URLs found in %s" % AutoConfigPath)
        self.assertLessEqual(
            hosts,
            set(m0_support.OfficialMirrorHosts),
            "non-official mirror host configured",
        )

    def test_serial_console_boot_parameters_present(self):
        values = option_values(config_tokens(), "--bootappend-live")
        self.assertEqual(len(values), 1, "--bootappend-live must be set exactly once")
        parameters = set(values[0].split())
        for required in RequiredBootParameters:
            self.assertIn(required, parameters)

    def test_iso_metadata_names_m0_development_image(self):
        tokens = config_tokens()
        combined = " ".join(
            value
            for name in ("--iso-application", "--iso-volume", "--iso-publisher")
            for value in option_values(tokens, name)
        ).lower()
        self.assertIn("cham3", combined)
        self.assertIn("m0", combined)

    def test_package_name_rule_rejects_out_of_scope_names(self):
        for name in ("plasma-desktop", "kde-full", "tor", "nftables", "cham3-core"):
            self.assertTrue(is_forbidden_package(name), name)
        for name in ("live-boot", "live-config", "torrent-tools", "sudo"):
            self.assertFalse(is_forbidden_package(name), name)

    def test_package_lists_add_no_out_of_scope_components(self):
        directory = os.path.join(m0_support.repo_root(), PackageListsDir)
        problems = []
        for root, _, files in os.walk(directory):
            for filename in files:
                path = os.path.join(root, filename)
                for name in package_list_names(path):
                    if is_forbidden_package(name):
                        problems.append("%s lists out-of-scope package %s" % (filename, name))
        self.assertEqual(problems, [])


if __name__ == "__main__":
    unittest.main()
