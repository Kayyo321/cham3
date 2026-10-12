"""Native APT check: the foreign default deny does not depend on signed Release identity (F6, SECURITY).

Codex review of 0fec3a0 (F6): the Kali and LibreWolf default-deny pins matched the
Release fields Origin=Kali and Codename=librewolf. A repository that was still validly
signed by the pinned key but carried a different Origin or Codename matched neither the
deny nor the allow rules, and every package in it got apt's ordinary priority 500.

This suite runs the real apt-get and apt-cache of the host (Debian 13) against synthetic
repositories. Each repository is signed with a disposable key generated for the run and
named through Signed-By, exactly like the candidate's sources. The candidate's source
files (build/live-desktop/config/archives/zzz_cham3_*.list) supply the host, path, suite
and component; its preference files (cham3_*.pref) are used unchanged. A local HTTP
proxy answers for the real host names (http.kali.org, repo.librewolf.net), so apt
records the configured host exactly as on the image. The only rewrite is https to http
for the LibreWolf URI, because the proxy does not terminate TLS; apt's host binding does
not depend on the scheme. No network is used, nothing is installed, and no real Kali or
LibreWolf key or signature is involved.

Every repository holds an allowlisted package (zaproxy or librewolf) and an unlisted
package, both at version 9.0. For each repository and each Release identity variant
(expected; Origin, Label, Suite or Codename changed; all four removed):
  - the allowlisted package is the candidate at priority 500 (positive control: the
    signed index was loaded, so a missing candidate below is not vacuous);
  - the unlisted package has priority -1 and no candidate (the F6 property).

Controls:
  - sensitivity: the superseded "Pin: release o=Kali" / "n=librewolf" rules from 0fec3a0,
    against the changed identity, give the unlisted package priority 500 and a candidate;
    this harness therefore detects the reviewed defect;
  - authentication: a repository signed by another key is rejected by apt and none of
    its packages becomes a candidate (Signed-By remains enforced);
  - host binding: the same repository configured under another host name is not
    covered by the pin. This is why every source of a pinned repository must use the
    pinned host; tests/desktop/test_m1_static_foreign_sources.py enforces that.

Requires apt-get, apt-cache and gpg (Debian 13). A missing tool is a FAIL.
"""

import email.utils
import hashlib
import http.server
import os
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import unittest
import urllib.parse

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m1_support  # noqa: E402
import m1_apt_support as apt  # noqa: E402

RequiredTools = ("apt-get", "apt-cache", "gpg", "gpgconf")
ProbeUnlisted = "cham3-unlisted-probe"
ProbeVersion = "9.0"
CommandTimeout = 300
ReleaseIdentityFields = ("Origin", "Label", "Suite", "Codename")
RepoSpecs = (
    {
        "name": "kali",
        "list": "zzz_cham3_kali.list",
        "pref": "cham3_kali.pref",
        "allowed": "zaproxy",
        "identity": {"Origin": "Kali", "Label": "Kali", "Suite": "kali-rolling", "Codename": "kali-rolling"},
        "superseded_pref": (
            "Package: *\nPin: release o=Kali\nPin-Priority: -1\n\n"
            "Package: zaproxy\nPin: release o=Kali\nPin-Priority: 500\n"
        ),
        "sensitivity_variant": "origin_changed",
    },
    {
        "name": "librewolf",
        "list": "zzz_cham3_librewolf.list",
        "pref": "cham3_librewolf.pref",
        "allowed": "librewolf",
        "identity": {"Origin": "LibreWolf", "Label": "LibreWolf", "Suite": "librewolf", "Codename": "librewolf"},
        "superseded_pref": (
            "Package: *\nPin: release n=librewolf\nPin-Priority: -1\n\n"
            "Package: librewolf\nPin: release n=librewolf\nPin-Priority: 500\n"
        ),
        "sensitivity_variant": "codename_changed",
    },
)
IdentityVariants = ("expected", "origin_changed", "label_changed", "suite_changed", "codename_changed",
                    "identity_removed")
VersionLine = re.compile(r"^\s+(?:\*\*\*\s+)?(\S+)\s+(-?\d+)\s*$")
SourceLine = re.compile(r"^\s+(-?\d+)\s+(\S+)\s+(\S+)")


def variant_identity(identity, variant):
    fields = dict(identity)
    if variant == "identity_removed":
        return {}
    if variant != "expected":
        name = variant.split("_", 1)[0].capitalize()
        fields[name] = fields[name] + "-changed"
    return fields


