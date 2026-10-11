#!/bin/bash
# Purpose: build the cham3 M0 development live image with Debian live-build.
# REQUIRES root and a Debian 13 amd64 build host (live-build uses chroot/mounts).
# Run it only in a disposable build environment (VM or privileged container).
# Needs: packages built first by build/scripts/build_packages.sh, network access to
# the official Debian mirrors, and the tools in build/host_packages.txt.
# Environment:
#   CHAM3_OUT_DIR                  output root (default <repo>/out)
#   CHAM3_WORK_DIR                 live-build work dir (default $CHAM3_OUT_DIR/live-build-work)
#   CHAM3_ALLOW_UNVERIFIED_HOST=1  allow a host that is not Debian 13 amd64; recorded
#                                  as an unverified host in provenance.json
# Outputs in $CHAM3_OUT_DIR/image/: cham3-m0-dev-amd64.iso, .packages (package
# manifest), build.log, provenance.json, SHA256SUMS.
# Limits: bit-for-bit reproducibility is NOT claimed. The image is an M0 development
# artifact with no cham3 functionality, not an installable cham3 distribution.
set -eu -o pipefail

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "${script_dir}/../.." && pwd)
image_name="cham3-m0-dev-amd64"
marker_name=".cham3_live_build_work"

die() { echo "build_image: $*" >&2; exit 1; }

# Validate the output and work directories first (before the root check, so this is
# testable unprivileged). The work dir is cleaned and rewritten by this script, so
# refuse anything that could point at a system or source directory.
out_dir_raw=${CHAM3_OUT_DIR-${repo_root}/out}
[ -n "${out_dir_raw}" ] || die "refusing empty CHAM3_OUT_DIR"
out_dir=$(realpath -m -- "${out_dir_raw}") || die "refusing CHAM3_OUT_DIR that cannot be resolved: ${out_dir_raw}"
work_dir_raw=${CHAM3_WORK_DIR-${out_dir}/live-build-work}
[ -n "${work_dir_raw}" ] || die "refusing empty CHAM3_WORK_DIR"
work_dir=$(realpath -m -- "${work_dir_raw}") || die "refusing CHAM3_WORK_DIR that cannot be resolved: ${work_dir_raw}"
repo_root_resolved=$(realpath -m -- "${repo_root}")
[ "${out_dir}" != "/" ] || die "refusing CHAM3_OUT_DIR that resolves to /"
[ "${work_dir}" != "/" ] || die "refusing CHAM3_WORK_DIR that resolves to /"
[ "${work_dir}" != "${out_dir}" ] || die "refusing CHAM3_WORK_DIR equal to CHAM3_OUT_DIR (${work_dir})"
[ "${work_dir}" != "${repo_root_resolved}" ] || die "refusing CHAM3_WORK_DIR equal to the repository root (${work_dir})"
image_dir="${out_dir}/image"
packages_dir="${out_dir}/packages"

[ "$(id -u)" -eq 0 ] || die "must run as root (live-build needs chroot and mounts); use a disposable environment"

unverified_override=0
host_arch=$(dpkg --print-architecture 2> /dev/null || echo unknown)
host_version_id=$(. /etc/os-release 2> /dev/null && echo "${VERSION_ID:-unknown}")
if [ "${host_arch}" != "amd64" ] || [ "${host_version_id}" != "13" ]; then
    if [ "${CHAM3_ALLOW_UNVERIFIED_HOST:-0}" = "1" ]; then
        echo "build_image: WARNING unverified host (arch=${host_arch}, VERSION_ID=${host_version_id}); recorded in provenance" >&2
        unverified_override=1
    else
        die "host is not Debian 13 amd64 (arch=${host_arch}, VERSION_ID=${host_version_id}); set CHAM3_ALLOW_UNVERIFIED_HOST=1 to override"
    fi
fi

for tool in lb debootstrap git python3 sha256sum; do
    command -v "${tool}" > /dev/null 2>&1 || die "missing tool: ${tool} (see build/host_packages.txt)"
done

