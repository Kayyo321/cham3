"""Checks for the image build outputs in $CHAM3_OUT_DIR/image."""

import datetime
import json
import os
import re
import sys
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402

ProvenanceFormat = "cham3_m0_provenance_v1"
ProvenanceKeys = (
    "provenance_format",
    "reproducibility",
    "source",
    "build_host",
    "tools",
    "inputs",
    "local_packages",
    "outputs",
    "build_started_utc",
    "build_finished_utc",
)
SourceKeys = ("git_commit", "git_tree_dirty")
BuildHostKeys = (
    "os_id",
    "os_version_id",
    "os_version_codename",
    "kernel_release",
    "machine",
    "dpkg_architecture",
    "in_container",
    "unverified_host_override",
)
ToolNames = (
    "live-build",
    "debootstrap",
    "dpkg-dev",
    "debhelper",
    "lintian",
    "xorriso",
    "squashfs-tools",
)
InputKeys = (
    "distribution",
    "architecture",
    "archive_areas",
    "mirrors",
    "lb_config_command",
    "source_date_epoch",
    "apt_release_files",
)
LocalPackageKeys = ("file", "package", "version", "sha256")
OutputKeys = ("file", "size_bytes", "sha256")
TimestampFormat = "%Y-%m-%dT%H:%M:%SZ"
TextArtifacts = (
    ("image", "build.log"),
    ("image", "provenance.json"),
    ("image", m0_support.ImageManifestName),
    ("image", "SHA256SUMS"),
    ("packages", "lintian.txt"),
    ("packages", "SHA256SUMS"),
)


def image_dir():
    directory = os.path.join(m0_support.out_dir(), "image")
    if not os.path.isdir(directory):
        raise AssertionError("required directory missing: %s (run build_image.sh)" % directory)
    return directory


def iso_path():
    path = os.path.join(image_dir(), m0_support.ImageIsoName)
    m0_support.require_file(path, "run build/scripts/build_image.sh as root")
    return path


def load_provenance():
    path = os.path.join(image_dir(), "provenance.json")
    m0_support.require_file(path, "written by build/scripts/write_provenance.py")
    with open(path, "r", encoding="utf-8") as handle:
        return json.load(handle)


def deb_on_disk():
    return os.path.join(m0_support.out_dir(), "packages", m0_support.DebFileName)


