# M1 package disposition matrix (revision 2)

- **Status:** RESEARCH. Availability facts, authenticated-source designs, and EARLY-SIGNAL simulated closure results. Nothing in this document is installed, built, booted, or approved. No foreign source is enabled in any build: `build/**`, `packaging/**`, `desktop/**`, and `tests/**` are not modified here.
- **Task:** M1 built-in application suite, `docs/tasks/M1-bootable-desktop-distribution.md` (owner decision 2026-10-11: Debian 13 first; Debian-native equivalent second; a Kali-origin package only where no suitable Debian option exists and only through a narrowly pinned, separately reviewed source; no general Kali repository).
- **Owner:** `m1_docs_research`. Companion documents: `M1_readiness_packet.md` (gates, decisions) and `M1_acceptance_matrix_corrections.md` (acceptance wording). Evidence index: section 9.
- **Run date:** 2026-10-11. Index snapshots are in section 1.
- **Environment limit (read first):** every closure simulation ran in `debian:trixie` amd64 containers under Docker Desktop on an arm64 macOS host, so x86-64 code ran under emulation with apt 3.0.3. These are simulations, not native x86-64 evidence and not full-system verification. The authoritative closure evidence is the native Codex Remote gate in `M1_readiness_packet.md` section 6. Where a native result differs from the early signal here, the native result governs.

## 0. Revision 2 corrections (what changed and why)

| # | Earlier statement | Correction and evidence |
|---|---|---|
| C1 | Kali dependency closures "not computed" | Computed for the 11 Kali candidates (section 7). CLEAN: zaproxy, exploitdb, ghidra, burpsuite (technical only; licence-blocked). CLEAN_WITH_KALI_ONLY_DEPS: amass, radare2. LEAKY (base-library skew): mitmproxy, wpscan, rizin, rizin-cutter, metasploit-framework. |
| C2 | Pin design: `Pin-Priority: -1`, or a "low" priority below 100 | Only `-1` is sufficient. With Pin-Priority 50 and only the candidate allowlisted, apt installed unlisted Kali packages to satisfy dependencies: `libpostal-data` (amass), `libradare2-6.0.0t64` and `libradare2-common` (radare2). With `-1` the same dependencies were blocked. Section 3.3. |
| C3 | Trixie `Release` "signed by `4CB50190207B4758A3F73A796ED0E7B82643E131` (NOT_RUN)" | `Release` is GOODSIG under the trixie automatic key shipped in `debian-archive-keyring`: primary `04B54C3CDCA79751B16BC6B5225629DF75B188BD` (uid "Debian Archive Automatic Signing Key (13/trixie) <ftpmaster@debian.org>"). `Release.gpg` also carries a signature by `4CB50190207B4758A3F73A796ED0E7B82643E131`, whose key is NOT in the shipped keyring (ERRSIG/NO_PUBKEY). That second key was not verified. Evidence: `evidence/f2_keys/trixie_release_signature_check.txt`. |
| C4 | LibreWolf: "the LibreWolf page does not publish the fingerprint" | `https://repo.librewolf.net/` states the fingerprint `662E3CDD6FE329002D0CA5BB40339DD82B12EF16`. Its `pubkey.gpg` has SHA-256 `cd1c45eb...`, identical to extrepo's `gpg-key-checksum`. `Release` and `InRelease` verify GOODSIG under that primary key. Section 3.1. |
| C5 | LibreWolf pin `Pin: origin repo.librewolf.net` | The repository's Origin header is `LibreWolf Software Repository`. The tested pin is `Pin: release n=librewolf`. Section 3.1. |
| C6 | Wireshark: "set `wireshark-common/install-setuid` to the non-setuid answer" | **Wrong.** `false` gives root-only capture (`root:root`, mode `u=rwx,go=rx`, no group, no capabilities). `true` gives the `wireshark` group with file capabilities and a setuid fallback when `setcap` fails. Keep `true` and close the fallback (Codex F3). Section 5. |
| C7 | `tor` 0.4.9.11-0+deb13u1; `wireshark-common` 4.4.18 | Current candidates are newer than the 2026-09-12 pool files: `tor` 0.4.9.14-0+deb13u1 (origin trixie-security, observed); `wireshark-common` 4.4.19-0+deb13u1 (origin not recorded in this pass). Re-record every version and origin at the target commit. |
| C8 | Ghidra "JDK dependency, not computed" | Computed. Kali `ghidra` 12.1.4 requires `openjdk-25-jdk`, which trixie-security provides (`25.0.4.1+1-1~deb13u1`, origin Debian-Security). Upstream README requires JDK 21, which trixie also provides. |
| C9 | mitmproxy and radare2 "no trixie version" | Confirmed by apt. `apt-cache policy` shows no candidate. Both names appear only as reverse references: `mitmproxy` from `python3-tornado` (a versioned relation) and `radare2` from `diffoscope` and `forensics-all`. The earlier tracker and testing-migration claims were not re-verified in this revision. |
| C10 | Firmware not covered | Added section 6. Two contrib installers are scheduled into the image by live-build and fetch from GitHub and OpenWrt during installation. This is a build blocker in Codex's M1 image attempt. |
| C11 | Acceptance A8 "PASS" for the curated suite | No document now marks the full suite as satisfied. See `M1_acceptance_matrix_corrections.md`. |
| C12 | Kali key verified against `kali-rolling/Release` only | Kali's HTTPS key was also cross-checked against the Kali-signed `kali-archive-keyring` 2025.2 package. That package also carries a legacy key `44C6513A8E4FB3D30875F758ED444FF07D8D0BF6` (2012-03-05, expires 2027-02-04). The narrow keyring must exclude it. Section 3.3. |
| C13 | `exiftool` | `libimage-exiftool-perl` provides the virtual name `exiftool` (reverse-provides evidence). The installed binary path is still NOT_RUN. |

## 1. Evidence method, snapshots, and key checks

### 1.1 Snapshots

Authority for presence and versions is the signed apt index, read in disposable containers with approved suites only. Web pages were used for upstream and provenance facts only.

| Short name | Source | Snapshot | Notes |
|---|---|---|---|
| `IDX_TRIXIE` | `deb.debian.org` `dists/trixie` (`main contrib non-free non-free-firmware`) | `Date: Sat, 12 Sep 2026 07:55:41 UTC`; `Origin: Debian`; `Suite: stable`; `Codename: trixie` | Presence and pool versions. |
| `IDX_TRIXIE_SECURITY` | `deb.debian.org/debian-security` `trixie-security`; `trixie-updates` | Candidates observed 2026-10-11 | Newer candidates used where present (tor, wireshark-common, openjdk-25, default-jre). |
| `IDX_KALI` | `http://http.kali.org/kali/` `kali-rolling` | `Date: Fri, 09 Oct 2026 12:06:20 UTC`; `Origin: Kali`; `Suite: kali-rolling`; components `main contrib non-free non-free-firmware` | `http.kali.org` answers HTTP 302 to `kali.download` from the host; apt in the container used a CDN mirror. Authenticity comes from the signatures, not from the mirror. |
| `IDX_LIBREWOLF` | `https://repo.librewolf.net` `librewolf` | `Date: Thu, 08 Oct 2026 03:30:38 +0000`; `Valid-Until: Wed, 04 Jul 2029 03:30:38 +0000`; `Origin: LibreWolf Software Repository`; `Components: main`; `Architectures: amd64 arm64` | Index holds `librewolf` 157.0.1-1 and 157.0-1 only. |
| `IDX_EXTREPO` | `extrepo-offline-data` 1.0.6~deb13u1, `usr/share/extrepo/offline-data/debian/trixie/index.yaml` | Downloaded from `deb.debian.org/debian/pool/main/e/extrepo-data/` | Source of the LibreWolf key checksum. Evidence: `evidence/f2_librewolf/extrepo_librewolf_entry.txt`. |

Web-page caveat (observed this session): `packages.debian.org/trixie/exploitdb` returns HTTP 200 with the body "No such package." HTTP status codes are not evidence of presence.

