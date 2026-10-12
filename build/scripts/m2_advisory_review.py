"""M2 dependency advisory review (G1 §11): Debian security tracker and RustSec.

Usage (python3 -I, inside the pinned Debian container so dpkg is available):
  m2_advisory_review.py --tracker TRACKER.json --advisory-db ADVISORY_DB_DIR \
      --inventory DEPENDENCY_INVENTORY.json --out-dir DIR

Inputs are fetched separately and recorded by digest/commit (this tool performs no
network access): TRACKER.json is https://security-tracker.debian.org/tracker/data/json;
ADVISORY_DB_DIR is a checkout of https://github.com/rustsec/advisory-db;
DEPENDENCY_INVENTORY.json is build/scripts/m2_dependency_inventory.py output.

Checks:
* Debian: maps every installed package that is a compiled-crate source, an explicit pin
  (build/m2/debian_packages.txt) or a runtime library of the toolchain/oracles to its
  source package; reports trixie issues still open (with urgency) and resolved issues
  whose fixed version is newer than the installed version ("behind").
* RustSec: every compiled crate version (plus declared test-only crates) against each
  non-withdrawn advisory's patched/unaffected ranges.

Writes debian_open.tsv, debian_behind.tsv, rustsec.tsv and summary.txt. Exit 1 if any
pinned package is behind a resolved fix or any crate version is affected; open issues
are reported for review, not failed automatically (their applicability needs judgment).
"""

import argparse
import json
import os
import re
import subprocess
import sys
import tomllib

ExtraRuntimePackages = {"libc6", "libssl3t64", "libsodium23", "libargon2-1", "libstd-rust-1.85", "libgcc-s1", "libstdc++6"}
TestOnlyCrates = {("proptest", "1.6.0"), ("ciborium", "0.2.2"), ("ciborium-io", "0.2.2"), ("ciborium-ll", "0.2.2")}


def installed_sources():
    output = subprocess.run(["dpkg-query", "-W", "-f", "${Package}\t${Source}\t${Version}\n"],
                            stdout=subprocess.PIPE, text=True, check=True).stdout
    rows = []
    for line in output.splitlines():
        package, source, version = line.split("\t")
        name = source.split(" ")[0] if source else package
        match = re.search(r"\((.*)\)", source)
        rows.append((package, name, match.group(1) if match else version))
    return rows


def version_less(left, right):
    return subprocess.run(["dpkg", "--compare-versions", left, "lt", right], check=False).returncode == 0


def parse_semver(text):
    text = text.split("+")[0]
    pre = ""
    if "-" in text:
        text, pre = text.split("-", 1)
    parts = [int(part) for part in text.split(".")]
    while len(parts) < 3:
        parts.append(0)
    return tuple(parts), pre


def compare_semver(left, right):
    (left_parts, left_pre), (right_parts, right_pre) = parse_semver(left), parse_semver(right)
    if left_parts != right_parts:
        return (left_parts > right_parts) - (left_parts < right_parts)
    if left_pre == right_pre:
        return 0
    if not left_pre:
        return 1
    if not right_pre:
        return -1
    return (left_pre > right_pre) - (left_pre < right_pre)


def satisfies(version, requirement):
    match = re.match(r"^(>=|<=|>|<|=|\^|~)?\s*(\S+)$", requirement.strip())
    operator, bound = match.group(1) or "^", match.group(2)
    order = compare_semver(version, bound)
    if operator == ">=":
        return order >= 0
    if operator == "<=":
        return order <= 0
    if operator == ">":
        return order > 0
    if operator == "<":
        return order < 0
    if operator == "=":
        return order == 0
    (bound_parts, _), (version_parts, _) = parse_semver(bound), parse_semver(version)
    if operator == "~":
        return order >= 0 and version_parts[:2] == bound_parts[:2]
    if order < 0:
        return False
    if bound_parts[0] > 0:
        return version_parts[0] == bound_parts[0]
    if bound_parts[1] > 0:
        return version_parts[0] == 0 and version_parts[1] == bound_parts[1]
    return version_parts == bound_parts


def any_range(version, ranges):
    return any(all(satisfies(version, part) for part in item.split(",")) for item in ranges)


