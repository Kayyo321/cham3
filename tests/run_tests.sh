#!/bin/sh
# Run the cham3 M0 unittest suites.
#
# Usage: tests/run_tests.sh static|packages|image|boot|all
#
# Environment:
#   CHAM3_REPO_ROOT  repository root (default: parent of this tests/ directory)
#   CHAM3_OUT_DIR    build output directory (default: <repo>/out)
#
# Suites that need build artifacts fail, rather than skip, when those
# artifacts are missing. Exit status is non-zero if any selected suite fails.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
CHAM3_REPO_ROOT=${CHAM3_REPO_ROOT:-$(CDPATH= cd -- "$script_dir/.." && pwd)}
CHAM3_OUT_DIR=${CHAM3_OUT_DIR:-$CHAM3_REPO_ROOT/out}
export CHAM3_REPO_ROOT CHAM3_OUT_DIR

usage() {
    echo "usage: $0 static|packages|image|boot|all" >&2
}

run_suite() {
    case "$1" in
        packages) dir_name=packaging ;;
        *) dir_name=$1 ;;
    esac
    suite_dir="$script_dir/$dir_name"
    echo "== tests/$dir_name (repo: $CHAM3_REPO_ROOT, out: $CHAM3_OUT_DIR) =="
    python3 -m unittest discover -v -s "$suite_dir" -p 'test_*.py'
}

if [ "$#" -ne 1 ]; then
    usage
    exit 2
fi

case "$1" in
    static|packages|image|boot)
        suites=$1
        ;;
    all)
        suites="static packages image boot"
        ;;
    *)
        usage
        exit 2
        ;;
esac

status=0
for suite in $suites; do
    if ! run_suite "$suite"; then
        status=1
    fi
done
exit "$status"
