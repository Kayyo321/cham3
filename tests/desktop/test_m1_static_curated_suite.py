"""Static checks for the M1 curated suite, duplicate exclusions and gated tools.

Inputs are the cham3-desktop and cham3-full control files plus the M1 live
config tree. Results here are static: they show what the metapackages request,
not what the built image contains. The image manifest check for transitive
exclusion is in test_m1_image_manifest.py (VM/container stage).
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

RelationFields = ("Depends", "Recommends", "Suggests", "Pre-Depends")
KaliSourcePattern = re.compile(r"(?i)(https?://[^\s\"']*kali[^\s\"']*|kali-rolling|kali\.org)")
# F2: Kali and LibreWolf sources are allowed only in the pinned archive files, whose
# policy is checked by test_m1_static_foreign_sources.py.
PinnedForeignArchives = (
    m1_support.ArchivesDir + "/cham3_kali.",
    m1_support.ArchivesDir + "/cham3_librewolf.",
)


def relation_fields(stanza):
    names = {}
    for field in RelationFields:
        names[field] = m1_support.relation_names(stanza.get(field, ""))
    return names


def forbidden_in_relations(stanza):
    """Return (category, name, field) for every forbidden name in relationship fields."""
    found = []
    categories = (
        ("duplicate utility", m1_support.ExcludedDuplicates),
        ("gated tool", m1_support.GatedTools),
        ("desktop conflict", m1_support.DesktopConflictNames),
    )
    for field, names in relation_fields(stanza).items():
        for category, forbidden in categories:
            for name in sorted(names & set(forbidden)):
                found.append((category, name, field))
    return found


def curated_problems(full_names, documented):
    """Check the curated suite against cham3-full names.

    documented(name) reports whether the package documents an omitted soft tool.
    """
    problems = []
    for name in m1_support.CuratedRequired:
        if name not in full_names:
            problems.append("curated tool missing from cham3-full: " + name)
    for name in m1_support.CuratedDocumentedGaps:
        if name not in full_names and not documented(name):
            problems.append("acceptance gap silently omitted (not documented): " + name)
    return problems


def kali_reference_problems(texts):
    """texts maps a repo path to its text. Kali URLs or suites are allowed only in the pinned archive files."""
    problems = []
    for path, text in texts.items():
        if path.startswith(PinnedForeignArchives):
            continue
        for match in KaliSourcePattern.finditer(text):
            problems.append("Kali source reference in %s: %s" % (path, match.group(0)))
    return problems


def text_of_tree(relative_root):
    """Read every file under a repo-relative directory, skipping control descriptions."""
    root = os.path.join(m0_support.repo_root(), relative_root)
    texts = {}
    if not os.path.isdir(root):
        raise AssertionError("required directory missing: " + relative_root)
    for dirpath, _, filenames in os.walk(root):
        for filename in filenames:
            path = os.path.join(dirpath, filename)
            with open(path, "r", encoding="utf-8", errors="replace") as handle:
                text = handle.read()
            if filename == "control":
                text = re.sub(r"(?m)^Description:.*(\n[ \t].*)*", "", text)
            texts[os.path.relpath(path, m0_support.repo_root())] = text
    return texts


def full_names():
    stanza = m1_support.package_control(m1_support.PackageFullDir, m1_support.PackageFull)
    names = set()
    for field in ("Depends", "Recommends"):
        names |= m1_support.relation_names(stanza.get(field, ""))
    return names


def documented_in_full(name):
    return m1_support.documented_omission(m1_support.PackageFullDir, name)


class curated_suite_real_t(unittest.TestCase):
    def test_cham3_full_includes_curated_required_tools(self):
        self.assertEqual(curated_problems(full_names(), documented_in_full), [])

    def test_acceptance_gaps_are_documented_not_silent(self):
        missing = [
            name
            for name in m1_support.CuratedDocumentedGaps
            if name not in full_names() and not documented_in_full(name)
        ]
        self.assertEqual(missing, [], "absent-from-trixie tools must be documented in cham3-full")

    def test_duplicate_utilities_are_excluded_from_metapackages(self):
        problems = []
        for package_dir, package in (
            (m1_support.PackageDesktopDir, m1_support.PackageDesktop),
            (m1_support.PackageFullDir, m1_support.PackageFull),
        ):
            stanza = m1_support.package_control(package_dir, package)
            problems += [
                "%s: %s %s in %s" % (package, category, name, field)
                for category, name, field in forbidden_in_relations(stanza)
                if category == "duplicate utility"
            ]
        self.assertEqual(problems, [])

    def test_gated_tools_are_not_dependencies(self):
        problems = []
        for package_dir, package in (
            (m1_support.PackageDesktopDir, m1_support.PackageDesktop),
            (m1_support.PackageFullDir, m1_support.PackageFull),
        ):
            stanza = m1_support.package_control(package_dir, package)
            problems += [
                "%s: %s %s in %s" % (package, category, name, field)
                for category, name, field in forbidden_in_relations(stanza)
                if category == "gated tool"
            ]
        self.assertEqual(problems, [], "gated tools must not be in M1 relationships")

    def test_kali_references_are_confined_to_the_pinned_archive_files(self):
        # Flipped for F2: a Kali URL or suite is permitted only in the narrowly pinned
        # cham3_kali archive files. Anywhere else (auto config, hooks, includes, packaging)
        # it is still a problem.
        texts = {}
        texts.update(text_of_tree(m1_support.LiveDesktopDir))
        for package_dir in (m1_support.PackageDesktopDir, m1_support.PackageFullDir):
            texts.update(text_of_tree(package_dir + "/debian"))
        self.assertEqual(kali_reference_problems(texts), [])


class curated_suite_fixture_t(unittest.TestCase):
    """Positive and negative controls for the curated-suite checkers."""

    def test_positive_control_clean_set_passes(self):
        names = set(m1_support.CuratedRequired)
        self.assertEqual(curated_problems(names, lambda name: name in m1_support.CuratedDocumentedGaps), [])

    def test_removed_required_tool_is_reported(self):
        names = set(m1_support.CuratedRequired) - {"wireshark"}
        problems = curated_problems(names, lambda name: name in m1_support.CuratedDocumentedGaps)
        self.assertIn("curated tool missing from cham3-full: wireshark", problems)

    def test_removed_zenmap_is_reported_as_missing(self):
        names = set(m1_support.CuratedRequired) - {"zenmap"}
        problems = curated_problems(names, lambda name: name in m1_support.CuratedDocumentedGaps)
        self.assertIn("curated tool missing from cham3-full: zenmap", problems)

    def test_silent_gap_omission_is_reported(self):
        names = set(m1_support.CuratedRequired)
        problems = curated_problems(names, lambda name: False)
        self.assertIn("acceptance gap silently omitted (not documented): mitmproxy", problems)

    def test_documented_gap_omission_is_accepted(self):
        names = set(m1_support.CuratedRequired)
        problems = curated_problems(names, lambda name: name in m1_support.CuratedDocumentedGaps)
        self.assertEqual(problems, [])

    def test_excluded_duplicate_is_flagged(self):
        stanza = {"Depends": "plasma-desktop, thunar, konsole", "Recommends": "pcmanfm"}
        found = forbidden_in_relations(stanza)
        self.assertIn(("duplicate utility", "thunar", "Depends"), found)
        self.assertIn(("duplicate utility", "pcmanfm", "Recommends"), found)

    def test_gated_tool_is_flagged_in_depends_and_suggests(self):
        # F2: the deferred tools (and omitted alternatives) are the gated names.
        stanza = {"Depends": "nmap, mitmproxy", "Suggests": "wpscan"}
        found = forbidden_in_relations(stanza)
        self.assertIn(("gated tool", "mitmproxy", "Depends"), found)
        self.assertIn(("gated tool", "wpscan", "Suggests"), found)

    def test_included_f2_package_is_not_flagged_as_gated(self):
        # Positive control: librewolf and ghidra are included now, so they are not gated.
        stanza = {"Depends": "nmap, librewolf, radare2", "Suggests": "ghidra"}
        self.assertEqual(forbidden_in_relations(stanza), [])

    def test_alternative_is_checked_not_just_first_name(self):
        stanza = {"Depends": "kate | thunar"}
        self.assertIn(("duplicate utility", "thunar", "Depends"), forbidden_in_relations(stanza))

    def test_kali_reference_outside_the_pinned_files_is_flagged(self):
        # Negative control: an unpinned, unexpectedly named Kali archive is a problem.
        bad = {"build/live-desktop/config/archives/kali.list.chroot": "deb http://http.kali.org/kali kali-rolling main\n"}
        self.assertTrue(kali_reference_problems(bad))

    def test_kali_reference_in_auto_config_is_flagged(self):
        # Negative control: a Kali mirror in the live-build config is never allowed.
        bad = {"build/live-desktop/auto/config": "--mirror-binary http://http.kali.org/kali\n"}
        self.assertTrue(kali_reference_problems(bad))

    def test_pinned_archive_reference_is_confined_and_clean_config_passes(self):
        # Positive control: the pinned cham3_kali archive file is allowed here; its policy
        # is checked by the foreign-sources suite.
        pinned = {"build/live-desktop/config/archives/cham3_kali.sources": "URIs: http://http.kali.org/kali/\nSuites: kali-rolling\n"}
        clean = {"build/live-desktop/auto/config": "--mirror-bootstrap http://deb.debian.org/debian/\n"}
        self.assertEqual(kali_reference_problems(pinned), [])
        self.assertEqual(kali_reference_problems(clean), [])


if __name__ == "__main__":
    unittest.main()
