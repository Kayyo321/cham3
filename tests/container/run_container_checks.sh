#!/bin/sh
# Run the M1 container-stage suites (tests/container/test_m1_container_*.py) in a
# disposable debian:trixie linux/amd64 container. The container runs as root, which
# the capture and staging checks need. It is removed when the run ends.
#
# Usage: tests/container/run_container_checks.sh
# Environment:
#   CHAM3_REPO_ROOT           repository root (default: parent of tests/)
#   CHAM3_OUT_DIR             build output directory (default: <repo>/out); the capture
#                             suite needs <out>/packages/cham3-desktop_*.deb, made by
#                             build/scripts/build_packages.sh
#   CHAM3_CONTAINER_IMAGE     container image (default: debian:trixie)
#   CHAM3_CONTAINER_PLATFORM  container platform (default: linux/amd64)
#   CHAM3_CONTAINER_PRIVILEGED  1 runs the container with --privileged and sets
#                             CHAM3_REQUIRE_PRIVILEGED_CAPTURE=1: the member/nonmember
#                             execution and packet-capture checks must then run, and a
#                             missing capability is a FAIL instead of a skip (F8)
#
# Requires Docker. A missing Docker or missing built packages is reported as BLOCKED
# and fails the run; nothing is skipped silently. Without CHAM3_CONTAINER_PRIVILEGED=1,
# ordinary Docker lacks CAP_NET_ADMIN, and the capture checks are skipped with that
# reason; those skips are not a pass of the privileged gate.
set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
CHAM3_REPO_ROOT=${CHAM3_REPO_ROOT:-$(CDPATH='' cd -- "$script_dir/../.." && pwd)}
CHAM3_OUT_DIR=${CHAM3_OUT_DIR:-$CHAM3_REPO_ROOT/out}
image=${CHAM3_CONTAINER_IMAGE:-debian:trixie}
platform=${CHAM3_CONTAINER_PLATFORM:-linux/amd64}
privileged=${CHAM3_CONTAINER_PRIVILEGED:-0}

if ! command -v docker > /dev/null 2>&1; then
    echo "BLOCKED (host environment): docker is absent; container checks were not run" >&2
    exit 1
fi
if [ ! -d "${CHAM3_OUT_DIR}/packages" ]; then
    echo "BLOCKED: ${CHAM3_OUT_DIR}/packages is absent; run build/scripts/build_packages.sh first" >&2
    exit 1
fi

if [ "${privileged}" = "1" ]; then
    set -- --privileged -e CHAM3_REQUIRE_PRIVILEGED_CAPTURE=1
else
    set --
fi

exec docker run --rm --platform "${platform}" "$@" \
    --mount "type=bind,source=${CHAM3_REPO_ROOT},target=/src,readonly" \
    --mount "type=bind,source=${CHAM3_OUT_DIR},target=/out,readonly" \
    -e CHAM3_REPO_ROOT=/src \
    -e CHAM3_OUT_DIR=/out \
    "${image}" sh /src/tests/container/in_container.sh
