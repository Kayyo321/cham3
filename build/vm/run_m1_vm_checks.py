#!/usr/bin/env python3
"""M1 VM gates in disposable QEMU machines: live session, disk installation, installed system.

Subcommands (each writes <out>/results.json, screenshots and the serial transcript):
  live       Boot the M1 ISO (BIOS, the image's own boot menu) without network. On the
             serial console, log in as the live user and record: session type of the
             autologin Plasma session, the running panel (plasmashell scripting), branding
             defaults, dumpcap state, member/nonmember capture, firmware packages and the
             image's apt policy over its retained indices. On screen: desktop, the
             categorized launcher, an application search and a real launch, and each
             backend-dependent widget's inert state (OCR).
  install    Install the ISO to a new virtual disk with the ISO's own debian-installer
             (live mode) and build/vm/m1_install.preseed, appended to a copy of the
             installer initrd. The machine powers off when the installer finishes.
  installed  Boot the installed disk with network. Log in on the serial console and at
             the SDDM greeter, record the same checks, and run the installed-system
             gates: apt update and upgrade origin by download host, an unlisted Kali
             package without candidate, and the dumpcap lifecycle (reinstall, reconfigure,
             setcap missing, recovery) with member/nonmember capture.

Requirements: qemu-system-x86_64, qemu-img, xorriso, cpio, gzip; OCR checks need
tesseract and python3-pil (BLOCKED without them). KVM is used when /dev/kvm is usable,
otherwise TCG. Credentials: the live image's public live-config defaults and the
throwaway account of the test preseed; all machines are disposable.

Limits: a PASS is evidence about this image in this virtual machine (q35, BIOS,
virtio-vga, software rendering). It says nothing about real hardware or UEFI, and no
check here is evidence of M2-M4 security enforcement. OCR reads rendered text and can
miss it; a missed word is a FAIL, never a PASS.

Exit status: 0 every check PASS, 1 any FAIL, 2 BLOCKED (environment).
"""

import argparse
import base64
import hashlib
import json
import os
import re
import shlex
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
import urllib.parse

RepoRoot = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
PreseedPath = os.path.join(RepoRoot, "build/vm/m1_install.preseed")
CaptureProbePath = os.path.join(RepoRoot, "build/vm/guest/m1_capture_probe.sh")
PanelScriptPath = os.path.join(RepoRoot, "build/vm/guest/m1_panel_state.js")
LiveUser = "user"
LivePassword = "live"  # live-config's public default for the live session
Cham3Widgets = ("org.cham3.user_launcher", "org.cham3.alias_wheel", "org.cham3.tag_pager",
                "org.cham3.contract_dock", "org.cham3.netproto_launcher", "org.cham3.killswitch",
                "org.cham3.settings_launcher", "org.cham3.task_manager")
PanelWidgetOrder = Cham3Widgets + ("org.kde.plasma.systemtray", "org.kde.plasma.digitalclock")
# Backend-dependent components whose rendered popup must state an unavailable state.
InertWidgets = ("org.cham3.alias_wheel", "org.cham3.tag_pager", "org.cham3.contract_dock",
                "org.cham3.netproto_launcher", "org.cham3.killswitch")
InertWords = re.compile(r"unavailable|unverified|disconnected", re.IGNORECASE)
MenuCategories = ("Development", "Graphics", "Internet", "Multimedia", "Office", "Settings", "System",
                  "Utilities", "Education", "All Applications")
PanelThicknessRange = (56, 64)
KaliSite = "http.kali.org"
LibreWolfSite = "repo.librewolf.net"
ApprovedSites = ("deb.debian.org", "security.debian.org", KaliSite, LibreWolfSite)
KaliAllowlist = ("zaproxy", "exploitdb", "ghidra", "amass", "libpostal-data", "radare2",
                 "libradare2-6.0.0t64", "libradare2-common")
ForeignRoles = ("zaproxy", "exploitdb", "ghidra", "amass", "radare2", "librewolf")
UnlistedKaliProbes = ("ghidra-data", "kali-defaults", "burpsuite", "metasploit-framework")
FirmwareB43 = ("firmware-b43-installer", "firmware-b43legacy-installer")
ReviewedFirmwareCount = 42
AnsiEscape = re.compile(r"\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b[@-_]")
PromptMarker = "cham3vm$ "
QcodeByChar = {" ": "spc", "-": "minus", ".": "dot", "/": "slash", "=": "equal", ",": "comma",
               ";": "semicolon", "'": "apostrophe", "\n": "ret"}
QcodeShifted = {"_": "minus", ":": "semicolon", "+": "equal", "?": "slash", "!": "1", "@": "2"}


# ---------------------------------------------------------------------------------
# Results


class results_t:
    def __init__(self, out_dir, phase):
        self.out_dir = out_dir
        self.phase = phase
        self.checks = []
        self.facts = {}

    def check(self, name, ok, detail=""):
        result = "PASS" if ok else "FAIL"
        self.checks.append({"name": name, "result": result, "detail": detail})
        print("%s %s %s" % (result, name, detail if isinstance(detail, str) else json.dumps(detail)[:400]),
              flush=True)
        return ok

    def blocked(self, name, detail):
        self.checks.append({"name": name, "result": "BLOCKED", "detail": detail})
        print("BLOCKED %s %s" % (name, detail), flush=True)

    def save(self, extra=None):
        payload = {"phase": self.phase, "checks": self.checks, "facts": self.facts}
        if extra:
            payload.update(extra)
        with open(os.path.join(self.out_dir, "results.json"), "w", encoding="utf-8") as handle:
            json.dump(payload, handle, indent=1, sort_keys=True)

    def status(self):
        if any(check["result"] == "FAIL" for check in self.checks):
            return 1
        if any(check["result"] == "BLOCKED" for check in self.checks):
            return 2
        return 0


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


