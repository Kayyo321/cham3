"""Static checks for shipped .desktop and .directory files (ADR 0002, M1 task).

Scans desktop/ and the cham3 packaging directories. Validation uses
desktop-file-validate when it is installed; otherwise a built-in parser checks
the same required keys and the tests report which validator ran. Rules:
required keys present, unique valid IDs, a registered menu category, and no
launcher that points at an absent cham3 backend or fakes an unavailable tool
(ADR 0002 and M1 task: "Do not add fake launchers for absent tools").
"""

import configparser
import os
import re
import shutil
import sys
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402
import m1_support  # noqa: E402

DesktopIdPattern = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*\.desktop$")
BackendExecPattern = re.compile(r"^cham3[-_]")
AbsentBackendNames = frozenset({
    "cham3-netd", "cham3-vaultd", "cham3-sessiond", "cham3-contractd", "cham3-policyd",
    "cham3-registryd", "cham3-identityd", "cham3-launcherd", "cham3-authui",
    "cham3_contract_manager", "cham3_ecr_explorer", "cham3_active_monitor", "cham3_inspector",
})
FakeLauncherPhrases = (
    "ecr explorer",
    "active contracts monitor",
    "contract handler",
    "hot contract view",
    "kill switch",
    "killswitch",
    "network protocol editor",
)
EnvPrefixPattern = re.compile(r"^(env\s+)?([A-Za-z_][A-Za-z0-9_]*=\S*\s+)*")
ValidatorName = "desktop-file-validate"


def desktop_sources():
    """Repo-relative .desktop and .directory files under desktop/ and cham3 packaging."""
    found = m1_support.all_desktop_files((".desktop", ".directory"))
    root = m0_support.repo_root()
    for package_dir in (m1_support.PackageDesktopDir, m1_support.PackageFullDir):
        absolute = os.path.join(root, package_dir)
        if os.path.isdir(absolute):
            for dirpath, _, filenames in os.walk(absolute):
                for filename in filenames:
                    if filename.endswith((".desktop", ".directory")):
                        found.append(os.path.relpath(os.path.join(dirpath, filename), root))
    return sorted(set(found))


def exec_program(exec_value):
    """Return the program basename of an Exec line (env assignments and field codes stripped)."""
    stripped = EnvPrefixPattern.sub("", exec_value.strip(), count=1)
    stripped = re.sub(r"%[a-zA-Z]", "", stripped).strip()
    if not stripped:
        return ""
    program = stripped.split()[0].strip('"')
    return os.path.basename(program)


def parse_entry(text):
    parser = configparser.ConfigParser(interpolation=None, strict=True, delimiters=("=",))
    parser.optionxform = str
    parser.read_string(text)
    if not parser.has_section("Desktop Entry"):
        raise AssertionError("missing [Desktop Entry] group")
    return dict(parser.items("Desktop Entry"))


def entry_problems(filename, text):
    problems = []
    try:
        entry = parse_entry(text)
    except (configparser.Error, AssertionError) as error:
        return ["%s does not parse: %s" % (filename, error)]
    if not DesktopIdPattern.match(filename):
        problems.append("%s is not a valid desktop-file ID" % filename)
    entry_type = entry.get("Type", "")
    if entry_type not in ("Application", "Directory"):
        problems.append("%s: Type must be Application or Directory" % filename)
    if not entry.get("Name", "").strip():
        problems.append("%s: Name is required" % filename)
    if not entry.get("Icon", "").strip():
        problems.append("%s: Icon is required" % filename)
    if entry_type == "Application":
        if not entry.get("Exec", "").strip():
            problems.append("%s: Exec is required for Type=Application" % filename)
        categories = [c for c in entry.get("Categories", "").split(";") if c]
        if not categories or not (set(categories) & m1_support.MainMenuCategories):
            problems.append("%s: needs a registered main menu category" % filename)
        program = exec_program(entry.get("Exec", ""))
        if program in AbsentBackendNames or BackendExecPattern.match(program):
            problems.append("%s: Exec targets an absent cham3 backend: %s" % (filename, program))
    lowered = " ".join(entry.get(key, "") for key in ("Name", "Comment", "Exec")).lower()
    for phrase in FakeLauncherPhrases:
        if phrase in lowered:
            problems.append("%s: launcher for an unavailable component (%s)" % (filename, phrase))
    return problems


