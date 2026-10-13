#!/bin/sh
# Run one M2 test suite with strict result validation.
#
# Usage: tests/m2/run_m2_tests.sh unit|integration|recovery|packaging|harness
#
# Suites:
#   unit       cargo test (debug and release, overflow checks on in both), each
#              validated against tests/m2/test_inventory.txt, then the provider
#              probe compared with the independent oracle, the published ECR v1
#              vectors regenerated and re-verified by the independent ECR oracle,
#              and the M2-003 format verifier compared with that oracle over a
#              seeded hostile corpus (format_differential); the vault's encrypted
#              key objects against an independent libargon2/libsodium oracle
#              (vault_object_oracle, M2-004).
#   harness    harness sensitivity, safe-core lint controls, naming checker and
#              deployed-profile (panic=abort, overflow trap, hardening) probes.
#   integration, recovery, packaging
#              not implemented at this commit (gated M2-005..M2-007); they FAIL
#              with NOT_RUN rather than reporting success.
#
# Environment:
#   CHAM3_REPO_ROOT          repository root (default: two levels above this file)
#   CHAM3_OUT_DIR            output root (default: <repo>/out); runs go to
#                            $CHAM3_OUT_DIR/m2/runs/<suite>_<UTC time>_<pid>
#   CHAM3_M2_REQUIRE_CLEAN   if 1, fail when tracked files differ from HEAD
#                            (identity failures always fail; review finding R01)
#
# Exit status: 0 only if every step of the suite passed; 1 on any failure, missing
# prerequisite, stale output or unimplemented suite; 2 on usage error. Run
# unprivileged inside the pinned Debian 13 container (build/m2/dev_container.dockerfile).
set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
CHAM3_REPO_ROOT=${CHAM3_REPO_ROOT:-$(CDPATH='' cd -- "$script_dir/../.." && pwd)}
CHAM3_OUT_DIR=${CHAM3_OUT_DIR:-$CHAM3_REPO_ROOT/out}
export CHAM3_REPO_ROOT CHAM3_OUT_DIR
# Cargo reads .cargo/config.toml (the offline Debian registry source) from the
# working directory, so every Cargo command runs from the repository root.
cd "$CHAM3_REPO_ROOT"

usage() {
    echo "usage: $0 unit|integration|recovery|packaging|harness" >&2
}

if [ "$#" -ne 1 ]; then
    usage
    exit 2
fi
suite=$1
case "$suite" in
    unit | harness) ;;
    integration | recovery | packaging)
        echo "NOT_RUN $suite: suite not implemented at this commit (gated M2 work package)"
        echo "m2 $suite: FAIL"
        exit 1
        ;;
    *)
        usage
        exit 2
        ;;
esac

for tool in cargo rustc python3 git; do
    if ! command -v "$tool" > /dev/null 2>&1; then
        echo "missing prerequisite: $tool" >&2
        echo "m2 $suite: FAIL"
        exit 1
    fi
done

if [ "$(id -u)" = "0" ]; then
    echo "refusing to run M2 tests as root (privilege changes test meaning)" >&2
    echo "m2 $suite: FAIL"
    exit 1
fi

identity_args=""
if [ "${CHAM3_M2_REQUIRE_CLEAN:-0}" = "1" ]; then
    identity_args=--require-clean
fi

run_dir=$CHAM3_OUT_DIR/m2/runs/${suite}_$(date -u +%Y%m%dT%H%M%SZ)_$$
if [ -e "$run_dir" ]; then
    echo "stale run directory exists: $run_dir" >&2
    echo "m2 $suite: FAIL"
    exit 1
fi
mkdir -p "$run_dir"
harness=$CHAM3_REPO_ROOT/tests/m2/lib/m2_harness.py
summary=$run_dir/summary.txt
overall=0

step() { # name command...
    name=$1
    shift
    if "$@"; then
        echo "PASS $name" >> "$summary"
    else
        echo "FAIL $name" >> "$summary"
        overall=1
    fi
}