# ---------------------------------------------------------------------------------
# QEMU control: QMP and serial console


class qmp_t:
    def __init__(self, path, timeout=60):
        self.sock = connect_unix(path, timeout)
        self.sock.settimeout(300)
        self.reader = self.sock.makefile("rb")
        self.read_message()
        self.execute("qmp_capabilities")

    def read_message(self):
        line = self.reader.readline()
        if not line:
            raise RuntimeError("QMP connection closed")
        return json.loads(line)

    def execute(self, command, arguments=None):
        message = {"execute": command}
        if arguments is not None:
            message["arguments"] = arguments
        self.sock.sendall(json.dumps(message).encode() + b"\n")
        while True:
            reply = self.read_message()
            if "return" in reply:
                return reply["return"]
            if "error" in reply:
                raise RuntimeError("QMP %s failed: %s" % (command, reply["error"]))

    def close(self):
        try:
            self.sock.close()
        except OSError:
            pass


def connect_unix(path, timeout):
    deadline = time.time() + timeout
    while True:
        try:
            sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            sock.connect(path)
            return sock
        except OSError:
            sock.close()
            if time.time() > deadline:
                raise
            time.sleep(0.5)


class serial_t:
    """The guest serial console: a background reader, marker-delimited commands."""

    def __init__(self, path, log_path):
        self.sock = connect_unix(path, 60)
        self.buffer = bytearray()
        self.lock = threading.Lock()
        self.log = open(log_path, "ab")
        self.counter = 0
        self.thread = threading.Thread(target=self.read_loop, daemon=True)
        self.thread.start()

    def read_loop(self):
        while True:
            try:
                chunk = self.sock.recv(65536)
            except OSError:
                return
            if not chunk:
                return
            with self.lock:
                self.buffer.extend(chunk)
            self.log.write(chunk)
            self.log.flush()

    def mark(self):
        with self.lock:
            return len(self.buffer)

    def text(self, start=0):
        with self.lock:
            raw = bytes(self.buffer[start:])
        return AnsiEscape.sub("", raw.decode("utf-8", errors="replace")).replace("\r", "")

    def send(self, text):
        self.sock.sendall(text.encode())

    def wait_for(self, pattern, timeout, start=0, poke=None, poke_every=10):
        regex = re.compile(pattern, re.MULTILINE)
        deadline = time.time() + timeout
        next_poke = time.time() + poke_every
        while time.time() < deadline:
            match = regex.search(self.text(start))
            if match:
                return match
            if poke is not None and time.time() >= next_poke:
                self.send(poke)
                next_poke = time.time() + poke_every
            time.sleep(0.5)
        return None

    def run(self, command, timeout=180):
        """Run one shell command line; return (exit status or None on timeout, output)."""
        self.counter += 1
        number = self.counter
        start = self.mark()
        self.send("printf '%%s_%%s\\n' __CHAM3S %d; ( %s ) 2>&1; printf '%%s_%%s_%%s\\n' __CHAM3E %d $?\n"
                  % (number, command, number))
        pattern = r"__CHAM3S_%d\n(.*?)__CHAM3E_%d_(\d+)" % (number, number)
        regex = re.compile(pattern, re.DOTALL)
        deadline = time.time() + timeout
        while time.time() < deadline:
            match = regex.search(self.text(start))
            if match:
                return int(match.group(2)), match.group(1)
            time.sleep(0.3)
        return None, self.text(start)

    def put_file(self, remote_path, data, mode="0644"):
        encoded = base64.b64encode(data).decode()
        status, output = self.run(": > %s.b64" % shlex.quote(remote_path))
        if status != 0:
            raise RuntimeError("cannot create %s: %s" % (remote_path, output))
        for index in range(0, len(encoded), 600):
            status, output = self.run("printf '%%s' '%s' >> %s.b64" % (encoded[index:index + 600],
                                                                       shlex.quote(remote_path)))
            if status != 0:
                raise RuntimeError("cannot write %s: %s" % (remote_path, output))
        status, output = self.run("base64 -d %s.b64 > %s && rm -f %s.b64 && chmod %s %s"
                                  % ((shlex.quote(remote_path),) * 3 + (mode, shlex.quote(remote_path))))
        if status != 0:
            raise RuntimeError("cannot decode %s: %s" % (remote_path, output))

    def close(self):
        try:
            self.sock.close()
        except OSError:
            pass
        self.log.close()


