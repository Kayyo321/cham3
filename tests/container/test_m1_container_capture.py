"""Container-stage checks for the dumpcap capture fallback (F3, security).

Run as root in a disposable debian:trixie linux/amd64 container by
tests/container/run_container_checks.sh. The suite installs Debian's real
wireshark-common with the image preseed, installs the candidate's built
cham3-desktop .deb (dependencies not installed; --force-depends), and checks what
dumpcap looks like after each step:

  1. upstream_ok        install, setcap works            (positive control)
  2. upstream_fallback  setcap missing, reinstall        (control: Debian setuid fallback)
  3. cham3_install      cham3-desktop installed          (capabilities, no setuid)
  4. reinstall_missing  setcap missing, reinstall        (fail closed)
  5. reinstall_failing  setcap present but failing       (fail closed)
  6. recovery           setcap works again, reinstall    (capabilities restored)
  7. reconfigure_ok     dpkg-reconfigure, setcap works
  8. reconfigure_missing dpkg-reconfigure, setcap missing (fail closed)

Step 2 is a sensitivity control. It must show the setuid bit that the upstream
postinst sets, or the checker proves nothing here. Capture-dependent checks run
only when this container can capture (the upstream fallback step captured as a
group member). Otherwise they are NOT_RUN (skipped with reason) and are Codex
Remote heavy gates. The member and nonmember checks prove the group restriction.

Limits: the container has no real network-interface policy and runs
--force-depends, so the KDE dependency closure is not exercised. Reinstall stands
in for a version upgrade. Live-session and installed-system capture are Codex
Remote gates, not established here.
"""

import glob
import grp
import os
import pwd
import re
import shutil
import socket
import stat
import subprocess
import sys
import tempfile
import time
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402
import m1_support  # noqa: E402

SetcapPath = "/usr/sbin/setcap"
HiddenSetcapPath = SetcapPath + ".cham3-f3-hidden"
MemberUser = "cham3member"
NonmemberUser = "cham3nonmember"
CaptureTimeout = 20
UpstreamPackage = "wireshark-common"
DebconfPreseedOwner = "wireshark-common"
SetuidQuestion = "wireshark-common/install-setuid"
PermissionDeniedMarker = re.compile(r"permission denied|not permitted|operation not permitted", re.IGNORECASE)


def run(argv, env=None, timeout=900, cwd=None):
    try:
        return subprocess.run(
            argv,
            capture_output=True,
            text=True,
            check=False,
            env=env,
            cwd=cwd,
            timeout=timeout,
        )
    except FileNotFoundError:
        raise AssertionError("required executable not found: " + argv[0]) from None
    except subprocess.TimeoutExpired:
        raise AssertionError("timed out: " + " ".join(argv))


def dumpcap_state():
    st = os.stat(m1_support.DumpcapPath)
    getcap = run(["getcap", m1_support.DumpcapPath])
    caps = getcap.stdout.strip() if getcap.returncode == 0 else ""
    return {
        "mode": stat.S_IMODE(st.st_mode),
        "owner": pwd.getpwuid(st.st_uid).pw_name,
        "group": group_name(st.st_gid),
        "caps": caps,
    }


def debconf_setuid_answer():
    completed = run(["debconf-show", DebconfPreseedOwner])
    for line in completed.stdout.splitlines():
        if SetuidQuestion in line:
            return line.split(":", 1)[-1].strip()
    return None


def setpriv_prefix(user):
    if user == MemberUser:
        entry = pwd.getpwnam(MemberUser)
        groups = [grp.getgrnam(m1_support.CaptureGroup).gr_gid, entry.pw_gid]
        return [
            "setpriv",
            "--reuid=%d" % entry.pw_uid,
            "--regid=%d" % entry.pw_gid,
            "--groups=%s" % ",".join(str(g) for g in groups),
        ]
    entry = pwd.getpwnam(NonmemberUser)
    return [
        "setpriv",
        "--reuid=%d" % entry.pw_uid,
        "--regid=%d" % entry.pw_gid,
        "--clear-groups",
    ]


def exec_as(user, argv):
    completed = run(setpriv_prefix(user) + argv, timeout=CaptureTimeout)
    return {"rc": completed.returncode, "stderr": completed.stderr, "stdout": completed.stdout}


