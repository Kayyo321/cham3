"""Checks inside the built M1 desktop ISO's root filesystem (VM/container stage).

The live root filesystem is read from the ISO with xorriso and listed or read with
unsquashfs. Both are read-only; nothing is mounted or installed on the host. Missing
ISO or tools FAIL (M0 convention). Runtime behavior (session type, panel geometry,
rendering) is NOT covered here; that is a VM boot-stage check.

Real-ISO tests (image_contents_t) are Codex Remote heavy gates and need the built ISO.
Fixture controls (image_contents_fixture_t) run without an ISO and check the logic.

F2 (SECURITY-CRITICAL): the image may contain the pinned Kali and LibreWolf sources. The
image apt policy must show the secure pins, and no installed package may have a foreign
origin outside its allowlist. Origin needs the Debian and foreign apt package indexes in
the image; when they are absent the check fails closed rather than passing silently.
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
import m1_apt_support as apt  # noqa: E402

SquashfsMember = "/live/filesystem.squashfs"
StatusMember = "var/lib/dpkg/status"
PlasmaSessionFile = m1_support.PackagedPlasmaSession
ColorSchemeFile = "usr/share/color-schemes/Cham3Dark.colors"
LookAndFeelMetadata = "usr/share/plasma/look-and-feel/org.cham3.desktop/metadata.json"
WallpaperImage = m1_support.PackagedWallpaperImage
WallpaperMetadata = m1_support.PackagedWallpaperMetadata
RequiredFiles = (PlasmaSessionFile, ColorSchemeFile, LookAndFeelMetadata, WallpaperImage, WallpaperMetadata)
BackendBinaryPattern = re.compile(r"^(usr/)?(s?bin|lib(exec)?)/cham3[-_]")
SystemdUnitPattern = re.compile(r"^(usr/)?lib/systemd/system/cham3[-_]")
AptSourceFile = "etc/apt/sources.list"
AptSourceDir = "etc/apt/sources.list.d/"
AptPreferenceDir = "etc/apt/preferences.d/"
ImageKeyringDir = "usr/share/keyrings/"
ImageTrustedKeyPaths = ("etc/apt/trusted.gpg.d/", "etc/apt/trusted.gpg")
AptIndexDir = "var/lib/apt/lists/"
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
    """Return the set of paths in the root filesystem (no leading slash)."""
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
    return {name for name, _ in apt.installed_pairs(status_text)}


def contents_problems(paths, installed):
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
    for required in RequiredFiles:
        if required not in paths:
            problems.append("required file missing from image: " + required)
    for path in sorted(paths):
        if BackendBinaryPattern.match(path):
            problems.append("absent backend binary shipped in image: " + path)
        if SystemdUnitPattern.match(path):
            problems.append("cham3 systemd unit shipped in image: " + path)
    return problems


def apt_inputs(paths, read):
    """(source files, preference files, keyring names, key files) from the image's apt tree.

    read(path) returns the member text. Sources are read from etc/apt/sources.list and
    sources.list.d; preferences from preferences.d. Keys are files under trusted.gpg.d or
    the legacy trusted.gpg.
    """
    source_files, pref_files = {}, {}
    for path in sorted(paths):
        if path == AptSourceFile or (path.startswith(AptSourceDir) and path.endswith((".list", ".sources"))):
            source_files[path] = apt.parse_sources(read(path))
        elif path.startswith(AptPreferenceDir) and "/" not in path[len(AptPreferenceDir):]:
            pref_files[path] = apt.parse_preferences(read(path))
    keyring_names = {
        path[len(ImageKeyringDir):]
        for path in paths
        if path.startswith(ImageKeyringDir) and "/" not in path[len(ImageKeyringDir):]
    }
    key_files = [
        path
        for path in paths
        if path.startswith(ImageTrustedKeyPaths[0]) and "/" not in path[len(ImageTrustedKeyPaths[0]):]
    ] + [path for path in paths if path == ImageTrustedKeyPaths[1]]
    return source_files, pref_files, keyring_names, key_files


DebianKeyringFileList = "var/lib/dpkg/info/debian-archive-keyring.list"
DebianKeyringPrefix = "debian-archive-"


def debian_owned_trust_files(file_list_text):
    """Paths under etc/apt/trusted.gpg.d/ that the debian-archive-keyring package installed."""
    owned = set()
    for line in file_list_text.splitlines():
        path = line.strip().lstrip("/")
        if path.startswith(ImageTrustedKeyPaths[0]) and path != ImageTrustedKeyPaths[0].rstrip("/"):
            owned.add(path)
    return owned


def foreign_trust_files(key_files, debian_owned):
    """Global-trust key files other than Debian's own archive keys (debian-archive-keyring).

    Debian 13's debian-archive-keyring package installs the Debian archive keys under
    /etc/apt/trusted.gpg.d; that is Debian's default trust for the Debian archives. Every
    other file there (a Kali or LibreWolf key above all) is a policy violation: those keys
    must reach apt only through Signed-By.
    """
    return [path for path in key_files
            if not (path in debian_owned and os.path.basename(path).startswith(DebianKeyringPrefix))]


def apt_policy_problems(source_files, pref_files, keyring_names, key_files, debian_owned=frozenset()):
    """Problems with the image's apt policy for the two foreign repositories (F2)."""
    problems = []
    for spec in apt.RepoSpecs:
        problems += apt.repo_policy_problems(spec, source_files, pref_files, keyring_names)
    problems += apt.key_problems(foreign_trust_files(key_files, debian_owned))
    return problems


