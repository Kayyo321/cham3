"""Regression tests for the trusted-time rule (review findings R09-r2 and R15).

Exercises tests/m2/oracle/trusted_time_model.py, the executable statement of ADR 0007
section 5: the reviewer's forward-step / backward-correction / idle counterexample, long
suspend, same-boot restart from floors and from the ledger, clean shutdown, regressing
floors, two services sharing offsets, new-boot limits and a seeded property check. The
revision-2 inequalities are kept as a contrast showing the counterexample. Revision 4
(R15, re-review f451f2e) adds the stated limits of section 5.7 — accumulated cross-boot
lag and acceleration of an already-admitted future start by an accepted forward error —
with the reviewer's reproductions, a seeded check of the corrected lag bound, and the
owner option B model of section 8 (PROPOSED, not selected). This is a design model, not
the service clock: M2-006 stage B1 implements it in core/registryd/src/clock.rs, and
the published vectors tests/m2/vectors/trusted_time_v1/vectors.json, generated from this
model by tests/m2/oracle/trusted_time_vectors.py, are replayed through it by the cargo
test trusted_time_model.
"""

import importlib.util
import json
import os
import random
import subprocess
import sys
import unittest

from m2_harness_support import RepoRoot

ModelPath = os.path.join(RepoRoot, "tests", "m2", "oracle", "trusted_time_model.py")
VectorScript = os.path.join(RepoRoot, "tests", "m2", "oracle", "trusted_time_vectors.py")
VectorPath = os.path.join(RepoRoot, "tests", "m2", "vectors", "trusted_time_v1", "vectors.json")


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

    def test_published_vectors_are_the_models(self):
        result = subprocess.run([sys.executable, "-I", VectorScript, "--check", VectorPath],
                                capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        with open(VectorPath, encoding="utf-8") as handle:
            document = json.load(handle)
        operations = {step["op"] for case in document["cases"] for step in case["steps"]}
        self.assertEqual(operations, {"sample", "adopt", "floor", "restart"})
        self.assertGreaterEqual(len(document["cases"]), 165)
        anomalies = [step["anomaly"] for case in document["cases"] for step in case["steps"] if step["op"] == "sample"]
        self.assertIn(True, anomalies)
        self.assertIn(False, anomalies)
        boots = {step["boot_id"] for case in document["cases"] for step in case["steps"] if step["op"] == "restart"}
        self.assertGreater(len(boots), 1, "restarts in a new boot")

    def test_a_changed_vector_is_detected(self):
        with open(VectorPath, encoding="utf-8") as handle:
            text = handle.read()
        changed = text.replace('"trusted":1791813601', '"trusted":1791813602', 1)
        self.assertNotEqual(changed, text)
        path = os.path.join(os.environ.get("TMPDIR", "/tmp"), "cham3_trusted_time_changed_%d.json" % os.getpid())
        try:
            with open(path, "w", encoding="utf-8") as handle:
                handle.write(changed)
            result = subprocess.run([sys.executable, "-I", VectorScript, "--check", path],
                                    capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 1, result.stdout)
        finally:
            os.remove(path)

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

    # ------------------------------------------------------------ R15: stated limits

    def test_r15_reviewer_cross_boot_lag_exceeds_latest_floor_age(self):
        # Re-review f451f2e: boot A persists noon; two hours powered off with the clock
        # reset to noon; boot B runs 60 s and persists 12:01 (actual 14:01); boot C.
        boot_a = self.model.trusted_clock_t("a", Noon, 0)
        floor_a = boot_a.floor(0)
        boot_b = self.model.trusted_clock_t("b", Noon, 0, [floor_a])
        floor_b = boot_b.floor(60)
        boot_c = self.model.trusted_clock_t("c", Noon + 60, 0, [floor_a, floor_b])
        trusted, actual, expiration = boot_c.now(5), 14 * Hour + 65, Noon + 30 * 60
        self.assertEqual(trusted, Noon + 65)
        self.assertEqual(expiration - trusted, 1735, "a 12:30 expiration remains usable although actual time is 14:01")
        floor_age = 5
        self.assertGreater(actual - trusted, floor_age, "the revision-3 latest-floor bound is false")
        unobserved = 2 * Hour  # powered-off time since trusted time last equalled actual time
        self.assertEqual(actual - trusted, unobserved, "the corrected bound of section 5.7a holds with equality")

    def test_r15_lag_accumulates_over_reboots_until_an_accurate_sample(self):
        floors = [self.model.trusted_clock_t("boot-0", Noon, 0).floor(0)]
        actual = Noon
        for boot in range(1, 4):
            actual += Hour  # an hour powered off; the hardware clock comes back at the last floor value
            clock = self.model.trusted_clock_t("boot-%d" % boot, floors[-1].trusted, 0, floors)
            floors.append(clock.floor(0))
        self.assertEqual(actual - clock.now(0), 3 * Hour, "three unobserved hours, whatever the floor ages")
        self.assertEqual(clock.sample(actual + 10, 10), actual + 10, "an accurate wall-clock sample repairs the lag")

    def test_r15_seeded_property_lag_bounded_by_unobserved_time(self):
        generator = random.Random(20261013)
        counterexamples = 0
        for run in range(200):
            actual, boottime, boot = Noon, 0, 0
            clock = self.model.trusted_clock_t("p-%d-0" % run, actual, boottime)   # first boot accurate
            floors = [clock.floor(boottime)]
            bound = 0
            floor_actual, floor_bound = actual, bound
            for _ in range(60):
                action = generator.random()
                if action < 0.15:
                    # Reboot after a random powered-off interval; the wall clock may be behind.
                    actual += generator.randint(0, 4 * Hour)
                    boot, boottime = boot + 1, generator.randint(0, 30)
                    actual += boottime
                    behind = 0 if generator.random() < 0.2 else generator.randint(0, 6 * Hour)
                    clock = self.model.trusted_clock_t("p-%d-%d" % (run, boot), actual - behind, boottime, floors)
                    bound = floor_bound + (actual - floor_actual)
                    if behind == 0:
                        bound = 0
                elif action < 0.4:
                    floors.append(clock.floor(boottime))
                    floor_actual, floor_bound = actual, bound
                elif action < 0.7:
                    accurate = generator.random() < 0.1
                    clock.sample(actual if accurate else actual - generator.randint(0, 4 * Hour), boottime)
                    if accurate:
                        bound = 0
                else:
                    step = generator.randint(0, 7200)
                    actual, boottime = actual + step, boottime + step
                trusted = clock.now(boottime)
                self.assertLessEqual(trusted, actual, "a wall clock never ahead of actual time never makes trusted time ahead")
                self.assertLessEqual(actual - trusted, bound, "lag exceeds the unobserved time since the last sync")
                if actual - trusted > actual - floor_actual:
                    counterexamples += 1
        self.assertGreater(counterexamples, 0, "the generator must also reach the re-review's latest-floor counterexample")

    def test_r15_forward_error_accelerates_admitted_future_start(self):
        # Re-review f451f2e: a grant admitted at noon (timestamp within +-300 s) with a
        # 15:00 start; a later accepted forward sample makes it eligible immediately.
        clock = self.model.trusted_clock_t("d", Noon, 0)
        start = 15 * Hour
        self.assertTrue(self.model.admission_timestamp_ok(clock, Noon, 0))
        self.assertLess(clock.now(0), start)
        clock.sample(30 * 24 * Hour, 1)
        self.assertGreaterEqual(clock.now(1), start, "eligible at actual 12:00:01: the admission check bounds nothing here")

    def test_r15_forward_error_is_never_corrected(self):
        clock = self.model.trusted_clock_t("e", Noon, 0)
        clock.sample(Noon + 30 * 24 * Hour, 10)
        clock.sample(Noon + 20, 20)  # the wall clock is corrected back
        self.assertTrue(clock.anomalies)
        self.assertGreaterEqual(clock.now(20), Noon + 30 * 24 * Hour, "trusted time never moves back within the boot")
        restarted = self.model.trusted_clock_t("e", Noon + 30, 30, [clock.floor(20)])
        rebooted = self.model.trusted_clock_t("f", Noon + 60, 0, [clock.floor(20)])
        for later in (restarted.now(30), rebooted.now(0)):
            self.assertGreaterEqual(later, Noon + 30 * 24 * Hour, "floors carry the error to restarts and new boots")
        self.assertFalse(self.model.admission_timestamp_ok(rebooted, Noon + 60, 0),
                         "honest commit timestamps are refused until actual time catches up (availability limit)")

    def test_r15_option_b_observed_start_is_not_accelerated(self):
        # Owner option B (ADR 0007 section 8, PROPOSED, not selected).
        trusted = self.model.trusted_clock_t("g", Noon, 0)
        observed = self.model.observed_clock_t("g", 0)
        start, admitted_trusted, admitted_observed = 15 * Hour, trusted.now(0), observed.now(0)
        trusted.sample(Noon + 30 * 24 * Hour, 1)
        self.assertFalse(self.model.observed_start_eligible(start, admitted_trusted, admitted_observed, trusted.now(1), observed.now(1)),
                         "a forward error accepted after admission does not advance the start")
        with self.assertRaises(TypeError):
            observed.sample(Noon + 30 * 24 * Hour, 2)
        self.assertTrue(self.model.observed_start_eligible(start, admitted_trusted, admitted_observed,
                                                           trusted.now(3 * Hour), observed.now(3 * Hour)))

    def test_r15_option_b_cost_powered_off_time_never_counts(self):
        trusted = self.model.trusted_clock_t("h", Noon, 0)
        observed = self.model.observed_clock_t("h", 0)
        start, admitted_trusted, admitted_observed = 15 * Hour, trusted.now(0), observed.now(0)
        floors = [trusted.floor(Hour)]
        observed_floors = [observed.floor(Hour)]
        # Two hours powered off, then an accurate clock: 15:00 is reached in actual time.
        trusted_next = self.model.trusted_clock_t("i", 15 * Hour, 0, floors)
        observed_next = self.model.observed_clock_t("i", 0, observed_floors)
        self.assertFalse(self.model.observed_start_eligible(start, admitted_trusted, admitted_observed,
                                                            trusted_next.now(0), observed_next.now(0)),
                         "the start waits for two more observed hours (stated cost)")
        self.assertTrue(self.model.observed_start_eligible(start, admitted_trusted, admitted_observed,
                                                           trusted_next.now(2 * Hour), observed_next.now(2 * Hour)))
        restarted = self.model.observed_clock_t("i", 2 * Hour + 10, [observed_next.floor(2 * Hour)])
        self.assertEqual(restarted.now(2 * Hour + 10), 3 * Hour + 10, "same-boot restart resumes the observed count")

    def test_r15_option_b_bounded_by_error_present_at_admission(self):
        trusted = self.model.trusted_clock_t("j", Noon + 600, 0)  # already 10 minutes ahead at admission
        observed = self.model.observed_clock_t("j", 0)
        start, admitted_trusted, admitted_observed = 15 * Hour, trusted.now(0), observed.now(0)
        early = 3 * Hour - 600  # actual 14:50: early by exactly the error present at admission
        self.assertFalse(self.model.observed_start_eligible(start, admitted_trusted, admitted_observed,
                                                            trusted.now(early - 1), observed.now(early - 1)))
        self.assertTrue(self.model.observed_start_eligible(start, admitted_trusted, admitted_observed,
                                                           trusted.now(early), observed.now(early)))


if __name__ == "__main__":
    unittest.main()
