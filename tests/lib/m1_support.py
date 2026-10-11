"""Shared helpers for the cham3 M1 desktop test suites (tests/desktop/).

Standard library only. Builds on tests/lib/m0_support.py for repository paths
and Debian control parsing; m0_support is not modified. Paths follow the
M1 interface contract (desktop/, packaging/cham3-desktop, packaging/cham3-full,
build/live-desktop). A missing required file or directory raises
AssertionError, so a suite reports FAIL instead of silently skipping.
"""

import json
import os
import re
import sys

sys.path.insert(
    0, os.path.dirname(os.path.abspath(__file__))
)
import m0_support  # noqa: E402

PackageDesktop = "cham3-desktop"
PackageFull = "cham3-full"
PackageDesktopDir = "packaging/cham3-desktop"
PackageFullDir = "packaging/cham3-full"
LiveDesktopDir = "build/live-desktop"
LiveDesktopConfig = LiveDesktopDir + "/auto/config"
LiveDesktopPackageList = LiveDesktopDir + "/config/package-lists/cham3_desktop.list.chroot"
DesktopTreeDir = "desktop"
PanelLayoutPath = "desktop/plasma/layout/cham3_panel_layout.js"
ImageDesktopName = "cham3-m1-desktop-amd64"
ImageDesktopIso = ImageDesktopName + ".iso"
ImageDesktopManifest = ImageDesktopName + ".packages"

# ADR 0002 top-to-bottom order. The first eight are cham3 plasmoids; the last
# two are upstream Plasma plasmoids placed by the layout (not cham3 plasmoids).
Cham3PlasmoidComponents = (
    "user_launcher",
    "alias_wheel",
    "tag_pager",
    "contract_dock",
    "netproto_launcher",
    "killswitch",
    "settings_launcher",
    "task_manager",
)
Cham3Namespace = "org.cham3."
UpstreamPanelWidgets = ("org.kde.plasma.systemtray", "org.kde.plasma.digitalclock")
PanelWidgetOrder = tuple(Cham3Namespace + name for name in Cham3PlasmoidComponents) + (
    UpstreamPanelWidgets
)
PanelMinWidth = 56
PanelMaxWidth = 64

# Curated suite: contract "INCLUDE now - CONFIRMED present in trixie" (amd64
# debian:trixie container, per the coordinator contract). bind9-dnsutils replaces
# the absent dnsutils name.
CuratedRequired = (
    "nmap",
    "zenmap",
    "masscan",
    "bind9-dnsutils",
    "whois",
    "wireshark",
    "tshark",
    "tcpdump",
    "nethogs",
    "iftop",
    "sqlmap",
    "nikto",
    "ffuf",
    "gobuster",
    "aircrack-ng",
    "binwalk",
    "libimage-exiftool-perl",
    "hashcat",
    "john",
    "hydra",
    "torbrowser-launcher",
)
# Contract acceptance GAPS: confirmed absent from trixie with no suitable
# Debian-native package. Each may be omitted only when the cham3-full package
# documents the gap (see documented_omission()). Silent omission is a FAIL.
CuratedDocumentedGaps = ("mitmproxy", "radare2", "rizin", "amass")

# Required KDE core utilities kept in cham3-desktop (contract CONFIRMED Depends).
KdeCoreRequired = (
    "plasma-desktop",
    "plasma-workspace",
    "kwin-wayland",
    "sddm",
    "systemsettings",
    "plasma-nm",
    "plasma-pa",
    "dolphin",
    "konsole",
    "kate",
    "ark",
    "okular",
    "gwenview",
    "kde-spectacle",
    "plasma-systemmonitor",
)

# Redundant utilities that must not be pulled in (task and contract EXCLUDE list).
ExcludedDuplicates = (
    "thunar",
    "pcmanfm",
    "qterminal",
    "xfce4-terminal",
    "tilix",
    "mousepad",
    "leafpad",
    "xarchiver",
    "file-roller",
    "evince",
    "atril",
    "ristretto",
    "firefox-esr",
    "firefox",
)

# Tools gated pending Codex source/readiness review: never a dependency in M1.
GatedTools = (
    "librewolf",
    "ghidra",
    "metasploit-framework",
    "exploitdb",
    "searchsploit",
    "zaproxy",
    "wpscan",
    "cutter",
    "cutter-re",
    "radare2",
    "rizin",
    "mitmproxy",
    "amass",
)

# Alternative desktop environments and task metapackages that conflict with the
# KDE Plasma default. Never allowed in cham3-desktop or cham3-full.
DesktopConflictNames = (
    "kde-full",
    "task-kde-desktop",
    "task-desktop",
    "task-xfce-desktop",
    "task-lxqt-desktop",
    "task-gnome-desktop",
    "task-cinnamon-desktop",
    "xfce4",
    "xfce4-goodies",
    "lxqt",
    "lxde",
    "gnome",
    "gnome-core",
    "cinnamon",
)

# Debian package name atom: lowercase, digits, + . - (Debian policy 5.6.1),
# or a substvar such as ${misc:Depends}.
DebianNamePattern = re.compile(r"^(\$\{[^}]+\}|[a-z0-9][a-z0-9+.\-]+)$")
RelationSplitWhitespace = re.compile(r"\s+")

# Freedesktop registered main categories used for menu placement.
MainMenuCategories = frozenset({
    "AudioVideo", "Audio", "Video", "Development", "Education", "Game",
    "Graphics", "Network", "Office", "Science", "Settings", "System", "Utility",
})

