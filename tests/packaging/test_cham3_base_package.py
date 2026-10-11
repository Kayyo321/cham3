"""Checks for the built cham3-base .deb.

check_package() is the single checker. It runs on the real package from
$CHAM3_OUT_DIR/packages and on controlled fixtures: a well-formed fixture that
must be accepted (positive control) and a deliberately defective package that
must be rejected (negative control).
"""

import os
import re
import sys
import tempfile
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402

DocDir = "usr/share/doc/cham3-base"
AllowedDirectories = frozenset({"usr", "usr/share", "usr/share/doc", DocDir})
AllowedControlMembers = frozenset({"control", "md5sums"})
RequiredDocFiles = (DocDir + "/copyright", DocDir + "/README.Debian")
RequiredChangelogFiles = (DocDir + "/changelog.Debian.gz", DocDir + "/changelog.gz")
RequiredDescriptionPhrases = (
    "m0",
    "development skeleton",
    "no services",
    "security policy",
    "enforcement",
    "system configuration",
    "image integration",
)
ContentsLine = re.compile(r"^(\S+)\s+\S+\s+\d+\s+\S+\s+\S+\s+(.+)$")
DescriptionText = (
    "M0 development skeleton\n"
    " This package is an M0 development skeleton. It installs no services,\n"
    " security policy, enforcement or system configuration, and exists only\n"
    " to validate packaging and image integration."
)
CopyrightText = (
    "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n"
    "Upstream-Name: cham3\n\n"
    "Files: *\n"
    "License: AGPL-3\n"
    " See /usr/share/common-licenses or the repository LICENSE file.\n"
    " PROJECT.md: license to be selected before public release.\n"
)


def check_package(deb_path, expected_version):
    """Return a list of problems with the package; an empty list means accepted."""
    problems = []
    info = m0_support.run_tool(["dpkg-deb", "--info", deb_path])
    if info.returncode != 0:
        return ["dpkg-deb --info rejected the archive: " + info.stderr.strip()]

    control_text = m0_support.run_tool(["dpkg-deb", "--field", deb_path]).stdout
    stanzas = m0_support.parse_control(control_text)
    if len(stanzas) != 1:
        return ["control data must contain exactly one stanza"]
    fields = stanzas[0]

    if fields.get("Package") != m0_support.ExpectedPackage:
        problems.append("Package is not %s" % m0_support.ExpectedPackage)
    if fields.get("Version") != expected_version:
        problems.append("Version %r does not equal %r" % (fields.get("Version"), expected_version))
    if fields.get("Architecture") != "all":
        problems.append("Architecture must be all")
    if fields.get("Depends", "").strip():
        problems.append("Depends must be empty, found: " + fields["Depends"])
    if "Pre-Depends" in fields:
        problems.append("Pre-Depends must be absent")

    addresses = re.findall(r"<([^>]+)>", fields.get("Maintainer", ""))
    if not addresses or not all(m0_support.email_domain_is_reserved(a) for a in addresses):
        problems.append("Maintainer is not a reserved placeholder address")

    description = m0_support.normalize_space(fields.get("Description", ""))
    for phrase in RequiredDescriptionPhrases:
        if phrase not in description:
            problems.append("Description lacks disclaimer phrase: " + phrase)
    if "kde" in description or "plasma" in description:
        problems.append("Description mentions desktop components")

    with tempfile.TemporaryDirectory() as temp:
        extracted = m0_support.run_tool(["dpkg-deb", "--control", deb_path, temp])
        if extracted.returncode != 0:
            problems.append("dpkg-deb --control failed: " + extracted.stderr.strip())
        else:
            members = set(os.listdir(temp))
            for member in sorted(members - AllowedControlMembers):
                problems.append("control member not allowed: " + member)
            if "control" not in members:
                problems.append("control member missing")

    contents = m0_support.run_tool(["dpkg-deb", "--contents", deb_path])
    if contents.returncode != 0:
        return problems + ["dpkg-deb --contents failed: " + contents.stderr.strip()]
    found_files = set()
    for line in contents.stdout.splitlines():
        match = ContentsLine.match(line.strip())
        if not match:
            problems.append("unparsed contents line: " + line)
            continue
        kind = match.group(1)[0]
        path = match.group(2).split(" -> ", 1)[0]
        path = path[2:] if path.startswith("./") else path
        path = path.rstrip("/")
        if path in ("", "."):
            continue
        if kind == "d":
            if path not in AllowedDirectories and not path.startswith(DocDir + "/"):
                problems.append("directory outside doc dir: " + path)
            continue
        if kind != "-":
            problems.append("non-regular entry is not allowed: " + path)
        if not path.startswith(DocDir + "/"):
            problems.append("file outside %s: %s" % (DocDir, path))
        found_files.add(path)

    for required in RequiredDocFiles:
        if required not in found_files:
            problems.append("required doc file missing: " + required)
    if not any(name in found_files for name in RequiredChangelogFiles):
        problems.append("required changelog doc file missing")
    return problems


