"""Container-stage checks for the dumpcap capture fallback (F3, security).

Run as root in a disposable debian:trixie linux/amd64 container by
tests/container/run_container_checks.sh. The suite installs Debian's wireshark-common
with the image preseed. It then installs the candidate's cham3-desktop control scripts
as a dependency-free stub, so dpkg configures the package and runs the shipped postinst
and triggers. dumpcap is observed after each step:

  1. upstream_ok          install, setcap works            (positive control)
  2. upstream_fallback    setcap missing, reinstall        (control: Debian setuid fallback)
  3. cham3_install        cham3-desktop configured         (capabilities, no setuid)
  4. reinstall_missing    setcap missing, reinstall        (fail closed)
  5. reinstall_failing    setcap present but failing       (fail closed)
  6. recovery             setcap works again, reinstall    (trigger restores capabilities)
  7. reconfigure_ok       dpkg-reconfigure, setcap works   (valid capabilities kept)
  8. reconfigure_missing  dpkg-reconfigure, setcap missing (never setuid; valid caps may remain)

Design, verified in disposable containers: a dpkg file trigger on /usr/bin/dumpcap runs
on reinstall but not on dpkg-reconfigure. With the dpkg-statoverride the postinst
installs, the upstream postinst does nothing on reconfigure, so dumpcap is untouched.
Retained capabilities without setuid are the intended secure state.

Step 2 is a sensitivity control: the setuid fallback must appear, or the checker proves
nothing. The dependency-free stub is not the KDE payload. Member execution and capture
need CAP_NET_ADMIN in the bounding set; unprivileged Docker lacks it, so those checks are
NOT_RUN (skipped with a reason) and are the live Codex Remote privileged gate.
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
MemberUid = 2001
NonmemberUid = 2002
CaptureTimeout = 20
UpstreamPackage = "wireshark-common"
SetuidQuestion = "wireshark-common/install-setuid"
DebconfOwner = "wireshark-common"
StubDroppedFields = ("Depends", "Recommends", "Suggests", "Pre-Depends")
ShippedControlScripts = ("postinst", "preinst", "prerm", "postrm", "triggers")
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


def group_name(gid):
    try:
        return grp.getgrgid(gid).gr_name
    except KeyError:
        return "gid:%d" % gid


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
    completed = run(["debconf-show", DebconfOwner])
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


def failing_setcap_env(scratch):
    """PATH with a setcap stub that exits 1 placed first (the setcap-failing case)."""
    bin_dir = tempfile.mkdtemp(prefix="failing-setcap-", dir=scratch)
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


def dependency_free_stub(desktop_deb, scratch):
    """A package with cham3-desktop's shipped control scripts and no dependencies.

    dpkg does not configure a package with unmet dependencies, so it would never run the
    shipped postinst or triggers in this container. The stub copies the postinst,
    preinst, prerm, postrm, and triggers of the built package unchanged, drops the
    Depends, Recommends, Suggests, and Pre-Depends fields, and has no payload. The KDE
    payload is not installed. The stub is only a harness for the shipped scripts.
    """
    control_dir = os.path.join(scratch, "desktop-control")
    completed = run(["dpkg-deb", "-e", desktop_deb, control_dir], timeout=300)
    if completed.returncode != 0:
        raise AssertionError("dpkg-deb -e failed for %s: %s" % (desktop_deb, completed.stderr.strip()))
    stub_root = os.path.join(scratch, "desktop-stub")
    debian = os.path.join(stub_root, "DEBIAN")
    os.makedirs(debian)
    with open(os.path.join(control_dir, "control"), "r", encoding="utf-8") as handle:
        kept, dropping = [], False
        for line in handle.read().splitlines():
            if line[:1] in (" ", "\t"):
                if not dropping:
                    kept.append(line)
                continue
            dropping = line.split(":", 1)[0] in StubDroppedFields
            if not dropping:
                kept.append(line)
    with open(os.path.join(debian, "control"), "w", encoding="utf-8") as handle:
        handle.write("\n".join(kept) + "\n")
    for name in ShippedControlScripts:
        source = os.path.join(control_dir, name)
        if os.path.isfile(source):
            shutil.copy2(source, os.path.join(debian, name))
    stub = os.path.join(scratch, "cham3-desktop-dependency-free.deb")
    completed = run(["dpkg-deb", "--build", "--root-owner-group", stub_root, stub], timeout=300)
    if completed.returncode != 0 or not os.path.isfile(stub):
        raise AssertionError("could not build the dependency-free stub: " + completed.stderr.strip())
    return stub


class capture_scenarios_t(unittest.TestCase):
    """Runs the scenarios once; each test asserts one step's observed state."""

    @classmethod
    def setUpClass(cls):
        gate = m1_support.capability_gate_problem()
        if gate is not None:
            raise unittest.SkipTest(gate)
        cls.scratch = tempfile.mkdtemp(prefix="cham3_f3_")
        cls.hidden = False
        cls.obs = {}
        cls.failures = []
        cls.preseed_answer = None
        cls.out_dir = m0_support.out_dir()
        cls.desktop_deb = m1_support.shipped_deb(cls.out_dir, m1_support.PackageDesktop)
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
        for name, uid in ((MemberUser, MemberUid), (NonmemberUser, NonmemberUid)):
            if run(["id", "-u", name]).returncode != 0:
                completed = run(["useradd", "--uid", str(uid), "--create-home", name])
                if completed.returncode != 0:
                    raise AssertionError("useradd failed for %s: %s" % (name, completed.stderr.strip()))

    @classmethod
    def apply_preseed(cls):
        preseed_path = os.path.join(m0_support.repo_root(), m1_support.PreseedFile)
        m0_support.require_file(preseed_path, "cham3 wireshark preseed")
        with open(preseed_path, "r", encoding="utf-8") as handle:
            lines = [line for line in handle.read().splitlines()
                     if line.strip() and not line.lstrip().startswith("#")]
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
    def reconfigure_upstream(cls):
        completed = run(["dpkg-reconfigure", "-f", "noninteractive", UpstreamPackage], env=dpkg_env(), timeout=900)
        if completed.returncode != 0:
            raise AssertionError("dpkg-reconfigure %s failed: %s" % (UpstreamPackage, completed.stderr.strip()))

    @classmethod
    def observe(cls, label, capture=False):
        state = dumpcap_state()
        state["member_exec"] = exec_as(MemberUser, [m1_support.DumpcapPath, "-D"])
        state["nonmember_exec"] = exec_as(NonmemberUser, [m1_support.DumpcapPath, "-D"])
        if capture and m1_support.bounding_set_has(m1_support.CapNetAdminBit):
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
        stub = dependency_free_stub(cls.desktop_deb, cls.scratch)
        completed = run(["dpkg", "-i", stub], env=dpkg_env(), timeout=900)
        if completed.returncode != 0:
            cls.failures.append("dpkg -i of the cham3-desktop stub failed: " + completed.stderr.strip())
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

    def require_privileged(self):
        if not m1_support.bounding_set_has(m1_support.CapNetAdminBit):
            self.skipTest(m1_support.PrivilegedGate)

    def require_capture(self):
        self.require_privileged()
        upstream = self.obs.get("upstream_fallback", {})
        if not upstream.get("member_capture", {}).get("bytes"):
            self.skipTest(
                "NOT_RUN: this container cannot capture even with the Debian setuid fallback; "
                "live capture is a Codex Remote heavy gate"
            )

    def problems(self, label, caps_expected):
        state = self.state(label)
        return m1_support.capture_state_problems(state["mode"], state["group"], state["caps"], caps_expected)

    def retained_problems(self, label):
        state = self.state(label)
        return m1_support.retained_state_problems(state["mode"], state["group"], state["caps"])

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

    # Install with cham3-desktop: capabilities, group restriction, and capture.

    def test_install_sets_capabilities_and_no_setuid(self):
        self.assertEqual(self.problems("cham3_install", caps_expected=True), [])

    def test_install_group_has_execute_and_others_do_not(self):
        # Mode-level pair for the group restriction (needs no capability): group execute
        # is granted, other-execute is denied. Run in every container that installs.
        state = self.state("cham3_install")
        self.assertTrue(state["mode"] & m1_support.GroupExecBit, "group cannot execute dumpcap (mode %04o)" % state["mode"])
        self.assertFalse(state["mode"] & m1_support.OtherExecBit, "other users can execute dumpcap (mode %04o)" % state["mode"])

    def test_install_nonmember_cannot_execute_dumpcap(self):
        self.require_privileged()
        result = self.state("cham3_install")["nonmember_exec"]
        self.assertNotEqual(result["rc"], 0, result)
        self.assertTrue(PermissionDeniedMarker.search(result["stderr"]), result["stderr"])

    def test_install_member_can_execute_dumpcap(self):
        self.require_privileged()
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

    # Reconfigure: the security properties hold; valid capabilities may be retained.

    def test_reconfigure_with_setcap_missing_never_introduces_setuid(self):
        problems = [p for p in self.retained_problems("reconfigure_missing") if "setuid" in p]
        self.assertEqual(problems, [], "dpkg-reconfigure introduced a setuid bit")

    def test_reconfigure_with_setcap_missing_keeps_group_and_other_execute_denied(self):
        problems = [p for p in self.retained_problems("reconfigure_missing")
                    if "group" in p or "executable by other" in p]
        self.assertEqual(problems, [])

    def test_reconfigure_with_setcap_missing_retains_only_the_valid_capabilities(self):
        problems = [p for p in self.retained_problems("reconfigure_missing") if "capabilit" in p]
        self.assertEqual(problems, [], "reconfigure left capabilities other than the agreed set")

    def test_reconfigure_with_setcap_working_restores_capabilities(self):
        # Valid capabilities are present after reconfigure: retained from the trigger-restored
        # state, or restored. Either way the agreed set and no setuid.
        self.assertEqual(self.problems("reconfigure_ok", caps_expected=True), [])

    def test_recovery_after_failure_restores_capabilities(self):
        self.assertEqual(self.problems("recovery", caps_expected=True), [])


if __name__ == "__main__":
    unittest.main()
