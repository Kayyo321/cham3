# M1 readiness packet: built-in application suite (revision 2)

- **Status:** RESEARCH PACKET for independent Codex review. It is not an approval of the suite and not an implementation. No package in it has been installed, built, booted, or approved. The Debian subset is available. The complete requested suite is NOT satisfied.
- **Suite status (2026-10-11):** UNRESOLVED. Nine requested items have no approved, verified source yet (section 2). `M1_acceptance_matrix_corrections.md` sets A8 to a non-PASS status until that changes.
- **Detail:** `M1_package_disposition.md` (per-tool matrix, authenticated-source designs, closure and origin results, firmware analysis, evidence index).
- **Environment limit:** every simulation ran in amd64 containers under emulation on an arm64 macOS host. The results are early signals, not native x86-64 evidence. The native gates in section 6 decide.
- **Scope rule (task):** Debian 13 first; a Debian-native equivalent second; a Kali-origin package only where no suitable Debian option exists, only through a narrowly pinned and separately reviewed source; no general Kali repository.

## 1. Debian subset (available in trixie; not yet built or installed)

Availability is confirmed from the signed trixie index and apt candidates. Installation and execution are NOT confirmed. Re-record versions at the target commit.

| Area | Debian 13 packages (version as observed, area) |
|---|---|
| Privacy browser (Tor) | `torbrowser-launcher` 0.3.7-3 (contrib, sha256 `a70a9da8...`). Recommends `tor` 0.4.9.14-0+deb13u1 (trixie-security). |
| Traffic analysis | `wireshark-common` 4.4.19-0+deb13u1 candidate (pool 4.4.18); `wireshark` and `tshark` (main); `tcpdump` 4.99.5-2; `nethogs` 0.8.8-1; `iftop` 1.0~pre4-9+b1 |
| Information gathering | `nmap` 7.95+dfsg-3; `zenmap` 7.95+dfsg-3; `masscan` 2:1.3.2+ds1-2; `bind9-dnsutils` 1:9.20.26-1~deb13u1 (for `dnsutils`); `whois` 5.6.3 |
| Web application analysis | `sqlmap` 1.9.6-1; `ffuf` 2.1.0-1+b9; `gobuster` 3.6.0-1+b10; `nikto` 1:2.1.5-3.1 (**non-free**) |
| Wireless | `aircrack-ng` 1:1.7+git20230807.4bf83f1a-2 |
| Password and crypto auditing | `hashcat` 6.2.6+ds2-1; `john` 1.9.0-2; `hydra` 9.5-3 |
| Reverse engineering and forensics | `binwalk` 2.4.3+dfsg1-2+deb13u1; `libimage-exiftool-perl` 13.25+dfsg-1 (virtual `exiftool`) |
| KDE core (keep) | `plasma-desktop` 4:6.3.6-1; `plasma-workspace` 4:6.3.6-2 (Wayland session `plasma.desktop`); `kwin-wayland` 4:6.3.6-1; `sddm` 0.21.0+git20250502.4fe234b-2; `systemsettings` 4:6.3.4-2; `dolphin` 4:25.04.3-1+deb13u1; `konsole` 4:25.04.2-1; `kate` 4:25.04.3-2; `ark` 4:25.04.3-1; `okular` 4:25.04.2-1+deb13u1; `gwenview` 4:25.04.3-1; `kde-spectacle` 4:6.3.5-2; `plasma-systemmonitor` 6.3.6-1; `fonts-noto` 20201225-2; `qml-module-org-kde-qqc2desktopstyle` 5.116.1-1 |

Substitutions to accept at review: `plasma-workspace-wayland` to `plasma-workspace`; `spectacle` to `kde-spectacle`; `qqc2-desktop-style` to `qml-module-org-kde-qqc2desktopstyle`; `dnsutils` to `bind9-dnsutils`; `exiftool` to `libimage-exiftool-perl`.

Excluded alternatives (must be absent from the image closure): `thunar`, `pcmanfm`, `qterminal`, `xfce4-terminal`, `tilix`, `mousepad`, `xarchiver`, `file-roller`, `evince`, `atril`, `ristretto`, and `firefox-esr` as the stock browser. `leafpad` is not in trixie.

Debian-set evidence still required at the target: the trixie `Release` signature (GOODSIG verified with `04B54C3CDCA79751B16BC6B5225629DF75B188BD`; a second signature `4CB5…E131` is not in the shipped keyring and is not verified), `getcap` and mode checks for dumpcap (section 4), and the `nikto` non-free licence decision.

