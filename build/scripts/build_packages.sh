#!/bin/bash
# Purpose: build every Debian package under packaging/*/ (unsigned, binary-only)
# and lint the result with lintian. Needs: dpkg-dev, debhelper, lintian (see
# build/host_packages.txt). No root. Works on a temporary copy, so the source tree
# is never written to.
# Environment: CHAM3_OUT_DIR (default <repo>/out).
# Outputs in $CHAM3_OUT_DIR/packages/: *.deb, *.buildinfo, *.changes, SHA256SUMS,
# lintian.txt.
# Lintian policy: lintian errors fail this script. Warnings, info and pedantic
# tags are recorded in lintian.txt but do not fail the build. The output is
# unsigned and is NOT release-quality.
set -eu -o pipefail

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "${script_dir}/../.." && pwd)
out_dir=${CHAM3_OUT_DIR:-${repo_root}/out}
packages_out="${out_dir}/packages"

for tool in dpkg-buildpackage lintian sha256sum; do
    command -v "${tool}" > /dev/null 2>&1 || { echo "build_packages: missing tool: ${tool}" >&2; exit 2; }
done

work_dir=$(mktemp -d)
trap 'rm -rf "${work_dir}"' EXIT INT TERM

mkdir -p "${packages_out}"
# Remove stale results of earlier runs so SHA256SUMS describes exactly this run.
rm -f "${packages_out}"/*.deb "${packages_out}"/*.buildinfo "${packages_out}"/*.changes \
    "${packages_out}/SHA256SUMS" "${packages_out}/lintian.txt"

built_any=0
for package_dir in "${repo_root}"/packaging/*/; do
    [ -d "${package_dir}debian" ] || continue
    package_name=$(basename "${package_dir}")
    echo "== building ${package_name}"
    build_dir="${work_dir}/${package_name}"
    mkdir -p "${build_dir}"
    cp -a "${package_dir}." "${build_dir}/"
    (cd "${build_dir}" && dpkg-buildpackage -us -uc -b)
    # Outputs land next to the build directory, i.e. in ${work_dir}.
    built_any=1
done

if [ "${built_any}" -ne 1 ]; then
    echo "build_packages: no packages found under packaging/" >&2
    exit 1
fi

cp -p "${work_dir}"/*.deb "${work_dir}"/*.buildinfo "${work_dir}"/*.changes "${packages_out}/"
(cd "${packages_out}" && sha256sum -- *.deb *.buildinfo *.changes > SHA256SUMS)

echo "== lintian"
lintian_status=0
(cd "${packages_out}" && lintian --fail-on error --info --display-info -- ./*.changes) \
    2>&1 | tee "${packages_out}/lintian.txt" || lintian_status=$?
# With "set -o pipefail" the pipeline status is lintian's own status.
echo "# lintian exit status: ${lintian_status}" \
    | tee -a "${packages_out}/lintian.txt"
if [ "${lintian_status}" -ne 0 ]; then
    echo "build_packages: lintian reported errors (exit ${lintian_status})" >&2
    exit 1
fi

echo "build_packages: PASS (packages in ${packages_out})"
