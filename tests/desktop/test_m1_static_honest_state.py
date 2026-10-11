"""Static honest-state checks for the M1 plasmoids (ADR 0002, M1 task).

Security negative control: M1 UI must never present a backend-dependent state
as active, enforced, isolated, connected or verified. The checks work on the
string literals of each plasmoid's QML/JS (comments removed) and on its code:

1. Required wording: each backend-dependent component states unavailable,
   disconnected or unverified, and the component-specific phrases from the
   contract appear (ECR Explorer pending ADR 0001, M4 editor, Full Isolation
   Kill, key_id placeholder).
2. Forbidden positive claims: a claim such as "Tor is enforced" or "kill
   engaged" fails unless a negation word appears in the same literal near the
   claim. Negations are checked in a window before the claim, not the whole
   literal.
3. Inert code: no executable data-source engine and no privileged command
   names in any cham3 plasmoid.

Limits: this is text and code analysis. It cannot show what the panel renders
at runtime. Runtime display is a VM-stage check, not evidence from this file.
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

StateWords = ("unavailable", "disconnected", "unverified")
NegationWords = re.compile(
    r"\b(not|no|never|without|cannot|unavailable|unverified|disconnected|pending|"
    r"unknown|inert|absent|requires?|failed|failure|none|unconfirmed|unsupported|until|"
    r"neither|nor|isn't|doesn't|won't)\b",
    re.IGNORECASE,
)
ForbiddenClaims = (
    re.compile(r"\b(tor|direct|offline|clearnet)\b[^.]{0,40}?\b(is|are|now|currently)\s+(active|enforced|engaged|connected|routing|on)\b", re.IGNORECASE),
    re.compile(r"\bkill\b[^.]{0,30}?\b((is|now|currently)\s+)?(active|engaged|enforced|armed)\b", re.IGNORECASE),
    re.compile(r"\b(enforced|enforcing)\b", re.IGNORECASE),
    re.compile(r"\b(is|are|now|currently|fully)\s+isolated\b", re.IGNORECASE),
    re.compile(r"\bconnected to tor\b", re.IGNORECASE),
    re.compile(r"\bcontract verified\b", re.IGNORECASE),
    re.compile(r"\bcontracts?\s+(is|are)\s+(valid|active)\b", re.IGNORECASE),
    re.compile(r"\bkey\s+(is\s+)?(valid|enrolled|verified|active)\b", re.IGNORECASE),
    re.compile(r"\bidentity\s+(is\s+)?(verified|enrolled)\b", re.IGNORECASE),
)
NegationWindow = 50
ForbiddenCodePatterns = (
    re.compile(r"""["']executable["']"""),
    re.compile(r"\b(sudo|pkexec|rfkill|setcap|iptables)\b"),
    re.compile(r"\bnft\s+(add|delete|flush|-f)\b"),
)
ComponentRequiredPhrases = {
    "user_launcher": ("key_id",),
    "contract_dock": ("ADR 0001",),
    "netproto_launcher": ("M4",),
    "killswitch": ("Full Isolation Kill",),
}
BackendDependent = (
    "user_launcher",
    "alias_wheel",
    "tag_pager",
    "contract_dock",
    "netproto_launcher",
    "killswitch",
    "task_manager",
)


def component_sources(directory):
    """Return (literals, code) for a plasmoid directory: QML/JS under contents/ and metadata.json."""
    literals = []
    code = []
    for dirpath, _, filenames in os.walk(directory):
        for filename in filenames:
            path = os.path.join(dirpath, filename)
            if filename.endswith((".qml", ".js")):
                with open(path, "r", encoding="utf-8", errors="replace") as handle:
                    text = handle.read()
                literals.extend(m1_support.qml_string_literals(text))
                code.append(m1_support.strip_js_comments(text))
            elif filename == "metadata.json":
                with open(path, "r", encoding="utf-8", errors="replace") as handle:
                    literals.extend(re.findall(r'"([^"]*)"', handle.read()))
    return literals, "\n".join(code)


def claim_violations(literal):
    """Return the claim phrases in literal that are not negated near the claim."""
    violations = []
    for pattern in ForbiddenClaims:
        for match in pattern.finditer(literal):
            window = literal[max(0, match.start() - NegationWindow):match.end()]
            if not NegationWords.search(window):
                violations.append(match.group(0))
    return violations


def honest_state_problems(component, literals, code):
    """Problems for one plasmoid: required wording, forbidden claims and inert code."""
    problems = []
    joined = " ".join(literals)
    if component in BackendDependent and not any(
        word in joined.lower() for word in StateWords + ("unknown",)
    ) and component != "task_manager":
        problems.append("%s shows no unavailable/disconnected/unverified state" % component)
    if component == "task_manager" and "unknown" not in joined.lower() and "unverified" not in joined.lower():
        problems.append("task_manager must show neutral/unknown attribution, not fabricated state")
    for phrase in ComponentRequiredPhrases.get(component, ()):
        if phrase not in joined:
            problems.append("%s lacks required wording: %s" % (component, phrase))
    for literal in literals:
        for phrase in claim_violations(literal):
            problems.append("%s makes a positive security claim: %r in %r" % (component, phrase, literal))
    for pattern in ForbiddenCodePatterns:
        match = pattern.search(code)
        if match:
            problems.append("%s contains a forbidden inert-code construct: %s" % (component, match.group(0)))
    return problems


