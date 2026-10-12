"""Static checks for the owner-approved foreign sources: pinned Kali and LibreWolf (F2, SECURITY-CRITICAL).

The owner decision includes Kali packages (zaproxy, exploitdb, ghidra, amass, radare2)
and librewolf. Each comes from a narrowly pinned source: the origin is denied by default
(Pin-Priority -1) and only an allowlist is raised to 500. Each source uses a Signed-By
keyring that is shipped and is never trusted=yes. No key may enter through apt-key or
trusted.gpg.d.

The files in build/live-desktop/config/archives are read by CONTENT. File names and
extensions (.sources or .list, .pref or .pref.chroot) are not part of the contract.
The policy itself lives in tests/lib/m1_apt_support.py and is shared with the image gate
(test_m1_image_contents.py, a Codex Remote check on the real ISO).

Negative controls: an un-pinned Kali source, a non-allowlisted package pinned at or
above 100, a key installed through apt-key or trusted.gpg.d, trusted=yes, and a missing
keyring must all FAIL. Positive controls show the accepted configuration passes.
"""

import os
import re
import sys
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402
import m1_support  # noqa: E402
import m1_apt_support as apt  # noqa: E402

ArchivePrefixes = ("cham3_kali.", "cham3_librewolf.")
ForeignMarkers = ("kali.org", "kali-rolling", "librewolf.net", "o=kali", "n=librewolf")
KeyFilePattern = re.compile(r"\.(key|asc|gpg)(\.chroot)?$")
PgpKeyBlock = "-----BEGIN PGP PUBLIC KEY BLOCK-----"


def read(relative_path):
    return m1_support.read_repo(relative_path)


def keyring_names():
    """Basenames of the keyrings the build tree ships under usr/share/keyrings."""
    root = os.path.join(m0_support.repo_root(), m1_support.KeyringIncludeDir)
    if not os.path.isdir(root):
        return set()
    return set(os.listdir(root))


def archive_inputs(root):
    """Classify the archives directory by content.

    Returns (source_files, pref_files, key_files, unexpected_files). A cham3_kali or
    cham3_librewolf file is read as a source when it has source syntax and as a pin file
    when it has Pin: stanzas. Any other file naming a foreign marker is unexpected. Any
    key-shaped file is a key.
    """
    sources, prefs, keys, unexpected = {}, {}, [], []
    if not os.path.isdir(root):
        raise AssertionError("required directory missing: " + root)
    for name in sorted(os.listdir(root)):
        path = os.path.join(root, name)
        if not os.path.isfile(path):
            continue
        with open(path, "r", encoding="utf-8", errors="replace") as handle:
            text = handle.read()
        if KeyFilePattern.search(name) or PgpKeyBlock in text:
            keys.append(name)
            continue
        if name.startswith(ArchivePrefixes):
            if re.search(r"(?m)^(URIs:|deb\s)", text):
                sources[name] = apt.parse_sources(text)
            if re.search(r"(?m)^Pin:", text):
                prefs[name] = apt.parse_preferences(text)
        elif any(marker in text for marker in ForeignMarkers):
            unexpected.append(name)
    return sources, prefs, keys, unexpected


def tree_mentions(relative_root, needle):
    root = os.path.join(m0_support.repo_root(), relative_root)
    if not os.path.isdir(root):
        raise AssertionError("required directory missing: " + relative_root)
    hits = []
    for dirpath, _, filenames in os.walk(root):
        for filename in filenames:
            path = os.path.join(dirpath, filename)
            with open(path, "r", encoding="utf-8", errors="replace") as handle:
                if needle in handle.read():
                    hits.append(os.path.relpath(path, m0_support.repo_root()))
    return hits


def foreign_file_problems(unexpected, keys):
    problems = ["foreign source or pin in an unexpected file: " + name for name in unexpected]
    problems += apt.key_problems(keys)
    return problems


