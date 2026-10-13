#!/bin/sh
# Restrictive reserve under an actually full filesystem (ADR 0006 §1; M2-005 review
# R03). cham3_reserve_probe creates a journal on a small disposable tmpfs, fills the
# filesystem with an unrelated writer until ENOSPC, and must show: a restrictive
# append becomes durable from the preallocated reserve; a non-restrictive append is
# refused while the reserve cannot be refilled; restrictive appends go on; the first
# non-restrictive append after space returns refills the reserve; the journal is
# chained throughout. The probe refuses any filesystem that is not a tmpfs of at most
# 16 MiB, so it never fills a real disk, and it owns only the root and filler it
# creates (an existing root is refused and never touched).
#
# The qualification container has no such filesystem, so this check is a separate
# step: run it in the pinned container started with
# --tmpfs /full-disk:rw,size=2m,uid=<uid>,gid=<gid>,mode=0700 (no --privileged, no
# added capabilities). The fault-injected reserve tests and the probe's refusal and
# allocate-only paths are part of the ordinary suites.
#
# Usage: sh tests/m2/storage/check_reserve_full_disk.sh <output directory> [tmpfs]
set -eu

out=${1:?usage: check_reserve_full_disk.sh <output directory> [tmpfs mount]}
disk=${2:-/full-disk}
repo=$(cd "$(dirname "$0")/../../.." && pwd)
if [ "$(id -u)" = "0" ]; then
    echo "refusing to run as root" >&2
    exit 1
fi
mkdir -p "$out"
target=$out/target
for profile in release debug; do
    if [ "$profile" = release ]; then
        flag=--release
    else
        flag=
    fi
    # shellcheck disable=SC2086
    env CARGO_TARGET_DIR="$target" cargo build --frozen $flag -p cham3_registryd \
        --bin cham3_reserve_probe --manifest-path "$repo/Cargo.toml"
    report=$out/reserve_full_disk_$profile.txt
    rm -rf "$disk/cham3_reserve_probe_root" "$disk/cham3_reserve_probe_filler"
    {
        echo "profile $profile"
        grep ' /full-disk \| '"$disk"' ' /proc/self/mounts | sed 's/^/mount /' || true
        status=0
        "$target/$profile/cham3_reserve_probe" \
            "$disk/cham3_reserve_probe_root" full-disk || status=$?
        echo "exit_status $status"
    } > "$report" 2>&1
    cat "$report"
    grep -qx 'exit_status 0' "$report"
    grep -qx 'reserve_preallocated' "$report"
    grep -q '^restrictive_append_on_full_filesystem ok' "$report"
    grep -qx 'admission_refused reserve_depleted' "$report"
    grep -q '^second_restrictive_append ok' "$report"
    grep -qx 'reserve_replenished' "$report"
    grep -qx 'journal_chained' "$report"
    grep -qx 'reserve_verified' "$report"
    # The probe removes only what it created, and all of it (R2-03).
    test ! -e "$disk/cham3_reserve_probe_root"
    test ! -e "$disk/cham3_reserve_probe_filler"
done
echo "restrictive reserve under a full filesystem: PASS"