### 1.2 Key and signature checks

| Item | Check performed | Result | Evidence |
|---|---|---|---|
| Trixie `Release` | `gpg --verify Release.gpg Release` with the shipped `debian-archive-trixie-automatic.gpg` | GOODSIG; VALIDSIG primary `04B54C3CDCA79751B16BC6B5225629DF75B188BD`. Second signature `4CB50190207B4758A3F73A796ED0E7B82643E131`: NO_PUBKEY (not verified). | `evidence/f2_keys/trixie_release_signature_check.txt` |
| Kali archive key | `curl` over HTTPS with certificate verification (`https://archive.kali.org/archive-key.asc`); `sha256sum`; `gpg --show-keys` | SHA-256 `bbaef4b375fdae6d361d648355c9a003b081d421a40f39440f296cbd458071b1`. Fingerprint `827C 8569 F251 8CC6 77FE CA1A ED65 462E C8D5 E4C5`. RSA 4096, created 2025-04-17, expires 2028-04-17. UID `Kali Linux Archive Automatic Signing Key (2025) <devel@kali.org>`. | `evidence/f2_keys/kali_archive-key.asc`, `.sha256`, `kali_archive-key.show.txt` |
| Kali repository signature | `apt-get update` with `Signed-By` pointing to a keyring containing only the 2025 key | No NO_PUBKEY, GPG, or signature errors. | `evidence/f2_keys/kali_apt_update.log` |
| Kali keyring package | `kali-archive-keyring_2025.2_all.deb` downloaded from the signed Kali index and read with `dpkg-deb -x` (not installed) | Contains the same 2025 key plus legacy `44C6513A8E4FB3D30875F758ED444FF07D8D0BF6` (created 2012-03-05, expires 2027-02-04). The narrow keyring excludes the legacy key. | `evidence/f2_keys/kali_archive_keyring_2025.2_contents.txt` |
| LibreWolf key bytes | extrepo `librewolf.asc` SHA-256 equals extrepo `gpg-key-checksum` (`cd1c45ebf33b42191af369b5ea901508eb7dff3dd5086abcb601b25121bc70ad`); `repo.librewolf.net/pubkey.gpg` has the same SHA-256 | Same bytes at both locations. | `evidence/f2_keys/librewolf_extrepo_key.asc`; `evidence/f2_librewolf/repo_landing_and_hashes.txt` |
| LibreWolf fingerprint | Repository landing page text: "import the GPG key with the fingerprint 662E3CDD6FE329002D0CA5BB40339DD82B12EF16" | Primary `662E 3CDD 6FE3 2900 2D0C A5BB 4033 9DD8 2B12 EF16`; UID `LibreWolf Maintainers <gpg@librewolf.net>`; created 2022-05-14; signing subkey `4DD4 BBEC ABD3 2E79 8CC7 7A40 4798 1EA7 3D0E 1C63`. | `evidence/f2_librewolf/repo_landing_and_hashes.txt` |
| LibreWolf repository signature | Host `gpg --verify` of `Release.gpg` and `InRelease` with a scratch keyring holding only the extrepo key; `apt-get update` in a container with `Signed-By` | GOODSIG on both (signed 2026-10-08) under primary `662E3CDD6FE329002D0CA5BB40339DD82B12EF16`. apt update: no NO_PUBKEY or GPG errors. | `evidence/f2_librewolf/release_gpg_verify_host.txt`; `evidence/f2_librewolf/apt_update_librewolf.log` |
| Tor Browser developers key | Tor Project page `support.torproject.org/tbb/how-to-verify-signature/` | `EF6E286DDA85EA2A4BA7DE684E2C6E8793298290` (`torbrowser@torproject.org`), also published via WKD, openpgpkey.torproject.org, and keys.openpgp.org. | `evidence/f2_keys/tor_project_verify_excerpt.txt` |
| torbrowser-launcher bundled key | Read from the Debian package `torbrowser-launcher` 0.3.7-3 (`usr/share/torbrowser-launcher/tor-browser-developers.asc`) | Primary `EF6E286DDA85EA2A4BA7DE684E2C6E8793298290`; the file also lists two subkey fingerprints. | `evidence/f2_keys/torbrowser_launcher_bundled_tor_developers.asc` |
| torbrowser-launcher package | `apt-cache show` and host download (`deb.debian.org/debian/pool/contrib/t/torbrowser-launcher/`) | SHA-256 `a70a9da89b4ea3adf4542de27595ab3d8e6914c5efd66ecbded627157f3b5274`, 54,872 bytes. Matches the index. Maintainer scripts contain no network access (AppArmor helpers only). | `evidence/f2_keys/torbrowser_launcher_postinst.txt` |

### 1.3 Limits of this revision

- No package was installed or executed. Packages were downloaded only for static reading: file lists, maintainer scripts, and control fields.
- Closure is an `apt-get -s` simulation (apt 3.0.3, `--no-install-recommends`, matching the live-build `--apt-recommends false`).
- The origin tests used a simulated installed state: one dpkg status stanza was appended inside a disposable container and then removed. No files were installed.
- Closure numbers depend on the index snapshot. The native gate must re-run them on the target date.

## 2. Matrix

Columns: requested item (task wording) | Debian 13 package and availability | selected, equivalent, or gap | source, authentication, licence, update | closure and privilege notes | unresolved decisions.

Status vocabulary: **INCLUDE-NOW** (Debian package, candidate for the default `cham3-full` set after normal review); **GAP-DEBIAN** (no installable Debian 13 package; the Kali or upstream candidate is listed); **GATED-KALI** / **GATED-THIRDPARTY** (not included; needs owner approval and the gate); **EXCLUDE** (redundant; must be absent). Closure classes are defined in section 7.

### 2a Privacy browsers

| Requested item | Debian 13 package | Selected / equivalent / gap | Source, authentication, licence, update | Closure and privilege | Unresolved |
|---|---|---|---|---|---|
| LibreWolf (clearnet) | None. `librewolf` is absent from `IDX_TRIXIE` and from `IDX_KALI`. | **GATED-THIRDPARTY.** Designed in section 3.1. | Official repository `https://repo.librewolf.net`, suite `librewolf`, component `main`, amd64. Signed `Release` verified; key provenance in section 1.2. Licence not checked. Update path: the LibreWolf repository, outside the Debian archive. | Closure **CLEAN** (emulated): `librewolf` 157.0.1-1 plus 92 Debian-origin dependencies. No base-library replacement in measure mode. Installed size 468,522 KiB; `.deb` 151,840,488 bytes. | Owner approval of the third-party source. Licence check. Whether the key ships in packaging (recommended). |
| Official Tor Browser (Tor browsing) | `torbrowser-launcher` 0.3.7-3, contrib, amd64. The Tor Browser bundle is not a Debian package. | **INCLUDE-NOW** (contrib). | Debian archive (signed index). The launcher downloads the official bundle from `dist.torproject.org` or a listed mirror and verifies its OpenPGP signature with the Tor Browser developers key bundled in the package. Mirrors listed in `mirrors.txt` include third-party hosts, so the signature, not the mirror identity, is the trust basis. The browser then self-updates through the launcher, outside apt. Maintainer scripts contain no network access. | Depends: `python3-gpg`, `python3-packaging`, `python3-pyqt5`, `python3-requests`, `python3-socks`, `python3`, `ca-certificates`, `gnupg`, `libasound2`, `libdbus-glib-1-2`, `libgtk-3-0`. Recommends `tor` (candidate 0.4.9.14-0+deb13u1, trixie-security). Per-user install under `~/.local/share/torbrowser`; no root after install. The bundle is fetched at first use, which needs runtime network. | Owner accepts the self-update path and the first-use download. Whether `tor` is installed by default. `contrib` must be enabled. |
| Firefox ESR | `firefox-esr` present in trixie main. | **EXCLUDE** as the stock or clearnet browser, per the task. | Debian archive. | Would conflict with the stock-browser role. | Owner decision only if LibreWolf is refused. |

