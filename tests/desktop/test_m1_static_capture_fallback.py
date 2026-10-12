"""Static contract checks for the dumpcap capture fallback (F3, security).

wireshark-common's own postinst applies a setuid-root fallback when setcap is
missing or fails (chmod u=rwxs). The candidate must prevent that. The agreed
interface is:
  - packaging/cham3-desktop/debian/preinst: registers the dpkg-statoverride
    (root root 0750) on install and upgrade, before wireshark-common can be
    configured in the same run, and FAILS when it cannot (Codex F3 re-review);
  - packaging/cham3-desktop/debian/triggers: 'interest-noawait /usr/bin/dumpcap',
    so the cham3 postinst re-checks dumpcap whenever wireshark-common replaces it;
  - packaging/cham3-desktop/debian/postinst: sets cap_net_admin,cap_net_raw=eip
    when setcap succeeds, never sets the setuid bit (fail closed), and fails
    instead of warning when the override cannot be registered;
  - the debconf preseed sets wireshark-common/install-setuid to false, so
    wireshark-common does not choose the set-user-id layout during the image build.

Text checks only. They cannot show that setcap, the trigger, or the group behave
as intended. Those are checked by tests/container/test_m1_container_capture.py in a
debian:trixie container. The reconfigure and upgrade paths are behavioral and are
tested only there.
"""

import os
import re
import sys
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m1_support  # noqa: E402

TriggersFile = m1_support.PackageDesktopDir + "/debian/triggers"
PostinstFile = m1_support.PackageDesktopDir + "/debian/postinst"
PreinstFile = m1_support.PackageDesktopDir + "/debian/preinst"
OverrideAdd = re.compile(r"dpkg-statoverride\s+--add\s+\"?\$\{?pin_owner\}?\"?\s+\"?\$\{?pin_group\}?\"?\s+"
                         r"\"?\$\{?pin_mode_argument\}?\"?")
OverrideAddFailure = re.compile(r"if\s+!\s+dpkg-statoverride\s+--add[^\n]*;\s*then\n(?:[^\n]*\n){0,3}?\s*(return|exit)\s+1\b")
PinValues = {"pin_owner": "root", "pin_group": "root", "pin_mode_argument": "0750"}
SetIdModeCheck = re.compile(r"\[2-7\]\[0-7\]\[0-7\]\[0-7\]")
TriggerDirective = "interest-noawait " + m1_support.DumpcapPath
PreseedLine = "wireshark-common wireshark-common/install-setuid boolean false"
RequiredCapabilityWords = ("setcap", "cap_net_admin", "cap_net_raw", "eip", "triggered")
SetuidPatterns = (
    re.compile(r"\bu\s*=\s*[rwx]*s"),
    re.compile(r"\bu\s*\+\s*s\b"),
    re.compile(r"\bchmod\s+(?:[ugoa]*\+s|\+s)\b"),
    re.compile(r"\bchmod\s+[0-7]?[4567][0-7]{3}\b"),
)


def without_shell_comments(text):
    text = re.sub(r"(?m)^\s*#.*$", "", text)
    return re.sub(r"(?m)\s#.*$", "", text)


def triggers_problems(text):
    directives = [" ".join(line.split()) for line in text.splitlines()
                  if line.strip() and not line.strip().startswith("#")]
    if TriggerDirective not in directives:
        return ["debian/triggers lacks the directive '%s'" % TriggerDirective]
    return []


def postinst_problems(text):
    """Problems with the cham3 postinst's capture handling (empty means accepted)."""
    problems = []
    code = without_shell_comments(text)
    for pattern in SetuidPatterns:
        match = pattern.search(code)
        if match:
            problems.append("postinst sets a setuid mode: %r" % match.group(0))
    for word in RequiredCapabilityWords:
        if word not in code:
            problems.append("postinst does not mention %r (capability enforcement incomplete)" % word)
    return problems


def preseed_problems(text):
    if PreseedLine not in [" ".join(line.split()) for line in text.splitlines()]:
        return ["preseed does not set install-setuid false: expected '%s'" % PreseedLine]
    return []


