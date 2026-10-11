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
import shutil
import subprocess
import sys
import tempfile

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
# documented in ADR 0002 and the M1 contract): the Cham3Dark color-scheme file
# and the Cham3 wallpaper theme directory. KDE references both by these exact
# capitalized names (color-scheme and wallpaper theme identifiers), so they are
# external-interface spellings, not cham3 internal identifiers.
ExternalBoundaryStems = frozenset({"Cham3Dark", "Cham3"})


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


# F1 per-profile local package staging (build/scripts/build_image.sh). Each image
# profile's cham3-local-packages manifest names the locally built packages that
# profile stages into config/packages.chroot. live-build force-installs every staged
# .deb, so the M0 profile stages only cham3-base and the M1 desktop profile stages
# only cham3-desktop and cham3-full (cham3-base is not staged for M1).
ProfileManifestName = "cham3-local-packages"
LiveBaseDir = "build/live"
PackageBase = "cham3-base"
M0ProfileDir = LiveBaseDir
M1ProfileDir = LiveDesktopDir
M0StagedPackages = (PackageBase,)
M1StagedPackages = (PackageDesktop, PackageFull)
# Every locally built M1 package name the manifests may draw from.
LocalPackageNames = (PackageBase, PackageDesktop, PackageFull)
BuildImageScript = "build/scripts/build_image.sh"
StageDryRunVariable = "CHAM3_STAGE_DRYRUN"


def local_manifest_names(text):
    """Package names in a cham3-local-packages manifest, in file order.

    One name per line; a '#' starts a comment; blank lines are ignored.
    """
    names = []
    for raw in text.splitlines():
        name = raw.split("#", 1)[0].strip()
        if name:
            names.append(name)
    return names


def dpkg_deb_available():
    return shutil.which("dpkg-deb") is not None


def build_fixture_deb(scratch, package, filename):
    """Build a minimal placeholder .deb in scratch whose control Package field is package.

    Used only as staging input for build_image.sh. The archive has no payload and is
    not a cham3 package. Missing dpkg-deb is BLOCKED, reported as a FAIL.
    """
    if not dpkg_deb_available():
        raise AssertionError(
            "BLOCKED (host environment): dpkg-deb is absent; run this suite in a "
            "debian:trixie container"
        )
    # The payload root lives outside scratch: build_image.sh globs *.deb in the
    # packages directory, and a root directory must never look like a .deb there.
    root = tempfile.mkdtemp(prefix="cham3_fixture_root_")
    try:
        debian = os.path.join(root, "DEBIAN")
        os.makedirs(debian)
        control = (
            "Package: %s\n"
            "Version: 0.0.0~fixture\n"
            "Architecture: all\n"
            "Maintainer: cham3 test fixture <fixture@invalid>\n"
            "Description: staging fixture, not a cham3 package\n"
            " Placeholder archive for build_image.sh staging tests. It has no payload.\n"
        ) % package
        with open(os.path.join(debian, "control"), "w", encoding="utf-8") as handle:
            handle.write(control)
        os.chmod(debian, 0o755)
        deb_path = os.path.join(scratch, filename)
        completed = m0_support.run_tool(
            ["dpkg-deb", "--build", "--root-owner-group", root, deb_path]
        )
        if completed.returncode != 0 or not os.path.isfile(deb_path):
            raise AssertionError(
                "dpkg-deb could not build fixture %s: %s" % (filename, completed.stderr.strip())
            )
        return deb_path
    finally:
        shutil.rmtree(root, ignore_errors=True)


def run_build_image_dryrun(repo, out_dir, live_dir=None):
    """Run the real build_image.sh in CHAM3_STAGE_DRYRUN mode; return the CompletedProcess.

    The environment is scrubbed of CHAM3_* variables so only the values passed here apply.
    A missing script or interpreter is a FAIL (not a skip).
    """
    script = os.path.join(repo, BuildImageScript)
    m0_support.require_file(script, "build_image entry point")
    env = {name: value for name, value in os.environ.items() if not name.startswith("CHAM3_")}
    env["CHAM3_OUT_DIR"] = out_dir
    env[StageDryRunVariable] = "1"
    if live_dir is not None:
        env["CHAM3_LIVE_DIR"] = live_dir
    argv = ["/bin/bash", script]
    try:
        return subprocess.run(
            argv,
            cwd=repo,
            env=env,
            capture_output=True,
            text=True,
            check=False,
            timeout=180,
        )
    except subprocess.TimeoutExpired:
        raise AssertionError("build_image.sh did not finish: " + " ".join(argv))


# F3 capture privilege state (agreed interface: wireshark group, file capabilities
# cap_net_admin,cap_net_raw=eip, and NO setuid bit when setcap is missing or fails).
DumpcapPath = "/usr/bin/dumpcap"
CaptureGroup = "wireshark"
CaptureCapabilities = frozenset({"cap_net_admin", "cap_net_raw"})
CaptureCapabilityFlags = "eip"
SetuidBit = 0o4000
GroupExecBit = 0o010
OtherExecBit = 0o001
CapabilitySpec = re.compile(r"^((?:cap_[a-z_]+)(?:,cap_[a-z_]+)*)[=+]([eip]+)$")
PreseedFile = LiveDesktopDir + "/config/preseed/cham3_wireshark.cfg.chroot"


