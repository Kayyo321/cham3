"""Disposable mirror of Codex's capture_fallback_probe.py for dumpcap postinst logic (F3).

Codex reproduced wireshark-common's setuid fallback without touching /usr/bin/dumpcap:
it extracted the real postinst, pointed it at a disposable file, supplied the
install-setuid answer, and put a failing setcap stub first on PATH. This module
does the same with two scripts:

  - the real upstream postinst from Debian's wireshark-common (the control: it
    must show the setuid fallback when setcap fails, or the checker proves nothing);
  - the postinst that cham3-desktop ships in the built .deb (the subject).

Each script runs as root against a disposable file in a temporary directory, in
three cases: setcap works, setcap fails, setcap is missing from PATH. The subject
must leave no setuid bit and no capabilities in the failure cases, and must set
cap_net_admin,cap_net_raw=eip in the success case. The upstream control runs with the
install-setuid answer true (Debian's group layout, where the fallback lives); a second
control runs it with the image preseed (false) and shows no set-id bit.

Codex F3 re-review: the shipped postinst must FAIL, after clearing a set-id bit, when
the dpkg-statoverride cannot be registered (dpkg-statoverride replaced by a failing
stub), and the shipped preinst must register the override, clear a set-id bit of an
existing file at install time, and fail when the override cannot be registered. Only
the disposable files, their overrides in this container's dpkg database, the temporary
wireshark group and the install-setuid debconf answer are touched.

Requires root, file capabilities, apt network access (to fetch the upstream .deb)
and a built cham3-desktop .deb. Otherwise the tests are skipped with a NOT_RUN
reason (Codex Remote heavy gate) or fail for a missing built artifact.
"""

import glob
import grp
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402
import m1_support  # noqa: E402

UpstreamPackage = "wireshark-common"
DumpcapLiteral = m1_support.DumpcapPath
PreseedOwner = "wireshark-common"


def group_name(gid):
    try:
        return grp.getgrgid(gid).gr_name
    except KeyError:
        return "gid:%d" % gid


def set_answer(value):
    line = "%s wireshark-common/install-setuid boolean %s\n" % (PreseedOwner, value)
    completed = subprocess.run(["debconf-set-selections"], input=line, text=True, capture_output=True, check=False)
    if completed.returncode != 0:
        raise AssertionError("debconf-set-selections failed: " + completed.stderr.strip())


def override_for(program):
    completed = subprocess.run(["dpkg-statoverride", "--list", program], capture_output=True, text=True,
                               check=False)
    return completed.stdout.strip() if completed.returncode == 0 else ""


def retarget(script_text, program):
    """Point a postinst at the disposable program. The script must name the real path."""
    if DumpcapLiteral not in script_text:
        raise AssertionError("postinst does not name %s; cannot retarget it to a disposable file" % DumpcapLiteral)
    return script_text.replace(DumpcapLiteral, program)


def path_without(tool, scratch):
    """A PATH directory holding a link to every executable except tool (a fresh directory per call)."""
    bin_dir = tempfile.mkdtemp(prefix="path-without-%s-" % tool, dir=scratch)
    seen = set()
    for directory in os.environ.get("PATH", "").split(os.pathsep):
        if not os.path.isdir(directory):
            continue
        for name in sorted(os.listdir(directory)):
            full = os.path.join(directory, name)
            if name == tool or name in seen or not os.path.isfile(full) or not os.access(full, os.X_OK):
                continue
            seen.add(name)
            os.symlink(full, os.path.join(bin_dir, name))
    return bin_dir


def failing_path(scratch, tool):
    """A PATH that puts a stub that exits 1 for tool first (a fresh directory per call)."""
    bin_dir = tempfile.mkdtemp(prefix="failing-%s-" % tool, dir=scratch)
    stub = os.path.join(bin_dir, tool)
    with open(stub, "w", encoding="utf-8") as handle:
        handle.write("#!/bin/sh\necho '%s: simulated failure (F3 probe)' >&2\nexit 1\n" % tool)
    os.chmod(stub, 0o755)
    return bin_dir + os.pathsep + os.environ.get("PATH", "")


