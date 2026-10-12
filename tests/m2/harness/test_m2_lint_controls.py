"""Safe-core lint controls: controlled unsafe insertion must fail compilation.

Works on a disposable copy of the workspace (Cargo.toml, Cargo.lock, .cargo/ and
core/), never on the checkout. Each domain crate is checked with both enforcement
layers together and with each layer alone (crate attribute #![forbid(unsafe_code)]
and workspace lint unsafe_code = "forbid"). The native secure_memory crate must
reject an unsafe operation in an unsafe fn without an unsafe block, and clippy must
reject an undocumented unsafe block there. A positive control proves the unmodified
copy builds.
"""

import os
import re
import shutil
import tempfile
import unittest

from m2_harness_support import RepoRoot, run

DomainCrates = {
    "cham3_contract_format": "core/contract_format",
    "cham3_identityd": "core/identityd",
    "cham3_vaultd": "core/vaultd",
    "cham3_contractd": "core/contractd",
    "cham3_policyd": "core/policyd",
    "cham3_registryd": "core/registryd",
    "cham3_os_adapter": "core/native/os_adapter",
    "cham3_provider_probe": "core/probes/provider_probe",
}
UnsafeInsertion = "\npub fn cham3_lint_probe(pointer: *const u8) -> u8 {\n    unsafe { *pointer }\n}\n"
ForbidAttribute = "#![forbid(unsafe_code)]"


class lint_controls_t(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.workspace = tempfile.mkdtemp(prefix="cham3_m2_lint_")
        for name in ("Cargo.toml", "Cargo.lock"):
            shutil.copy2(os.path.join(RepoRoot, name), os.path.join(cls.workspace, name))
        shutil.copytree(os.path.join(RepoRoot, ".cargo"), os.path.join(cls.workspace, ".cargo"))
        shutil.copytree(os.path.join(RepoRoot, "core"), os.path.join(cls.workspace, "core"))
        # The provider probe embeds the committed vector file by relative path.
        os.makedirs(os.path.join(cls.workspace, "tests", "m2", "vectors"))
        shutil.copy2(
            os.path.join(RepoRoot, "tests", "m2", "vectors", "provider_kat.json"),
            os.path.join(cls.workspace, "tests", "m2", "vectors", "provider_kat.json"),
        )
        cls.environment = dict(os.environ, CARGO_TARGET_DIR=os.path.join(cls.workspace, "target"))
        cls.environment.pop("RUSTFLAGS", None)

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.workspace, ignore_errors=True)

    def cargo(self, *arguments):
        return run(["cargo"] + list(arguments) + ["--frozen"], cwd=self.workspace, env=self.environment)

    def edit(self, relative, transform):
        path = os.path.join(self.workspace, relative)
        with open(path, "r", encoding="utf-8") as handle:
            original = handle.read()
        changed = transform(original)
        self.assertNotEqual(changed, original, "edit had no effect on " + relative)
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(changed)
        self.addCleanup(self.restore, path, original)

    @staticmethod
    def restore(path, original):
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(original)

    def lib_path(self, crate_dir):
        return os.path.join(crate_dir, "src", "lib.rs")

    def test_positive_control_unmodified_workspace_builds(self):
        status, output = self.cargo("check", "--workspace", "--all-targets")
        self.assertEqual(status, 0, output)

    def test_unsafe_insertion_fails_in_every_domain_crate(self):
        for package, crate_dir in DomainCrates.items():
            with self.subTest(package=package):
                self.edit(self.lib_path(crate_dir), lambda text: text + UnsafeInsertion)
                status, output = self.cargo("check", "-p", package)
                self.assertNotEqual(status, 0, output)
                self.assertRegex(output, r"usage of an `unsafe` block")
                self.doCleanups()

    def test_crate_attribute_alone_rejects_unsafe(self):
        for package, crate_dir in DomainCrates.items():
            with self.subTest(package=package):
                self.edit(os.path.join(crate_dir, "Cargo.toml"), lambda text: text.replace("[lints]\nworkspace = true\n", ""))
                self.edit(self.lib_path(crate_dir), lambda text: text + UnsafeInsertion)
                status, output = self.cargo("check", "-p", package)
                self.assertNotEqual(status, 0, output)
                self.assertRegex(output, r"usage of an `unsafe` block")
                self.doCleanups()

    def test_workspace_lint_alone_rejects_unsafe(self):
        for package, crate_dir in DomainCrates.items():
            with self.subTest(package=package):
                self.edit(self.lib_path(crate_dir), lambda text: text.replace(ForbidAttribute, "") + UnsafeInsertion)
                status, output = self.cargo("check", "-p", package)
                self.assertNotEqual(status, 0, output)
                self.assertRegex(output, r"usage of an `unsafe` block")
                self.doCleanups()

    def test_every_domain_crate_root_forbids_unsafe(self):
        for package, crate_dir in DomainCrates.items():
            with self.subTest(package=package):
                roots = [self.lib_path(crate_dir)]
                main_path = os.path.join(crate_dir, "src", "main.rs")
                if os.path.exists(os.path.join(RepoRoot, main_path)):
                    roots.append(main_path)
                for root in roots:
                    with open(os.path.join(RepoRoot, root), "r", encoding="utf-8") as handle:
                        self.assertIn(ForbidAttribute, handle.read(), root)
                with open(os.path.join(RepoRoot, crate_dir, "Cargo.toml"), "r", encoding="utf-8") as handle:
                    self.assertRegex(handle.read(), r"\[lints\]\nworkspace = true\n")

    def test_native_crate_rejects_unsafe_op_without_block(self):
        insertion = "\n/// Probe.\n/// # Safety\n/// Never called.\npub unsafe fn cham3_lint_probe(pointer: *const u8) -> u8 {\n    *pointer\n}\n"
        self.edit("core/native/secure_memory/src/lib.rs", lambda text: text + insertion)
        status, output = self.cargo("check", "-p", "cham3_secure_memory")
        self.assertNotEqual(status, 0, output)
        self.assertRegex(output, r"unsafe_op_in_unsafe_fn|requires unsafe block")

    def test_native_crate_rejects_undocumented_unsafe_block(self):
        insertion = "\n/// Probe.\npub fn cham3_lint_probe(pointer: *const u8) -> u8 {\n    unsafe { pointer.read_volatile() }\n}\n"
        self.edit("core/native/secure_memory/src/lib.rs", lambda text: text + insertion)
        status, output = self.cargo("clippy", "-p", "cham3_secure_memory", "--", "-D", "warnings")
        self.assertNotEqual(status, 0, output)
        self.assertRegex(output, r"undocumented_unsafe_blocks|unsafe block missing a safety comment")

    def test_only_secure_memory_contains_unsafe(self):
        offenders = []
        for directory, _, files in os.walk(os.path.join(RepoRoot, "core")):
            for name in files:
                if not name.endswith(".rs"):
                    continue
                path = os.path.join(directory, name)
                relative = os.path.relpath(path, RepoRoot)
                if relative.startswith("core/native/secure_memory/"):
                    continue
                with open(path, "r", encoding="utf-8") as handle:
                    code = re.sub(r"//[^\n]*", "", handle.read())
                if re.search(r"\bunsafe\b", code):
                    offenders.append(relative)
        self.assertEqual(offenders, [])


if __name__ == "__main__":
    unittest.main()
