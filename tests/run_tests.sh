#!/bin/sh
# Run the cham3 M0 unittest suites.
#
# Usage: tests/run_tests.sh static|packages|image|boot|desktop|image-m1|staging|container|all
#
# Environment:
#   CHAM3_REPO_ROOT  repository root (default: parent of this tests/ directory)
#   CHAM3_OUT_DIR    build output directory (default: <repo>/out)
#
# Suites that need build artifacts fail, rather than skip, when those
# artifacts are missing. Exit status is non-zero if any selected suite fails.
set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
CHAM3_REPO_ROOT=${CHAM3_REPO_ROOT:-$(CDPATH='' cd -- "$script_dir/.." && pwd)}
CHAM3_OUT_DIR=${CHAM3_OUT_DIR:-$CHAM3_REPO_ROOT/out}
export CHAM3_REPO_ROOT CHAM3_OUT_DIR

usage() {
    echo "usage: $0 static|packages|image|boot|desktop|image-m1|staging|container|all" >&2
}

run_suite() {
    # desktop: M1 static desktop/package checks (no build artifacts needed).
    # image-m1: M1 image-artifact checks (need the M1 image in $CHAM3_OUT_DIR).
    # staging: M1 per-profile staging dry run (needs dpkg-deb, no Docker; not in "all").
    # container: M1 container-stage checks (Docker; see tests/container/). Not in "all".
    if [ "$1" = "container" ]; then
        echo "== tests/container (container, repo: $CHAM3_REPO_ROOT, out: $CHAM3_OUT_DIR) =="
        sh "$script_dir/container/run_container_checks.sh"
        return
    fi
    case "$1" in
        packages) dir_name=packaging; pattern='test_*.py' ;;
        desktop) dir_name=desktop; pattern='test_m1_static_*.py' ;;
        image-m1) dir_name=desktop; pattern='test_m1_image_*.py' ;;
        staging) dir_name=container; pattern='test_m1_container_staging.py' ;;
        *) dir_name=$1; pattern='test_*.py' ;;
    esac
    suite_dir="$script_dir/$dir_name"
    echo "== tests/$dir_name ($1, repo: $CHAM3_REPO_ROOT, out: $CHAM3_OUT_DIR) =="
    python3 -m unittest discover -v -s "$suite_dir" -p "$pattern"
}

if [ "$#" -ne 1 ]; then
    usage
    exit 2
fi

case "$1" in
    static|packages|image|boot|desktop|image-m1|staging|container)
        suites=$1
        ;;
    all)
        suites="static packages image boot desktop"
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
