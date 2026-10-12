#!/usr/bin/env python3
"""Native F2 source gates: signatures, exhaustive pin policy, closure, URI, controls and upgrade.

Run as root in a DISPOSABLE native amd64 Debian 13 container with network access to
deb.debian.org, security.debian.org, http.kali.org (and the Kali mirrors it redirects
to) and repo.librewolf.net. It rewrites /etc/apt and installs packages: never run it on
a host you care about. TLS and apt signature verification stay enabled; nothing here
sets trusted=yes, disables authentication or adds a key to the global trust store.

The apt configuration is the candidate's, unchanged: the keyring package built by
build/live-desktop/auto/build_archive_keyring from the reviewed keys, the
config/archives/*.list sources and config/archives/*.pref pins, and the Debian sources
live-build writes into the final image (main non-free non-free-firmware for trixie,
trixie-updates and trixie-security; contrib comes from cham3_debian_contrib.list).

Gates (each PASS or FAIL in results.json, with transcripts beside it):
  signatures      apt-get update with Error-Mode=any succeeds; no GPG/NO_PUBKEY error;
                  the keys are only under /usr/share/keyrings
  identity        the Origin/Label/Suite/Codename the real InRelease files carry
                  (recorded only: the pins do not depend on them, Codex F6)
  policy          every package whose candidate is available only from a foreign host
                  (http.kali.org, repo.librewolf.net) is on that host's allowlist; every
                  allowlisted name has a candidate; Kali-only unlisted names have none
  closure_uri     apt-get --print-uris for installing the built cham3-desktop and
                  cham3-full: every download host is an approved host, Kali downloads are
                  allowlisted names only, LibreWolf downloads are librewolf only, and the
                  six foreign roles are present. Origin is attributed by download host,
                  never by the mutable Release labels
  control_deny    with only the default-deny stanzas, the foreign roles have no candidate
  control_leak    without the cham3 pins, the same closure downloads un-allowlisted Kali
                  packages (sensitivity: the pins are what prevents it)
  upgrade         radare2 (with its Kali libraries) and librewolf are really installed;
                  afterwards dist-upgrade downloads nothing from a foreign host outside
                  the allowlists, and an unlisted Kali package still has no candidate

Usage: f2_source_gates.py --repo <checkout> --packages <dir with built .debs> --out <dir>
Exit 0 only when every gate passes.
"""

import argparse
import glob
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import urllib.parse

KaliSite = "http.kali.org"
LibreWolfSite = "repo.librewolf.net"
DebianSites = ("deb.debian.org", "security.debian.org")
ApprovedSites = DebianSites + (KaliSite, LibreWolfSite)
KaliAllowlist = ("zaproxy", "exploitdb", "ghidra", "amass", "libpostal-data", "radare2",
                 "libradare2-6.0.0t64", "libradare2-common")
LibreWolfAllowlist = ("librewolf",)
ForeignRoles = ("zaproxy", "exploitdb", "ghidra", "amass", "radare2", "librewolf")
# Kali-only names that must stay denied (examples the reviews named; not exhaustive:
# the policy gate checks every package).
UnlistedKaliProbes = ("ghidra-data", "kali-defaults", "kali-linux-core", "burpsuite", "metasploit-framework")
DebianSourcesList = (
    "deb http://deb.debian.org/debian/ trixie main non-free non-free-firmware\n"
    "deb http://security.debian.org/ trixie-security main non-free non-free-firmware\n"
    "deb http://deb.debian.org/debian/ trixie-updates main non-free non-free-firmware\n"
)
UriLine = re.compile(r"^'([^']+)'\s+(\S+)\s+(\d+)")
UpgradeProbe = ("radare2", "librewolf")


class gates_t:
    def __init__(self, out_dir):
        self.out_dir = out_dir
        self.results = []

    def record(self, gate, ok, detail):
        self.results.append({"gate": gate, "result": "PASS" if ok else "FAIL", "detail": detail})
        print("%s %s: %s" % ("PASS" if ok else "FAIL", gate, detail if isinstance(detail, str) else json.dumps(detail)),
              flush=True)

    def write(self, name, text):
        with open(os.path.join(self.out_dir, name), "w", encoding="utf-8") as handle:
            handle.write(text)


def run(argv, log=None, env=None, check=False, timeout=3600):
    full_env = dict(os.environ, DEBIAN_FRONTEND="noninteractive", LC_ALL="C")
    if env:
        full_env.update(env)
    completed = subprocess.run(argv, capture_output=True, text=True, env=full_env, timeout=timeout, check=False)
    if log is not None:
        log.append({"argv": argv, "rc": completed.returncode, "stdout": completed.stdout, "stderr": completed.stderr})
    if check and completed.returncode != 0:
        raise SystemExit("command failed (%d): %s\n%s%s" % (completed.returncode, " ".join(argv),
                                                            completed.stdout, completed.stderr))
    return completed


