"""Parsing and policy for the owner-approved foreign apt sources, pins, and keys (F2).

Two foreign repositories are included by the owner decision (SECURITY-CRITICAL):
  - Kali (kali-rolling, main): zaproxy, exploitdb, ghidra, amass, radare2, and the
    allowlisted dependencies libpostal-data, libradare2-6.0.0t64, libradare2-common;
  - LibreWolf (suite librewolf): librewolf.

Policy for each repository:
  - the origin is denied by default (Package: * with Pin-Priority -1);
  - only its allowlist is raised to Pin-Priority 500; no other package may be pinned
    for the origin;
  - every source names a Signed-By keyring under /usr/share/keyrings that is shipped,
    is not trusted=yes, and uses binary deb entries over http(s);
  - no key is installed through apt-key or trusted.gpg.d.

The parsers read text, so the same policy runs over the build configuration (static
check), an image's /etc/apt tree (Codex Remote image gate), and fixtures. Standard
library only. Callers put tests/lib on sys.path first.
"""

import os
import re

import m1_support

KeyringDir = "/usr/share/keyrings/"
DefaultDenyPriority = -1
AllowedPriority = 500
LocalPackagePrefix = "cham3-"
WildcardCharacters = frozenset("*?[")
AllowedSourceTypes = ("deb",)
OneLineSource = re.compile(r"^(deb|deb-src)\s+(?:\[([^\]]*)\]\s+)?(\S+)\s+(\S+)(?:\s+(.*))?$")

KaliSpec = {
    "name": "kali",
    "host": "kali.org",
    "pin_token": "o=kali",
    "origin_pin": "release o=Kali",
    "suite": "kali-rolling",
    "components": ("main",),
    "allowlist": m1_support.KaliAllowlist,
}
LibreWolfSpec = {
    "name": "librewolf",
    "host": "librewolf.net",
    "pin_token": "n=librewolf",
    "origin_pin": "release n=librewolf",
    "suite": None,
    "components": None,
    "allowlist": m1_support.LibreWolfAllowlist,
}
RepoSpecs = (KaliSpec, LibreWolfSpec)


def split_stanzas(text):
    return [block for block in re.split(r"\n\s*\n", text.strip()) if block.strip()]


def fields_of(block):
    """Top-level 'Key: value' fields of one stanza (keys lower-cased, comments skipped)."""
    fields = {}
    for line in block.splitlines():
        if not line.strip() or line.lstrip().startswith("#") or line[0] in " \t":
            continue
        key, sep, value = line.partition(":")
        if sep:
            fields[key.strip().lower()] = value.strip()
    return fields


def parse_options(text):
    options = {}
    for token in text.split():
        key, sep, value = token.partition("=")
        options[key.lower()] = value if sep else ""
    return options


def parse_sources(text):
    """Source entries from deb822 (.sources) or one-line (.list) text."""
    entries = []
    one_line = any(re.match(r"^(deb|deb-src)\s", line.strip()) for line in text.splitlines())
    if one_line:
        for line in text.splitlines():
            stripped = line.strip()
            if not stripped or stripped.startswith("#"):
                continue
            match = OneLineSource.match(stripped)
            if not match:
                entries.append({"types": [], "uris": [], "suites": [], "components": [],
                                "signed_by": None, "trusted": False, "malformed": stripped})
                continue
            options = parse_options(match.group(2) or "")
            entries.append({
                "types": [match.group(1)],
                "uris": [match.group(3)],
                "suites": [match.group(4)],
                "components": (match.group(5) or "").split(),
                "signed_by": options.get("signed-by"),
                "trusted": options.get("trusted", "").lower() == "yes",
                "malformed": None,
            })
        return entries
    for block in split_stanzas(text):
        fields = fields_of(block)
        if not fields:
            continue
        entries.append({
            "types": fields.get("types", "").split(),
            "uris": fields.get("uris", "").split(),
            "suites": fields.get("suites", "").split(),
            "components": fields.get("components", "").split(),
            "signed_by": fields.get("signed-by") or None,
            "trusted": fields.get("trusted", "").lower() == "yes",
            "malformed": None if fields.get("uris") and fields.get("suites") else "missing URIs or Suites",
        })
    return entries


def parse_preferences(text):
    """apt preference stanzas: packages, pin text (whitespace-normalized), and priority."""
    stanzas = []
    for block in split_stanzas(text):
        fields = fields_of(block)
        if not fields:
            continue
        priority = fields.get("pin-priority", "")
        stanzas.append({
            "packages": fields.get("package", "").split(),
            "pin": " ".join(fields.get("pin", "").split()),
            "priority": int(priority) if re.fullmatch(r"-?[0-9]+", priority) else None,
        })
    return stanzas


def parse_packages_index(text):
    pairs = set()
    for block in split_stanzas(text):
        fields = fields_of(block)
        if fields.get("package") and fields.get("version"):
            pairs.add((fields["package"], fields["version"]))
    return pairs


def installed_pairs(status_text):
    """(name, version) for packages whose dpkg Status says installed."""
    pairs = set()
    for block in split_stanzas(status_text):
        fields = fields_of(block)
        status = fields.get("status", "")
        if status.endswith(" installed") and fields.get("package") and fields.get("version"):
            pairs.add((fields["package"], fields["version"]))
    return pairs