class image_artifacts_t(unittest.TestCase):
    def test_iso_is_iso9660(self):
        with open(iso_path(), "rb") as handle:
            handle.seek(0x8000)
            block = handle.read(6)
        self.assertEqual(len(block), 6, "ISO is shorter than the volume descriptor area")
        self.assertEqual(block[0:1], b"\x01", "primary volume descriptor type is not 1")
        self.assertEqual(block[1:6], b"CD001", "ISO 9660 identifier missing at 0x8001")

    def test_image_sha256sums_verify_and_cover_iso(self):
        directory = image_dir()
        self.assertEqual(m0_support.sha256sums_problems(directory), [])
        listed = {name for _, name in m0_support.sha256sums_entries(os.path.join(directory, "SHA256SUMS"))}
        self.assertIn(m0_support.ImageIsoName, listed)

    def test_provenance_top_level_schema(self):
        provenance = load_provenance()
        for key in ProvenanceKeys:
            self.assertIn(key, provenance, "provenance.json lacks key: " + key)
        self.assertEqual(provenance["provenance_format"], ProvenanceFormat)
        self.assertEqual(provenance["reproducibility"], "not_claimed")

    def test_provenance_source_is_exact_clean_commit(self):
        source = load_provenance()["source"]
        for key in SourceKeys:
            self.assertIn(key, source)
        self.assertRegex(source["git_commit"], r"^[0-9a-f]{40}$")
        head = m0_support.git_output(["rev-parse", "HEAD"]).strip()
        self.assertEqual(source["git_commit"], head, "provenance commit differs from HEAD")
        self.assertIs(source["git_tree_dirty"], False, "image was built from a dirty tree")

    def test_provenance_build_host_is_recorded(self):
        host = load_provenance()["build_host"]
        for key in BuildHostKeys:
            self.assertIn(key, host, "build_host lacks key: " + key)
        self.assertIsInstance(host["in_container"], bool)
        self.assertIsInstance(host["unverified_host_override"], bool)
        if not host["unverified_host_override"]:
            self.assertEqual(host["os_id"], "debian")
            self.assertEqual(host["os_version_id"], "13")
            self.assertEqual(host["dpkg_architecture"], "amd64")

    def test_provenance_tools_are_versioned(self):
        tools = load_provenance()["tools"]
        for name in ToolNames:
            self.assertIn(name, tools, "tools lacks entry: " + name)
            self.assertTrue(str(tools[name]).strip(), "empty version for " + name)

    def test_provenance_inputs_match_m0_definition(self):
        inputs = load_provenance()["inputs"]
        for key in InputKeys:
            self.assertIn(key, inputs, "inputs lacks key: " + key)
        self.assertEqual(inputs["distribution"], "trixie")
        self.assertEqual(inputs["architecture"], "amd64")
        areas = inputs["archive_areas"]
        areas = areas.split() if isinstance(areas, str) else areas
        self.assertEqual(areas, ["main"])
        hosts = set(re.findall(r"https?://([^/\"\s]+)", json.dumps(inputs["mirrors"])))
        self.assertTrue(hosts, "no mirror hosts recorded")
        self.assertLessEqual(hosts, set(m0_support.OfficialMirrorHosts))
        self.assertIn("lb config", inputs["lb_config_command"])

    def test_source_date_epoch_is_head_commit_time(self):
        epoch = load_provenance()["inputs"]["source_date_epoch"]
        expected = int(m0_support.git_output(["log", "-1", "--format=%ct"]).strip())
        self.assertEqual(int(epoch), expected)

    def test_apt_release_files_recorded_or_reason_given(self):
        inputs = load_provenance()["inputs"]
        releases = inputs["apt_release_files"]
        if releases is None:
            reasons = [k for k in inputs if "reason" in k and inputs[k]]
            self.assertTrue(reasons, "apt_release_files is null without a reason")
            return
        self.assertIsInstance(releases, list)
        for entry in releases:
            for key in ("file", "date", "sha256"):
                self.assertIn(key, entry)

    def test_local_packages_match_built_deb(self):
        packages = load_provenance()["local_packages"]
        self.assertTrue(packages, "no local packages recorded")
        matches = [p for p in packages if os.path.basename(p["file"]) == m0_support.DebFileName]
        self.assertEqual(len(matches), 1, "built cham3-base deb not recorded")
        entry = matches[0]
        for key in LocalPackageKeys:
            self.assertIn(key, entry)
        self.assertEqual(entry["package"], m0_support.ExpectedPackage)
        self.assertEqual(entry["version"], m0_support.ExpectedVersion)
        deb = deb_on_disk()
        m0_support.require_file(deb, "run build_packages.sh first")
        self.assertEqual(entry["sha256"], m0_support.sha256_file(deb))

    def test_outputs_record_iso_size_and_digest(self):
        outputs = load_provenance()["outputs"]
        iso = iso_path()
        matches = [o for o in outputs if os.path.basename(o["file"]) == m0_support.ImageIsoName]
        self.assertEqual(len(matches), 1, "ISO not recorded in outputs")
        for key in OutputKeys:
            self.assertIn(key, matches[0])
        self.assertEqual(matches[0]["size_bytes"], os.path.getsize(iso))
        self.assertEqual(matches[0]["sha256"], m0_support.sha256_file(iso))

    def test_build_timestamps_are_ordered(self):
        provenance = load_provenance()
        started = datetime.datetime.strptime(provenance["build_started_utc"], TimestampFormat)
        finished = datetime.datetime.strptime(provenance["build_finished_utc"], TimestampFormat)
        self.assertLessEqual(started, finished)

    def test_package_manifest_lists_cham3_base_at_expected_version(self):
        path = os.path.join(image_dir(), m0_support.ImageManifestName)
        m0_support.require_file(path, "live-build package manifest")
        with open(path, "r", encoding="utf-8") as handle:
            lines = [line.split() for line in handle if line.strip()]
        matches = [
            fields
            for fields in lines
            if fields and fields[0].split(":", 1)[0] == m0_support.ExpectedPackage
            and m0_support.ExpectedVersion in fields[1:]
        ]
        self.assertTrue(matches, "manifest lacks cham3-base at %s" % m0_support.ExpectedVersion)

    def test_text_artifacts_contain_no_secrets(self):
        paths = []
        for directory, name in TextArtifacts:
            path = os.path.join(m0_support.out_dir(), directory, name)
            m0_support.require_file(path, "text artifact required for secret scan")
            paths.append(path)
        scanner = os.path.join(m0_support.repo_root(), "build", "scripts", "scan_secrets.sh")
        completed = m0_support.run_tool([scanner] + paths, cwd=m0_support.repo_root())
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)


if __name__ == "__main__":
    unittest.main()
