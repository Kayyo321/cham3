#!/bin/sh
# Runs the M1 container-stage suites inside a disposable debian:trixie container as
# root. Invoked by tests/container/run_container_checks.sh; it can also be run
# directly inside a throwaway Debian 13 amd64 system. Do not run it on a host you
# care about: the capture suite changes users, groups and /usr/bin/dumpcap.
set -eu
export DEBIAN_FRONTEND=noninteractive
export PYTHONDONTWRITEBYTECODE=1
export CHAM3_REPO_ROOT="${CHAM3_REPO_ROOT:-/src}"
export CHAM3_OUT_DIR="${CHAM3_OUT_DIR:-/out}"

if [ "$(id -u)" -ne 0 ]; then
    echo "in_container: must run as root (capture and staging checks change system state)" >&2
    exit 1
fi

apt-get update -qq > /tmp/cham3-apt-update.log 2>&1 || { cat /tmp/cham3-apt-update.log >&2; exit 1; }
apt-get install -y -qq --no-install-recommends python3 util-linux libcap2-bin debconf adduser \
    > /tmp/cham3-apt-tools.log 2>&1 || { cat /tmp/cham3-apt-tools.log >&2; exit 1; }

cd "${CHAM3_REPO_ROOT}"
exec python3 -m unittest discover -v -s "${CHAM3_REPO_ROOT}/tests/container" -p 'test_m1_container_*.py'