def main(arguments):
    parser = argparse.ArgumentParser(prog="m2_advisory_review.py")
    parser.add_argument("--tracker", required=True)
    parser.add_argument("--advisory-db", required=True)
    parser.add_argument("--inventory", required=True)
    parser.add_argument("--pins", default="build/m2/debian_packages.txt")
    parser.add_argument("--out-dir", required=True)
    options = parser.parse_args(arguments)
    os.makedirs(options.out_dir, exist_ok=True)

    with open(options.inventory, "r", encoding="utf-8") as handle:
        inventory = json.load(handle)
    with open(options.pins, "r", encoding="utf-8") as handle:
        pins = {line.split("=")[0] for line in handle if line.strip() and not line.startswith("#")}
    wanted = {row["debian_package"] for row in inventory if row["debian_package"]} | pins | ExtraRuntimePackages
    sources = {}
    for package, source, version in installed_sources():
        if package in wanted:
            sources.setdefault(source, set()).add(version)
    with open(options.tracker, "r", encoding="utf-8") as handle:
        tracker = json.load(handle)

    open_rows, behind_rows = [], []
    for source in sorted(sources):
        for issue, info in sorted(tracker.get(source, {}).items()):
            release = info.get("releases", {}).get("trixie")
            if not release:
                continue
            for version in sorted(sources[source]):
                if release.get("status") == "open":
                    open_rows.append((source, version, issue, release.get("urgency", ""), info.get("description", "")[:160]))
                fixed = release.get("fixed_version")
                if release.get("status") == "resolved" and fixed not in (None, "0") and version_less(version, fixed):
                    behind_rows.append((source, version, issue, fixed))

    crates = {(row["name"], row["version"]) for row in inventory} | TestOnlyCrates
    rustsec_rows = []
    for name, version in sorted(crates):
        directory = os.path.join(options.advisory_db, "crates", name)
        if not os.path.isdir(directory):
            continue
        for entry in sorted(os.listdir(directory)):
            with open(os.path.join(directory, entry), "r", encoding="utf-8") as handle:
                front = re.search(r"```toml\n(.*?)\n```", handle.read(), re.S)
            metadata = tomllib.loads(front.group(1))
            advisory, versions = metadata["advisory"], metadata.get("versions", {})
            if advisory.get("withdrawn"):
                continue
            patched, unaffected = versions.get("patched", []), versions.get("unaffected", [])
            affected = not (patched or unaffected) or not (any_range(version, patched) or any_range(version, unaffected))
            rustsec_rows.append((name, version, advisory["id"], advisory.get("informational", "vulnerability"),
                                 "AFFECTED" if affected else "not_affected", ",".join(patched)))

    def write(name, header, rows):
        with open(os.path.join(options.out_dir, name), "w", encoding="utf-8") as handle:
            handle.write("\t".join(header) + "\n")
            for row in rows:
                handle.write("\t".join(row) + "\n")

    write("debian_open.tsv", ("source", "installed", "issue", "urgency", "description"), open_rows)
    write("debian_behind.tsv", ("source", "installed", "issue", "fixed"), behind_rows)
    write("rustsec.tsv", ("crate", "version", "advisory", "kind", "result", "patched"), rustsec_rows)
    affected = [row for row in rustsec_rows if row[4] == "AFFECTED"]
    rust_open = [row for row in open_rows if row[0] == "rustc" or row[0].startswith("rust-")]
    triage = [row for row in open_rows if row[3] != "unimportant"]
    summary = [
        "sources checked: %d" % len(sources),
        "debian open trixie issues: %d (not marked unimportant: %d)" % (len(open_rows), len(triage)),
        "debian open issues in rustc/rust-* sources: %d" % len(rust_open),
        "debian resolved fixes newer than installed: %d" % len(behind_rows),
        "rustsec advisories considered: %d, affected crate versions: %d" % (len(rustsec_rows), len(affected)),
    ]
    with open(os.path.join(options.out_dir, "summary.txt"), "w", encoding="utf-8") as handle:
        handle.write("\n".join(summary) + "\n")
    print("\n".join(summary))
    return 1 if behind_rows or affected else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
