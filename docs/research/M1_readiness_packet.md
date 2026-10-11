# M1 readiness packet: built-in application suite

- **Status:** RESEARCH PACKET for independent Codex review. Availability only. No package in this packet has been installed, built, or run.
- **Retrieval date:** 2026-10-11. Archive snapshots: trixie index Release 2026-09-12; Kali `kali-rolling` Release 2026-10-09.
- **Detail:** `docs/research/M1_package_disposition.md` (matrix, resolutions, short names for each index and its sha256).
- **Scope rule (task):** Debian 13 first; a Debian-native equivalent second; a Kali-origin package only if no suitable Debian option exists, and only through a narrowly pinned, separately reviewed source.

## 1. Include-now set (Debian 13 trixie, available, pending the normal M1 build review)

Availability is confirmed from the trixie archive indices. Installation and execution are NOT confirmed.

| Area | Debian packages (trixie version, area) |
|---|---|
| Privacy browser (Tor) | `torbrowser-launcher` 0.3.7-3 (contrib). Recommends `tor` 0.4.9.11-0+deb13u1 (main). |
| Traffic analysis | `wireshark` 4.4.18-0+deb13u1, `tshark` 4.4.18-0+deb13u1, `wireshark-common` 4.4.18-0+deb13u1 (net, main); `tcpdump` 4.99.5-2; `nethogs` 0.8.8-1; `iftop` 1.0~pre4-9+b1 (net, main) |
| Information gathering | `nmap` 7.95+dfsg-3; `zenmap` 7.95+dfsg-3 (net, main); `masscan` 2:1.3.2+ds1-2; `bind9-dnsutils` 1:9.20.26-1~deb13u1 (for `dnsutils`); `whois` 5.6.3 |
| Web app analysis | `sqlmap` 1.9.6-1; `ffuf` 2.1.0-1+b9 (devel); `gobuster` 3.6.0-1+b10 (utils); `nikto` 1:2.1.5-3.1 (**non-free**) |
| Wireless | `aircrack-ng` 1:1.7+git20230807.4bf83f1a-2 (net, main) |
| Password/crypto auditing | `hashcat` 6.2.6+ds2-1; `john` 1.9.0-2; `hydra` 9.5-3 (main) |
| Reverse engineering / forensics | `binwalk` 2.4.3+dfsg1-2+deb13u1 (devel, main); `libimage-exiftool-perl` 13.25+dfsg-1 (perl, main) |
| KDE core (keep) | `plasma-desktop` 4:6.3.6-1; `plasma-workspace` 4:6.3.6-2 (Wayland session `plasma.desktop`); `kwin-wayland` 4:6.3.6-1; `sddm` 0.21.0+git20250502.4fe234b-2; `systemsettings` 4:6.3.4-2; `dolphin` 4:25.04.3-1+deb13u1; `konsole` 4:25.04.2-1; `kate` 4:25.04.3-2 (`kwrite` optional, 4:25.04.3-2); `ark` 4:25.04.3-1; `okular` 4:25.04.2-1+deb13u1; `gwenview` 4:25.04.3-1; `kde-spectacle` 4:6.3.5-2; `plasma-systemmonitor` 6.3.6-1; `fonts-noto` 20201225-2; `qml-module-org-kde-qqc2desktopstyle` 5.116.1-1 (Recommends) |
| Dev/build helper (not a user app) | `extrepo` 0.14 (admin, main) only if the owner approves the LibreWolf path in section 3 |

Items confirmed present in trixie but excluded from the suite (must be absent in the image): `thunar`, `pcmanfm`, `qterminal`, `xfce4-terminal`, `tilix`, `mousepad`, `xarchiver`, `file-roller`, `evince`, `atril`, `ristretto`, `firefox-esr` (as stock browser). `leafpad` is not in trixie.

Name substitutions to accept at review: `plasma-workspace-wayland` -> `plasma-workspace`; `spectacle` -> `kde-spectacle`; `qqc2-desktop-style` -> `qml-module-org-kde-qqc2desktopstyle`; `dnsutils` -> `bind9-dnsutils`; `exiftool` -> `libimage-exiftool-perl`.

### Per-item review evidence still required (Debian set)

- Trixie Release signature verified in the build environment (not run in this research: the signing key `4CB50190207B4758A3F73A796ED0E7B82643E131` was not retrieved).
- Image sets `wireshark-common/install-setuid` explicitly (see section 4). Verify `getcap /usr/bin/dumpcap` after install (NOT_RUN).
- Confirm `nikto` non-free terms are acceptable for the image, and that `non-free` and `contrib` archive areas are enabled.
- Confirm the image's `exiftool` binary path (package description says it ships the program; the path was not verified).
- Desktop-entry validation and launcher-target checks at the image build (task requirement; not part of this research).

## 2. GATED set (NOT in the build; each needs separate review before inclusion)

### 2a. Third-party repository (not Kali, not Debian)

**LibreWolf (clearnet privacy browser).** Not in Debian, not in the Kali archive.

