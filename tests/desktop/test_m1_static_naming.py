"""Naming-convention checks for M1 sources (AGENTS.md coding convention, ADR 0002).

snake_case for files, directories, identifiers and fields; PascalCase only for
constants; types snake_case_t; no camelCase in cham3-authored names. External
boundaries are exempt and listed explicitly: the org.kde.* and org.cham3.* ids
and their KPlugin JSON keys, the Cham3Dark.colors file name (KDE expects it),
the Debian package directory names cham3-desktop and cham3-full, and the
conventional README.md. Exceptions are narrow and named, not pattern-wide.

The contract's debian/cham3-extra-sources file name is listed in
ConventionExceptions with a note: it is a cham3 name, not an external one, so
it is recorded as an internal-review finding rather than silently accepted.
"""

import ast
import os
import re
import sys
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402
import m1_support  # noqa: E402

SnakeName = re.compile(r"^[a-z_][a-z0-9_]*$")
PackageDirName = re.compile(r"^cham3-[a-z0-9-]+$")
ExternalDottedId = re.compile(r"^org\.(cham3|kde)\.[a-z0-9_.]+$")
ConventionExceptions = {
    # Contract name for the extra-sources list. Flagged for coordinator decision.
    "cham3-extra-sources": "contract name, internal convention; review finding F-naming",
}
AllowedFileNames = frozenset({
    "README.md", "README.Debian", "metadata.json", "main.qml", "LICENSE", "Cham3Dark.colors",
})
# Directory names that mirror KDE/Debian install layout paths (external boundary).
ExternalDirNames = frozenset({"look-and-feel", "color-schemes", "layout-templates", "plasma", "plasmoids"})
DeclarationPatterns = (
    re.compile(r"\bproperty\s+(?:[\w.<>]+\s+)([A-Za-z_]\w*)"),
    re.compile(r"\bfunction\s+([A-Za-z_]\w*)\s*\("),
    re.compile(r"\bsignal\s+([A-Za-z_]\w*)"),
    re.compile(r"\bid\s*:\s*([A-Za-z_]\w*)"),
    re.compile(r"\b(?:var|let|const)\s+([A-Za-z_]\w*)"),
)
UnittestMethodNames = frozenset({"setUp", "tearDown", "setUpClass", "tearDownClass"})


def path_component_problems(relative_path):
    problems = []
    parts = relative_path.split("/")
    for index, part in enumerate(parts):
        if part.startswith("."):
            continue
        if part in AllowedFileNames or part in ConventionExceptions or part in ExternalDirNames:
            continue
        if part.startswith("org.cham3.") or part.startswith("org.kde."):
            if not ExternalDottedId.match(part) and part.startswith("org.cham3."):
                problems.append("%s: invalid org.cham3 id %s" % (relative_path, part))
            continue
        if index >= 1 and parts[index - 1] == "packaging" and PackageDirName.match(part):
            continue
        stem = part.split(".", 1)[0]
        if part.endswith((".colors",)):
            stem = part[: -len(".colors")]
            if stem in m1_support.ExternalBoundaryStems:
                continue
        if not SnakeName.match(stem):
            problems.append("%s: not snake_case: %s" % (relative_path, part))
    return problems


def identifier_problems(relative_path, code):
    problems = []
    for pattern in DeclarationPatterns:
        for match in pattern.finditer(code):
            name = match.group(1)
            if not SnakeName.match(name):
                problems.append("%s: camelCase or non-snake identifier %s" % (relative_path, name))
    return problems


def python_name_problems(relative_path, text):
    """Class names must end in _t and be snake_case; function names must be snake_case."""
    problems = []
    tree = ast.parse(text, filename=relative_path)
    for node in ast.walk(tree):
        if isinstance(node, ast.ClassDef):
            if not re.match(r"^[a-z][a-z0-9_]*_t$", node.name):
                problems.append("%s: class %s must be snake_case ending in _t" % (relative_path, node.name))
        elif isinstance(node, ast.FunctionDef):
            if node.name in UnittestMethodNames or node.name.startswith("test_"):
                continue
            if node.name.startswith("__") and node.name.endswith("__"):
                continue
            if not SnakeName.match(node.name):
                problems.append("%s: function %s is not snake_case" % (relative_path, node.name))
    return problems


