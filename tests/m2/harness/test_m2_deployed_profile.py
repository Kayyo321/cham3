"""Deployed-profile behavior of real binaries (not the cargo test harness).

cargo test always builds the harness with unwinding, so panic=abort and overflow
trapping in the release (deployment) profile are verified here by running the
release and dev builds of cham3_provider_probe as subprocesses. The dispatcher sets
CHAM3_M2_PROBE_DEBUG and CHAM3_M2_PROBE_RELEASE; their absence is a failure.
"""

import os
import signal
import subprocess
import unittest

UnwindExitStatus = 101


def probe(variable):
    path = os.environ.get(variable)
    if not path or not os.access(path, os.X_OK):
        raise AssertionError("%s must name the built probe binary (got %r)" % (variable, path))
    return path


def run(binary, *arguments):
    completed = subprocess.run([binary] + list(arguments), stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=False)
    return completed.returncode, completed.stdout, completed.stderr


class deployed_profile_t(unittest.TestCase):
    def test_release_panic_aborts(self):
        status, _, stderr = run(probe("CHAM3_M2_PROBE_RELEASE"), "panic")
        self.assertEqual(status, -signal.SIGABRT, stderr)
        self.assertIn("deliberate panic", stderr)

    def test_dev_panic_unwinds(self):
        status, _, stderr = run(probe("CHAM3_M2_PROBE_DEBUG"), "panic")
        self.assertEqual(status, UnwindExitStatus, stderr)

    def test_release_overflow_traps_and_aborts(self):
        status, stdout, stderr = run(probe("CHAM3_M2_PROBE_RELEASE"), "overflow", "1")
        self.assertEqual(status, -signal.SIGABRT, stdout + stderr)
        self.assertIn("overflow", stderr)
        self.assertNotIn("NO_OVERFLOW_TRAP", stdout)

    def test_dev_overflow_traps(self):
        status, stdout, stderr = run(probe("CHAM3_M2_PROBE_DEBUG"), "overflow", "1")
        self.assertEqual(status, UnwindExitStatus, stdout + stderr)
        self.assertIn("overflow", stderr)

    def test_overflow_probe_positive_control(self):
        # Adding zero does not overflow: the trap above is input-dependent, not a
        # constant failure of the probe.
        status, stdout, _ = run(probe("CHAM3_M2_PROBE_RELEASE"), "overflow", "0")
        self.assertEqual(status, 1)
        self.assertIn("NO_OVERFLOW_TRAP", stdout)

    def test_release_provider_report_passes(self):
        status, stdout, stderr = run(probe("CHAM3_M2_PROBE_RELEASE"), "report")
        self.assertEqual(status, 0, stdout + stderr)
        self.assertEqual(len([line for line in stdout.splitlines() if line.startswith("PASS ")]), 7, stdout)

    def test_release_hardening_makes_process_non_dumpable(self):
        status, stdout, stderr = run(probe("CHAM3_M2_PROBE_RELEASE"), "harden")
        self.assertEqual(status, 0, stdout + stderr)
        self.assertIn("DUMPABLE false", stdout)

    def test_unknown_mode_is_usage_error(self):
        status, _, _ = run(probe("CHAM3_M2_PROBE_RELEASE"), "unknown-mode")
        self.assertEqual(status, 2)


if __name__ == "__main__":
    unittest.main()
