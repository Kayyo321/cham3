"""Sensitivity of the M2 validator against real cargo test runs of canary crates.

Each canary is a dependency-free package compiled offline with the pinned toolchain.
The validator must accept only the healthy canary.
"""

import unittest

from m2_harness_support import canary_project_t, m2_harness

Healthy = """
pub fn add(left: u32, right: u32) -> u32 { left + right }
#[cfg(test)]
mod tests {
    #[test]
    fn adds() { assert_eq!(super::add(2, 2), 4); }
    #[test]
    fn adds_zero() { assert_eq!(super::add(0, 7), 7); }
}
"""
Inventory = {"cargo:cham3_canary": {"tests::adds", "tests::adds_zero"}}


class harness_canary_t(unittest.TestCase):
    def outcome(self, source, *extra):
        project = canary_project_t(source)
        try:
            status, output = project.cargo_test(*extra)
        finally:
            project.remove()
        try:
            m2_harness.validate_suites(m2_harness.parse_cargo_test_output(output), status, Inventory, "cargo")
        except m2_harness.harness_error_t as error:
            return "rejected", str(error), output
        return "accepted", "", output

    def assert_rejected(self, source, *extra, reason=None):
        verdict, error, output = self.outcome(source, *extra)
        self.assertEqual(verdict, "rejected", output)
        if reason:
            self.assertRegex(error, reason)

    def test_healthy_canary_is_accepted(self):
        verdict, error, output = self.outcome(Healthy)
        self.assertEqual(verdict, "accepted", error + "\n" + output)

    def test_assertion_failure_is_rejected(self):
        self.assert_rejected(Healthy.replace("== 4", "== 5").replace("(2, 2), 4", "(2, 2), 5"), reason="non-zero exit|failed")

    def test_compile_error_is_rejected(self):
        self.assert_rejected(Healthy.replace("left + right", "left + "), reason="non-zero exit|no test suites")

    def test_link_failure_is_rejected(self):
        source = Healthy.replace(
            "fn adds() {",
            'fn adds() { unsafe extern "C" { fn cham3_missing_symbol() -> i32; } '
            "assert_eq!(unsafe { cham3_missing_symbol() }, 0);",
        )
        self.assert_rejected(source, reason="non-zero exit|no test suites")

    def test_zero_tests_are_rejected(self):
        self.assert_rejected("pub fn add(left: u32, right: u32) -> u32 { left + right }\n", reason="zero tests")

    def test_ignored_test_is_rejected(self):
        self.assert_rejected(Healthy.replace("#[test]\n    fn adds_zero", "#[test]\n    #[ignore]\n    fn adds_zero"), reason="ignored")

    def test_compile_only_run_is_rejected(self):
        self.assert_rejected(Healthy, "--no-run", reason="compile-only")

    def test_filtered_run_is_rejected(self):
        self.assert_rejected(Healthy, "--", "adds_zero", reason="filtered")

    def test_panic_in_test_is_rejected(self):
        self.assert_rejected(Healthy.replace("fn adds_zero() {", 'fn adds_zero() { panic!("canary"); '), reason="non-zero exit|failed")


if __name__ == "__main__":
    unittest.main()
