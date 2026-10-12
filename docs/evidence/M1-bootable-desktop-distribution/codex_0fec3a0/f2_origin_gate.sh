#!/bin/sh
# Native gate prerequisite: authentic live indices are mandatory; no empty-index false PASS.
set -e
export DEBIAN_FRONTEND=noninteractive
dpkg -i /out/cham3-archive-keyring_0.0.0~m1_all.deb
cp /out/zzz_cham3_kali.list /out/zzz_cham3_librewolf.list /etc/apt/sources.list.d/
cp /out/cham3_kali.pref /out/cham3_librewolf.pref /etc/apt/preferences.d/
apt-get -o APT::Update::Error-Mode=any update
apt-cache policy libc6 libgcc-s1 libstdc++6 curl
apt-get -s dist-upgrade
# Strict allowlisted simulated-state and naive controls require these successful indices first.
