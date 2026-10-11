#!/usr/bin/env python3
"""Boot an M0 live ISO in a throwaway QEMU VM and classify the serial console.

Purpose: disposable-VM boot check (python3 stdlib only, no root, no host changes).
The ISO is attached read-only as a CD-ROM; no disk and no network are attached.
The firmware is QEMU's default SeaBIOS (BIOS boot path only, no UEFI).

Limits: a PASS means only that the image booted to a login prompt (or an
autologin shell) on the serial console in this VM. It says nothing about
hardware compatibility, UEFI boot, or any cham3 security property. No credentials
are ever typed. The live-build BIOS boot menu (syslinux) has no timeout, so this
script presses Enter through the QEMU monitor (sendkey) until the kernel starts
writing to the serial console; that is recorded in matched_markers.

Exit status: 0 PASS, 1 FAIL, 2 BLOCKED (for example QEMU missing).
"""

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time

FailMarkers = [
    ("kernel_panic", re.compile(r"Kernel panic")),
    ("initramfs_shell", re.compile(r"\(initramfs\)")),
    ("live_boot_no_medium", re.compile(r"Unable to find a medium")),
    ("emergency_mode", re.compile(r"You are in emergency mode")),
]
LoginPrompt = re.compile(r"(?m)^[A-Za-z0-9][A-Za-z0-9_.\-]* login: ?")
ShellPrompt = re.compile(r"(?m)^[A-Za-z0-9_.\-]+@[A-Za-z0-9_.\-]+:[^\r\n]*[$#] ?$")
AnsiEscape = re.compile(r"\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b[@-_]")


def classify_boot_log(log_text):
    """Classify serial console text.

    Returns a dict with result ("PASS", "FAIL" or "PENDING"), reason, and
    markers (list of matched marker names). Failure markers win over a prompt.
    """
    text = AnsiEscape.sub("", log_text).replace("\r", "")
    for name, pattern in FailMarkers:
        if pattern.search(text):
            return {"result": "FAIL", "reason": "failure marker seen: " + name, "markers": [name]}
    if LoginPrompt.search(text):
        return {"result": "PASS", "reason": "login prompt reached on serial console", "markers": ["login_prompt"]}
    if ShellPrompt.search(text):
        return {"result": "PASS", "reason": "autologin shell prompt on serial console", "markers": ["shell_prompt"]}
    return {"result": "PENDING", "reason": "no login prompt or failure marker yet", "markers": []}


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def choose_accel(requested):
    if requested == "tcg":
        return "tcg", None
    kvm_usable = (
        sys.platform.startswith("linux")
        and platform.machine() in ("x86_64", "AMD64")
        and os.access("/dev/kvm", os.R_OK | os.W_OK)
    )
    if requested == "kvm":
        if not kvm_usable:
            return None, "kvm requested but /dev/kvm is not usable on this host"
        return "kvm", None
    return ("kvm" if kvm_usable else "tcg"), None


class serial_capture_t:
    """Reads QEMU stdout (the serial console) into memory and a log file."""

    def __init__(self, stream, log_path):
        self.stream = stream
        self.log_path = log_path
        self.buffer = bytearray()
        self.lock = threading.Lock()
        self.thread = threading.Thread(target=self.run, daemon=True)

    def start(self):
        self.thread.start()

    def run(self):
        with open(self.log_path, "wb") as log:
            while True:
                chunk = os.read(self.stream.fileno(), 4096)
                if not chunk:
                    return
                log.write(chunk)
                log.flush()
                with self.lock:
                    self.buffer.extend(chunk)

    def text(self):
        with self.lock:
            return self.buffer.decode("utf-8", errors="replace")

    def size(self):
        with self.lock:
            return len(self.buffer)


def monitor_command(sock_path, command, timeout=5.0):
    """Send one command to the QEMU human monitor over its unix socket."""
    client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    client.settimeout(timeout)
    try:
        client.connect(sock_path)
        time.sleep(0.2)
        client.recv(65536)
        client.sendall((command + "\n").encode("ascii"))
        time.sleep(0.2)
        return True
    except OSError:
        return False
    finally:
        client.close()


def qemu_version(qemu_path):
    try:
        completed = subprocess.run([qemu_path, "--version"], stdout=subprocess.PIPE, text=True, check=False)
        return completed.stdout.splitlines()[0] if completed.stdout else None
    except OSError:
        return None


def runtime_checks_for(result_info, capture, process):
    """Run read-only commands only if a shell prompt is present without logging in."""
    if "shell_prompt" not in result_info["markers"]:
        return {
            "performed": False,
            "reason": "login prompt requires credentials; none are typed by this script",
        }
    end_token = "CHAM3_CHK_END_"
    command = (
        "dpkg-query -W -f='${Package} ${Version}\\n' cham3-base; cat /etc/os-release; "
        "echo " + end_token + "$((1+1))\n"
    )
    start = capture.size()
    try:
        process.stdin.write(command.encode("ascii"))
        process.stdin.flush()
    except OSError:
        return {"performed": False, "reason": "could not write to serial console"}
    deadline = time.time() + 60
    while time.time() < deadline:
        new_text = capture.text()[start:]
        if end_token + "2" in new_text:
            return {"performed": True, "output": AnsiEscape.sub("", new_text).replace("\r", "")}
        time.sleep(1)
    return {"performed": False, "reason": "timeout waiting for command output"}


