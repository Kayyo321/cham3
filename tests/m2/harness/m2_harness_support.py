"""Shared helpers for the M2 harness-sensitivity tests."""

import os
import shutil
import subprocess
import sys
import tempfile

RepoRoot = os.environ.get(
    "CHAM3_REPO_ROOT",
    os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))),
)
sys.path.insert(0, os.path.join(RepoRoot, "tests", "m2", "lib"))
import m2_harness  # noqa: E402,F401  (re-exported for the test modules)


def run(command, cwd=None, env=None):
    completed = subprocess.run(
        command, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, check=False
    )
    return completed.returncode, completed.stdout


class canary_project_t:
    """A throwaway Cargo package with no dependencies, built offline in a temp dir."""

    def __init__(self, source):
        self.directory = tempfile.mkdtemp(prefix="cham3_m2_canary_")
        os.makedirs(os.path.join(self.directory, "src"))
        with open(os.path.join(self.directory, "Cargo.toml"), "w", encoding="utf-8") as handle:
            handle.write(
                '[package]\nname = "cham3_canary"\nversion = "0.1.0"\nedition = "2024"\n'
                "publish = false\n\n[lib]\ndoctest = false\n"
            )
        with open(os.path.join(self.directory, "src", "lib.rs"), "w", encoding="utf-8") as handle:
            handle.write(source)

    def cargo_test(self, *extra):
        environment = dict(os.environ, CARGO_TARGET_DIR=os.path.join(self.directory, "target"))
        environment.pop("RUSTFLAGS", None)
        return run(["cargo", "test", "--offline"] + list(extra), cwd=self.directory, env=environment)

    def remove(self):
        shutil.rmtree(self.directory, ignore_errors=True)
