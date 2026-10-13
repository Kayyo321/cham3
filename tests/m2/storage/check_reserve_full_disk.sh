#!/bin/sh
# Restrictive reserve under an actually full filesystem (ADR 0006 §1; M2-005 review
# R03). cham3_reserve_probe creates a journal on a small disposable tmpfs, fills the
# filesystem with an unrelated writer until ENOSPC, and must show: a restrictive
# append becomes durable from the preallocated reserve; a non-restrictive append is
# refused while the reserve cannot be refilled; restrictive appends go on; the first
# non-restrictive append after space returns refills the reserve; the journal is
# chained throughout. The probe refuses any filesystem that is not a tmpfs of at most
# 16 MiB, so it never fills a real disk, and it owns only the root and filler it
# creates. This wrapper owns nothing on the disk either (M2-005 review R3-01): it
# removes nothing before the probe runs, refuses to start when the probe's root or
# filler path already exists (a file, directory or symbolic link, whatever it points
# to), and only checks afterwards that the probe left nothing behind.
#
# The qualification container has no such filesystem, so this check is a separate
# step: run it in the pinned container started with
# --tmpfs /full-disk:rw,size=2m,uid=<uid>,gid=<gid>,mode=0700 (no --privileged, no
# added capabilities). The fault-injected reserve tests, the probe's refusal and
# allocate-only paths and this wrapper's preservation cases are part of the ordinary
# suites (tests/m2/harness/test_m2_reserve_probe.py).
#
# Usage: sh tests/m2/storage/check_reserve_full_disk.sh <output directory> [tmpfs]
# Environment: CHAM3_REPO_ROOT (the checkout; default: this script's repository),
# CHAM3_M2_RESERVE_TARGET_DIR (cargo target directory; default: <output>/target).
set -eu

out=${1:?usage: check_reserve_full_disk.sh <output directory> [tmpfs mount]}
disk=${2:-/full-disk}
repo=${CHAM3_REPO_ROOT:-$(cd "$(dirname "$0")/../../.." && pwd)}
if [ "$(id -u)" = "0" ]; then
    echo "refusing to run as root" >&2
    exit 1
fi
root_path=$disk/cham3_reserve_probe_root
filler_path=$disk/cham3_reserve_probe_filler
# Nothing on the disk is this wrapper's to remove: an existing root or filler path
# (including a symbolic link, dangling or not) refuses the whole check, untouched.
for path in "$root_path" "$filler_path"; do
    if [ -e "$path" ] || [ -L "$path" ]; then
        echo "refusing: $path exists; this check creates and removes only its own paths" >&2
        exit 1
    fi
done
mkdir -p "$out"
target=${CHAM3_M2_RESERVE_TARGET_DIR:-$out/target}
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
    {
        echo "profile $profile"
        grep ' /full-disk \| '"$disk"' ' /proc/self/mounts | sed 's/^/mount /' || true
        status=0
        "$target/$profile/cham3_reserve_probe" "$root_path" full-disk || status=$?
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
    test ! -e "$root_path"
    test ! -L "$root_path"
    test ! -e "$filler_path"
    test ! -L "$filler_path"
done
echo "restrictive reserve under a full filesystem: PASS"
