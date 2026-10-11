"""Validates the recorded VM boot result in $CHAM3_OUT_DIR/boot_test.

A missing result.json is a FAIL: the boot suite exists to check the boot
attempt. A result other than PASS is also a FAIL because a required boot check
that does not pass leaves M0-001 open. The result must bind to the ISO that was
built, and the serial log must reproduce the recorded classification.
"""

import importlib.util
import json
import os
import sys
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402

RequiredResultKeys = (
    "result",
    "reason",
    "iso_path",
    "iso_sha256",
    "qemu_binary",
    "qemu_version",
    "accel",
    "machine",
    "memory_mib",
    "cpus",
    "firmware",
    "elapsed_seconds",
    "matched_markers",
    "runtime_checks",
    "host_platform",
)
AllowedResults = ("PASS", "FAIL", "BLOCKED")


def boot_dir():
    return os.path.join(m0_support.out_dir(), "boot_test")


def load_result():
    path = os.path.join(boot_dir(), "result.json")
    m0_support.require_file(path, "run build/vm/run_boot_test.py --iso <iso>")
    with open(path, "r", encoding="utf-8") as handle:
        return json.load(handle)


def load_classifier():
    path = os.path.join(m0_support.repo_root(), "build", "vm", "run_boot_test.py")
    m0_support.require_file(path, "boot test entry point")
    spec = importlib.util.spec_from_file_location("cham3_run_boot_test_result", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.classify_boot_log


class boot_result_t(unittest.TestCase):
    def test_result_json_has_required_keys_and_types(self):
        result = load_result()
        for key in RequiredResultKeys:
            self.assertIn(key, result, "result.json lacks key: " + key)
        self.assertIn(result["result"], AllowedResults)
        self.assertIsInstance(result["matched_markers"], list)
        self.assertIsInstance(result["runtime_checks"], dict)
        self.assertIsInstance(result["memory_mib"], int)
        self.assertIsInstance(result["cpus"], int)
        self.assertIsInstance(result["elapsed_seconds"], (int, float))

    def test_result_binds_to_built_iso(self):
        result = load_result()
        iso = os.path.join(m0_support.out_dir(), "image", m0_support.ImageIsoName)
        m0_support.require_file(iso, "built ISO")
        self.assertEqual(os.path.basename(result["iso_path"]), m0_support.ImageIsoName)
        self.assertEqual(result["iso_sha256"], m0_support.sha256_file(iso))

    def test_serial_log_reproduces_recorded_classification(self):
        result = load_result()
        serial = os.path.join(boot_dir(), "serial.log")
        m0_support.require_file(serial, "serial console capture")
        with open(serial, "r", encoding="utf-8", errors="replace") as handle:
            classified = load_classifier()(handle.read())
        self.assertEqual(classified.get("result"), result["result"])

    def test_boot_result_is_pass(self):
        result = load_result()
        self.assertEqual(
            result["result"],
            "PASS",
            "boot check did not pass: %s" % result.get("reason", "no reason recorded"),
        )


if __name__ == "__main__":
    unittest.main()