class vm_t:
    def __init__(self, out_dir, name, accel, cpus, memory, cdrom=None, disk=None, network=False,
                 kernel=None, initrd=None, append=None, no_reboot=False):
        self.out_dir = out_dir
        self.name = name
        self.runtime = tempfile.mkdtemp(prefix="cham3vm-")
        self.qmp_path = os.path.join(self.runtime, "qmp.sock")
        self.serial_path = os.path.join(self.runtime, "serial.sock")
        self.serial_log = os.path.join(out_dir, "%s_serial.log" % name)
        argv = ["qemu-system-x86_64", "-name", "cham3-" + name, "-machine", "q35", "-accel", accel,
                "-cpu", "host" if accel == "kvm" else "max", "-smp", str(cpus), "-m", str(memory),
                "-device", "virtio-vga", "-display", "none",
                "-device", "qemu-xhci", "-device", "usb-tablet",
                "-chardev", "socket,id=ser0,path=%s,server=on,wait=off" % self.serial_path,
                "-serial", "chardev:ser0",
                "-qmp", "unix:%s,server=on,wait=off" % self.qmp_path,
                "-nic", "user,model=virtio-net-pci" if network else "none"]
        if cdrom:
            argv += ["-drive", "file=%s,media=cdrom,readonly=on" % cdrom]
        if disk:
            argv += ["-drive", "file=%s,if=virtio,format=qcow2" % disk]
        argv += ["-boot", "order=c" if disk and not cdrom else "order=d"]
        if kernel:
            argv += ["-kernel", kernel, "-initrd", initrd, "-append", append]
        if no_reboot:
            argv += ["-no-reboot"]
        self.argv = argv
        with open(os.path.join(out_dir, "%s_qemu_argv.json" % name), "w", encoding="utf-8") as handle:
            json.dump(argv, handle, indent=1)
        self.stderr = open(os.path.join(out_dir, "%s_qemu_stderr.log" % name), "wb")
        self.process = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=self.stderr, stderr=self.stderr)
        self.qmp = qmp_t(self.qmp_path)
        self.serial = serial_t(self.serial_path, self.serial_log)
        self.shots = 0

    def alive(self):
        return self.process.poll() is None

    def screenshot(self, label):
        self.shots += 1
        path = os.path.join(self.out_dir, "%s_%02d_%s.png" % (self.name, self.shots, label))
        self.qmp.execute("screendump", {"filename": path, "format": "png"})
        for _ in range(20):
            if os.path.exists(path) and os.path.getsize(path) > 0:
                break
            time.sleep(0.25)
        return path

    def key(self, *names):
        self.qmp.execute("send-key", {"keys": [{"type": "qcode", "data": name} for name in names]})
        time.sleep(0.15)

    def type_text(self, text):
        for char in text:
            if char.isalpha() and char.isupper():
                self.key("shift", char.lower())
            elif char.isalnum():
                self.key(char)
            elif char in QcodeShifted:
                self.key("shift", QcodeShifted[char])
            elif char in QcodeByChar:
                self.key(QcodeByChar[char])
            else:
                raise ValueError("no key mapping for %r" % char)

    def click(self, x, y, width, height):
        def axis(value, size):
            return max(0, min(0x7FFF, int(round(value * 0x7FFF / max(1, size - 1)))))
        self.qmp.execute("input-send-event", {"events": [
            {"type": "abs", "data": {"axis": "x", "value": axis(x, width)}},
            {"type": "abs", "data": {"axis": "y", "value": axis(y, height)}}]})
        time.sleep(0.3)
        self.qmp.execute("input-send-event", {"events": [{"type": "btn", "data": {"down": True, "button": "left"}}]})
        time.sleep(0.1)
        self.qmp.execute("input-send-event", {"events": [{"type": "btn", "data": {"down": False, "button": "left"}}]})

    def move(self, x, y, width, height):
        self.qmp.execute("input-send-event", {"events": [
            {"type": "abs", "data": {"axis": "x", "value": int(x * 0x7FFF / max(1, width - 1))}},
            {"type": "abs", "data": {"axis": "y", "value": int(y * 0x7FFF / max(1, height - 1))}}]})

    def stop(self):
        if self.alive():
            try:
                self.qmp.execute("quit")
            except (RuntimeError, OSError):
                pass
            try:
                self.process.wait(timeout=60)
            except subprocess.TimeoutExpired:
                self.process.kill()
        self.qmp.close()
        self.serial.close()
        self.stderr.close()
        shutil.rmtree(self.runtime, ignore_errors=True)


