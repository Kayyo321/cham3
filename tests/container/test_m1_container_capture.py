"""Container-stage checks for the dumpcap capture policy (F3 and F8, security).

Run as root in a disposable debian:trixie linux/amd64 container by
tests/container/run_container_checks.sh (ordinary Docker) or directly by
tests/container/in_container.sh in a privileged disposable container. The suite uses
Debian's real wireshark-common and dpkg, and the candidate's cham3-desktop control
scripts in a dependency-free stub (no KDE payload), so dpkg runs the shipped preinst,
postinst, postrm and triggers exactly as built. dumpcap is observed after each step.

Controls (wireshark-common alone):
  upstream_ok        install-setuid true, setcap works      (Debian group + capabilities)
  upstream_fallback  install-setuid true, setcap missing    (sensitivity: Debian setuid fallback)
  upstream_false     install-setuid false (image preseed), setcap failing (root:root 0755, no setuid)

First installation, every supported ordering (Codex F3 re-review). install-setuid is TRUE
here, the worst case that does not rely on the image preseed, and setcap FAILS while
wireshark-common is configured:
  order_stub_first      dpkg --unpack cham3-desktop wireshark-common; configure
                        wireshark-common only (interrupted run), then cham3-desktop
  order_upstream_first  dpkg --unpack wireshark-common cham3-desktop; configure
                        wireshark-common only (interrupted run), then cham3-desktop
                        (failing setcap: fail closed), then recovery
  order_apt             apt-get install of both in one run, apt choosing the order (the
                        final state is observed; the transient states are covered by
                        the two dpkg orderings above)
  order_preseed_false   install-setuid false: wireshark-common fully configured before
                        cham3-desktop is unpacked (the image-build ordering gap), then
                        cham3-desktop
  order_found_setuid    install-setuid true and setcap missing: wireshark-common left
                        set-user-id BEFORE cham3-desktop exists (a state found, not
                        created); the preinst clears it at unpack time

Override registration must fail safely:
  preinst_override_failure   dpkg-statoverride fails in the preinst: the unpack is
                             refused and dumpcap stays without a set-id bit
  postinst_override_failure  the override is missing and cannot be registered at
                             configure: configure fails and a set-id bit is cleared
  setid_override_refused     an existing override with a set-id mode: the unpack is refused

Lifecycle after a normal installation: reinstall with setcap missing or failing (fail
closed), recovery, dpkg-reconfigure of wireshark-common with setcap working or missing,
dpkg-reconfigure cham3-desktop, and a real wireshark-common version downgrade/upgrade when
apt offers two versions.

Capture (F8): the tested program is executed only after credentials are dropped (setpriv
starts a shell as the user, and the shell executes dumpcap), so the executable permission
check is made with the user's identity. Member capture writes into a directory owned by
the member; an output-path failure is a harness error, never "capture unavailable".
Member execution and capture need CAP_NET_ADMIN and CAP_NET_RAW in the bounding set,
which ordinary Docker lacks: there those checks are skipped with that reason. With
CHAM3_REQUIRE_PRIVILEGED_CAPTURE=1 (the privileged gate) a missing capability is a FAIL
and no capture check may be skipped.
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
OutputPathFailure = re.compile(r"could not be opened|The file to which the capture would be saved", re.IGNORECASE)
PinListing = "root root 750 " + m1_support.DumpcapPath
RequirePrivilegedVariable = "CHAM3_REQUIRE_PRIVILEGED_CAPTURE"
CapNetRawBit = 13
ExecHelper = 'exec "$0" "$@"'
# setpriv starts a shell with the dropped identity; the shell executes the tested program.
LaunchTail = ["--", "/bin/sh", "-c", ExecHelper]
UnpackedStates = ("installed", "unpacked", "half-configured", "triggers-awaited", "triggers-pending")


def run(argv, env=None, timeout=900, cwd=None, input_text=None):
    try:
        return subprocess.run(
            argv,
            capture_output=True,
            text=True,
            check=False,
            env=env,
            cwd=cwd,
            timeout=timeout,
            input=input_text,
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
    if not os.path.exists(m1_support.DumpcapPath):
        return {"mode": None, "owner": None, "group": None, "caps": "", "present": False}
    st = os.stat(m1_support.DumpcapPath)
    getcap = run(["getcap", m1_support.DumpcapPath])
    caps = getcap.stdout.strip() if getcap.returncode == 0 else ""
    return {
        "mode": stat.S_IMODE(st.st_mode),
        "owner": pwd.getpwuid(st.st_uid).pw_name,
        "group": group_name(st.st_gid),
        "caps": caps,
        "present": True,
    }


def override_listing():
    completed = run(["dpkg-statoverride", "--list", m1_support.DumpcapPath])
    return completed.stdout.strip() if completed.returncode == 0 else ""


def package_status(package):
    completed = run(["dpkg-query", "-W", "-f=${Status}", package])
    return completed.stdout.strip() if completed.returncode == 0 else "not-installed"


def debconf_setuid_answer():
    completed = run(["debconf-show", DebconfOwner])
    for line in completed.stdout.splitlines():
        if SetuidQuestion in line:
            return line.split(":", 1)[-1].strip()
    return None


def require_privileged_capture():
    return os.environ.get(RequirePrivilegedVariable) == "1"


def capture_capability_problem():
    """None when member execution and capture can be exercised here; else the reason."""
    missing = [name for name, bit in (("CAP_NET_ADMIN", m1_support.CapNetAdminBit), ("CAP_NET_RAW", CapNetRawBit))
               if not m1_support.bounding_set_has(bit)]
    if missing:
        return ("%s not in this container's bounding set (ordinary Docker); member execution and capture "
                "cannot be exercised here" % " and ".join(missing))
    return None


def setpriv_prefix(user):
    """setpriv arguments that switch to the user, then start a shell that executes the program.

    The shell is the first program started with the user's identity, so the executable
    permission check on the tested program is made after the credentials are dropped.
    """
    if user == MemberUser:
        entry = pwd.getpwnam(MemberUser)
        groups = [entry.pw_gid]
        try:
            groups.insert(0, grp.getgrnam(m1_support.CaptureGroup).gr_gid)
        except KeyError:
            pass  # no wireshark group yet (install-setuid false, before cham3-desktop)
        credentials = [
            "--reuid=%d" % entry.pw_uid,
            "--regid=%d" % entry.pw_gid,
            "--groups=%s" % ",".join(str(g) for g in groups),
        ]
    else:
        entry = pwd.getpwnam(NonmemberUser)
        credentials = ["--reuid=%d" % entry.pw_uid, "--regid=%d" % entry.pw_gid, "--clear-groups"]
    return ["setpriv"] + credentials + LaunchTail


def exec_as(user, argv):
    completed = run(setpriv_prefix(user) + argv, timeout=CaptureTimeout)
    return {"rc": completed.returncode, "stderr": completed.stderr, "stdout": completed.stdout}


def member_output_dir(label):
    """A directory owned by the member (mode 0700) under /tmp, for the capture file."""
    entry = pwd.getpwnam(MemberUser)
    path = tempfile.mkdtemp(prefix="cham3-member-capture-%s-" % label, dir="/tmp")
    os.chown(path, entry.pw_uid, entry.pw_gid)
    os.chmod(path, 0o700)
    return path


def capture_as_member(label):
    """One packet captured on lo as the member; returns rc, stderr, output size and path class."""
    out_dir = member_output_dir(label)
    out_file = os.path.join(out_dir, "member-%s.pcapng" % label)
    argv = [m1_support.DumpcapPath, "-p", "-i", "lo", "-a", "duration:8", "-c", "1", "-w", out_file]
    process = subprocess.Popen(
        setpriv_prefix(MemberUser) + argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True
    )
    sender = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    deadline = time.time() + CaptureTimeout
    try:
        while process.poll() is None and time.time() < deadline:
            sender.sendto(b"cham3 f3 capture probe", ("127.0.0.1", 9))
            time.sleep(0.3)
    finally:
        sender.close()
    if process.poll() is None:
        process.kill()
    _, stderr = process.communicate()
    size = os.path.getsize(out_file) if os.path.exists(out_file) else 0
    shutil.rmtree(out_dir, ignore_errors=True)
    return {"rc": process.returncode, "stderr": stderr, "bytes": size,
            "output_path_failure": bool(OutputPathFailure.search(stderr))}


def tool_override_env(scratch, tool, mode):
    """PATH with a stub for tool placed first: 'fail' exits 1."""
    bin_dir = tempfile.mkdtemp(prefix="%s-%s-" % (mode, tool), dir=scratch)
    stub = os.path.join(bin_dir, tool)
    with open(stub, "w", encoding="utf-8") as handle:
        handle.write("#!/bin/sh\necho '%s: simulated failure (F3 control)' >&2\nexit 1\n" % tool)
    os.chmod(stub, 0o755)
    return {"PATH": bin_dir + os.pathsep + os.environ.get("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")}


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
    if not os.path.isdir(control_dir):
        completed = run(["dpkg-deb", "-e", desktop_deb, control_dir], timeout=300)
        if completed.returncode != 0:
            raise AssertionError("dpkg-deb -e failed for %s: %s" % (desktop_deb, completed.stderr.strip()))
    stub = os.path.join(scratch, "cham3-desktop-dependency-free.deb")
    if os.path.isfile(stub):
        return stub
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
    completed = run(["dpkg-deb", "--build", "--root-owner-group", stub_root, stub], timeout=300)
    if completed.returncode != 0 or not os.path.isfile(stub):
        raise AssertionError("could not build the dependency-free stub: " + completed.stderr.strip())
    return stub


class capture_scenarios_t(unittest.TestCase):
    """Runs every scenario once; each test asserts one observation."""

    @classmethod
    def setUpClass(cls):
        gate = m1_support.capability_gate_problem()
        if gate is not None:
            if require_privileged_capture():
                raise AssertionError("FAIL: %s=1 but %s" % (RequirePrivilegedVariable, gate))
            raise unittest.SkipTest(gate)
        cls.capture_problem = capture_capability_problem()
        if cls.capture_problem is not None and require_privileged_capture():
            raise AssertionError("FAIL: %s=1 but %s" % (RequirePrivilegedVariable, cls.capture_problem))
        cls.scratch = tempfile.mkdtemp(prefix="cham3_f3_")
        cls.hidden = False
        cls.obs = {}
        cls.steps = {}
        cls.answers = {}
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

    # Environment helpers.

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
    def ensure_membership(cls):
        # Test fixture: the member account is a deliberate wireshark group member, the
        # way an administrator adds one. The image itself adds no members.
        if run(["getent", "group", m1_support.CaptureGroup]).returncode != 0:
            return
        completed = run(["usermod", "-aG", m1_support.CaptureGroup, MemberUser])
        if completed.returncode != 0:
            raise AssertionError("usermod could not add the member to wireshark: " + completed.stderr.strip())

    @classmethod
    def set_answer(cls, value):
        line = "%s %s boolean %s\n" % (DebconfOwner, SetuidQuestion, value)
        completed = run(["debconf-set-selections"], input_text=line)
        if completed.returncode != 0:
            raise AssertionError("debconf-set-selections failed: " + completed.stderr.strip())

    @classmethod
    def step(cls, label, argv, extra_env=None, expect_success=True):
        completed = run(argv, env=dpkg_env(extra_env), timeout=1200)
        cls.steps[label] = {"argv": argv, "rc": completed.returncode,
                            "output": completed.stdout + completed.stderr}
        if expect_success and completed.returncode != 0:
            raise AssertionError("%s failed (%d): %s" % (" ".join(argv), completed.returncode,
                                                         (completed.stdout + completed.stderr).strip()))
        return completed

    @classmethod
    def reset(cls):
        """Remove both packages and any override: a fresh first-installation state."""
        cls.restore_setcap()
        run(["dpkg", "--purge", m1_support.PackageDesktop], env=dpkg_env())
        run(["apt-get", "purge", "-y", "-qq", UpstreamPackage], env=dpkg_env(), timeout=900)
        if override_listing():
            completed = run(["dpkg-statoverride", "--remove", m1_support.DumpcapPath])
            if completed.returncode != 0:
                raise AssertionError("could not remove a leftover override: " + completed.stderr.strip())
        if os.path.exists(m1_support.DumpcapPath):
            raise AssertionError("dumpcap still present after purging wireshark-common")

    @classmethod
    def observe(cls, label, capture=False):
        state = dumpcap_state()
        state["override"] = override_listing()
        state["cham3_status"] = package_status(m1_support.PackageDesktop)
        state["upstream_status"] = package_status(UpstreamPackage)
        if state["present"] and cls.capture_problem is None:
            cls.ensure_membership()
            state["member_exec"] = exec_as(MemberUser, [m1_support.DumpcapPath, "-D"])
            state["nonmember_exec"] = exec_as(NonmemberUser, [m1_support.DumpcapPath, "-D"])
            if capture:
                state["member_capture"] = capture_as_member(label)
        cls.obs[label] = state
        return state

    # Scenarios.

    @classmethod
    def run_scenarios(cls):
        cls.ensure_users()
        # Install wireshark-common once so its library dependencies stay present for the
        # dpkg --unpack orderings below; every scenario then starts from reset().
        cls.set_answer("false")
        cls.step("prepare", ["apt-get", "install", "-y", "-qq", UpstreamPackage])
        cls.upstream_deb = cls.fetch_upstream_deb()
        cls.stub = dependency_free_stub(cls.desktop_deb, cls.scratch)
        failing_setcap = tool_override_env(cls.scratch, "setcap", "fail")
        failing_override = tool_override_env(cls.scratch, "dpkg-statoverride", "fail")

        # Controls: wireshark-common alone.
        cls.reset()
        cls.set_answer("true")
        cls.step("upstream_ok", ["apt-get", "install", "-y", "-qq", UpstreamPackage])
        cls.answers["upstream_ok"] = debconf_setuid_answer()
        cls.observe("upstream_ok", capture=True)
        cls.hide_setcap()
        cls.step("upstream_fallback", ["dpkg", "-i", cls.upstream_deb])
        cls.restore_setcap()
        cls.observe("upstream_fallback")

        cls.reset()
        cls.set_answer("false")
        cls.step("upstream_false", ["apt-get", "install", "-y", "-qq", UpstreamPackage], failing_setcap)
        cls.answers["upstream_false"] = debconf_setuid_answer()
        cls.observe("upstream_false")

        # First installation orderings (install-setuid true, setcap failing upstream).
        cls.reset()
        cls.set_answer("true")
        cls.step("order_stub_first_unpack", ["dpkg", "--unpack", cls.stub, cls.upstream_deb])
        cls.step("order_stub_first_upstream", ["dpkg", "--configure", UpstreamPackage], failing_setcap)
        cls.observe("order_stub_first_interrupted")
        cls.step("order_stub_first_cham3", ["dpkg", "--configure", m1_support.PackageDesktop])
        cls.observe("order_stub_first_complete", capture=True)

        cls.reset()
        cls.set_answer("true")
        cls.step("order_upstream_first_unpack", ["dpkg", "--unpack", cls.upstream_deb, cls.stub])
        cls.step("order_upstream_first_upstream", ["dpkg", "--configure", UpstreamPackage], failing_setcap)
        cls.observe("order_upstream_first_interrupted")
        cls.step("order_upstream_first_cham3", ["dpkg", "--configure", m1_support.PackageDesktop], failing_setcap)
        cls.observe("order_upstream_first_fail_closed", capture=True)
        cls.step("order_upstream_first_recovery", ["dpkg-reconfigure", "-f", "noninteractive",
                                                   m1_support.PackageDesktop])
        cls.observe("order_upstream_first_recovered", capture=True)

        cls.reset()
        cls.set_answer("true")
        # apt runs dpkg with PATH set from DPkg::Path, not from its own environment, so the
        # failing setcap stub is put first there.
        dpkg_path = failing_setcap["PATH"].split(os.pathsep, 1)[0] + ":/usr/sbin:/usr/bin:/sbin:/bin"
        cls.step("order_apt", ["apt-get", "-o", "DPkg::Path=" + dpkg_path, "install", "-y", "-qq", cls.stub,
                               UpstreamPackage])
        cls.observe("order_apt")

        cls.reset()
        cls.set_answer("false")
        cls.step("order_preseed_false_upstream", ["apt-get", "install", "-y", "-qq", UpstreamPackage], failing_setcap)
        cls.observe("order_preseed_false_before_cham3")
        cls.step("order_preseed_false_cham3", ["dpkg", "-i", cls.stub])
        cls.observe("order_preseed_false_complete")

        cls.reset()
        cls.set_answer("true")
        cls.hide_setcap()
        cls.step("order_found_setuid_upstream", ["apt-get", "install", "-y", "-qq", UpstreamPackage])
        cls.observe("order_found_setuid_before_cham3")
        cls.step("order_found_setuid_unpack", ["dpkg", "--unpack", cls.stub])
        cls.observe("order_found_setuid_after_unpack")
        cls.restore_setcap()
        cls.step("order_found_setuid_cham3", ["dpkg", "--configure", m1_support.PackageDesktop])
        cls.observe("order_found_setuid_complete")

        # Override registration failures.
        cls.reset()
        cls.set_answer("false")
        cls.step("preinst_failure_upstream", ["apt-get", "install", "-y", "-qq", UpstreamPackage])
        cls.step("preinst_override_failure", ["dpkg", "-i", cls.stub], failing_override, expect_success=False)
        cls.observe("preinst_override_failure")

        cls.reset()
        cls.set_answer("false")
        cls.step("postinst_failure_upstream", ["apt-get", "install", "-y", "-qq", UpstreamPackage])
        cls.step("postinst_failure_unpack", ["dpkg", "--unpack", cls.stub])
        cls.step("postinst_failure_remove_override", ["dpkg-statoverride", "--remove", m1_support.DumpcapPath])
        os.chmod(m1_support.DumpcapPath, 0o4755)
        cls.observe("postinst_failure_before")
        cls.step("postinst_override_failure", ["dpkg", "--configure", m1_support.PackageDesktop], failing_override,
                 expect_success=False)
        cls.observe("postinst_override_failure")

        cls.reset()
        cls.set_answer("false")
        cls.step("setid_override_upstream", ["apt-get", "install", "-y", "-qq", UpstreamPackage])
        cls.step("setid_override_add", ["dpkg-statoverride", "--add", "root", "root", "4755", m1_support.DumpcapPath])
        cls.step("setid_override_refused", ["dpkg", "-i", cls.stub], expect_success=False)
        cls.observe("setid_override_refused")
        run(["dpkg-statoverride", "--remove", m1_support.DumpcapPath])

        # Lifecycle after a normal installation (install-setuid true, worst case).
        cls.reset()
        cls.set_answer("true")
        cls.step("cham3_install_upstream", ["apt-get", "install", "-y", "-qq", UpstreamPackage])
        cls.step("cham3_install", ["dpkg", "-i", cls.stub])
        cls.observe("cham3_install", capture=True)

        cls.hide_setcap()
        cls.step("reinstall_missing", ["dpkg", "-i", cls.upstream_deb])
        cls.restore_setcap()
        cls.observe("reinstall_missing", capture=True)

        cls.step("reinstall_failing", ["dpkg", "-i", cls.upstream_deb], failing_setcap)
        cls.observe("reinstall_failing", capture=True)

        cls.step("recovery", ["dpkg", "-i", cls.upstream_deb])
        cls.observe("recovery", capture=True)

        cls.step("reconfigure_ok", ["dpkg-reconfigure", "-f", "noninteractive", UpstreamPackage])
        cls.observe("reconfigure_ok")

        cls.hide_setcap()
        cls.step("reconfigure_missing", ["dpkg-reconfigure", "-f", "noninteractive", UpstreamPackage])
        cls.restore_setcap()
        cls.observe("reconfigure_missing")

        cls.step("reconfigure_cham3", ["dpkg-reconfigure", "-f", "noninteractive", m1_support.PackageDesktop])
        cls.observe("reconfigure_cham3", capture=True)

        cls.versions = cls.available_versions()
        if len(cls.versions) >= 2:
            older, newest = cls.versions[-1], cls.versions[0]
            cls.step("version_older", ["apt-get", "install", "-y", "-qq", "--allow-downgrades"]
                     + cls.same_source_pins(older))
            cls.observe("version_older")
            cls.step("version_upgrade", ["apt-get", "install", "-y", "-qq"] + cls.same_source_pins(newest))
            cls.observe("version_upgrade", capture=True)

    @classmethod
    def fetch_upstream_deb(cls):
        target = os.path.join(cls.scratch, "upstream-download")
        os.makedirs(target, exist_ok=True)
        completed = run(["apt-get", "download", UpstreamPackage], timeout=600, cwd=target)
        fetched = sorted(glob.glob(os.path.join(target, "%s_*.deb" % UpstreamPackage)))
        if completed.returncode != 0 or not fetched:
            raise AssertionError("could not fetch %s: %s" % (UpstreamPackage, completed.stderr.strip()))
        return fetched[-1]

    @classmethod
    def same_source_pins(cls, version):
        """wireshark-common=<version> plus its dependencies that require that exact version."""
        shown = run(["apt-cache", "show", "%s=%s" % (UpstreamPackage, version)])
        depends = ""
        for line in shown.stdout.splitlines():
            if line.startswith("Depends:"):
                depends = line.split(":", 1)[1]
                break
        pins = ["%s=%s" % (UpstreamPackage, version)]
        for relation in depends.split(","):
            match = re.match(r"^\s*([a-z0-9][a-z0-9+.-]*)\s*\(\s*(=|>=)\s*([^)\s]+)\s*\)", relation)
            if match and match.group(3) == version:
                pins.append("%s=%s" % (match.group(1), version))
        return pins

    @classmethod
    def available_versions(cls):
        completed = run(["apt-cache", "madison", UpstreamPackage])
        versions = []
        for line in completed.stdout.splitlines():
            fields = [field.strip() for field in line.split("|")]
            if len(fields) >= 2 and fields[1] not in versions:
                versions.append(fields[1])
        return versions

    # Assertion helpers.

    def state(self, label):
        self.assertIn(label, self.obs, "scenario did not run: " + label)
        return self.obs[label]

    def require_capture_environment(self):
        if self.capture_problem is not None:
            # Only reachable without CHAM3_REQUIRE_PRIVILEGED_CAPTURE=1 (setUpClass fails otherwise).
            self.skipTest("NOT_RUN: " + self.capture_problem)

    def capture(self, label):
        self.require_capture_environment()
        capture = self.state(label).get("member_capture")
        self.assertIsNotNone(capture, "no capture was recorded for " + label)
        self.assertFalse(capture["output_path_failure"],
                         "harness error: the member could not write the capture file: " + capture["stderr"])
        return capture

    def problems(self, label, caps_expected):
        state = self.state(label)
        return m1_support.capture_state_problems(state["mode"], state["group"], state["caps"], caps_expected)

    def retained_problems(self, label):
        state = self.state(label)
        return m1_support.retained_state_problems(state["mode"], state["group"], state["caps"])

    def assert_no_set_id(self, label):
        state = self.state(label)
        self.assertTrue(state["present"], "dumpcap is absent in " + label)
        self.assertFalse(state["mode"] & 0o6000, "%s: dumpcap mode %04o has a set-id bit" % (label, state["mode"]))

    def assert_member_captures(self, label):
        capture = self.capture(label)
        self.assertEqual(capture["rc"], 0, capture["stderr"])
        self.assertGreater(capture["bytes"], 100, "capture file has no packet")

    def assert_member_cannot_capture(self, label):
        capture = self.capture(label)
        self.assertNotEqual(capture["rc"], 0, "capture succeeded without capabilities")
        self.assertEqual(capture["bytes"], 0)

    # Controls.

    def test_control_upstream_answer_true_applied(self):
        self.assertEqual(self.answers.get("upstream_ok"), "true")

    def test_control_upstream_success_state_is_accepted(self):
        state = self.state("upstream_ok")
        self.assertFalse(state["mode"] & m1_support.SetuidBit)
        self.assertEqual(m1_support.parse_capability_text(state["caps"])[0], m1_support.CaptureCapabilities)

    def test_control_environment_can_capture_with_capabilities(self):
        # Positive control for the capture checks: Debian's own group + capabilities layout.
        self.assert_member_captures("upstream_ok")

    def test_control_upstream_fallback_produces_setuid(self):
        # Sensitivity control: the setuid fallback this suite is meant to catch exists here.
        state = self.state("upstream_fallback")
        self.assertTrue(state["mode"] & m1_support.SetuidBit,
                        "control failed: upstream setcap-missing step did not set the setuid bit (mode %04o)"
                        % state["mode"])

    def test_control_checker_detects_the_upstream_setuid_fallback(self):
        problems = self.problems("upstream_fallback", caps_expected=False)
        self.assertTrue(any("setuid" in p for p in problems), problems)

    def test_image_preseed_false_never_selects_setuid(self):
        self.assertEqual(self.answers.get("upstream_false"), "false")
        state = self.state("upstream_false")
        self.assertEqual((state["owner"], state["group"], state["caps"]), ("root", "root", ""))
        self.assertEqual(state["mode"], 0o755)

    # First installation orderings.

    def test_unpack_cham3_first_interrupted_run_has_no_setuid(self):
        self.assert_no_set_id("order_stub_first_interrupted")

    def test_unpack_cham3_first_interrupted_run_has_the_override(self):
        state = self.state("order_stub_first_interrupted")
        self.assertEqual(state["override"], PinListing)
        self.assertIn("unpacked", state["cham3_status"])

    def test_unpack_cham3_first_completed_run_sets_capabilities(self):
        self.assertEqual(self.problems("order_stub_first_complete", caps_expected=True), [])

    def test_unpack_cham3_first_member_captures(self):
        self.assert_member_captures("order_stub_first_complete")

    def test_unpack_upstream_first_interrupted_run_has_no_setuid(self):
        self.assert_no_set_id("order_upstream_first_interrupted")

    def test_unpack_upstream_first_interrupted_run_has_the_override(self):
        state = self.state("order_upstream_first_interrupted")
        self.assertEqual(state["override"], PinListing)
        self.assertIn("unpacked", state["cham3_status"])

    def test_unpack_upstream_first_failing_setcap_fails_closed(self):
        self.assertEqual(self.problems("order_upstream_first_fail_closed", caps_expected=False), [])

    def test_unpack_upstream_first_failing_setcap_member_cannot_capture(self):
        self.assert_member_cannot_capture("order_upstream_first_fail_closed")

    def test_unpack_upstream_first_recovery_sets_capabilities(self):
        self.assertEqual(self.problems("order_upstream_first_recovered", caps_expected=True), [])

    def test_unpack_upstream_first_recovery_member_captures(self):
        self.assert_member_captures("order_upstream_first_recovered")

    def test_apt_ordered_first_install_has_no_setuid(self):
        self.assert_no_set_id("order_apt")
        self.assertEqual(self.problems("order_apt", caps_expected=False), [])
        self.assertEqual(self.state("order_apt")["override"], PinListing)

    def test_preseed_false_upstream_configured_first_has_no_setuid(self):
        state = self.state("order_preseed_false_before_cham3")
        self.assertEqual((state["mode"], state["group"]), (0o755, "root"))

    def test_preseed_false_then_cham3_sets_capabilities(self):
        self.assertEqual(self.problems("order_preseed_false_complete", caps_expected=True), [])

    def test_found_setuid_control_is_setuid_before_cham3(self):
        # The state cham3-desktop finds (not one it creates): sensitivity for the next test.
        self.assertTrue(self.state("order_found_setuid_before_cham3")["mode"] & m1_support.SetuidBit)

    def test_found_setuid_is_cleared_when_cham3_is_unpacked(self):
        self.assert_no_set_id("order_found_setuid_after_unpack")
        self.assertEqual(self.state("order_found_setuid_after_unpack")["override"], PinListing)

    def test_found_setuid_then_configured_sets_capabilities(self):
        self.assertEqual(self.problems("order_found_setuid_complete", caps_expected=True), [])

    # Override registration failures.

    def test_preinst_override_failure_refuses_the_unpack(self):
        self.assertNotEqual(self.steps["preinst_override_failure"]["rc"], 0)
        status = self.state("preinst_override_failure")["cham3_status"]
        self.assertNotIn(status.split()[-1], UnpackedStates, status)
        self.assert_no_set_id("preinst_override_failure")

    def test_postinst_override_failure_fails_configuration(self):
        self.assertTrue(self.state("postinst_failure_before")["mode"] & m1_support.SetuidBit,
                        "control: the set-id bit was not in place before configure")
        self.assertNotEqual(self.steps["postinst_override_failure"]["rc"], 0)
        self.assertNotEqual(self.state("postinst_override_failure")["cham3_status"], "install ok installed")

    def test_postinst_override_failure_clears_set_id_bits(self):
        self.assert_no_set_id("postinst_override_failure")

    def test_existing_set_id_override_is_refused(self):
        self.assertNotEqual(self.steps["setid_override_refused"]["rc"], 0)
        self.assertNotEqual(self.state("setid_override_refused")["cham3_status"], "install ok installed")

    # Normal installation and lifecycle.

    def test_install_sets_capabilities_and_no_setuid(self):
        self.assertEqual(self.problems("cham3_install", caps_expected=True), [])

    def test_install_group_has_execute_and_others_do_not(self):
        state = self.state("cham3_install")
        self.assertTrue(state["mode"] & m1_support.GroupExecBit, "group cannot execute dumpcap (mode %04o)"
                        % state["mode"])
        self.assertFalse(state["mode"] & m1_support.OtherExecBit, "other users can execute dumpcap (mode %04o)"
                         % state["mode"])

    def test_install_nonmember_cannot_execute_dumpcap(self):
        self.require_capture_environment()
        result = self.state("cham3_install")["nonmember_exec"]
        self.assertEqual(result["rc"], 126, result)
        self.assertTrue(PermissionDeniedMarker.search(result["stderr"]), result["stderr"])

    def test_install_member_can_execute_dumpcap(self):
        self.require_capture_environment()
        result = self.state("cham3_install")["member_exec"]
        self.assertEqual(result["rc"], 0, result["stderr"])

    def test_install_member_captures_with_capabilities(self):
        self.assert_member_captures("cham3_install")

    def test_reinstall_with_setcap_missing_fails_closed(self):
        self.assertEqual(self.problems("reinstall_missing", caps_expected=False), [])

    def test_reinstall_with_setcap_failing_fails_closed(self):
        self.assertEqual(self.problems("reinstall_failing", caps_expected=False), [])

    def test_member_cannot_capture_in_fail_closed_state(self):
        self.assert_member_cannot_capture("reinstall_missing")
        self.assert_member_cannot_capture("reinstall_failing")

    def test_recovery_after_failure_restores_capabilities(self):
        self.assertEqual(self.problems("recovery", caps_expected=True), [])
        self.assert_member_captures("recovery")

    def test_reconfigure_with_setcap_working_keeps_valid_capabilities(self):
        self.assertEqual(self.problems("reconfigure_ok", caps_expected=True), [])

    def test_reconfigure_with_setcap_missing_never_introduces_setuid(self):
        self.assertEqual(self.retained_problems("reconfigure_missing"), [])

    def test_reconfigure_cham3_restores_capabilities(self):
        self.assertEqual(self.problems("reconfigure_cham3", caps_expected=True), [])
        self.assert_member_captures("reconfigure_cham3")

    def test_version_downgrade_and_upgrade_keep_policy(self):
        if len(getattr(self, "versions", [])) < 2:
            self.skipTest("NOT_RUN: apt offers only one %s version here (%r); no version change is possible"
                          % (UpstreamPackage, getattr(self, "versions", [])))
        self.assert_no_set_id("version_older")
        self.assertEqual(self.problems("version_upgrade", caps_expected=True), [])
        self.assert_member_captures("version_upgrade")


class harness_controls_t(unittest.TestCase):
    """Fixture checks for the F8 harness corrections (no root needed)."""

    def test_program_is_executed_by_the_shell_after_credentials_drop(self):
        # The program follows the shell's -c script as $0, so setpriv itself never
        # executes it; only the shell does, with the user's identity.
        self.assertEqual(LaunchTail[1:3], ["/bin/sh", "-c"])
        self.assertEqual(LaunchTail[-1], 'exec "$0" "$@"')

    def test_output_path_failure_is_recognised(self):
        stderr = ('dumpcap: The file to which the capture would be saved ("/tmp/x/member.pcapng") '
                  'could not be opened: Permission denied.\n')
        self.assertTrue(OutputPathFailure.search(stderr))
        self.assertFalse(OutputPathFailure.search("dumpcap: You don't have permission to capture on that device\n"))


if __name__ == "__main__":
    unittest.main()
