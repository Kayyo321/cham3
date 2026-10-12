"""Regression tests for the revision-3 trusted-time rule (review finding R09-r2).

Exercises tests/m2/oracle/trusted_time_model.py, the executable statement of ADR 0007
section 5: the reviewer's forward-step / backward-correction / idle counterexample, long
suspend, same-boot restart from floors and from the ledger, clean shutdown, regressing
floors, two services sharing offsets, new-boot limits and a seeded property check. The
revision-2 inequalities are kept as a contrast showing the counterexample. This is a
design model, not the service clock (NOT_RUN until M2-006).
"""

import importlib.util
import os
import random
import unittest

from m2_harness_support import RepoRoot

ModelPath = os.path.join(RepoRoot, "tests", "m2", "oracle", "trusted_time_model.py")


def load_model():
    spec = importlib.util.spec_from_file_location("cham3_trusted_time_model", ModelPath)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


Hour = 3600
Noon = 12 * Hour


class trusted_time_t(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.model = load_model()

    def reviewer_sequence(self, clock):
        clock.sample(Noon, 0)
        clock.sample(14 * Hour, 60)          # forward step observed at BOOTTIME 60 s
        clock.sample(13 * Hour, 61)          # wall clock corrected back

    def test_reviewer_counterexample_rebases_on_forward_sample(self):
        clock = self.model.trusted_clock_t("boot-a", Noon, 0)
        self.reviewer_sequence(clock)
        # One hour of BOOTTIME after the 14:00 observation, wall clock still corrected.
        trusted = clock.sample(13 * Hour + 3600, 3660)
        self.assertGreaterEqual(trusted, 15 * Hour)
        self.assertTrue(clock.expired(14 * Hour + 30 * 60, 3660), "a 14:30 expiry must be past an hour after 14:00")
        self.assertTrue(clock.anomalies, "a correction more than 300 s back is an anomaly")

    def test_revision_2_bounds_reproduce_the_defect(self):
        # Control: the disproved revision-2 rule freezes at 14:00 (review appendix).
        high = self.model.revision_2_high_water(Noon, 0, [(14 * Hour, 60), (13 * Hour, 61), (13 * Hour + 60, 3660)])
        self.assertEqual(high, 14 * Hour)
        self.assertLess(high, 14 * Hour + 30 * 60)

    def test_long_suspend_after_correction_advances(self):
        clock = self.model.trusted_clock_t("boot-a", Noon, 0)
        self.reviewer_sequence(clock)
        # Ten hours of suspend: BOOTTIME keeps counting, realtime set back further.
        trusted = clock.sample(11 * Hour, 61 + 10 * Hour)
        self.assertEqual(trusted, 14 * Hour + 1 + 10 * Hour)

    def test_same_boot_restart_from_trusted_floor(self):
        clock = self.model.trusted_clock_t("boot-a", Noon, 0)
        self.reviewer_sequence(clock)
        floor = clock.floor(120)
        self.assertEqual(floor.trusted, 14 * Hour + 60, "the floor stores trusted time, not the corrected realtime")
        # Service down for 30 minutes; restarts with the corrected (13:xx) wall clock.
        restarted = self.model.trusted_clock_t("boot-a", 13 * Hour + 1800, 120 + 1800, [floor])
        self.assertEqual(restarted.now(120 + 1800), 14 * Hour + 60 + 1800)

    def test_raw_realtime_floor_would_regress(self):
        # Control for the second review issue: a floor holding raw realtime (13:00) would
        # let a restart move trusted time back by an hour.
        raw_floor = self.model.floor_t("boot-a", 120, 13 * Hour)
        restarted = self.model.trusted_clock_t("boot-a", 13 * Hour + 10, 130, [raw_floor])
        self.assertLess(restarted.now(130), 14 * Hour)

    def test_restart_from_ledger_entry_newer_than_journal_floor(self):
        clock = self.model.trusted_clock_t("boot-a", Noon, 0)
        journal_floor = clock.floor(10)
        clock.sample(14 * Hour, 60)
        ledger_entry = clock.floor(60)   # every offset increase reaches the tmpfs ledger first
        restarted = self.model.trusted_clock_t("boot-a", 13 * Hour, 90, [journal_floor, ledger_entry])
        self.assertEqual(restarted.now(90), 14 * Hour + 30)

    def test_clean_shutdown_floor_then_restart(self):
        clock = self.model.trusted_clock_t("boot-a", Noon, 0)
        self.reviewer_sequence(clock)
        shutdown_floor = clock.floor(500)
        restarted = self.model.trusted_clock_t("boot-a", 13 * Hour, 600, [shutdown_floor])
        self.assertEqual(restarted.now(600), 14 * Hour + 540)

    def test_regressing_floor_cannot_lower_trusted_time(self):
        high = self.model.floor_t("boot-a", 100, 14 * Hour + 40)
        low = self.model.floor_t("boot-a", 200, 13 * Hour)
        for floors in ([high, low], [low, high]):
            restarted = self.model.trusted_clock_t("boot-a", 13 * Hour, 300, floors)
            self.assertEqual(restarted.now(300), 14 * Hour + 240)

    def test_floors_written_by_a_clock_never_regress(self):
        clock = self.model.trusted_clock_t("boot-a", Noon, 0)
        previous = clock.floor(0).trusted
        for boottime, realtime in ((10, 14 * Hour), (20, 13 * Hour), (30, 16 * Hour), (40, Noon)):
            clock.sample(realtime, boottime)
            value = clock.floor(boottime).trusted
            self.assertGreaterEqual(value, previous + 10 if boottime > 10 else previous)
            previous = value

    def test_two_services_share_the_maximum_offset(self):
        policyd = self.model.trusted_clock_t("boot-a", Noon, 0)
        registryd = self.model.trusted_clock_t("boot-a", Noon, 0)
        policyd.sample(14 * Hour, 60)                 # only policyd saw the forward step
        registryd.adopt(policyd.offset)               # reported before policyd relies on it
        registryd.sample(13 * Hour, 61)
        floor = registryd.floor(61)
        restarted_policyd = self.model.trusted_clock_t("boot-a", 13 * Hour, 70, [floor])
        self.assertEqual(restarted_policyd.now(70), registryd.now(70))
        self.assertEqual(registryd.now(3660), 15 * Hour)

    def test_new_boot_starts_no_earlier_than_last_floor(self):
        clock = self.model.trusted_clock_t("boot-a", Noon, 0)
        self.reviewer_sequence(clock)
        last = clock.floor(3660)
        # Reboot with the hardware clock set back: elapsed time is unknown (stated limit),
        # trusted time never starts before the last persisted value.
        rebooted = self.model.trusted_clock_t("boot-b", 13 * Hour, 5, [last])
        self.assertEqual(rebooted.now(5), last.trusted)
        self.assertEqual(rebooted.now(65), last.trusted + 60)
        # A later wall clock is accepted on the new boot.
        self.assertEqual(rebooted.sample(17 * Hour, 70), 17 * Hour)

    def test_seeded_property_monotonic_and_boottime_paced(self):
        generator = random.Random(20261012)
        for _ in range(200):
            floors = []
            clock = self.model.trusted_clock_t("boot-p", Noon, 0)
            boottime, accepted = 0, (0, Noon)
            previous = clock.now(0)
            for _ in range(40):
                boottime += generator.randint(0, 7200)
                action = generator.random()
                if action < 0.15:
                    floors.append(clock.floor(boottime))
                    clock = self.model.trusted_clock_t("boot-p", Noon + generator.randint(-5 * Hour, 5 * Hour), boottime, floors)
                    trusted = clock.now(boottime)
                else:
                    realtime = previous + generator.randint(-4 * Hour, 4 * Hour)
                    trusted = clock.sample(realtime, boottime)
                    if trusted == realtime and realtime >= accepted[1] + (boottime - accepted[0]):
                        accepted = (boottime, realtime)
                self.assertGreaterEqual(trusted, previous)
                self.assertGreaterEqual(trusted, accepted[1] + (boottime - accepted[0]))
                previous = trusted


if __name__ == "__main__":
    unittest.main()
