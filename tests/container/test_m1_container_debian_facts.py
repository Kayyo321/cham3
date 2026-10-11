"""Container-stage checks that the Debian packages match what the candidate assumes (F4, F5, F3).

The static suites assume:
  - the Plasma Wayland session file is usr/share/wayland-sessions/plasma.desktop
    (F4: the image gate and the SDDM component both use this name);
  - the Plasma-private QML modules the launcher and session actions import are
    shipped by a package that cham3-desktop depends on (F5);
  - /usr/bin/dumpcap is shipped by wireshark-common (F3).

This module downloads the real packages with apt-get download (no install) in a
debian:trixie container and checks those assumptions against their file lists.
It needs apt network access; a failed download is BLOCKED, reported as a FAIL.
The fixture controls at the bottom run without the network.
"""

import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402
import m1_support  # noqa: E402

DownloadPackages = ("plasma-workspace", "plasma-desktop", "wireshark-common")
PrivateModuleImport = re.compile(r"^\s*import\s+org\.kde\.plasma\.private\.([a-z0-9_]+)", re.MULTILINE)
SessionSelection = re.compile(r"Session=([A-Za-z0-9_\-]+\.desktop)\b")
DesktopOwners = ("plasma-workspace", "plasma-desktop")


def listing_of(deb_path):
    """Set of member paths of a .deb, without the leading './'."""
    completed = subprocess.run(
        ["dpkg-deb", "-c", deb_path], capture_output=True, text=True, check=False
    )
    if completed.returncode != 0:
        raise AssertionError("dpkg-deb -c failed for %s: %s" % (deb_path, completed.stderr.strip()))
    paths = set()
    for line in completed.stdout.splitlines():
        fields = line.split()
        if fields:
            member = fields[-1]
            if member.startswith("./"):
                member = member[2:]
            paths.add(member.rstrip("/"))
    return paths


def qmldir_suffix(module_name):
    """The member path that a Plasma-private QML module ships its qmldir at (any multiarch dir)."""
    return "qt6/qml/org/kde/plasma/private/%s/qmldir" % module_name


def providers(listings, suffix):
    """Names of the packages whose listing ends with suffix."""
    return sorted(name for name, paths in listings.items() if any(p.endswith(suffix) for p in paths))


def private_modules_imported(qml_texts):
    names = set()
    for text in qml_texts:
        names.update(PrivateModuleImport.findall(text))
    return sorted(names)


def facts_problems(listings, imported_modules, session_name, cham3_depends):
    """Problems where the candidate's assumptions disagree with the Debian file lists."""
    problems = []
    session_path = m1_support.PackagedPlasmaSession
    if session_path not in listings.get("plasma-workspace", set()):
        problems.append("plasma-workspace does not ship " + session_path)
    if os.path.basename(session_path) != session_name:
        problems.append("the selected session %s does not match the shipped file %s" % (session_name, session_path))
    if "usr/bin/dumpcap" not in listings.get("wireshark-common", set()):
        problems.append("wireshark-common does not ship usr/bin/dumpcap")
    for module in imported_modules:
        suffix = qmldir_suffix(module)
        owners = providers(listings, suffix)
        if not owners:
            problems.append("no downloaded package ships the QML module org.kde.plasma.private." + module)
        elif not set(owners) & set(cham3_depends):
            problems.append(
                "org.kde.plasma.private.%s is shipped by %s, not by a cham3-desktop dependency"
                % (module, ", ".join(owners))
            )
    return problems


def download(scratch):
    """apt-get download the packages into scratch and return {package: deb path}."""
    completed = subprocess.run(
        ["apt-get", "download"] + list(DownloadPackages),
        cwd=scratch,
        capture_output=True,
        text=True,
        check=False,
        timeout=900,
    )
    if completed.returncode != 0:
        raise AssertionError(
            "BLOCKED: apt-get download failed (network or package index): " + completed.stderr.strip()
        )
    debs = {}
    for name in DownloadPackages:
        matches = [f for f in os.listdir(scratch) if f.startswith(name + "_") and f.endswith(".deb")]
        if len(matches) != 1:
            raise AssertionError("expected one downloaded %s .deb, found %d" % (name, len(matches)))
        debs[name] = os.path.join(scratch, matches[0])
    return debs