class foreign_sources_real_t(unittest.TestCase):
    def inputs(self):
        return archive_inputs(os.path.join(m0_support.repo_root(), m1_support.ArchivesDir))

    def test_kali_source_is_narrowly_pinned(self):
        sources, prefs, _, _ = self.inputs()
        self.assertEqual(apt.repo_policy_problems(apt.KaliSpec, sources, prefs, keyring_names()), [])

    def test_librewolf_source_is_narrowly_pinned(self):
        sources, prefs, _, _ = self.inputs()
        self.assertEqual(apt.repo_policy_problems(apt.LibreWolfSpec, sources, prefs, keyring_names()), [])

    def test_no_foreign_source_or_pin_outside_the_cham3_archive_files(self):
        _, _, keys, unexpected = self.inputs()
        self.assertEqual(unexpected, [])

    def test_no_key_through_apt_key_or_trusted_gpg_d(self):
        _, _, keys, _ = self.inputs()
        self.assertEqual(apt.key_problems(keys), [])
        trusted = os.path.join(m0_support.repo_root(), m1_support.TrustedKeyIncludeDir)
        present = sorted(os.listdir(trusted)) if os.path.isdir(trusted) else []
        self.assertEqual(present, [], "keys shipped into trusted.gpg.d")
        self.assertEqual(tree_mentions(m1_support.LiveDesktopDir, "apt-key"), [],
                         "build configuration calls apt-key")