def image_trusted_sources(source_files):
    """Installed sources that set trusted=yes (any repository, not only the foreign ones)."""
    return sorted(path for path, entries in source_files.items() for entry in entries if entry["trusted"])


def shipped_keyring_names():
    """The keyring names build_archive_keyring ships (its *_keyring_name= constants, read from the repo)."""
    return sorted(apt.keyring_script_names(m1_support.read_repo(m1_support.KeyringScript)).values())


def image_keyring_problems(keyring_names, expected_names):
    return ["shipped keyring missing from the image: " + name
            for name in sorted(set(expected_names) - set(keyring_names))]


def kali_source_present(source_files):
    return any(
        apt.KaliSpec["host"] in uri
        for entries in source_files.values()
        for entry in entries
        for uri in entry["uris"]
    )


def image_origin_problems(status_pairs, index_texts, kali_sources_present):
    """Installed packages with a foreign origin outside its allowlist (fails closed, F2).

    index_texts maps an apt list file name to its text. The Debian index is identified
    by 'debian' in the name, the Kali and LibreWolf indexes by 'kali' and 'librewolf'.
    """
    debian, foreign = set(), {}
    for name, text in index_texts.items():
        lowered = name.lower()
        pairs = apt.parse_packages_index(text)
        if "kali" in lowered:
            foreign.setdefault("kali", set()).update(pairs)
        elif "librewolf" in lowered:
            foreign.setdefault("librewolf", set()).update(pairs)
        elif "debian" in lowered:
            debian |= pairs
    if not debian:
        return ["no Debian package index retained in the image: package origin cannot be established"]
    problems = []
    if kali_sources_present and "kali" not in foreign:
        problems.append("a Kali source is configured but no Kali index is retained: Kali-origin packages cannot be verified")
    allowlists = {"kali": m1_support.KaliAllowlist, "librewolf": m1_support.LibreWolfAllowlist}
    return problems + apt.origin_problems(status_pairs, debian, foreign, allowlists)


class image_contents_t(unittest.TestCase):
    """Real-ISO gates (Codex Remote). Missing ISO or tools FAIL."""

    def test_contents_match_m1_contract(self):
        paths = rootfs_listing()
        installed = installed_package_names(read_member(StatusMember))
        self.assertEqual(contents_problems(paths, installed), [])

    def test_image_apt_policy_pins_foreign_sources(self):
        paths = rootfs_listing()
        source_files, pref_files, keyring_names, key_files = apt_inputs(paths, read_member)
        owned = debian_owned_trust_files(read_member(DebianKeyringFileList))
        self.assertEqual(apt_policy_problems(source_files, pref_files, keyring_names, key_files, owned), [])

    def test_final_apt_sources_have_no_trusted_yes(self):
        # The final-image state. The build chroot's own local repository uses trusted=yes
        # transiently (live-build); that is build-only and is not checked here.
        paths = rootfs_listing()
        source_files, _, _, _ = apt_inputs(paths, read_member)
        self.assertEqual(image_trusted_sources(source_files), [])

    def test_keys_reach_apt_only_through_shipped_signed_by_keyrings(self):
        # Final-image assertion: the reviewed keys are dearmored into /usr/share/keyrings by the
        # cham3-archive-keyring package and nowhere in the global trust store.
        paths = rootfs_listing()
        _, _, keyring_names, key_files = apt_inputs(paths, read_member)
        problems = image_keyring_problems(keyring_names, shipped_keyring_names())
        owned = debian_owned_trust_files(read_member(DebianKeyringFileList))
        problems += apt.key_problems(foreign_trust_files(key_files, owned))
        self.assertEqual(problems, [])

    def test_no_unallowlisted_foreign_origin_package(self):
        paths = rootfs_listing()
        source_files, _, _, _ = apt_inputs(paths, read_member)
        index_texts = {
            os.path.basename(path): read_member(path)
            for path in sorted(paths)
            if path.startswith(AptIndexDir) and path.endswith("_Packages")
        }
        problems = image_origin_problems(
            apt.installed_pairs(read_member(StatusMember)),
            index_texts,
            kali_source_present(source_files),
        )
        self.assertEqual(problems, [])


