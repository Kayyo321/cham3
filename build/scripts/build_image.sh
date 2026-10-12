#!/bin/bash
# Purpose: build the cham3 M0 development live image with Debian live-build.
# REQUIRES root and a Debian 13 amd64 build host (live-build uses chroot/mounts).
# Run it only in a disposable build environment (VM or privileged container).
# Needs: packages built first by build/scripts/build_packages.sh, network access to
# the official Debian mirrors, and the tools in build/host_packages.txt.
# Environment:
#   CHAM3_OUT_DIR                  output root (default <repo>/out)
#   CHAM3_WORK_DIR                 live-build work dir (default $CHAM3_OUT_DIR/live-build-work)
#   CHAM3_LIVE_DIR                 live-build config dir / image profile (default build/live);
#                                  the M1 desktop image uses build/live-desktop
#   CHAM3_IMAGE_NAME               output image basename (default cham3-m0-dev-amd64)
#   CHAM3_ALLOW_UNVERIFIED_HOST=1  allow a host that is not Debian 13 amd64; recorded
#                                  as an unverified host in provenance.json
#   CHAM3_STAGE_DRYRUN=1           print the local packages that would be staged for the
#                                  selected profile (from its cham3-local-packages manifest)
#                                  and exit, without root or a build (for tests)
# Each image profile's cham3-local-packages manifest names which locally built packages
# belong in that image; only those are staged into config/packages.chroot. live-build
# force-installs every .deb placed there, so an unparameterized M0 build must not stage
# the M1 desktop suite.
# Outputs in $CHAM3_OUT_DIR/image/: <CHAM3_IMAGE_NAME>.iso, .packages (package
# manifest), build.log, provenance.json, SHA256SUMS.
# Limits: bit-for-bit reproducibility is NOT claimed. The image is an M0 development
# artifact with no cham3 functionality, not an installable cham3 distribution.
set -eu -o pipefail

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "${script_dir}/../.." && pwd)
# Image profile selectors. Defaults reproduce the M0 development image exactly,
# so an unparameterized invocation (for example M0 CI) is unchanged. The M1
# desktop image sets CHAM3_LIVE_DIR=build/live-desktop and
# CHAM3_IMAGE_NAME=cham3-m1-desktop-amd64.
image_name=${CHAM3_IMAGE_NAME:-cham3-m0-dev-amd64}
live_dir_rel=${CHAM3_LIVE_DIR:-build/live}
marker_name=".cham3_live_build_work"

die() { echo "build_image: $*" >&2; exit 1; }

# The live-build config directory must stay inside the repository.
case "${live_dir_rel}" in
    /*|*..*) die "refusing unsafe CHAM3_LIVE_DIR: ${live_dir_rel}" ;;
esac
live_dir_abs="${repo_root}/${live_dir_rel}"
[ -d "${live_dir_abs}" ] || die "CHAM3_LIVE_DIR not found: ${live_dir_rel}"
[ -f "${live_dir_abs}/auto/config" ] || die "CHAM3_LIVE_DIR missing auto/config: ${live_dir_rel}"

# Each profile declares which locally built packages belong in its image.
local_packages_manifest="${live_dir_abs}/cham3-local-packages"
[ -f "${local_packages_manifest}" ] || die "CHAM3_LIVE_DIR missing cham3-local-packages manifest: ${live_dir_rel}/cham3-local-packages"

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

# Select, per the profile manifest, which locally built packages to stage. This runs
# before the root check so it is testable unprivileged (CHAM3_STAGE_DRYRUN=1). Matching
# is by each .deb's real Package field, not its filename.
command -v dpkg-deb > /dev/null 2>&1 || die "missing tool: dpkg-deb (see build/host_packages.txt)"
shopt -s nullglob
deb_files=("${packages_dir}"/*.deb)
shopt -u nullglob
[ "${#deb_files[@]}" -gt 0 ] || die "no .deb files in ${packages_dir}; run build/scripts/build_packages.sh first"

declare -A built_deb_by_name=()
for deb in "${deb_files[@]}"; do
    pkg_name=$(dpkg-deb -f "${deb}" Package 2> /dev/null) || die "cannot read Package field of ${deb}"
    [ -n "${pkg_name}" ] || die "empty Package field in ${deb}"
    [ -z "${built_deb_by_name[${pkg_name}]:-}" ] \
        || die "two built .deb files provide package ${pkg_name}: ${built_deb_by_name[${pkg_name}]} and ${deb}"
    built_deb_by_name[${pkg_name}]="${deb}"
done

staged_debs=()
staged_names=()
while IFS= read -r raw_line || [ -n "${raw_line}" ]; do
    line=${raw_line%%#*}
    line=$(printf '%s' "${line}" | tr -d '[:space:]')
    [ -n "${line}" ] || continue
    deb_path=${built_deb_by_name[${line}]:-}
    [ -n "${deb_path}" ] \
        || die "profile ${live_dir_rel} requires package '${line}' but no built .deb provides it (run build/scripts/build_packages.sh)"
    staged_debs+=("${deb_path}")
    staged_names+=("${line}")
done < "${local_packages_manifest}"
[ "${#staged_debs[@]}" -gt 0 ] || die "profile manifest ${live_dir_rel}/cham3-local-packages names no packages"

if [ "${CHAM3_STAGE_DRYRUN:-0}" = "1" ]; then
    printf '%s\n' "${staged_names[@]}"
    exit 0
fi

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
cp -a "${live_dir_abs}/." "${work_dir}/"
mkdir -p "${work_dir}/config/packages.chroot"
cp -p "${staged_debs[@]}" "${work_dir}/config/packages.chroot/"

started_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)
log_file="${image_dir}/build.log"
{
    echo "build_image: started ${started_utc}"
    echo "build_image: SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH} unverified_host_override=${unverified_override}"
    echo "build_image: profile=${live_dir_rel} staged local packages: ${staged_names[*]}"
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
    --live-dir "${live_dir_rel}" \
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
