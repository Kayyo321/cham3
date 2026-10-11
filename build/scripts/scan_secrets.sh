#!/bin/bash
# Purpose: best-effort scan for obvious committed secrets (private-key blocks and
# common token formats). Usage: scan_secrets.sh [PATH...]
#   No arguments: scans all git-tracked files of this repository.
#   With PATH arguments: scans those files or directories recursively
#   (used for build artifacts and logs). Symlinks are followed, so a symlink to a
#   regular file is scanned using its target's content; a dangling symlink is
#   skipped and is not an error.
# Exit: 0 clean, 1 finding(s), 2 usage or scan error. Fail closed: if any file
# cannot be read (permission denied, I/O error) the script prints
# "scan_secrets: ERROR cannot scan <path>" to stderr and exits 2, unless a finding
# was also made, in which case it exits 1. It never exits 0 if a file went unscanned.
# Limits: pattern matching only. It is NOT a complete secret scanner and does not
# prove the absence of secrets. Binary files are skipped. Matched values are never
# printed; a finding prints "FINDING <rule>: <path>:<line>".
# The patterns below are written so that this file does not match them.
set -eu -o pipefail

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "${script_dir}/../.." && pwd)

if [ "${1-}" = "-h" ] || [ "${1-}" = "--help" ]; then
    sed -n '2,16p' "$0"
    exit 0
fi

list_file=$(mktemp)
trap 'rm -f "${list_file}"' EXIT INT TERM

scan_error=0
if [ "$#" -eq 0 ]; then
    git -c safe.directory="${repo_root}" -C "${repo_root}" ls-files -z > "${list_file}" \
        || { echo "scan_secrets: git ls-files failed" >&2; exit 2; }
    cd "${repo_root}"
else
    for scan_path in "$@"; do
        if [ ! -e "${scan_path}" ] && [ ! -L "${scan_path}" ]; then
            echo "scan_secrets: no such path: ${scan_path}" >&2
            exit 2
        fi
    done
    # -L follows symlinks. A traversal error (unreadable directory, symlink loop)
    # means some content was not scanned, so it is a scan error.
    if ! find -L "$@" -type f -print0 > "${list_file}"; then
        echo "scan_secrets: ERROR cannot scan all of: $*" >&2
        scan_error=1
    fi
fi

found=0
# rule_name|extended-regex
rules=(
    'private_key_block|BEGIN [A-Z ]*PRIVATE KEY'
    'github_token|gh[pousr]_[A-Za-z0-9]{36,}'
    'github_fine_grained_token|github_pat_[A-Za-z0-9_]{22,}'
    'aws_access_key_id|AKIA[0-9A-Z]{16}'
    'slack_token|xox[baprs]-[A-Za-z0-9-]{10,}'
)

while IFS= read -r -d '' file_path; do
    # Dangling symlinks and directories (git submodules) are not scannable and not errors.
    if [ -d "${file_path}" ] || { [ -L "${file_path}" ] && [ ! -e "${file_path}" ]; }; then
        continue
    fi
    file_failed=0
    for rule in "${rules[@]}"; do
        rule_name=${rule%%|*}
        rule_regex=${rule#*|}
        # -I skips binary files; -n prefixes the line number. grep exit status:
        # 0 match, 1 no match, 2 error (for example permission denied).
        grep_status=0
        matches=$(grep -I -n -E -e "${rule_regex}" -- "${file_path}" 2> /dev/null) || grep_status=$?
        if [ "${grep_status}" -eq 2 ]; then
            file_failed=1
        fi
        if [ -n "${matches}" ]; then
            found=1
            # Keep only the line number (text before the first colon); never the match.
            while IFS= read -r line_number; do
                echo "FINDING ${rule_name}: ${file_path}:${line_number}"
            done < <(printf '%s\n' "${matches}" | cut -d: -f1)
        fi
    done
    if [ "${file_failed}" -ne 0 ]; then
        echo "scan_secrets: ERROR cannot scan ${file_path}" >&2
        scan_error=1
    fi
done < "${list_file}"

if [ "${found}" -ne 0 ]; then
    echo "scan_secrets: FAIL (potential secrets found; values not shown)" >&2
    exit 1
fi
if [ "${scan_error}" -ne 0 ]; then
    echo "scan_secrets: FAIL (scan incomplete; some files could not be scanned)" >&2
    exit 2
fi
echo "scan_secrets: PASS (no matches for the configured patterns)"
exit 0
