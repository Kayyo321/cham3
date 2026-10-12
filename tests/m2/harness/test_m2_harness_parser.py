"""Sensitivity of the M2 result validator to synthetic cargo/unittest outputs."""

import unittest

from m2_harness_support import m2_harness

Inventory = {"cargo:cham3_demo": {"tests::alpha", "tests::beta"}, "python:demo": {"test_demo.demo_t.test_one"}}


def cargo_output(lines, running=None, result=None):
    tests = "\n".join(lines)
    count = len(lines) if running is None else running
    result = result or "test result: ok. %d passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s" % len(lines)
    return (
        "   Compiling cham3_demo v0.1.0\n"
        "     Running unittests src/lib.rs (out/target/debug/deps/cham3_demo-0123456789abcdef)\n\n"
        "running %d tests\n%s\n\n%s\n" % (count, tests, result)
    )


Valid = cargo_output(["test tests::alpha ... ok", "test tests::beta ... ok"])


def validate(text, status=0, inventory=Inventory):
    return m2_harness.validate_suites(m2_harness.parse_cargo_test_output(text), status, inventory, "cargo")


class harness_parser_t(unittest.TestCase):
    def test_valid_output_is_accepted(self):
        self.assertEqual(validate(Valid), {"cargo:cham3_demo": ["tests::alpha", "tests::beta"]})

    def test_nonzero_exit_status_is_rejected(self):
        with self.assertRaisesRegex(m2_harness.harness_error_t, "non-zero exit"):
            validate(Valid, status=101)

    def test_zero_tests_are_rejected(self):
        text = cargo_output([], running=0, result="test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s")
        with self.assertRaisesRegex(m2_harness.harness_error_t, "zero tests passed"):
            validate(text)

    def test_failed_test_is_rejected(self):
        text = cargo_output(
            ["test tests::alpha ... ok", "test tests::beta ... FAILED"],
            result="test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s",
        )
        with self.assertRaisesRegex(m2_harness.harness_error_t, "failed"):
            validate(text, status=101)

    def test_ignored_test_is_rejected(self):
        text = cargo_output(
            ["test tests::alpha ... ok", "test tests::beta ... ignored"],
            result="test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s",
        )
        with self.assertRaisesRegex(m2_harness.harness_error_t, "ignored"):
            validate(text)

    def test_filtered_run_is_rejected(self):
        text = cargo_output(
            ["test tests::alpha ... ok"],
            result="test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s",
        )
        with self.assertRaisesRegex(m2_harness.harness_error_t, "filtered"):
            validate(text)

    def test_missing_result_line_is_rejected(self):
        text = Valid.split("test result:")[0]
        with self.assertRaisesRegex(m2_harness.harness_error_t, "no result line"):
            validate(text)

    def test_compile_only_output_is_rejected(self):
        text = "   Compiling cham3_demo v0.1.0\n  Executable unittests src/lib.rs (out/target/debug/deps/cham3_demo-0123456789abcdef)\n"
        with self.assertRaisesRegex(m2_harness.harness_error_t, "compile-only"):
            validate(text)

    def test_empty_output_is_rejected(self):
        with self.assertRaisesRegex(m2_harness.harness_error_t, "no test suites"):
            validate("")

    def test_doc_test_suite_is_rejected(self):
        text = Valid + "   Doc-tests cham3_demo\n\nrunning 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n"
        with self.assertRaisesRegex(m2_harness.harness_error_t, "doc-test"):
            validate(text)

    def test_inventory_test_not_executed_is_rejected(self):
        text = cargo_output(["test tests::alpha ... ok"])
        with self.assertRaisesRegex(m2_harness.harness_error_t, "inventory tests not executed: tests::beta"):
            validate(text)

    def test_unlisted_test_is_rejected(self):
        text = cargo_output(["test tests::alpha ... ok", "test tests::beta ... ok", "test tests::gamma ... ok"])
        with self.assertRaisesRegex(m2_harness.harness_error_t, "missing from inventory: tests::gamma"):
            validate(text)

    def test_unlisted_suite_is_rejected(self):
        with self.assertRaisesRegex(m2_harness.harness_error_t, "inventory tests not executed"):
            validate(Valid, inventory={"cargo:cham3_other": {"tests::alpha"}})

    def test_running_count_mismatch_is_rejected(self):
        text = cargo_output(["test tests::alpha ... ok", "test tests::beta ... ok"], running=3)
        with self.assertRaisesRegex(m2_harness.harness_error_t, "running 3 but passed 2"):
            validate(text)

    def test_duplicate_test_line_is_rejected(self):
        text = cargo_output(["test tests::alpha ... ok", "test tests::alpha ... ok"])
        with self.assertRaisesRegex(m2_harness.harness_error_t, "twice"):
            validate(text)

    def test_python_valid_output_is_accepted(self):
        text = "test_one (test_demo.demo_t.test_one) ... ok\n\n----\nRan 1 test in 0.001s\n\nOK\n"
        parsed = m2_harness.parse_python_unittest_output(text)
        self.assertEqual(m2_harness.validate_python_suite(parsed, 0, Inventory, "demo"), ["test_demo.demo_t.test_one"])

    def test_python_skip_is_rejected(self):
        text = "test_one (test_demo.demo_t.test_one) ... skipped 'tool missing'\n\nRan 1 test in 0.001s\n\nOK (skipped=1)\n"
        parsed = m2_harness.parse_python_unittest_output(text)
        with self.assertRaisesRegex(m2_harness.harness_error_t, "skip"):
            m2_harness.validate_python_suite(parsed, 0, Inventory, "demo")

    def test_python_zero_tests_are_rejected(self):
        parsed = m2_harness.parse_python_unittest_output("\nRan 0 tests in 0.000s\n\nNO TESTS RAN\n")
        with self.assertRaisesRegex(m2_harness.harness_error_t, "zero tests ran"):
            m2_harness.validate_python_suite(parsed, 5, Inventory, "demo")

    def test_python_failure_is_rejected(self):
        text = "test_one (test_demo.demo_t.test_one) ... FAIL\n\nRan 1 test in 0.001s\n\nFAILED (failures=1)\n"
        parsed = m2_harness.parse_python_unittest_output(text)
        with self.assertRaisesRegex(m2_harness.harness_error_t, "FAILED"):
            m2_harness.validate_python_suite(parsed, 1, Inventory, "demo")

    def test_stale_run_directory_is_refused(self):
        import tempfile, os

        with tempfile.TemporaryDirectory() as run_dir:
            os.makedirs(os.path.join(run_dir, "target_debug"))
            with self.assertRaisesRegex(m2_harness.harness_error_t, "stale build"):
                m2_harness.run_cargo_suite("debug", run_dir)


if __name__ == "__main__":
    unittest.main()
