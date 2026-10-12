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
#
# Requires Docker. A missing Docker or missing built packages is reported as BLOCKED
# and fails the run; nothing is skipped silently. Individual capture checks that this
# container cannot run are skipped with a NOT_RUN reason (Codex Remote gates).
set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
CHAM3_REPO_ROOT=${CHAM3_REPO_ROOT:-$(CDPATH='' cd -- "$script_dir/../.." && pwd)}
CHAM3_OUT_DIR=${CHAM3_OUT_DIR:-$CHAM3_REPO_ROOT/out}
image=${CHAM3_CONTAINER_IMAGE:-debian:trixie}
platform=${CHAM3_CONTAINER_PLATFORM:-linux/amd64}

if ! command -v docker > /dev/null 2>&1; then
    echo "BLOCKED (host environment): docker is absent; container checks were not run" >&2
    exit 1
fi
if [ ! -d "${CHAM3_OUT_DIR}/packages" ]; then
    echo "BLOCKED: ${CHAM3_OUT_DIR}/packages is absent; run build/scripts/build_packages.sh first" >&2
    exit 1
fi

exec docker run --rm --platform "${platform}" \
    --mount "type=bind,source=${CHAM3_REPO_ROOT},target=/src,readonly" \
    --mount "type=bind,source=${CHAM3_OUT_DIR},target=/out,readonly" \
    -e CHAM3_REPO_ROOT=/src \
    -e CHAM3_OUT_DIR=/out \
    "${image}" sh /src/tests/container/in_container.sh
