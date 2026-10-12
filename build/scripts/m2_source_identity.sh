#!/bin/sh
# Purpose: record the exact source identity of an M2 run (review finding R01).
# Usage: build/scripts/m2_source_identity.sh OUTPUT_FILE [--require-clean]
#   Writes commit, tree, tracked-dirty count, Debian version and Cargo.lock digest of
#   the repository containing this script to OUTPUT_FILE.
# Exit: 0 only if every identity command succeeded and produced well-formed values
#   (and, with --require-clean, no tracked file differs from HEAD); 1 otherwise. A
#   missing repository, failed git command, malformed object id or missing Cargo.lock
#   is a FAIL, never reported as a clean or dirty state.
set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/../.." && pwd)
output=${1:?usage: m2_source_identity.sh OUTPUT_FILE [--require-clean]}
require_clean=0
if [ "${2-}" = "--require-clean" ]; then
    require_clean=1
fi

ok=1
problems=""
fail() {
    ok=0
    problems="$problems $1;"
}
git_in_repo() {
    git -c safe.directory="$repo_root" -C "$repo_root" "$@" 2> /dev/null
}

head_id=$(git_in_repo rev-parse --verify HEAD) || fail "git rev-parse HEAD failed"
tree_id=$(git_in_repo rev-parse --verify 'HEAD^{tree}') || fail "git rev-parse HEAD^{tree} failed"
if status_text=$(git_in_repo status --porcelain --untracked-files=no); then
    dirty_count=$(printf '%s' "$status_text" | grep -c . || true)
else
    dirty_count=unknown
    fail "git status failed"
fi
for value in "$head_id" "$tree_id"; do
    case "$value" in
        *[!0-9a-f]* | '') fail "malformed object id" ;;
        *) [ "${#value}" -eq 40 ] || fail "object id length" ;;
    esac
done
if [ -f "$repo_root/Cargo.lock" ]; then
    lock_digest=$(sha256sum "$repo_root/Cargo.lock" | cut -d' ' -f1)
else
    lock_digest=MISSING
    fail "Cargo.lock missing"
fi
debian_version=$(cat /etc/debian_version 2> /dev/null) || {
    debian_version=MISSING
    fail "/etc/debian_version missing"
}
if [ "$require_clean" -eq 1 ] && [ "$dirty_count" != "0" ]; then
    fail "tracked files differ from HEAD (dirty_tracked $dirty_count)"
fi

{
    echo "commit ${head_id:-MISSING}"
    echo "tree ${tree_id:-MISSING}"
    echo "dirty_tracked $dirty_count"
    echo "architecture $(dpkg --print-architecture 2> /dev/null || uname -m)"
    echo "debian_version $debian_version"
    echo "uname $(uname -srmo)"
    echo "euid $(id -u)"
    echo "cargo_lock_sha256 $lock_digest"
    if [ "$ok" -eq 1 ]; then
        echo "identity PASS"
    else
        echo "identity FAIL:$problems"
    fi
} > "$output"

if [ "$ok" -eq 1 ]; then
    exit 0
fi
echo "source identity FAIL:$problems" >&2
exit 1
