#!/bin/sh
# Purpose: record the exact source identity of an M2 run (review findings R01, R01-r2).
# Usage: build/scripts/m2_source_identity.sh OUTPUT_FILE [--require-clean]
#   Writes commit, tree, tracked-dirty count, architecture, Debian version, kernel,
#   EUID and Cargo.lock digest of the repository containing this script to OUTPUT_FILE.
# Exit: 0 only if every identity command succeeded and produced well-formed values
#   (and, with --require-clean, no tracked file differs from HEAD); 1 otherwise. A
#   missing repository, failed git command, malformed object id, missing or unreadable
#   Cargo.lock, failed or malformed hash, or any other failed collection is a FAIL,
#   never reported as a clean or dirty state.
# Fail-closed rule (R01-r2): no identity value is taken from a pipeline or from a
#   command whose own exit status was not checked; each value is validated before use.
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
# collect NAME COMMAND...: sets $collected to the command's stdout, or to FAILED and
# records a problem when the command itself fails or prints nothing.
collect() {
    collect_name=$1
    shift
    if collected=$("$@" 2> /dev/null) && [ -n "$collected" ]; then
        return 0
    fi
    collected=FAILED
    fail "$collect_name failed"
}

head_id=$(git_in_repo rev-parse --verify HEAD) || {
    head_id=MISSING
    fail "git rev-parse HEAD failed"
}
tree_id=$(git_in_repo rev-parse --verify 'HEAD^{tree}') || {
    tree_id=MISSING
    fail "git rev-parse HEAD^{tree} failed"
}
for value in "$head_id" "$tree_id"; do
    case "$value" in
        MISSING) ;;
        *[!0-9a-f]* | '') fail "malformed object id" ;;
        *) [ "${#value}" -eq 40 ] || fail "object id length" ;;
    esac
done

# Count changed tracked files without a pipeline: the status command's own exit
# status decides whether a count exists at all.
if status_text=$(git_in_repo status --porcelain --untracked-files=no); then
    dirty_count=0
    while IFS= read -r status_line; do
        if [ -n "$status_line" ]; then
            dirty_count=$((dirty_count + 1))
        fi
    done << EOF
$status_text
EOF
else
    dirty_count=unknown
    fail "git status failed"
fi

# Cargo.lock digest: the hash command's own status is checked before its output is
# used, and the digest must be exactly 64 lowercase hex characters (R01-r2).
lock=$repo_root/Cargo.lock
if [ ! -f "$lock" ]; then
    lock_digest=MISSING
    fail "Cargo.lock missing"
elif hash_output=$(sha256sum -- "$lock" 2> /dev/null); then
    lock_digest=${hash_output%%[!0-9a-f]*}
    if [ "${#lock_digest}" -ne 64 ]; then
        lock_digest=INVALID
        fail "Cargo.lock digest malformed"
    fi
else
    lock_digest=FAILED
    fail "sha256sum of Cargo.lock failed"
fi

debian_version=MISSING
if [ -r /etc/debian_version ] && debian_version=$(cat /etc/debian_version) && [ -n "$debian_version" ]; then
    :
else
    debian_version=MISSING
    fail "/etc/debian_version missing"
fi
collect "dpkg --print-architecture" dpkg --print-architecture
architecture=$collected
collect "uname" uname -srmo
kernel=$collected
collect "id -u" id -u
euid=$collected
case "$euid" in
    FAILED) ;;
    *[!0-9]* | '') fail "malformed euid" ;;
esac

if [ "$require_clean" -eq 1 ] && [ "$dirty_count" != "0" ]; then
    fail "tracked files differ from HEAD (dirty_tracked $dirty_count)"
fi

{
    echo "commit $head_id"
    echo "tree $tree_id"
    echo "dirty_tracked $dirty_count"
    echo "architecture $architecture"
    echo "debian_version $debian_version"
    echo "uname $kernel"
    echo "euid $euid"
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