def capture_as_member(scratch, label):
    """One packet captured on lo as the member; returns rc, stderr and output size."""
    out_file = os.path.join(scratch, "member-%s.pcapng" % label)
    argv = [m1_support.DumpcapPath, "-p", "-i", "lo", "-a", "duration:8", "-c", "1", "-w", out_file]
    process = subprocess.Popen(
        setpriv_prefix(MemberUser) + argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True
    )
    sender = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    deadline = time.time() + CaptureTimeout
    try:
        while process.poll() is None and time.time() < deadline:
            sender.sendto(b"cham3 f3 capture probe", ("127.0.0.1", 9))
            time.sleep(0.5)
    finally:
        sender.close()
    if process.poll() is None:
        process.kill()
    _, stderr = process.communicate()
    size = os.path.getsize(out_file) if os.path.exists(out_file) else 0
    return {"rc": process.returncode, "stderr": stderr, "bytes": size}


def write_failing_setcap(scratch):
    bin_dir = os.path.join(scratch, "failing-setcap-bin")
    os.makedirs(bin_dir)
    stub = os.path.join(bin_dir, "setcap")
    with open(stub, "w", encoding="utf-8") as handle:
        handle.write("#!/bin/sh\necho 'setcap: simulated failure (F3 control)' >&2\nexit 1\n")
    os.chmod(stub, 0o755)
    return {"PATH": bin_dir + os.pathsep + os.environ.get("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")}


def upstream_deb_path(scratch):
    """Path to the installed-version wireshark-common .deb, downloaded if apt has not kept it."""
    cached = sorted(glob.glob("/var/cache/apt/archives/%s_*.deb" % UpstreamPackage))
    if cached:
        return cached[-1]
    target = os.path.join(scratch, "upstream-download")
    os.makedirs(target, exist_ok=True)
    completed = run(["apt-get", "download", UpstreamPackage], timeout=600, cwd=target)
    fetched = sorted(glob.glob(os.path.join(target, "%s_*.deb" % UpstreamPackage)))
    if completed.returncode != 0 or not fetched:
        raise AssertionError("could not fetch %s: %s" % (UpstreamPackage, completed.stderr.strip()))
    return fetched[-1]


def dpkg_env(extra=None):
    env = dict(os.environ)
    env["DEBIAN_FRONTEND"] = "noninteractive"
    env["LC_ALL"] = "C"
    if extra:
        env.update(extra)
    return env