### 2b Traffic analysis

| Requested item | Debian 13 package | Selected / equivalent / gap | Source, authentication, licence, update | Closure and privilege | Unresolved |
|---|---|---|---|---|---|
| Wireshark | `wireshark-common` candidate 4.4.19-0+deb13u1 (newer than the pool file 4.4.18 in `IDX_TRIXIE`; origin to be recorded at the target). `wireshark`, `tshark` in trixie main. | **INCLUDE-NOW.** Capture mechanism corrected in section 5. | Debian archive. | `wireshark-common` hard-depends on `libcap2-bin`. Capture needs `cap_net_raw` and `cap_net_admin` through the `wireshark` group (section 5). | Fail-closed setuid fallback (Codex F3). Live-system capability retention (section 5). |
| tshark | `tshark` (trixie main). | **INCLUDE-NOW.** | Debian archive. | Same capture mechanism as Wireshark. | None beyond section 5. |
| mitmproxy / mitmweb | None installable. `apt-cache policy mitmproxy`: no candidate. Reverse reference only: `python3-tornado` (versioned relation). | **GAP-DEBIAN.** Kali candidate LEAKY (below). | Kali `mitmproxy` 12.2.3-0kali5 (`all`), `pool/main/m/mitmproxy/mitmproxy_12.2.3-0kali5_all.deb`, SHA-256 `0d2d281a0cc9353a612efb2295124226bf6060f1802976b0e4fe5c45dbc6a26d`. Upstream candidate (not validated): PyPI `mitmproxy` 12.2.3, wheel `py3-none-any`, `requires_python >= 3.12` (trixie provides 3.13), SHA-256 `df75ccd15ccb39ab...`, uploaded 2026-05-12. | **LEAKY** (section 7): the Kali build needs Kali-only Python 3.14 packages (`python3-mitmproxy-rs` 0.12.11-0kali1 and the `libpython3.14` stack) and `libc6 >= 2.42`, while trixie has libc6 2.41-12+deb13u4. | Owner decision: no compliant Kali path. The PyPI route needs an owner-approved third-party source and a hash-locked dependency set (not computed). |
| OWASP ZAP (zaproxy) | None. No record of `zaproxy` in trixie. | **GAP-DEBIAN.** Kali candidate CLEAN (below). Burp is the alternative (next row). | Kali `zaproxy` 2.17.0-0kali1 (`all`), `pool/main/z/zaproxy/zaproxy_2.17.0-0kali1_all.deb`, SHA-256 `9490cc3c613ca847db8f12c5c6b00d162e17274e4fcfed566bb93d1aa1feda47`. Upstream: GitHub release v2.17.0 (nine assets, GitHub-computed SHA-256 digests, no detached signatures among them). | **CLEAN** (emulated). Kali `zaproxy` only. Its `default-jre` dependency resolves to Debian `openjdk-21`. | Owner choice ZAP or Burp. Upstream signature evidence is absent (GitHub digests are not publisher signatures). |
| Burp Suite Community | None. Debian `burp` 3.1.4-4 is an unrelated backup program; do not treat it as Burp Suite. | **GAP-DEBIAN.** Kali candidate technically CLEAN, licence-blocked. | Kali `burpsuite` 2026.8-0kali1 (`amd64`), SHA-256 `6e68cad47c66d854ab944aa417a54290c18b396e0727583108b790b857638b1f`. PortSwigger terms not checked. | **CLEAN** (emulated, technical). | **Blocking:** PortSwigger licence and redistribution review. Only if chosen instead of ZAP. |
| `tcpdump`, `nethogs`, `iftop` | tcpdump 4.99.5-2; nethogs 0.8.8-1; iftop 1.0~pre4-9+b1 (trixie main). | **INCLUDE-NOW.** | Debian archive. | Packet capture and per-process stats need root or capabilities. None are granted by this research. | Whether these get the same capture-group treatment as Wireshark. Recommended: no. |

### 2c Information gathering

| Requested item | Debian 13 package | Selected / equivalent / gap | Source, authentication, licence, update | Closure and privilege | Unresolved |
|---|---|---|---|---|---|
| `nmap` | nmap 7.95+dfsg-3 (trixie main) | **INCLUDE-NOW.** | Debian archive. | SYN and raw scans need root or capabilities. | None. |
| Zenmap | zenmap 7.95+dfsg-3 (trixie main, source `nmap`) | **INCLUDE-NOW** as the Debian Nmap GUI. | Debian archive. | Same as `nmap`. | Owner confirms a GTK application is acceptable on KDE, or names a KDE-native alternative (none found). |
| `masscan` | masscan 2:1.3.2+ds1-2 (trixie main) | **INCLUDE-NOW.** | Debian archive. | Raw packets; root or capabilities. | None. |
| `amass` | None installable. | **GAP-DEBIAN.** Kali candidate CLEAN_WITH_KALI_ONLY_DEPS. | Kali `amass` 5.1.1-0kali3 (`amd64`), SHA-256 `72d3563addb64ada4527170be7b45a88d7aec29acb367261365a6beecea11999`. Kali-only dependency `libpostal-data` 1.1.4-0kali1 (`all`), SHA-256 `e6ae56364ef9bb1f90410604a3867fd8e9c4fe8c9480a58fddffca3883bd9f63`, installed size 20 KiB. Its only program, `/usr/bin/libpostal_data`, downloads models from GitHub releases and an S3 bucket when a user runs it. The package has no postinst. | **CLEAN_WITH_KALI_ONLY_DEPS** (emulated): amass plus `libpostal-data`. Debian equivalents (partial, not equivalent): `dnsrecon`, `dnsenum`, `recon-ng` (section 4). | Owner decision on the Kali allowlist. Model downloads at runtime sit outside package authentication. |
| `dnsutils` | Virtual name; provided by `bind9-dnsutils` 1:9.20.26-1~deb13u1 (trixie main). | **INCLUDE-NOW** by installing `bind9-dnsutils`. | Debian archive. | BIND client libraries only. | Name the real package in the manifest. |
| `whois` | whois 5.6.3 (trixie main) | **INCLUDE-NOW.** | Debian archive. | None. | None. |

### 2d Web application analysis

| Requested item | Debian 13 package | Selected / equivalent / gap | Source, authentication, licence, update | Closure and privilege | Unresolved |
|---|---|---|---|---|---|
| `sqlmap` | sqlmap 1.9.6-1 (trixie main) | **INCLUDE-NOW.** | Debian archive. | None. | None. |
| `ffuf` | ffuf 2.1.0-1+b9 (trixie main) | **INCLUDE-NOW.** | Debian archive. | None. | None. |
| `gobuster` | gobuster 3.6.0-1+b10 (trixie main) | **INCLUDE-NOW.** | Debian archive. | None. | None. |
| `nikto` | nikto 1:2.1.5-3.1 (trixie **non-free**) | **INCLUDE-NOW** only if `non-free` is enabled. | Debian archive. Licence terms not reviewed here. | Non-free area. | Confirm the licence is acceptable for M1. |
| `wpscan` | None installable. | **GAP-DEBIAN.** Kali candidate LEAKY. | Kali `wpscan` 4.1.0-0kali1 (**non-free**), `pool/non-free/w/wpscan/wpscan_4.1.0-0kali1_all.deb`, SHA-256 `20caf190d6adc4cc2bedc5b8df34038aaacb452853ac5dfede260aafe49583e9`. | **LEAKY** (section 7): needs `ruby-addressable >= 2.9` while trixie has 2.8.7-2, and the Kali-only `ruby-cms-scanner`. Partial Debian-native option: `cmsscanner` (scans CMS installations; not the WPScan vulnerability database). | Owner scope amendment or omission. Non-free licence and the WPScan API terms. |

### 2e Wireless and hardware

