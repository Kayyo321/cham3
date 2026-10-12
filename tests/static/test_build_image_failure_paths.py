"""Regression tests for Codex review finding R2: build_image.sh must stop when
entering the live-build work directory or running "lb config" fails, must never
run "lb build" after such a failure, and must not publish image artifacts,
provenance or checksums from a failed attempt.

These are fault-injection tests of the wrapper's control flow only. They run
the script WITHOUT root, with stub "id", "lb" and "debootstrap" executables
first in PATH: the stub "id" reports uid 0 so the wrapper passes its root check,
and the stub "lb" never mounts, debootstraps or builds anything. All output and
work directories are temporary. The synthetic ISO the stub writes is not an
image, and a PASS here is not image-build evidence. The tests skip when run as
root, so a wrapper defect cannot reach a privileged real build.
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402

ScriptPath = "build/scripts/build_image.sh"
PublishedNames = (
    m0_support.ImageIsoName,
    m0_support.ImageManifestName,
    "provenance.json",
    "SHA256SUMS",
)
FakeId = """#!/bin/sh
if [ "$1" = "-u" ]; then
    echo 0
    exit 0
fi
exec %s "$@"
"""
FakeDebootstrap = "#!/bin/sh\nexit 0\n"
# Records every invocation with its working directory. "config" and "build"
# exit with FAULT_LB_CONFIG_STATUS / FAULT_LB_BUILD_STATUS. "build" writes
# synthetic outputs only when it runs in the expected work directory.
FakeLb = """#!/bin/sh
printf '%s %s\\n' "$1" "$PWD" >> "$FAULT_LB_CALLS"
case "$1" in
    config)
        echo "fault injection: lb config exit ${FAULT_LB_CONFIG_STATUS}" >&2
        exit "${FAULT_LB_CONFIG_STATUS}"
        ;;
    build)
        if [ "$PWD" = "$FAULT_EXPECTED_WORK_DIR" ]; then
            printf 'synthetic iso, not an image\\n' > fault-injection.iso
            printf 'cham3-base\\t0.0.0~m0\\n' > fault-injection.packages
        fi
        exit "${FAULT_LB_BUILD_STATUS}"
        ;;
esac
exit 0
"""
# Moves the work directory away just before the wrapper enters it, as if it had
# been removed by another process. The wrapper calls dpkg-query once, for the
# build-log header, immediately before running live-build.
FakeDpkgQueryMovingWorkDir = """#!/bin/sh
mv -- "$FAULT_EXPECTED_WORK_DIR" "$FAULT_EXPECTED_WORK_DIR.moved"
echo 0-fault-injection
"""
# build_image.sh reads each staged .deb's Package field for per-profile staging.
# The fixtures are not real .deb archives, so stub dpkg-deb to report the package
# name from the file name (the part before the first underscore), e.g.
# cham3-base_0.0.0~m0_all.deb -> cham3-base. This keeps the fault-injection tests
# hermetic (no real dpkg-deb needed) and lets the wrapper reach the lb steps.
FakeDpkgDeb = """#!/bin/sh
if [ "$1" = "-f" ]; then
    base=$(basename -- "$2")
    echo "${base%%_*}"
    exit 0
