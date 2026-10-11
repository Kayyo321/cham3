#!/bin/sh
# Purpose: best-effort scan for obvious committed secrets (private-key blocks and
# common token formats). Usage: scan_secrets.sh [PATH...]
#   No arguments: scans all git-tracked files of this repository.
#   With PATH arguments: scans those files or directories recursively
#   (used for build artifacts and logs).
# Exit: 0 clean, 1 finding(s), 2 usage error.
# Limits: pattern matching only. It is NOT a complete secret scanner and does not
# prove the absence of secrets. Binary files are skipped. Matched values are never
# printed, only file:line and the rule name.
# The patterns below are written so that this file does not match them.
set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "${script_dir}/../.." && pwd)

if [ "${1-}" = "-h" ] || [ "${1-}" = "--help" ]; then
    sed -n '2,11p' "$0"
    exit 0
fi

list_file=$(mktemp)
trap 'rm -f "${list_file}"' EXIT INT TERM

if [ "$#" -eq 0 ]; then
    git -c safe.directory="${repo_root}" -C "${repo_root}" ls-files -z > "${list_file}" \
        || { echo "scan_secrets: git ls-files failed" >&2; exit 2; }
    cd "${repo_root}"
else
    for scan_path in "$@"; do
        if [ ! -e "${scan_path}" ]; then
            echo "scan_secrets: no such path: ${scan_path}" >&2
            exit 2
        fi
    done
    find "$@" -type f -print0 > "${list_file}"
fi

found=0
# rule_name|extended-regex
rules='private_key_block|BEGIN [A-Z ]*PRIVATE KEY
github_token|gh[pousr]_[A-Za-z0-9]{36,}
github_fine_grained_token|github_pat_[A-Za-z0-9_]{22,}
aws_access_key_id|AKIA[0-9A-Z]{16}
slack_token|xox[baprs]-[A-Za-z0-9-]{10,}'

oldifs=${IFS}
IFS='
'
for rule in ${rules}; do
    rule_name=${rule%%|*}
    rule_regex=${rule#*|}
    # -I skips binary files; -n gives line numbers; -H always prints file name.
    # Only file:line is kept from grep output so the secret is never echoed.
    hits=$(xargs -r -0 grep -I -n -H -E -e "${rule_regex}" -- < "${list_file}" 2>/dev/null \
        | cut -d: -f1,2 || true)
    if [ -n "${hits}" ]; then
        found=1
        for hit in ${hits}; do
            echo "FINDING ${rule_name}: ${hit}"
        done
    fi
done
IFS=${oldifs}

if [ "${found}" -ne 0 ]; then
    echo "scan_secrets: FAIL (potential secrets found; values not shown)" >&2
    exit 1
fi
echo "scan_secrets: PASS (no matches for the configured patterns)"
exit 0
