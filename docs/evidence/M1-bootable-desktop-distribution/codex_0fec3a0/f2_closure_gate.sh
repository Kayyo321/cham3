#!/bin/sh
# GATE F2-CLOSURE: simulation only. Native x86-64. Writes transcripts to /out/f2_closure.
set -eu
out=/out/f2_closure
mkdir -p /work "$out"

# 1. Approved Debian suites only (no backports).
cat > /etc/apt/sources.list.d/debian.sources <<'SRC'
Types: deb
URIs: http://deb.debian.org/debian
Suites: trixie trixie-updates
Components: main contrib non-free non-free-firmware
Signed-By: /usr/share/keyrings/debian-archive-keyring.pgp

Types: deb
URIs: http://deb.debian.org/debian-security
Suites: trixie-security
Components: main contrib non-free non-free-firmware
Signed-By: /usr/share/keyrings/debian-archive-keyring.pgp
SRC
rm -f /etc/apt/sources.list
apt-get -q update
apt-get -q install -y --no-install-recommends ca-certificates curl gnupg

# 2. Kali key over HTTPS with certificate verification; check SHA-256 and primary fingerprint.
curl -fsS --proto '=https' --tlsv1.2 -o /work/kali_archive_key.asc https://archive.kali.org/archive-key.asc
echo 'bbaef4b375fdae6d361d648355c9a003b081d421a40f39440f296cbd458071b1  /work/kali_archive_key.asc' | sha256sum -c -
gpg --batch --with-colons --show-keys /work/kali_archive_key.asc | awk -F: '$1=="fpr"{print $10; exit}' | grep -qx '827C8569F2518CC677FECA1AED65462EC8D5E4C5'
gpg --batch --dearmor -o /usr/share/keyrings/cham3_kali_archive.gpg /work/kali_archive_key.asc

# 3. LibreWolf key: same checks (SHA-256 cd1c45eb..., primary 662E3CDD...).
curl -fsS --proto '=https' --tlsv1.2 -o /work/librewolf_pubkey.gpg https://repo.librewolf.net/pubkey.gpg
echo 'cd1c45ebf33b42191af369b5ea901508eb7dff3dd5086abcb601b25121bc70ad  /work/librewolf_pubkey.gpg' | sha256sum -c -
gpg --batch --with-colons --show-keys /work/librewolf_pubkey.gpg | awk -F: '$1=="fpr"{print $10; exit}' | grep -qx '662E3CDD6FE329002D0CA5BB40339DD82B12EF16'
gpg --batch --dearmor -o /usr/share/keyrings/cham3_librewolf.gpg /work/librewolf_pubkey.gpg

# 4. Candidate sources (proposed). Kali: main and non-free (non-free only for the LEAKY non-free tests).
cat > /etc/apt/sources.list.d/cham3_kali.sources <<'SRC'
Types: deb
URIs: http://http.kali.org/kali/
Suites: kali-rolling
Components: main non-free
Signed-By: /usr/share/keyrings/cham3_kali_archive.gpg
SRC
cat > /etc/apt/sources.list.d/cham3_librewolf.sources <<'SRC'
Types: deb
URIs: https://repo.librewolf.net
Suites: librewolf
Components: main
Architectures: amd64
Signed-By: /usr/share/keyrings/cham3_librewolf.gpg
SRC
apt-get -q update > "$out/apt_update.log" 2>&1
! grep -E 'NO_PUBKEY|GPG error|NOSPLIT' "$out/apt_update.log"

# 5. Pin helper. Kali and LibreWolf origins are blocked (-1) by default. Only the named packages are raised to 500.
pin_file=/etc/apt/preferences.d/cham3_gate.pref
pin_for() {
  {
    printf 'Package: *\nPin: release o=Kali\nPin-Priority: -1\n\n'
    printf 'Package: *\nPin: release n=librewolf\nPin-Priority: -1\n\n'
    for name in "$@"; do
      if [ "$name" = librewolf ]; then
        printf 'Package: librewolf\nPin: release n=librewolf\nPin-Priority: 500\n\n'
      else
        printf 'Package: %s\nPin: release o=Kali\nPin-Priority: 500\n\n' "$name"
      fi
    done
  } > "$pin_file"
}

