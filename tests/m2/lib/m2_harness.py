"""M2 test harness: strict execution and validation of Rust and Python test suites.

A successful exit status is not accepted as evidence on its own. Every run must:

* execute in a fresh run directory (an existing directory is refused as stale);
* actually execute tests: compile-only (cargo test --no-run) output, a suite with
  zero tests, or a missing per-suite result line fails;
* report zero failed, ignored, measured and filtered tests and a zero exit status;
* execute exactly the test names listed in tests/m2/test_inventory.txt for that
  suite: a missing or an unexpected test fails, so silently dropped tests and
  unreviewed additions are both visible;
* record the commit, tree, dirty state, toolchain identity, command, exit status and
  raw output under the run directory.

Usage (run with python3 -I):
  m2_harness.py cargo-suite --profile debug|release --run-dir DIR
  m2_harness.py python-suite --name NAME --start-dir DIR --run-dir DIR
  m2_harness.py provider-oracle --probe BINARY --run-dir DIR
  m2_harness.py environment --run-dir DIR
"""

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys

RepoRoot = os.environ.get(
    "CHAM3_REPO_ROOT",
    os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))),
)
InventoryPath = os.path.join(RepoRoot, "tests", "m2", "test_inventory.txt")

CargoRunningPattern = re.compile(
    r"^\s*Running (?:unittests |tests/|benches/)?(?P<source>\S+) \((?P<binary>[^)]+)\)\s*$"
)
CargoDocTestsPattern = re.compile(r"^\s*Doc-tests (?P<crate>\S+)\s*$")
CargoExecutablePattern = re.compile(r"^\s*Executable ")
RunningCountPattern = re.compile(r"^running (?P<count>\d+) tests?$")
TestLinePattern = re.compile(r"^test (?P<name>\S+) \.\.\. (?P<outcome>ok|FAILED|ignored.*)$")
CargoResultPattern = re.compile(
    r"^test result: (?P<status>ok|FAILED)\. (?P<passed>\d+) passed; (?P<failed>\d+) failed; "
    r"(?P<ignored>\d+) ignored; (?P<measured>\d+) measured; (?P<filtered>\d+) filtered out;"
)
PythonRanPattern = re.compile(r"^Ran (?P<count>\d+) tests? in ")


class harness_error_t(Exception):
    """A validation failure; the message names the rule that failed."""


def binary_stem(binary_path):
    name = os.path.basename(binary_path)
    stem, separator, suffix = name.rpartition("-")
    if separator and re.fullmatch(r"[0-9a-f]{16}", suffix):
        return stem
    return name


def load_inventory(path=InventoryPath):
    """Returns {suite_key: set(test names)} from the inventory file."""
    inventory = {}
    with open(path, "r", encoding="utf-8") as handle:
        for number, raw in enumerate(handle, start=1):
            line = raw.strip()
            if not line or line.startswith("#"):
                continue
            parts = line.split()
            if len(parts) != 3:
                raise harness_error_t("inventory line %d malformed: %r" % (number, raw))
            kind, suite, name = parts
            key = "%s:%s" % (kind, suite)
            names = inventory.setdefault(key, set())
            if name in names:
                raise harness_error_t("inventory line %d duplicates %s %s" % (number, key, name))
            names.add(name)
    return inventory


