"""Static checks for the cham3-base package source (packaging/cham3-base/debian)."""

import os
import re
import sys
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402

DebianDir = "packaging/cham3-base/debian"
DocDir = "usr/share/doc/cham3-base"
RequiredDescriptionPhrases = (
    "m0",
    "development skeleton",
    "no services",
    "security policy",
    "enforcement",
    "system configuration",
    "image integration",
)
ForbiddenDescriptionWords = ("kde", "plasma")
ForbiddenDebianNames = (
    "postinst",
    "preinst",
    "prerm",
    "postrm",
    "config",
    "templates",
    "conffiles",
    "triggers",
    "compat",
)
ForbiddenDebianSuffixes = (
    ".postinst",
    ".preinst",
    ".prerm",
    ".postrm",
    ".service",
    ".timer",
    ".socket",
    ".path",
    ".target",
    ".init",
    ".default",
    ".tmpfiles",
    ".sysusers",
)
CopyrightFormatPattern = (
    r"Format:\s*https://www\.debian\.org/doc/packaging-manuals/copyright-format/1\.0/?"
)


def debian_path(name):
    return DebianDir + "/" + name


def debian_directory():
    directory = os.path.join(m0_support.repo_root(), DebianDir)
    if not os.path.isdir(directory):
        raise AssertionError("required directory missing: " + DebianDir)
    return directory


def binary_stanza():
    stanzas = m0_support.parse_control(m0_support.read_text(debian_path("control")))
    if len(stanzas) != 2:
        raise AssertionError("debian/control must have a source and a binary stanza")
    return stanzas


class package_source_t(unittest.TestCase):
    def test_source_stanza_metadata(self):
        source = binary_stanza()[0]
        self.assertEqual(source.get("Source"), m0_support.ExpectedPackage)
        self.assertEqual(source.get("Rules-Requires-Root"), "no")
        self.assertIn("debhelper-compat (= 13)", source.get("Build-Depends", ""))
        self.assertIn("Standards-Version", source)

    def test_binary_stanza_metadata_and_dependencies(self):
        binary = binary_stanza()[1]
        self.assertEqual(binary.get("Package"), m0_support.ExpectedPackage)
        self.assertEqual(binary.get("Architecture"), "all")
        self.assertEqual(binary.get("Depends"), "${misc:Depends}")
        self.assertNotIn("Pre-Depends", binary)

    def test_maintainer_is_reserved_placeholder(self):
        stanzas = binary_stanza()
        maintainer = stanzas[1].get("Maintainer", "")
        addresses = re.findall(r"<([^>]+)>", maintainer)
        self.assertTrue(addresses, "Maintainer must include an email address")
        for address in addresses:
            self.assertTrue(
                m0_support.email_domain_is_reserved(address),
                "Maintainer address is not an RFC 2606 reserved placeholder",
            )

    def test_description_states_m0_skeleton_limits(self):
        description = m0_support.normalize_space(binary_stanza()[1].get("Description", ""))
        missing = [p for p in RequiredDescriptionPhrases if p not in description]
        self.assertEqual(missing, [], "description lacks required disclaimers")
        for word in ForbiddenDescriptionWords:
            self.assertNotIn(word, description)

    def test_changelog_version_and_distribution(self):
        text = m0_support.read_text(debian_path("changelog"))
        first = text.split("\n", 1)[0]
        self.assertRegex(
            first,
            r"^%s \(%s\) UNRELEASED; urgency=\w+$"
            % (re.escape(m0_support.ExpectedPackage), re.escape(m0_support.ExpectedVersion)),
        )

    def test_changelog_and_control_emails_are_reserved(self):
        for name in ("control", "changelog", "copyright"):
            text = m0_support.read_text(debian_path(name))
            for address in re.findall(r"[\w.+-]+@[\w.-]+", text):
                self.assertTrue(
                    m0_support.email_domain_is_reserved(address),
                    "%s contains a non-reserved email address" % name,
                )

    def test_source_format_is_native_3_0(self):
        self.assertEqual(
            m0_support.read_text(debian_path("source/format")).strip(), "3.0 (native)"
        )

    def test_rules_is_executable_and_no_compat_file(self):
        modes = m0_support.tracked_modes()
        self.assertEqual(
            modes.get(debian_path("rules")), "100755", "debian/rules must be tracked as executable"
        )
        self.assertNotIn(debian_path("compat"), modes)

    def test_no_maintainer_scripts_conffiles_or_units(self):
        root = m0_support.repo_root()
        problems = []
        for dirpath, _, filenames in os.walk(debian_directory()):
            for name in filenames:
                if name in ForbiddenDebianNames or name.endswith(ForbiddenDebianSuffixes):
                    problems.append(os.path.relpath(os.path.join(dirpath, name), root))
        self.assertEqual(problems, [])

    def test_install_and_dirs_stay_under_doc_directory(self):
        problems = []
        for name in sorted(os.listdir(debian_directory())):
            if not (name.endswith(".install") or name.endswith(".dirs")):
                continue
            for line in m0_support.read_text(debian_path(name)).splitlines():
                entry = line.strip()
                if entry and not entry.startswith("#") and not entry.startswith(DocDir):
                    problems.append("%s installs outside doc dir: %s" % (name, entry))
        self.assertEqual(problems, [])

    def test_copyright_reflects_agpl_and_pending_license_choice(self):
        text = m0_support.read_text(debian_path("copyright"))
        self.assertRegex(text, CopyrightFormatPattern)
        self.assertRegex(text, r"(?m)^License:\s*AGPL-3\s*$")
        self.assertNotIn("AGPL-3+", text)
        self.assertNotIn("any later version", text)
        self.assertRegex(text, r"(?i)PROJECT\.md")
        self.assertRegex(text, r"(?i)before public release")

    def test_license_file_is_agpl_v3(self):
        text = m0_support.read_text("LICENSE")
        self.assertIn("GNU AFFERO GENERAL PUBLIC LICENSE", text)
        self.assertIn("Version 3, 19 November 2007", text)


if __name__ == "__main__":
    unittest.main()
