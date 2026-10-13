"""M2-005 review R03: the restrictive-reserve probe on an ordinary filesystem.

The release-built cham3_reserve_probe must report a preallocated reserve (written
zeros, real blocks) for a new journal, and must refuse its full-disk mode on anything
but a small tmpfs, so it never fills a real filesystem. The full-disk path itself runs
separately on a disposable 2 MiB tmpfs (tests/m2/storage/check_reserve_full_disk.sh).
"""

import os
import subprocess
import tempfile
import unittest

Probe = os.environ.get("CHAM3_M2_RESERVE_PROBE", "")
ReserveBytes = 1024 * 1024


class reserve_probe_t(unittest.TestCase):
    def run_probe(self, mode):
        self.assertTrue(os.path.isfile(Probe), "probe not built: %r" % Probe)
        with tempfile.TemporaryDirectory(prefix="cham3_reserve_probe_") as directory:
            root = os.path.join(directory, "registry")
            completed = subprocess.run([Probe, root, mode], capture_output=True, text=True,
                                       check=False, timeout=600)
            self.assertFalse(os.path.exists(root), "the probe removes its root")
        return completed

    def test_allocate_only_preallocates_the_reserve(self):
        completed = self.run_probe("allocate-only")
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)
        lines = completed.stdout.split("\n")
        self.assertIn("reserve_bytes %d" % ReserveBytes, lines)
        allocated = [int(line.split()[1]) for line in lines if line.startswith("reserve_allocated_bytes ")]
        self.assertEqual(len(allocated), 1)
        self.assertGreaterEqual(allocated[0], ReserveBytes)
        self.assertIn("reserve_preallocated", lines)
        self.assertIn("reserve_verified", lines)

    def test_full_disk_mode_refuses_an_ordinary_filesystem(self):
        completed = self.run_probe("full-disk")
        self.assertEqual(completed.returncode, 3, completed.stdout + completed.stderr)
        self.assertIn("reserve_preallocated", completed.stdout)
        self.assertIn("refusing_to_fill", completed.stdout)
        self.assertNotIn("filler_bytes", completed.stdout)


if __name__ == "__main__":
    unittest.main()