def run(argv, env=None, cwd=None):
    try:
        return subprocess.run(argv, capture_output=True, text=True, check=False, env=env, cwd=cwd,
                              timeout=CommandTimeout)
    except FileNotFoundError:
        raise AssertionError("required executable not found: " + argv[0]) from None
    except subprocess.TimeoutExpired:
        raise AssertionError("timed out: " + " ".join(argv)) from None


def checked(argv, env=None, cwd=None):
    completed = run(argv, env=env, cwd=cwd)
    if completed.returncode != 0:
        raise AssertionError("%s failed (%d): %s%s" % (" ".join(argv), completed.returncode,
                                                         completed.stdout, completed.stderr))
    return completed


def candidate_source(spec):
    """(uri, suite, components) of the candidate's single source entry for this repository."""
    text = m1_support.read_repo(m1_support.ArchivesDir + "/" + spec["list"])
    entries = [entry for entry in apt.parse_sources(text) if entry["types"] == ["deb"]]
    if len(entries) != 1 or len(entries[0]["uris"]) != 1 or len(entries[0]["suites"]) != 1:
        raise AssertionError("%s must hold exactly one deb entry, found %r" % (spec["list"], entries))
    entry = entries[0]
    return entry["uris"][0], entry["suites"][0], entry["components"]


class repository_proxy_t(http.server.ThreadingHTTPServer):
    """HTTP proxy that serves <serve_root>/<host>/<path> for absolute-URI requests."""

    daemon_threads = True
    serve_root = None