def cham3_desktop_depends():
    stanza = m1_support.package_control(m1_support.PackageDesktopDir, m1_support.PackageDesktop)
    groups, _ = m1_support.relation_groups(stanza.get("Depends", ""))
    return sorted({alt for group in groups for alt in group})


class debian_facts_t(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.scratch = tempfile.mkdtemp(prefix="cham3_debian_facts_")
        try:
            cls.debs = download(cls.scratch)
            cls.listings = {name: listing_of(path) for name, path in cls.debs.items()}
        except AssertionError:
            shutil.rmtree(cls.scratch, ignore_errors=True)
            raise

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.scratch, ignore_errors=True)

    def qml_texts(self):
        texts = []
        for directory in m1_support.plasmoid_dirs("org.cham3.user_launcher"):
            for relative in ("contents/ui/main.qml", "contents/ui/session_actions.qml"):
                path = os.path.join(directory, relative)
                if os.path.isfile(path):
                    with open(path, "r", encoding="utf-8") as handle:
                        texts.append(handle.read())
        return texts

    def test_plasma_wayland_session_file_is_shipped(self):
        self.assertIn(m1_support.PackagedPlasmaSession, self.listings["plasma-workspace"])

    def test_sddm_selection_matches_the_shipped_session(self):
        hook = m1_support.read_repo(m1_support.SddmSessionHook)
        selections = set(SessionSelection.findall(hook))
        self.assertEqual(selections, {os.path.basename(m1_support.PackagedPlasmaSession)})

    def test_launcher_private_modules_are_shipped_by_cham3_desktop_dependencies(self):
        modules = private_modules_imported(self.qml_texts())
        self.assertTrue(modules, "the launcher imports no Plasma-private module to check")
        problems = facts_problems(self.listings, modules, os.path.basename(m1_support.PackagedPlasmaSession),
                                  cham3_desktop_depends())
        self.assertEqual(problems, [])

    def test_dumpcap_is_shipped_by_wireshark_common(self):
        self.assertIn("usr/bin/dumpcap", self.listings["wireshark-common"])


class debian_facts_fixture_t(unittest.TestCase):
    """Positive and negative controls for facts_problems() (no network)."""

    def listings(self):
        return {
            "plasma-workspace": {
                m1_support.PackagedPlasmaSession,
                "usr/lib/x86_64-linux-gnu/qt6/qml/org/kde/plasma/private/kicker/qmldir",
                "usr/lib/x86_64-linux-gnu/qt6/qml/org/kde/plasma/private/sessions/qmldir",
            },
            "plasma-desktop": {"usr/share/applications/org.kde.dolphin.desktop"},
            "wireshark-common": {"usr/bin/dumpcap"},
        }

    def test_positive_control_accepted(self):
        problems = facts_problems(
            self.listings(), ["kicker", "sessions"], "plasma.desktop",
            ["plasma-workspace", "plasma-desktop", "wireshark-common"],
        )
        self.assertEqual(problems, [])

    def test_module_missing_from_every_package_is_rejected(self):
        problems = facts_problems(self.listings(), ["kickoff"], "plasma.desktop", ["plasma-workspace"])
        self.assertTrue(any("no downloaded package ships" in p for p in problems), problems)

    def test_module_from_a_non_dependency_is_rejected(self):
        problems = facts_problems(self.listings(), ["kicker"], "plasma.desktop", ["plasma-desktop"])
        self.assertTrue(any("not by a cham3-desktop dependency" in p for p in problems), problems)

    def test_superseded_session_name_is_rejected(self):
        problems = facts_problems(self.listings(), [], "plasmawayland.desktop", ["plasma-workspace"])
        self.assertTrue(any("does not match" in p for p in problems), problems)

    def test_missing_dumpcap_is_rejected(self):
        listings = self.listings()
        listings["wireshark-common"] = set()
        problems = facts_problems(listings, [], "plasma.desktop", ["plasma-workspace"])
        self.assertTrue(any("dumpcap" in p for p in problems), problems)

    def test_private_module_extraction(self):
        text = "import org.kde.plasma.private.kicker as Kicker\nimport org.kde.plasma.private.sessions\n"
        self.assertEqual(private_modules_imported([text]), ["kicker", "sessions"])


if __name__ == "__main__":
    unittest.main()
