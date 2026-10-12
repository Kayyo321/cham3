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
cap_net_admin,cap_net_raw=eip in the success case. Only the disposable file, the
temporary wireshark group and the install-setuid debconf answer are touched.

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
    def run_script(cls, name, script_text, path_env=None):
        """Run one postinst configure against a fresh disposable program; return its state."""
        case_dir = os.path.join(cls.scratch, "case-" + name)
        os.makedirs(case_dir)
        program = os.path.join(case_dir, "dumpcap")
        with open(program, "w", encoding="utf-8") as handle:
            handle.write("disposable file, not a program\n")
        os.chmod(program, 0o755)
        script = os.path.join(case_dir, "postinst")
        with open(script, "w", encoding="utf-8") as handle:
            handle.write(retarget(script_text, program))
        os.chmod(script, 0o755)
        env = dict(os.environ)
        env["DEBIAN_FRONTEND"] = "noninteractive"
        if path_env is not None:
            env["PATH"] = path_env
        completed = subprocess.run(
            ["sh", script, "configure"], env=env, capture_output=True, text=True, check=False, timeout=600
        )
        st = os.stat(program)
        getcap = subprocess.run(["getcap", program], capture_output=True, text=True, check=False)
        caps_text = getcap.stdout.strip()
        return {
            "rc": completed.returncode,
            "stderr": completed.stderr,
            "mode": stat.S_IMODE(st.st_mode),
            "group": group_name(st.st_gid),
            "caps": caps_text,
        }

    @classmethod
    def run_cases(cls):
        cham3_deb = m1_support.shipped_deb(m0_support.out_dir(), m1_support.PackageDesktop)
        upstream_deb = cls.fetch_upstream()
        cls.apply_preseed()
        cham3_control = cls.control_dir("cham3", cham3_deb)
        upstream_control = cls.control_dir("upstream", upstream_deb)
        cls.cham3_path = os.path.join(cham3_control, "postinst")
        cls.upstream_path = os.path.join(upstream_control, "postinst")
        cls.cham3_text = None
        if os.path.isfile(cls.cham3_path):
            with open(cls.cham3_path, "r", encoding="utf-8") as handle:
                cls.cham3_text = handle.read()
        with open(cls.upstream_path, "r", encoding="utf-8") as handle:
            upstream_text = handle.read()

        cls.results["control_upstream_fails"] = cls.run_script(
            "control-upstream-fails", upstream_text, failing_path(cls.scratch, "setcap"))
        cls.results["control_upstream_works"] = cls.run_script("control-upstream-works", upstream_text)
        if cls.cham3_text is not None:
            cls.results["cham3_works"] = cls.run_script("cham3-works", cls.cham3_text)
            cls.results["cham3_fails"] = cls.run_script(
                "cham3-fails", cls.cham3_text, failing_path(cls.scratch, "setcap"))
            cls.results["cham3_missing"] = cls.run_script(
                "cham3-missing", cls.cham3_text, path_without("setcap", cls.scratch))

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


if __name__ == "__main__":
    unittest.main()
