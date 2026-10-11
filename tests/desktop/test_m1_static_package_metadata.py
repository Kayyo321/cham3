"""Static checks for the M1 metapackages: cham3-desktop and cham3-full.

Control-file, changelog and copyright checks are pure text parsing, so they run
without dpkg. control_problems() and the other *_problems() functions are the
single checkers. The real-tree classes run them on packaging/ and FAIL when a
package is missing. The fixture classes run them on synthetic text: a
well-formed positive control must be accepted and each deliberate defect must
be reported (negative controls).
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

ExpectedSection = {
    m1_support.PackageDesktop: "kde",
    m1_support.PackageFull: "metapackages",
}
ExtraSourcesRequired = frozenset({"desktop", "assets"})
ForbiddenPackagingSuffixes = (".service", ".timer", ".socket", ".path", ".target")
ForbiddenPackagingNames = frozenset({"conffiles"})
ChangelogHead = re.compile(
    r"^(?P<name>[a-z0-9][a-z0-9+.\-]+) \((?P<version>[^)\s]+)\) (?P<dist>UNRELEASED|trixie); "
    r"urgency=(low|medium|high|critical|emergency)$"
)
ChangelogTrailer = re.compile(r"^ -- .+ <[^>]+>  .+$", re.MULTILINE)
CopyrightFormat = re.compile(
    r"Format:\s*https://www\.debian\.org/doc/packaging-manuals/copyright-format/1\.0/?"
)


def control_problems(package, text):
    """Return problems with a debian/control text for package (empty means accepted)."""
    problems = []
    try:
        stanzas = m0_support.parse_control(text)
    except AssertionError as error:
        return ["control does not parse: %s" % error]
    binaries = [stanza for stanza in stanzas if "Package" in stanza]
    if len(stanzas) != 2 or len(binaries) != 1:
        problems.append(
            "control must have one source and one binary stanza (found %d stanzas)" % len(stanzas)
        )
        if not binaries:
            return problems
    source = stanzas[0]
    binary = binaries[0]
    if source.get("Source") != package:
        problems.append("Source must be %s, found %r" % (package, source.get("Source")))
    if source.get("Rules-Requires-Root") != "no":
        problems.append("Rules-Requires-Root must be no")
    # Section and Priority may be set on the source stanza, the binary stanza, or both;
    # the binary stanza wins when both are present.
    section = binary.get("Section", source.get("Section", ""))
    priority = binary.get("Priority", source.get("Priority", ""))
    for field in ("Package", "Architecture", "Description", "Depends"):
        if not binary.get(field, "").strip():
            problems.append("missing or empty field: " + field)
    if not section.strip():
        problems.append("missing or empty field: Section")
    if not priority.strip():
        problems.append("missing or empty field: Priority")
    if not source.get("Maintainer", "").strip():
        problems.append("missing Maintainer in source stanza")
    if binary.get("Package") != package:
        problems.append("Package must be %s" % package)
    if binary.get("Architecture") != "all":
        problems.append("Architecture must be all")
    if section != ExpectedSection.get(package):
        problems.append("Section must be %s" % ExpectedSection.get(package))
    if priority != "optional":
        problems.append("Priority must be optional")
    if "Pre-Depends" in binary:
        problems.append("Pre-Depends must be absent")
    addresses = re.findall(r"<([^>]+)>", source.get("Maintainer", ""))
    if not addresses or not all(m0_support.email_domain_is_reserved(a) for a in addresses):
        problems.append("Maintainer is not a reserved placeholder address")
    groups, relation_problems = m1_support.relation_groups(binary.get("Depends", ""))
    problems.extend(relation_problems)
    if not groups:
        problems.append("Depends has no relationships")
    names = m1_support.relation_names(binary.get("Depends", ""))
    if package in names:
        problems.append("self dependency on " + package)
    return problems


def dependency_graph_problems(desktop_text, full_text):
    """Check cham3-full -> cham3-desktop only, with no cycle."""
    problems = []
    desktop_names = m1_support.relation_names(
        m1_support.binary_stanza(desktop_text, m1_support.PackageDesktop).get("Depends", "")
    )
    full_names = m1_support.relation_names(
        m1_support.binary_stanza(full_text, m1_support.PackageFull).get("Depends", "")
    )
    if m1_support.PackageDesktop not in full_names:
        problems.append("cham3-full must Depends on cham3-desktop")
    if m1_support.PackageFull in desktop_names:
        problems.append("circular dependency: cham3-desktop depends on cham3-full")
    return problems


def changelog_problems(package, text):
    problems = []
    first = text.split("\n", 1)[0]
    match = ChangelogHead.match(first)
    if not match:
        return ["changelog first line does not parse: %r" % first]
    if match.group("name") != package:
        problems.append("changelog package name must be %s" % package)
    if not ChangelogTrailer.search(text):
        problems.append("changelog has no maintainer trailer line")
    for address in re.findall(r"[\w.+-]+@[\w.-]+", text):
        if not m0_support.email_domain_is_reserved(address):
            problems.append("changelog contains a non-reserved address")
    return problems


def copyright_problems(text):
    problems = []
    if not CopyrightFormat.search(text):
        problems.append("copyright lacks DEP-5 Format line")
    if not re.search(r"(?m)^License:", text):
        problems.append("copyright has no License field")
    if not re.search(r"(?m)^Files:", text):
        problems.append("copyright has no Files stanza")
    return problems


def packaging_content_problems(package_dir, file_names, install_texts):
    """Reject services, timers and conffiles, and install paths under /etc or systemd."""
    problems = []
    for name in file_names:
        if name in ForbiddenPackagingNames or name.endswith(ForbiddenPackagingSuffixes):
            problems.append("%s ships forbidden packaging file %s" % (package_dir, name))
    for name, text in install_texts.items():
        for line in text.splitlines():
            entry = line.strip()
            if not entry or entry.startswith("#"):
                continue
            if entry.startswith(("etc/", "lib/systemd", "usr/lib/systemd")):
                problems.append("%s installs into a forbidden path: %s" % (name, entry))
    return problems


def extra_sources_problems(text):
    problems = []
    entries = set()
    for line in text.splitlines():
        entry = line.strip()
        if not entry or entry.startswith("#"):
            continue
        if entry.startswith("/") or ".." in entry:
            problems.append("unsafe cham3-extra-sources entry: " + entry)
        entries.add(entry)
    missing = sorted(ExtraSourcesRequired - entries)
    if missing:
        problems.append("cham3-extra-sources lacks: " + ", ".join(missing))
    return problems


def package_debian_texts(package_dir):
    """Return (file_names, {name: text}) for files directly under debian/."""
    debian = os.path.join(m0_support.repo_root(), package_dir, "debian")
    if not os.path.isdir(debian):
        raise AssertionError("required directory missing: %s/debian" % package_dir)
    names = sorted(os.listdir(debian))
    install_texts = {}
    for name in names:
        if name.endswith(".install"):
            install_texts[name] = m1_support.read_repo(package_dir + "/debian/" + name)
    return names, install_texts


class package_metadata_real_t(unittest.TestCase):
    def test_cham3_desktop_control_is_valid(self):
        text = m1_support.read_repo(m1_support.PackageDesktopDir + "/debian/control")
        self.assertEqual(control_problems(m1_support.PackageDesktop, text), [])

    def test_cham3_full_control_is_valid(self):
        text = m1_support.read_repo(m1_support.PackageFullDir + "/debian/control")
        self.assertEqual(control_problems(m1_support.PackageFull, text), [])

    def test_dependency_graph_is_acyclic_and_ordered(self):
        self.assertEqual(
            dependency_graph_problems(
                m1_support.read_repo(m1_support.PackageDesktopDir + "/debian/control"),
                m1_support.read_repo(m1_support.PackageFullDir + "/debian/control"),
            ),
            [],
        )

    def test_cham3_desktop_keeps_kde_core_utilities(self):
        names = m1_support.relation_names(
            m1_support.package_control(m1_support.PackageDesktopDir, m1_support.PackageDesktop).get("Depends", "")
        )
        missing = sorted(set(m1_support.KdeCoreRequired) - names)
        self.assertEqual(missing, [], "KDE core utilities missing from cham3-desktop Depends")

    def test_cham3_desktop_recommends_fonts_and_plasma6_qqc2_style(self):
        # The contract name qqc2-desktop-style is ABSENT from trixie (verified with
        # apt-cache in debian:trixie); the Plasma 6 (KF6) style module is
        # qml6-module-org-kde-desktop (6.13.0). See internal finding F-qqc2.
        stanza = m1_support.package_control(m1_support.PackageDesktopDir, m1_support.PackageDesktop)
        names = m1_support.relation_names(stanza.get("Recommends", ""))
        self.assertTrue(any(name.startswith("fonts-noto") for name in names), "no fonts-noto recommendation")
        self.assertIn("qml6-module-org-kde-desktop", names)

    def test_no_alternative_desktop_or_task_metapackage(self):
        for package_dir, package in (
            (m1_support.PackageDesktopDir, m1_support.PackageDesktop),
            (m1_support.PackageFullDir, m1_support.PackageFull),
        ):
            stanza = m1_support.package_control(package_dir, package)
            names = set()
            for field in ("Depends", "Recommends", "Suggests"):
                names |= m1_support.relation_names(stanza.get(field, ""))
            present = sorted(names & set(m1_support.DesktopConflictNames))
            self.assertEqual(present, [], "%s pulls a conflicting desktop: %s" % (package, present))

    def test_changelogs_parse_and_name_their_package(self):
        for package_dir, package in (
            (m1_support.PackageDesktopDir, m1_support.PackageDesktop),
            (m1_support.PackageFullDir, m1_support.PackageFull),
        ):
            text = m1_support.read_repo(package_dir + "/debian/changelog")
            self.assertEqual(changelog_problems(package, text), [])

    def test_copyright_present_for_each_package(self):
        for package_dir in (m1_support.PackageDesktopDir, m1_support.PackageFullDir):
            text = m1_support.read_repo(package_dir + "/debian/copyright")
            self.assertEqual(copyright_problems(text), [], package_dir)

    def test_no_services_timers_conffiles_or_system_installs(self):
        problems = []
        for package_dir in (m1_support.PackageDesktopDir, m1_support.PackageFullDir):
            names, install_texts = package_debian_texts(package_dir)
            problems.extend(packaging_content_problems(package_dir, names, install_texts))
        self.assertEqual(problems, [])

    def test_cham3_desktop_extra_sources_list_desktop_and_assets(self):
        text = m1_support.read_repo(m1_support.PackageDesktopDir + "/debian/cham3-extra-sources")
        self.assertEqual(extra_sources_problems(text), [])


class package_metadata_fixture_t(unittest.TestCase):
    """Positive and negative controls for the metadata checkers."""

    def good_desktop(self):
        return (
            "Source: cham3-desktop\n"
            "Section: kde\n"
            "Priority: optional\n"
            "Maintainer: cham3 developers <maintainers@cham3.invalid>\n"
            "Build-Depends: debhelper-compat (= 13)\n"
            "Rules-Requires-Root: no\n"
            "\n"
            "Package: cham3-desktop\n"
            "Architecture: all\n"
            "Depends: plasma-desktop, plasma-workspace-wayland, kwin-wayland, sddm,\n"
            " systemsettings, dolphin, konsole, kate, ark, okular, gwenview,\n"
            " kde-spectacle, plasma-systemmonitor\n"
            "Recommends: fonts-noto, qml6-module-org-kde-desktop\n"
            "Description: cham3 KDE customization\n"
            " Visual foundation only.\n"
        )

    def good_full(self):
        return (
            "Source: cham3-full\n"
            "Section: metapackages\n"
            "Priority: optional\n"
            "Maintainer: cham3 developers <maintainers@cham3.invalid>\n"
            "Rules-Requires-Root: no\n"
            "\n"
            "Package: cham3-full\n"
            "Architecture: all\n"
            "Depends: cham3-desktop, nmap\n"
            "Description: cham3 complete desktop\n"
            " Metapackage.\n"
        )

    def test_well_formed_desktop_control_is_accepted(self):
        self.assertEqual(control_problems(m1_support.PackageDesktop, self.good_desktop()), [])

    def test_well_formed_full_control_is_accepted(self):
        self.assertEqual(control_problems(m1_support.PackageFull, self.good_full()), [])

    def test_self_dependency_is_reported(self):
        text = self.good_full().replace("Depends: cham3-desktop, nmap", "Depends: cham3-full, nmap")
        problems = control_problems(m1_support.PackageFull, text)
        self.assertTrue(any("self dependency" in p for p in problems), problems)

    def test_circular_dependency_is_reported(self):
        desktop = self.good_desktop().replace(
            "Recommends: fonts-noto", "Depends: cham3-full, plasma-desktop\nRecommends: fonts-noto"
        )
        problems = dependency_graph_problems(desktop, self.good_full())
        self.assertTrue(any("circular" in p for p in problems), problems)

    def test_missing_full_dependency_on_desktop_is_reported(self):
        full = self.good_full().replace("cham3-desktop, nmap", "nmap")
        problems = dependency_graph_problems(self.good_desktop(), full)
        self.assertTrue(any("must Depends on cham3-desktop" in p for p in problems), problems)

    def test_malformed_relationship_is_reported(self):
        text = self.good_desktop().replace("ark, okular", "ark, , okular").replace("kate, ark", "kate, Ark!")
        problems = control_problems(m1_support.PackageDesktop, text)
        self.assertTrue(any("empty clause" in p for p in problems), problems)
        self.assertTrue(any("malformed package reference" in p for p in problems), problems)

    def test_empty_depends_is_reported(self):
        text = self.good_full().replace("Depends: cham3-desktop, nmap", "Depends:")
        problems = control_problems(m1_support.PackageFull, text)
        self.assertTrue(any("Depends" in p for p in problems), problems)

    def test_wrong_architecture_is_reported(self):
        text = self.good_full().replace("Architecture: all", "Architecture: any")
        problems = control_problems(m1_support.PackageFull, text)
        self.assertIn("Architecture must be all", problems)

    def test_wrong_section_is_reported(self):
        text = self.good_desktop().replace("Section: kde", "Section: x11")
        problems = control_problems(m1_support.PackageDesktop, text)
        self.assertIn("Section must be kde", problems)

    def test_non_reserved_maintainer_is_reported(self):
        text = self.good_full().replace("maintainers@cham3.invalid", "dev@cham3-real-domain.com")
        problems = control_problems(m1_support.PackageFull, text)
        self.assertIn("Maintainer is not a reserved placeholder address", problems)

    def test_changelog_fixture_accepts_and_rejects(self):
        good = "cham3-full (0.0.0~m1) UNRELEASED; urgency=medium\n\n  * Initial.\n\n -- cham3 developers <maintainers@cham3.invalid>  Sun, 11 Oct 2026 00:00:00 +0000\n"
        bad = good.replace("cham3-full (0.0.0~m1)", "cham3-other (0.0.0~m1)")
        self.assertEqual(changelog_problems(m1_support.PackageFull, good), [])
        self.assertTrue(changelog_problems(m1_support.PackageFull, bad))
        self.assertTrue(changelog_problems(m1_support.PackageFull, "not a changelog\n"))

    def test_forbidden_service_and_conffile_are_reported(self):
        problems = packaging_content_problems(
            "packaging/cham3-desktop",
            ["conffiles", "cham3-netd.service", "debian.install"],
            {"debian.install": "etc/cham3/defaults.conf\n"},
        )
        self.assertTrue(any("conffiles" in p for p in problems), problems)
        self.assertTrue(any("cham3-netd.service" in p for p in problems), problems)
        self.assertTrue(any("forbidden path" in p for p in problems), problems)

    def test_extra_sources_positive_and_negative(self):
        self.assertEqual(extra_sources_problems("# c\ndesktop\nassets\n"), [])
        self.assertTrue(extra_sources_problems("desktop\n"))
        self.assertTrue(any("unsafe" in p for p in extra_sources_problems("desktop\nassets\n/etc\n")))
        self.assertTrue(any("unsafe" in p for p in extra_sources_problems("desktop\nassets\n../secret\n")))


if __name__ == "__main__":
    unittest.main()
