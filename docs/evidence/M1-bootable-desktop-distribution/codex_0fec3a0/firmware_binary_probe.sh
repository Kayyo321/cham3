#!/bin/sh
# Reproduce the exact live-build installer-side selector; no packages or firmware are installed.
set -e
mkdir -p /tmp/codex-firmware-native
cd /tmp/codex-firmware-native
. /usr/lib/live/build.sh
LB_ARCHITECTURE=amd64
LB_CACHE=true
WGET_OPTIONS=--no-verbose
FIRMWARE_DETAILS_FILE=/workspace/M1-rereview-evidence/installer_firmware_details.txt
FIRMWARE_PACKAGES=firmware-linux
Firmware_List_From_Contents http://deb.debian.org/debian trixie 'main contrib non-free non-free-firmware'
for package in $FIRMWARE_PACKAGES; do
 printf '%s\n' "${package##*/}"
done | sort -u > /workspace/M1-rereview-evidence/installer_firmware_packages.txt
cat /workspace/M1-rereview-evidence/installer_firmware_packages.txt