shopt -s nullglob
deb_files=("${packages_dir}"/*.deb)
shopt -u nullglob
[ "${#deb_files[@]}" -gt 0 ] || die "no .deb files in ${packages_dir}; run build/scripts/build_packages.sh first"

# SOURCE_DATE_EPOCH: commit time of HEAD unless already set (no reproducibility claim).
git_cmd=(git -c "safe.directory=${repo_root}" -C "${repo_root}")
if [ -z "${SOURCE_DATE_EPOCH:-}" ]; then
    SOURCE_DATE_EPOCH=$("${git_cmd[@]}" log -1 --format=%ct) || die "cannot read commit time of HEAD"
fi
export SOURCE_DATE_EPOCH

# Prepare the work directory. Only a directory this script created earlier
# (marked) is ever cleaned or reused.
if [ -e "${work_dir}" ] && [ ! -f "${work_dir}/${marker_name}" ]; then
    die "work dir ${work_dir} exists and is not a cham3 live-build work dir (no ${marker_name}); refusing to touch it"
fi
mkdir -p "${work_dir}" "${image_dir}"
if [ -d "${work_dir}/auto" ]; then
    (cd "${work_dir}" && lb clean --purge) || die "lb clean --purge failed in ${work_dir}"
fi
rm -rf "${work_dir:?}/auto" "${work_dir:?}/config"
rm -f "${image_dir}/${image_name}".* "${image_dir}/build.log" "${image_dir}/provenance.json" "${image_dir}/SHA256SUMS"
touch "${work_dir}/${marker_name}"
cp -a "${repo_root}/build/live/." "${work_dir}/"
mkdir -p "${work_dir}/config/packages.chroot"
cp -p "${deb_files[@]}" "${work_dir}/config/packages.chroot/"

started_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)
log_file="${image_dir}/build.log"
{
    echo "build_image: started ${started_utc}"
    echo "build_image: SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH} unverified_host_override=${unverified_override}"
    echo "build_image: live-build $(dpkg-query -W -f='${Version}' live-build)"
} | tee "${log_file}"

# Run one live-build step in the work dir, appending its output to the log, and
# stop on failure. Each step's status is checked explicitly: errexit does not apply
# inside a subshell whose status is tested, so a failed cd or "lb config" must
# never be followed by the next step. On failure nothing is published; only the
# diagnostic build.log is left in the image dir.
run_lb_step() { # lb subcommand
    local step_status=0
    (
        cd -- "${work_dir}" || { echo "build_image: cannot enter work dir ${work_dir}"; exit 1; }
        lb "$1"
    ) 2>&1 | tee -a "${log_file}" || step_status=$?
    [ "${step_status}" -eq 0 ] || die "lb $1 failed (exit ${step_status}); see ${log_file}"
}

run_lb_step config
run_lb_step build

finished_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)
echo "build_image: finished ${finished_utc}" >> "${log_file}"

shopt -s nullglob
iso_candidates=("${work_dir}"/*.iso)
shopt -u nullglob
[ "${#iso_candidates[@]}" -eq 1 ] || die "expected exactly one ISO in ${work_dir}, found ${#iso_candidates[@]}"
cp -p "${iso_candidates[0]}" "${image_dir}/${image_name}.iso"

shopt -s nullglob
manifest_candidates=("${work_dir}"/*.packages)
shopt -u nullglob
[ "${#manifest_candidates[@]}" -eq 1 ] || die "expected exactly one *.packages manifest in ${work_dir}, found ${#manifest_candidates[@]}"
manifest_source=${manifest_candidates[0]}
cp -p "${manifest_source}" "${image_dir}/${image_name}.packages"

python3 "${script_dir}/write_provenance.py" \
    --output "${image_dir}/provenance.json" \
    --repo-root "${repo_root}" \
    --work-dir "${work_dir}" \
    --packages-dir "${packages_dir}" \
    --artifact "${image_dir}/${image_name}.iso" \
    --artifact "${image_dir}/${image_name}.packages" \
    --artifact "${log_file}" \
    --build-started-utc "${started_utc}" \
    --build-finished-utc "${finished_utc}" \
    --source-date-epoch "${SOURCE_DATE_EPOCH}" \
    --unverified-host-override "${unverified_override}"

(cd "${image_dir}" && sha256sum -- "${image_name}.iso" "${image_name}.packages" build.log provenance.json > SHA256SUMS)

echo "build_image: PASS (artifacts in ${image_dir})"