class foreign_sources_fixture_t(unittest.TestCase):
    """Positive and negative controls for the foreign-source policy."""

    KaliSource = (
        "Types: deb\n"
        "URIs: http://http.kali.org/kali/\n"
        "Suites: kali-rolling\n"
        "Components: main\n"
        "Signed-By: /usr/share/keyrings/cham3_kali_archive.gpg\n"
    )
    LibreWolfSource = (
        "Types: deb\n"
        "URIs: https://repo.librewolf.net\n"
        "Suites: librewolf\n"
        "Components: main\n"
        "Architectures: amd64\n"
        "Signed-By: /usr/share/keyrings/cham3_librewolf.gpg\n"
    )
    Keys = {"cham3_kali_archive.gpg", "cham3_librewolf.gpg"}

    def kali_pref(self, allow=None, default="-1"):
        text = "Package: *\nPin: release o=Kali\nPin-Priority: %s\n" % default
        for package in (m1_support.KaliAllowlist if allow is None else allow):
            text += "\nPackage: %s\nPin: release o=Kali\nPin-Priority: 500\n" % package
        return text

    LibreWolfPref = (
        "Package: *\nPin: release n=librewolf\nPin-Priority: -1\n\n"
        "Package: librewolf\nPin: release n=librewolf\nPin-Priority: 500\n"
    )

    def policy(self, kali_source=None, librewolf_source=None, kali_pref=None, librewolf_pref=None,
               keys=None, key_files=()):
        sources = {
            "cham3_kali.sources": apt.parse_sources(self.KaliSource if kali_source is None else kali_source),
            "cham3_librewolf.sources": apt.parse_sources(
                self.LibreWolfSource if librewolf_source is None else librewolf_source),
        }
        prefs = {
            "cham3_kali.pref": apt.parse_preferences(self.kali_pref() if kali_pref is None else kali_pref),
            "cham3_librewolf.pref": apt.parse_preferences(
                self.LibreWolfPref if librewolf_pref is None else librewolf_pref),
        }
        problems = apt.repo_policy_problems(apt.KaliSpec, sources, prefs, self.Keys if keys is None else keys)
        problems += apt.repo_policy_problems(apt.LibreWolfSpec, sources, prefs, self.Keys if keys is None else keys)
        problems += foreign_file_problems([], key_files)
        return problems

    def test_positive_control_accepted(self):
        self.assertEqual(self.policy(), [])

    def test_unpinned_kali_source_is_rejected(self):
        problems = self.policy(kali_pref="")
        self.assertTrue(any("no default deny" in p for p in problems), problems)

    def test_global_kali_priority_is_rejected(self):
        problems = self.policy(kali_pref="Package: *\nPin: release o=Kali\nPin-Priority: 500\n")
        self.assertTrue(any("default priority must be -1" in p for p in problems), problems)

    def test_non_allowlisted_package_at_500_is_rejected(self):
        problems = self.policy(kali_pref=self.kali_pref() + "\nPackage: kali-linux-large\nPin: release o=Kali\nPin-Priority: 500\n")
        self.assertTrue(any("non-allowlisted kali package kali-linux-large" in p for p in problems), problems)

    def test_non_allowlisted_package_at_100_is_rejected(self):
        problems = self.policy(kali_pref=self.kali_pref() + "\nPackage: wpscan-extra\nPin: release o=Kali\nPin-Priority: 100\n")
        self.assertTrue(any("non-allowlisted kali package" in p for p in problems), problems)

    def test_allowlisted_package_below_500_is_rejected(self):
        text = self.kali_pref().replace("Package: amass\nPin: release o=Kali\nPin-Priority: 500",
                                        "Package: amass\nPin: release o=Kali\nPin-Priority: 100")
        problems = self.policy(kali_pref=text)
        self.assertTrue(any("amass is pinned at 100" in p for p in problems), problems)

    def test_missing_allowlist_pin_is_rejected(self):
        problems = self.policy(kali_pref=self.kali_pref(allow=[p for p in m1_support.KaliAllowlist if p != "ghidra"]))
        self.assertTrue(any("ghidra has no pin" in p for p in problems), problems)

    def test_wildcard_pin_is_rejected(self):
        problems = self.policy(kali_pref=self.kali_pref() + "\nPackage: kali-*\nPin: release o=Kali\nPin-Priority: 500\n")
        self.assertTrue(any("wildcard kali pin" in p for p in problems), problems)

    def test_trusted_yes_kali_source_is_rejected(self):
        source = self.KaliSource.replace("Signed-By: /usr/share/keyrings/cham3_kali_archive.gpg", "Trusted: yes")
        problems = self.policy(kali_source=source)
        self.assertTrue(any("trusted=yes" in p for p in problems), problems)

    def test_kali_source_without_signed_by_is_rejected(self):
        source = self.KaliSource.replace("Signed-By: /usr/share/keyrings/cham3_kali_archive.gpg\n", "")
        problems = self.policy(kali_source=source)
        self.assertTrue(any("no Signed-By keyring" in p for p in problems), problems)

    def test_signed_by_keyring_that_is_not_shipped_is_rejected(self):
        problems = self.policy(keys={"cham3_librewolf.gpg"})
        self.assertTrue(any("Signed-By keyring cham3_kali_archive.gpg is not shipped" in p for p in problems), problems)

    def test_key_installed_through_apt_key_or_trusted_gpg_d_is_rejected(self):
        problems = foreign_file_problems([], ["cham3_kali.key.chroot"])
        self.assertTrue(any("apt-key or trusted.gpg.d" in p for p in problems), problems)

    def test_foreign_source_in_unexpected_file_is_rejected(self):
        problems = foreign_file_problems(["kali.list.chroot"], [])
        self.assertTrue(any("unexpected file: kali.list.chroot" in p for p in problems), problems)

    def test_librewolf_unpinned_is_rejected(self):
        problems = self.policy(librewolf_pref="")
        self.assertTrue(any("no default deny" in p and "librewolf" in p for p in problems), problems)

    def test_librewolf_non_allowlisted_at_500_is_rejected(self):
        problems = self.policy(librewolf_pref=self.LibreWolfPref + "\nPackage: firefox-extra\nPin: release n=librewolf\nPin-Priority: 500\n")
        self.assertTrue(any("non-allowlisted librewolf package firefox-extra" in p for p in problems), problems)

    def test_librewolf_trusted_yes_is_rejected(self):
        source = self.LibreWolfSource.replace("Signed-By: /usr/share/keyrings/cham3_librewolf.gpg", "Trusted: yes")
        problems = self.policy(librewolf_source=source)
        self.assertTrue(any("librewolf" in p and "trusted=yes" in p for p in problems), problems)


if __name__ == "__main__":
    unittest.main()