def plasmoid_component_dir(component):
    plugin_id = m1_support.Cham3Namespace + component
    found = m1_support.plasmoid_dirs(plugin_id)
    if len(found) != 1:
        raise AssertionError("expected exactly one %s directory under desktop/, found %d" % (plugin_id, len(found)))
    return found[0]


class honest_state_real_t(unittest.TestCase):
    def check_component(self, component):
        literals, code = component_sources(plasmoid_component_dir(component))
        self.assertEqual(honest_state_problems(component, literals, code), [])

    def test_user_launcher_is_honest(self):
        self.check_component("user_launcher")

    def test_alias_wheel_is_honest(self):
        self.check_component("alias_wheel")

    def test_tag_pager_is_honest(self):
        self.check_component("tag_pager")

    def test_contract_dock_is_honest_and_reports_ecr_pending(self):
        self.check_component("contract_dock")

    def test_netproto_launcher_is_honest(self):
        self.check_component("netproto_launcher")

    def test_killswitch_is_inert_and_reports_unavailable(self):
        self.check_component("killswitch")

    def test_task_manager_reports_unknown_attribution(self):
        self.check_component("task_manager")

    def test_settings_launcher_has_no_forbidden_claims_or_privileged_code(self):
        literals, code = component_sources(plasmoid_component_dir("settings_launcher"))
        self.assertEqual(honest_state_problems("settings_launcher", literals, code), [])

    def test_alias_wheel_labels_its_states_in_text(self):
        literals, _ = component_sources(plasmoid_component_dir("alias_wheel"))
        joined = " ".join(literals)
        for label in ("Direct", "Tor", "Offline"):
            self.assertIn(label, joined, "color-only state: alias label %s missing" % label)

    def test_desktop_readme_states_foundation_only(self):
        text = m1_support.read_repo(m1_support.DesktopTreeDir + "/README.md").lower()
        self.assertIn("foundation", text)
        self.assertTrue("inert" in text or "unavailable" in text, "README must state inert/unavailable limits")


class honest_state_fixture_t(unittest.TestCase):
    """Positive and negative controls for the honest-state checkers."""

    def test_negated_wording_is_accepted(self):
        self.assertEqual(claim_violations("Tor is not enforced"), [])
        self.assertEqual(claim_violations("Full Isolation Kill unavailable - no trusted enforcement"), [])
        self.assertEqual(claim_violations("Active Contracts Monitor"), [])
        self.assertEqual(claim_violations("Network Protocol Editor unavailable (M4 enforcement not present)"), [])

    def test_positive_claims_are_rejected(self):
        self.assertTrue(claim_violations("Tor is enforced"))
        self.assertTrue(claim_violations("Full Isolation Kill engaged"))
        self.assertTrue(claim_violations("Kill is active"))
        self.assertTrue(claim_violations("Session is isolated"))
        self.assertTrue(claim_violations("Contract verified"))
        self.assertTrue(claim_violations("Key is valid"))
        self.assertTrue(claim_violations("Connected to Tor"))

    def test_negation_far_away_does_not_excuse_claim(self):
        far = "Not configured yet. " + ("x" * 80) + " Tor is enforced"
        self.assertTrue(claim_violations(far))

    def test_component_missing_unavailable_state_is_rejected(self):
        problems = honest_state_problems("killswitch", ["Full Isolation Kill", "Off"], "")
        self.assertTrue(any("no unavailable" in p for p in problems), problems)

    def test_component_claim_is_reported_with_component(self):
        literals = ["Full Isolation Kill unavailable", "Kill is active"]
        problems = honest_state_problems("killswitch", literals, "")
        self.assertTrue(any("positive security claim" in p for p in problems), problems)

    def test_ecr_phrase_is_required_on_contract_dock(self):
        problems = honest_state_problems("contract_dock", ["ECR Explorer unavailable"], "")
        self.assertTrue(any("ADR 0001" in p for p in problems), problems)

    def test_executable_engine_is_rejected(self):
        code = 'Plasma5Support.DataSource { engine: "executable" }'
        problems = honest_state_problems("killswitch", ["Full Isolation Kill unavailable"], code)
        self.assertTrue(any("inert-code" in p for p in problems), problems)

    def test_privileged_command_is_rejected(self):
        problems = honest_state_problems("killswitch", ["Full Isolation Kill unavailable"], "run('sudo nft -f rules')")
        self.assertTrue(any("inert-code" in p for p in problems), problems)

    def test_positive_control_component_passes(self):
        literals = ["Full Isolation Kill unavailable - no trusted enforcement", "Kill switch state: unavailable"]
        code = "function noop() { return 0; }"
        self.assertEqual(honest_state_problems("killswitch", literals, code), [])


if __name__ == "__main__":
    unittest.main()