def main():
    parser = argparse.ArgumentParser(description="Boot a cham3 M0 ISO in a throwaway QEMU VM.")
    parser.add_argument("--iso", required=True)
    parser.add_argument("--out-dir", default=None)
    parser.add_argument("--timeout-seconds", type=int, default=900)
    parser.add_argument("--memory-mib", type=int, default=1024)
    parser.add_argument("--cpus", type=int, default=2)
    parser.add_argument("--accel", choices=["auto", "kvm", "tcg"], default="auto")
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    args = parser.parse_args()

    out_root = os.environ.get("CHAM3_OUT_DIR") or os.path.join(
        os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "out"
    )
    out_dir = args.out_dir or os.path.join(out_root, "boot_test")
    os.makedirs(out_dir, exist_ok=True)
    serial_log = os.path.join(out_dir, "serial.log")
    result_path = os.path.join(out_dir, "result.json")

    result = {
        "result": "BLOCKED",
        "reason": "",
        "iso_path": os.path.abspath(args.iso),
        "iso_sha256": None,
        "qemu_binary": None,
        "qemu_version": None,
        "accel": None,
        "machine": "pc",
        "memory_mib": args.memory_mib,
        "cpus": args.cpus,
        "firmware": "seabios (QEMU default BIOS)",
        "elapsed_seconds": 0.0,
        "matched_markers": [],
        "runtime_checks": {"performed": False, "reason": "not reached"},
        "host_platform": platform.platform() + " " + platform.machine(),
    }

    def finish(code, outcome, reason):
        result["result"] = outcome
        result["reason"] = reason
        with open(result_path, "w", encoding="utf-8") as handle:
            json.dump(result, handle, indent=2, sort_keys=True)
            handle.write("\n")
        print("boot_test: {} - {}".format(outcome, reason))
        return code

    if not os.path.isfile(args.iso):
        return finish(2, "BLOCKED", "ISO not found: " + args.iso)
    qemu_path = shutil.which(args.qemu) if os.sep not in args.qemu else (args.qemu if os.access(args.qemu, os.X_OK) else None)
    if not qemu_path:
        return finish(2, "BLOCKED", "QEMU binary not found: " + args.qemu)
    result["qemu_binary"] = qemu_path
    result["qemu_version"] = qemu_version(qemu_path)
    accel, accel_problem = choose_accel(args.accel)
    if accel is None:
        return finish(2, "BLOCKED", accel_problem)
    result["accel"] = accel
    result["iso_sha256"] = sha256_file(args.iso)

    scratch = tempfile.mkdtemp(prefix="cham3_boot_")
    monitor_path = os.path.join(scratch, "monitor.sock")
    command = [
        qemu_path,
        "-machine", "pc,accel=" + accel,
        "-cpu", "host" if accel == "kvm" else "max",
        "-m", str(args.memory_mib),
        "-smp", str(args.cpus),
        "-drive", "file={},media=cdrom,readonly=on,format=raw,if=ide".format(os.path.abspath(args.iso)),
        "-boot", "order=d",
        "-display", "none",
        "-nic", "none",
        "-no-reboot",
        "-serial", "stdio",
        "-monitor", "unix:{},server=on,wait=off".format(monitor_path),
    ]
    start_time = time.time()
    process = None
    capture = None
    sent_enter = 0
    try:
        with open(os.path.join(out_dir, "qemu_stderr.log"), "wb") as stderr_log:
            process = subprocess.Popen(
                command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=stderr_log, bufsize=0
            )
            capture = serial_capture_t(process.stdout, serial_log)
            capture.start()
            last_enter = 0.0
            outcome = None
            while True:
                elapsed = time.time() - start_time
                if process.poll() is not None:
                    outcome = classify_boot_log(capture.text())
                    if outcome["result"] != "PASS":
                        outcome = {
                            "result": "FAIL",
                            "reason": "QEMU exited early (status {}) without a login prompt".format(process.returncode),
                            "markers": outcome["markers"],
                        }
                    break
                outcome = classify_boot_log(capture.text())
                if outcome["result"] != "PENDING":
                    break
                if elapsed > args.timeout_seconds:
                    outcome = {
                        "result": "FAIL",
                        "reason": "timeout after {} s without login prompt".format(args.timeout_seconds),
                        "markers": [],
                    }
                    break
                # Syslinux menu has no timeout: press Enter until the kernel writes to serial.
                if capture.size() == 0 and elapsed > 15 and time.time() - last_enter >= 10:
                    if monitor_command(monitor_path, "sendkey ret"):
                        sent_enter += 1
                    last_enter = time.time()
                time.sleep(1)

            markers = list(outcome["markers"])
            if sent_enter:
                markers.append("sendkey_ret_x{}".format(sent_enter))
            result["matched_markers"] = markers
            if outcome["result"] == "PASS":
                time.sleep(2)
                result["runtime_checks"] = runtime_checks_for(outcome, capture, process)
            result["elapsed_seconds"] = round(time.time() - start_time, 1)
            code = {"PASS": 0, "FAIL": 1}[outcome["result"]]
            return finish(code, outcome["result"], outcome["reason"])
    finally:
        if process is not None and process.poll() is None:
            monitor_command(monitor_path, "quit")
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        shutil.rmtree(scratch, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