def uri_records(text):
    """[(host, package name, file name)] from apt-get --print-uris output."""
    records = []
    for line in text.splitlines():
        match = UriLine.match(line.strip())
        if not match:
            continue
        uri, filename = match.group(1), match.group(2)
        parts = urllib.parse.urlsplit(uri)
        if parts.scheme == "file":
            continue  # the locally built cham3 packages themselves
        host = (parts.hostname or "").lower()
        records.append((host, urllib.parse.unquote(filename).split("_", 1)[0], filename))
    return records


def uri_problems(records):
    problems = []
    for host, package, filename in records:
        if host not in ApprovedSites:
            problems.append("download from an unapproved host %s: %s" % (host, filename))
        elif host == KaliSite and package not in KaliAllowlist:
            problems.append("un-allowlisted package downloaded from Kali: %s" % filename)
        elif host == LibreWolfSite and package not in LibreWolfAllowlist:
            problems.append("un-allowlisted package downloaded from LibreWolf: %s" % filename)
    return problems


def prepare_apt(repo, gates, log):
    """Install the candidate's keyring, sources and pins; return the archives directory."""
    run(["apt-get", "update"], log, check=True)
    run(["apt-get", "install", "-y", "--no-install-recommends", "python3-apt", "gnupg", "dpkg-dev",
         "ca-certificates"], log, check=True)
    work = tempfile.mkdtemp(prefix="cham3_f2_live_")
    live = os.path.join(work, "live-desktop")
    shutil.copytree(os.path.join(repo, "build/live-desktop"), live)
    completed = subprocess.run(["sh", "auto/build_archive_keyring"], cwd=live, capture_output=True, text=True,
                               check=False)
    log.append({"argv": ["sh", "auto/build_archive_keyring"], "rc": completed.returncode,
                "stdout": completed.stdout, "stderr": completed.stderr})
    if completed.returncode != 0:
        raise SystemExit("build_archive_keyring failed: " + completed.stderr)
    keyring_debs = glob.glob(os.path.join(live, "config/archives/cham3-archive-keyring_*.deb"))
    if len(keyring_debs) != 1:
        raise SystemExit("expected one keyring package, found %r" % keyring_debs)
    run(["dpkg", "-i", keyring_debs[0]], log, check=True)
    for legacy in ("/etc/apt/sources.list.d/debian.sources",):
        if os.path.exists(legacy):
            os.unlink(legacy)
    with open("/etc/apt/sources.list", "w", encoding="utf-8") as handle:
        handle.write(DebianSourcesList)
    archives = os.path.join(live, "config/archives")
    for path in sorted(glob.glob(os.path.join(archives, "*.list"))):
        shutil.copy(path, "/etc/apt/sources.list.d/" + os.path.basename(path))
    for path in sorted(glob.glob(os.path.join(archives, "*.pref"))):
        shutil.copy(path, "/etc/apt/preferences.d/" + os.path.basename(path))
    return archives


def gate_signatures(gates, log):
    update = run(["apt-get", "-o", "APT::Update::Error-Mode=any", "update"], log)
    text = update.stdout + update.stderr
    gates.write("apt_update.log", text)
    bad = re.findall(r"(?m)^.*(GPG error|NO_PUBKEY|not signed|EXPKEYSIG|BADSIG|Signature).*$", text)
    trusted = sorted(name for name in os.listdir("/etc/apt/trusted.gpg.d") if "cham3" in name or "kali" in name
                     or "librewolf" in name) if os.path.isdir("/etc/apt/trusted.gpg.d") else []
    keyrings = sorted(os.path.basename(path) for path in glob.glob("/usr/share/keyrings/cham3_*"))
    ok = update.returncode == 0 and not bad and not trusted and \
        keyrings == ["cham3_kali_archive.gpg", "cham3_librewolf.gpg"]
    gates.record("signatures", ok, {"update_rc": update.returncode, "signature_errors": bad,
                                    "global_trust_entries": trusted, "keyrings": keyrings})
    return ok


def gate_identity(gates):
    identities = {}
    for path in sorted(glob.glob("/var/lib/apt/lists/*InRelease")):
        name = os.path.basename(path)
        if KaliSite not in name and LibreWolfSite not in name:
            continue
        with open(path, "r", encoding="utf-8", errors="replace") as handle:
            text = handle.read()
        identities[name] = {field: (re.search(r"(?m)^%s:\s*(.*)$" % field, text) or [None, None])[1]
                            for field in ("Origin", "Label", "Suite", "Codename")}
    gates.record("identity", bool(identities), identities)


