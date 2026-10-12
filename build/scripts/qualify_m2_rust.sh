#!/bin/bash
# Purpose: M2-001 Rust toolchain/dependency qualification on Debian 13.
# Usage: build/scripts/qualify_m2_rust.sh [--record]
#   Run unprivileged inside the pinned container (build/m2/dev_container.dockerfile)
#   from a clean checkout. --record additionally copies the observed installed-package
#   set and downloaded .deb digests into the run directory as review proposals when no
#   committed lock exists for this architecture (the run still FAILs: unpinned).
# Checks (each PASS/FAIL, required unless marked):
#   platform/user, exact toolchain identity, explicit package pins, full installed
#   package lock and downloaded .deb digests per architecture, cargo metadata
#   --locked --offline, per-file crate source manifest, Debian-only dependency
#   inventory, rustfmt/clippy/naming, fresh debug and release builds, the fiat
#   curve25519 backend, profile settings (overflow checks, release panic=abort),
#   valgrind memcheck of the release probe, then tests/m2/run_m2_tests.sh unit and
#   harness. Miri, sanitizers and coverage-guided fuzzing are recorded as
#   NOT_AVAILABLE on this toolchain; they never count as PASS.
# Output: $CHAM3_OUT_DIR/m2/qualification/<arch>_<UTC time>_<pid>/ (summary.txt and
#   per-step logs). Exit 0 only if every required check passes.
# Limits: only an amd64 run qualifies the M2 target; arm64 runs are development
#   evidence. Qualification proves build inputs and probe behavior, not M2 security.
set -eu -o pipefail

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "${script_dir}/../.." && pwd)
CHAM3_OUT_DIR=${CHAM3_OUT_DIR:-${repo_root}/out}
export CHAM3_REPO_ROOT=${repo_root} CHAM3_OUT_DIR
cd "${repo_root}"

record_mode=0
case "${1-}" in
    "") ;;
    --record) record_mode=1 ;;
    *)
        echo "usage: $0 [--record]" >&2
        exit 2
        ;;
esac

if [ "$(id -u)" = "0" ]; then
    echo "refusing to qualify as root" >&2
    exit 1
fi

arch=$(dpkg --print-architecture 2> /dev/null || echo unknown)
run_dir=${CHAM3_OUT_DIR}/m2/qualification/${arch}_$(date -u +%Y%m%dT%H%M%SZ)_$$
if [ -e "${run_dir}" ]; then
    echo "stale run directory exists: ${run_dir}" >&2
    exit 1
fi
mkdir -p "${run_dir}"
summary=${run_dir}/summary.txt
overall=0
unset RUSTFLAGS

record() { # status name [detail]
    printf '%s %s%s\n' "$1" "$2" "${3:+ ($3)}" | tee -a "${summary}"
    case "$1" in
        PASS | NOT_AVAILABLE | INFO) ;;
        *) overall=1 ;;
    esac
}

check() { # name command... (output to <run>/<name>.log)
    local name=$1
    shift
    if "$@" > "${run_dir}/${name}.log" 2>&1; then
        record PASS "${name}"
    else
        record FAIL "${name}" "see ${name}.log"
    fi
}

{
    echo "commit $(git -c safe.directory="${repo_root}" rev-parse HEAD)"
    echo "tree $(git -c safe.directory="${repo_root}" rev-parse 'HEAD^{tree}')"
    echo "dirty_tracked $(git -c safe.directory="${repo_root}" status --porcelain --untracked-files=no | wc -l)"
    echo "architecture ${arch}"
    echo "debian_version $(cat /etc/debian_version 2> /dev/null || echo missing)"
    echo "uname $(uname -srmo)"
    echo "euid $(id -u)"
    echo "cargo_lock_sha256 $(sha256sum Cargo.lock | cut -d' ' -f1)"
} > "${run_dir}/identity.txt"
cat "${run_dir}/identity.txt"

# Platform.
case "$(cat /etc/debian_version 2> /dev/null || true)" in
    13.*) record PASS platform_debian_13 ;;
    *) record FAIL platform_debian_13 "not Debian 13" ;;
esac
if [ "${arch}" = "amd64" ]; then
    record INFO target_architecture "amd64: M2 qualification target"
else
    record INFO target_architecture "${arch}: development evidence only, not target qualification"
fi

# Toolchain identity.
{
    rustc -Vv
    cargo -V
    cc --version | head -n 1
    ld --version | head -n 1
    ldd --version | head -n 1
} > "${run_dir}/toolchain.txt" 2>&1 || true
if grep -qx 'release: 1.85.1' "${run_dir}/toolchain.txt" && grep -q '^cargo 1\.85\.1' "${run_dir}/toolchain.txt"; then
    record PASS toolchain_identity "rustc/cargo 1.85.1"
else
    record FAIL toolchain_identity "expected rustc and cargo 1.85.1"
fi

