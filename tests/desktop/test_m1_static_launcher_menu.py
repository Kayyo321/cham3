"""Static checks for the user launcher's application menu (F5, A11).

The default launcher must present installed applications in menu categories with
a search field. The list must come from a maintained KDE application-model
facility (org.kde.plasma.private.kicker or org.kde.plasma.kickoff), not from a
fixed array of hardcoded desktop IDs. The launcher keeps its honest key_id and
unavailable wording and its ordinary session actions. It shows no contract badges
or pips (M5 scope) and makes no security claim.

These are text and code checks of user_launcher's QML. They cannot show the menu
on screen. Whether the categorized menu lists the installed suite and whether
entries launch is a live or installed-desktop check: a Codex Remote heavy gate
that this file does not establish.
"""

import os
import re
import sys
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m1_support  # noqa: E402

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import test_m1_static_honest_state as honest  # noqa: E402

LauncherComponent = "user_launcher"
LauncherMain = "contents/ui/main.qml"
SessionActionsFile = "session_actions.qml"
ApplicationModelImport = re.compile(
    r"^\s*import\s+(org\.kde\.plasma\.private\.kicker|org\.kde\.plasma\.kickoff)"
    r"(?:\s+[0-9][0-9.]*)?(?:\s+as\s+(\w+))?\s*$",
    re.MULTILINE,
)
FixedApplicationArray = re.compile(r"\bapplications\s*:\s*\[")
DesktopIdLiteral = re.compile(r"\.desktop$")
CategoryIdentifier = re.compile(r"\bcategor\w*\b|\bsection\b", re.IGNORECASE)
SearchElement = re.compile(r"\b(SearchField|TextField)\b")
# Identifiers, including underscored names such as contract_badge. "pip" must be a whole word.
BadgeIdentifier = re.compile(r"(?i)(badge|shield|chip|(?<![a-z])pips?(?![a-z]))")
AbsenceClaim = re.compile(
    r"(?i)\b(search|categor\w*)\b[^.]{0,60}\b(not present|absent|not available)\b"
)
BadgeLiteral = re.compile(r"(?i)\b(badges?|pips?)\b")
SessionCalls = ("do_lock", "do_logout", "do_reboot", "do_shutdown")


def code_without_literals(qml_text):
    """Code with comments removed and string-literal contents blanked out."""
    return m1_support.StringLiteral.sub('""', m1_support.strip_js_comments(qml_text))


def affirmative_literal(literals, pattern):
    """True when some literal matches pattern and carries no negation word."""
    for literal in literals:
        if re.search(pattern, literal, re.IGNORECASE) and not honest.NegationWords.search(literal):
            return True
    return False


def launcher_menu_problems(qml_text, literals, session_actions_present):
    """Problems with the launcher's application menu (empty means accepted)."""
    problems = []
    code = code_without_literals(qml_text)
    code_with_literals = m1_support.strip_js_comments(qml_text)

    if FixedApplicationArray.search(code):
        problems.append("hardcoded application list (applications: [...]) is still present")
    for literal in literals:
        if DesktopIdLiteral.search(literal):
            problems.append("hardcoded desktop entry id in launcher: %r" % literal)

    facility = ApplicationModelImport.search(code_with_literals)
    if not facility:
        problems.append("no maintained application-model facility is imported (kicker or kickoff)")
    else:
        alias = facility.group(2)
        used = re.search(r"\b%s\." % re.escape(alias), code) if alias else re.search(
            r"\b(RootModel|AppsModel|Kickoff)\b", code
        )
        if not used:
            problems.append("the imported application-model facility is never used")

    if not CategoryIdentifier.search(code):
        problems.append("no categorized presentation (category or section model roles) in the launcher")
    if not SearchElement.search(code):
        problems.append("no search field (TextField or SearchField) in the launcher")
    if not affirmative_literal(literals, r"\bsearch"):
        problems.append("no affirmative search label or placeholder in the launcher")

    # An absence disclaimer is accurate only while categories and search are absent.
    presented = bool(CategoryIdentifier.search(code) or SearchElement.search(code))
    for literal in literals:
        if presented and AbsenceClaim.search(literal):
            problems.append("launcher says categories or search are absent but presents them: %r" % literal)

    if BadgeIdentifier.search(code):
        problems.append("contract badge or pip element present (M5 scope)")
    for literal in literals:
        if BadgeLiteral.search(literal) and not honest.NegationWords.search(literal):
            problems.append("badge or pip wording is not negated: %r" % literal)

    for call in SessionCalls:
        if call not in code:
            problems.append("session action %s is missing" % call)
    if SessionActionsFile not in code_with_literals or not session_actions_present:
        problems.append("session actions module (session_actions.qml) is not loaded")
    return problems


def launcher_inputs():
    """(qml_text, literals, code, session_actions_present) for the real user_launcher."""
    directory = honest.plasmoid_component_dir(LauncherComponent)
    with open(os.path.join(directory, LauncherMain), "r", encoding="utf-8") as handle:
        qml_text = handle.read()
    literals, code = honest.component_sources(directory)
    session_present = os.path.isfile(os.path.join(directory, "contents", "ui", SessionActionsFile))
    return qml_text, literals, code, session_present