def parse_cargo_test_output(text):
    """Parses merged cargo test stdout/stderr into suites.

    Returns a list of dicts with keys: key, source, binary, running, tests, result.
    Raises harness_error_t for structural problems.
    """
    suites = []
    current = None
    saw_executable = False
    for line in text.splitlines():
        if CargoExecutablePattern.match(line):
            saw_executable = True
            continue
        running = CargoRunningPattern.match(line)
        if running:
            if current is not None and current["result"] is None:
                raise harness_error_t("suite %s has no result line" % current["key"])
            current = {
                "key": "cargo:" + binary_stem(running.group("binary")),
                "source": running.group("source"),
                "binary": running.group("binary"),
                "running": None,
                "tests": {},
                "result": None,
            }
            suites.append(current)
            continue
        doc_tests = CargoDocTestsPattern.match(line)
        if doc_tests:
            if current is not None and current["result"] is None:
                raise harness_error_t("suite %s has no result line" % current["key"])
            current = {
                "key": "cargo-doc:" + doc_tests.group("crate"),
                "source": "doc-tests",
                "binary": "",
                "running": None,
                "tests": {},
                "result": None,
            }
            suites.append(current)
            continue
        if current is None:
            if TestLinePattern.match(line) or CargoResultPattern.match(line):
                raise harness_error_t("test output outside any suite: %r" % line)
            continue
        count = RunningCountPattern.match(line)
        if count and current["running"] is None and current["result"] is None:
            current["running"] = int(count.group("count"))
            continue
        test_line = TestLinePattern.match(line)
        if test_line and current["result"] is None:
            name = test_line.group("name")
            if name in current["tests"]:
                raise harness_error_t("suite %s reports %s twice" % (current["key"], name))
            current["tests"][name] = test_line.group("outcome")
            continue
        result = CargoResultPattern.match(line)
        if result and current["result"] is None:
            current["result"] = {
                key: (value if key == "status" else int(value)) for key, value in result.groupdict().items()
            }
    if current is not None and current["result"] is None:
        raise harness_error_t("suite %s has no result line" % current["key"])
    if saw_executable and not suites:
        raise harness_error_t("compile-only output (cargo test --no-run): no tests executed")
    return suites


def validate_suites(suites, exit_status, inventory, kind):
    """Applies every acceptance rule. Returns the executed {key: sorted names}."""
    errors = []
    if exit_status != 0:
        errors.append("non-zero exit status %r" % (exit_status,))
    if not suites:
        errors.append("no test suites executed")
    executed = {}
    for suite in suites:
        key = suite["key"]
        result = suite["result"] or {}
        if key.startswith("cargo-doc:"):
            errors.append("%s: doc-test suite present (doctests must be disabled)" % key)
        if suite["running"] is None:
            errors.append("%s: missing 'running N tests' line" % key)
        if result.get("status") != "ok":
            errors.append("%s: result status %r" % (key, result.get("status")))
        for field in ("failed", "ignored", "measured", "filtered"):
            if result.get(field, 0) != 0:
                errors.append("%s: %d %s" % (key, result[field], field))
        passed = result.get("passed", 0)
        if passed == 0:
            errors.append("%s: zero tests passed" % key)
        if suite["running"] is not None and suite["running"] != passed:
            errors.append("%s: running %d but passed %d" % (key, suite["running"], passed))
        outcomes = suite["tests"]
        not_ok = sorted(name for name, outcome in outcomes.items() if outcome != "ok")
        if not_ok:
            errors.append("%s: tests not ok: %s" % (key, ", ".join(not_ok)))
        if len(outcomes) != passed:
            errors.append("%s: %d test lines but %d passed" % (key, len(outcomes), passed))
        if key in executed:
            errors.append("%s: suite executed twice" % key)
        executed[key] = sorted(outcomes)
    expected = {key: names for key, names in inventory.items() if key.startswith(kind + ":")}
    for key in sorted(set(expected) | set(executed)):
        want = expected.get(key, set())
        have = set(executed.get(key, []))
        missing = sorted(want - have)
        extra = sorted(have - want)
        if missing:
            errors.append("%s: inventory tests not executed: %s" % (key, ", ".join(missing)))
        if extra:
            errors.append("%s: executed tests missing from inventory: %s" % (key, ", ".join(extra)))
    if errors:
        raise harness_error_t("; ".join(errors))
    return executed


