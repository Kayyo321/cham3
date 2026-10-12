"""Static checks for the owner-approved foreign sources: pinned Kali and LibreWolf (F2, SECURITY-CRITICAL).

The owner decision includes Kali packages (zaproxy, exploitdb, ghidra, amass, radare2)
and librewolf. Each comes from a narrowly pinned source: the origin is denied by default
(Pin-Priority -1) and only an allowlist is raised to 500. Each source names a Signed-By
keyring that the build ships, and no source is trusted=yes.

Discovery (file names and layout are not the contract; file CONTENT is):
  - sources: build/live-desktop/config/archives/zzz_cham3_kali.list and
    zzz_cham3_librewolf.list, one-line deb entries. The zzz_ prefix is a security
    requirement: it sorts the foreign source after live-build's Debian sources
    (zz-sources.list), so Debian versions win ties. This suite checks that ordering.
  - preferences: archives/cham3_kali.pref and cham3_librewolf.pref;
  - keys: build/live-desktop/keys/*.asc, the reviewed armored public keys. The
    primary fingerprint is computed from the packets (no gpg needed) and must equal the
    pinned owner fingerprint. build/live-desktop/auto/build_archive_keyring turns them into
    the cham3-archive-keyring package. Its keyring names come from the *_keyring_name=
    constants there, and those names are the only ones a Signed-By may name.

Keys reach apt through Signed-By keyrings, never apt-key or trusted.gpg.d. The final-image
state (no key in trusted.gpg.d, Signed-By honored, no trusted=yes in any installed source)
is asserted on the real ISO by test_m1_image_contents.py (Codex Remote), not here.

Negative controls: an un-pinned Kali source, a non-allowlisted package pinned at or above
100, a source that sorts before zz-sources.list, a key installed through apt-key or
trusted.gpg.d, trusted=yes, a missing keyring, an extra or secret key, a revocation,
and a build script that does not check the pinned hash, fingerprint, or key set.
Positive controls show the accepted configuration passes.
"""

import base64
import hashlib
import os
import re
import struct
import sys
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402
import m1_support  # noqa: E402
import m1_apt_support as apt  # noqa: E402

ForeignMarkers = ("kali.org", "kali-rolling", "librewolf.net", "o=kali", "n=librewolf")
KeyFilePattern = re.compile(r"\.(key|asc|gpg)(\.chroot)?$")
PgpKeyBlock = "-----BEGIN PGP PUBLIC KEY BLOCK-----"
ReviewedKeyEntries = frozenset(m1_support.ArchiveKeyFiles.values()) | {"README"}


def read(relative_path):
    return m1_support.read_repo(relative_path)


def keyring_names_from_script(text):
    """Shipped keyring names the build script declares (the only names a Signed-By may use)."""
    names = apt.keyring_script_names(text)
    for repo in ("kali", "librewolf"):
        if repo not in names:
            raise AssertionError("build_archive_keyring declares no %s_keyring_name" % repo)
    return set(names.values())