## 2. Foreign roles: status, closure class, and blocker

Closure classes (early signal, emulated; the native gate in section 6 governs): **CLEAN** (only the candidate comes from Kali); **CLEAN_WITH_KALI_ONLY_DEPS** (also needs Kali-only names absent from trixie); **LEAKY_BASE_SKEW** (needs a Kali replacement of a package trixie provides); **UNRESOLVABLE**.

| # | Requested item | Source considered | Closure class | Blocker or condition | Owner decision needed |
|---|---|---|---|---|---|
| 1 | LibreWolf (clearnet browser) | Official repository `repo.librewolf.net` (not Kali, not Debian). Key provenance in disposition section 1.2 and 3.1. | CLEAN (own repository, 92 Debian dependencies) | None technical. Third-party source needs approval. | Approve or reject the source. |
| 2 | Official Tor Browser | Debian `torbrowser-launcher` 0.3.7-3 (contrib) | Debian package; no Kali path | Self-update and first-use download outside apt | Accept the update path (disposition 3.2). |
| 3 | mitmproxy / mitmweb | Kali `mitmproxy` 12.2.3-0kali5; upstream PyPI 12.2.3 (not validated) | LEAKY_BASE_SKEW | Kali-only Python 3.14 stack; `libc6 >= 2.42` (trixie 2.41) | Scope amendment, or an approved upstream PyPI route with a hash-locked dependency set. |
| 4 | OWASP ZAP or Burp Suite Community | ZAP: Kali `zaproxy` 2.17.0-0kali1 (CLEAN); Burp: Kali `burpsuite` 2026.8-0kali1 (CLEAN technically) | ZAP CLEAN; Burp CLEAN but licence-blocked | Burp licence and redistribution review is blocking. | Choose ZAP or Burp (or neither). |
| 5 | amass | Kali `amass` 5.1.1-0kali3 + Kali-only `libpostal-data` 1.1.4-0kali1 | CLEAN_WITH_KALI_ONLY_DEPS | Allowlist of two packages; runtime model downloads by the user | Approve the two-package allowlist. |
| 6 | wpscan | Kali `wpscan` 4.1.0-0kali1 (non-free) | LEAKY_BASE_SKEW | `ruby-addressable >= 2.9` (trixie 2.8.7-2); Kali-only `ruby-cms-scanner` | Scope amendment, or omit. |
| 7 | metasploit-framework | Kali `metasploit-framework` 6.5.3-0kali1; Rapid7 apt returned HTTP 403 here (BLOCKED) | LEAKY_BASE_SKEW | Kali non-free `oracle-instantclient-basic` (proprietary client) needs `libaio1t64 >= 0.3.113-8+kali1` | Scope amendment, or omit. |
| 8 | searchsploit (exploitdb) | Kali `exploitdb` 20260904-0kali1 (`/usr/bin/searchsploit` confirmed) | CLEAN | Allowlist of one package; data licence not checked | Approve the allowlist entry. |
| 9 | Ghidra | Kali `ghidra` 12.1.4+ds-0kali1 with Kali-only `ghidra-data` 10.5-0kali1, JDK from Debian-Security; or NSA GitHub release | CLEAN (strict design; 132 to 133 new packages) | About 0.9 GB installed plus a JDK; `ghidra-data` needed for function is NOT_RUN | Accept the footprint and source, or choose the upstream release. |
| 10 | radare2 or Cutter | radare2: Kali `radare2` 6.0.4+dfsg-1 with two Kali-only libraries (CLEAN_WITH_KALI_ONLY_DEPS). Cutter: Kali `rizin-cutter` 2.5.0-0kali1 (LEAKY: Qt 6.10 and Python 3.14 from Kali). | radare2 CLEAN_WITH_KALI_ONLY_DEPS; Cutter LEAKY | Cutter needs a Kali Qt and Python replacement | Choose radare2 (recommended by this revision) or drop Cutter. |

Items 3, 6, 7, and the Cutter branch of item 10 are the LEAKY set. They cannot enter the image through the approved mechanism without a base-library replacement from Kali, which the task does not permit.

Also recorded: `rizin` (Kali 0.9.1-0kali1) is LEAKY (`librizin0` needs `libc6 >= 2.42`). Its only use is Cutter's dependency.

### 2.1 Authenticated-source designs (not enabled)

