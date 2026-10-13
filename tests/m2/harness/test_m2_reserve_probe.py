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

The wrapper check_reserve_full_disk.sh owns nothing on the disk either (R3-01): it
removes nothing beforehand, refuses an existing root or filler path (file, directory
or symbolic link) untouched, and leaves an ineligible filesystem as it found it. The
wrapper cases run against a temporary directory, or against a fresh subdirectory of
CHAM3_M2_WRAPPER_DISK when set (the CI step points it at the 2 MiB tmpfs). A seeded
wrapper defect (the baseline's pre-deletion restored) must be caught.
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
Wrapper = os.path.join(RepoRoot, "tests", "m2", "storage", "check_reserve_full_disk.sh")
WrapperDisk = os.environ.get("CHAM3_M2_WRAPPER_DISK", "")
RootName = "cham3_reserve_probe_root"
FillerName = "cham3_reserve_probe_filler"

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


def wrapper_environment():
    # The dispatcher's probe build (release/ and debug/ under one target directory).
    target = os.path.dirname(os.path.dirname(Probe))
    return dict(os.environ, CHAM3_REPO_ROOT=RepoRoot, CHAM3_M2_RESERVE_TARGET_DIR=target)


class wrapper_disk_t:
    """A disk directory for one wrapper case: a temporary directory, or a fresh
    subdirectory of CHAM3_M2_WRAPPER_DISK (removed afterwards; only this test writes
    there)."""

    def __enter__(self):
        if WrapperDisk:
            self.path = tempfile.mkdtemp(prefix="wrapper_case_", dir=WrapperDisk)
        else:
            self.path = tempfile.mkdtemp(prefix="cham3_wrapper_disk_")
        return self.path

    def __exit__(self, *_):
        shutil.rmtree(self.path, ignore_errors=True)


def run_wrapper(disk, script=Wrapper):
    with tempfile.TemporaryDirectory(prefix="cham3_wrapper_out_") as out:
        return run(["sh", script, out, disk], cwd=RepoRoot, env=wrapper_environment())


def listing(directory):
    return sorted(os.listdir(directory))


class full_disk_wrapper_t(unittest.TestCase):
    """check_reserve_full_disk.sh on paths it does not own (R3-01)."""

    def setUp(self):
        self.assertTrue(os.path.isfile(Probe), "probe not built: %r" % Probe)

    def test_existing_root_and_filler_are_refused_and_preserved(self):
        with wrapper_disk_t() as disk:
            root = os.path.join(disk, RootName)
            os.mkdir(root, 0o700)
            with open(os.path.join(root, "sentinel"), "w", encoding="utf-8") as handle:
                handle.write("synthetic existing root must survive refusal\n")
            with open(os.path.join(disk, FillerName), "w", encoding="utf-8") as handle:
                handle.write("synthetic existing filler must survive refusal\n")
            status, output = run_wrapper(disk)
            self.assertNotEqual(status, 0, output[-2000:])
            self.assertIn("refusing:", output)
            self.assertNotIn("filler_bytes", output)
            self.assertEqual(listing(disk), [FillerName, RootName])
            self.assertEqual(listing(root), ["sentinel"])
            with open(os.path.join(root, "sentinel"), encoding="utf-8") as handle:
                self.assertEqual(handle.read(), "synthetic existing root must survive refusal\n")
            with open(os.path.join(disk, FillerName), encoding="utf-8") as handle:
                self.assertEqual(handle.read(), "synthetic existing filler must survive refusal\n")

    def test_symbolic_links_and_their_targets_are_refused_and_preserved(self):
        for dangling in (False, True):
            with self.subTest(dangling=dangling), wrapper_disk_t() as disk, \
                    tempfile.TemporaryDirectory(prefix="cham3_wrapper_targets_") as targets:
                target_root = os.path.join(targets, "elsewhere")
                os.mkdir(target_root, 0o700)
                with open(os.path.join(target_root, "sentinel"), "w", encoding="utf-8") as handle:
                    handle.write("synthetic link target must survive refusal\n")
                target_filler = os.path.join(targets, "elsewhere_filler")
                with open(target_filler, "w", encoding="utf-8") as handle:
                    handle.write("synthetic filler target must survive refusal\n")
                root_target = os.path.join(targets, "missing") if dangling else target_root
                os.symlink(root_target, os.path.join(disk, RootName))
                os.symlink(target_filler, os.path.join(disk, FillerName))
                status, output = run_wrapper(disk)
                self.assertNotEqual(status, 0, output[-2000:])
                self.assertIn("refusing:", output)
                self.assertEqual(listing(disk), [FillerName, RootName])
                self.assertTrue(os.path.islink(os.path.join(disk, RootName)))
                self.assertTrue(os.path.islink(os.path.join(disk, FillerName)))
                self.assertEqual(os.readlink(os.path.join(disk, RootName)), root_target)
                self.assertEqual(listing(target_root), ["sentinel"])
                self.assertTrue(os.path.isfile(target_filler))

    def test_a_disk_without_the_paths_is_left_as_found(self):
        with wrapper_disk_t() as disk:
            status, output = run_wrapper(disk)
            # An ordinary filesystem is refused by the probe; a small tmpfs (the CI
            # step) runs the positive path. Either way nothing is left on the disk.
            if status == 0:
                self.assertIn("reserve_verified", output)
            else:
                self.assertIn("refusing_to_fill", output)
                self.assertNotIn("filler_bytes", output)
            self.assertEqual(listing(disk), [])


class seeded_wrapper_defects_t(unittest.TestCase):
    """The baseline wrapper's pre-deletion, restored in a copy of the script, must be
    caught by the preservation case; the unmodified copy must pass it."""

    PreCheck = "for path in \"$root_path\" \"$filler_path\"; do\n"
    ProbeCall = "    report=$out/reserve_full_disk_$profile.txt\n"

    def seeded_copy(self, defect):
        with open(Wrapper, encoding="utf-8") as handle:
            source = handle.read()
        if defect:
            self.assertEqual(source.count(self.ProbeCall), 1)
            source = source.replace(self.ProbeCall, self.ProbeCall
                                    + "    rm -rf \"$root_path\" \"$filler_path\"\n")
            start = source.index(self.PreCheck)
            end = source.index("done\n", start) + len("done\n")
            source = source[:start] + source[end:]
        directory = tempfile.mkdtemp(prefix="cham3_wrapper_copy_")
        path = os.path.join(directory, "check_reserve_full_disk.sh")
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(source)
        return directory, path

    def existing_paths_survive(self, script):
        with wrapper_disk_t() as disk:
            root = os.path.join(disk, RootName)
            os.mkdir(root, 0o700)
            with open(os.path.join(root, "sentinel"), "w", encoding="utf-8") as handle:
                handle.write("synthetic\n")
            with open(os.path.join(disk, FillerName), "w", encoding="utf-8") as handle:
                handle.write("synthetic\n")
            status, output = run_wrapper(disk, script)
            survived = os.path.isfile(os.path.join(root, "sentinel")) and os.path.isfile(
                os.path.join(disk, FillerName))
            return status, survived, output

    def test_unmodified_copy_preserves_existing_paths(self):
        directory, script = self.seeded_copy(defect=False)
        try:
            status, survived, output = self.existing_paths_survive(script)
        finally:
            shutil.rmtree(directory, ignore_errors=True)
        self.assertNotEqual(status, 0, output[-2000:])
        self.assertTrue(survived, output[-2000:])

    def test_restored_pre_deletion_is_detected(self):
        directory, script = self.seeded_copy(defect=True)
        try:
            status, survived, output = self.existing_paths_survive(script)
        finally:
            shutil.rmtree(directory, ignore_errors=True)
        # On an eligible tmpfs the pre-deletion is followed by a successful positive run,
        # on an ordinary filesystem by a refusal: the detection is the lost data alone.
        del status
        self.assertFalse(survived, "the seeded pre-deletion was not detected: " + output[-2000:])


if __name__ == "__main__":
    unittest.main()