def archive_inputs(root):
    """Classify the archives directory by content.

    Returns (source_files, pref_files, key_files, unexpected_files). A cham3 archive file
    (by the ArchiveNamePrefixes contract) is read as a source when it has source syntax and as
    a pin file when it has Pin: stanzas. Any other file naming a foreign marker is unexpected.
    Any key-shaped file is a key.
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
        if name.startswith(m1_support.ArchiveNamePrefixes):
            if re.search(r"(?m)^(URIs:|deb\s)", text):
                sources[name] = apt.parse_sources(text)
            if re.search(r"(?m)^Pin:", text):
                prefs[name] = apt.parse_preferences(text)
        elif any(marker in text for marker in ForeignMarkers):
            unexpected.append(name)
    return sources, prefs, keys, unexpected


def tree_texts(relative_root):
    """{repo-relative path: text} for every file under a repo-relative directory."""
    root = os.path.join(m0_support.repo_root(), relative_root)
    if not os.path.isdir(root):
        raise AssertionError("required directory missing: " + relative_root)
    texts = {}
    for dirpath, _, filenames in os.walk(root):
        for filename in filenames:
            path = os.path.join(dirpath, filename)
            with open(path, "r", encoding="utf-8", errors="replace") as handle:
                texts[os.path.relpath(path, m0_support.repo_root())] = handle.read()
    return texts


KeyringToolPackages = {"gpg": "gnupg", "dpkg-deb": "dpkg-dev"}
HostPackagesFile = "build/host_packages.txt"
KeyringToolLoop = re.compile(r"for tool in ([^;]+);")


def keyring_tool_problems(script_text, host_text):
    """Host packages missing for the tools build_archive_keyring requires (empty means listed)."""
    tools = KeyringToolLoop.search(script_text)
    if tools is None:
        return ["build_archive_keyring declares no required-tool loop"]
    listed = set()
    for raw in host_text.splitlines():
        line = raw.split("#", 1)[0].strip()
        if line:
            listed.add(line.split()[0])
    problems = []
    for tool in tools.group(1).split():
        package = KeyringToolPackages.get(tool)
        if package is not None and package not in listed:
            problems.append("%s is required by build_archive_keyring but %s is not in %s"
                            % (tool, package, HostPackagesFile))
    return problems


def foreign_file_problems(unexpected, keys):
    problems = ["foreign source or pin in an unexpected file: " + name for name in unexpected]
    problems += apt.key_problems(keys)
    return problems


def source_order_problems(source_files):
    """Each foreign source must sort after the Debian sources, or Debian versions would not win ties."""
    problems = []
    for name in sorted(source_files):
        if not name > m1_support.DebianSourcesListName:
            problems.append("%s sorts before %s, so a Kali or LibreWolf build of a Debian version could win"
                            % (name, m1_support.DebianSourcesListName))
    return problems


def key_file_problems():
    """Problems with the reviewed key files shipped in keys/ (empty means exactly the two pins)."""
    problems = []
    root = os.path.join(m0_support.repo_root(), m1_support.ArchiveKeysDir)
    if not os.path.isdir(root):
        raise AssertionError("required directory missing: " + m1_support.ArchiveKeysDir)
    present = set(os.listdir(root))
    for extra in sorted(present - ReviewedKeyEntries):
        problems.append("unreviewed file in keys/: " + extra)
    for repo, name in sorted(m1_support.ArchiveKeyFiles.items()):
        if name not in present:
            problems.append("reviewed %s key is missing: %s" % (repo, name))
            continue
        text = read(os.path.join(m1_support.ArchiveKeysDir, name))
        for problem in apt.key_text_problems(text, m1_support.ArchiveKeyFingerprints[repo]):
            problems.append("%s key %s: %s" % (repo, name, problem))
    return problems


class foreign_sources_real_t(unittest.TestCase):
    def inputs(self):
        return archive_inputs(os.path.join(m0_support.repo_root(), m1_support.ArchivesDir))

    def keyring_names(self):
        return keyring_names_from_script(read(m1_support.KeyringScript))

    def test_kali_source_is_narrowly_pinned(self):
        sources, prefs, _, _ = self.inputs()
        self.assertEqual(apt.repo_policy_problems(apt.KaliSpec, sources, prefs, self.keyring_names()), [])

    def test_librewolf_source_is_narrowly_pinned(self):
        sources, prefs, _, _ = self.inputs()
        self.assertEqual(apt.repo_policy_problems(apt.LibreWolfSpec, sources, prefs, self.keyring_names()), [])

    def test_no_foreign_source_or_pin_outside_the_cham3_archive_files(self):
        _, _, _, unexpected = self.inputs()
        self.assertEqual(unexpected, [])

    def test_no_key_through_apt_key_or_trusted_gpg_d(self):
        # Build-time (static) assertions. The final image state is checked in image-m1.
        sources, prefs, keys, _ = self.inputs()
        self.assertEqual(apt.key_problems(keys), [], "a key file is in the archives directory")
        self.assertEqual(apt_key_build_problems(), [], "the build tree invokes apt-key")
        trusted = os.path.join(m0_support.repo_root(), m1_support.TrustedKeyIncludeDir)
        self.assertFalse(os.path.isdir(trusted) and os.listdir(trusted),
                         "keys are shipped into includes.chroot/etc/apt/trusted.gpg.d")

    def test_foreign_sources_sort_after_the_debian_sources(self):
        sources, _, _, _ = self.inputs()
        self.assertEqual(source_order_problems(sources), [])

    def test_archive_keys_are_the_reviewed_fingerprints(self):
        self.assertEqual(key_file_problems(), [])

    def test_keyring_build_tools_are_listed_in_host_packages(self):
        # build_archive_keyring needs gpg (package gnupg) and dpkg-deb; the host list must name them.
        script = read(m1_support.KeyringScript)
        host = read(HostPackagesFile)
        self.assertEqual(keyring_tool_problems(script, host), [])

    def test_keyring_build_pins_hashes_and_fingerprints_and_rejects_bad_keys(self):
        text = read(m1_support.KeyringScript)
        self.assertEqual(apt.keyring_script_problems(text), [])
        pinned = {match.group(1): match.group(2) for match in apt.KeyringScriptHashPattern.finditer(text)}
        for repo, name in m1_support.ArchiveKeyFiles.items():
            with open(os.path.join(m0_support.repo_root(), m1_support.ArchiveKeysDir, name), "rb") as handle:
                actual = hashlib.sha256(handle.read()).hexdigest()
            self.assertEqual(pinned.get(repo), actual, "pinned SHA-256 differs from keys/%s" % name)

    def test_source_keyrings_are_the_ones_the_build_ships(self):
        sources, _, _, _ = self.inputs()
        shipped = self.keyring_names()
        for repo, spec in (("kali", apt.KaliSpec), ("librewolf", apt.LibreWolfSpec)):
            signed = {
                os.path.basename(entry["signed_by"] or "")
                for files in sources.values()
                for entry in files
                if any(spec["host"] in uri for uri in entry["uris"])
            }
            self.assertTrue(signed, "no %s source names a Signed-By keyring" % repo)
            self.assertTrue(signed <= shipped, "%s Signed-By names a keyring the build does not ship: %s"
                            % (repo, sorted(signed - shipped)))


def apt_key_build_problems():
    """Build-time apt-key invocations in the build tree (prose and comments are not invocations)."""
    return apt.apt_key_invocation_problems(tree_texts(m1_support.LiveDesktopDir))


def synthetic_armor(packets):
    """Armored public key text for the given (tag, body) packets, with a valid CRC24 line."""
    payload = b""
    for tag, body in packets:
        payload += bytes([0xC0 | tag])
        if len(body) < 192:
            payload += bytes([len(body)])
        elif len(body) < 8384:
            payload += bytes([((len(body) - 192) >> 8) + 192, (len(body) - 192) & 0xFF])
        else:
            payload += b"\xff" + struct.pack(">I", len(body))
        payload += body
    crc = apt.crc24(payload).to_bytes(3, "big")
    encoded = base64.b64encode(payload).decode()
    lines = [apt.ArmorBegin, ""]
    lines += [encoded[index:index + 64] for index in range(0, len(encoded), 64)]
    lines += ["=" + base64.b64encode(crc).decode(), apt.ArmorEnd]
    return "\n".join(lines) + "\n"


def primary_body(seed=b"\x42"):
    return bytes([4]) + struct.pack(">I", 1600000000) + bytes([1]) + seed * 32


class foreign_sources_fixture_t(unittest.TestCase):
    """Positive and negative controls for the foreign-source policy and key checks."""

    KaliSource = (
        "# comment lines are ignored\n"
        "deb [arch=amd64 signed-by=/usr/share/keyrings/cham3_kali_archive.gpg] "
        "http://http.kali.org/kali/ kali-rolling main\n"
    )
    LibreWolfSource = (
        "deb [arch=amd64 signed-by=/usr/share/keyrings/cham3_librewolf.gpg] "
        "https://repo.librewolf.net librewolf main\n"
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
            "zzz_cham3_kali.list": apt.parse_sources(self.KaliSource if kali_source is None else kali_source),
            "zzz_cham3_librewolf.list": apt.parse_sources(
                self.LibreWolfSource if librewolf_source is None else librewolf_source),
        }
        prefs = {
            "cham3_kali.pref": apt.parse_preferences(self.kali_pref() if kali_pref is None else kali_pref),
            "cham3_librewolf.pref": apt.parse_preferences(
                self.LibreWolfPref if librewolf_pref is None else librewolf_pref),
        }
        names = self.Keys if keys is None else keys
        problems = apt.repo_policy_problems(apt.KaliSpec, sources, prefs, names)
        problems += apt.repo_policy_problems(apt.LibreWolfSpec, sources, prefs, names)
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
        source = self.KaliSource.replace("signed-by=/usr/share/keyrings/cham3_kali_archive.gpg", "trusted=yes")
        problems = self.policy(kali_source=source)
        self.assertTrue(any("trusted=yes" in p for p in problems), problems)

    def test_kali_source_without_signed_by_is_rejected(self):
        source = self.KaliSource.replace("signed-by=/usr/share/keyrings/cham3_kali_archive.gpg", "")
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

    def test_source_sorting_before_debian_is_rejected(self):
        # Negative control: the cham3_kali.list name the earlier layout used sorts before zz-sources.list.
        problems = source_order_problems({"cham3_kali.list": {}, "zzz_cham3_kali.list": {}})
        self.assertEqual(len(problems), 1, problems)
        self.assertTrue(problems[0].startswith("cham3_kali.list sorts before zz-sources.list"), problems)

    def test_librewolf_unpinned_is_rejected(self):
        problems = self.policy(librewolf_pref="")
        self.assertTrue(any("no default deny" in p and "librewolf" in p for p in problems), problems)

    def test_librewolf_non_allowlisted_at_500_is_rejected(self):
        problems = self.policy(librewolf_pref=self.LibreWolfPref + "\nPackage: firefox-extra\nPin: release n=librewolf\nPin-Priority: 500\n")
        self.assertTrue(any("non-allowlisted librewolf package firefox-extra" in p for p in problems), problems)

    def test_librewolf_trusted_yes_is_rejected(self):
        source = self.LibreWolfSource.replace("signed-by=/usr/share/keyrings/cham3_librewolf.gpg", "trusted=yes")
        problems = self.policy(librewolf_source=source)
        self.assertTrue(any("librewolf" in p and "trusted=yes" in p for p in problems), problems)

    def test_key_packet_positive_control_accepted(self):
        body = primary_body()
        expected = apt.v4_primary_fingerprint(body)
        self.assertEqual(apt.key_text_problems(synthetic_armor([(6, body)]), expected), [])

    def test_extra_primary_key_is_rejected(self):
        body = primary_body()
        text = synthetic_armor([(6, body), (6, primary_body(b"\x43"))])
        problems = apt.key_text_problems(text, apt.v4_primary_fingerprint(body))
        self.assertTrue(any("holds 2 primary keys" in p for p in problems), problems)

    def test_secret_key_material_is_rejected(self):
        body = primary_body()
        problems = apt.key_text_problems(synthetic_armor([(6, body), (5, body)]), apt.v4_primary_fingerprint(body))
        self.assertTrue(any("secret key packet" in p for p in problems), problems)

    def test_primary_revocation_signature_is_rejected(self):
        body = primary_body()
        revocation = bytes([4, 0x20, 1, 8, 0, 0])
        problems = apt.key_text_problems(synthetic_armor([(6, body), (2, revocation)]), apt.v4_primary_fingerprint(body))
        self.assertTrue(any("revocation signature" in p for p in problems), problems)

    def test_wrong_primary_fingerprint_is_rejected(self):
        body = primary_body()
        problems = apt.key_text_problems(synthetic_armor([(6, body)]), "0" * 40)
        self.assertTrue(any("expected 000000" in p for p in problems), problems)

    def test_armor_with_bad_crc_is_rejected(self):
        body = primary_body()
        text = synthetic_armor([(6, body)])
        lines = text.splitlines()
        lines[-2] = "=AAAA"
        problems = apt.key_text_problems("\n".join(lines) + "\n", apt.v4_primary_fingerprint(body))
        self.assertTrue(any("CRC24" in p for p in problems), problems)

    def test_keyring_build_positive_control_accepted(self):
        self.assertEqual(apt.keyring_script_problems(self.good_keyring_script()), [])

    def test_keyring_build_without_revocation_or_expiry_check_is_rejected(self):
        text = self.good_keyring_script().replace('$2 == "e" || $2 == "r"', 'bogus')
        problems = apt.keyring_script_problems(text)
        self.assertTrue(any("expired or revoked" in p for p in problems), problems)

    def test_keyring_build_without_pinned_hash_is_rejected(self):
        text = self.good_keyring_script().replace("kali_sha256=", "# removed kali_sha256=")
        problems = apt.keyring_script_problems(text)
        self.assertTrue(any("does not pin the SHA-256 of the kali key" in p for p in problems), problems)

    def test_keyring_build_calling_apt_key_is_rejected(self):
        text = self.good_keyring_script() + "\napt-key add keys/extra.asc\n"
        problems = apt.keyring_script_problems(text)
        self.assertTrue(any("calls apt-key" in p for p in problems), problems)

    def test_apt_key_in_prose_is_not_an_invocation(self):
        self.assertEqual(apt.apt_key_invocation_problems({"x": "# no apt-key here, not trusted.gpg.d\n"}), [])
        self.assertTrue(apt.apt_key_invocation_problems({"x": "apt-key add keys/a.asc\n"}))

    def test_keyring_tools_missing_from_host_packages_are_rejected(self):
        script = "for tool in gpg sha256sum dpkg-deb awk date mktemp; do\n"
        problems = keyring_tool_problems(script, "dpkg-dev\nlive-build\n")
        self.assertEqual(problems, ["gpg is required by build_archive_keyring but gnupg is not in build/host_packages.txt"])

    def test_keyring_tools_listed_in_host_packages_are_accepted(self):
        script = "for tool in gpg sha256sum dpkg-deb awk date mktemp; do\n"
        self.assertEqual(keyring_tool_problems(script, "gnupg\ndpkg-dev\n"), [])

    def test_keyring_names_come_from_the_script_constants(self):
        names = apt.keyring_script_names(self.good_keyring_script())
        self.assertEqual(names, {"kali": "cham3_kali_archive.gpg", "librewolf": "cham3_librewolf.gpg"})

    def good_keyring_script(self):
        return (
            "kali_sha256=bbaef4b375fdae6d361d648355c9a003b081d421a40f39440f296cbd458071b1\n"
            "kali_fingerprint=827C8569F2518CC677FECA1AED65462EC8D5E4C5\n"
            "kali_keyring_name=cham3_kali_archive.gpg\n"
            "librewolf_sha256=cd1c45ebf33b42191af369b5ea901508eb7dff3dd5086abcb601b25121bc70ad\n"
            "librewolf_fingerprint=662E3CDD6FE329002D0CA5BB40339DD82B12EF16\n"
            "librewolf_keyring_name=cham3_librewolf.gpg\n"
            "sha256sum \"$1\"\n"
            "check_primary=$(printf '%s' x | awk -F: '$1 == \"fpr\"')\n"
            "[ \"${check_count}\" = \"1\" ] || die \"holds ${check_count} primary keys, expected exactly 1\"\n"
            "[ \"${check_secret}\" = \"0\" ] || die \"contains secret key material\"\n"
            "awk -F: '$1 == \"pub\" { if ($2 == \"e\" || $2 == \"r\") bad = 1 }'\n"
            "gpg --batch --no-tty --dearmor < key > \"stage/usr/share/keyrings/x\"\n"
            "# keyrings are written under usr/share/keyrings/ only\n"
        )


if __name__ == "__main__":
    unittest.main()