def override_registration_problems(text, script):
    """The script registers the root root 0750 override and fails (not warns) when it cannot."""
    problems = []
    code = without_shell_comments(text)
    for name, value in PinValues.items():
        if not re.search(r"(?m)^%s=%s$" % (name, re.escape(value)), code):
            problems.append("%s does not set %s=%s" % (script, name, value))
    if not OverrideAdd.search(code):
        problems.append("%s does not register the dpkg-statoverride" % script)
    if not OverrideAddFailure.search(code):
        problems.append("%s does not fail when the dpkg-statoverride cannot be registered" % script)
    if not SetIdModeCheck.search(code):
        problems.append("%s does not refuse an existing override with a set-id mode" % script)
    return problems


def preinst_problems(text):
    """Problems with the cham3 preinst (empty means accepted)."""
    problems = []
    code = without_shell_comments(text)
    for pattern in SetuidPatterns:
        match = pattern.search(code)
        if match:
            problems.append("preinst sets a setuid mode: %r" % match.group(0))
    if not re.search(r"(?m)^\s*install\s*\|\s*upgrade\)", code):
        problems.append("preinst does not act on install and upgrade")
    if not re.search(r"(?m)^set -e$", code):
        problems.append("preinst is not set -e")
    if not re.search(r"chmod\s+u-s,g-s", code):
        problems.append("preinst does not clear set-id bits of an installed dumpcap")
    problems += override_registration_problems(text, "preinst")
    return problems


class capture_fallback_real_t(unittest.TestCase):
    def test_trigger_watches_dumpcap(self):
        self.assertEqual(triggers_problems(m1_support.read_repo(TriggersFile)), [])

    def test_postinst_never_sets_setuid_and_enforces_capabilities(self):
        self.assertEqual(postinst_problems(m1_support.read_repo(PostinstFile)), [])

    def test_postinst_fails_when_the_override_cannot_be_registered(self):
        self.assertEqual(override_registration_problems(m1_support.read_repo(PostinstFile), "postinst"), [])

    def test_preinst_registers_the_override_before_configuration(self):
        self.assertEqual(preinst_problems(m1_support.read_repo(PreinstFile)), [])

    def test_preseed_sets_setuid_question_false(self):
        self.assertEqual(preseed_problems(m1_support.read_repo(m1_support.PreseedFile)), [])