| Requested item | Debian 13 package | Selected / equivalent / gap | Source, authentication, licence, update | Closure and privilege | Unresolved |
|---|---|---|---|---|---|
| aircrack-ng suite | aircrack-ng 1:1.7+git20230807.4bf83f1a-2 (trixie main) | **INCLUDE-NOW.** | Debian archive. | Monitor mode and injection need root and drivers. | Hardware support is outside M1 verification scope. |

### 2f Exploitation and auditing

| Requested item | Debian 13 package | Selected / equivalent / gap | Source, authentication, licence, update | Closure and privilege | Unresolved |
|---|---|---|---|---|---|
| metasploit-framework | None installable. | **GAP-DEBIAN.** Kali candidate LEAKY. | Kali `metasploit-framework` 6.5.3-0kali1 (`amd64`), `pool/main/m/metasploit-framework/metasploit-framework_6.5.3-0kali1_amd64.deb`, SHA-256 `fcb1100612b748b5122f3413033279693f80fd42a9e6122c3464f4db748c19b6`. Upstream route: Rapid7's apt host `apt.metasploit.com` returned HTTP 403 (AccessDenied) from this environment, so it is BLOCKED and was not investigated further. | **LEAKY** (section 7). The Kali package depends on `oracle-instantclient-basic` 19.6.0.0.0-0kali5 (Kali **non-free**, a proprietary Oracle client, installed size 210,503 KiB). That needs `libaio1t64 >= 0.3.113-8+kali1`, while trixie has 0.3.113-8+b1. The solver stopped at this first blocker; later blockers were not enumerated. | Owner scope amendment (exploit framework; LEAKY; Oracle client licence and redistribution). Otherwise omit. |
| searchsploit (exploitdb) | None. No record of `exploitdb` in trixie. | **GAP-DEBIAN.** Kali candidate CLEAN. | Kali `exploitdb` 20260904-0kali1 (`all`), `pool/main/e/exploitdb/exploitdb_20260904-0kali1_all.deb`, SHA-256 `b417d86c6e3c126d2fa2c9be2f8d6c10417697fc95ebaf5a729c299d59d3f385`. The file list includes `/usr/bin/searchsploit` and `/usr/bin/exploitdb` (48,047 entries). No postinst; only conffiles, control, md5sums. The data licence was not checked. | **CLEAN** (emulated): one Kali package, no base replacement. | Owner allowlist decision. Data-licence review. |

### 2g Password and cryptographic auditing

| Requested item | Debian 13 package | Selected / equivalent / gap | Source, authentication, licence, update | Closure and privilege | Unresolved |
|---|---|---|---|---|---|
| `hashcat` | hashcat 6.2.6+ds2-1 (trixie main) | **INCLUDE-NOW.** | Debian archive. | Uses GPU drivers if present. | None. |
| `john` | john 1.9.0-2 (trixie main) | **INCLUDE-NOW.** | Debian archive. | None. | None. |
| `hydra` | hydra 9.5-3 (trixie main) | **INCLUDE-NOW.** | Debian archive. | None. | None. |

### 2h Reverse engineering and forensics

| Requested item | Debian 13 package | Selected / equivalent / gap | Source, authentication, licence, update | Closure and privilege | Unresolved |
|---|---|---|---|---|---|
| ghidra | None installable. | **GAP-DEBIAN.** Kali candidate CLEAN (emulated). | Kali `ghidra` 12.1.4+ds-0kali1 (`amd64`), `pool/main/g/ghidra/ghidra_12.1.4+ds-0kali1_amd64.deb`, SHA-256 `128b96ae74b22eb508401378950a6b8eb3274c2e29b50f10d1a59f1fca3caad6`, installed size 843,563 KiB. Recommends the Kali-only `ghidra-data` 10.5-0kali1 (`all`), SHA-256 `8006fa035088846ab9460f69c3ab37b263f392c4d8405ba99648e30f606b6be9`, installed 77,670 KiB. Upstream alternative: NSA GitHub release `Ghidra_12.1.4_build` (published 2026-09-21), asset `ghidra_12.1.4_PUBLIC_20260921.zip`, 569,649,598 bytes, GitHub-computed SHA-256 `ddac49f903da9d5bac833e5cc79395098b9c33cfd3279be5f31bd00387d2d4db`. Its README requires JDK 21. | **CLEAN** (emulated, strict design): 132 new packages, including Debian `openjdk-25-jdk` 25.0.4.1+1-1~deb13u1 (Debian-Security). With `ghidra-data` allowlisted, 133 new packages. The `--no-install-recommends` run does not show whether Ghidra works without `ghidra-data`. | Owner decision on footprint (about 0.9 GB installed, plus a JDK) and source (Kali package or NSA upstream). Functional check: NOT_RUN. |
| radare2 | None installable. `radare2` is referenced only by `diffoscope` and `forensics-all`. The earlier "unstable 6.1.8+ds-1, removed from testing" claim was not re-verified. | **GAP-DEBIAN.** Kali candidate CLEAN_WITH_KALI_ONLY_DEPS. | Kali `radare2` 6.0.4+dfsg-1 (`amd64`), SHA-256 `dcb375213e9c1477ae08d47bfb25c1b13a6ba2d41bc2665b81a4bae66bb64eb0`. Kali-only libraries: `libradare2-6.0.0t64` 6.0.4+dfsg-1 (`amd64`, SHA-256 `081091ad8e76c3b19587262a17c0a833bd1beca509188a5c51c1d79c18adf5ce`) and `libradare2-common` 6.0.4+dfsg-1 (`all`, SHA-256 `31069d2726bdcab6758568064fa7d5e37692f819dabc06b7b23e708aa93ea3af`). | **CLEAN_WITH_KALI_ONLY_DEPS** (emulated): 7 new packages (strict), 0 base replacements in measure mode. | Owner choice radare2 or Cutter. Owner allowlist decision (three names). |
| Cutter | None. | **GAP-DEBIAN.** Kali candidate LEAKY. | Kali `rizin-cutter` 2.5.0-0kali1, SHA-256 `e0e647c191b41bd6b4ffb3bf6b76c8ed5dd381dc00ae179aab94edc7b0440d79`. No package named `cutter` exists in Kali. Upstream AppImage distribution was not re-verified here (NOT_RUN). | **LEAKY** (section 7): needs `libqt6core6t64 >= 6.10.2` (trixie 6.8.2+dfsg-9+deb13u2), `libpython3.14 >= 3.14.1`, PySide6 and Shiboken 6.10, and `librizin0 >= 0.9.1`. The Qt and Python runtime would come from Kali. | Owner choice of radare2 instead (recommended by this revision if the owner prefers a Kali-origin tool). |
| rizin (not requested; Cutter dependency) | None. | **GAP-DEBIAN.** | Kali `rizin` 0.9.1-0kali1, SHA-256 `41a2acc204a9adad6174139ea3b0b50639f67bb0e664bbf6601defdc91002fd1`; Kali-only `librizin0` 0.9.1-0kali1, SHA-256 `b8d28bfc7686fe98ee010abd777b8f95832d1f1da69f2a827442e2bb306c7f38`. | **LEAKY** (section 7): `librizin0` needs `libc6 >= 2.42` (Kali 2.43-6 replaces trixie 2.41-12+deb13u4). | Only with a Kali base replacement, which is refused. |
| binwalk | binwalk 2.4.3+dfsg1-2+deb13u1 (trixie main) | **INCLUDE-NOW.** | Debian archive. | Some optional extractors pull in external tools. Not reviewed. | Optional extractor closure. |
| exiftool | Virtual name provided by `libimage-exiftool-perl` 13.25+dfsg-1 (trixie main, `all`). | **INCLUDE-NOW** as `libimage-exiftool-perl`. | Debian archive. | Perl. | Binary path in the built image: NOT_RUN. |

### 2i KDE core utilities (keep) and redundant alternatives (exclude)

The KDE set is unchanged from revision 1 (index snapshot values; re-record at the target commit):

