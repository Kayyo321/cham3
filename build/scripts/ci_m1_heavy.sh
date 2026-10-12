#!/bin/bash
# Purpose: the M1 heavy gates, run INSIDE a disposable privileged debian:trixie
# (Debian 13, native amd64) container by .github/workflows/m1_image.yml.
#   ci_m1_heavy.sh iso   build the packages and the M1 ISO, run image-m1 (including
#                        the ISO pool / installer payload audit) and the build-log
#                        firmware-fetch check; evidence in out/evidence/iso/
#   ci_m1_heavy.sh vm    boot the ISO (live), install it to a disposable disk and boot
#                        the installed system (build/vm/run_m1_vm_checks.py);
#                        evidence in out/vm/
# Run from the repository root. Never run on a host you care about: it installs
# packages, needs --privileged for live-build and uses /dev/kvm when available.
# Network: the runner's ordinary outbound access; TLS and apt signature checks
# stay enabled, no proxy or CA is injected, nothing is marked trusted.
set -euo pipefail

mode=${1:-}
export DEBIAN_FRONTEND=noninteractive
repo_root=$(pwd)
evidence_root="${repo_root}/out/evidence"
iso_name=cham3-m1-desktop-amd64

install_host_packages() {
    apt-get update
    # shellcheck disable=SC2046  # one package name per line in the list
    apt-get install -y --no-install-recommends $(grep -v '^#' build/host_packages.txt) desktop-file-utils git "$@"
    git config --global --add safe.directory "${repo_root}"
}

record_environment() {
    local target=$1
    mkdir -p "${target}"
    {
        echo "date_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
        echo "commit=$(git rev-parse HEAD)"
        echo "kernel=$(uname -srm)"
        grep -E '^(PRETTY_NAME|VERSION_ID)=' /etc/os-release
        echo "dpkg_architecture=$(dpkg --print-architecture)"
        echo "kvm=$([ -w /dev/kvm ] && echo usable || echo absent)"
        echo "nproc=$(nproc)"
        free -m | sed -n 2p
        df -h "${repo_root}" | tail -1
    } > "${target}/environment.txt"
    dpkg-query -W -f='${Package} ${Version}\n' live-build debootstrap xorriso squashfs-tools \
        qemu-system-x86 qemu-utils tesseract-ocr 2> /dev/null > "${target}/tool_versions.txt" || true
}

run_iso() {
    local evidence="${evidence_root}/iso"
    install_host_packages
    record_environment "${evidence}"
    build/scripts/build_packages.sh 2>&1 | tee "${evidence}/build_packages.log"
    CHAM3_LIVE_DIR=build/live-desktop CHAM3_IMAGE_NAME="${iso_name}" build/scripts/build_image.sh \
        2>&1 | tee "${evidence}/build_image_console.log"
    local status=0
    tests/run_tests.sh image-m1 2>&1 | tee "${evidence}/image_m1_tests.log" || status=1
    # No firmware host may be contacted during the build (b43 installers, F2/F7). The
    # markers are those of the b43 installers' download step (readiness packet 6.3).
    if grep -E -n 'Resolving github\.com|downloads\.openwrt\.org|Some problem occurred during the firmware download' \
        out/image/build.log > "${evidence}/firmware_fetch_matches.txt"; then
        echo "FAIL: the build log shows a firmware download attempt" | tee -a "${evidence}/firmware_fetch_matches.txt"
        status=1
    else
        echo "PASS: no firmware host in the build log" > "${evidence}/firmware_fetch_matches.txt"
    fi
    xorriso -indev "out/image/${iso_name}.iso" -find /pool -name '*.deb' > "${evidence}/iso_pool_debs.txt" 2>&1 || status=1
    xorriso -indev "out/image/${iso_name}.iso" -find /firmware > "${evidence}/iso_firmware_dir.txt" 2>&1 || status=1
    (cd out/image && sha256sum ./*.iso ./*.packages ./*.json ./build.log 2> /dev/null) > "${evidence}/artifact_SHA256SUMS"
    cp out/image/"${iso_name}".packages out/image/provenance.json out/image/SHA256SUMS "${evidence}/" 2> /dev/null || true
    return "${status}"
}

run_vm() {
    local evidence="${repo_root}/out/vm"
    local iso="out/image/${iso_name}.iso"
    local disk="${repo_root}/out/vm_disk/installed.qcow2"
    local phases=${CHAM3_VM_PHASES:-live install installed}
    install_host_packages qemu-system-x86 qemu-utils xorriso cpio gzip tesseract-ocr tesseract-ocr-eng \
        python3-pil ca-certificates
    record_environment "${evidence}"
    mkdir -p "$(dirname "${disk}")"
    sha256sum "${iso}" > "${evidence}/iso_SHA256SUMS"
    local status=0
    local phase
    for phase in ${phases}; do
        case "${phase}" in
            live)
                python3 build/vm/run_m1_vm_checks.py live --iso "${iso}" --out "${evidence}/live" || status=1
                ;;
            install)
                python3 build/vm/run_m1_vm_checks.py install --iso "${iso}" --disk "${disk}" \
                    --out "${evidence}/install" || status=1
                ;;
            installed)
                python3 build/vm/run_m1_vm_checks.py installed --disk "${disk}" --out "${evidence}/installed" \
                    || status=1
                ;;
            *)
                echo "unknown VM phase: ${phase}" >&2
                status=1
                ;;
        esac
    done
    return "${status}"
}

case "${mode}" in
    iso) run_iso ;;
    vm) run_vm ;;
    *)
        echo "usage: $0 iso|vm" >&2
        exit 2
        ;;
esac
