#!/usr/bin/env python3
"""Write provenance.json for an M0 image build.

Purpose: record material build inputs and tool versions (python3 stdlib only).
Limits: this is a record of what was observed, not a reproducibility claim.
Bit-for-bit reproducibility is NOT claimed. Package versions are read from the
build host's dpkg database; apt Release files are read from the live-build
chroot when still present.
"""

import argparse
import datetime
import hashlib
import json
import os
import platform
import re
import subprocess
import sys

ProvenanceFormat = "cham3_m0_provenance_v1"
ToolPackages = [
    "live-build",
    "debootstrap",
    "dpkg-dev",
    "debhelper",
    "lintian",
    "xorriso",
    "squashfs-tools",
    "debian-archive-keyring",
    "qemu-system-x86",
]


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def run_text(command, cwd=None):
    try:
        completed = subprocess.run(
            command, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False, text=True
        )
    except OSError:
        return None
    if completed.returncode != 0:
        return None
    return completed.stdout.strip()


def read_os_release():
    values = {}
    try:
        with open("/etc/os-release", encoding="utf-8") as handle:
            for line in handle:
                line = line.strip()
                if "=" in line and not line.startswith("#"):
                    key, _, value = line.partition("=")
                    values[key] = value.strip('"')
    except OSError:
        pass
    return values


def in_container():
    if os.path.exists("/.dockerenv") or os.path.exists("/run/.containerenv"):
        return True
    try:
        with open("/proc/1/cgroup", encoding="utf-8") as handle:
            text = handle.read()
        return any(word in text for word in ("docker", "kubepods", "containerd", "lxc"))
    except OSError:
        return False


def package_version(package):
    return run_text(["dpkg-query", "-W", "-f=${Version}", package])


def parse_shell_assignments(path):
    values = {}
    try:
        with open(path, encoding="utf-8") as handle:
            for line in handle:
                match = re.match(r'^([A-Z_]+)="(.*)"\s*$', line)
                if match:
                    values[match.group(1)] = match.group(2)
    except OSError:
        pass
    return values


def lb_config_command(auto_config_path):
    """Return the logical 'lb config' command line spelled in auto/config."""
    with open(auto_config_path, encoding="utf-8") as handle:
        text = handle.read()
    text = text.replace("\\\n", " ")
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("lb config"):
            command = re.sub(r"\s+", " ", stripped)
            # Drop the pass-through of extra arguments; it is not part of the fixed command.
            return command.replace(' "${@}"', "")
    return None