class postinst_probe_t(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        gate = m1_support.capability_gate_problem()
        if gate is not None:
            raise unittest.SkipTest(gate)
        cls.scratch = tempfile.mkdtemp(prefix="cham3_postinst_probe_")
        cls.results = {}
        try:
            cls.run_cases()
        except BaseException:
            shutil.rmtree(cls.scratch, ignore_errors=True)
            raise

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.scratch, ignore_errors=True)

    @classmethod
    def control_dir(cls, name, deb):
        target = os.path.join(cls.scratch, "control-" + name)
        completed = subprocess.run(["dpkg-deb", "-e", deb, target], capture_output=True, text=True, check=False)
        if completed.returncode != 0:
            raise AssertionError("dpkg-deb -e failed for %s: %s" % (deb, completed.stderr.strip()))
        return target

    @classmethod
    def fetch_upstream(cls):
        download_dir = os.path.join(cls.scratch, "upstream")
        os.makedirs(download_dir)
        completed = subprocess.run(
            ["apt-get", "download", UpstreamPackage], cwd=download_dir, capture_output=True, text=True, check=False
        )
        matches = sorted(glob.glob(os.path.join(download_dir, UpstreamPackage + "_*.deb")))
        if completed.returncode != 0 or not matches:
            raise AssertionError("BLOCKED: could not download %s: %s" % (UpstreamPackage, completed.stderr.strip()))
        return matches[-1]

    @classmethod
    def apply_preseed(cls):
        preseed = m1_support.read_repo(m1_support.PreseedFile)
        lines = [line for line in preseed.splitlines() if line.strip() and not line.lstrip().startswith("#")]
        completed = subprocess.run(
            ["debconf-set-selections"], input="\n".join(lines) + "\n", text=True, capture_output=True, check=False
        )
        if completed.returncode != 0:
            raise AssertionError("debconf-set-selections failed: " + completed.stderr.strip())

    @classmethod
    def run_script(cls, name, script_text, path_env=None, action="configure", initial_mode=0o755):
        """Run one maintainer script action against a fresh disposable program; return its state."""
        case_dir = os.path.join(cls.scratch, "case-" + name)
        os.makedirs(case_dir)
        program = os.path.join(case_dir, "dumpcap")
        with open(program, "w", encoding="utf-8") as handle:
            handle.write("disposable file, not a program\n")
        os.chmod(program, initial_mode)
        script = os.path.join(case_dir, "script")
        with open(script, "w", encoding="utf-8") as handle:
            handle.write(retarget(script_text, program))
        os.chmod(script, 0o755)
        env = dict(os.environ)
        env["DEBIAN_FRONTEND"] = "noninteractive"
        if path_env is not None:
            env["PATH"] = path_env
        completed = subprocess.run(
            ["sh", script, action], env=env, capture_output=True, text=True, check=False, timeout=600
        )
        st = os.stat(program)
        getcap = subprocess.run(["getcap", program], capture_output=True, text=True, check=False)
        caps_text = getcap.stdout.strip()
        result = {
            "rc": completed.returncode,
            "stderr": completed.stderr,
            "mode": stat.S_IMODE(st.st_mode),
            "group": group_name(st.st_gid),
            "caps": caps_text,
            "override": override_for(program),
            "program": program,
        }
        if result["override"]:
            subprocess.run(["dpkg-statoverride", "--remove", program], capture_output=True, check=False)
        return result

    @classmethod
    def run_cases(cls):
        cham3_deb = m1_support.shipped_deb(m0_support.out_dir(), m1_support.PackageDesktop)
        upstream_deb = cls.fetch_upstream()
        cls.apply_preseed()
        cham3_control = cls.control_dir("cham3", cham3_deb)
        upstream_control = cls.control_dir("upstream", upstream_deb)
        cls.cham3_path = os.path.join(cham3_control, "postinst")
        cls.preinst_path = os.path.join(cham3_control, "preinst")
        cls.upstream_path = os.path.join(upstream_control, "postinst")
        cls.cham3_text = None
        cls.preinst_text = None
        if os.path.isfile(cls.cham3_path):
            with open(cls.cham3_path, "r", encoding="utf-8") as handle:
                cls.cham3_text = handle.read()
        if os.path.isfile(cls.preinst_path):
            with open(cls.preinst_path, "r", encoding="utf-8") as handle:
                cls.preinst_text = handle.read()
        with open(cls.upstream_path, "r", encoding="utf-8") as handle:
            upstream_text = handle.read()

        # Upstream controls: Debian's group layout (answer true), then the image preseed.
        set_answer("true")
        cls.results["control_upstream_fails"] = cls.run_script(
            "control-upstream-fails", upstream_text, failing_path(cls.scratch, "setcap"))
        cls.results["control_upstream_works"] = cls.run_script("control-upstream-works", upstream_text)
        cls.apply_preseed()
        cls.results["control_upstream_preseed"] = cls.run_script(
            "control-upstream-preseed", upstream_text, failing_path(cls.scratch, "setcap"))
        if cls.cham3_text is not None:
            cls.results["cham3_works"] = cls.run_script("cham3-works", cls.cham3_text)
            cls.results["cham3_fails"] = cls.run_script(
                "cham3-fails", cls.cham3_text, failing_path(cls.scratch, "setcap"))
            cls.results["cham3_missing"] = cls.run_script(
                "cham3-missing", cls.cham3_text, path_without("setcap", cls.scratch))
            cls.results["cham3_override_fails"] = cls.run_script(
                "cham3-override-fails", cls.cham3_text, failing_path(cls.scratch, "dpkg-statoverride"),
                initial_mode=0o4755)
        if cls.preinst_text is not None:
            cls.results["preinst_install"] = cls.run_script(
                "preinst-install", cls.preinst_text, action="install", initial_mode=0o4755)
            cls.results["preinst_override_fails"] = cls.run_script(
                "preinst-override-fails", cls.preinst_text, failing_path(cls.scratch, "dpkg-statoverride"),
                action="install", initial_mode=0o4755)

    def result(self, name):
        if "cham3_works" not in self.results and name.startswith("cham3"):
            self.fail("cham3-desktop ships no postinst: the capture fallback is not enforced by the package")
        return self.results[name]

    def problems(self, name, caps_expected):
        state = self.result(name)
        return m1_support.capture_state_problems(state["mode"], state["group"], state["caps"], caps_expected)

    def test_control_upstream_postinst_sets_setuid_when_setcap_fails(self):
        # Sensitivity control: the Debian fallback this probe exists to catch must appear here.
        state = self.result("control_upstream_fails")
        self.assertTrue(state["mode"] & m1_support.SetuidBit,
                        "control failed: the upstream postinst did not set the setuid bit (mode %04o)" % state["mode"])
        self.assertTrue(any("setuid" in p for p in self.problems("control_upstream_fails", caps_expected=False)))

    def test_control_upstream_postinst_succeeds_when_setcap_works(self):
        self.assertEqual(self.problems("control_upstream_works", caps_expected=True), [])

    def test_cham3_postinst_sets_capabilities_without_setuid(self):
        self.assertEqual(self.problems("cham3_works", caps_expected=True), [])

    def test_cham3_postinst_fails_closed_when_setcap_fails(self):
        self.assertEqual(self.problems("cham3_fails", caps_expected=False), [])

    def test_cham3_postinst_fails_closed_when_setcap_is_missing(self):
        self.assertEqual(self.problems("cham3_missing", caps_expected=False), [])

    def test_control_upstream_postinst_with_image_preseed_sets_no_setuid(self):
        state = self.result("control_upstream_preseed")
        self.assertEqual(state["rc"], 0, state["stderr"])
        self.assertEqual((state["mode"], state["group"], state["caps"]), (0o755, "root", ""))

    def test_cham3_postinst_registers_the_override(self):
        state = self.result("cham3_works")
        self.assertEqual(state["override"], "root root 750 " + state["program"])

    def test_cham3_postinst_fails_when_the_override_cannot_be_registered(self):
        state = self.result("cham3_override_fails")
        self.assertNotEqual(state["rc"], 0, "postinst reported success without the override: " + state["stderr"])
        self.assertFalse(state["mode"] & 0o6000, "set-id bit left: mode %04o" % state["mode"])

    def preinst_result(self, name):
        if self.preinst_text is None:
            self.fail("cham3-desktop ships no preinst: the override is not in place before configuration")
        return self.results[name]

    def test_cham3_preinst_registers_the_override_and_clears_set_id(self):
        state = self.preinst_result("preinst_install")
        self.assertEqual(state["rc"], 0, state["stderr"])
        self.assertEqual(state["override"], "root root 750 " + state["program"])
        self.assertFalse(state["mode"] & 0o6000, "set-id bit left: mode %04o" % state["mode"])

    def test_cham3_preinst_fails_when_the_override_cannot_be_registered(self):
        state = self.preinst_result("preinst_override_fails")
        self.assertNotEqual(state["rc"], 0, "preinst reported success without the override: " + state["stderr"])
        self.assertFalse(state["mode"] & 0o6000, "set-id bit left: mode %04o" % state["mode"])


if __name__ == "__main__":
    unittest.main()
