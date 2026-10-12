#!/bin/bash
# Purpose: build the M2 Rust workspace from frozen, offline inputs.
# Usage: build/scripts/build_m2_core.sh
#   Run unprivileged inside the pinned Debian 13 container
#   (build/m2/dev_container.dockerfile). Builds debug and release (deployment) profiles
#   into a fresh target directory and records the toolchain, Cargo.lock digest and the
#   SHA-256 of every produced executable.
# Output: $CHAM3_OUT_DIR/m2/build/<arch>_<UTC time>_<pid>/ with target/, toolchain.txt
#   and executables.sha256. Exit 0 only if both builds succeed.
# Limits: building proves compilation of the frozen inputs only. Service binaries and
#   Debian packages are added by gated work package M2-007.
set -eu -o pipefail

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "${script_dir}/../.." && pwd)
CHAM3_OUT_DIR=${CHAM3_OUT_DIR:-${repo_root}/out}
cd "${repo_root}"
unset RUSTFLAGS

arch=$(dpkg --print-architecture 2> /dev/null || uname -m)
run_dir=${CHAM3_OUT_DIR}/m2/build/${arch}_$(date -u +%Y%m%dT%H%M%SZ)_$$
if [ -e "${run_dir}" ]; then
    echo "stale build directory exists: ${run_dir}" >&2
    exit 1
fi
mkdir -p "${run_dir}"
export CARGO_TARGET_DIR=${run_dir}/target

{
    rustc -Vv
    cargo -V
    echo "Cargo.lock sha256 $(sha256sum Cargo.lock | cut -d' ' -f1)"
    echo "commit $(git -c safe.directory="${repo_root}" rev-parse HEAD)"
} > "${run_dir}/toolchain.txt"

cargo build --workspace --frozen
cargo build --workspace --release --frozen

: > "${run_dir}/executables.sha256"
for profile in debug release; do
    find "${CARGO_TARGET_DIR}/${profile}" -maxdepth 1 -type f -perm -u+x -print0 \
        | LC_ALL=C sort -z \
        | xargs -0 -r sha256sum >> "${run_dir}/executables.sha256"
done
if [ ! -s "${run_dir}/executables.sha256" ]; then
    echo "no executables produced" >&2
    exit 1
fi
cat "${run_dir}/executables.sha256"
echo "m2 build: PASS (${run_dir})"