# Explicit package pins.
pin_failures=0
while IFS= read -r pin; do
    case "${pin}" in '' | '#'*) continue ;; esac
    package=${pin%%=*}
    wanted=${pin#*=}
    have=$(dpkg-query -W -f '${Version}' "${package}" 2> /dev/null || echo missing)
    if [ "${have}" != "${wanted}" ]; then
        echo "PIN MISMATCH ${package}: want ${wanted} have ${have}" >> "${run_dir}/package_pins.log"
        pin_failures=$((pin_failures + 1))
    fi
done < build/m2/debian_packages.txt
if [ "${pin_failures}" -eq 0 ]; then
    record PASS package_pins
else
    record FAIL package_pins "${pin_failures} mismatches, see package_pins.log"
fi

# Full installed set and downloaded .deb digests, per architecture.
dpkg-query -W -f '${Package}:${Architecture}=${Version}\n' | LC_ALL=C sort > "${run_dir}/installed_packages.txt"
cp /usr/local/share/cham3/m2/downloaded_debs.sha256 "${run_dir}/downloaded_debs.sha256" 2> /dev/null \
    || : > "${run_dir}/downloaded_debs.sha256"
for lock in "installed_packages:debian_lock_${arch}.txt" "downloaded_debs:debian_debs_${arch}.sha256"; do
    observed=${run_dir}/${lock%%:*}
    case "${lock%%:*}" in
        installed_packages) observed=${observed}.txt ;;
        downloaded_debs) observed=${observed}.sha256 ;;
    esac
    committed=build/m2/${lock#*:}
    if [ -f "${committed}" ]; then
        if diff -u "${committed}" "${observed}" > "${run_dir}/${lock%%:*}_diff.log"; then
            record PASS "${lock%%:*}_lock" "${committed}"
        else
            record FAIL "${lock%%:*}_lock" "differs from ${committed}"
        fi
    else
        if [ "${record_mode}" -eq 1 ]; then
            cp "${observed}" "${run_dir}/proposed_${lock#*:}"
        fi
        record FAIL "${lock%%:*}_lock" "no committed ${committed} (unpinned for ${arch})"
    fi
done
if [ ! -s "${run_dir}/downloaded_debs.sha256" ]; then
    record FAIL downloaded_debs_present "image did not record downloaded .deb digests"
fi

# Cargo resolution, sources and inventory.
if cargo metadata --locked --offline --format-version 1 > "${run_dir}/metadata.json" 2> "${run_dir}/metadata.log"; then
    record PASS cargo_metadata_locked_offline
    check crate_sources python3 -I build/scripts/m2_dependency_inventory.py sources \
        --metadata "${run_dir}/metadata.json" --check build/m2/crate_sources.sha256
    check dependency_inventory python3 -I build/scripts/m2_dependency_inventory.py inventory \
        --metadata "${run_dir}/metadata.json" --out-dir "${run_dir}"
else
    record FAIL cargo_metadata_locked_offline "see metadata.log"
fi

# Static checks.
check rustfmt cargo fmt --all --check
check clippy env CARGO_TARGET_DIR="${run_dir}/clippy_target" \
    cargo clippy --workspace --all-targets --frozen -- -D warnings
check rust_naming python3 -I build/scripts/check_rust_naming.py

# Profile settings (source of truth: workspace Cargo.toml).
check profile_settings python3 -I - << 'PYTHON'
import sys, tomllib
with open("Cargo.toml", "rb") as handle:
    profiles = tomllib.load(handle)["profile"]
problems = []
for name in ("dev", "release", "test", "bench"):
    if profiles.get(name, {}).get("overflow-checks") is not True:
        problems.append(name + ": overflow-checks must be true")
if profiles["release"].get("panic") != "abort":
    problems.append("release: panic must be abort")
if profiles["dev"].get("panic") != "unwind":
    problems.append("dev: panic must be unwind")
print("profiles:", {name: profiles[name] for name in sorted(profiles)})
print("\n".join(problems) or "profile settings OK")
sys.exit(1 if problems else 0)
PYTHON

# Fresh debug and release builds.
build_dir=${run_dir}/build_target
check build_debug env CARGO_TARGET_DIR="${build_dir}" cargo build --workspace --frozen
check build_release env CARGO_TARGET_DIR="${build_dir}" cargo build --workspace --release --frozen
backend_files=$(grep -l 'curve25519_dalek_backend="fiat"' "${build_dir}"/*/build/curve25519-dalek-*/output 2> /dev/null | wc -l)
if [ "${backend_files}" -ge 2 ]; then
    record PASS curve25519_fiat_backend "${backend_files} build-script outputs"
else
    record FAIL curve25519_fiat_backend "fiat backend not selected in debug and release builds"
fi

# Native memory checking available on the stable toolchain.
release_probe=${build_dir}/release/cham3_provider_probe
if command -v valgrind > /dev/null 2>&1; then
    check valgrind_memcheck_probe valgrind --error-exitcode=99 --track-origins=yes "${release_probe}" report
else
    record FAIL valgrind_memcheck_probe "valgrind missing"
fi
if cargo miri --version > /dev/null 2>&1; then
    record INFO miri "available: $(cargo miri --version 2>&1)"
else
    record NOT_AVAILABLE miri "requires a nightly toolchain; not provided by Debian 13"
fi
record NOT_AVAILABLE sanitizers "-Zsanitizer requires nightly; valgrind memcheck used instead"
record NOT_AVAILABLE coverage_guided_fuzzing "cargo-fuzz/libfuzzer-sys not packaged in Debian 13 and require nightly; bounded property/mutation tests are used instead"

# Test suites with strict validation.
check suite_unit sh tests/m2/run_m2_tests.sh unit
check suite_harness sh tests/m2/run_m2_tests.sh harness

echo "== qualification summary (${run_dir})"
cat "${summary}"
if [ "${overall}" -eq 0 ]; then
    echo "m2 qualification: PASS"
else
    echo "m2 qualification: FAIL"
fi
exit "${overall}"