# Invoked indirectly through step "$@", which shellcheck cannot follow.
# shellcheck disable=SC2317
run_ecr_oracle() {
    python3 -I "$CHAM3_REPO_ROOT/tests/m2/oracle/ecr_oracle.py" check \
        "$CHAM3_REPO_ROOT/tests/m2/vectors/ecr_v1/vectors.json" > "$run_dir/ecr_vector_oracle.log" 2>&1
}

# shellcheck disable=SC2317
run_vault_object_oracle() {
    python3 -I "$CHAM3_REPO_ROOT/tests/m2/oracle/vault_object_oracle.py" \
        --probe "$run_dir/target_debug/debug/cham3_vault_probe" > "$run_dir/vault_object_oracle.log" 2>&1
}

# Source identity must be complete (and clean when required); a failure is FAIL.
# shellcheck disable=SC2086
step source_identity sh "$CHAM3_REPO_ROOT/build/scripts/m2_source_identity.sh" "$run_dir/identity.txt" $identity_args
step environment python3 -I "$harness" environment --run-dir "$run_dir"

case "$suite" in
    unit)
        step cargo_test_debug python3 -I "$harness" cargo-suite --profile debug --run-dir "$run_dir"
        step cargo_test_release python3 -I "$harness" cargo-suite --profile release --run-dir "$run_dir"
        # Reuses the debug test build of this run (fresh target_debug) for the probe.
        step build_probe env CARGO_TARGET_DIR="$run_dir/target_debug" \
            cargo build --frozen -p cham3_provider_probe --manifest-path "$CHAM3_REPO_ROOT/Cargo.toml"
        step provider_oracle python3 -I "$harness" provider-oracle \
            --probe "$run_dir/target_debug/debug/cham3_provider_probe" --run-dir "$run_dir"
        step ecr_vector_oracle run_ecr_oracle
        step build_format_probe env CARGO_TARGET_DIR="$run_dir/target_debug" \
            cargo build --frozen -p cham3_contract_format --bin cham3_format_probe --manifest-path "$CHAM3_REPO_ROOT/Cargo.toml"
        step format_differential python3 -I "$harness" format-differential \
            --probe "$run_dir/target_debug/debug/cham3_format_probe" --run-dir "$run_dir"
        step build_vault_probe env CARGO_TARGET_DIR="$run_dir/target_debug" \
            cargo build --frozen -p cham3_vaultd --bin cham3_vault_probe --manifest-path "$CHAM3_REPO_ROOT/Cargo.toml"
        step vault_object_oracle run_vault_object_oracle
        ;;
    harness)
        profile_dir=$run_dir/profile_target
        step build_probe_debug env CARGO_TARGET_DIR="$profile_dir" \
            cargo build --frozen -p cham3_provider_probe --manifest-path "$CHAM3_REPO_ROOT/Cargo.toml"
        step build_probe_release env CARGO_TARGET_DIR="$profile_dir" \
            cargo build --frozen --release -p cham3_provider_probe --manifest-path "$CHAM3_REPO_ROOT/Cargo.toml"
        step build_vault_protection_probe env CARGO_TARGET_DIR="$profile_dir" \
            cargo build --frozen --release -p cham3_vaultd --bin cham3_vault_protection_probe --manifest-path "$CHAM3_REPO_ROOT/Cargo.toml"
        step python_harness env CHAM3_M2_PROBE_DEBUG="$profile_dir/debug/cham3_provider_probe" \
            CHAM3_M2_PROBE_RELEASE="$profile_dir/release/cham3_provider_probe" \
            CHAM3_M2_VAULT_PROTECTION_PROBE="$profile_dir/release/cham3_vault_protection_probe" \
            python3 -I "$harness" python-suite --name harness --start-dir tests/m2/harness --run-dir "$run_dir"
        ;;
esac

echo "== m2 $suite summary ($run_dir)"
cat "$summary"
if [ "$overall" -eq 0 ]; then
    echo "m2 $suite: PASS"
else
    echo "m2 $suite: FAIL"
fi
exit "$overall"