def gate_policy(gates):
    import apt  # python3-apt, installed by prepare_apt from Debian

    cache = apt.Cache()
    escaped, denied, allowed = [], [], {}
    kali_only = 0
    for package in cache:
        candidate = package.candidate
        sites = {origin.site for version in package.versions for origin in version.origins}
        foreign_versions = [version for version in package.versions
                            if {origin.site for origin in version.origins} & {KaliSite, LibreWolfSite}]
        if not foreign_versions:
            continue
        if candidate is not None:
            candidate_sites = {origin.site for origin in candidate.origins}
            if candidate_sites and candidate_sites <= {KaliSite, LibreWolfSite}:
                allow = KaliAllowlist if KaliSite in candidate_sites else LibreWolfAllowlist
                if package.shortname not in allow:
                    escaped.append("%s %s from %s" % (package.shortname, candidate.version,
                                                       ",".join(sorted(candidate_sites))))
        if not sites & set(DebianSites):
            kali_only += 1
            if candidate is None:
                denied.append(package.shortname)
    for name in KaliAllowlist + LibreWolfAllowlist:
        package = cache[name] if name in cache else None
        candidate = package.candidate if package is not None else None
        allowed[name] = None if candidate is None else {
            "version": candidate.version, "sites": sorted({origin.site for origin in candidate.origins})}
    probes = {name: (cache[name].candidate.version if name in cache and cache[name].candidate else None)
              for name in UnlistedKaliProbes}
    missing = sorted(name for name, value in allowed.items() if value is None)
    probe_escapes = sorted(name for name, value in probes.items() if value is not None)
    ok = not escaped and not missing and not probe_escapes
    gates.record("policy", ok, {"foreign_only_packages": kali_only, "denied_foreign_only_packages": len(denied),
                                "escaped": escaped[:50], "escaped_count": len(escaped), "allowlisted": allowed,
                                "missing_allowlisted": missing, "unlisted_probes": probes})
    return ok


def closure_records(local_debs, log, label, gates):
    completed = run(["apt-get", "--print-uris", "-qq", "-y", "install", "--no-install-recommends"] + local_debs, log)
    gates.write("%s_print_uris.log" % label, completed.stdout + completed.stderr)
    simulated = run(["apt-get", "-s", "-y", "install", "--no-install-recommends"] + local_debs, log)
    gates.write("%s_simulate.log" % label, simulated.stdout + simulated.stderr)
    return completed, uri_records(completed.stdout)


def gate_closure(gates, local_debs, log):
    completed, records = closure_records(local_debs, log, "closure", gates)
    problems = uri_problems(records)
    hosts = {}
    for host, package, _ in records:
        hosts.setdefault(host, set()).add(package)
    roles = {}
    for role in ForeignRoles:
        roles[role] = sorted(host for host, packages in hosts.items() if role in packages)
    absent = sorted(role for role, role_hosts in roles.items() if not role_hosts)
    # A role satisfied by a package already installed would not be downloaded; in a fresh
    # container none of them is installed, so absence is a failure.
    kali_packages = sorted(hosts.get(KaliSite, set()))
    librewolf_packages = sorted(hosts.get(LibreWolfSite, set()))
    ok = completed.returncode == 0 and not problems and not absent and librewolf_packages == ["librewolf"]
    gates.record("closure_uri", ok, {"rc": completed.returncode, "downloads": len(records),
                                     "hosts": {host: len(packages) for host, packages in sorted(hosts.items())},
                                     "kali_packages": kali_packages, "librewolf_packages": librewolf_packages,
                                     "roles": roles, "absent_roles": absent, "problems": problems[:50]})
    return ok


def with_preferences(archives, text_by_name, action):
    """Run action() with /etc/apt/preferences.d holding text_by_name instead of the cham3 pins."""
    names = [os.path.basename(path) for path in glob.glob(os.path.join(archives, "*.pref"))]
    try:
        for name in names:
            target = "/etc/apt/preferences.d/" + name
            if name in text_by_name:
                with open(target, "w", encoding="utf-8") as handle:
                    handle.write(text_by_name[name])
            elif os.path.exists(target):
                os.unlink(target)
        return action()
    finally:
        for name in names:
            shutil.copy(os.path.join(archives, name), "/etc/apt/preferences.d/" + name)


def deny_only(text):
    """The default-deny stanzas of a pin file (the Package: * stanzas), nothing else."""
    blocks = [block for block in re.split(r"\n\s*\n", text) if re.search(r"(?m)^Package:\s*\*\s*$", block)]
    return "\n\n".join(blocks) + "\n"