- Path: extrepo's `librewolf` definition, repo `https://repo.librewolf.net`, suite `librewolf`, `main`, amd64 arm64. Key fingerprint `662E 3CDD 6FE3 2900 2D0C A5BB 4033 9DD8 2B12 EF16` (LibreWolf Maintainers, created 2022-05-14), key sha256 `cd1c45ebf33b42191af369b5ea901508eb7dff3dd5086abcb601b25121bc70ad`.
- Required review evidence before inclusion:
  1. Owner approval of this third-party source (the task requires it).
  2. Independent confirmation of the LibreWolf key fingerprint against a LibreWolf project source (not done; the LibreWolf page does not publish it).
  3. Fetch `repo.librewolf.net` `InRelease`/`Release.gpg`, verify with the pinned key. NOT_RUN.
  4. Narrow pin (proposal in the matrix section 3.1): `Pin-Priority: -1` default for origin `repo.librewolf.net`, `librewolf` only at 500. Pin behavior NOT validated.
  5. Dependency closure of `librewolf` against trixie. NOT computed.
  6. Upgrade evidence: `apt upgrade` leaves Debian packages unchanged; `librewolf` updates only from its pinned origin. NOT performed.
  7. Decision whether the key ships in the cham3 packaging (preferred) or is fetched at build time with a fingerprint check. Runtime `extrepo update` is not recommended.
- Fallback (owner decision only): Debian `firefox-esr` 140.15.0esr-1~deb13u1 is excluded as the stock browser by the task. Not a substitute without a new owner decision.

**Tor Browser:** not a gated item. `torbrowser-launcher` (INCLUDE-NOW candidate, section 1) is the authenticated Debian path. Owner must accept its self-update through Tor's updater, outside apt.

### 2b. Kali-origin candidates (no Debian-native equivalent in trixie)

Every item below is GATED-KALI. None is included in the M1 build. None has a dependency closure computed. The Kali archive key `827C 8569 F251 8CC6 77FE CA1A ED65 462E C8D5 E4C5` verified the `kali-rolling` Release signature in this research (PASS).

| Tool | Kali package (version, area) | Kali pool file, sha256 | Why no Debian option | Review still required |
|---|---|---|---|---|
| OWASP ZAP | `zaproxy` 2.17.0-0kali1 (utils, all) | `pool/main/z/zaproxy/zaproxy_2.17.0-0kali1_all.deb`, `9490cc3c613ca847db8f12c5c6b00d162e17274e4fcfed566bb93d1aa1feda47` | Not in Debian (tracker 404) | Closure (Java runtime), licence, pin, upgrade |
| Burp Suite Community | `burpsuite` 2026.8-0kali1 (net, main) | `pool/main/b/burpsuite/burpsuite_2026.8-0kali1_amd64.deb`, `6e68cad47c66d854ab944aa417a54290c18b396e0727583108b790b857638b1f` (344 MB) | Not in Debian (Debian `burp` is a backup tool) | PortSwigger licence and redistribution (**blocking**), closure, pin, upgrade |
| wpscan | `wpscan` 4.1.0-0kali1 (utils, **non-free**) | `pool/non-free/w/wpscan/wpscan_4.1.0-0kali1_all.deb`, `20caf190d6adc4cc2bedc5b8df34038aaacb452853ac5dfede260aafe49583e9` | Not in Debian (tracker 404) | Non-free licence, third-party API use, Ruby closure, pin |
| Metasploit Framework | `metasploit-framework` 6.5.3-0kali1 (net, main) | `pool/main/m/metasploit-framework/metasploit-framework_6.5.3-0kali1_amd64.deb`, `fcb1100612b748b5122f3413033279693f80fd42a9e6122c3464f4db748c19b6` (270 MB) | Not in Debian (tracker 404) | Licence, Ruby closure, owner decision whether an exploit framework belongs in M1 |
| searchsploit (exploitdb) | `exploitdb` 20260904-0kali1 (utils, all) | `pool/main/e/exploitdb/exploitdb_20260904-0kali1_all.deb`, `b417d86c6e3c126d2fa2c9be2f8d6c10417697fc95ebaf5a729c299d59d3f385` | Not in Debian (tracker 404) | Confirm `searchsploit` path, data licence, pin |
| Ghidra | `ghidra` 12.1.4+ds-0kali1 (misc, main) | `pool/main/g/ghidra/ghidra_12.1.4+ds-0kali1_amd64.deb`, `128b96ae74b22eb508401378950a6b8eb3274c2e29b50f10d1a59f1fca3caad6` (515 MB) | Not in Debian (tracker 404) | JDK closure, licence, disk size, pin |
| Cutter | `rizin-cutter` 2.5.0-0kali1 + `rizin` 0.9.1-0kali1 | `rizin-cutter_2.5.0-0kali1_amd64.deb` `e0e647c191b41bd6b4ffb3bf6b76c8ed5dd381dc00ae179aab94edc7b0440d79`; `rizin_0.9.1-0kali1_amd64.deb` `41a2acc204a9adad6174139ea3b0b50639f67bb0e664bbf6601defdc91002fd1` | Not in trixie (Rizin also absent) | Qt/Rizin closure from Kali, licence, pin |
| radare2 | `radare2` 6.0.4+dfsg-1 (devel, main) | `pool/main/r/radare2/radare2_6.0.4+dfsg-1_amd64.deb`, `dcb375213e9c1477ae08d47bfb25c1b13a6ba2d41bc2665b81a4bae66bb64eb0` | Trixie has none; Debian unstable 6.1.8 is cross-release and not permitted | Owner choice radare2 vs Cutter; closure; pin |
| mitmproxy | `mitmproxy` 12.2.3-0kali5 (net, all) | `pool/main/m/mitmproxy/mitmproxy_12.2.3-0kali5_all.deb`, `0d2d281a0cc9353a612efb2295124226bf6060f1802976b0e4fe5c45dbc6a26d` | Trixie has none; Debian testing/sid 8.1.1-4 is cross-release, flagged for autoremoval 2026-11-16 and carries open CVE-2026-40606 | Closure (Python), licence, pin |
| amass | `amass` 5.1.1-0kali3 (net, main) | `pool/main/a/amass/amass_5.1.1-0kali3_amd64.deb`, `72d3563addb64ada4527170be7b45a88d7aec29acb367261365a6beecea11999` | Not in trixie (tracker 404) | Licence, closure, pin |