class image_contents_fixture_t(unittest.TestCase):
    """Positive and negative controls for the image contents and apt policy checkers."""

    def clean_paths(self):
        paths = set(RequiredFiles)
        for component in m1_support.Cham3PlasmoidComponents:
            paths.add("usr/share/plasma/plasmoids/%s%s/metadata.json" % (m1_support.Cham3Namespace, component))
        return paths

    def clean_installed(self):
        return {m1_support.PackageDesktop, m1_support.PackageFull}

    def test_positive_control_accepted(self):
        self.assertEqual(contents_problems(self.clean_paths(), self.clean_installed()), [])

    def test_shipped_backend_binary_is_caught(self):
        paths = self.clean_paths() | {"usr/bin/cham3-netd"}
        problems = contents_problems(paths, self.clean_installed())
        self.assertTrue(any("absent backend binary" in p for p in problems), problems)

    def test_shipped_systemd_unit_is_caught(self):
        paths = self.clean_paths() | {"usr/lib/systemd/system/cham3-vaultd.service"}
        problems = contents_problems(paths, self.clean_installed())
        self.assertTrue(any("systemd unit" in p for p in problems), problems)

    def test_missing_plasmoid_is_caught(self):
        paths = self.clean_paths() - {"usr/share/plasma/plasmoids/org.cham3.killswitch/metadata.json"}
        problems = contents_problems(paths, self.clean_installed())
        self.assertTrue(any("org.cham3.killswitch" in p for p in problems), problems)

    def test_missing_metapackage_is_caught(self):
        problems = contents_problems(self.clean_paths(), {m1_support.PackageDesktop})
        self.assertTrue(any("cham3-full" in p for p in problems), problems)

    def test_missing_real_session_file_is_caught(self):
        paths = self.clean_paths() - {PlasmaSessionFile}
        problems = contents_problems(paths, self.clean_installed())
        self.assertTrue(any(PlasmaSessionFile in p for p in problems), problems)

    def test_missing_real_wallpaper_is_caught(self):
        paths = self.clean_paths() - {WallpaperImage}
        problems = contents_problems(paths, self.clean_installed())
        self.assertTrue(any(WallpaperImage in p for p in problems), problems)

    def test_missing_wallpaper_metadata_is_caught(self):
        paths = self.clean_paths() - {WallpaperMetadata}
        problems = contents_problems(paths, self.clean_installed())
        self.assertTrue(any(WallpaperMetadata in p for p in problems), problems)

    def test_superseded_names_do_not_satisfy_the_gate(self):
        # The names an earlier fixture used. Only those present: both real files are missing.
        paths = (self.clean_paths() - {PlasmaSessionFile, WallpaperImage}) | {
            "usr/share/wayland-sessions/plasmawayland.desktop",
            "usr/share/wallpapers/Cham3/contents/images/cham3_minimal_charcoal.png",
        }
        problems = contents_problems(paths, self.clean_installed())
        self.assertTrue(any(PlasmaSessionFile in p for p in problems), problems)
        self.assertTrue(any(WallpaperImage in p for p in problems), problems)


