#!/bin/bash
# Purpose: M0 static checks over git-tracked files: shellcheck, python syntax
# (py_compile), yamllint, secret scan. Needs: git, shellcheck, python3, yamllint.
# No root, no network, never writes into the source tree.
# Exit 0 only if every check passes. A missing tool is a FAIL, not a skip.
# Limits: these are lint-level checks; they do not demonstrate any product behavior.
set -eu -o pipefail

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "${script_dir}/../.." && pwd)
cd "${repo_root}"

git_cmd() { git -c safe.directory="${repo_root}" "$@"; }

tmp_dir=$(mktemp -d)
trap 'rm -rf "${tmp_dir}"' EXIT INT TERM

declare -a summary=()
overall=0

record() { # name status
    summary+=("$2 $1")
    if [ "$2" != "PASS" ]; then overall=1; fi
}

need_tool() {
    if ! command -v "$1" > /dev/null 2>&1; then
        echo "missing required tool: $1" >&2
        return 1
    fi
}

mapfile -d '' tracked_files < <(git_cmd ls-files -z)

# Tracked shell scripts: *.sh, or any file whose first line is a sh/bash shebang.
shell_files=()
for tracked in "${tracked_files[@]}"; do
    [ -f "${tracked}" ] || continue
    case "${tracked}" in
        *.sh) shell_files+=("${tracked}"); continue ;;
    esac
    first_line=$(head -n 1 "${tracked}" 2> /dev/null | tr -d '\0' || true)
    case "${first_line}" in
        '#!'*/sh* | '#!'*/bash* | '#!'*env\ sh* | '#!'*env\ bash*) shell_files+=("${tracked}") ;;
    esac
done

python_files=()
yaml_files=()
for tracked in "${tracked_files[@]}"; do
    case "${tracked}" in
        *.py) python_files+=("${tracked}") ;;
        *.yml | *.yaml) yaml_files+=("${tracked}") ;;
    esac
done

echo "== shellcheck (${#shell_files[@]} files)"
if need_tool shellcheck && { [ "${#shell_files[@]}" -eq 0 ] || shellcheck -x -- "${shell_files[@]}"; }; then
    record shellcheck PASS
else
    record shellcheck FAIL
fi

echo "== py_compile (${#python_files[@]} files)"
if need_tool python3 && PYTHONDONTWRITEBYTECODE=1 python3 -I -c '
import os, py_compile, sys, tempfile
failed = False
with tempfile.TemporaryDirectory() as scratch:
    for index, path in enumerate(sys.argv[1:]):
        try:
            py_compile.compile(path, cfile=os.path.join(scratch, "{}.pyc".format(index)), doraise=True)
        except py_compile.PyCompileError as error:
            print(error, file=sys.stderr)
            failed = True
sys.exit(1 if failed else 0)
' "${python_files[@]}"; then
    record py_compile PASS
else
    record py_compile FAIL
fi

echo "== yamllint (${#yaml_files[@]} files)"
if need_tool yamllint && { [ "${#yaml_files[@]}" -eq 0 ] || yamllint -c .yamllint.yml -- "${yaml_files[@]}"; }; then
    record yamllint PASS
else
    record yamllint FAIL
fi

echo "== scan_secrets"
if "${script_dir}/scan_secrets.sh"; then
    record scan_secrets PASS
else
    record scan_secrets FAIL
fi

echo
echo "== summary"
printf '%s\n' "${summary[@]}"
if [ "${overall}" -eq 0 ]; then
    echo "static checks: PASS"
else
    echo "static checks: FAIL"
fi
exit "${overall}"
