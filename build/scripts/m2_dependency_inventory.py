"""M2 dependency, provenance and unsafe/native inventory (G1 qualification input).

Usage (python3 -I, from the repository root inside the pinned Debian environment):
  m2_dependency_inventory.py inventory --metadata METADATA.json --out-dir DIR
  m2_dependency_inventory.py sources --metadata METADATA.json --write MANIFEST
  m2_dependency_inventory.py sources --metadata METADATA.json --check MANIFEST

METADATA.json is `cargo metadata --format-version 1 --locked --offline` output; it
supplies paths and targets. The set of packages actually compiled is taken from
`cargo tree --frozen --target <triple>` for each qualified target (Targets below),
because cargo metadata's resolve also lists optional/weak dependencies that are never
built (for example getrandom, serde and pkcs8 here).

inventory: for every compiled non-workspace package, records version, source
directory, owning Debian package/version, enabled features, MSRV, whether it runs a
build script, is a procedural macro or declares native `links`, and occurrences of the
`unsafe` keyword (comments excluded) with the files that contain them. Packages are
classified by how the workspace reaches them: runtime (normal dependency edges), build
(compile-time code: build scripts, proc macros and their dependencies) or dev
(test-only). Writes dependency_inventory.json and dependency_inventory.md. Keyword
counts locate code for review; they are not a soundness assessment.

sources: per-file SHA-256 of every file in every resolved dependency directory, sorted
by path. --write records a manifest; --check fails (exit 1) on any missing, extra or
changed file. Debian's registry entries carry a placeholder package checksum, so this
manifest, not Cargo.lock, pins dependency source content.
"""

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys

RegistryRoot = "/usr/share/cargo/registry"
# x86_64 is the M2 qualification target; aarch64 is the native development host.
Targets = ("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu")
TreeLinePattern = re.compile(r"^(?P<name>[A-Za-z0-9_-]+) v(?P<version>\S+)(?P<rest>.*)$")