def parse_python_unittest_output(text):
    """Parses `python3 -m unittest -v` output into one suite."""
    tests = {}
    ran = None
    status = None
    pending = None
    for line in text.splitlines():
        match = re.match(r"^(?P<method>test\w*) \((?P<qualified>[\w.]+)\)", line)
        if match:
            name = match.group("qualified")
            if not name.endswith("." + match.group("method")):
                name = name + "." + match.group("method")
            pending = name
        if pending is not None and " ... " in line:
            outcome = line.rsplit(" ... ", 1)[1].strip()
            if pending in tests:
                raise harness_error_t("python test %s reported twice" % pending)
            tests[pending] = outcome
            pending = None
            continue
        ran_match = PythonRanPattern.match(line)
        if ran_match:
            ran = int(ran_match.group("count"))
            continue
        if line.startswith("OK") or line.startswith("FAILED"):
            status = line.strip()
    return {"tests": tests, "ran": ran, "status": status}


def validate_python_suite(parsed, exit_status, inventory, suite_name):
    errors = []
    key = "python:" + suite_name
    if exit_status != 0:
        errors.append("non-zero exit status %r" % (exit_status,))
    if parsed["ran"] is None:
        errors.append("no 'Ran N tests' line")
    elif parsed["ran"] == 0:
        errors.append("zero tests ran")
    if parsed["status"] != "OK":
        errors.append("unittest status %r (skips, failures and errors are not accepted)" % parsed["status"])
    not_ok = sorted(name for name, outcome in parsed["tests"].items() if outcome != "ok")
    if not_ok:
        errors.append("tests not ok: " + ", ".join(not_ok))
    if parsed["ran"] is not None and parsed["ran"] != len(parsed["tests"]):
        errors.append("ran %d but %d test lines" % (parsed["ran"], len(parsed["tests"])))
    want = inventory.get(key, set())
    have = set(parsed["tests"])
    if sorted(want - have):
        errors.append("inventory tests not executed: " + ", ".join(sorted(want - have)))
    if sorted(have - want):
        errors.append("executed tests missing from inventory: " + ", ".join(sorted(have - want)))
    if errors:
        raise harness_error_t("%s: %s" % (key, "; ".join(errors)))
    return sorted(have)


def command_output(command, **kwargs):
    completed = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, check=False, **kwargs)
    return completed.returncode, completed.stdout


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 16), b""):
            digest.update(block)
    return digest.hexdigest()


HexObjectPattern = re.compile(r"^[0-9a-f]{40}$")
Sha256HexPattern = re.compile(r"^[0-9a-f]{64}$")


def environment_record():
    """Collects the run identity. Returns (record, problems).

    Required evidence: commit and tree ids, tracked-file status, rustc/cargo identity,
    Debian version, dpkg architecture and the Cargo.lock digest. A failed or malformed
    collection is a problem, never a dirty or clean state: missing identity cannot be
    repaired by later test success (review finding R01).
    """
    record = {}
    problems = []
    for name, command in (
        ("git_head", ["git", "-C", RepoRoot, "rev-parse", "--verify", "HEAD"]),
        ("git_tree", ["git", "-C", RepoRoot, "rev-parse", "--verify", "HEAD^{tree}"]),
        ("git_status", ["git", "-C", RepoRoot, "status", "--porcelain", "--untracked-files=no"]),
        ("rustc", ["rustc", "-Vv"]),
        ("cargo", ["cargo", "-V"]),
        ("uname", ["uname", "-srmo"]),
        ("debian_version", ["cat", "/etc/debian_version"]),
        ("dpkg_architecture", ["dpkg", "--print-architecture"]),
    ):
        try:
            status, output = command_output(command)
        except OSError as error:
            status, output = None, "cannot execute: %s" % error
        record[name] = {"status": status, "output": output.strip()}
        if status != 0:
            problems.append("%s: command failed (status %r)" % (name, status))
    for name in ("git_head", "git_tree"):
        if record[name]["status"] == 0 and not HexObjectPattern.match(record[name]["output"]):
            problems.append("%s: not a 40-hex object id" % name)
    if record["rustc"]["status"] == 0 and not re.search(r"(?m)^release: \S+$", record["rustc"]["output"]):
        problems.append("rustc: no release line")
    if record["cargo"]["status"] == 0 and not record["cargo"]["output"].startswith("cargo "):
        problems.append("cargo: unexpected version output")
    for name in ("uname", "debian_version", "dpkg_architecture"):
        if record[name]["status"] == 0 and not record[name]["output"]:
            problems.append("%s: empty output" % name)
    # Dirty state is only meaningful when the status command itself succeeded.
    record["git_dirty"] = bool(record["git_status"]["output"]) if record["git_status"]["status"] == 0 else None
    # R01-r2: an unreadable lock or a failed/malformed digest is a problem, never a value.
    lock = os.path.join(RepoRoot, "Cargo.lock")
    record["cargo_lock_sha256"] = None
    if not os.path.isfile(lock):
        problems.append("Cargo.lock missing")
    else:
        try:
            digest = sha256_file(lock)
        except OSError as error:
            problems.append("Cargo.lock digest failed: %s" % error.strerror)
        else:
            if Sha256HexPattern.match(digest):
                record["cargo_lock_sha256"] = digest
            else:
                problems.append("Cargo.lock digest malformed")
    record["euid"] = os.geteuid()
    return record, problems


