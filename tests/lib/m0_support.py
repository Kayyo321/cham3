"""Shared helpers for the cham3 M0 test suites.

Standard library only. Paths come from CHAM3_REPO_ROOT (default: the parent
of tests/) and CHAM3_OUT_DIR (default: <repo>/out). A missing required
artifact or tool raises AssertionError, so the suite reports FAIL instead of
silently skipping.
"""

import hashlib
import os
import re
import subprocess

TestsDir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

ExpectedPackage = "cham3-base"
ExpectedVersion = "0.0.0~m0"
ImageBaseName = "cham3-m0-dev-amd64"
ImageIsoName = ImageBaseName + ".iso"
ImageManifestName = ImageBaseName + ".packages"
DebFileName = "cham3-base_0.0.0~m0_all.deb"
OfficialMirrorHosts = frozenset({"deb.debian.org", "security.debian.org"})
ReservedDomains = ("example.com", "example.net", "example.org")
ReservedDomainSuffixes = (".invalid", ".test", ".example")
Sha256SumsLine = re.compile(r"^([0-9a-f]{64}) [ *](.+)$")


def repo_root():
    value = os.environ.get("CHAM3_REPO_ROOT")
    if value:
        return os.path.abspath(value)
    return os.path.dirname(TestsDir)


def out_dir():
    value = os.environ.get("CHAM3_OUT_DIR")
    if value:
        return os.path.abspath(value)
    return os.path.join(repo_root(), "out")


def run_tool(argv, cwd=None):
    """Run an external tool and capture text output. A missing tool is a FAIL."""
    try:
        return subprocess.run(
            argv, cwd=cwd, capture_output=True, text=True, check=False
        )
    except FileNotFoundError:
        raise AssertionError("required executable not found: %s" % argv[0]) from None


def git_output(args, root=None):
    completed = run_tool(["git", "-C", root or repo_root()] + list(args))
    if completed.returncode != 0:
        raise AssertionError(
            "git %s failed: %s" % (" ".join(args), completed.stderr.strip())
        )
    return completed.stdout


def tracked_files(root=None):
    output = git_output(["ls-files", "-z"], root)
    return sorted(item for item in output.split("\0") if item)


def tracked_modes(root=None):
    """Map each tracked path to its git index mode, for example 100755."""
    modes = {}
    output = git_output(["ls-files", "-s", "-z"], root)
    for entry in output.split("\0"):
        if not entry:
            continue
        meta, _, path = entry.partition("\t")
        modes[path] = meta.split(" ", 1)[0]
    return modes


def require_file(path, hint):
    if not os.path.isfile(path):
        raise AssertionError("required file missing: %s (%s)" % (path, hint))


def read_text(relative_path, root=None):
    path = os.path.join(root or repo_root(), relative_path)
    require_file(path, "expected repository file")
    with open(path, "r", encoding="utf-8", errors="replace") as handle:
        return handle.read()


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def sha256sums_entries(path):
    entries = []
    with open(path, "r", encoding="utf-8") as handle:
        for line in handle:
            line = line.rstrip("\n")
            if not line.strip():
                continue
            match = Sha256SumsLine.match(line)
            if not match:
                raise AssertionError(
                    "malformed SHA256SUMS line in %s: %r" % (path, line)
                )
            entries.append((match.group(1), match.group(2)))
    return entries


def sha256sums_problems(directory):
    """Return a list of problems with <directory>/SHA256SUMS (empty if clean)."""
    sums_path = os.path.join(directory, "SHA256SUMS")
    if not os.path.isfile(sums_path):
        return ["missing SHA256SUMS in " + directory]
    problems = []
    entries = sha256sums_entries(sums_path)
    if not entries:
        problems.append("SHA256SUMS lists no files in " + directory)
    for digest, name in entries:
        path = os.path.join(directory, name)
        if not os.path.isfile(path):
            problems.append("listed file is missing: " + name)
        elif sha256_file(path) != digest:
            problems.append("sha256 mismatch for " + name)
    return problems


def parse_control(text):
    """Parse Debian control-format text into a list of field dictionaries."""
    stanzas = []
    current = None
    field = None
    for raw in text.splitlines():
        if raw.startswith("#"):
            continue
        if not raw.strip():
            if current is not None:
                stanzas.append(current)
            current = None
            field = None
            continue
        if raw[0] in " \t":
            if current is None or field is None:
                raise AssertionError("continuation line without a field: %r" % raw)
            current[field] += "\n" + raw.strip()
            continue
        if ":" not in raw:
            raise AssertionError("malformed control line: %r" % raw)
        name, value = raw.split(":", 1)
        if current is None:
            current = {}
        field = name.strip()
        current[field] = value.strip()
    if current is not None:
        stanzas.append(current)
    return stanzas


def normalize_space(text):
    return " ".join(text.split()).lower()


def email_domain_is_reserved(address):
    domain = address.rsplit("@", 1)[-1].lower()
    return domain in ReservedDomains or domain.endswith(ReservedDomainSuffixes)