LibreWolf and Kali designs, including keyring files, pins, and the origin-change rule, are in disposition section 3. Summary:

- **LibreWolf:** `Signed-By` a keyring shipped in packaging (sha256 `cd1c45eb...`, primary `662E3CDD6FE329002D0CA5BB40339DD82B12EF16`). Pin `Package: *`, `Pin: release n=librewolf`, `Pin-Priority: -1`; `librewolf` at 500. Do not run extrepo at build or install time.
- **Kali:** `Signed-By` a single-key keyring for `827C8569F2518CC677FECA1AED65462EC8D5E4C5` (armored sha256 `bbaef4b3...`). Exclude the legacy 2012 key `44C6513A…0BF6`. Pin `Package: *`, `Pin: release o=Kali`, `Pin-Priority: -1`; each owner-approved name at 500, and no others. Pin-Priority 50 is rejected: it installed unlisted Kali packages in simulation.

## 3. Owner and architecture decisions (summary; full list in disposition section 10)

1. LibreWolf: approve the third-party repository (item 1).
2. Tor Browser: accept the launcher's self-update and first-use download outside apt (item 2).
3. Kali allowlist: approve per-package entries for the CLEAN items (items 5, 8, 9, 10-radare2), each subject to the native gate and Codex re-review.
4. Scope amendment or omission for the LEAKY items (3, 6, 7, and Cutter).
5. ZAP or Burp (item 4). Burp requires a licence review first.
6. Firmware: accept the explicit removal of the two b43 installers, or approve a build-time, hash-pinned blob package with licence review (section 5).
7. Wireshark: approve the fail-closed capture design (section 4).
8. Kali keyring: approve shipping a single-key keyring in packaging.
9. Name substitutions and Zenmap (section 1).

## 4. Wireshark and dumpcap capture privilege (corrected)

- Keep `wireshark-common/install-setuid boolean true` in `build/live-desktop/config/preseed/cham3_wireshark.cfg.chroot`. `true` gives the `wireshark` group with `cap_net_raw,cap_net_admin` file capabilities. `false` removes the non-root capture mechanism entirely.
- The defect Codex F3 describes is the setuid fallback. The upstream postinst applies `u=rwxs` when `setcap` fails, and when `setcap` is absent. `libcap2-bin` is a hard dependency of `wireshark-common`, so the fallback is reached only when `setcap` fails (for example on a filesystem without security xattrs). The fix must make that path fail closed, and gate F3-CAP must test it.
- Membership: no automatic members (Debian README). Membership is capture authority.
- Open: whether `security.capability` xattrs survive the live-build squashfs (NOT_RUN).
- Earlier advice to "set the non-setuid answer" is withdrawn (disposition section 5).

## 5. Firmware blocker: two install-time fetch packages (new)

- live-build scheduled `firmware-b43-installer` and `firmware-b43legacy-installer` (contrib) into the image because their contents are `.placeholder` files under `usr/lib/firmware/`. The M1 config's `--firmware-chroot true` makes live-build schedule every such package in every configured archive area.
- Both postinsts download fixed URLs (GitHub; `downloads.openwrt.org`) and verify an embedded SHA-512. In a chroot they always download, whatever the hardware. Codex's M1 image attempt failed here (DNS and proxy 403), which is why M1 has no ISO.
- A static audit of all 47 pre-blocklist firmware packages found network fetches only in these two.
- No Debian archive area ships b43 or b43legacy blobs. Self-contained coverage for brcmfmac and brcmsmac is in `firmware-brcm80211`.
- Recommendation: set `--firmware-chroot false` and use an explicit 42-package list (`docs/research/evidence/firmware/proposed_set_without_b43_installers_42.txt`). This removes the build-time fetch and is an explicit reduction: b43 and b43legacy SoftMAC Wi-Fi do not work out of the box. The installed-system opt-in path (`apt install firmware-b43-installer`) remains. The config change belongs to m1_worker.

## 6. Native verification gates for Codex Remote (fold into `codex_remote_verification.md`)

Run on the native x86-64 Codex Remote Debian 13 host, inside a disposable `debian:trixie` container with `/out` mounted. All gates are simulations (`apt-get -s`), not installations. Record each criterion as PASS, FAIL, NOT_RUN, or BLOCKED with the transcript path. The native results replace the emulated early signals in disposition section 7.

### 6.1 Gate F2-CLOSURE (closure, signatures, and pins)