def repo_policy_problems(spec, source_files, pref_files, keyring_names):
    """Problems with one foreign repository's sources and pins (empty means accepted).

    source_files maps a file name to its parsed source entries. pref_files maps a file
    name to its parsed preference stanzas. keyring_names holds the basenames of the
    keyrings that are actually shipped. Key files are checked by key_problems().
    """
    name = spec["name"]
    problems = []
    sources = [
        (fname, entry)
        for fname, entries in sorted(source_files.items())
        for entry in entries
        if any(spec["host"] in uri for uri in entry["uris"])
    ]
    if not sources:
        problems.append("no %s source is configured" % name)
    for fname, entry in sources:
        where = "%s (%s)" % (fname, name)
        if entry["malformed"]:
            problems.append("%s: malformed source entry: %s" % (where, entry["malformed"]))
        if entry["trusted"]:
            problems.append("%s: trusted=yes disables signature checks" % where)
        if not entry["types"] or any(t not in AllowedSourceTypes for t in entry["types"]):
            problems.append("%s: only binary deb sources are allowed" % where)
        signed_by = entry["signed_by"] or ""
        if not signed_by:
            problems.append("%s: no Signed-By keyring" % where)
        elif not signed_by.startswith(KeyringDir):
            problems.append("%s: Signed-By %s is not under %s" % (where, signed_by, KeyringDir))
        elif os.path.basename(signed_by) not in keyring_names:
            problems.append("%s: Signed-By keyring %s is not shipped" % (where, os.path.basename(signed_by)))
        for uri in entry["uris"]:
            if not re.match(r"^https?://", uri):
                problems.append("%s: URI is not http(s): %s" % (where, uri))
        if spec["suite"] is not None and entry["suites"] != [spec["suite"]]:
            problems.append("%s: suite must be %s, found %s"
                            % (where, spec["suite"], " ".join(entry["suites"]) or "none"))
        if spec["components"] is not None and sorted(entry["components"]) != sorted(spec["components"]):
            problems.append("%s: components must be %s" % (where, " ".join(spec["components"])))

    origin = []
    for fname, stanzas in sorted(pref_files.items()):
        for stanza in stanzas:
            if spec["pin_token"] in stanza["pin"].lower() and stanza["pin"] != spec["origin_pin"]:
                problems.append("%s: unexpected %s pin form: %r" % (fname, name, stanza["pin"]))
            if stanza["pin"] == spec["origin_pin"]:
                origin.append((fname, stanza))
    defaults = [stanza for _, stanza in origin if stanza["packages"] == ["*"]]
    if not any(stanza["priority"] == DefaultDenyPriority for stanza in defaults):
        problems.append("no default deny (Package: *, Pin-Priority -1) for the %s origin" % name)
    for stanza in defaults:
        if stanza["priority"] != DefaultDenyPriority:
            problems.append("the %s origin default priority must be -1, found %s" % (name, stanza["priority"]))
    raised = {}
    for fname, stanza in origin:
        if stanza["packages"] == ["*"]:
            continue
        if not stanza["packages"] or stanza["priority"] is None:
            problems.append("%s: malformed %s pin stanza" % (fname, name))
            continue
        for package in stanza["packages"]:
            if WildcardCharacters & set(package):
                problems.append("%s: wildcard %s pin for %r" % (fname, name, package))
            elif package not in spec["allowlist"]:
                problems.append("%s: non-allowlisted %s package %s pinned at %s"
                                % (fname, name, package, stanza["priority"]))
            else:
                raised[package] = stanza["priority"]
    for package in spec["allowlist"]:
        if package not in raised:
            problems.append("allowlisted %s package %s has no pin" % (name, package))
        elif raised[package] != AllowedPriority:
            problems.append("allowlisted %s package %s is pinned at %s, expected %d"
                            % (name, package, raised[package], AllowedPriority))
    return problems


def key_problems(key_files):
    """Keys must reach apt through Signed-By keyrings, never apt-key or trusted.gpg.d."""
    return ["key installed through apt-key or trusted.gpg.d: %s" % name for name in sorted(key_files)]


def origin_problems(installed, debian_index, foreign_indexes, allowlists):
    """Installed packages whose origin is a foreign repository not on its allowlist.

    installed: set of (name, version). debian_index: set of (name, version) from the
    Debian lists. foreign_indexes: {repo name: set of (name, version)}. allowlists:
    {repo name: allowlist}. A cham3 local package is exempt. An installed version found
    in no index is a problem (origin cannot be established).
    """
    problems = []
    for name, version in sorted(installed):
        if name.startswith(LocalPackagePrefix) or (name, version) in debian_index:
            continue
        origins = sorted(repo for repo, index in foreign_indexes.items() if (name, version) in index)
        if not origins:
            problems.append("origin cannot be established for %s %s" % (name, version))
        for repo in origins:
            if name not in allowlists.get(repo, ()):
                problems.append("un-allowlisted %s-origin package installed: %s %s" % (repo, name, version))
    return problems