- Keep: `plasma-desktop` 4:6.3.6-1; `plasma-workspace` 4:6.3.6-2 (provides the Wayland session `plasma.desktop`, replacing the contract name `plasma-workspace-wayland`); `kwin-wayland` 4:6.3.6-1; `sddm` 0.21.0+git20250502.4fe234b-2; `systemsettings` 4:6.3.4-2; `dolphin` 4:25.04.3-1+deb13u1; `konsole` 4:25.04.2-1; `kate` 4:25.04.3-2 (`kwrite` optional); `ark` 4:25.04.3-1; `okular` 4:25.04.2-1+deb13u1; `gwenview` 4:25.04.3-1; `kde-spectacle` 4:6.3.5-2 (replacing `spectacle`); `plasma-systemmonitor` 6.3.6-1; `fonts-noto` 20201225-2 (Recommends); `qml-module-org-kde-qqc2desktopstyle` 5.116.1-1 (replacing `qqc2-desktop-style`).
- Exclude (absent from the image closure): `thunar`, `pcmanfm`, `qterminal`, `xfce4-terminal`, `tilix`, `mousepad`, `leafpad` (not in trixie), `xarchiver`, `file-roller`, `evince`, `atril`, `ristretto`, and `firefox-esr` as the stock browser. Each is present in trixie main except `leafpad`.

## 3. Foreign-role designs (authenticated source, key provenance, narrow pin)

Nothing in this section is enabled. Each design is an input to the owner decision and the native gate.

### 3.1 LibreWolf: official repository

Upstream instruction (`https://librewolf.net/installation/debian/`, fetched 2026-10-11) uses `extrepo enable librewolf && extrepo update librewolf`, then `apt install librewolf`. **cham3 does not use extrepo at build or install time.** It uses the same repository with a key that is checked at build time.

Authentication chain, in order:

1. Key bytes: extrepo's shipped `librewolf.asc` and `repo.librewolf.net/pubkey.gpg` have the same SHA-256 (`cd1c45ebf33b42191af369b5ea901508eb7dff3dd5086abcb601b25121bc70ad`), which extrepo records as `gpg-key-checksum`.
2. Fingerprint: `662E3CDD6FE329002D0CA5BB40339DD82B12EF16`, printed on the repository landing page.
3. Signed index: `Release` and `InRelease` verify GOODSIG under the primary key via signing subkey `4DD4BBECABD32E798CC77A4047981EA73D0E1C63`.

Limitation: items 1 and 2 are both served from `repo.librewolf.net`, except that the extrepo copy is carried in a Debian-archive-signed package. No second LibreWolf channel was found, so an independent confirmation remains NOT_RUN.

Proposed configuration (design only):

```
# cham3_librewolf.sources (proposed; not enabled)
Types: deb
URIs: https://repo.librewolf.net
Suites: librewolf
Components: main
Architectures: amd64
Signed-By: /usr/share/keyrings/cham3_librewolf.gpg
```

```
# cham3_librewolf.pref (proposed; not enabled)
Package: *
Pin: release n=librewolf
Pin-Priority: -1

Package: librewolf
Pin: release n=librewolf
Pin-Priority: 500
```

Keyring provenance (proposed): ship `cham3_librewolf.gpg`, dearmored from the SHA-256-checked key file, in the packaging. The build asserts both the SHA-256 and the primary fingerprint `662E3CDD6FE329002D0CA5BB40339DD82B12EF16`. Do not run `extrepo` at build or install time.

Tested in simulation: `apt-get update` with this keyring; closure of `librewolf` under strict, measure, and low-priority pins (all exit 0). Every other package in the closure has Debian origin. `librewolf` is the only LibreWolf-origin package. Update behaviour (rollback, cadence) is NOT_RUN.

Decision state: GATED-THIRDPARTY. Owner approval of this source is required, then Codex review, then the native gate.

### 3.2 Official Tor Browser: torbrowser-launcher

No new source is needed. The Debian package `torbrowser-launcher` 0.3.7-3 is included from the Debian archive (section 2a). The official Tor Browser bundle is downloaded at first use. The launcher verifies its OpenPGP signature with the bundled key `EF6E286DDA85EA2A4BA7DE684E2C6E8793298290`, which matches the Tor Project's published fingerprint. The launcher's mirror list includes third-party hosts, so the signature check is the trust basis. Updates are performed by the launcher, outside apt. The owner must accept this path. An apt-managed Tor Browser path does not exist in trixie.

### 3.3 Kali-origin candidates: source, key, pin, and origin preservation

Proposed source (only if the owner approves a per-package allowlist; not enabled):

```
# cham3_kali.sources (proposed; not enabled)
Types: deb
URIs: http://http.kali.org/kali/
Suites: kali-rolling
Components: main
Signed-By: /usr/share/keyrings/cham3_kali_archive.gpg
```

Use `Components: main` for the CLEAN candidates. Add `non-free` only if an allowlisted candidate needs it. The LEAKY non-free candidates (wpscan, oracle-instantclient-basic) do not qualify today. The URI is the one Kali documents (`kali-apt-sources`). The name redirects to a CDN mirror, so authenticity comes from the signatures, not from the mirror.

Key provenance:

- Armored key from `https://archive.kali.org/archive-key.asc`, SHA-256 `bbaef4b375fdae6d361d648355c9a003b081d421a40f39440f296cbd458071b1`.
- Fingerprint `827C8569F2518CC677FECA1AED65462EC8D5E4C5`: the 2025 Kali archive automatic signing key, expires 2028-04-17.
- Cross-check: the identical key is in Kali's signed `kali-archive-keyring` 2025.2 package. That package also carries the legacy key `44C6513A8E4FB3D30875F758ED444FF07D8D0BF6`, which the narrow keyring must exclude.
- Keyring: export only `827C…E4C5` as `/usr/share/keyrings/cham3_kali_archive.gpg`. The build asserts the SHA-256 and primary fingerprint. Do not use `trusted=yes` or `apt-key`.

Pin design (validated in simulation):

```
# cham3_kali.pref (proposed; not enabled)
Package: *
Pin: release o=Kali
Pin-Priority: -1

Package: <allowlisted-name>        # one stanza per owner-approved package
Pin: release o=Kali
Pin-Priority: 500
```

Evidence for the `-1` default:

- With `-1`, Kali-only dependencies were not installable (for example `python3-mitmproxy-rs`, `libcgraph8`, `ruby-cms-scanner`), so the solver reported them as blockers.
- A Kali base library was never selected. The strict policy output lists Kali `libc6` 2.43-6 at priority `-1` and Debian 2.41-12+deb13u4 at 500 (`evidence/f2_origin/A_strict_no_allow.dist_upgrade.log`).
- With Pin-Priority 50 and only the candidate allowlisted, apt installed unlisted Kali packages to satisfy dependencies (`libpostal-data` for amass; `libradare2-6.0.0t64` and `libradare2-common` for radare2). Pins below 100 therefore do not contain the closure. Use `-1`.

Upgrade and origin preservation (simulation):

- Strict design, no allowlist (`apt-get -s dist-upgrade` on a trixie base): 1 upgrade, `liblzma5` from Debian-Security, and no Kali-origin line (`evidence/f2_origin/A_strict_no_allow.dist_upgrade.log`).
- Naive negative control (Kali at 500, no allowlist): 117 upgrades and 36 new packages from Kali, 153 Kali-origin lines, including `libc6` 2.41 to 2.43, `libgcc-s1` and `libstdc++6` 14.2 to 16.2, and `curl` 8.14 to 8.22 (`evidence/f2_origin/B_naive.dist_upgrade.log`).
- Strict design with a simulated installed `exploitdb` (20250101-0kali1): the only Kali line upgrades that allowlisted package to 20260904-0kali1. Debian packages update from Debian (`evidence/f2_origin/C_strict_allowlisted_upgrade.dist_upgrade.log`). The naive design gives 118 Kali upgrades for the same state (`evidence/f2_origin/D_naive_allowlisted_upgrade.dist_upgrade.log`).

Silent origin-change detection (design; to be implemented by the coordinator or m1_worker):