```sh
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
```

Pass criteria for F2-CLOSURE:

- **P1 (CLEAN candidates: zaproxy, exploitdb, ghidra, amass, radare2):** `e_lines=0`; no `Remv` line; the Kali-origin `Inst` names equal the allowlist exactly.
- **P2 (LEAKY candidates: mitmproxy, wpscan, rizin, rizin-cutter, metasploit-framework):** still `E:` lines. If one now resolves, record the new class and re-review before any inclusion.
- **P3 (controls):** every control log contains `has no installation candidate`.
- **P4 (origin preservation):** `dist_upgrade_strict.log` has zero Kali-origin `Inst` lines (the `test` in step 9 passes).
- **P5 (LibreWolf):** exit 0; `librewolf_origin_lines.txt` contains only `librewolf`.
- **P6 (M1 context):** exit 0 for each CLEAN candidate with the 53-name set.
- **P7 (signatures):** `apt_update.log` has no NO_PUBKEY or GPG error; the three key checks match the recorded fingerprints and hashes.

Known limits: the allowlists come from the emulated solver on 2026-10-11. Native results may differ if the indices changed. Simulation does not prove that a package installs or runs.

### 6.2 Gate F2-ORIGIN (upgrade behaviour with an allowlisted package)

Optional and must be approved by the coordinator, because it needs installed state. The default is the simulated installed state used in disposition section 3.3: append one dpkg status stanza in the disposable container, run `apt-get -s dist-upgrade` under the strict pin, then restore the status file. Pass only if the only Kali-origin line is the allowlisted package. Record the simulation as simulation.

### 6.3 Gate FW-B43 (firmware scheduling and fetch)

```sh
# GATE FW-B43: static and build-config checks. No installer is run.
apt-cache rdepends --no-recommends --no-suggests firmware-b43-installer firmware-b43legacy-installer
# Expect: no reverse dependencies.

# Reproduce the live-build selection from the indices live-build fetches:
#   for area in main contrib non-free non-free-firmware; do
#     for arch in all amd64; do
#       curl -fsS "http://deb.debian.org/debian/dists/trixie/$area/Contents-$arch.gz" | gunzip -c |
#         awk '/^(usr\/)?lib\/firmware/ { print $NF }'
#     done
#   done | sort -u
# Remove the section prefix, drop firmware-nvidia-gsp and firmware-nvidia-tesla-gsp, apply the amd64 ARM blocklist,
# and add firmware-linux. Expect 44 names; without the two installers, 42.
# Compare with docs/research/evidence/firmware/proposed_set_without_b43_installers_42.txt.

# After the change (--firmware-chroot false and an explicit package list):
dpkg-query -W 'firmware-b43*' 2>/dev/null      # expect: empty on the built system
# Build log must not contain: "Resolving github.com", "downloads.openwrt.org", or "Some problem occurred during the firmware download".
```

Pass criteria: no `firmware-b43*` package in the chroot or the installed system; the chroot firmware set equals the reviewed 42-package list; the build log contains no fetch attempts for firmware.

### 6.4 Gate F3-CAP (dumpcap capture privilege)

```sh
# GATE F3-CAP: installed system (disposable VM). Pass only if no setuid bit is ever set.
stat -c '%a %U:%G %n' /usr/bin/dumpcap      # expect: 754 root:wireshark
getcap /usr/bin/dumpcap                     # expect: cap_net_admin,cap_net_raw=eip
# Failure path (negative control): run the wireshark-common postinst with a setcap stub that fails.
# Expect: no setuid bit. The upstream postinst produces 4754 here, so the test FAILS until the packaging change lands.
# Membership: a non-member capture is denied; a member capture succeeds.
# Live system: getcap on the mounted live /usr/bin/dumpcap (NOT_RUN until the ISO exists).
```

## 7. What this packet does NOT claim

- No package is installed, built, booted, approved, or enabled.
- No foreign source is approved. LibreWolf, the Kali allowlist, and any upstream route need owner approval first.
- Closure and origin results are emulated early signals. Only section 6 gates, run natively, can confirm them.
- The LibreWolf key has no independent second channel. The trixie second signature `4CB5…E131` is unverified.
- Firmware disposition is a recommendation. It has not been implemented or tested on hardware.
- The Wireshark setuid fallback is unfixed. Its test is designed but not run.
- Package availability and closure are not evidence of security, correctness, or M2–M4 enforcement.