class repository_handler_t(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def resolve(self):
        parts = urllib.parse.urlsplit(self.path)
        if not parts.hostname or ".." in parts.path.split("/"):
            return None
        root = self.server.serve_root
        if root is None:
            return None
        path = os.path.join(root, parts.hostname, parts.path.lstrip("/"))
        return path if os.path.isfile(path) else None

    def answer(self, with_body):
        path = self.resolve()
        if path is None:
            self.send_error(404)
            return
        with open(path, "rb") as handle:
            body = handle.read()
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        if with_body:
            self.wfile.write(body)

    def do_GET(self):  # noqa: N802 (http.server interface name)
        self.answer(True)

    def do_HEAD(self):  # noqa: N802 (http.server interface name)
        self.answer(False)


def parse_policy(text, package):
    """{'candidate': str or None, 'versions': {version: priority}, 'sites': [uri ...]} for one package."""
    result = {"candidate": None, "versions": {}, "sites": []}
    lines = text.splitlines()
    inside = False
    for line in lines:
        if line.rstrip() == package + ":":
            inside = True
            continue
        if inside and line and not line[0].isspace():
            break
        if not inside:
            continue
        stripped = line.strip()
        if stripped.startswith("Candidate:"):
            value = stripped.split(":", 1)[1].strip()
            result["candidate"] = None if value == "(none)" else value
            continue
        source = SourceLine.match(line)
        if source and source.group(2).startswith(("http://", "https://", "file:")):
            result["sites"].append(source.group(2))
            continue
        version = VersionLine.match(line)
        if version:
            result["versions"][version.group(1)] = int(version.group(2))
    return result


class pin_identity_t(unittest.TestCase):
    """Builds every repository variant once; each test asserts one observation."""

    @classmethod
    def setUpClass(cls):
        missing = [tool for tool in RequiredTools if shutil.which(tool) is None]
        if missing:
            raise AssertionError("FAIL: required tools are absent: " + ", ".join(missing))
        cls.scratch = tempfile.mkdtemp(prefix="cham3_pin_identity_")
        os.chmod(cls.scratch, 0o755)
        cls.keys = {name: cls.make_key(name) for name in ("pinned", "unpinned")}
        cls.server = repository_proxy_t(("127.0.0.1", 0), repository_handler_t)
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()
        cls.observations = {}
        try:
            for spec in RepoSpecs:
                for variant in IdentityVariants:
                    cls.observe(spec, variant, "candidate", "pinned", None)
                cls.observe(spec, spec["sensitivity_variant"], "superseded", "pinned", None)
                cls.observe(spec, "expected", "candidate", "unpinned", None)
                cls.observe(spec, "expected", "candidate", "pinned", "mirror.cham3.invalid")
        except Exception:
            cls.tearDownClass()
            raise

    @classmethod
    def tearDownClass(cls):
        server = getattr(cls, "server", None)
        if server is not None:
            server.shutdown()
            server.server_close()
            cls.server = None
        for key in getattr(cls, "keys", {}).values():
            run(["gpgconf", "--kill", "gpg-agent"], env=dict(os.environ, GNUPGHOME=key["home"]))
        if getattr(cls, "scratch", None):
            shutil.rmtree(cls.scratch, ignore_errors=True)
            cls.scratch = None

    @classmethod
    def make_key(cls, label):
        home = os.path.join(cls.scratch, "gnupg-" + label)
        os.mkdir(home, 0o700)
        env = dict(os.environ, GNUPGHOME=home)
        checked(["gpg", "--batch", "--pinentry-mode", "loopback", "--passphrase", "", "--quick-generate-key",
                 "cham3 F6 disposable %s test key <f6-%s@cham3.invalid>" % (label, label),
                 "ed25519", "sign", "1d"], env=env)
        keyring = os.path.join(cls.scratch, "%s.gpg" % label)
        exported = subprocess.run(["gpg", "--batch", "--export"], capture_output=True, env=env, check=False,
                                  timeout=CommandTimeout)
        if exported.returncode != 0 or not exported.stdout:
            raise AssertionError("gpg --export failed for the %s key" % label)
        with open(keyring, "wb") as handle:
            handle.write(exported.stdout)
        os.chmod(keyring, 0o644)
        return {"home": home, "keyring": keyring}

    @classmethod
    def write_repository(cls, serve_root, host, path, suite, identity, signer, allowed):
        dist = os.path.join(serve_root, host, path.strip("/"), "dists", suite)
        packages_dir = os.path.join(dist, "main", "binary-amd64")
        os.makedirs(packages_dir)
        stanzas = []
        for name in (allowed, ProbeUnlisted):
            stanzas.append(
                "Package: %s\nVersion: %s\nArchitecture: amd64\nMaintainer: cham3 test <test@cham3.invalid>\n"
                "Description: inert F6 probe package\nFilename: pool/%s_%s_amd64.deb\nSize: 100\nSHA256: %s\n"
                % (name, ProbeVersion, name, ProbeVersion, "0" * 64)
            )
        packages = ("\n".join(stanzas)).encode()
        with open(os.path.join(packages_dir, "Packages"), "wb") as handle:
            handle.write(packages)
        release = ["%s: %s" % (field, identity[field]) for field in ReleaseIdentityFields if field in identity]
        release += [
            "Date: %s" % email.utils.formatdate(usegmt=True),
            "Architectures: amd64",
            "Components: main",
            "SHA256:",
            " %s %d main/binary-amd64/Packages" % (hashlib.sha256(packages).hexdigest(), len(packages)),
        ]
        release_path = os.path.join(dist, "Release")
        with open(release_path, "w", encoding="utf-8") as handle:
            handle.write("\n".join(release) + "\n")
        checked(["gpg", "--batch", "--yes", "--pinentry-mode", "loopback", "--passphrase", "", "--clearsign",
                 "--output", os.path.join(dist, "InRelease"), release_path],
                env=dict(os.environ, GNUPGHOME=signer["home"]))

    @classmethod
    def observe(cls, spec, variant, pref_kind, signer_label, served_host):
        """Configure one isolated apt root, update it and record apt-cache policy."""
        uri, suite, components = candidate_source(spec)
        parts = urllib.parse.urlsplit(uri)
        host = served_host or parts.hostname
        label = "%s-%s-%s-%s-%s" % (spec["name"], variant, pref_kind, signer_label, host)
        base = os.path.join(cls.scratch, label)
        serve_root = os.path.join(base, "serve")
        cls.write_repository(serve_root, host, parts.path, suite, variant_identity(spec["identity"], variant),
                             cls.keys[signer_label], spec["allowed"])
        root = os.path.join(base, "apt")
        for sub in ("etc/apt/preferences.d", "etc/apt/apt.conf.d", "etc/apt/trusted.gpg.d",
                    "etc/apt/sources.list.d", "var/lib/apt/lists/partial", "var/cache/apt/archives/partial",
                    "var/lib/dpkg"):
            os.makedirs(os.path.join(root, sub))
        open(os.path.join(root, "var/lib/dpkg/status"), "w").close()
        configured = urllib.parse.urlunsplit(("http", host, parts.path, "", ""))
        with open(os.path.join(root, "etc/apt/sources.list"), "w", encoding="utf-8") as handle:
            handle.write("deb [arch=amd64 signed-by=%s] %s %s %s\n"
                         % (cls.keys["pinned"]["keyring"], configured, suite, " ".join(components)))
        if pref_kind == "candidate":
            pref_text = m1_support.read_repo(m1_support.ArchivesDir + "/" + spec["pref"])
        else:
            pref_text = spec["superseded_pref"]
        with open(os.path.join(root, "etc/apt/preferences.d", spec["pref"]), "w", encoding="utf-8") as handle:
            handle.write(pref_text)
        config = os.path.join(root, "apt.conf")
        with open(config, "w", encoding="utf-8") as handle:
            handle.write(
                'Dir "%s/";\nDir::State::status "%s/var/lib/dpkg/status";\n'
                'APT::Architecture "amd64";\nAPT::Architectures { "amd64"; };\n'
                'Acquire::Languages "none";\nAcquire::http::Proxy "http://127.0.0.1:%d/";\n'
                'Acquire::IndexTargets::deb::Contents-deb::DefaultEnabled "false";\n'
                'APT::Get::List-Cleanup "false";\nAPT::Sandbox::User "root";\nDebug::NoLocking "true";\n'
                % (root, root, cls.server.server_address[1])
            )
        env = dict(os.environ, APT_CONFIG=config, LC_ALL="C")
        env.pop("http_proxy", None)
        env.pop("https_proxy", None)
        cls.server.serve_root = serve_root
        update = run(["apt-get", "-q", "update"], env=env)
        policy = run(["apt-cache", "policy", spec["allowed"], ProbeUnlisted], env=env)
        cls.observations[(spec["name"], variant, pref_kind, signer_label, served_host)] = {
            "update_rc": update.returncode,
            "update_output": update.stdout + update.stderr,
            "allowed": parse_policy(policy.stdout, spec["allowed"]),
            "unlisted": parse_policy(policy.stdout, ProbeUnlisted),
            "policy_output": policy.stdout + policy.stderr,
            "expected_site": configured.rstrip("/"),
        }

    def observation(self, *key):
        self.assertIn(key, self.observations, "scenario did not run: %r" % (key,))
        return self.observations[key]

    def assert_loaded(self, observed, spec):
        """Positive control: the signed index was read and the allowlisted package is the candidate at 500."""
        self.assertEqual(observed["update_rc"], 0, observed["update_output"])
        allowed = observed["allowed"]
        self.assertEqual(allowed["candidate"], ProbeVersion,
                         "%s is not the candidate; index not loaded?\n%s" % (spec["allowed"], observed["policy_output"]))
        self.assertEqual(allowed["versions"].get(ProbeVersion), apt.AllowedPriority, observed["policy_output"])
        self.assertTrue(any(site.rstrip("/") == observed["expected_site"] for site in allowed["sites"]),
                        "policy does not name the configured source %s:\n%s"
                        % (observed["expected_site"], observed["policy_output"]))

    def assert_denied(self, observed):
        unlisted = observed["unlisted"]
        self.assertIsNone(unlisted["candidate"], "unlisted package is installable:\n" + observed["policy_output"])
        self.assertEqual(unlisted["versions"].get(ProbeVersion), apt.DefaultDenyPriority, observed["policy_output"])

    def check_variant(self, spec, variant):
        observed = self.observation(spec["name"], variant, "candidate", "pinned", None)
        self.assert_loaded(observed, spec)
        self.assert_denied(observed)

    # The F6 property, per repository and Release identity variant.

    def test_kali_expected_identity_denies_unlisted(self):
        self.check_variant(RepoSpecs[0], "expected")

    def test_kali_changed_origin_still_denies_unlisted(self):
        self.check_variant(RepoSpecs[0], "origin_changed")

    def test_kali_changed_label_still_denies_unlisted(self):
        self.check_variant(RepoSpecs[0], "label_changed")

    def test_kali_changed_suite_still_denies_unlisted(self):
        self.check_variant(RepoSpecs[0], "suite_changed")

    def test_kali_changed_codename_still_denies_unlisted(self):
        self.check_variant(RepoSpecs[0], "codename_changed")

    def test_kali_removed_identity_still_denies_unlisted(self):
        self.check_variant(RepoSpecs[0], "identity_removed")

    def test_librewolf_expected_identity_denies_unlisted(self):
        self.check_variant(RepoSpecs[1], "expected")

    def test_librewolf_changed_origin_still_denies_unlisted(self):
        self.check_variant(RepoSpecs[1], "origin_changed")

    def test_librewolf_changed_label_still_denies_unlisted(self):
        self.check_variant(RepoSpecs[1], "label_changed")

    def test_librewolf_changed_suite_still_denies_unlisted(self):
        self.check_variant(RepoSpecs[1], "suite_changed")

    def test_librewolf_changed_codename_still_denies_unlisted(self):
        self.check_variant(RepoSpecs[1], "codename_changed")

    def test_librewolf_removed_identity_still_denies_unlisted(self):
        self.check_variant(RepoSpecs[1], "identity_removed")

    # Controls.

    def check_sensitivity(self, spec):
        observed = self.observation(spec["name"], spec["sensitivity_variant"], "superseded", "pinned", None)
        self.assertEqual(observed["update_rc"], 0, observed["update_output"])
        unlisted = observed["unlisted"]
        self.assertEqual(unlisted["candidate"], ProbeVersion,
                         "control failed: the superseded Release-field pin did not fail open here:\n"
                         + observed["policy_output"])
        self.assertEqual(unlisted["versions"].get(ProbeVersion), 500, observed["policy_output"])

    def test_control_superseded_kali_release_pin_fails_open(self):
        self.check_sensitivity(RepoSpecs[0])

    def test_control_superseded_librewolf_release_pin_fails_open(self):
        self.check_sensitivity(RepoSpecs[1])

    def check_authentication(self, spec):
        observed = self.observation(spec["name"], "expected", "candidate", "unpinned", None)
        self.assertRegex(observed["update_output"], r"NO_PUBKEY|not signed|signature|Signature|GPG error|OpenPGP",
                         "apt did not report the bad signature")
        self.assertIsNone(observed["allowed"]["candidate"], observed["policy_output"])
        self.assertIsNone(observed["unlisted"]["candidate"], observed["policy_output"])

    def test_control_kali_index_signed_by_another_key_is_rejected(self):
        self.check_authentication(RepoSpecs[0])

    def test_control_librewolf_index_signed_by_another_key_is_rejected(self):
        self.check_authentication(RepoSpecs[1])

    def check_host_binding(self, spec):
        observed = self.observation(spec["name"], "expected", "candidate", "pinned", "mirror.cham3.invalid")
        self.assertEqual(observed["update_rc"], 0, observed["update_output"])
        self.assertEqual(observed["unlisted"]["candidate"], ProbeVersion,
                         "control failed: a source under another host was covered by the pin:\n"
                         + observed["policy_output"])

    def test_control_kali_pin_is_bound_to_the_configured_host(self):
        self.check_host_binding(RepoSpecs[0])

    def test_control_librewolf_pin_is_bound_to_the_configured_host(self):
        self.check_host_binding(RepoSpecs[1])


class policy_parser_fixture_t(unittest.TestCase):
    """Positive and negative controls for the apt-cache policy parser used above."""

    Sample = (
        "zaproxy:\n  Installed: (none)\n  Candidate: 9.0\n  Version table:\n"
        "     9.0 500\n        500 http://http.kali.org/kali kali-rolling/main amd64 Packages\n"
        "cham3-unlisted-probe:\n  Installed: (none)\n  Candidate: (none)\n  Version table:\n"
        "     9.0 -1\n         -1 http://http.kali.org/kali kali-rolling/main amd64 Packages\n"
    )

    def test_candidate_and_priority_are_read(self):
        parsed = parse_policy(self.Sample, "zaproxy")
        self.assertEqual(parsed["candidate"], "9.0")
        self.assertEqual(parsed["versions"], {"9.0": 500})
        self.assertEqual(parsed["sites"], ["http://http.kali.org/kali"])

    def test_denied_package_has_no_candidate(self):
        parsed = parse_policy(self.Sample, ProbeUnlisted)
        self.assertIsNone(parsed["candidate"])
        self.assertEqual(parsed["versions"], {"9.0": -1})

    def test_unknown_package_is_empty(self):
        self.assertEqual(parse_policy(self.Sample, "absent"), {"candidate": None, "versions": {}, "sites": []})

    def test_variant_identity_changes_one_field(self):
        identity = {"Origin": "Kali", "Label": "Kali", "Suite": "kali-rolling", "Codename": "kali-rolling"}
        self.assertEqual(variant_identity(identity, "origin_changed")["Origin"], "Kali-changed")
        self.assertEqual(variant_identity(identity, "origin_changed")["Label"], "Kali")
        self.assertEqual(variant_identity(identity, "identity_removed"), {})
        self.assertEqual(variant_identity(identity, "expected"), identity)


if __name__ == "__main__":
    unittest.main()