def fresh_run_dir(path):
    if os.path.lexists(path) and os.listdir(path):
        raise harness_error_t("run directory %s already exists and is not empty (stale output)" % path)
    os.makedirs(path, exist_ok=True)


def write_json(path, value):
    with open(path, "w", encoding="utf-8") as handle:
        json.dump(value, handle, indent=2, sort_keys=True)
        handle.write("\n")


def run_cargo_suite(profile, run_dir):
    target_dir = os.path.join(run_dir, "target_" + profile)
    if os.path.lexists(target_dir):
        raise harness_error_t("cargo target directory %s already exists (stale build)" % target_dir)
    command = ["cargo", "test", "--workspace", "--frozen"]
    if profile == "release":
        command.append("--release")
    elif profile != "debug":
        raise harness_error_t("unknown profile %r" % profile)
    environment = dict(os.environ, CARGO_TARGET_DIR=target_dir)
    environment.pop("RUSTFLAGS", None)
    status, output = command_output(command, cwd=RepoRoot, env=environment)
    log_path = os.path.join(run_dir, "cargo_test_%s.log" % profile)
    with open(log_path, "w", encoding="utf-8") as handle:
        handle.write(output)
    record = {"command": command, "cargo_target_dir": target_dir, "exit_status": status, "log": log_path}
    try:
        suites = parse_cargo_test_output(output)
        executed = validate_suites(suites, status, load_inventory(), "cargo")
        record["result"] = "PASS"
        record["suites"] = {key: len(names) for key, names in executed.items()}
        record["total_tests"] = sum(record["suites"].values())
        record["tests"] = executed
    except harness_error_t as error:
        record["result"] = "FAIL"
        record["error"] = str(error)
    write_json(os.path.join(run_dir, "cargo_test_%s.json" % profile), record)
    return record


def run_python_suite(name, start_dir, run_dir):
    command = [sys.executable, "-I", "-m", "unittest", "discover", "-v", "-s", start_dir, "-p", "test_*.py"]
    status, output = command_output(command, cwd=RepoRoot, env=dict(os.environ, CHAM3_M2_RUN_DIR=run_dir))
    log_path = os.path.join(run_dir, "python_%s.log" % name)
    with open(log_path, "w", encoding="utf-8") as handle:
        handle.write(output)
    record = {"command": command, "exit_status": status, "log": log_path}
    try:
        executed = validate_python_suite(parse_python_unittest_output(output), status, load_inventory(), name)
        record["result"] = "PASS"
        record["total_tests"] = len(executed)
        record["tests"] = executed
    except harness_error_t as error:
        record["result"] = "FAIL"
        record["error"] = str(error)
    write_json(os.path.join(run_dir, "python_%s.json" % name), record)
    return record