def parse_capability_text(text):
    """Parse getcap output ('' or '<path> cap_a,cap_b=eip'). Returns (names, flags).

    Returns (None, None) when the text is present but not a recognized capability spec.
    """
    tokens = text.split()
    if not tokens:
        return frozenset(), ""
    match = CapabilitySpec.match(tokens[-1])
    if not match:
        return None, None
    return frozenset(match.group(1).split(",")), match.group(2)


def capture_state_problems(mode, group_name, capability_text, caps_expected):
    """Problems with the dumpcap state (empty means accepted).

    mode: st_mode & 0o7777. group_name: owning group. capability_text: getcap output.
    caps_expected: True for the setcap-success state (capabilities required) and
    False for the fail-closed state (setcap missing or failed: no setuid, no caps).
    The setuid bit is rejected in both states.
    """
    problems = []
    if mode & SetuidBit:
        problems.append("setuid bit set on dumpcap (mode %04o)" % mode)
    if group_name != CaptureGroup:
        problems.append("dumpcap group is %r, expected %r" % (group_name, CaptureGroup))
    if mode & OtherExecBit:
        problems.append("dumpcap is executable by other users (mode %04o)" % mode)
    names, flags = parse_capability_text(capability_text)
    if names is None:
        problems.append("unrecognized capability text: %r" % capability_text.strip())
        return problems
    if caps_expected:
        if names != CaptureCapabilities or flags != CaptureCapabilityFlags:
            problems.append(
                "capabilities must be cap_net_admin,cap_net_raw=eip, found %r"
                % capability_text.strip()
            )
        if not mode & GroupExecBit:
            problems.append("group %s cannot execute dumpcap (mode %04o)" % (CaptureGroup, mode))
    elif names:
        problems.append(
            "capabilities still set in the fail-closed state: %r" % capability_text.strip()
        )
    return problems


# Container-stage gating. Checks that change users, groups or dumpcap, or that need file
# capabilities, run only in a disposable container with root. Elsewhere they are NOT_RUN
# (skipped with the reason below) and count as a Codex Remote heavy gate.
NotRunGate = (
    "NOT_RUN: this check changes users, groups or /usr/bin/dumpcap and needs root and "
    "file capabilities in a disposable debian:trixie container; it is a Codex Remote "
    "heavy gate (tests/container/run_container_checks.sh)"
)


def capability_gate_problem():
    """None when this environment can run capability-dependent checks; else the NOT_RUN reason."""
    if os.geteuid() != 0:
        return NotRunGate + " [not root]"
    if shutil.which("setcap") is None or shutil.which("getcap") is None:
        return NotRunGate + " [setcap or getcap is absent]"
    probe_dir = tempfile.mkdtemp(prefix="cham3_capprobe_")
    try:
        probe = os.path.join(probe_dir, "probe")
        with open(probe, "w", encoding="utf-8") as handle:
            handle.write("probe\n")
        if subprocess.run(["setcap", "cap_net_raw+ep", probe], check=False).returncode != 0:
            return NotRunGate + " [file capabilities cannot be set here]"
        listing = subprocess.run(["getcap", probe], capture_output=True, text=True, check=False).stdout
        if "cap_net_raw" not in listing:
            return NotRunGate + " [file capabilities do not persist (no security.capability xattr)]"
        return None
    finally:
        shutil.rmtree(probe_dir, ignore_errors=True)


def shipped_deb(out_dir, package):
    """The one built <package>_*.deb under out_dir/packages (missing or ambiguous is a FAIL)."""
    matches = sorted(
        os.path.join(out_dir, "packages", name)
        for name in os.listdir(os.path.join(out_dir, "packages"))
        if name.startswith(package + "_") and name.endswith(".deb")
    ) if os.path.isdir(os.path.join(out_dir, "packages")) else []
    if len(matches) != 1:
        raise AssertionError(
            "BLOCKED: expected one built %s .deb in %s/packages, found %d; run "
            "build/scripts/build_packages.sh first" % (package, out_dir, len(matches))
        )
    return matches[0]


# F4 packaged-interface names (paths inside the image root, no leading slash).
# The image gate (test_m1_image_contents.py) uses these literals. The static
# cross-check (test_m1_static_packaged_paths.py) derives the same paths from the
# packaging sources: debian/rules (wallpaper image), debian/install (wallpaper
# metadata), and the 0086-cham3-sddm-wayland live-config component (session choice).
PackagedPlasmaSession = "usr/share/wayland-sessions/plasma.desktop"
PackagedWallpaperImage = "usr/share/wallpapers/Cham3/contents/images/3840x2160.png"
PackagedWallpaperMetadata = "usr/share/wallpapers/Cham3/metadata.json"
SddmSessionHook = LiveDesktopDir + "/config/includes.chroot/usr/lib/live/config/0086-cham3-sddm-wayland"
WallpaperRules = PackageDesktopDir + "/debian/rules"
WallpaperInstallList = PackageDesktopDir + "/debian/install"