def gate_controls(gates, archives, local_debs, log):
    pins = {}
    for path in glob.glob(os.path.join(archives, "*.pref")):
        with open(path, "r", encoding="utf-8") as handle:
            pins[os.path.basename(path)] = handle.read()

    def deny_action():
        results = {}
        for role in ForeignRoles:
            completed = run(["apt-get", "-s", "-y", "install", "--no-install-recommends", role], log)
            results[role] = completed.returncode
        return results

    deny = with_preferences(archives, {name: deny_only(text) for name, text in pins.items()}, deny_action)
    # zaproxy, exploitdb, ghidra and amass exist only in Kali; radare2 also exists in Debian
    # (an older version, which remains installable from Debian); librewolf only at LibreWolf.
    kali_only_roles = [role for role in ForeignRoles if role != "radare2"]
    deny_ok = all(deny[role] != 0 for role in kali_only_roles)
    gates.record("control_deny", deny_ok, {"simulate_rc": deny})

    def leak_action():
        _, records = closure_records(local_debs, log, "control_leak", gates)
        return records

    records = with_preferences(archives, {}, leak_action)
    leaked = [problem for problem in uri_problems(records) if "un-allowlisted package downloaded from Kali" in problem]
    gates.record("control_leak", bool(leaked), {"unallowlisted_kali_downloads_without_pins": len(leaked),
                                                "examples": leaked[:20]})
    return deny_ok and bool(leaked)


def gate_upgrade(gates, log):
    install = run(["apt-get", "install", "-y", "--no-install-recommends"] + list(UpgradeProbe), log)
    gates.write("upgrade_install.log", install.stdout + install.stderr)
    import apt  # python3-apt

    cache = apt.Cache()
    installed = {}
    for name in ("radare2", "libradare2-6.0.0t64", "libradare2-common", "librewolf"):
        if name in cache and cache[name].installed is not None:
            version = cache[name].installed
            installed[name] = {"version": version.version,
                               "sites": sorted({origin.site for origin in version.origins})}
    upgrade_uris = run(["apt-get", "--print-uris", "-qq", "-y", "dist-upgrade"], log)
    upgrade_sim = run(["apt-get", "-s", "-y", "dist-upgrade"], log)
    gates.write("upgrade_dist_upgrade.log", upgrade_sim.stdout + upgrade_sim.stderr)
    problems = uri_problems(uri_records(upgrade_uris.stdout))
    denied = run(["apt-get", "-s", "-y", "install", "ghidra-data"], log)
    ok = (install.returncode == 0 and upgrade_uris.returncode == 0 and not problems and denied.returncode != 0
          and all(name in installed for name in UpgradeProbe))
    gates.record("upgrade", ok, {"install_rc": install.returncode, "installed": installed,
                                 "dist_upgrade_problems": problems, "unlisted_after_install_rc": denied.returncode})
    return ok


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--repo", required=True)
    parser.add_argument("--packages", required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    if os.geteuid() != 0:
        raise SystemExit("must run as root in a disposable container")
    arch = subprocess.run(["dpkg", "--print-architecture"], capture_output=True, text=True).stdout.strip()
    if arch != "amd64":
        raise SystemExit("must run on native amd64, found " + arch)
    os.makedirs(args.out, exist_ok=True)
    gates = gates_t(args.out)
    log = []
    local_debs = sorted(glob.glob(os.path.join(args.packages, "cham3-desktop_*.deb"))
                        + glob.glob(os.path.join(args.packages, "cham3-full_*.deb")))
    if len(local_debs) != 2:
        raise SystemExit("expected cham3-desktop and cham3-full .debs in %s, found %r" % (args.packages, local_debs))
    local_debs = [os.path.abspath(path) for path in local_debs]
    try:
        archives = prepare_apt(os.path.abspath(args.repo), gates, log)
        if gate_signatures(gates, log):
            gate_identity(gates)
            gate_policy(gates)
            gate_closure(gates, local_debs, log)
            gate_controls(gates, archives, local_debs, log)
            gate_upgrade(gates, log)
    finally:
        with open(os.path.join(args.out, "commands.json"), "w", encoding="utf-8") as handle:
            json.dump(log, handle, indent=1)
        with open(os.path.join(args.out, "results.json"), "w", encoding="utf-8") as handle:
            json.dump(gates.results, handle, indent=1)
    failed = [result["gate"] for result in gates.results if result["result"] != "PASS"]
    expected = {"signatures", "identity", "policy", "closure_uri", "control_deny", "control_leak", "upgrade"}
    missing = sorted(expected - {result["gate"] for result in gates.results})
    print("F2 native gates: %s" % ("PASS" if not failed and not missing else
                                   "FAIL (failed: %s; not run: %s)" % (failed, missing)), flush=True)
    return 0 if not failed and not missing else 1


if __name__ == "__main__":
    sys.exit(main())
