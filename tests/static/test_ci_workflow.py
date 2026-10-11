"""Static checks for the M0 CI workflow (.github/workflows/m0_checks.yml).

PyYAML is not used (stdlib only), so these are line-oriented checks over the
workflow text with YAML comments removed.
"""

import os
import re
import sys
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402

WorkflowPath = ".github/workflows/m0_checks.yml"
RequiredTriggers = ("push", "pull_request", "workflow_dispatch")
RequiredCommands = (
    "build/scripts/run_static_checks.sh",
    "tests/run_tests.sh static",
    "build/scripts/build_packages.sh",
    "tests/run_tests.sh packages",
    "build/scripts/build_image.sh",
    "tests/run_tests.sh image",
    "build/vm/run_boot_test.py",
    "--accel auto",
    "--privileged",
)
MaxRetentionDays = 14


def workflow_raw():
    return m0_support.read_text(WorkflowPath)


def workflow_lines_without_comments(text):
    lines = []
    for raw in text.splitlines():
        if raw.lstrip().startswith("#"):
            continue
        lines.append(raw.split(" #", 1)[0].rstrip())
    return lines


def top_level_block(lines, key):
    start = None
    for index, line in enumerate(lines):
        if line.startswith(key + ":"):
            start = index
            break
    if start is None:
        return None
    block = [lines[start]]
    for line in lines[start + 1 :]:
        if line and not line[0].isspace():
            break
        block.append(line)
    return block


class ci_workflow_t(unittest.TestCase):
    def code_text(self):
        return "\n".join(workflow_lines_without_comments(workflow_raw()))

    def test_workflow_triggers(self):
        lines = workflow_lines_without_comments(workflow_raw())
        block = top_level_block(lines, "on")
        self.assertIsNotNone(block, "workflow has no top-level 'on:' block")
        block_text = "\n".join(block)
        for trigger in RequiredTriggers:
            self.assertRegex(
                block_text,
                r"(?<![\w-])%s(?![\w-])" % re.escape(trigger),
                "missing trigger %s" % trigger,
            )

    def test_top_level_permissions_are_contents_read_only(self):
        lines = workflow_lines_without_comments(workflow_raw())
        block = top_level_block(lines, "permissions")
        self.assertIsNotNone(block, "workflow has no top-level permissions block")
        inline = block[0][len("permissions:") :].strip()
        entries = ([inline] if inline else []) + [
            line.strip() for line in block[1:] if line.strip()
        ]
        self.assertEqual(entries, ["contents: read"])

    def test_no_write_permission_grants(self):
        code = self.code_text()
        self.assertNotRegex(code, r"(?m)^\s*[a-z-]+:\s*write\s*$")
        self.assertNotIn("write-all", code)

    def test_no_secrets_context_or_pull_request_target(self):
        code = self.code_text()
        self.assertNotIn("secrets.", code)
        self.assertNotIn("secrets[", code)
        self.assertNotIn("pull_request_target", code)

    def test_third_party_actions_pinned_to_full_sha(self):
        problems = []
        for raw in workflow_raw().splitlines():
            if not re.search(r"\buses:", raw):
                continue
            match = re.match(r"^\s*-?\s*uses:\s*(\S+)(\s+#\s*\S.*)?\s*$", raw)
            if not match:
                problems.append("unparseable uses line: " + raw.strip())
                continue
            value = match.group(1)
            if value.startswith("./"):
                continue
            if not re.match(r"^[^@\s]+@[0-9a-f]{40}$", value):
                problems.append("action not pinned to a 40-hex SHA: " + value)
            if not match.group(2):
                problems.append("pinned action lacks a version comment: " + value)
        self.assertEqual(problems, [])

    def test_jobs_run_in_debian_trixie_containers(self):
        code = self.code_text()
        images = re.findall(r"(?m)^\s*(?:-\s*)?(?:image|container):\s*(\S+)\s*$", code)
        self.assertTrue(images, "no debian:trixie container image is declared")
        for image in images:
            self.assertEqual(image, "debian:trixie")

    def test_runs_static_packages_image_and_boot_steps(self):
        code = self.code_text()
        for command in RequiredCommands:
            self.assertIn(command, code, "CI does not run: " + command)

    def test_artifact_retention_is_short(self):
        code = self.code_text()
        if "upload-artifact" not in code:
            self.fail("CI does not upload any artifacts")
        values = re.findall(r"retention-days:\s*(\d+)", code)
        self.assertTrue(values, "upload-artifact steps must set retention-days")
        for value in values:
            self.assertLessEqual(int(value), MaxRetentionDays)

    def test_git_installed_and_safe_directory_configured(self):
        code = self.code_text()
        self.assertRegex(code, r"apt-get\s+install[\s\S]{0,400}?\bgit\b")
        self.assertIn("safe.directory", code)


if __name__ == "__main__":
    unittest.main()