class capture_fallback_fixture_t(unittest.TestCase):
    """Positive and negative controls for the static F3 checkers."""

    GoodPostinst = """#!/bin/sh
set -e
# Upstream wireshark-common's fallback chmod u=rwxs is never used here.
case "$1" in
    configure|triggered) ;;
    *) exit 0 ;;
esac
PROGRAM=/usr/bin/dumpcap
if command -v setcap > /dev/null && setcap cap_net_admin,cap_net_raw=eip "$PROGRAM"; then
    chown root:wireshark "$PROGRAM"
    chmod u=rwx,g=rx,o=r "$PROGRAM"
else
    setcap -r "$PROGRAM" 2> /dev/null || true
    chown root:wireshark "$PROGRAM"
    chmod u=rwx,g=rx,o= "$PROGRAM"
fi
"""

    def test_positive_control_accepted(self):
        self.assertEqual(postinst_problems(self.GoodPostinst), [])

    def test_upstream_fallback_chmod_is_rejected(self):
        text = self.GoodPostinst.replace('chmod u=rwx,g=rx,o= "$PROGRAM"', 'chmod u=rwxs,g=rx,o=r "$PROGRAM"')
        problems = postinst_problems(text)
        self.assertTrue(any("setuid" in p for p in problems), problems)

    def test_numeric_setuid_mode_is_rejected(self):
        text = self.GoodPostinst.replace('chmod u=rwx,g=rx,o= "$PROGRAM"', 'chmod 4754 "$PROGRAM"')
        problems = postinst_problems(text)
        self.assertTrue(any("setuid" in p for p in problems), problems)

    def test_plus_s_is_rejected(self):
        text = self.GoodPostinst.replace('chmod u=rwx,g=rx,o= "$PROGRAM"', 'chmod u+s "$PROGRAM"')
        problems = postinst_problems(text)
        self.assertTrue(any("setuid" in p for p in problems), problems)

    def test_numeric_non_setuid_mode_is_accepted(self):
        text = self.GoodPostinst.replace('chmod u=rwx,g=rx,o= "$PROGRAM"', 'chmod 0754 "$PROGRAM"')
        self.assertEqual(postinst_problems(text), [])

    def test_setuid_only_in_a_comment_is_accepted(self):
        text = self.GoodPostinst.replace(
            "# Upstream wireshark-common's fallback chmod u=rwxs is never used here.",
            "# never chmod u=rwxs or chmod 4754 here",
        )
        self.assertEqual(postinst_problems(text), [])

    def test_missing_setcap_enforcement_is_rejected(self):
        problems = postinst_problems(self.GoodPostinst.replace("setcap cap_net_admin,cap_net_raw=eip", "true"))
        self.assertTrue(any("setcap" in p or "cap_net_admin" in p for p in problems), problems)

    def test_missing_trigger_handling_is_rejected(self):
        problems = postinst_problems(self.GoodPostinst.replace("configure|triggered", "configure"))
        self.assertTrue(any("triggered" in p for p in problems), problems)

    def test_correct_trigger_directive_is_accepted(self):
        self.assertEqual(triggers_problems("# dpkg triggers\ninterest-noawait /usr/bin/dumpcap\n"), [])

    def test_wrong_trigger_directive_is_rejected(self):
        problems = triggers_problems("interest /usr/bin/dumpcap\n")
        self.assertTrue(problems)

    def test_missing_trigger_file_content_is_rejected(self):
        self.assertTrue(triggers_problems("interest-noawait /usr/bin/other\n"))

    def test_preseed_false_is_accepted(self):
        self.assertEqual(preseed_problems(PreseedLine + "\n"), [])

    def test_preseed_true_is_rejected(self):
        problems = preseed_problems("wireshark-common wireshark-common/install-setuid boolean true\n")
        self.assertTrue(problems)

    GoodPreinst = """#!/bin/sh
set -e
pin_owner=root
pin_group=root
pin_mode_argument=0750
register() {
    current=$(dpkg-statoverride --list /usr/bin/dumpcap) || current=
    if [ -z "$current" ]; then
        if ! dpkg-statoverride --add "${pin_owner}" "${pin_group}" "${pin_mode_argument}" /usr/bin/dumpcap; then
            echo "cannot register" >&2
            return 1
        fi
    fi
    case "$(echo "$current" | awk '{ print $3 }')" in
        [2-7][0-7][0-7][0-7]) return 1 ;;
    esac
    [ -f /usr/bin/dumpcap ] && chmod u-s,g-s /usr/bin/dumpcap
}
case "$1" in
    install | upgrade)
        register
        ;;
esac
"""

    def test_preinst_positive_control_accepted(self):
        self.assertEqual(preinst_problems(self.GoodPreinst), [])

    def test_preinst_that_only_warns_is_rejected(self):
        # Negative control for F3: the 0fec3a0 postinst behaviour (log and continue).
        text = self.GoodPreinst.replace('            echo "cannot register" >&2\n            return 1\n',
                                        '            echo "WARNING: cannot register" >&2\n')
        problems = preinst_problems(text)
        self.assertTrue(any("does not fail when the dpkg-statoverride cannot be registered" in p
                            for p in problems), problems)

    def test_preinst_without_override_is_rejected(self):
        text = self.GoodPreinst.replace('dpkg-statoverride --add "${pin_owner}" "${pin_group}" "${pin_mode_argument}"',
                                        "true")
        problems = preinst_problems(text)
        self.assertTrue(any("does not register the dpkg-statoverride" in p for p in problems), problems)

    def test_preinst_acting_only_on_upgrade_is_rejected(self):
        problems = preinst_problems(self.GoodPreinst.replace("install | upgrade)", "upgrade)"))
        self.assertTrue(any("install and upgrade" in p for p in problems), problems)

    def test_preinst_with_setuid_override_mode_is_rejected(self):
        problems = preinst_problems(self.GoodPreinst.replace("pin_mode_argument=0750", "pin_mode_argument=4754"))
        self.assertTrue(any("pin_mode_argument=0750" in p for p in problems), problems)

    def test_preinst_accepting_set_id_overrides_is_rejected(self):
        problems = preinst_problems(self.GoodPreinst.replace("[2-7][0-7][0-7][0-7]) return 1 ;;", ""))
        self.assertTrue(any("set-id mode" in p for p in problems), problems)


if __name__ == "__main__":
    unittest.main()