# 6. Strict closures. The allowlist is the candidate plus the Kali-only names from the emulated solver
#    (disposition section 7). The native run must re-derive any name the index no longer matches.
sim() {
  label=$1; target=$2; shift 2
  pin_for "$target" "$@"
  apt-get -s install --no-install-recommends -y "$target" > "$out/$label.strict.log" 2>&1 || true
  printf '%s e_lines=%s kali_only_inst=%s\n' "$label" \
    "$(grep -c '^E: ' "$out/$label.strict.log" || true)" \
    "$(grep '^Inst ' "$out/$label.strict.log" | grep 'kali-rolling' | grep -vc 'Debian' || true)" >> "$out/summary.txt"
}
sim zaproxy zaproxy
sim burpsuite burpsuite
sim exploitdb exploitdb
sim ghidra ghidra ghidra-data
sim amass amass libpostal-data
sim radare2 radare2 libradare2-6.0.0t64 libradare2-common
sim mitmproxy mitmproxy python3-mitmproxy-rs python3-mitmproxy-linux libpython3.14 libpython3.14-stdlib libpython3.14-minimal
sim wpscan wpscan ruby-cms-scanner
sim rizin rizin librizin0 librizin-common
sim rizin_cutter rizin-cutter libcgraph8 libgvc7 libpyside6-py3-6.10 libpython3.14
sim metasploit_framework metasploit-framework oracle-instantclient-basic

# 7. Controls: each candidate must be unavailable with no allowlist (expect "has no installation candidate").
pin_for
for name in zaproxy burpsuite exploitdb ghidra amass radare2 mitmproxy wpscan rizin rizin-cutter metasploit-framework; do
  apt-get -s install --no-install-recommends -y "$name" > "$out/$name.control.log" 2>&1 || true
done

# 8. Measure (leak measurement only; never used to install): Kali at 500, no allowlist.
printf 'Package: *\nPin: release o=Kali\nPin-Priority: 500\n\n' > "$pin_file"
for name in zaproxy ghidra amass radare2 mitmproxy; do
  apt-get -s install --no-install-recommends -y "$name" > "$out/$name.measure.log" 2>&1 || true
  grep '^Inst [^ ]* \[' "$out/$name.measure.log" | grep 'kali-rolling' | grep -v 'Debian' > "$out/$name.measure_base_upgrades.txt" || true
done

# 9. Origin preservation: strict design, base container, no allowlist. Expect zero Kali-origin lines.
pin_for
apt-cache policy libc6 libgcc-s1 libstdc++6 curl > "$out/policy_strict.txt"
apt-get -s dist-upgrade > "$out/dist_upgrade_strict.log" 2>&1 || true
test "$(grep '^Inst ' "$out/dist_upgrade_strict.log" | grep -c 'kali-rolling' || true)" -eq 0

# 10. LibreWolf (own repository): strict design with librewolf at 500.
pin_for librewolf
apt-get -s install --no-install-recommends -y librewolf > "$out/librewolf.strict.log" 2>&1 || true
grep '^Inst ' "$out/librewolf.strict.log" | grep 'LibreWolf Software Repository' > "$out/librewolf_origin_lines.txt" || true

# 11. M1 context: the 53 Debian names of cham3-desktop and cham3-full plus each CLEAN candidate.
m1_names='aircrack-ng ark bind9-dnsutils binwalk dbus-user-session dolphin ffuf fonts-noto-color-emoji fonts-noto-core gobuster gwenview hashcat hydra iftop john kate kde-spectacle kf6-breeze-icon-theme kio-extras konsole kscreen kwin-wayland libimage-exiftool-perl libpam-systemd masscan nethogs nikto nmap okular plasma-desktop plasma-nm plasma-pa plasma-systemmonitor plasma-workspace powerdevil qml6-module-org-kde-kirigami qml6-module-org-kde-plasma-plasma5support qt6-image-formats-plugins sddm sddm-theme-debian-breeze sqlmap systemsettings tcpdump torbrowser-launcher tshark unzip whois wireplumber wireshark xdg-desktop-portal-kde xwayland zenmap zip'
pin_for zaproxy
apt-get -s install --no-install-recommends -y $m1_names zaproxy > "$out/m1_context_zaproxy.log" 2>&1 || true
# Repeat the m1 step for exploitdb, ghidra (+ ghidra-data), amass (+ libpostal-data), and radare2 (+ its two libraries).

# 12. Transcript checksums.
( cd "$out" && sha256sum ./* > SHA256SUMS )
