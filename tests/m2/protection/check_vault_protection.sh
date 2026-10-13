#!/bin/sh
# Positive path of the vault's process protection (ADR 0005 §5): with a memlock
# limit that covers the default locked budget plus headroom (768 MiB; the deployment
# unit sets LimitMEMLOCK >= 1 GiB), cham3_vault_protection_probe applies
# RLIMIT_CORE=0, PR_SET_DUMPABLE=0 and mlockall, derives at the release Argon2id
# calibration and must report every resident mapping locked, a dont-dump secret
# region, a non-dumpable process and a zero core limit.
#
# The standard qualification container has an 8 MiB memlock limit that an
# unprivileged process cannot raise, so this check is a separate step: run it in the
# pinned container started with --ulimit memlock=1073741824:1073741824 (no
# --privileged, no added capabilities). The refusal path (insufficient limit) is
# part of the harness suite.
#
# Usage: sh tests/m2/protection/check_vault_protection.sh <output directory>
set -eu

out=${1:?usage: check_vault_protection.sh <output directory>}
repo=$(cd "$(dirname "$0")/../../.." && pwd)
if [ "$(id -u)" = "0" ]; then
    echo "refusing to run as root" >&2
    exit 1
fi
mkdir -p "$out"
target=$out/target
report=$out/vault_protection_report.txt
env CARGO_TARGET_DIR="$target" cargo build --frozen --release -p cham3_vaultd \
    --bin cham3_vault_protection_probe --manifest-path "$repo/Cargo.toml"
{
    echo "memlock_limit $(grep 'Max locked memory' /proc/self/limits)"
    status=0
    "$target/release/cham3_vault_protection_probe" || status=$?
    echo "exit_status $status"
} > "$report" 2>&1
cat "$report"
grep -qx 'exit_status 0' "$report"
grep -qx 'protection_verified' "$report"
grep -qx 'unlocked_mappings 0' "$report"
grep -qx 'dumpable 0' "$report"
grep -qx 'core_limit 0' "$report"
echo "vault protection positive path: PASS"