def stage_package(directory, name, scripts=None, files=None, depends=None):
    root = os.path.join(directory, "stage")
    os.makedirs(os.path.join(root, "DEBIAN"))
    os.chmod(os.path.join(root, "DEBIAN"), 0o755)
    control_lines = [
        "Package: " + m0_support.ExpectedPackage,
        "Version: " + m0_support.ExpectedVersion,
        "Architecture: all",
        "Maintainer: cham3 developers <maintainers@cham3.invalid>",
        "Description: " + DescriptionText,
    ]
    if depends:
        control_lines.append("Depends: " + depends)
    with open(os.path.join(root, "DEBIAN", "control"), "w", encoding="utf-8") as handle:
        handle.write("\n".join(control_lines) + "\n")
    for script_name, content in (scripts or {}).items():
        path = os.path.join(root, "DEBIAN", script_name)
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(content)
        os.chmod(path, 0o755)
    for relative, content in (files or {}).items():
        path = os.path.join(root, relative)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(content)
    deb_file = os.path.join(directory, name)
    built = m0_support.run_tool(
        ["dpkg-deb", "--build", "--root-owner-group", root, deb_file]
    )
    if built.returncode != 0:
        raise AssertionError("dpkg-deb --build failed: " + built.stderr.strip())
    return deb_file


def doc_files():
    return {
        DocDir + "/copyright": CopyrightText,
        DocDir + "/README.Debian": "M0 development skeleton. No services.\n",
        DocDir + "/changelog.Debian.gz": "placeholder\n",
    }


def real_deb_path():
    path = os.path.join(m0_support.out_dir(), "packages", m0_support.DebFileName)
    m0_support.require_file(path, "run build/scripts/build_packages.sh first")
    return path


class cham3_base_package_t(unittest.TestCase):
    def test_real_package_passes_checker(self):
        problems = check_package(real_deb_path(), m0_support.ExpectedVersion)
        self.assertEqual(problems, [], "real package rejected")

    def test_well_formed_fixture_is_accepted(self):
        with tempfile.TemporaryDirectory() as temp:
            deb = stage_package(temp, "fixture.deb", files=doc_files())
            problems = check_package(deb, m0_support.ExpectedVersion)
        self.assertEqual(problems, [], "positive control rejected by checker")

    def test_defective_package_is_rejected(self):
        scripts = {
            "postinst": "#!/bin/sh\nexit 0\n",
            "conffiles": "/etc/cham3/defect.conf\n",
        }
        files = dict(doc_files())
        files["etc/cham3/defect.conf"] = "enabled=true\n"
        files["lib/systemd/system/cham3-defect.service"] = "[Unit]\nDescription=defect\n"
        with tempfile.TemporaryDirectory() as temp:
            deb = stage_package(
                temp, "defective.deb", scripts=scripts, files=files, depends="libc6"
            )
            problems = check_package(deb, m0_support.ExpectedVersion)
        joined = "\n".join(problems)
        for needle in ("postinst", "conffiles", "etc/cham3", "lib/systemd", "Depends"):
            self.assertIn(needle, joined, "checker missed defect: " + needle)

    def test_wrong_expected_version_is_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            deb = stage_package(temp, "fixture.deb", files=doc_files())
            problems = check_package(deb, "9.9.9~wrong")
        self.assertTrue(
            any("Version" in problem for problem in problems),
            "version mismatch not reported",
        )


if __name__ == "__main__":
    unittest.main()
