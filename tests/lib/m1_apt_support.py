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

import base64
import hashlib
import os
import re
import struct

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


# OpenPGP key checks (F2). The reviewed keys are ASCII-armored public keys. This reader
# computes the primary fingerprint from the packets without gpg, so the static suite can
# confirm the pinned fingerprints independently of build_archive_keyring.
ArmorBegin = "-----BEGIN PGP PUBLIC KEY BLOCK-----"
ArmorEnd = "-----END PGP PUBLIC KEY BLOCK-----"
PrimaryKeyTag = 6
SecretKeyTags = (5, 7)
SignatureTag = 2
KeyRevocationSignature = 0x20
RevocationSignatureTypes = (0x20,)


def crc24(data):
    crc = 0xB704CE
    for byte in data:
        crc ^= byte << 16
        for _ in range(8):
            crc <<= 1
            if crc & 0x1000000:
                crc ^= 0x1864CFB
    return crc & 0xFFFFFF


def dearmor_blocks(text):
    """Binary payloads of every ASCII-armored public key block; raises ValueError if malformed."""
    blocks = []
    lines = text.splitlines()
    index = 0
    while index < len(lines):
        if lines[index].strip() != ArmorBegin:
            index += 1
            continue
        index += 1
        while index < len(lines) and lines[index].strip() != "":
            index += 1          # armor headers, up to the separating blank line
        index += 1
        body, checksum = [], None
        while index < len(lines) and lines[index].strip() != ArmorEnd:
            line = lines[index].strip()
            if line.startswith("="):
                checksum = line[1:]
            elif line:
                body.append(line)
            index += 1
        if index >= len(lines):
            raise ValueError("armored block has no END line")
        try:
            data = base64.b64decode("".join(body), validate=True)
        except ValueError:
            raise ValueError("armored block is not valid base64") from None
        if checksum is None:
            raise ValueError("armored block has no CRC24 line")
        if crc24(data) != int.from_bytes(base64.b64decode(checksum), "big"):
            raise ValueError("armored block CRC24 does not match")
        blocks.append(data)
        index += 1
    if not blocks:
        raise ValueError("no armored PGP public key block found")
    return blocks


def openpgp_packets(data):
    """(tag, body) for each OpenPGP packet; partial and indeterminate lengths are refused."""
    packets, index = [], 0
    while index < len(data):
        first = data[index]
        if not first & 0x80:
            raise ValueError("not an OpenPGP packet header")
        if first & 0x40:
            tag = first & 0x3F
            index += 1
            length_byte = data[index]
            if length_byte < 192:
                length, index = length_byte, index + 1
            elif length_byte <= 223:
                length, index = ((length_byte - 192) << 8) + data[index + 1] + 192, index + 2
            elif length_byte == 255:
                length, index = struct.unpack(">I", data[index + 1:index + 5])[0], index + 5
            else:
                raise ValueError("partial body lengths are not supported")
        else:
            tag = (first & 0x3C) >> 2
            length_type = first & 0x03
            index += 1
            if length_type == 0:
                length, index = data[index], index + 1
            elif length_type == 1:
                length, index = struct.unpack(">H", data[index:index + 2])[0], index + 2
            elif length_type == 2:
                length, index = struct.unpack(">I", data[index:index + 4])[0], index + 4
            else:
                raise ValueError("indeterminate packet length is not supported")
        body = data[index:index + length]
        if len(body) != length:
            raise ValueError("truncated OpenPGP packet")
        packets.append((tag, body))
        index += length
    return packets


def v4_primary_fingerprint(body):
    """OpenPGP v4 fingerprint: SHA-1 over 0x99, the two-octet length, and the key packet body."""
    return hashlib.sha1(b"\x99" + struct.pack(">H", len(body)) + body).hexdigest().upper()