# Names that legitimately break the snake_case rule (external KDE boundaries,
# documented in ADR 0002 and the M1 contract).
ExternalBoundaryStems = frozenset({"Cham3Dark"})


def desktop_path(relative):
    return os.path.join(m0_support.repo_root(), relative)


def require_file(relative, hint):
    path = desktop_path(relative)
    m0_support.require_file(path, hint)
    return path


def read_repo(relative):
    return m0_support.read_text(relative)


def relation_groups(value):
    """Split a Debian relationship field into groups of alternatives.

    Returns (groups, problems). Each group is a list of bare package names;
    version constraints, architecture qualifiers and build profiles are removed.
    """
    problems = []
    groups = []
    flat = RelationSplitWhitespace.sub(" ", value).strip()
    if not flat:
        return groups, problems
    for clause in flat.split(","):
        clause = clause.strip()
        if not clause:
            problems.append("empty clause in relationship field")
            continue
        alternatives = []
        for atom in clause.split("|"):
            bare = re.sub(r"\([^)]*\)", "", atom)
            bare = re.sub(r"\[[^\]]*\]", "", bare)
            bare = re.sub(r"<[^>]*>", "", bare).strip()
            if not DebianNamePattern.match(bare):
                problems.append("malformed package reference: %r" % atom.strip())
            alternatives.append(bare)
        groups.append(alternatives)
    return groups, problems


def relation_names(value):
    names = set()
    groups, _ = relation_groups(value)
    for group in groups:
        names.update(group)
    return names


def binary_stanza(control_text, package):
    """Return the binary stanza whose Package equals package (exactly one)."""
    stanzas = m0_support.parse_control(control_text)
    matches = [stanza for stanza in stanzas if stanza.get("Package") == package]
    if len(matches) != 1:
        raise AssertionError(
            "expected exactly one binary stanza for %s, found %d" % (package, len(matches))
        )
    return matches[0]


def package_control(package_dir, package):
    """Parse <package_dir>/debian/control and return the binary stanza for package."""
    text = read_repo(package_dir + "/debian/control")
    return binary_stanza(text, package)


def documented_omission(package_dir, name):
    """True when name is mentioned in the package's non-relation text.

    Used for soft curated tools: a tool omitted from Depends/Recommends counts as
    documented only when the package's own documentation names it.
    """
    root = os.path.join(m0_support.repo_root(), package_dir)
    if not os.path.isdir(root):
        return False
    for dirpath, _, filenames in os.walk(root):
        for filename in filenames:
            path = os.path.join(dirpath, filename)
            try:
                with open(path, "r", encoding="utf-8", errors="replace") as handle:
                    text = handle.read()
            except OSError:
                continue
            text = re.sub(r"(?m)^(Depends|Recommends|Suggests|Pre-Depends):.*$", "", text)
            if re.search(r"(?<![a-z0-9+.\-])" + re.escape(name) + r"(?![a-z0-9+.\-])", text):
                return True
    return False


def strip_js_comments(text):
    """Remove // and /* */ comments from JS or QML, leaving string literals intact."""
    out = []
    i = 0
    length = len(text)
    while i < length:
        ch = text[i]
        if ch in ("'", '"', "`"):
            end = i + 1
            while end < length and text[end] != ch:
                if text[end] == "\\":
                    end += 1
                if end < length and text[end] == "\n" and ch != "`":
                    break
                end += 1
            out.append(text[i:end + 1])
            i = end + 1
            continue
        if text.startswith("//", i):
            end = text.find("\n", i)
            i = length if end < 0 else end
            continue
        if text.startswith("/*", i):
            end = text.find("*/", i + 2)
            i = length if end < 0 else end + 2
            out.append(" ")
            continue
        out.append(ch)
        i += 1
    return "".join(out)


StringLiteral = re.compile(r"\"(?:[^\"\\\n]|\\.)*\"|'(?:[^'\\\n]|\\.)*'")


def qml_string_literals(text):
    """Return the contents of string literals in QML/JS text (comments removed)."""
    literals = []
    for match in StringLiteral.finditer(strip_js_comments(text)):
        literals.append(match.group(0)[1:-1])
    return literals


def plasmoid_dirs(plugin_id):
    """Find every directory named plugin_id under desktop/ (contract path varies)."""
    root = desktop_path(DesktopTreeDir)
    found = []
    if not os.path.isdir(root):
        return found
    for dirpath, dirnames, _ in os.walk(root):
        if os.path.basename(dirpath) == plugin_id:
            found.append(dirpath)
    return sorted(found)


def load_json(path):
    with open(path, "r", encoding="utf-8") as handle:
        return json.load(handle)


def all_desktop_files(suffixes):
    """Return repo-relative paths of files under desktop/ ending with any suffix."""
    root = desktop_path(DesktopTreeDir)
    results = []
    if not os.path.isdir(root):
        return results
    for dirpath, _, filenames in os.walk(root):
        for filename in filenames:
            if filename.endswith(suffixes):
                results.append(os.path.relpath(os.path.join(dirpath, filename), m0_support.repo_root()))
    return sorted(results)


def image_manifest_path():
    return os.path.join(m0_support.out_dir(), "image", ImageDesktopManifest)


def image_iso_path():
    return os.path.join(m0_support.out_dir(), "image", ImageDesktopIso)


def manifest_package_names(manifest_text):
    """Package names from a live-build .packages manifest (first token per line)."""
    names = set()
    for line in manifest_text.splitlines():
        parts = line.split()
        if parts:
            names.add(parts[0].split(":", 1)[0])
    return names