1. At build time, run `apt-get -s install --no-install-recommends` on the manifest. Fail if any `Inst` line has origin `kali-rolling` and the name is not allowlisted, or if an allowlisted name also exists in trixie.
2. On every scheduled update check, run `apt-get -s dist-upgrade` with the same rule. A Debian package must never move to Kali. Allowlisted packages may follow Kali within their pin.
3. Record the origin (`apt-cache policy`) of the base set in the build manifest.

Closure classes and per-tool results are in section 7. The allowlist that would be required per tool is listed there.

### 3.4 Non-Kali upstream alternatives (honest notes; each needs its own owner approval)

- **Ghidra:** NSA GitHub release `Ghidra_12.1.4_build`. The GitHub API digest is a SHA-256 computed by GitHub, not a publisher signature. The upstream README requires JDK 21 64-bit, which trixie provides as `openjdk-21`. The Kali package instead requires `openjdk-25-jdk`, which trixie-security also provides.
- **OWASP ZAP:** GitHub release v2.17.0, nine assets (Linux tarball, installers, zips, dmg, exe). No `.asc` or `.sig` assets were present. Requires Java 17 or later.
- **mitmproxy:** PyPI `mitmproxy` 12.2.3 (`py3-none-any`, SHA-256 `df75ccd15ccb39ab...`, `requires_python >= 3.12`). A usable install needs a hash-locked dependency set (`pip --require-hashes`), which has not been computed.
- **metasploit-framework:** Rapid7 `apt.metasploit.com` returned HTTP 403 from this environment. BLOCKED; not investigated further.
- **Cutter, radare2 upstream releases, and the exploit-db upstream repository:** not re-verified in this revision (NOT_RUN).

## 4. Debian-native equivalent search (trixie index, names and descriptions)

Search run 2026-10-11 against approved suites (`evidence/f2_equivalents/trixie_equivalent_search.txt`):

| Requested role | Result | Verdict |
|---|---|---|
| Intercepting web proxy (mitmproxy, ZAP, Burp) | No intercepting-proxy package found. Matches were unrelated (`interception-tools`, `webext-foxyproxy`). | No Debian-native equivalent. |
| amass (attack-surface and DNS enumeration) | `dnsrecon`, `dnsenum`, `recon-ng` | Partial functional overlap only. Owner decision needed. |
| wpscan (WordPress scanning) | `cmsscanner` (scans CMS installations). `wordpress` is the application itself. | Partial overlap only. Not the WPScan vulnerability database. |
| Metasploit (exploitation framework) | No package found. | No Debian-native equivalent. |
| searchsploit (exploit-db) | No package found. | No Debian-native equivalent. |
| Ghidra (decompiler) | `procyon-decompiler` (Java only). | Not an equivalent. |
| radare2, rizin, Cutter | No installable package (names exist only as references). | No Debian-native equivalent. |

## 5. Wireshark and dumpcap capture privilege (corrected)

Mechanism, from the `wireshark-common` postinst that Codex retained (`evidence/wireshark/wireshark_common_4.4.19_postinst.txt`, Debian 4.4.19):

- `install-setuid = false`: `chown root:root`, `chmod u=rwx,go=rx`. Root-only capture. No group and no capabilities.
- `install-setuid = true`: create the system group `wireshark`, `chown root:wireshark`, `chmod u=rwx,g=rx,o=r`, then `setcap cap_net_raw,cap_net_admin=eip`. If `setcap` fails, the postinst shows the `setcap-failed` note and applies `chmod u=rwxs,g=rx,o=r`, which is setuid root, restricted to the group. If `setcap` is absent, it applies the setuid mode directly.

Dependency check: `libcap2-bin` is a hard `Depends` of `wireshark-common` (4.4.19 and 4.4.18), so `setcap` is present in a normal install. The setuid fallback is reached only when `setcap` fails, for example on a filesystem without security xattrs.

Consequences for M1:

- The image preseed `build/live-desktop/config/preseed/cham3_wireshark.cfg.chroot` sets `wireshark-common/install-setuid boolean true`. That is the intended group and capability mechanism. Keep it.
- The earlier advice to set the non-setuid answer is withdrawn. A `false` answer would remove the non-root capture mechanism entirely.
- The defect Codex F3 describes is the setuid fallback. It needs a fail-closed fix and a test that forces `setcap` to fail (section 6 of the readiness packet, gate F3-CAP).
- Open (NOT_RUN): whether `security.capability` xattrs survive live-build's squashfs. If they do not, the live system loses `dumpcap` capabilities even though the installed system keeps them. Verify on the native ISO.
- Membership: no user is added automatically (per the Debian README). Membership is a capture authority (`cap_net_raw`, `cap_net_admin`), not a benign convenience.

## 6. Firmware: the two fetch-at-install installers (new in revision 2)

### 6.1 What the image contains and why (verified)

- live-build 1:20250505+deb13u1 `chroot_firmware` runs when `LB_FIRMWARE_CHROOT=true`, which the M1 config sets with `--firmware-chroot true`. It calls `Firmware_List_From_Contents` (`/usr/share/live/build/functions/firmwarelists.sh`). That function selects every package whose `Contents-<arch>` index lists a path beginning `lib/firmware` or `usr/lib/firmware`, across every configured archive area. It also adds `firmware-linux` by name. Its only blocklist entries are `firmware-nvidia-gsp` and `firmware-nvidia-tesla-gsp`, plus ARM-only and x86-only packages according to the build architecture (evidence: `evidence/firmware/livebuild_source/`).
- `firmware-b43-installer` and `firmware-b43legacy-installer` (contrib, 1:019-14) are selected because their contents are `usr/lib/firmware/b43/.placeholder` and `usr/lib/firmware/b43legacy/.placeholder`. Nothing else in them is firmware.
- Reproduced selection from the same Contents indices for `main`, `contrib`, `non-free`, and `non-free-firmware`: 47 packages before the ARM blocklist. Adding `firmware-linux` gives 48. On amd64 the ARM blocklist removes `firmware-qcom-soc`, `firmware-samsung`, `firmware-ti-connectivity`, and `raspi-firmware`, leaving 44. The Codex image log is consistent with this set: `firmware-linux`, both installers, and the astronomy packages appear as configured, and no ARM package appears. Only spot checks were made; the full comparison is NOT_RUN. Without the two installers the set is 42 (`evidence/firmware/proposed_set_without_b43_installers_42.txt`).

### 6.2 What the two installers do at install time (verified from the packages)

- Both postinsts fetch a fixed URL and verify a SHA-512 embedded in the script:
  - `firmware-b43-installer`: `https://github.com/minios-linux/b43-firmware/releases/download/b43-firmware/broadcom-wl-6.30.163.46.tar.bz2`; SHA-512 `0144894fbbb5e8ebab6c423d9bd0f3249be94f2f468a50b8bf721a3b17f1f6e57467c79e87abc8d136bfc92e701ed046885fead892e9a73efa5217d710311ae9`.
  - `firmware-b43legacy-installer`: `https://downloads.openwrt.org/sources/wl_apsta-3.130.20.0.o`; SHA-512 `d89ed52045307449bbae79a4d1807cc6cd89ae67c4a22e8e8aa51c1396edbb6ed8b157cd0756faf8b660a537b48b62117c57967f2048245b5b102d9d9bca4bbd`.
- Both postinsts detect a chroot (`stat` of `/` against `/proc/1/root`). In a chroot they set `unconditional=1`, which forces a download whatever the hardware and whatever the debconf answer. Outside a chroot, the download happens only when a matching Broadcom card is detected, or when `b43-fwcutter/install-unconditional` is true.
- Observed in Codex's build (`evidence/firmware/codex_b82919c_image_attempt_b43_failure_excerpt.txt`): `Resolving github.com ... failed: Temporary failure in name resolution`, then `downloads.openwrt.org` failed the same way, then `dpkg: error processing package firmware-b43-installer` and `firmware-b43legacy-installer`, then `lb build` exit 123.
- A static audit of the maintainer scripts of all 47 pre-blocklist packages (non-comment lines) found network commands only in these two packages. The microcode, raspi, and dahdi hits were URLs inside comments (`evidence/firmware/maintainer_script_audit.tsv`). The audit is text-based and cannot rule out fetches in code the scripts call.