Evidence URLs: `https://http.kali.org/kali/dists/kali-rolling/main/binary-amd64/Packages.gz` (sha256 `5302e9aa9fcffaf31bc607e92237db807f6a236bb922eb61b97c5ed64886f1d0`); `https://http.kali.org/kali/dists/kali-rolling/non-free/binary-amd64/Packages.gz` (sha256 `8854213cd07d7e2367aa2be4844e8039cd2836722c728d7eeb86dc61664e5b0a`).

### 2c. Common review requirements for every Kali candidate

For each allowlisted Kali package, before inclusion:

1. Owner-approved allowlist entry with exact package name and version.
2. Narrow source: `Suites: kali-rolling`, `Components` limited to the areas actually needed, `Signed-By` a pinned keyring. Default `Pin-Priority: -1` for origin `Kali`, with only the allowlisted names raised. Exact priorities NOT validated.
3. Keyring provenance: how the image obtains the Kali key (fingerprint `827C…E4C5`) must be decided and reviewed. Not decided.
4. Dependency closure resolved against trixie. Every Kali-origin dependency that would replace a trixie library must be listed and reviewed, because Kali rolling is not trixie.
5. Upgrade evidence on a trixie system: Debian packages are not replaced by Kali versions; allowlisted Kali packages upgrade only within their pin.
6. Licence review per package (not performed for any candidate).
7. Independent Codex review of the complete set (required by the task before any GATED package enters `cham3-full`).

## 3. Open owner and architecture decisions

1. **LibreWolf:** approve or reject the third-party `repo.librewolf.net` source (section 2a).
2. **Tor Browser:** accept torbrowser-launcher's self-update path for M1, or require a different design.
3. **Clearnet browser fallback:** if LibreWolf is gated, decide whether Debian `firefox-esr` may fill the clearnet role (currently excluded by the task).
4. **Burp vs ZAP:** choose one, or neither. Burp requires a licence review first.
5. **Gated tool scope:** confirm that M1 ships without the Kali-origin tools (the default under the sourcing rule) or approve a specific reviewed subset.
6. **Wireshark group name:** use Debian's `wireshark` group (recommended) rather than a new group. Confirm the image's `install-setuid` answer (non-setuid).
7. **Name substitutions:** accept `plasma-workspace`, `kde-spectacle`, `qml-module-org-kde-qqc2desktopstyle`, `bind9-dnsutils`, and `libimage-exiftool-perl` in the contract.
8. **Zenmap:** confirm the Nmap GUI is acceptable on the KDE desktop, or name a KDE-native alternative (none researched).
9. **Radare2 vs Cutter:** the task allows either; owner picks one.
10. **Kali key provenance:** decide how the image obtains and verifies the Kali archive key.
11. **Trixie archive key:** confirm the image build pins the trixie automatic key `4CB5…E131` (signature verification NOT_RUN in this research).

## 4. Wireshark / dumpcap capture privilege (summary)

- Debian group: **`wireshark`** (system group, created by `wireshark-common` postinst).
- `dumpcap` is `root:wireshark` with file capabilities `cap_net_raw,cap_net_admin=eip` (`setcap`). Setuid is only a fallback if `setcap` fails and the `wireshark-common/install-setuid` answer permits it.
- Adding a user: `sudo usermod -a -G wireshark <username>` (manual, per Debian README). Member capture authority is `cap_net_raw` and `cap_net_admin`, not a root shell.
- Set the setuid question to the non-setuid answer in the image. The default was NOT read (template fetch returned 404).
- Disposable-VM check (NOT_RUN): confirm `getcap` output, group ownership, and a non-root group member capturing on an interface.

## 5. What this packet does NOT claim

- No package is installed, built, executed, or verified to run.
- No Kali package is approved. No third-party repository is approved.
- No dependency closure is complete for any candidate.
- Trixie and Kali signatures on the indices were checked only as described in the matrix section 1 (Kali PASS; trixie NOT_RUN).
- Package availability is not evidence of security, correctness, or M2–M4 enforcement.
