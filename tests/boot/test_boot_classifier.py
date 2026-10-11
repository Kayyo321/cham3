"""Synthetic-log tests for classify_boot_log() in build/vm/run_boot_test.py.

The classifier is pure: it is imported from the implementation file and fed
serial-console text. PASS needs a login prompt or a live-config autologin
shell. FAIL markers must win over PASS markers. An incomplete log is PENDING,
never PASS.
"""

import importlib.util
import os
import sys
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402

BootScriptPath = "build/vm/run_boot_test.py"
LoginLog = (
    "Debian GNU/Linux 13 cham3 ttyS0\n\n"
    "cham3 login: "
)
AutologinLog = (
    "[  OK  ] Started getty on ttyS0.\n"
    "user@cham3:~$ "
)
KernelPanicLog = "[    3.1] Kernel panic - not syncing: VFS: Unable to mount root fs\n"
InitramfsLog = "Gave up waiting for root file system device.\n(initramfs) "
NoMediumLog = "live-boot: Unable to find a medium containing a live file system\n"
EmergencyLog = (
    "You are in emergency mode. After logging in, type \"journalctl -xb\"\n"
)
IncompleteLog = "Booting Linux on physical CPU 0x0\n[    0.000000] Linux version 6.12\n"


def load_boot_module():
    path = os.path.join(m0_support.repo_root(), BootScriptPath)
    m0_support.require_file(path, "boot test entry point")
    spec = importlib.util.spec_from_file_location("cham3_run_boot_test", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    if not hasattr(module, "classify_boot_log"):
        raise AssertionError("classify_boot_log is not defined in " + BootScriptPath)
    return module


def classify(log_text):
    result = load_boot_module().classify_boot_log(log_text)
    if not isinstance(result, dict):
        raise AssertionError("classify_boot_log must return a dict")
    return result


class boot_classifier_t(unittest.TestCase):
    def assert_result(self, log_text, expected):
        result = classify(log_text)
        self.assertEqual(result.get("result"), expected)
        self.assertIsInstance(result.get("reason"), str)
        self.assertTrue(result["reason"].strip(), "reason must not be empty")

    def test_login_prompt_passes(self):
        self.assert_result(LoginLog, "PASS")

    def test_live_config_autologin_shell_passes(self):
        self.assert_result(AutologinLog, "PASS")

    def test_kernel_panic_fails(self):
        self.assert_result(KernelPanicLog, "FAIL")

    def test_initramfs_shell_fails(self):
        self.assert_result(InitramfsLog, "FAIL")

    def test_missing_live_medium_fails(self):
        self.assert_result(NoMediumLog, "FAIL")

    def test_emergency_mode_fails(self):
        self.assert_result(EmergencyLog, "FAIL")

    def test_fail_marker_overrides_login_prompt(self):
        self.assert_result(LoginLog + KernelPanicLog, "FAIL")

    def test_incomplete_log_is_pending_not_pass(self):
        self.assert_result(IncompleteLog, "PENDING")

    def test_empty_log_is_pending_not_pass(self):
        self.assert_result("", "PENDING")


class boot_rescue_and_root_shell_t(unittest.TestCase):
    """Regression tests for review finding F5: rescue mode, the maintenance
    password prompt and an unexpected root shell are not a healthy boot."""

    def classify_result(self, log_text):
        result = classify(log_text)
        self.assertIsInstance(result.get("reason"), str)
        self.assertTrue(result["reason"].strip(), "reason must not be empty")
        return result

    def test_rescue_mode_fails(self):
        log_text = (
            "[  OK  ] Reached target rescue.target.\n"
            "You are in rescue mode. After logging in, type \"journalctl -xb\" to view system logs.\n"
        )
        self.assertEqual(self.classify_result(log_text)["result"], "FAIL")

    def test_maintenance_password_prompt_fails(self):
        log_text = (
            "Give root password for maintenance\n"
            "(or press Control-D to continue): "
        )
        self.assertEqual(self.classify_result(log_text)["result"], "FAIL")

    def test_bare_root_shell_without_login_prompt_fails(self):
        log_text = "Debian GNU/Linux 13 debian ttyS0\n\nroot@debian:~# "
        result = self.classify_result(log_text)
        self.assertEqual(result["result"], "FAIL")
        self.assertIn("unexpected root shell", result["reason"])

    def test_non_root_autologin_prompt_passes(self):
        log_text = "Debian GNU/Linux 13 debian ttyS0\n\nuser@debian:~$ "
        self.assertEqual(self.classify_result(log_text)["result"], "PASS")

    def test_login_prompt_passes(self):
        log_text = "Debian GNU/Linux 13 debian ttyS0\n\ndebian login: "
        self.assertEqual(self.classify_result(log_text)["result"], "PASS")


if __name__ == "__main__":
    unittest.main()