class capture_scenarios_t(unittest.TestCase):
    """Runs the scenarios once; each test asserts one step's observed state."""

    @classmethod
    def setUpClass(cls):
        gate = m1_support.capability_gate_problem()
        if gate is not None:
            raise unittest.SkipTest(gate)
        cls.scratch = tempfile.mkdtemp(prefix="cham3_f3_")
        cls.out_dir = m0_support.out_dir()
        cls.desktop_deb = m1_support.shipped_deb(cls.out_dir, m1_support.PackageDesktop)
        cls.hidden = False
        cls.obs = {}
        cls.failures = []
        cls.preseed_answer = None
        try:
            cls.run_scenarios()
        finally:
            cls.restore_setcap()

    @classmethod
    def tearDownClass(cls):
        cls.restore_setcap()
        shutil.rmtree(cls.scratch, ignore_errors=True)

    @classmethod
    def restore_setcap(cls):
        if cls.hidden and os.path.exists(HiddenSetcapPath):
            os.rename(HiddenSetcapPath, SetcapPath)
        cls.hidden = False

    @classmethod
    def hide_setcap(cls):
        if not os.path.exists(SetcapPath):
            raise AssertionError("cannot hide setcap: " + SetcapPath + " is absent")
        os.rename(SetcapPath, HiddenSetcapPath)
        cls.hidden = True

    @classmethod
    def ensure_users(cls):
        for name, uid in ((MemberUser, 2001), (NonmemberUser, 2002)):
            if run(["id", "-u", name]).returncode != 0:
                completed = run(["useradd", "--uid", str(uid), "--create-home", name])
                if completed.returncode != 0:
                    raise AssertionError("useradd failed for %s: %s" % (name, completed.stderr.strip()))

    @classmethod
    def apply_preseed(cls):
        preseed_path = os.path.join(m0_support.repo_root(), m1_support.PreseedFile)
        m0_support.require_file(preseed_path, "cham3 wireshark preseed")
        with open(preseed_path, "r", encoding="utf-8") as handle:
            lines = [line for line in handle.read().splitlines() if line.strip() and not line.lstrip().startswith("#")]
        completed = subprocess.run(
            ["debconf-set-selections"], input="\n".join(lines) + "\n", text=True, capture_output=True, check=False
        )
        if completed.returncode != 0:
            raise AssertionError("debconf-set-selections failed: " + completed.stderr.strip())

    @classmethod
    def purge_upstream(cls):
        run(["apt-get", "purge", "-y", "-qq", UpstreamPackage], env=dpkg_env(), timeout=900)

    @classmethod
    def install_upstream(cls):
        completed = run(["apt-get", "install", "-y", "-qq", UpstreamPackage], env=dpkg_env(), timeout=1200)
        if completed.returncode != 0:
            raise AssertionError("apt-get install %s failed: %s" % (UpstreamPackage, completed.stderr.strip()))

    @classmethod
    def reinstall_upstream(cls, extra_env=None):
        deb = upstream_deb_path(cls.scratch)
        completed = run(["dpkg", "-i", deb], env=dpkg_env(extra_env), timeout=900)
        if completed.returncode != 0:
            raise AssertionError("dpkg -i reinstall of %s failed: %s" % (UpstreamPackage, completed.stderr.strip()))

    @classmethod
    def reconfigure_upstream(cls, extra_env=None):
        completed = run(["dpkg-reconfigure", "-f", "noninteractive", UpstreamPackage], env=dpkg_env(extra_env), timeout=900)
        if completed.returncode != 0:
            raise AssertionError("dpkg-reconfigure %s failed: %s" % (UpstreamPackage, completed.stderr.strip()))

    @classmethod
    def observe(cls, label, capture=False):
        state = dumpcap_state()
        state["member_exec"] = exec_as(MemberUser, [m1_support.DumpcapPath, "-D"])
        state["nonmember_exec"] = exec_as(NonmemberUser, [m1_support.DumpcapPath, "-D"])
        if capture:
            state["member_capture"] = capture_as_member(cls.scratch, label)
        cls.obs[label] = state
        return state

    @classmethod
    def run_scenarios(cls):
        cls.ensure_users()
        cls.apply_preseed()
        cls.purge_upstream()

        cls.install_upstream()
        cls.preseed_answer = debconf_setuid_answer()
        # Test fixture: the member account is a deliberate wireshark group member, the
        # way an administrator adds one. The image itself adds no members.
        membership = run(["usermod", "-aG", m1_support.CaptureGroup, MemberUser])
        if membership.returncode != 0:
            raise AssertionError("usermod could not add the member to wireshark: " + membership.stderr.strip())
        cls.observe("upstream_ok", capture=True)

        cls.hide_setcap()
        cls.reinstall_upstream()
        cls.observe("upstream_fallback", capture=True)
        cls.restore_setcap()

        cls.purge_upstream()
        cls.install_upstream()
        completed = run(["dpkg", "-i", "--force-depends", cls.desktop_deb], env=dpkg_env(), timeout=900)
        if completed.returncode != 0:
            cls.failures.append("dpkg -i cham3-desktop failed: " + completed.stderr.strip())
        cls.observe("cham3_install", capture=True)

        cls.hide_setcap()
        cls.reinstall_upstream()
        cls.observe("reinstall_missing", capture=True)
        cls.restore_setcap()

        cls.reinstall_upstream(extra_env=failing_setcap_env(cls.scratch))
        cls.observe("reinstall_failing", capture=True)

        cls.reinstall_upstream()
        cls.observe("recovery", capture=True)

        cls.reconfigure_upstream()
        cls.observe("reconfigure_ok")

        cls.hide_setcap()
        cls.reconfigure_upstream()
        cls.observe("reconfigure_missing")
        cls.restore_setcap()

    def state(self, label):
        self.assertIn(label, self.obs, "scenario did not run: " + label)
        return self.obs[label]

    def require_capture(self):
        upstream = self.obs.get("upstream_fallback", {})
        if not upstream.get("member_capture", {}).get("bytes"):
            self.skipTest(
                "NOT_RUN: this container cannot capture even with the Debian setuid fallback; "
                "live capture is a Codex Remote heavy gate"
            )

    def problems(self, label, caps_expected):
        state = self.state(label)
        return m1_support.capture_state_problems(state["mode"], state["group"], state["caps"], caps_expected)

    # Positive and sensitivity controls.

    def test_preseed_selects_the_setuid_question_true(self):
        self.assertEqual(self.preseed_answer, "true", "preseed not applied: %r" % self.preseed_answer)

    def test_cham3_desktop_installs_on_top_of_wireshark_common(self):
        self.assertEqual(self.failures, [], "cham3-desktop install failed")

    def test_control_upstream_fallback_produces_setuid(self):
        # Sensitivity control: the setuid fallback this suite is meant to catch exists here.
        state = self.state("upstream_fallback")
        self.assertTrue(state["mode"] & m1_support.SetuidBit,
                        "control failed: upstream setcap-missing step did not set the setuid bit (mode %04o)" % state["mode"])

    def test_control_checker_detects_the_upstream_setuid_fallback(self):
        problems = self.problems("upstream_fallback", caps_expected=False)
        self.assertTrue(any("setuid" in p for p in problems), problems)

    def test_control_upstream_success_state_is_accepted(self):
        self.assertEqual(self.problems("upstream_ok", caps_expected=True), [])

    # Install with cham3-desktop: capabilities, group restriction and capture.

    def test_install_sets_capabilities_and_no_setuid(self):
        self.assertEqual(self.problems("cham3_install", caps_expected=True), [])

    def test_install_nonmember_cannot_execute_dumpcap(self):
        result = self.state("cham3_install")["nonmember_exec"]
        self.assertNotEqual(result["rc"], 0, result)
        self.assertTrue(PermissionDeniedMarker.search(result["stderr"]), result["stderr"])

    def test_install_member_can_execute_dumpcap(self):
        result = self.state("cham3_install")["member_exec"]
        self.assertEqual(result["rc"], 0, result["stderr"])

    def test_install_member_captures_with_capabilities(self):
        self.require_capture()
        capture = self.state("cham3_install")["member_capture"]
        self.assertEqual(capture["rc"], 0, capture["stderr"])
        self.assertGreater(capture["bytes"], 100, "capture file has no packet")

    # Failure paths: fail closed (no setuid, no capabilities).

    def test_reinstall_with_setcap_missing_leaves_no_setuid(self):
        problems = [p for p in self.problems("reinstall_missing", caps_expected=False) if "setuid" in p]
        self.assertEqual(problems, [])

    def test_reinstall_with_setcap_missing_leaves_no_capabilities(self):
        problems = [p for p in self.problems("reinstall_missing", caps_expected=False) if "capabilities" in p]
        self.assertEqual(problems, [])

    def test_reinstall_with_setcap_failing_leaves_no_setuid(self):
        problems = [p for p in self.problems("reinstall_failing", caps_expected=False) if "setuid" in p]
        self.assertEqual(problems, [])

    def test_reinstall_with_setcap_failing_leaves_no_capabilities(self):
        problems = [p for p in self.problems("reinstall_failing", caps_expected=False) if "capabilities" in p]
        self.assertEqual(problems, [])

    def test_member_cannot_capture_in_fail_closed_state(self):
        self.require_capture()
        capture = self.state("reinstall_missing")["member_capture"]
        self.assertNotEqual(capture["rc"], 0, "capture succeeded without capabilities")

    def test_reconfigure_with_setcap_missing_leaves_no_setuid(self):
        problems = [p for p in self.problems("reconfigure_missing", caps_expected=False) if "setuid" in p]
        self.assertEqual(problems, [])

    def test_reconfigure_with_setcap_missing_leaves_no_capabilities(self):
        problems = [p for p in self.problems("reconfigure_missing", caps_expected=False) if "capabilities" in p]
        self.assertEqual(problems, [])

    # Recovery paths: capabilities come back when setcap works again.

    def test_reconfigure_with_setcap_working_restores_capabilities(self):
        self.assertEqual(self.problems("reconfigure_ok", caps_expected=True), [])

    def test_recovery_after_failure_restores_capabilities(self):
        self.assertEqual(self.problems("recovery", caps_expected=True), [])


def failing_setcap_env(scratch):
    return write_failing_setcap(scratch)


if __name__ == "__main__":
    unittest.main()
