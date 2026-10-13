"""M2-005 review R03 and R2-03: the restrictive-reserve probe on an ordinary filesystem.

The release-built cham3_reserve_probe must report a preallocated reserve (written
zeros, real blocks) for a new journal, and must refuse its full-disk mode on anything
but a small tmpfs, so it never fills a real filesystem. The full-disk path itself runs
separately on a disposable 2 MiB tmpfs (tests/m2/storage/check_reserve_full_disk.sh).

The probe owns only what it creates (R2-03): an existing root is refused and left
untouched in every mode, an unknown mode or an ineligible filesystem creates nothing,
and only a root this invocation created is removed. Two seeded defects in a disposable
workspace copy (adopting an existing root; removing the root whether or not it was
created) must be caught by the preservation check.
"""

import os
import shutil
import stat
import subprocess
import tempfile
import unittest

from m2_harness_support import RepoRoot, run

Probe = os.environ.get("CHAM3_M2_RESERVE_PROBE", "")
ReserveBytes = 1024 * 1024
ProbeSource = "core/registryd/src/bin/cham3_reserve_probe.rs"

# (name, original, replacement): each must compile and be caught by the preservation check.
SeededProbeDefects = [
    ("probe_adopts_an_existing_root",
     "        Err(error) if error.kind() == ErrorKind::AlreadyExists => Err(existing_root(root)),\n",
     "        Err(error) if error.kind() == ErrorKind::AlreadyExists => Ok(()),\n"),
    ("probe_removes_a_root_it_did_not_create",
     "    create_root(root)?;\n    let outcome = exercise(root, mount.as_deref());\n",
     "    let created = create_root(root);\n    let outcome = created.and_then(|()| exercise(root, mount.as_deref()));\n"),
]


def run_probe(probe, root, mode):
    return subprocess.run([probe, root, mode], capture_output=True, text=True, check=False, timeout=600)


def existing_root(directory):
    """A pre-existing root holding one synthetic file, as a caller might pass by mistake."""
    root = os.path.join(directory, "existing")
    os.mkdir(root, 0o700)
    with open(os.path.join(root, "sentinel"), "w", encoding="utf-8") as handle:
        handle.write("synthetic data that the probe must not touch\n")
    return root


def preserved(root):
    if sorted(os.listdir(root)) != ["sentinel"]:
        return False
    with open(os.path.join(root, "sentinel"), encoding="utf-8") as handle:
        return handle.read() == "synthetic data that the probe must not touch\n"