class launcher_menu_real_t(unittest.TestCase):
    def check_real(self):
        qml_text, literals, _, session_present = launcher_inputs()
        return launcher_menu_problems(qml_text, literals, session_present)

    def test_real_launcher_menu_meets_f5_contract(self):
        self.assertEqual(self.check_real(), [])

    def test_no_hardcoded_application_list(self):
        problems = [p for p in self.check_real() if "hardcoded" in p]
        self.assertEqual(problems, [])

    def test_application_model_facility_is_imported_and_used(self):
        problems = [p for p in self.check_real() if "application-model" in p]
        self.assertEqual(problems, [])

    def test_honest_key_id_wording_and_no_security_claims(self):
        _, literals, code, _ = launcher_inputs()
        self.assertEqual(honest.honest_state_problems(LauncherComponent, literals, code), [])
        self.assertTrue(any("key_id" in literal for literal in literals), "key_id wording missing")

    def test_no_fabricated_badges_or_pips(self):
        problems = [p for p in self.check_real() if "badge" in p or "pip" in p]
        self.assertEqual(problems, [])


class launcher_menu_fixture_t(unittest.TestCase):
    """Positive and negative controls for launcher_menu_problems()."""

    Good = """import QtQuick
import org.kde.plasma.plasmoid
import org.kde.plasma.components as PlasmaComponents
import org.kde.plasma.private.kicker as Kicker
import org.kde.kirigami as Kirigami
PlasmoidItem {
    // Categorized applications from a maintained KDE application model.
    Kicker.RootModel {
        id: app_model
        autoExpandCategories: true
    }
    PlasmaComponents.TextField {
        placeholderText: "Search applications"
    }
    ListView {
        model: app_model
        section.property: "category"
    }
    PlasmaComponents.Label { text: "key_id: " + Shared.state_unavailable }
    PlasmaComponents.Label { text: "Session actions: unavailable (session module not loaded)." }
    Loader { source: Qt.resolvedUrl("session_actions.qml") }
    PlasmaComponents.Button { onClicked: root.session_item.do_lock() }
    PlasmaComponents.Button { onClicked: root.session_item.do_logout() }
    PlasmaComponents.Button { onClicked: root.session_item.do_reboot() }
    PlasmaComponents.Button { onClicked: root.session_item.do_shutdown() }
}
"""

    def literals_of(self, text):
        return m1_support.qml_string_literals(text)

    def check(self, text):
        return launcher_menu_problems(text, self.literals_of(text), True)

    def test_positive_control_accepted(self):
        self.assertEqual(self.check(self.Good), [])

    def test_hardcoded_application_list_is_rejected(self):
        text = self.Good.replace(
            "PlasmoidItem {\n",
            'PlasmoidItem {\n    readonly property var applications: [\n'
            '        { label: "Files", icon: "system-file-manager" }\n    ]\n',
        )
        problems = self.check(text)
        self.assertTrue(any("hardcoded application list" in p for p in problems), problems)

    def test_desktop_id_literal_is_rejected(self):
        text = self.Good.replace(
            'Qt.resolvedUrl("session_actions.qml")',
            'Qt.resolvedUrl("session_actions.qml") + "org.kde.dolphin.desktop"',
        )
        problems = self.check(text)
        self.assertTrue(any("desktop entry id" in p for p in problems), problems)

    def test_missing_application_model_import_is_rejected(self):
        text = self.Good.replace("import org.kde.plasma.private.kicker as Kicker\n", "")
        problems = self.check(text)
        self.assertTrue(any("no maintained application-model facility" in p for p in problems), problems)

    def test_imported_but_unused_facility_is_rejected(self):
        text = self.Good.replace("Kicker.RootModel {", "ListView {").replace(
            "        autoExpandCategories: true\n", ""
        )
        problems = self.check(text)
        self.assertTrue(any("never used" in p for p in problems), problems)

    def test_missing_search_field_is_rejected(self):
        text = self.Good.replace("PlasmaComponents.TextField {", "PlasmaComponents.Label {")
        problems = self.check(text)
        self.assertTrue(any("no search field" in p for p in problems), problems)

    def test_absence_claim_for_categories_is_rejected(self):
        text = self.Good.replace(
            'PlasmaComponents.Label { text: "key_id: " + Shared.state_unavailable }',
            'PlasmaComponents.Label { text: "key_id: " + Shared.state_unavailable }\n'
            '    PlasmaComponents.Label { text: "Search, categories and contract badges are not present (M5)." }',
        )
        problems = self.check(text)
        self.assertTrue(any("absent but presents them" in p for p in problems), problems)

    def test_contract_badge_element_is_rejected(self):
        text = self.Good.replace(
            "PlasmoidItem {\n",
            "PlasmoidItem {\n    Rectangle { id: contract_badge }\n",
        )
        problems = self.check(text)
        self.assertTrue(any("badge or pip element" in p for p in problems), problems)

    def test_missing_session_action_is_rejected(self):
        text = self.Good.replace("root.session_item.do_reboot()", "root.session_item.noop()")
        problems = self.check(text)
        self.assertTrue(any("do_reboot" in p for p in problems), problems)

    def test_missing_session_module_is_rejected(self):
        problems = launcher_menu_problems(self.Good, self.literals_of(self.Good), False)
        self.assertTrue(any("session_actions.qml" in p for p in problems), problems)


if __name__ == "__main__":
    unittest.main()