def compiled_packages(edges, target):
    """Returns {(name, version)} compiled for `target` along the given edge kinds."""
    completed = subprocess.run(
        ["cargo", "tree", "--frozen", "--target", target, "-e", edges, "--prefix", "none", "--format", "{p}"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        raise SystemExit("cargo tree failed for %s: %s" % (target, completed.stderr))
    packages = set()
    for line in completed.stdout.splitlines():
        match = TreeLinePattern.match(line.strip())
        if match and "(/" not in match.group("rest"):
            packages.add((match.group("name"), match.group("version")))
    return packages


def load_metadata(path):
    with open(path, "r", encoding="utf-8") as handle:
        return json.load(handle)


def dependency_packages(metadata):
    members = set(metadata["workspace_members"])
    return [package for package in metadata["packages"] if package["id"] not in members]


def package_dir(package):
    return os.path.dirname(package["manifest_path"])


def classify(metadata):
    """Returns {package_id: set of "runtime@triple"/"build@triple"} for compiled packages."""
    by_key = {(package["name"], package["version"]): package["id"] for package in dependency_packages(metadata)}
    classes = {}
    for target in Targets:
        runtime = compiled_packages("normal", target)
        everything = compiled_packages("normal,build", target)
        for key in everything:
            if key not in by_key:
                raise SystemExit("cargo tree package %s %s missing from metadata" % key)
            label = "runtime" if key in runtime else "build"
            classes.setdefault(by_key[key], set()).add("%s@%s" % (label, target.split("-", 1)[0]))
    return classes


def strip_comments(text):
    text = re.sub(r"/\*.*?\*/", " ", text, flags=re.S)
    return re.sub(r"//[^\n]*", " ", text)


def unsafe_scan(directory):
    total = 0
    files = {}
    for current, _, names in os.walk(directory):
        for name in sorted(names):
            if not name.endswith(".rs"):
                continue
            path = os.path.join(current, name)
            with open(path, "r", encoding="utf-8", errors="replace") as handle:
                count = len(re.findall(r"\bunsafe\b", strip_comments(handle.read())))
            if count:
                files[os.path.relpath(path, directory)] = count
                total += count
    return total, files


def debian_owner(directory):
    completed = subprocess.run(["dpkg", "-S", directory], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, check=False)
    if completed.returncode != 0:
        return None, None
    package = completed.stdout.split(":", 1)[0].strip()
    version = subprocess.run(
        ["dpkg-query", "-W", "-f", "${Version}", package], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, check=False
    ).stdout.strip()
    return package, version or None


def inventory(metadata, out_dir):
    features = {node["id"]: sorted(node["features"]) for node in metadata["resolve"]["nodes"]}
    classes = classify(metadata)
    rows = []
    for package in sorted(dependency_packages(metadata), key=lambda item: item["name"]):
        if package["id"] not in classes:
            continue
        directory = package_dir(package)
        kinds = sorted({kind for target in package["targets"] for kind in target["kind"]})
        unsafe_total, unsafe_files = unsafe_scan(directory)
        owner, owner_version = debian_owner(directory)
        rows.append(
            {
                "name": package["name"],
                "version": package["version"],
                "classes": sorted(classes[package["id"]]),
                "directory": directory,
                "under_debian_registry": directory.startswith(RegistryRoot + "/"),
                "debian_package": owner,
                "debian_version": owner_version,
                "features": features.get(package["id"], []),
                "rust_version": package.get("rust_version"),
                "build_script": "custom-build" in kinds,
                "proc_macro": "proc-macro" in kinds,
                "links": package.get("links"),
                "unsafe_keyword_count": unsafe_total,
                "unsafe_files": unsafe_files,
                "license": package.get("license"),
            }
        )
    os.makedirs(out_dir, exist_ok=True)
    with open(os.path.join(out_dir, "dependency_inventory.json"), "w", encoding="utf-8") as handle:
        json.dump(rows, handle, indent=2, sort_keys=True)
        handle.write("\n")
    lines = [
        "| Crate | Version | Class | Debian package | Features | MSRV | build.rs | proc-macro | links | unsafe (files) |",
        "|---|---|---|---|---|---|---|---|---|---|",
    ]
    for row in rows:
        lines.append(
            "| %s | %s | %s | %s %s | %s | %s | %s | %s | %s | %d (%d) |"
            % (
                row["name"],
                row["version"],
                ",".join(row["classes"]),
                row["debian_package"] or "NOT-DEBIAN",
                row["debian_version"] or "",
                ",".join(row["features"]) or "-",
                row["rust_version"] or "-",
                "yes" if row["build_script"] else "no",
                "yes" if row["proc_macro"] else "no",
                row["links"] or "-",
                row["unsafe_keyword_count"],
                len(row["unsafe_files"]),
            )
        )
    with open(os.path.join(out_dir, "dependency_inventory.md"), "w", encoding="utf-8") as handle:
        handle.write("\n".join(lines) + "\n")
    problems = [row["name"] for row in rows if not row["under_debian_registry"] or not row["debian_package"]]
    return rows, problems


def source_manifest(metadata):
    compiled = classify(metadata)
    entries = []
    for package in dependency_packages(metadata):
        if package["id"] not in compiled:
            continue
        directory = package_dir(package)
        base = os.path.basename(directory)
        for current, dirs, names in os.walk(directory):
            dirs.sort()
            for name in sorted(names):
                path = os.path.join(current, name)
                if os.path.islink(path):
                    entries.append(("symlink:" + os.readlink(path), os.path.join(base, os.path.relpath(path, directory))))
                    continue
                digest = hashlib.sha256()
                with open(path, "rb") as handle:
                    for block in iter(lambda: handle.read(1 << 16), b""):
                        digest.update(block)
                entries.append((digest.hexdigest(), os.path.join(base, os.path.relpath(path, directory))))
    entries.sort(key=lambda entry: entry[1])
    return entries


def main(arguments):
    parser = argparse.ArgumentParser(prog="m2_dependency_inventory.py")
    commands = parser.add_subparsers(dest="command", required=True)
    inventory_parser = commands.add_parser("inventory")
    inventory_parser.add_argument("--metadata", required=True)
    inventory_parser.add_argument("--out-dir", required=True)
    sources_parser = commands.add_parser("sources")
    sources_parser.add_argument("--metadata", required=True)
    mode = sources_parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write")
    mode.add_argument("--check")
    options = parser.parse_args(arguments)
    metadata = load_metadata(options.metadata)

    if options.command == "inventory":
        rows, problems = inventory(metadata, options.out_dir)
        print("inventory: %d compiled dependency packages across %s" % (len(rows), ", ".join(Targets)))
        if problems:
            print("FAIL dependencies not from the Debian registry: " + ", ".join(problems))
            return 1
        print("PASS all dependencies resolve to Debian-packaged registry sources")
        return 0

    entries = source_manifest(metadata)
    if not entries:
        print("FAIL no dependency source files found")
        return 1
    text = "".join("%s  %s\n" % entry for entry in entries)
    if options.write:
        with open(options.write, "w", encoding="utf-8") as handle:
            handle.write(text)
        print("wrote %d entries to %s" % (len(entries), options.write))
        return 0
    with open(options.check, "r", encoding="utf-8") as handle:
        expected = {}
        for line in handle:
            if line.strip():
                digest, path = line.rstrip("\n").split("  ", 1)
                expected[path] = digest
    actual = {path: digest for digest, path in entries}
    missing = sorted(set(expected) - set(actual))
    extra = sorted(set(actual) - set(expected))
    changed = sorted(path for path in set(expected) & set(actual) if expected[path] != actual[path])
    for label, paths in (("missing", missing), ("unexpected", extra), ("changed", changed)):
        for path in paths[:50]:
            print("%s %s" % (label.upper(), path))
    if missing or extra or changed:
        print("FAIL crate sources: %d missing, %d unexpected, %d changed" % (len(missing), len(extra), len(changed)))
        return 1
    print("PASS crate sources: %d files match %s" % (len(actual), options.check))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