class image_apt_policy_fixture_t(unittest.TestCase):
    """F2 image apt policy controls (no ISO needed)."""

    KaliSource = (
        "Types: deb\nURIs: http://http.kali.org/kali/\nSuites: kali-rolling\n"
        "Components: main\nSigned-By: /usr/share/keyrings/cham3_kali_archive.gpg\n"
    )
    LibreWolfSource = (
        "Types: deb\nURIs: https://repo.librewolf.net\nSuites: librewolf\nComponents: main\n"
        "Architectures: amd64\nSigned-By: /usr/share/keyrings/cham3_librewolf.gpg\n"
    )
    Keyrings = {"cham3_kali_archive.gpg", "cham3_librewolf.gpg"}

    def pinned(self):
        pref = "Package: *\nPin: origin \"http.kali.org\"\nPin-Priority: -1\n"
        for package in m1_support.KaliAllowlist:
            pref += "\nPackage: %s\nPin: origin \"http.kali.org\"\nPin-Priority: 500\n" % package
        lw_pref = (
            "Package: *\nPin: origin \"repo.librewolf.net\"\nPin-Priority: -1\n\n"
            "Package: librewolf\nPin: origin \"repo.librewolf.net\"\nPin-Priority: 500\n"
        )
        sources = {
            "etc/apt/sources.list.d/cham3_kali.sources": apt.parse_sources(self.KaliSource),
            "etc/apt/sources.list.d/cham3_librewolf.sources": apt.parse_sources(self.LibreWolfSource),
        }
        prefs = {
            "etc/apt/preferences.d/cham3_kali.pref": apt.parse_preferences(pref),
            "etc/apt/preferences.d/cham3_librewolf.pref": apt.parse_preferences(lw_pref),
        }
        return sources, prefs

    def test_pinned_foreign_sources_pass_the_image_policy(self):
        sources, prefs = self.pinned()
        self.assertEqual(apt_policy_problems(sources, prefs, self.Keyrings, []), [])

    def test_unpinned_kali_source_fails_the_image_policy(self):
        sources, _ = self.pinned()
        problems = apt_policy_problems(sources, {}, self.Keyrings, [])
        self.assertTrue(any("no default deny" in p and "kali" in p for p in problems), problems)

    def test_kali_key_in_trusted_gpg_d_fails_the_image_policy(self):
        sources, prefs = self.pinned()
        problems = apt_policy_problems(sources, prefs, self.Keyrings, ["etc/apt/trusted.gpg.d/kali.gpg"])
        self.assertTrue(any("apt-key or trusted.gpg.d" in p for p in problems), problems)

    def test_release_field_pins_fail_the_image_policy(self):
        # Negative control (Codex F6): the 0fec3a0 pin form in an image.
        sources, prefs = self.pinned()
        prefs["etc/apt/preferences.d/cham3_kali.pref"] = apt.parse_preferences(
            "Package: *\nPin: release o=Kali\nPin-Priority: -1\n")
        problems = apt_policy_problems(sources, prefs, self.Keyrings, [])
        self.assertTrue(any("bound to a field of the signed Release file" in p for p in problems), problems)

    DebianList = ("/.\n/etc\n/etc/apt\n/etc/apt/trusted.gpg.d\n"
                  "/etc/apt/trusted.gpg.d/debian-archive-trixie-stable.asc\n"
                  "/usr/share/keyrings/debian-archive-keyring.pgp\n")

    def test_debian_owned_archive_keys_are_accepted(self):
        owned = debian_owned_trust_files(self.DebianList)
        self.assertEqual(owned, {"etc/apt/trusted.gpg.d/debian-archive-trixie-stable.asc"})
        sources, prefs = self.pinned()
        keys = ["etc/apt/trusted.gpg.d/debian-archive-trixie-stable.asc"]
        self.assertEqual(apt_policy_problems(sources, prefs, self.Keyrings, keys, owned), [])

    def test_unowned_key_named_like_debian_is_rejected(self):
        owned = debian_owned_trust_files(self.DebianList)
        keys = ["etc/apt/trusted.gpg.d/debian-archive-kali-masquerade.asc", "etc/apt/trusted.gpg.d/kali.gpg"]
        self.assertEqual(foreign_trust_files(keys, owned), keys)

    def test_apt_inputs_classify_files_by_location_not_extension(self):
        paths = {"etc/apt/sources.list.d/cham3_kali.sources", "etc/apt/preferences.d/cham3_kali.pref",
                 "usr/share/keyrings/cham3_kali_archive.gpg", "etc/apt/trusted.gpg.d/extra.gpg"}
        texts = {
            "etc/apt/sources.list.d/cham3_kali.sources": self.KaliSource,
            "etc/apt/preferences.d/cham3_kali.pref": "Package: *\nPin: origin \"http.kali.org\"\nPin-Priority: -1\n",
        }
        sources, prefs, keyrings, keys = apt_inputs(paths, texts.__getitem__)
        self.assertEqual(sorted(sources), ["etc/apt/sources.list.d/cham3_kali.sources"])
        self.assertEqual(sorted(prefs), ["etc/apt/preferences.d/cham3_kali.pref"])
        self.assertEqual(keyrings, {"cham3_kali_archive.gpg"})
        self.assertEqual(keys, ["etc/apt/trusted.gpg.d/extra.gpg"])