def desktop_relative_paths():
    results = []
    root = m0_support.repo_root()
    for base in (m1_support.DesktopTreeDir, m1_support.PackageDesktopDir, m1_support.PackageFullDir):
        absolute = os.path.join(root, base)
        if not os.path.isdir(absolute):
            raise AssertionError("required directory missing: " + base)
        for dirpath, dirnames, filenames in os.walk(absolute):
            for name in dirnames + filenames:
                results.append(os.path.relpath(os.path.join(dirpath, name), root))
    return sorted(results)


def desktop_source_texts():
    texts = {}
    for relative in m1_support.all_desktop_files((".qml", ".js")):
        texts[relative] = m1_support.strip_js_comments(m1_support.read_repo(relative))
    return texts


class naming_real_t(unittest.TestCase):
    def test_desktop_and_package_paths_are_snake_case(self):
        problems = []
        for relative in desktop_relative_paths():
            problems += path_component_problems(relative)
        self.assertEqual(problems, [])

    def test_qml_and_js_declarations_are_snake_case(self):
        problems = []
        for relative, code in desktop_source_texts().items():
            problems += identifier_problems(relative, code)
        self.assertEqual(problems, [])

    def test_m1_python_test_names_follow_convention(self):
        problems = []
        for base in ("tests/desktop", "tests/lib"):
            absolute = os.path.join(m0_support.repo_root(), base)
            if not os.path.isdir(absolute):
                raise AssertionError("required directory missing: " + base)
            for name in sorted(os.listdir(absolute)):
                if name.startswith("test_m1_") or name == "m1_support.py":
                    path = os.path.join(absolute, name)
                    with open(path, "r", encoding="utf-8") as handle:
                        problems += python_name_problems(os.path.join(base, name), handle.read())
        self.assertEqual(problems, [])


class naming_fixture_t(unittest.TestCase):
    def test_snake_paths_accepted(self):
        self.assertEqual(path_component_problems("desktop/plasma/plasmoids/org.cham3.killswitch/metadata.json"), [])
        self.assertEqual(path_component_problems("desktop/plasma/layout/cham3_panel_layout.js"), [])
        self.assertEqual(path_component_problems("desktop/plasma/color-schemes/Cham3Dark.colors"), [])
        self.assertEqual(path_component_problems("packaging/cham3-desktop/debian/control"), [])

    def test_camel_case_path_is_rejected(self):
        self.assertTrue(path_component_problems("desktop/plasma/layout/cham3PanelLayout.js"))

    def test_extra_sources_exception_is_listed_not_hidden(self):
        self.assertIn("cham3-extra-sources", ConventionExceptions)
        self.assertEqual(path_component_problems("packaging/cham3-desktop/debian/cham3-extra-sources"), [])

    def test_camel_case_declarations_are_rejected(self):
        bad = "property int avatarSize: 40\nfunction openFlyout() {}\nvar tagCount = 5;\n"
        problems = identifier_problems("desktop/x.qml", bad)
        self.assertEqual(len(problems), 3, problems)

    def test_snake_declarations_and_handlers_accepted(self):
        good = "property int avatar_size: 40\nfunction open_flyout() {}\nvar tag_count = 5;\nid: root\n"
        self.assertEqual(identifier_problems("desktop/x.qml", good), [])

    def test_python_camel_class_is_rejected(self):
        self.assertTrue(python_name_problems("tests/desktop/x.py", "class badName(object):\n    pass\n"))
        self.assertEqual(python_name_problems("tests/desktop/x.py", "class good_t(object):\n    pass\n"), [])


if __name__ == "__main__":
    unittest.main()