def apt_release_files(lists_dir):
    if not lists_dir or not os.path.isdir(lists_dir):
        return None, "apt lists directory not found: {}".format(lists_dir)
    entries = []
    for name in sorted(os.listdir(lists_dir)):
        if not name.endswith(("InRelease", "_Release")):
            continue
        path = os.path.join(lists_dir, name)
        date_value = None
        with open(path, encoding="utf-8", errors="replace") as handle:
            for line in handle:
                if line.startswith("Date:"):
                    date_value = line.split(":", 1)[1].strip()
                    break
        entries.append({"file": name, "date": date_value, "sha256": sha256_file(path)})
    if not entries:
        return None, "no InRelease/Release files present in {}".format(lists_dir)
    return entries, None


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--output", required=True, help="provenance.json path to write")
    parser.add_argument("--repo-root", required=True)
    parser.add_argument(
        "--live-dir",
        default="build/live",
        help="repo-relative live-build config directory (default build/live)",
    )
    parser.add_argument("--work-dir", required=True, help="live-build work directory (holds config/ and chroot/)")
    parser.add_argument("--packages-dir", required=True, help="directory with the local .deb files")
    parser.add_argument("--artifact", action="append", default=[], help="output file to record (repeatable)")
    parser.add_argument("--build-started-utc", required=True)
    parser.add_argument("--build-finished-utc", required=True)
    parser.add_argument("--source-date-epoch", required=True)
    parser.add_argument("--unverified-host-override", choices=["0", "1"], default="0")
    args = parser.parse_args()

    git_base = ["git", "-c", "safe.directory={}".format(args.repo_root), "-C", args.repo_root]
    git_commit = run_text(git_base + ["rev-parse", "HEAD"])
    git_status = run_text(git_base + ["status", "--porcelain"])
    git_tree_dirty = None if git_status is None else bool(git_status)

    os_release = read_os_release()
    bootstrap = parse_shell_assignments(os.path.join(args.work_dir, "config", "bootstrap"))
    chroot_cfg = parse_shell_assignments(os.path.join(args.work_dir, "config", "chroot"))

    lists, lists_reason = apt_release_files(os.path.join(args.work_dir, "chroot", "var", "lib", "apt", "lists"))
    apt_value = lists if lists is not None else None

    live_inputs = {}
    live_root = os.path.join(args.repo_root, args.live_dir)
    for current, _dirs, files in sorted(os.walk(live_root)):
        for name in sorted(files):
            path = os.path.join(current, name)
            live_inputs[os.path.relpath(path, args.repo_root)] = sha256_file(path)

    local_packages = []
    for name in sorted(os.listdir(args.packages_dir)):
        if not name.endswith(".deb"):
            continue
        path = os.path.join(args.packages_dir, name)
        local_packages.append(
            {
                "file": name,
                "package": run_text(["dpkg-deb", "-f", path, "Package"]),
                "version": run_text(["dpkg-deb", "-f", path, "Version"]),
                "sha256": sha256_file(path),
            }
        )

    outputs = []
    for path in args.artifact:
        outputs.append(
            {"file": os.path.basename(path), "size_bytes": os.path.getsize(path), "sha256": sha256_file(path)}
        )

    provenance = {
        "provenance_format": ProvenanceFormat,
        "reproducibility": "not_claimed",
        "source": {"git_commit": git_commit, "git_tree_dirty": git_tree_dirty},
        "build_host": {
            "os_id": os_release.get("ID"),
            "os_version_id": os_release.get("VERSION_ID"),
            "os_version_codename": os_release.get("VERSION_CODENAME"),
            "kernel_release": platform.release(),
            "machine": platform.machine(),
            "dpkg_architecture": run_text(["dpkg", "--print-architecture"]),
            "in_container": in_container(),
            "unverified_host_override": args.unverified_host_override == "1",
        },
        "tools": {package: package_version(package) for package in ToolPackages},
        "inputs": {
            "distribution": bootstrap.get("LB_DISTRIBUTION"),
            "architecture": bootstrap.get("LB_ARCHITECTURE"),
            "archive_areas": bootstrap.get("LB_ARCHIVE_AREAS"),
            "mirrors": {
                "bootstrap": bootstrap.get("LB_MIRROR_BOOTSTRAP"),
                "chroot": bootstrap.get("LB_MIRROR_CHROOT"),
                "chroot_security": bootstrap.get("LB_MIRROR_CHROOT_SECURITY"),
                "binary": bootstrap.get("LB_MIRROR_BINARY"),
                "binary_security": bootstrap.get("LB_MIRROR_BINARY_SECURITY"),
            },
            "security_updates_enabled": chroot_cfg.get("LB_SECURITY"),
            "updates_enabled": chroot_cfg.get("LB_UPDATES"),
            "lb_config_command": lb_config_command(os.path.join(args.repo_root, args.live_dir, "auto", "config")),
            "source_date_epoch": args.source_date_epoch,
            "apt_release_files": apt_value,
            "apt_release_files_unavailable_reason": lists_reason,
            "live_config_file_sha256": live_inputs,
            "package_versions_note": "installed package versions of the image are listed in the .packages manifest",
        },
        "local_packages": local_packages,
        "outputs": outputs,
        "build_started_utc": args.build_started_utc,
        "build_finished_utc": args.build_finished_utc,
        "written_utc": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    }
    with open(args.output, "w", encoding="utf-8") as handle:
        json.dump(provenance, handle, indent=2, sort_keys=True)
        handle.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