def validator_problems(paths):
    """Run desktop-file-validate on each path when present; return problems."""
    if shutil.which(ValidatorName) is None:
        return None
    problems = []
    for path in paths:
        completed = m0_support.run_tool([ValidatorName, path])
        if completed.returncode != 0:
            problems.append("%s rejected: %s" % (path, (completed.stdout + completed.stderr).strip()))
    return problems


def uniqueness_problems(paths):
    seen = {}
    problems = []
    for path in paths:
        name = os.path.basename(path)
        if name in seen:
            problems.append("duplicate desktop-file ID %s in %s and %s" % (name, seen[name], path))
        seen[name] = path
    return problems


class desktop_files_real_t(unittest.TestCase):
    def test_every_shipped_desktop_entry_is_valid(self):
        paths = desktop_sources()
        root = m0_support.repo_root()
        problems = []
        for relative in paths:
            text = m1_support.read_repo(relative)
            problems += entry_problems(os.path.basename(relative), text)
        problems += uniqueness_problems(paths)
        validator = validator_problems([os.path.join(root, p) for p in paths])
        if validator is None:
            sys.stderr.write("\n[m1] %s not installed: built-in parser used for %d file(s)\n" % (ValidatorName, len(paths)))
        else:
            problems += validator
        self.assertEqual(problems, [], "desktop entry problems")

    def test_no_launcher_points_at_absent_backend(self):
        problems = []
        for relative in desktop_sources():
            try:
                entry = parse_entry(m1_support.read_repo(relative))
            except (configparser.Error, AssertionError) as error:
                problems.append("%s: %s" % (relative, error))
                continue
            program = exec_program(entry.get("Exec", ""))
            if program in AbsentBackendNames or BackendExecPattern.match(program):
                problems.append("%s runs absent backend %s" % (relative, program))
        self.assertEqual(problems, [])


class desktop_files_fixture_t(unittest.TestCase):
    def good(self):
        return (
            "[Desktop Entry]\nType=Application\nName=Settings\nIcon=preferences-system\n"
            "Exec=systemsettings\nCategories=Settings;System;\n"
        )

    def test_positive_control_accepted(self):
        self.assertEqual(entry_problems("org.cham3.settings.desktop", self.good()), [])

    def test_missing_exec_and_name_reported(self):
        text = self.good().replace("Exec=systemsettings\n", "").replace("Name=Settings\n", "")
        problems = entry_problems("org.cham3.settings.desktop", text)
        self.assertTrue(any("Exec is required" in p for p in problems), problems)
        self.assertTrue(any("Name is required" in p for p in problems), problems)

    def test_backend_launcher_is_rejected(self):
        text = self.good().replace("Exec=systemsettings", "Exec=cham3-netd --apply")
        problems = entry_problems("org.cham3.netd.desktop", text)
        self.assertTrue(any("absent cham3 backend" in p for p in problems), problems)

    def test_fake_ecr_launcher_is_rejected(self):
        text = self.good().replace("Name=Settings", "Name=ECR Explorer").replace("Exec=systemsettings", "Exec=true")
        problems = entry_problems("org.cham3.ecr.desktop", text)
        self.assertTrue(any("unavailable component" in p for p in problems), problems)

    def test_missing_registered_category_is_rejected(self):
        text = self.good().replace("Categories=Settings;System;", "Categories=Cham3;")
        problems = entry_problems("org.cham3.settings.desktop", text)
        self.assertTrue(any("registered main menu category" in p for p in problems), problems)

    def test_invalid_id_and_duplicate_id_are_rejected(self):
        self.assertTrue(entry_problems("bad id.desktop", self.good()))
        self.assertTrue(uniqueness_problems(["a/org.cham3.x.desktop", "b/org.cham3.x.desktop"]))

    def test_duplicate_key_is_rejected(self):
        text = self.good() + "Exec=true\n"
        problems = entry_problems("org.cham3.settings.desktop", text)
        self.assertTrue(any("does not parse" in p for p in problems), problems)

    def test_exec_program_strips_env_and_field_codes(self):
        self.assertEqual(exec_program("env QT_QPA=x /usr/bin/konsole %U"), "konsole")
        self.assertEqual(exec_program("systemsettings"), "systemsettings")


if __name__ == "__main__":
    unittest.main()