class image_trust_fixture_t(unittest.TestCase):
    """Controls for the final-image trust and keyring checks (no ISO needed)."""

    def test_trusted_yes_in_the_final_image_is_rejected(self):
        sources = {"etc/apt/sources.list.d/x.list": apt.parse_sources("deb [trusted=yes] http://deb.debian.org/debian trixie main\n")}
        self.assertEqual(image_trusted_sources(sources), ["etc/apt/sources.list.d/x.list"])

    def test_plain_signed_sources_are_not_trusted_yes(self):
        sources = {"etc/apt/sources.list.d/x.list": apt.parse_sources("deb [signed-by=/usr/share/keyrings/x.gpg] http://deb.debian.org/debian trixie main\n")}
        self.assertEqual(image_trusted_sources(sources), [])

    def test_missing_shipped_keyring_is_rejected(self):
        problems = image_keyring_problems({"cham3_kali_archive.gpg"}, {"cham3_kali_archive.gpg", "cham3_librewolf.gpg"})
        self.assertEqual(problems, ["shipped keyring missing from the image: cham3_librewolf.gpg"])

    def test_all_shipped_keyrings_present_is_accepted(self):
        self.assertEqual(image_keyring_problems({"cham3_kali_archive.gpg", "cham3_librewolf.gpg"},
                                                {"cham3_kali_archive.gpg", "cham3_librewolf.gpg"}), [])


class image_origin_fixture_t(unittest.TestCase):
    """F2 origin controls: allowlisted foreign origin passes; anything else fails closed."""

    Debian = "Package: nmap\nVersion: 7.95-1\n\n"
    Kali = "Package: zaproxy\nVersion: 2025.1-1\n\nPackage: kali-linux-large\nVersion: 2025.1\n\n"

    def indexes(self, kali=None, debian=None):
        return {
            "deb.debian.org_debian_dists_trixie_main_binary-amd64_Packages": self.Debian if debian is None else debian,
            "http.kali.org_kali_dists_kali-rolling_main_binary-amd64_Packages": self.Kali if kali is None else kali,
        }

    def test_allowlisted_kali_origin_package_is_accepted(self):
        status = {("nmap", "7.95-1"), ("zaproxy", "2025.1-1"), ("cham3-desktop", "0.0.0~m1")}
        self.assertEqual(image_origin_problems(status, self.indexes(), True), [])

    def test_non_allowlisted_kali_origin_package_is_rejected(self):
        status = {("nmap", "7.95-1"), ("kali-linux-large", "2025.1")}
        problems = image_origin_problems(status, self.indexes(), True)
        self.assertTrue(any("un-allowlisted kali-origin package installed: kali-linux-large" in p for p in problems), problems)

    def test_unknown_origin_fails_closed(self):
        status = {("nmap", "7.95-1"), ("mystery", "1.0")}
        problems = image_origin_problems(status, self.indexes(), True)
        self.assertTrue(any("origin cannot be established for mystery" in p for p in problems), problems)

    def test_missing_debian_index_fails_closed(self):
        problems = image_origin_problems({("nmap", "7.95-1")}, {"x_kali_Packages": self.Kali}, True)
        self.assertTrue(any("no Debian package index" in p for p in problems), problems)

    def test_missing_kali_index_fails_closed_when_kali_is_configured(self):
        status = {("nmap", "7.95-1")}
        index = {"deb.debian.org_debian_dists_trixie_main_binary-amd64_Packages": self.Debian}
        problems = image_origin_problems(status, index, True)
        self.assertTrue(any("no Kali index" in p for p in problems), problems)


if __name__ == "__main__":
    unittest.main()