class reserve_probe_t(unittest.TestCase):
    def setUp(self):
        self.assertTrue(os.path.isfile(Probe), "probe not built: %r" % Probe)

    def run_fresh(self, mode):
        with tempfile.TemporaryDirectory(prefix="cham3_reserve_probe_") as directory:
            root = os.path.join(directory, "registry")
            completed = run_probe(Probe, root, mode)
            self.assertFalse(os.path.exists(root), "the probe removes the root it created")
        return completed

    def test_allocate_only_preallocates_the_reserve(self):
        completed = self.run_fresh("allocate-only")
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)
        lines = completed.stdout.split("\n")
        self.assertIn("reserve_bytes %d" % ReserveBytes, lines)
        allocated = [int(line.split()[1]) for line in lines if line.startswith("reserve_allocated_bytes ")]
        self.assertEqual(len(allocated), 1)
        self.assertGreaterEqual(allocated[0], ReserveBytes)
        self.assertIn("reserve_preallocated", lines)
        self.assertIn("reserve_verified", lines)

    def test_full_disk_mode_refuses_an_ordinary_filesystem_before_creating_anything(self):
        completed = self.run_fresh("full-disk")
        self.assertEqual(completed.returncode, 3, completed.stdout + completed.stderr)
        self.assertIn("refusing_to_fill", completed.stdout)
        self.assertNotIn("filler_bytes", completed.stdout)
        self.assertNotIn("reserve_preallocated", completed.stdout, "nothing is created before the refusal")

    def test_an_unknown_mode_creates_nothing(self):
        completed = self.run_fresh("no-such-mode")
        self.assertEqual(completed.returncode, 3, completed.stdout + completed.stderr)
        self.assertIn("refusing_to_fill unknown mode", completed.stdout)
        self.assertNotIn("reserve_preallocated", completed.stdout)

    def test_a_missing_parent_is_refused(self):
        with tempfile.TemporaryDirectory(prefix="cham3_reserve_probe_") as directory:
            root = os.path.join(directory, "no", "such", "parent")
            completed = run_probe(Probe, root, "allocate-only")
            self.assertEqual(completed.returncode, 3, completed.stdout + completed.stderr)
            self.assertIn("refusing_to_fill cannot create", completed.stdout)
            self.assertEqual(os.listdir(directory), [])

    def test_an_existing_root_is_refused_and_preserved_in_every_mode(self):
        for mode in ("allocate-only", "full-disk", "no-such-mode"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory(prefix="cham3_reserve_probe_") as directory:
                root = existing_root(directory)
                completed = run_probe(Probe, root, mode)
                self.assertEqual(completed.returncode, 3, completed.stdout + completed.stderr)
                self.assertIn("refusing_to_fill", completed.stdout)
                self.assertNotIn("reserve_preallocated", completed.stdout)
                self.assertTrue(os.path.isdir(root), "the existing root is not removed")
                self.assertTrue(preserved(root), "the existing root is not touched")
                self.assertEqual(stat.S_IMODE(os.stat(root).st_mode), 0o700)


class seeded_probe_defects_t(unittest.TestCase):
    """Builds the probe from a disposable workspace copy with one defect each; the
    preservation check must fail for every defect and pass for the unmodified copy."""

    @classmethod
    def setUpClass(cls):
        cls.workspace = tempfile.mkdtemp(prefix="cham3_m2_reserve_probe_defect_")
        for name in ("Cargo.toml", "Cargo.lock"):
            shutil.copy2(os.path.join(RepoRoot, name), os.path.join(cls.workspace, name))
        shutil.copytree(os.path.join(RepoRoot, ".cargo"), os.path.join(cls.workspace, ".cargo"))
        shutil.copytree(os.path.join(RepoRoot, "core"), os.path.join(cls.workspace, "core"))
        cls.environment = dict(os.environ, CARGO_TARGET_DIR=os.path.join(cls.workspace, "target"))
        cls.environment.pop("RUSTFLAGS", None)

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.workspace, ignore_errors=True)

    def build_probe(self):
        status, output = run(["cargo", "build", "--frozen", "-p", "cham3_registryd", "--bin", "cham3_reserve_probe"],
                             cwd=self.workspace, env=self.environment)
        self.assertEqual(status, 0, output[-3000:])
        return os.path.join(self.workspace, "target", "debug", "cham3_reserve_probe")

    def existing_root_survives(self, probe):
        with tempfile.TemporaryDirectory(prefix="cham3_reserve_probe_") as directory:
            root = existing_root(directory)
            completed = run_probe(probe, root, "allocate-only")
            return os.path.isdir(root) and preserved(root), completed

    def test_unmodified_copy_preserves_an_existing_root(self):
        survived, completed = self.existing_root_survives(self.build_probe())
        self.assertEqual(completed.returncode, 3, completed.stdout + completed.stderr)
        self.assertTrue(survived)

    def test_each_seeded_defect_is_detected(self):
        path = os.path.join(self.workspace, ProbeSource)
        with open(path, encoding="utf-8") as handle:
            pristine = handle.read()
        for name, original, replacement in SeededProbeDefects:
            with self.subTest(defect=name):
                self.assertEqual(pristine.count(original), 1, "defect anchor for %s" % name)
                try:
                    with open(path, "w", encoding="utf-8") as handle:
                        handle.write(pristine.replace(original, replacement))
                    survived, completed = self.existing_root_survives(self.build_probe())
                finally:
                    with open(path, "w", encoding="utf-8") as handle:
                        handle.write(pristine)
                self.assertFalse(survived, "%s was not detected: %s" % (name, completed.stdout + completed.stderr))


if __name__ == "__main__":
    unittest.main()