def png_size(path):
    with open(path, "rb") as handle:
        header = handle.read(24)
    if header[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("not a PNG: " + path)
    return struct.unpack(">II", header[16:24])


def choose_accel(requested):
    usable = os.access("/dev/kvm", os.R_OK | os.W_OK)
    if requested == "kvm" and not usable:
        return None
    if requested == "tcg":
        return "tcg"
    return "kvm" if usable else "tcg"


def ocr(path, out_dir, label):
    """Text that tesseract reads from a screenshot (preprocessed for the dark theme), or None."""
    if shutil.which("tesseract") is None:
        return None
    try:
        from PIL import Image, ImageOps  # python3-pil
    except ImportError:
        return None
    image = Image.open(path).convert("L")
    if sum(image.getdata()) / float(image.width * image.height) < 128:
        image = ImageOps.invert(image)
    image = ImageOps.autocontrast(image).resize((image.width * 2, image.height * 2))
    prepared = os.path.join(out_dir, "ocr_%s.png" % label)
    image.save(prepared)
    completed = subprocess.run(["tesseract", prepared, "stdout", "--psm", "11"], capture_output=True, text=True,
                               check=False)
    os.unlink(prepared)
    text = completed.stdout
    with open(os.path.join(out_dir, "ocr_%s.txt" % label), "w", encoding="utf-8") as handle:
        handle.write(text)
    return text


# ---------------------------------------------------------------------------------
# Guest helpers


def login(vm, user, password, timeout):
    """Log in on the serial console; returns True when a marked shell prompt answers."""
    serial = vm.serial
    match = serial.wait_for(r"(login:\s*$)|([$#]\s*$)", timeout, poke="\n", poke_every=15)
    if match is None:
        return False
    if match.group(1):
        start = serial.mark()
        serial.send(user + "\n")
        if serial.wait_for(r"[Pp]assword:\s*$", 60, start=start) is None:
            return False
        start = serial.mark()
        serial.send(password + "\n")
        if serial.wait_for(r"[$#]\s*$", 120, start=start) is None:
            return False
    start = serial.mark()
    serial.send("stty -echo cols 250; export TERM=dumb PAGER=cat SYSTEMD_PAGER=; PS1='%s'\n" % PromptMarker)
    return serial.wait_for(re.escape(PromptMarker), 60, start=start) is not None


def root_command(command, password):
    if password is None:
        return "sudo -n -- sh -c %s" % shlex.quote(command)
    return "printf '%%s\\n' %s | sudo -S -p '' -- sh -c %s" % (shlex.quote(password), shlex.quote(command))


def guest(vm, results, label, command, timeout=180):
    status, output = vm.serial.run(command, timeout)
    results.facts[label] = {"command": command, "status": status, "output": output[-20000:]}
    return status, output


def parse_policy(text):
    """{package: {'candidate': str or None, 'versions': {version: (priority, [sites])}}} from apt-cache policy."""
    packages, current, version = {}, None, None
    for line in text.splitlines():
        if line and not line[0].isspace() and line.rstrip().endswith(":"):
            current = line.rstrip()[:-1]
            packages[current] = {"candidate": None, "versions": {}}
            version = None
            continue
        if current is None:
            continue
        stripped = line.strip()
        if stripped.startswith("Candidate:"):
            value = stripped.split(":", 1)[1].strip()
            packages[current]["candidate"] = None if value == "(none)" else value
            continue
        match = re.match(r"^\s+(?:\*\*\*\s+)?(\S+)\s+(-?\d+)\s*$", line)
        if match and not stripped.startswith(("Installed", "Candidate", "Version")):
            version = match.group(1)
            packages[current]["versions"][version] = [int(match.group(2)), []]
            continue
        source = re.match(r"^\s+(-?\d+)\s+(\S+)\s", line)
        if source and version is not None:
            host = urllib.parse.urlsplit(source.group(2)).hostname or source.group(2)
            packages[current]["versions"][version][1].append(host)
    return packages


def candidate_sites(entry):
    if entry is None or entry["candidate"] is None:
        return []
    return entry["versions"].get(entry["candidate"], [None, []])[1]


def uri_problems(text):
    problems = []
    for line in text.splitlines():
        match = re.match(r"^'([^']+)'\s+(\S+)\s+\d+", line.strip())
        if not match:
            continue
        host = (urllib.parse.urlsplit(match.group(1)).hostname or "").lower()
        package = urllib.parse.unquote(match.group(2)).split("_", 1)[0]
        if host not in ApprovedSites:
            problems.append("unapproved host %s: %s" % (host, match.group(2)))
        elif host == KaliSite and package not in KaliAllowlist:
            problems.append("un-allowlisted Kali download: " + match.group(2))
        elif host == LibreWolfSite and package != "librewolf":
            problems.append("un-allowlisted LibreWolf download: " + match.group(2))
    return problems


def parse_key_values(text):
    values = {}
    for line in text.splitlines():
        key, sep, value = line.partition("=")
        if sep and re.match(r"^[a-z_]+$", key.strip()):
            values[key.strip()] = value.strip()
    return values


def dumpcap_checks(vm, results, prefix, password, expect_caps=True):
    status, output = guest(vm, results, prefix + "_dumpcap", root_command(
        "stat -c '%a %U %G' /usr/bin/dumpcap; getcap /usr/bin/dumpcap; echo override=$(dpkg-statoverride --list "
        "/usr/bin/dumpcap); debconf-show wireshark-common | grep install-setuid", password))
    lines = [line.strip() for line in output.splitlines() if line.strip()]
    mode_line = lines[0] if lines else ""
    mode = int(mode_line.split()[0], 8) if re.match(r"^[0-7]{3,4} ", mode_line) else None
    results.check(prefix + "_dumpcap_no_setuid", status == 0 and mode is not None and not mode & 0o6000,
                  "stat: %r" % mode_line)
    results.check(prefix + "_dumpcap_group_restricted",
                  mode is not None and mode_line.split()[1:] == ["root", "wireshark"] and not mode & 0o001,
                  "stat: %r" % mode_line)
    caps = [line for line in lines if "cap_net" in line]
    caps_ok = bool(caps) and "cap_net_admin,cap_net_raw=eip" in caps[0].replace(" ", "")
    if expect_caps:
        results.check(prefix + "_dumpcap_capabilities", caps_ok, "getcap: %r" % caps)
    results.check(prefix + "_dumpcap_override", "override=root root 750 /usr/bin/dumpcap" in output,
                  [line for line in lines if line.startswith("override=")])
    return mode, caps_ok


def capture_checks(vm, results, prefix, password, expect_capture=True):
    with open(CaptureProbePath, "rb") as handle:
        vm.serial.put_file("/tmp/cham3_capture_probe.sh", handle.read(), "0755")
    status, output = guest(vm, results, prefix + "_capture", root_command("bash /tmp/cham3_capture_probe.sh", password),
                           timeout=240)
    values = parse_key_values(output)
    member_rc, member_bytes = values.get("member_rc"), int(values.get("member_bytes", "0") or 0)
    output_path_failure = "could not be opened" in values.get("member_log", "")
    results.check(prefix + "_capture_harness_output_path_ok", status == 0 and not output_path_failure,
                  values.get("member_log", output[-400:]))
    if expect_capture:
        results.check(prefix + "_member_captures", member_rc == "0" and member_bytes > 100,
                      "rc=%s bytes=%s log=%s" % (member_rc, member_bytes, values.get("member_log")))
    else:
        results.check(prefix + "_member_cannot_capture_fail_closed", member_rc not in (None, "0") and member_bytes == 0,
                      "rc=%s bytes=%s log=%s" % (member_rc, member_bytes, values.get("member_log")))
    results.check(prefix + "_nonmember_denied",
                  values.get("nonmember_rc") == "126" and "ermission denied" in values.get("nonmember_log", ""),
                  "rc=%s log=%s" % (values.get("nonmember_rc"), values.get("nonmember_log")))


def common_system_checks(vm, results, prefix, password):
    guest(vm, results, prefix + "_identity", "uname -a; cat /proc/cmdline; grep -E '^(PRETTY_NAME|VERSION_CODENAME)=' "
          "/etc/os-release")
    status, output = guest(vm, results, prefix + "_packages", "dpkg-query -W -f='${Package} ${Version} ${Status}\\n' "
                           "cham3-desktop cham3-full cham3-archive-keyring wireshark-common torbrowser-launcher "
                           + " ".join(ForeignRoles) + " 2>&1")
    installed = {line.split()[0] for line in output.splitlines() if line.endswith("install ok installed")}
    needed = {"cham3-desktop", "cham3-full", "cham3-archive-keyring", "torbrowser-launcher"} | set(ForeignRoles)
    results.check(prefix + "_suite_installed", needed <= installed, sorted(needed - installed))
    status, output = guest(vm, results, prefix + "_firmware",
                           "dpkg-query -W -f='${Package} ${Status}\\n' 'firmware-*' amd64-microcode intel-microcode "
                           "alsa-topology-conf wireless-regdb indi-dsi libsbig4t64 libfishcamp1t64 atmel-firmware "
                           "bluez-firmware dahdi-firmware-nonfree 2>/dev/null | grep 'install ok installed'")
    names = {line.split()[0] for line in output.splitlines() if line.strip()}
    results.check(prefix + "_no_b43_installer", not (set(FirmwareB43) & names), sorted(set(FirmwareB43) & names))
    results.check(prefix + "_firmware_set_installed", len(names) >= ReviewedFirmwareCount and "firmware-brcm80211"
                  in names, "%d installed firmware packages" % len(names))
    status, output = guest(vm, results, prefix + "_apt_trust",
                           "ls -1 /etc/apt/trusted.gpg.d/ 2>/dev/null; ls -1 /usr/share/keyrings/ | grep cham3; "
                           "grep -rh '^deb' /etc/apt/sources.list /etc/apt/sources.list.d/ 2>/dev/null")
    results.check(prefix + "_no_trusted_yes", status == 0 and "trusted=yes" not in output, "")
    results.check(prefix + "_keys_only_signed_by",
                  "cham3_kali_archive.gpg" in output and "cham3_librewolf.gpg" in output
                  and not re.search(r"(?m)^(cham3|kali|librewolf)\S*\.(gpg|asc)$",
                                    output.split("cham3_kali_archive.gpg")[0]), "")
    apt_policy_checks(vm, results, prefix)


def apt_policy_checks(vm, results, prefix):
    status, output = guest(vm, results, prefix + "_apt_policy",
                           "apt-cache policy " + " ".join(ForeignRoles + UnlistedKaliProbes + ("libc6",)))
    policy = parse_policy(output)
    escaped = {name: candidate_sites(policy.get(name)) for name in UnlistedKaliProbes
               if policy.get(name) and policy[name]["candidate"]}
    results.check(prefix + "_unlisted_kali_denied", status == 0 and not escaped, escaped)
    roles = {name: candidate_sites(policy.get(name)) for name in ForeignRoles}
    results.check(prefix + "_foreign_roles_have_candidates", all(roles.values()), roles)
    results.check(prefix + "_debian_base_from_debian",
                  bool(policy.get("libc6")) and all(site not in (KaliSite, LibreWolfSite)
                                                     for site in candidate_sites(policy.get("libc6"))),
                  candidate_sites(policy.get("libc6")))


# ---------------------------------------------------------------------------------
# Graphical checks


def wait_for_session(vm, results, user, timeout):
    deadline = time.time() + timeout
    info = {}
    while time.time() < deadline:
        status, output = vm.serial.run(
            "for s in $(loginctl list-sessions --no-legend | awk '{print $1}'); do "
            "loginctl show-session \"$s\" -p Name -p Type -p Class -p Active -p Desktop | tr '\\n' ' '; echo; done; "
            "pgrep -u %s -x plasmashell >/dev/null && echo plasmashell=running" % user, 60)
        sessions = []
        for line in output.splitlines():
            fields = dict(item.split("=", 1) for item in line.split() if "=" in item)
            if fields.get("Name") == user and fields.get("Class") == "user":
                sessions.append(fields)
        graphical = [fields for fields in sessions if fields.get("Type") in ("wayland", "x11")]
        info = {"sessions": sessions, "plasmashell": "plasmashell=running" in output}
        if graphical and info["plasmashell"]:
            info["graphical"] = graphical
            return info
        time.sleep(10)
    return info


def panel_state(vm, results, prefix):
    with open(PanelScriptPath, "rb") as handle:
        vm.serial.put_file("/tmp/cham3_panel_state.js", handle.read())
    status, output = guest(vm, results, prefix + "_panel_state",
                           "busctl --user --json=short call org.kde.plasmashell /PlasmaShell org.kde.PlasmaShell "
                           "evaluateScript s \"$(cat /tmp/cham3_panel_state.js)\"")
    try:
        reply = json.loads(output.strip().splitlines()[-1])
        state = json.loads(reply["data"][0])
    except (ValueError, KeyError, IndexError):
        state = None
    return status, state


def panel_checks(results, prefix, state):
    if state is None:
        results.check(prefix + "_panel_state_read", False, "plasmashell did not return its panel state")
        return None
    results.check(prefix + "_panel_state_read", True, "")
    panels = state.get("panels", [])
    right = [panel for panel in panels if panel.get("location") == "right"]
    results.check(prefix + "_panel_on_right_edge", len(right) == 1, [panel.get("location") for panel in panels])
    if not right:
        return None
    panel = right[0]
    results.check(prefix + "_panel_vertical", panel.get("form_factor") == "vertical", panel.get("form_factor"))
    thickness = panel.get("height")
    results.check(prefix + "_panel_thickness", isinstance(thickness, (int, float))
                  and PanelThicknessRange[0] <= thickness <= PanelThicknessRange[1], thickness)
    order = tuple(widget.get("type") for widget in panel.get("widgets", []))
    results.check(prefix + "_panel_component_order", order == PanelWidgetOrder, list(order))
    images = [desktop.get("image") or "" for desktop in state.get("desktops", [])]
    results.check(prefix + "_cham3_wallpaper", bool(images) and all("Cham3" in image for image in images), images)
    return panel


def widget_center(panel, state, widget_type, screen_size):
    width, height = screen_size
    thickness = panel.get("height") or 60
    for widget in panel.get("widgets", []):
        geometry = widget.get("geometry")
        if widget.get("type") == widget_type and geometry and geometry.get("height"):
            x = width - thickness + geometry["x"] + geometry["width"] / 2.0
            y = (panel.get("offset") or 0) + geometry["y"] + geometry["height"] / 2.0
            if width - thickness <= x <= width and 0 <= y <= height:
                return x, y
    return None


def graphical_checks(vm, results, prefix, user, session_timeout):
    info = wait_for_session(vm, results, user, session_timeout)
    results.facts[prefix + "_session"] = info
    graphical = info.get("graphical", [])
    results.check(prefix + "_plasma_session_running", bool(graphical), info)
    results.check(prefix + "_session_is_wayland", bool(graphical) and all(fields.get("Type") == "wayland"
                                                                          for fields in graphical),
                  [fields.get("Type") for fields in graphical])
    if not graphical:
        vm.screenshot("no_session")
        return
    status, output = guest(vm, results, prefix + "_session_processes",
                           "pgrep -a -u %s 'kwin_wayland|plasmashell|Xwayland' ; tr '\\0' '\\n' < /proc/$(pgrep -u %s "
                           "-x plasmashell | head -1)/environ | grep -E '^(XDG_SESSION_TYPE|WAYLAND_DISPLAY|"
                           "QT_QPA_PLATFORM)='" % (user, user))
    results.check(prefix + "_kwin_wayland_compositor", "kwin_wayland" in output and "XDG_SESSION_TYPE=wayland" in output,
                  output[-600:])
    guest(vm, results, prefix + "_branding_config",
          "grep -hE '^(LookAndFeelPackage|ColorScheme|widgetStyle)=' /etc/xdg/kdeglobals ~/.config/kdeglobals 2>/dev/null")
    look = results.facts[prefix + "_branding_config"]["output"]
    results.check(prefix + "_cham3_look_and_feel", "LookAndFeelPackage=org.cham3.desktop" in look, look[-300:])
    time.sleep(30)
    status, state = panel_state(vm, results, prefix)
    panel = panel_checks(results, prefix, state)
    desktop_shot = vm.screenshot("desktop")
    screen_size = png_size(desktop_shot)
    results.facts[prefix + "_screen_size"] = screen_size
    if panel is None:
        return
    launcher_checks(vm, results, prefix, user, panel, state, screen_size)
    inert_checks(vm, results, prefix, panel, state, screen_size)


def launcher_checks(vm, results, prefix, user, panel, state, screen_size):
    width, height = screen_size
    center = widget_center(panel, state, "org.cham3.user_launcher", screen_size) or (width - 30, 30)
    vm.move(width / 2, height / 2, width, height)
    vm.click(center[0], center[1], width, height)
    time.sleep(4)
    opened = vm.screenshot("launcher_categories")
    text = ocr(opened, results.out_dir, prefix + "_launcher_categories")
    if text is None:
        results.blocked(prefix + "_launcher_categories_rendered", "tesseract or python3-pil is absent")
    else:
        found = sorted({name for name in MenuCategories if name.lower() in text.lower()})
        results.check(prefix + "_launcher_categories_rendered", len(found) >= 3, found)
        results.check(prefix + "_launcher_key_id_unavailable", "key_id" in text.replace(" ", "")
                      and "unavailable" in text.lower(), "")
    vm.type_text("wireshark")
    time.sleep(4)
    searched = vm.screenshot("launcher_search")
    text = ocr(searched, results.out_dir, prefix + "_launcher_search")
    if text is None:
        results.blocked(prefix + "_launcher_search_result", "tesseract or python3-pil is absent")
    else:
        results.check(prefix + "_launcher_search_result", "wireshark" in text.lower(), "")
    vm.key("ret")
    launched = False
    for _ in range(30):
        status, output = vm.serial.run("pgrep -u %s -x wireshark" % user, 30)
        if status == 0:
            launched = True
            break
        time.sleep(3)
    time.sleep(10)
    vm.screenshot("launched_wireshark")
    results.check(prefix + "_launcher_real_launch", launched, "wireshark process started from the launcher search")
    vm.serial.run("pkill -u %s -x wireshark; sleep 2" % user, 30)
    vm.key("esc")


def inert_checks(vm, results, prefix, panel, state, screen_size):
    width, height = screen_size
    for widget_type in Cham3Widgets[1:]:
        center = widget_center(panel, state, widget_type, screen_size)
        short = widget_type.split(".")[-1]
        if center is None:
            results.check(prefix + "_inert_" + short, False, "widget geometry not reported")
            continue
        vm.key("esc")
        time.sleep(1)
        baseline = ocr(vm.screenshot("before_" + short), results.out_dir, prefix + "_before_" + short)
        vm.click(center[0], center[1], width, height)
        time.sleep(3)
        shot = vm.screenshot("widget_" + short)
        text = ocr(shot, results.out_dir, prefix + "_widget_" + short)
        vm.key("esc")
        time.sleep(1)
        if text is None:
            results.blocked(prefix + "_inert_" + short, "tesseract or python3-pil is absent")
            continue
        new_state = bool(InertWords.search(text)) and not InertWords.search(baseline or "")
        if widget_type in InertWidgets:
            results.check(prefix + "_inert_" + short, new_state, "OCR of the opened widget")
        else:
            results.facts[prefix + "_inert_" + short] = {"ocr_state_word": bool(InertWords.search(text))}


# ---------------------------------------------------------------------------------
# Phases


def boot_iso_menu(vm, results, timeout):
    time.sleep(8)
    vm.screenshot("boot_menu")
    deadline = time.time() + timeout
    while time.time() < deadline:
        if re.search(r"Linux version|Booting|Loading", vm.serial.text()):
            return True
        vm.key("ret")
        time.sleep(5)
    return False


def phase_live(args, results):
    vm = vm_t(args.out, "live", args.accel, args.cpus, args.memory, cdrom=args.iso)
    try:
        results.check("live_boot_menu_to_kernel", boot_iso_menu(vm, results, 300), "")
        logged_in = login(vm, LiveUser, LivePassword, args.boot_timeout)
        results.check("live_serial_login", logged_in, "")
        if not logged_in:
            vm.screenshot("no_login")
            return
        vm.serial.run(root_command("dmesg -n 1", None), 30)
        common_system_checks(vm, results, "live", None)
        dumpcap_checks(vm, results, "live", None)
        capture_checks(vm, results, "live", None)
        graphical_checks(vm, results, "live", LiveUser, args.session_timeout)
    finally:
        vm.stop()


def initrd_with_preseed(iso, out_dir):
    work = tempfile.mkdtemp(prefix="cham3-di-")
    listing = subprocess.run(["xorriso", "-indev", iso, "-find", "/install", "-type", "f"], capture_output=True,
                             text=True, check=False)
    members = re.findall(r"'(/install/[^']+)'", listing.stdout)
    kernel_member = "/install/vmlinuz" if "/install/vmlinuz" in members else None
    initrd_member = "/install/initrd.gz" if "/install/initrd.gz" in members else None
    if kernel_member is None or initrd_member is None:
        raise RuntimeError("installer kernel/initrd not found in the ISO: %r" % members[:40])
    kernel = os.path.join(work, "vmlinuz")
    initrd = os.path.join(work, "initrd.gz")
    completed = subprocess.run(["xorriso", "-osirrox", "on", "-indev", iso, "-extract", kernel_member, kernel,
                                "-extract", initrd_member, initrd], capture_output=True, text=True, check=False)
    if completed.returncode != 0:
        raise RuntimeError("cannot extract the installer: " + completed.stderr)
    seed_dir = os.path.join(work, "seed")
    os.makedirs(seed_dir)
    shutil.copy(PreseedPath, os.path.join(seed_dir, "preseed.cfg"))
    archive = subprocess.run("cd %s && echo preseed.cfg | cpio -o -H newc --quiet | gzip -9" % shlex.quote(seed_dir),
                             shell=True, capture_output=True, check=False)
    if archive.returncode != 0:
        raise RuntimeError("cannot build the preseed archive")
    combined = os.path.join(work, "initrd-preseed.gz")
    with open(combined, "wb") as handle:
        with open(initrd, "rb") as source:
            shutil.copyfileobj(source, handle)
        handle.write(archive.stdout)
    return kernel, combined, {"kernel_member": kernel_member, "initrd_member": initrd_member,
                              "preseed_sha256": sha256_file(PreseedPath)}


def phase_install(args, results):
    kernel, initrd, info = initrd_with_preseed(args.iso, args.out)
    results.facts["installer"] = info
    if os.path.exists(args.disk):
        os.unlink(args.disk)
    subprocess.run(["qemu-img", "create", "-f", "qcow2", args.disk, args.disk_size], check=True,
                   capture_output=True)
    append = "priority=critical DEBIAN_FRONTEND=text console=ttyS0,115200n8 --- console=ttyS0,115200n8"
    vm = vm_t(args.out, "install", args.accel, args.cpus, args.memory, cdrom=args.iso, disk=args.disk, network=True,
              kernel=kernel, initrd=initrd, append=append, no_reboot=True)
    started = time.time()
    try:
        deadline = started + args.install_timeout
        next_shot = started + 120
        while vm.alive() and time.time() < deadline:
            if time.time() >= next_shot:
                vm.screenshot("progress")
                next_shot = time.time() + 300
            time.sleep(5)
        finished = not vm.alive()
        if not finished:
            vm.screenshot("timeout")
        log = vm.serial.text()
        results.facts["install_seconds"] = round(time.time() - started)
        errors = re.findall(r"(?m)^.*(Installation step failed|An installation step failed|\[!!\].*|error.*)$", log)
        results.check("install_completed_and_powered_off", finished and vm.process.returncode == 0,
                      {"seconds": round(time.time() - started), "qemu_rc": vm.process.returncode})
        results.facts["install_error_lines"] = errors[-30:]
    finally:
        vm.stop()
    results.facts["disk_bytes"] = os.path.getsize(args.disk)


def phase_installed(args, results):
    user, password = preseed_account()
    vm = vm_t(args.out, "installed", args.accel, args.cpus, args.memory, disk=args.disk, network=True)
    try:
        logged_in = login(vm, user, password, args.boot_timeout)
        results.check("installed_boot_serial_login", logged_in, "")
        if not logged_in:
            vm.screenshot("no_login")
            return
        vm.serial.run(root_command("dmesg -n 1", password), 30)
        guest(vm, results, "installed_live_packages_removed", "dpkg-query -W -f='${Package} ${Status}\\n' live-boot "
              "live-config 2>&1 | grep 'install ok installed' || echo none")
        results.check("installed_live_packages_removed",
                      results.facts["installed_live_packages_removed"]["output"].strip().endswith("none"), "")
        common_system_checks(vm, results, "installed", password)
        dumpcap_checks(vm, results, "installed", password)
        capture_checks(vm, results, "installed", password)
        installed_apt_gates(vm, results, password)
        installed_dumpcap_lifecycle(vm, results, password)
        vm.screenshot("sddm_greeter")
        vm.type_text(password)
        vm.key("ret")
        graphical_checks(vm, results, "installed", user, args.session_timeout)
    finally:
        vm.stop()


def installed_apt_gates(vm, results, password):
    status, output = guest(vm, results, "installed_apt_update",
                           root_command("apt-get -o APT::Update::Error-Mode=any update", password), timeout=1800)
    results.check("installed_apt_update_authenticated", status == 0 and not re.search(
        r"GPG error|NO_PUBKEY|not signed|BADSIG", output), output[-800:])
    apt_policy_checks(vm, results, "installed_after_update")
    status, output = guest(vm, results, "installed_upgrade_uris",
                           root_command("apt-get --print-uris -qq -y dist-upgrade", password), timeout=900)
    problems = uri_problems(output)
    results.check("installed_upgrade_origin_by_host", status == 0 and not problems, problems[:20])
    guest(vm, results, "installed_upgrade_simulation", root_command("apt-get -s -y dist-upgrade", password), timeout=900)
    status, output = guest(vm, results, "installed_unlisted_install",
                           root_command("apt-get -s -y install ghidra-data", password), timeout=300)
    results.check("installed_unlisted_kali_not_installable", status != 0, output[-400:])


def installed_dumpcap_lifecycle(vm, results, password):
    steps = (
        ("reinstall", "apt-get install -y --reinstall wireshark-common", True),
        ("reconfigure_upstream", "dpkg-reconfigure -f noninteractive wireshark-common", True),
        ("setcap_missing", "mv /usr/sbin/setcap /usr/sbin/setcap.cham3-hidden && "
                           "apt-get install -y --reinstall wireshark-common; rc=$?; "
                           "mv /usr/sbin/setcap.cham3-hidden /usr/sbin/setcap; exit $rc", False),
    )
    for label, command, expect_caps in steps:
        status, _ = guest(vm, results, "installed_lifecycle_" + label, root_command(command, password), timeout=900)
        results.check("installed_lifecycle_%s_ran" % label, status == 0, "")
        mode, caps = dumpcap_checks(vm, results, "installed_" + label, password, expect_caps=expect_caps)
        if not expect_caps:
            results.check("installed_%s_fails_closed" % label, not caps, "capabilities removed: %s" % (not caps))
            capture_checks(vm, results, "installed_" + label, password, expect_capture=False)
    status, _ = guest(vm, results, "installed_lifecycle_recovery",
                      root_command("dpkg-reconfigure -f noninteractive cham3-desktop", password), timeout=600)
    results.check("installed_lifecycle_recovery_ran", status == 0, "")
    dumpcap_checks(vm, results, "installed_recovered", password)
    capture_checks(vm, results, "installed_recovered", password)


def preseed_account():
    user = password = None
    with open(PreseedPath, "r", encoding="utf-8") as handle:
        for line in handle:
            fields = line.split()
            if len(fields) >= 4 and fields[1] == "passwd/username":
                user = fields[3]
            if len(fields) >= 4 and fields[1] == "passwd/user-password":
                password = fields[3]
    if not user or not password:
        raise RuntimeError("the test preseed defines no account")
    return user, password


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("phase", choices=("live", "install", "installed"))
    parser.add_argument("--iso")
    parser.add_argument("--disk")
    parser.add_argument("--disk-size", default="48G")
    parser.add_argument("--out", required=True)
    parser.add_argument("--accel", choices=("auto", "kvm", "tcg"), default="auto")
    parser.add_argument("--cpus", type=int, default=4)
    parser.add_argument("--memory", type=int, default=6144)
    parser.add_argument("--boot-timeout", type=int, default=1200)
    parser.add_argument("--session-timeout", type=int, default=900)
    parser.add_argument("--install-timeout", type=int, default=7200)
    args = parser.parse_args()
    os.makedirs(args.out, exist_ok=True)
    results = results_t(args.out, args.phase)
    for tool in ("qemu-system-x86_64", "qemu-img", "xorriso", "cpio", "gzip"):
        if shutil.which(tool) is None:
            results.blocked("environment", "missing tool: " + tool)
            results.save()
            return 2
    accel = choose_accel(args.accel)
    if accel is None:
        results.blocked("environment", "kvm requested but /dev/kvm is not usable")
        results.save()
        return 2
    args.accel = accel
    results.facts["accel"] = accel
    results.facts["qemu_version"] = subprocess.run(["qemu-system-x86_64", "--version"], capture_output=True,
                                                   text=True).stdout.splitlines()[0]
    if args.phase in ("live", "install"):
        if not args.iso or not os.path.isfile(args.iso):
            results.blocked("environment", "ISO not found: %r" % args.iso)
            results.save()
            return 2
        results.facts["iso_sha256"] = sha256_file(args.iso)
    if args.phase in ("install", "installed") and not args.disk:
        parser.error("--disk is required for " + args.phase)
    try:
        {"live": phase_live, "install": phase_install, "installed": phase_installed}[args.phase](args, results)
    except Exception as error:  # recorded as a FAIL with the reason, never swallowed
        results.check(args.phase + "_harness", False, "%s: %s" % (type(error).__name__, error))
    finally:
        results.save()
    status = results.status()
    print("M1 VM %s: %s" % (args.phase, {0: "PASS", 1: "FAIL", 2: "BLOCKED"}[status]), flush=True)
    return status


if __name__ == "__main__":
    sys.exit(main())