### 6.3 Self-contained coverage (verified from Contents)

- No area of trixie ships any b43 or b43legacy firmware blob. The only matches are the two `.placeholder` files. The proprietary blobs are not in the archive, so the installers are the only route to them. Self-contained b43 coverage is not possible from Debian content.
- `firmware-brcm80211` (non-free-firmware, no fetch) ships 111 `brcm/` paths, including brcmfmac (FullMAC) and brcmsmac (SoftMAC, `brcm/bcm43xx-0.fw` and `brcm/bcm43xx_hdr-0.fw`), plus `cypress/` (cyfmac) paths.
- The other firmware packages in the set show no fetch commands in the maintainer-script audit (section 6.2).
- Five packages in the set are not Wi-Fi firmware: `indi-dsi`, `libsbig4t64`, `libfishcamp1t64` (astronomy camera firmware), `alsa-topology-conf`, and `wireless-regdb`. They are kept to preserve the current image's hardware coverage. The owner may drop them.

### 6.4 Recommended disposition

1. **Stop scheduling the two installers.** Set `--firmware-chroot false` in `build/live-desktop/auto/config` and add an explicit firmware package list of 42 packages (`evidence/firmware/proposed_set_without_b43_installers_42.txt`). The list reproduces the Codex image's firmware set minus the two installers and minus the ARM packages. Its contents are fully determined by Debian's signed archive (apt-secure). This is a supply-chain improvement: the build no longer contacts GitHub or OpenWrt, and the chroot no longer fails on network access. It is also an explicit reduction, described next.
2. **Explicit capability reduction (owner acknowledgement required).** Wi-Fi drivers for Broadcom SoftMAC chips driven by `b43` (for example BCM4306, BCM4311–4318, and BCM4331-class parts) and `b43legacy` (BCM4301 and BCM4306 revision 2) will not work out of the box. brcmfmac and brcmsmac coverage remains. Hardware-specific verification is NOT_RUN.
3. **Opt-in path for users.** On an installed system with network access, `apt install firmware-b43-installer` (or `firmware-b43legacy-installer`) remains available. It downloads only on a matching Broadcom card or when explicitly configured, and it checks the embedded SHA-512. This path is not verified here (NOT_RUN).
4. **Owner or legal option (not recommended without review).** Fetch `broadcom-wl-6.30.163.46.tar.bz2` at build time, verify SHA-512 `0144894f…`, run `b43-fwcutter`, and package the result as a local `.deb`. This would be a supply-chain improvement over install-time fetching, but it would redistribute Broadcom-derived firmware inside the ISO. Debian ships these installers in contrib and does not ship the blobs themselves. A licence review is required first.
5. **Verify `--firmware-binary true`** (the installer-side firmware list uses the same heuristic). Check that no b43 installer `.deb` is copied into the installer payload (NOT_RUN).

## 7. Closure and origin results (early signal, emulated; native gate governs)

### 7.1 Closure classes

- **CLEAN:** the strict design (Kali origin `-1`, allowlist = the candidate) resolves. Every Kali-origin line is the candidate, no package is removed, and every allowlisted name is absent from trixie.
- **CLEAN_WITH_KALI_ONLY_DEPS:** resolves only after Kali-only dependencies are allowlisted. Those names are absent from trixie. No base-library replacement.
- **LEAKY_BASE_SKEW:** the closure needs a Kali version of a package that trixie also provides, at a version trixie does not have (base replacement). Recorded with the first blocker found by the iterative solver.
- **UNRESOLVABLE:** no trixie or Kali candidate, or a conflict. No such case occurred for the Kali candidates.

Method: an iterative solver ran `apt-get -s install --no-install-recommends` under the strict design. Each unmet dependency was classified by whether trixie has the name (base skew) or only Kali does (Kali-only dependency). The allowlist grew until the closure resolved or a blocker was reached. Logs: `evidence/f2_closure/<tool>.i<N>.strict.log`; results: `evidence/f2_closure/closure_solver_results.json`.

### 7.2 Results per requested candidate

| Candidate (Kali version) | Class | Allowlist needed | Strict new packages (tier 1) | Measure mode: Debian base packages Kali would upgrade (tier 1 / M1 context) | Low-priority (50), candidate-only result | Control (no allowlist) |
|---|---|---|---|---|---|---|
| zaproxy 2.17.0-0kali1 | CLEAN | zaproxy | 132 | 7 / 29 | No unlisted Kali package pulled | Unavailable |
| burpsuite 2026.8-0kali1 | CLEAN (licence-blocked) | burpsuite | 134 | 7 / 29 | No unlisted Kali package pulled | Unavailable |
| exploitdb 20260904-0kali1 | CLEAN | exploitdb | 3 | 0 / 29 | No unlisted Kali package pulled | Unavailable |
| ghidra 12.1.4+ds-0kali1 | CLEAN (JDK from Debian-Security) | ghidra (+ ghidra-data optional) | 132 (133 with ghidra-data) | 7 / 29 | No unlisted Kali package pulled | Unavailable |
| amass 5.1.1-0kali3 | CLEAN_WITH_KALI_ONLY_DEPS | amass, libpostal-data | 4 | 4 / 29 | Pulled unlisted `libpostal-data` | Unavailable |
| radare2 6.0.4+dfsg-1 | CLEAN_WITH_KALI_ONLY_DEPS | radare2, libradare2-6.0.0t64, libradare2-common | 7 | 0 / 29 | Pulled unlisted `libradare2-6.0.0t64`, `libradare2-common` | Unavailable |
| mitmproxy 12.2.3-0kali5 | LEAKY_BASE_SKEW | blocker: `libc6 >= 2.42` for the Kali libpython3.14 stack | not reached | 3 / 30 | Failed (no base upgrade under pin 50) | Unavailable |
| wpscan 4.1.0-0kali1 | LEAKY_BASE_SKEW | blocker: `ruby-addressable >= 2.9` (trixie 2.8.7-2) | not reached | 2 / 29 | Failed | Unavailable |
| rizin 0.9.1-0kali1 | LEAKY_BASE_SKEW | blocker: `librizin0` needs `libc6 >= 2.42` | not reached | 3 / 29 | Failed | Unavailable |
| rizin-cutter 2.5.0-0kali1 | LEAKY_BASE_SKEW | blocker: `libqt6core6t64 >= 6.10.2` (trixie 6.8.2) | not reached | 10 / 29 (and Kali Qt 6.10.2 as a new install) | Failed | Unavailable |
| metasploit-framework 6.5.3-0kali1 | LEAKY_BASE_SKEW | blocker: `oracle-instantclient-basic` needs `libaio1t64 >= 0.3.113-8+kali1` | not reached | 12 / 29 | Failed | Unavailable |
| librewolf 157.0.1-1 (own repository) | CLEAN | librewolf | 92 (all Debian-origin) | 0 | Pins not needed (exit 0) | n/a |

Notes:

- "Measure mode" puts Kali at priority 500 with no allowlist. It is a leak measurement only and must never be used to install. Its M1-context figure is the number of Debian packages in the 53-package M1 Debian set that a general Kali source would upgrade.
- "M1 context" resolves the 53 Debian names of `cham3-desktop` and `cham3-full` plus the candidate. Under the strict design the CLEAN candidates resolve, with 1107 to 1114 new packages each (Debian-only baseline 1105). The LEAKY candidates were not run in strict mode because they already fail in tier 1.
- The control runs confirm each Kali candidate is unavailable without an allowlist entry (`Package '<name>' has no installation candidate`).
- Blockers for LEAKY candidates are the first ones the solver found. Other blockers may exist behind them.
- Kali candidates in the CLEAN class are also subject to the `-1` pin behaviour described in section 3.3.