fi
exit 0
"""


class build_image_failure_paths_t(unittest.TestCase):
    def setUp(self):
        if os.geteuid() == 0:
            self.skipTest(
                "running as root: fault-injection tests of build_image.sh run only "
                "as non-root so a wrapper defect cannot start a privileged real build"
            )
        self.script = os.path.join(m0_support.repo_root(), ScriptPath)
        m0_support.require_file(self.script, "build_image entry point")
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.tools_dir = os.path.join(self.scratch.name, "tools")
        os.makedirs(self.tools_dir)
        real_id = shutil.which("id")
        if real_id is None:
            raise AssertionError("required executable not found: id")
        self.write_tool("id", FakeId % real_id)
        self.write_tool("debootstrap", FakeDebootstrap)
        self.write_tool("lb", FakeLb)
        self.write_tool("dpkg-deb", FakeDpkgDeb)
        # Resolved, because the wrapper resolves its directories with realpath.
        self.out_dir = os.path.join(os.path.realpath(self.scratch.name), "out")
        self.image_dir = os.path.join(self.out_dir, "image")
        self.work_dir = os.path.join(self.out_dir, "live-build-work")
        packages_dir = os.path.join(self.out_dir, "packages")
        os.makedirs(packages_dir)
        with open(os.path.join(packages_dir, m0_support.DebFileName), "wb") as handle:
            handle.write(b"synthetic deb, not a package\n")
        self.calls_path = os.path.join(self.scratch.name, "lb_calls.txt")

    def write_tool(self, name, content):
        path = os.path.join(self.tools_dir, name)
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(content)
        os.chmod(path, 0o755)

    def run_build_image(self, config_status, build_status):
        env = {name: value for name, value in os.environ.items() if not name.startswith("CHAM3_")}
        env.update(
            {
                "PATH": self.tools_dir + os.pathsep + os.environ.get("PATH", ""),
                "CHAM3_OUT_DIR": self.out_dir,
                "CHAM3_ALLOW_UNVERIFIED_HOST": "1",
                "SOURCE_DATE_EPOCH": "1700000000",
                "FAULT_LB_CALLS": self.calls_path,
                "FAULT_LB_CONFIG_STATUS": str(config_status),
                "FAULT_LB_BUILD_STATUS": str(build_status),
                "FAULT_EXPECTED_WORK_DIR": self.work_dir,
            }
        )
        try:
            return subprocess.run(
                [self.script],
                cwd=self.scratch.name,
                env=env,
                capture_output=True,
                text=True,
                check=False,
                timeout=300,
            )
        except subprocess.TimeoutExpired:
            raise AssertionError("build_image.sh did not finish under fault injection")

    def lb_calls(self):
        if not os.path.exists(self.calls_path):
            return []
        with open(self.calls_path, "r", encoding="utf-8") as handle:
            return [line.split(" ", 1)[0] for line in handle.read().splitlines()]

    def published(self):
        return sorted(
            name for name in PublishedNames if os.path.exists(os.path.join(self.image_dir, name))
        )

    def plant_stale_artifacts(self):
        os.makedirs(self.image_dir)
        for name in PublishedNames:
            with open(os.path.join(self.image_dir, name), "w", encoding="utf-8") as handle:
                handle.write("stale output of an earlier run\n")

    def assert_failed_without_publishing(self, completed, expected_calls):
        output = completed.stdout + completed.stderr
        self.assertNotEqual(completed.returncode, 0, output)
        self.assertNotIn("build_image: PASS", output)
        self.assertEqual(self.lb_calls(), expected_calls, output)
        self.assertEqual(self.published(), [], "failed attempt left image artifacts: " + output)
        self.assertTrue(
            os.path.isfile(os.path.join(self.image_dir, "build.log")),
            "diagnostic build.log missing after failure",
        )

    def test_config_failure_stops_before_build_and_publishes_nothing(self):
        self.plant_stale_artifacts()
        completed = self.run_build_image(config_status=42, build_status=0)
        self.assert_failed_without_publishing(completed, ["config"])
        self.assertIn("lb config failed (exit 42)", completed.stderr)
        with open(os.path.join(self.image_dir, "build.log"), "r", encoding="utf-8") as handle:
            self.assertIn("fault injection: lb config exit 42", handle.read())

    def test_work_dir_entry_failure_runs_no_live_build(self):
        self.write_tool("dpkg-query", FakeDpkgQueryMovingWorkDir)
        completed = self.run_build_image(config_status=0, build_status=0)
        self.assert_failed_without_publishing(completed, [])
        self.assertIn("cannot enter work dir", completed.stdout + completed.stderr)

    def test_build_failure_after_config_publishes_nothing(self):
        completed = self.run_build_image(config_status=0, build_status=7)
        self.assert_failed_without_publishing(completed, ["config", "build"])
        self.assertIn("lb build failed (exit 7)", completed.stderr)

    def test_successful_stub_build_publishes_artifacts(self):
        # Positive control for the harness and the success path. Synthetic outputs only.
        completed = self.run_build_image(config_status=0, build_status=0)
        output = completed.stdout + completed.stderr
        self.assertEqual(completed.returncode, 0, output)
        self.assertIn("build_image: PASS", output)
        with open(self.calls_path, "r", encoding="utf-8") as handle:
            calls = [line.split(" ", 1) for line in handle.read().splitlines()]
        self.assertEqual(calls, [["config", self.work_dir], ["build", self.work_dir]])
        self.assertEqual(self.published(), sorted(PublishedNames))
        self.assertEqual(m0_support.sha256sums_problems(self.image_dir), [])
        with open(os.path.join(self.image_dir, "provenance.json"), "r", encoding="utf-8") as handle:
            provenance = json.load(handle)
        self.assertEqual(provenance["reproducibility"], "not_claimed")


if __name__ == "__main__":
    unittest.main()