def key_text_problems(text, expected_fingerprint):
    """Problems with one armored key file (empty means it holds exactly the expected primary key).

    Rejects: a file that does not parse, any number other than one primary key, secret key
    material, a primary-key revocation signature, and a primary fingerprint other than the
    expected one. Expiry is enforced at build time by build_archive_keyring (gpg); this check
    does not depend on today's date.
    """
    problems = []
    try:
        blocks = dearmor_blocks(text)
    except ValueError as error:
        return ["key file does not parse: %s" % error]
    primaries, secrets, revocations = [], 0, 0
    for block in blocks:
        try:
            packets = openpgp_packets(block)
        except (ValueError, IndexError, struct.error) as error:
            return ["key packets do not parse: %s" % error]
        for tag, body in packets:
            if tag == PrimaryKeyTag:
                primaries.append(body)
            elif tag in SecretKeyTags:
                secrets += 1
            elif tag == SignatureTag and len(body) > 2:
                signature_type = body[1] if body[0] == 4 else body[2] if body[0] == 3 else None
                if signature_type in RevocationSignatureTypes:
                    revocations += 1
    if len(primaries) != 1:
        problems.append("key file holds %d primary keys, expected exactly 1" % len(primaries))
    if secrets:
        problems.append("key file contains %d secret key packet(s)" % secrets)
    if revocations:
        problems.append("key file contains a primary-key revocation signature")
    if len(primaries) == 1:
        body = primaries[0]
        if body[0] != 4:
            problems.append("primary key is not an OpenPGP v4 key (version %d)" % body[0])
        elif v4_primary_fingerprint(body) != expected_fingerprint:
            problems.append("primary fingerprint is %s, expected %s"
                            % (v4_primary_fingerprint(body), expected_fingerprint))
    return problems


KeyringScriptNamePattern = re.compile(r"(?m)^(kali|librewolf)_keyring_name=(\S+)\s*$")
KeyringScriptHashPattern = re.compile(r"(?m)^(kali|librewolf)_sha256=([0-9a-f]{64})\s*$")
KeyringScriptFingerprintPattern = re.compile(r"(?m)^(kali|librewolf)_fingerprint=([0-9A-F]{40})\s*$")
KeyringScriptRequirements = (
    ("SHA-256 check of each armored key", r"sha256sum"),
    ("primary fingerprint check", r"check_primary"),
    ("exactly one primary key", r'=\s*"1"\s*\]'),
    ("secret key material rejection", r"contains secret key material"),
    ("expired or revoked key rejection", r'\$2 == "e" \|\| \$2 == "r"'),
    ("keyring dearmored under /usr/share/keyrings", r"usr/share/keyrings/"),
    ("dearmor with gpg", r"--dearmor"),
)
AptKeyInvocation = re.compile(r"(?m)^[^#\n]*\bapt-key\s+(add|adv|del|update)\b")
TrustedStoreWrite = re.compile(r"(?m)^[^#\n]*(>|\bcp\b|\binstall\b|\bmv\b)[^#\n]*trusted\.gpg")


def keyring_script_names(text):
    """{'kali': shipped keyring name, 'librewolf': ...} from the *_keyring_name= constants."""
    return {match.group(1): match.group(2) for match in KeyringScriptNamePattern.finditer(text)}


def keyring_script_problems(text):
    """Text evidence that build_archive_keyring checks and rejects what it must (empty means present).

    This is a requirement list over the script text. Behavior is asserted by the build itself,
    which fails closed, and by the Codex Remote image gate.
    """
    problems = []
    for label, pattern in KeyringScriptRequirements:
        if not re.search(pattern, text):
            problems.append("build_archive_keyring lacks: " + label)
    pinned_hashes = {match.group(1): match.group(2) for match in KeyringScriptHashPattern.finditer(text)}
    pinned_prints = {match.group(1): match.group(2) for match in KeyringScriptFingerprintPattern.finditer(text)}
    for repo in ("kali", "librewolf"):
        if repo not in pinned_hashes:
            problems.append("build_archive_keyring does not pin the SHA-256 of the %s key" % repo)
        if repo not in pinned_prints:
            problems.append("build_archive_keyring does not pin the %s primary fingerprint" % repo)
    if AptKeyInvocation.search(text):
        problems.append("build_archive_keyring calls apt-key")
    if TrustedStoreWrite.search(text):
        problems.append("build_archive_keyring writes into a trusted.gpg store")
    return problems


def apt_key_invocation_problems(texts):
    """Build-time scan: no apt-key add/adv/del/update is invoked (comments and prose are ignored)."""
    problems = []
    for path, text in sorted(texts.items()):
        if AptKeyInvocation.search(text):
            problems.append("apt-key is invoked in %s" % path)
    return problems