### 7.3 LibreWolf closure

`librewolf` 157.0.1-1 under strict, measure, and low-priority pins: exit 0 in all three. The closure is 92 new packages, all Debian-origin except `librewolf` (`evidence/f2_librewolf/closure_librewolf.*.log`). No base package is upgraded.

### 7.4 Origin preservation (simulated dist-upgrade)

| Case | Result | Evidence |
|---|---|---|
| Strict design, no allowlist | 1 upgrade (Debian-Security `liblzma5`), 0 Kali lines | `f2_origin/A_strict_no_allow.dist_upgrade.log` |
| Naive control (Kali 500, no allowlist) | 117 upgrades + 36 new from Kali; 153 Kali lines | `f2_origin/B_naive.dist_upgrade.log` |
| Strict design, allowlisted `exploitdb` (simulated installed 20250101-0kali1) | Only the allowlisted package follows Kali (to 20260904-0kali1); Debian packages update from Debian | `f2_origin/C_strict_allowlisted_upgrade.dist_upgrade.log` |
| Naive control with the same simulated state | 118 Kali upgrades, including base libraries | `f2_origin/D_naive_allowlisted_upgrade.dist_upgrade.log` |

## 8. Verification status summary

| Check | Result | Environment and evidence |
|---|---|---|
| Trixie archive index presence (approved suites) | PASS (index read) | Emulated amd64 container, apt 3.0.3; `evidence/f2_equivalents/trixie_availability_and_dangling_names.txt` |
| Trixie `Release` signature | PASS for GOODSIG by `04B54C3C…88BD`; second signature `4CB5…E131` NOT_RUN | `evidence/f2_keys/trixie_release_signature_check.txt` |
| Kali archive key (HTTPS fetch, fingerprint, cross-check to signed keyring package) | PASS | `evidence/f2_keys/kali_*` |
| Kali repository signature via apt | PASS | `evidence/f2_keys/kali_apt_update.log` |
| LibreWolf key (extrepo checksum = repository key file; fingerprint on repository page) | PASS | `evidence/f2_librewolf/repo_landing_and_hashes.txt` |
| LibreWolf repository signature (`Release`, `InRelease`) | PASS (host gpg; apt update in container) | `evidence/f2_librewolf/release_gpg_verify_host.txt`, `apt_update_librewolf.log` |
| Tor Browser key (Tor Project page vs bundled key) | PASS | `evidence/f2_keys/tor_project_verify_excerpt.txt`, `torbrowser_launcher_bundled_tor_developers.asc` |
| torbrowser-launcher package hash and scripts | PASS (index hash; scripts read) | `evidence/f2_keys/torbrowser_launcher_postinst.txt` |
| Kali closure (11 candidates) | Early signal only (emulated). Native gate NOT_RUN. | `evidence/f2_closure/` |
| LibreWolf closure | Early signal only (emulated). Native gate NOT_RUN. | `evidence/f2_librewolf/` |
| Origin preservation (dist-upgrade dry-runs) | Early signal only (emulated). Native gate NOT_RUN. | `evidence/f2_origin/` |
| Firmware installers (scheduling mechanism, fetch behaviour, audit) | PASS (read from primary sources and indices). Build fix NOT_RUN. | `evidence/firmware/` |
| Wireshark setuid fallback | PASS (read from postinst). Fix and tests NOT_RUN. | `evidence/wireshark/` |
| Any package installed or executed | NOT_RUN (out of scope) | — |
| Licences of candidates | NOT_RUN (except as noted) | — |
| Rapid7 apt repository | BLOCKED (HTTP 403 from research environment) | — |

## 9. Evidence index

All files are under `docs/research/evidence/`. Checksums are in `evidence/SHA256SUMS`.

- `f2_closure/`: per-tool solver transcripts (`<tool>.i<N>.strict.log`), control (`<tool>.ctl.control.log`), low-priority (`<tool>.lp.lowprio.log` with the final allowlist, `<tool>.lp0.lowprio.log` candidate-only), measure (`<tool>.ms.measure.log`), `ghidra_plus_data.strict.log`; results JSON (`closure_solver_results.json`, `closure_modes_results.json`, `lowprio_candidate_only.json`, `m1_context_results.json`); Kali version and hash indices (`kali_candidate_index.txt`, `kali_candidate_hashes.txt`); size and file-list facts (`kali_pkg_sizes.txt`, `exploitdb_searchsploit_paths.txt`, `libpostal_data_contents.txt`); `kali_control/` (control files, no postinst). `m1_context/` holds gzipped M1-context transcripts.
- `f2_origin/`: four `dist-upgrade` dry-runs (A to D).
- `f2_librewolf/`: strict, measure, and low-priority closure logs; apt update log; key evidence and extrepo entry; repository landing page and hashes; host signature verification.
- `f2_keys/`: Kali archive key (armored, SHA-256, fingerprint display, keyring package listing); LibreWolf key files; trixie signature check; Tor Project excerpt, bundled key, mirror list, and postinst.
- `f2_equivalents/`: trixie availability and dangling names; equivalent search output.
- `firmware/`: apt firmware queries; live-build selection (47, 48, 44, 42 lists); Contents-derived selection (gzipped); maintainer-script audit (TSV and scripts of all 47 packages); live-build firmware scripts; Codex b43 failure excerpt.
- `wireshark/`: retained `wireshark-common` 4.4.19 postinst (copied from `origin/review/M1-codex-b82919c`, `docs/evidence/M1-bootable-desktop-distribution/codex_b82919c/`).

Packages downloaded for static reading only: `kali-archive-keyring_2025.2_all.deb`, `libpostal-data`, `exploitdb`, `torbrowser-launcher_0.3.7-3`, `firmware-b43-installer_019-14`, `firmware-b43legacy-installer_019-14`, `live-build_1:20250505+deb13u1`, `extrepo-offline-data_1.0.6~deb13u1`, and the 47 firmware packages (scripts only, deleted after reading).

## 10. Unresolved decisions

Owner:

1. LibreWolf: approve or reject `https://repo.librewolf.net` (section 3.1).
2. Tor Browser: accept torbrowser-launcher's self-update and first-use download outside apt (section 3.2).
3. Kali allowlist: approve per-package entries for the CLEAN candidates (`zaproxy` or `burpsuite`, `exploitdb`, `ghidra` with or without `ghidra-data`, `amass` with `libpostal-data`, `radare2` with two libraries). Each entry needs the native gate and Codex re-review.
4. Scope amendments needed for LEAKY candidates: `metasploit-framework` (LEAKY; proprietary Oracle client), `mitmproxy` (LEAKY; the only compliant route is upstream PyPI, which needs its own approval), `wpscan` (LEAKY; non-free; API terms), `rizin` and `rizin-cutter` (LEAKY; use radare2 instead).
5. ZAP or Burp (Burp needs a licence review first).
6. Ghidra footprint and source (Kali package or NSA upstream release).
7. Firmware: accept the removal of the two b43 installers as an explicit reduction, or approve a build-time hash-pinned blob package with a licence review (section 6.4).
8. Wireshark: approve the fail-closed capture design (section 5).
9. Kali keyring: approve shipping a single-key keyring in packaging (section 3.3).
10. Name substitutions: `plasma-workspace`, `kde-spectacle`, `qml-module-org-kde-qqc2desktopstyle`, `bind9-dnsutils`, `libimage-exiftool-perl`. Zenmap as the Nmap GUI.

Reviewer and coordinator:

11. Run the native gates in `M1_readiness_packet.md` section 6 on the target commit. Replace the early signals in section 7 with native results.
12. Confirm the trixie second signature (`4CB5…E131`) belongs to an archive key, or document why it is absent from the shipped keyring.
13. Confirm `searchsploit` and the `exiftool` binary paths in the built image. Confirm `ghidra` starts without `ghidra-data`.
14. Confirm whether `security.capability` xattrs survive the live-build squashfs (section 5).
15. Check `--firmware-binary true` output for b43 installer packages (section 6.4).