def run_provider_oracle(probe, run_dir):
    vectors = os.path.join(RepoRoot, "tests", "m2", "vectors", "provider_kat.json")
    oracle = os.path.join(RepoRoot, "tests", "m2", "oracle", "provider_oracle.py")
    probe_status, probe_output = command_output([probe, "report"])
    oracle_status, oracle_output = command_output([sys.executable, "-I", oracle, vectors])
    record = {
        "probe": probe,
        "probe_exit_status": probe_status,
        "probe_output": probe_output.splitlines(),
        "oracle_exit_status": oracle_status,
        "oracle_output": oracle_output.splitlines(),
    }
    errors = []
    if probe_status != 0 or any(line.startswith("FAIL") for line in record["probe_output"]):
        errors.append("probe reported failure")
    if oracle_status != 0 or any(line.startswith("FAIL") for line in record["oracle_output"]):
        errors.append("oracle reported failure")
    probe_passes = [line for line in record["probe_output"] if line.startswith("PASS ")]
    oracle_passes = [line for line in record["oracle_output"] if line.startswith("PASS ")]
    if len(probe_passes) != 7:
        errors.append("probe passed %d checks, expected 7" % len(probe_passes))
    if len(oracle_passes) != 7:
        errors.append("oracle passed %d checks, expected 7" % len(oracle_passes))
    probe_argon = [line for line in record["probe_output"] if line.startswith("ARGON2ID_ORACLE_CASE ")]
    oracle_argon = [line for line in record["oracle_output"] if line.startswith("ARGON2ID_ORACLE_CASE ")]
    if len(probe_argon) != 1 or probe_argon != oracle_argon:
        errors.append("Argon2id oracle case differs: probe %r oracle %r" % (probe_argon, oracle_argon))
    record["result"] = "FAIL" if errors else "PASS"
    if errors:
        record["error"] = "; ".join(errors)
    write_json(os.path.join(run_dir, "provider_oracle.json"), record)
    return record


def main(arguments):
    parser = argparse.ArgumentParser(prog="m2_harness.py")
    commands = parser.add_subparsers(dest="command", required=True)
    cargo = commands.add_parser("cargo-suite")
    cargo.add_argument("--profile", required=True, choices=["debug", "release"])
    cargo.add_argument("--run-dir", required=True)
    python = commands.add_parser("python-suite")
    python.add_argument("--name", required=True)
    python.add_argument("--start-dir", required=True)
    python.add_argument("--run-dir", required=True)
    oracle = commands.add_parser("provider-oracle")
    oracle.add_argument("--probe", required=True)
    oracle.add_argument("--run-dir", required=True)
    environment = commands.add_parser("environment")
    environment.add_argument("--run-dir", required=True)
    options = parser.parse_args(arguments)

    os.makedirs(options.run_dir, exist_ok=True)
    try:
        if options.command == "cargo-suite":
            record = run_cargo_suite(options.profile, options.run_dir)
        elif options.command == "python-suite":
            record = run_python_suite(options.name, os.path.join(RepoRoot, options.start_dir), options.run_dir)
        elif options.command == "provider-oracle":
            record = run_provider_oracle(options.probe, options.run_dir)
        else:
            environment, problems = environment_record()
            record = {"result": "FAIL" if problems else "PASS", "environment": environment}
            if problems:
                record["error"] = "; ".join(problems)
            write_json(os.path.join(options.run_dir, "environment.json"), record)
    except harness_error_t as error:
        record = {"result": "FAIL", "error": str(error)}
        write_json(os.path.join(options.run_dir, "%s_error.json" % options.command), record)
    summary = record.get("result")
    detail = record.get("error") or "%s tests" % record.get("total_tests", "-")
    print("%s %s %s" % (summary, options.command, detail))
    return 0 if summary == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
