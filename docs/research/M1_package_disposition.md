# M1 package disposition matrix

- **Status:** RESEARCH DRAFT. Availability facts only. Nothing in this document is installed, built, or executed.
- **Task:** M1 built-in application suite, per `docs/tasks/M1-bootable-desktop-distribution.md`.
- **Coordinator contract:** `M1_interface_contract.md`, baseline `401c6cf`, branch `docs/M1-research`.
- **Retrieval date:** 2026-10-11.
- **Owner:** `m1_docs_research`. Independent review required before any Kali-origin or third-party item is included (see the readiness packet).

## 1. Evidence method and snapshots

Availability was checked against the primary archive indices, not only against web pages. Web pages were used for upstream and provenance facts.

| Short name | Source | Snapshot / date | Notes |
|---|---|---|---|
| `IDX_TRIXIE_MAIN` | `https://deb.debian.org/debian/dists/trixie/main/binary-amd64/Packages.xz` | `Release` Date Sat, 12 Sep 2026 07:55:41 UTC; sha256 of `Packages.xz` `7778d3e3f303b7ddb8ce0fe7c8d57473a076c6bf2e8f241f75421d2396352498` | Authoritative for trixie main. Used for presence, version, and area. |
| `IDX_TRIXIE_CONTRIB` | `.../trixie/contrib/binary-amd64/Packages.xz` | same snapshot; sha256 `926e365110c83a7bc974c2700a82cbd833a4ce1ea6b4134c97b6aba85e5ecb84` | |
| `IDX_TRIXIE_NONFREE` | `.../trixie/non-free/binary-amd64/Packages.xz` | same snapshot; sha256 `873a1152a9641bbbd32ae06d9bea40c0eb9486caeea340a786d4ab84cb6de6c5` | |
| `IDX_TRIXIE_NONFREE_FW` | `.../trixie/non-free-firmware/binary-amd64/Packages.xz` | same snapshot; sha256 `a05bfaf35b64eb8271e374f617b3b75b5ba24dacfa86fd7ddbec5a60d99a17da` | |
| `CONTENTS_TRIXIE_MAIN` | `https://deb.debian.org/debian/dists/trixie/main/Contents-amd64.gz` | same snapshot | Used to confirm which binary package ships a file. |
| `TRIXIE_RELEASE` | `https://deb.debian.org/debian/dists/trixie/Release` | sha256 `ed56aac47e7911e65ee63aae8d67e29f20e023f840e49f8ed037043a33de138a` | Origin Debian, Suite stable, Codename trixie. |
| `IDX_KALI_MAIN` | `https://http.kali.org/kali/dists/kali-rolling/main/binary-amd64/Packages.gz` | `kali-rolling` Release Date Fri, 09 Oct 2026 12:06:20 UTC; sha256 `5302e9aa9fcffaf31bc607e92237db807f6a236bb922eb61b97c5ed64886f1d0` | Kali archive is `Origin: Kali`, `Suite: kali-rolling`. Kali publishes `Packages.gz` only, not `.xz`. |
| `IDX_KALI_NONFREE` | `.../kali-rolling/non-free/binary-amd64/Packages.gz` | sha256 `8854213cd07d7e2367aa2be4844e8039cd2836722c728d7eeb86dc61664e5b0a` | |
| `IDX_KALI_CONTRIB` | `.../kali-rolling/contrib/binary-amd64/Packages.gz` | sha256 `0ff407bf4b860874c3ef70c160428b3be502efa6504da53b0202c8113a4c28d0` | |
| `IDX_BACKPORTS_MAIN` | `https://deb.debian.org/debian/dists/trixie-backports/main/binary-amd64/Packages.xz` | Release Date Sun, 11 Oct 2026 20:09:52 UTC | Checked for the Kali-only tools. No matches. Backports is not used as a source in this matrix. |

Key authentication checks performed in this research:

- **Kali archive signature (PASS):** `kali-rolling/Release` was verified against `Release.gpg` using the published Kali archive key `https://archive.kali.org/archive-key.asc`. Result: "Good signature from Kali Linux Archive Automatic Signing Key (2025) <devel@kali.org>". Fingerprint `827C 8569 F251 8CC6 77FE CA1A ED65 462E C8D5 E4C5`, RSA 4096, created 2025-04-17, expires 2028-04-17. The "not certified" warning is expected for an unimported key. The key's trust still depends on how the target system obtains it (see section 4).
- **Debian trixie Release signature (NOT_RUN):** `Release.gpg` is signed by key `4CB50190207B4758A3F73A796ED0E7B82643E131`. The matching public key was not retrieved in this session (the `archive-key-13-automatic.asc` URL returned 404, and the `archive-key-13.asc` key fetched was a different key). Trixie signatures remain to be verified in the build environment.
- **Not done:** no `.deb` files were downloaded or installed. No package file lists or dependency closures were resolved beyond the index fields shown.

Important version note: `packages.debian.org` pages fetched during this research sometimes show newer point releases than the 2026-09-12 index (for example, `wireshark` 4.4.19 on the web page versus 4.4.18 in the index). Versions below are from the index snapshot unless marked otherwise.

## 2. Matrix

Status key for the "Selected / equivalent / gap" column:

- **INCLUDE-NOW:** Debian 13 package present in trixie main/contrib/non-free; a candidate for the default `cham3-full` dependency set after normal review.
- **GAP-DEBIAN:** Not in trixie. No suitable Debian-native equivalent in trixie.
- **GATED-KALI:** Kali-origin candidate only. Not included. Requires the Kali review in section 5.
- **GATED-THIRDPARTY:** Third-party repository or distribution path. Not included. Requires separate review.
- **EXCLUDE:** Redundant duplicate; must be absent from the image.

Evidence URLs use `packages.debian.org/trixie/<pkg>` where the page was fetched, and the index name otherwise. Licences were not individually re-verified in this research; that column says so.

### 2a. Privacy browsers

| # | Requested tool | Debian 13 package | Available in trixie? | Selected / equivalent / gap | Source, auth, licence, update | Dependency / conflict / privilege | Evidence | Unresolved |
|---|---|---|---|---|---|---|---|---|
| P1 | LibreWolf (clearnet) | None. The name `librewolf` does not appear in `IDX_TRIXIE_*` or `IDX_KALI_*`. | **no.** Not in Debian. Not in Kali archive index either. | **GATED-THIRDPARTY.** Debian-native mechanism is `extrepo` (trixie 0.14, admin, main) with the `librewolf` repository definition. Section 3.1. | Upstream repo `https://repo.librewolf.net`, suite `librewolf`, component `main`, arch amd64 arm64. Signing key fingerprint `662E 3CDD 6FE3 2900 2D0C A5BB 4033 9DD8 2B12 EF16` (`LibreWolf Maintainers <gpg@librewolf.net>`, created 2022-05-14), sha256 `cd1c45ebf33b42191af369b5ea901508eb7dff3dd5086abcb601b25121bc70ad`. Source: `extrepo-offline-data` 1.0.6~deb13u1 (`index.yaml`, trixie). Licence not checked. | Repo-owned update path, outside apt-of-trixie. Not a Debian archive. | `https://packages.debian.org/trixie/extrepo`; `https://wiki.debian.org/extrepo`; `https://librewolf.net/installation/debian/`; `IDX_TRIXIE_MAIN` (extrepo) | Owner decision to approve the third-party source. Independent review. Whether to ship the key in the image rather than fetch it at runtime. |
| P2 | Official Tor Browser (Tor browsing) | `torbrowser-launcher` (the official bundle is downloaded and verified by the launcher; no Debian package of Tor Browser exists) | **yes. contrib.** 0.3.7-3 (index section `contrib/python`; source section `contrib/web`; packages.debian.org lists it under contrib). amd64 and i386. | **INCLUDE-NOW** (contrib). Section 3.2. The `tor` daemon is a Recommends. | Debian archive, authenticated by the trixie Release signature. Browser bundle is downloaded from torproject.org and verified by Tor Project GPG signature; the browser then self-updates through Tor's updater (outside apt). | Depends: `gnupg`, `python3-gpg`, `python3-packaging`, `python3-pyqt5`, `python3-requests`, `python3-socks`, `ca-certificates`, `libdbus-glib-1-2`, `libgtk-3-0`, `libasound2`, `python3`. Recommends `tor`. Suggests `apparmor`. Runs per user, installs to `~/.local/share/torbrowser`. No root. | `https://packages.debian.org/trixie/torbrowser-launcher`; `IDX_TRIXIE_CONTRIB` (sha256 of deb `a70a9da89b4ea3adf4542de27595ab3d8e6914c5efd66ecbded627157f3b5274`, size 54872) | Whether `contrib` is enabled in the image (contract says yes). Whether the self-update path is acceptable for M1. Whether `tor` is installed by default. |
| P3 | Firefox ESR (generic stock browser; explicitly NOT to be the stock browser) | `firefox-esr` 140.15.0esr-1~deb13u1 (web, main) | yes, main | **EXCLUDE** as the stock/clearnet browser per owner rule. Listed only so the decision is explicit. Not a substitute for LibreWolf without a new owner decision. | Debian archive | Would conflict with the stock-browser role. Its mobile-config helper is unrelated. | `IDX_TRIXIE_MAIN` | Owner decision if the clearnet role is to fall back to Debian's Firefox ESR while LibreWolf is gated. |

### 2b. Traffic analysis

| # | Requested tool | Debian 13 package | Available in trixie? | Selected / equivalent / gap | Source, auth, licence, update | Dependency / conflict / privilege | Evidence | Unresolved |
|---|---|---|---|---|---|---|---|---|
| T1 | Wireshark | `wireshark` 4.4.18-0+deb13u1 (net, main); `wireshark-common` 4.4.18-0+deb13u1 (provides `dumpcap`) | **yes, main.** Index version 4.4.18-0+deb13u1. Web page shows security update 4.4.19-0+deb13u1 (later). | **INCLUDE-NOW.** Capture via group `wireshark` (section 4). | Debian archive. Source package `wireshark`. Upgrades through apt. | `wireshark-common` creates group `wireshark` and owns `dumpcap`. Capture without root by group membership plus file capabilities (section 4). | `https://packages.debian.org/trixie/wireshark` (4.4.19 web); `IDX_TRIXIE_MAIN` (`wireshark-common` sha256 `b95ab65ae1e20b36d8fffbe88dac1c632a2b576e4f8c6ffa53cabd2f81033978`) | Re-check the trixie version at the target commit. Confirm the `install-setuid` answer (section 4). |
| T2 | tshark | `tshark` 4.4.18-0+deb13u1 (net, main) | **yes, main.** | **INCLUDE-NOW.** | Debian archive | Same capture privilege as T1. | `IDX_TRIXIE_MAIN` (sha256 `adeef5cc162cb0aba9b41b6bf67cbed99dc6b1ca4a07ca94f3e50c917c87bce6`) | None. |
| T3 | mitmproxy / mitmweb | None in trixie. | **no.** Not in `IDX_TRIXIE_*`. Debian tracker shows it in testing/sid only: sid and testing 8.1.1-4, flagged for autoremoval from testing on 2026-11-16 (RC bug #1149800). Debian sid also has an open CVE-2026-40606 (fixed upstream in mitmproxy 12.2.2+). | **GAP-DEBIAN** in trixie. Kali candidate (below). | Kali `mitmproxy` 12.2.3-0kali5 (net, `all`), `IDX_KALI_MAIN`, sha256 `0d2d281a0cc9353a612efb2295124226bf6060f1802976b0e4fe5c45dbc6a26d`, size 1328956. Upstream licence not checked here. | Would be a cross-release dependency if taken from Debian sid. | `https://tracker.debian.org/pkg/mitmproxy`; `IDX_TRIXIE_MAIN` (no match) | Kali candidate review (section 5). Owner decision on whether to ship without mitmproxy in M1. |
| T4 | OWASP ZAP (zaproxy) | None. Tracker lookup returned 404 (no Debian source package). | **no.** | **GAP-DEBIAN.** Kali candidate: `zaproxy` 2.17.0-0kali1 (utils, `all`), `IDX_KALI_MAIN`, sha256 `9490cc3c613ca847db8f12c5c6b00d162e17274e4fcfed566bb93d1aa1feda47`, size 221820224. | Kali. Upstream licence not checked here. | Java runtime, large closure. Not computed. | `https://tracker.debian.org/pkg/zaproxy` (404); `IDX_TRIXIE_MAIN` (no match) | Kali review. Choice vs Burp (T5). Owner decision on ZAP vs Burp. |
| T5 | Burp Suite Community | None. Debian `burp` is an unrelated backup program (`Simple cross-platform network BackUp and Restore Program`). Do not treat it as Burp Suite. | **no.** | **GAP-DEBIAN.** Kali candidate: `burpsuite` 2026.8-0kali1 (net, main), `IDX_KALI_MAIN`, sha256 `6e68cad47c66d854ab944aa417a54290c18b396e0727583108b790b857638b1f`, size 344402560. | Kali. The PortSwigger licence and redistribution terms are NOT checked in this research and are a blocking review item. | Large closure. | `IDX_TRIXIE_MAIN` (`burp` description); `IDX_KALI_MAIN` | Licence and redistribution review. Choose ZAP or Burp, or neither. |
| T6 | tcpdump | `tcpdump` 4.99.5-2 (net, main) | **yes, main.** | **INCLUDE-NOW.** | Debian archive | Packet capture by root or capabilities. Not granted by this research. | `https://packages.debian.org/trixie/tcpdump` | Whether tcpdump gets the same capture-group treatment as Wireshark. Recommended: no, out of scope. |
| T7 | nethogs | `nethogs` 0.8.8-1 (net, main) | **yes, main.** | **INCLUDE-NOW.** | Debian archive | Needs root or capability to read per-process network stats. Not granted by this research. | `https://packages.debian.org/trixie/nethogs` | Same as T6. |
| T8 | iftop | `iftop` 1.0~pre4-9+b1 (net, main) | **yes, main.** | **INCLUDE-NOW.** | Debian archive | Same as T6. | `https://packages.debian.org/trixie/iftop` | Same as T6. |

### 2c. Information gathering

| # | Requested tool | Debian 13 package | Available in trixie? | Selected / equivalent / gap | Source, auth, licence, update | Dependency / conflict / privilege | Evidence | Unresolved |
|---|---|---|---|---|---|---|---|---|
| I1 | nmap | `nmap` 7.95+dfsg-3 (net, main) | **yes, main.** | **INCLUDE-NOW.** | Debian archive | SYN and raw scans need root or capabilities. Not granted by this research. | `https://packages.debian.org/trixie/nmap` | None. |
| I2 | Zenmap | `zenmap` 7.95+dfsg-3 (net, main; source `nmap`) | **yes, main.** | **INCLUDE-NOW** as the trixie-available Nmap GUI. Toolkit and KDE fit were NOT verified in this research. | Debian archive | Same as I1. | `https://packages.debian.org/trixie/zenmap` | Whether Zenmap (GTK) is acceptable on a KDE desktop, or an alternative is wanted. Not researched further. |
| I3 | masscan | `masscan` 2:1.3.2+ds1-2 (net, main) | **yes, main.** | **INCLUDE-NOW.** | Debian archive | Raw packets, needs root or capabilities. | `https://packages.debian.org/trixie/masscan` | Same as I1. |
| I4 | amass | None in trixie (`IDX_TRIXIE_*` no match). Tracker lookup returned 404. | **no.** | **GAP-DEBIAN.** Kali candidate: `amass` 5.1.1-0kali3 (net, main), `IDX_KALI_MAIN`, sha256 `72d3563addb64ada4527170be7b45a88d7aec29acb367261365a6beecea11999`, size 12420072. | Kali. Upstream licence not checked. | Go binary. Not computed. | `https://tracker.debian.org/pkg/amass` (404); `IDX_TRIXIE_MAIN` (no match) | Kali review, or omit. |
| I5 | dnsutils | `dnsutils` is a virtual package provided by `bind9-dnsutils` 1:9.20.26-1~deb13u1 (net, main) | **yes** through `bind9-dnsutils`. | **INCLUDE-NOW** by installing `bind9-dnsutils`, not the virtual name. | Debian archive | None beyond BIND client libs. | `https://packages.debian.org/trixie/dnsutils` (virtual); `IDX_TRIXIE_MAIN` | Name the real package in the manifest. |
| I6 | whois | `whois` 5.6.3 (net, main) | **yes, main.** | **INCLUDE-NOW.** | Debian archive | None. | `https://packages.debian.org/trixie/whois` | None. |

### 2d. Web application analysis

| # | Requested tool | Debian 13 package | Available in trixie? | Selected / equivalent / gap | Source, auth, licence, update | Dependency / conflict / privilege | Evidence | Unresolved |
|---|---|---|---|---|---|---|---|---|
| W1 | sqlmap | `sqlmap` 1.9.6-1 (net, main) | **yes, main.** | **INCLUDE-NOW.** | Debian archive | None. | `IDX_TRIXIE_MAIN` | None. |
| W2 | ffuf | `ffuf` 2.1.0-1+b9 (devel, main) | **yes, main.** | **INCLUDE-NOW.** | Debian archive | None. | `IDX_TRIXIE_MAIN` | None. |
| W3 | gobuster | `gobuster` 3.6.0-1+b10 (utils, main) | **yes, main.** | **INCLUDE-NOW.** | Debian archive | None. | `IDX_TRIXIE_MAIN` | None. |
| W4 | nikto | `nikto` 1:2.1.5-3.1 (net, **non-free**) | **yes, non-free.** | **INCLUDE-NOW, but only if the image enables `non-free`.** The contract says it does. | Debian archive | Non-free area; the reason it is non-free is not checked in this research. | `IDX_TRIXIE_NONFREE` | Confirm the non-free licence terms are acceptable for an M1 image. |
| W5 | wpscan | None in trixie. Tracker lookup returned 404. | **no.** | **GAP-DEBIAN.** Kali candidate: `wpscan` 4.1.0-0kali1 (utils, **non-free**), `IDX_KALI_NONFREE`, sha256 `20caf190d6adc4cc2bedc5b8df34038aaacb452853ac5dfede260aafe49583e9`, size 109064. | Kali. Non-free in Kali as well. Licence not checked here. Wpscan uses a WordPress vulnerability database API that the user must consider. | Ruby runtime and gems. Not computed. | `https://tracker.debian.org/pkg/wpscan` (404); `IDX_TRIXIE_*` (no match) | Kali review. Licence and API-service review. |

### 2e. Wireless and hardware

| # | Requested tool | Debian 13 package | Available in trixie? | Selected / equivalent / gap | Source, auth, licence, update | Dependency / conflict / privilege | Evidence | Unresolved |
|---|---|---|---|---|---|---|---|---|
| R1 | aircrack-ng suite | `aircrack-ng` 1:1.7+git20230807.4bf83f1a-2 (net, main) | **yes, main.** | **INCLUDE-NOW.** The contract says "aircrack-ng suite"; the package bundles the suite. | Debian archive | Monitor mode and packet injection need root and drivers. Not granted by this research. | `IDX_TRIXIE_MAIN` | Hardware support is out of M1 verification scope. |

### 2f. Exploitation and auditing

| # | Requested tool | Debian 13 package | Available in trixie? | Selected / equivalent / gap | Source, auth, licence, update | Dependency / conflict / privilege | Evidence | Unresolved |
|---|---|---|---|---|---|---|---|---|
| E1 | metasploit-framework | None in trixie. Tracker lookup returned 404. | **no.** | **GAP-DEBIAN.** Kali candidate: `metasploit-framework` 6.5.3-0kali1 (net, main), `IDX_KALI_MAIN`, sha256 `fcb1100612b748b5122f3413033279693f80fd42a9e6122c3464f4db748c19b6`, size 270131624. | Kali. Upstream licence not checked. Large Ruby closure. | Not computed. | `https://tracker.debian.org/pkg/metasploit-framework` (404); `IDX_TRIXIE_MAIN` (no match) | Kali review. Dependency closure. Whether an exploit framework belongs in M1 at all (owner). |
| E2 | searchsploit (exploitdb) | None in trixie (no `exploitdb` package). Tracker lookup returned 404. | **no.** | **GAP-DEBIAN.** Kali candidate: `exploitdb` 20260904-0kali1 (utils, `all`), `IDX_KALI_MAIN`, sha256 `b417d86c6e3c126d2fa2c9be2f8d6c10417697fc95ebaf5a729c299d59d3f385`, size 30414644. The `searchsploit` command is expected to be shipped by this package; not verified against its file list. | Kali. Upstream data licence not checked. | Not computed. | `https://tracker.debian.org/pkg/exploitdb` (404); `IDX_TRIXIE_MAIN` (no match) | Kali review. Confirm `searchsploit` file list. |

### 2g. Password and cryptographic auditing

| # | Requested tool | Debian 13 package | Available in trixie? | Selected / equivalent / gap | Source, auth, licence, update | Dependency / conflict / privilege | Evidence | Unresolved |
|---|---|---|---|---|---|---|---|---|
| C1 | hashcat | `hashcat` 6.2.6+ds2-1 (net, main) | **yes, main.** | **INCLUDE-NOW.** | Debian archive | Uses GPU drivers if present. | `IDX_TRIXIE_MAIN` | None. |
| C2 | john (John the Ripper) | `john` 1.9.0-2 (admin, main) | **yes, main.** | **INCLUDE-NOW.** | Debian archive | None. | `IDX_TRIXIE_MAIN` | None. |
| C3 | hydra | `hydra` 9.5-3 (net, main) | **yes, main.** | **INCLUDE-NOW.** | Debian archive | None. | `IDX_TRIXIE_MAIN` | None. |

### 2h. Reverse engineering and forensics

| # | Requested tool | Debian 13 package | Available in trixie? | Selected / equivalent / gap | Source, auth, licence, update | Dependency / conflict / privilege | Evidence | Unresolved |
|---|---|---|---|---|---|---|---|---|
| RE1 | ghidra | None in trixie. Tracker lookup returned 404. No Debian-native equivalent. | **no.** | **GAP-DEBIAN.** Kali candidate: `ghidra` 12.1.4+ds-0kali1 (misc, main), `IDX_KALI_MAIN`, sha256 `128b96ae74b22eb508401378950a6b8eb3274c2e29b50f10d1a59f1fca3caad6`, size 515275208. | Kali. Upstream licence not checked here. | JDK dependency. Not computed. | `https://tracker.debian.org/pkg/ghidra` (404); `IDX_TRIXIE_MAIN` (no match) | Kali review. Size (515 MB) and JDK closure. |
| RE2 | radare2 | None in trixie. Debian `radare2` 6.1.8+ds-1 exists only in unstable and was removed from testing on 2025-05-21 (migration blocked). Not usable under the no-cross-release rule. | **no.** | **GAP-DEBIAN** in trixie. Kali candidate: `radare2` 6.0.4+dfsg-1 (devel, main), `IDX_KALI_MAIN`, sha256 `dcb375213e9c1477ae08d47bfb25c1b13a6ba2d41bc2665b81a4bae66bb64eb0`, size 278352. | Kali. Debian unstable is not an allowed source for M1. | Not computed. | `https://tracker.debian.org/pkg/radare2` (unstable 6.1.8+ds-1; testing removed 2025-05-21); `IDX_TRIXIE_MAIN` (no match) | Kali review. Owner decision on radare2 vs Cutter vs Rizin. |
| RE3 | Cutter | None in trixie. | **no.** | **GAP-DEBIAN.** Kali candidate: `rizin-cutter` 2.5.0-0kali1 (the Cutter GUI, pulls Rizin), `IDX_KALI_MAIN`, sha256 `e0e647c191b41bd6b4ffb3bf6b76c8ed5dd381dc00ae179aab94edc7b0440d79`, size 2028760. Requires Kali `rizin` 0.9.1-0kali1 (`IDX_KALI_MAIN`, sha256 `41a2acc204a9adad6174139ea3b0b50639f67bb0e664bbf6601defdc91002fd1`). Rizin is absent from trixie. | Kali. | Cutter is a Qt GUI. Dependency closure includes Qt and Rizin from Kali. Not computed. | `IDX_TRIXIE_MAIN` (no `rizin`, no `cutter` match) | Kali review. Owner choice between Cutter and radare2 (the task allows either). |
| RE4 | binwalk | `binwalk` 2.4.3+dfsg1-2+deb13u1 (devel, main) | **yes, main.** | **INCLUDE-NOW.** | Debian archive | Some optional extractors pull in external tools. Not checked. | `IDX_TRIXIE_MAIN` | Optional extractor closure. |
| RE5 | exiftool | `libimage-exiftool-perl` 13.25+dfsg-1 (perl, main). Debian description says it includes the program. | **yes, main.** | **INCLUDE-NOW.** The `exiftool` command is provided by this package per its description; the file path was NOT verified against the Contents index. | Debian archive | Perl. | `IDX_TRIXIE_MAIN` | Verify the `exiftool` binary path in the built image. |

### 2i. KDE core utilities to keep

These are the upstream Debian KDE utilities from the task's "Built-in application suite" section. Names that differ from the contract are noted.

| # | Requested | Debian 13 package | Available in trixie? | Selected / equivalent / gap | Notes | Evidence | Unresolved |
|---|---|---|---|---|---|---|---|
| K1 | Plasma desktop | `plasma-desktop` 4:6.3.6-1 (kde, main) | **yes, main.** | **INCLUDE-NOW.** | Contract `Depends`. | `https://packages.debian.org/trixie/plasma-desktop` | None. |
| K2 | Plasma 6 Wayland session | Contract name `plasma-workspace-wayland` is **not in trixie** (no such binary package in `IDX_TRIXIE_MAIN`). The Wayland session file `/usr/share/wayland-sessions/plasma.desktop` is shipped by `plasma-workspace` 4:6.3.6-2 (kde, main). | **Contract name: no. Real package `plasma-workspace`: yes.** | **INCLUDE-NOW as `plasma-workspace`.** Substitution: contract's `plasma-workspace-wayland` is replaced by `plasma-workspace`. | Confirmed via `CONTENTS_TRIXIE_MAIN` (`usr/share/wayland-sessions/plasma.desktop` -> `kde/plasma-workspace`). | `https://packages.debian.org/trixie/plasma-workspace`; `CONTENTS_TRIXIE_MAIN` | Contract owner should accept the rename. |
| K3 | KWin Wayland | `kwin-wayland` 4:6.3.6-1 (kde, main) | **yes, main.** | **INCLUDE-NOW.** | | `https://packages.debian.org/trixie/kwin-wayland` | None. |
| K4 | SDDM | `sddm` 0.21.0+git20250502.4fe234b-2 (kde, main) | **yes, main.** | **INCLUDE-NOW.** | | `IDX_TRIXIE_MAIN` | Autologin for live is handled by live-config (contract). |
| K5 | System Settings | `systemsettings` 4:6.3.4-2 (kde, main) | **yes, main.** | **INCLUDE-NOW.** | Settings launcher target. | `https://packages.debian.org/trixie/systemsettings` | None. |
| K6 | Dolphin | `dolphin` 4:25.04.3-1+deb13u1 (utils, main) | **yes, main.** | **INCLUDE-NOW.** | File manager. | `https://packages.debian.org/trixie/dolphin` | None. |
| K7 | Konsole | `konsole` 4:25.04.2-1 (kde, main) | **yes, main.** | **INCLUDE-NOW.** | Terminal. | `https://packages.debian.org/trixie/konsole` | None. |
| K8 | Kate (and/or KWrite) | `kate` 4:25.04.3-2 (editors, main); `kwrite` 4:25.04.3-2 (editors, main) | **yes, main.** | **INCLUDE-NOW `kate`.** `kwrite` optional per the task's "and/or". | Editor. | `https://packages.debian.org/trixie/kate` | Pick Kate, KWrite, or both (owner or worker). |
| K9 | Ark | `ark` 4:25.04.3-1 (utils, main) | **yes, main.** | **INCLUDE-NOW.** | Archive manager. | `https://packages.debian.org/trixie/ark` | None. |
| K10 | Okular | `okular` 4:25.04.2-1+deb13u1 (graphics, main) | **yes, main.** | **INCLUDE-NOW.** | PDF viewer. | `https://packages.debian.org/trixie/okular` | None. |
| K11 | Gwenview | `gwenview` 4:25.04.3-1 (graphics, main) | **yes, main.** | **INCLUDE-NOW.** | Image viewer. | `https://packages.debian.org/trixie/gwenview` | None. |
| K12 | Spectacle | Contract name `spectacle` is **not in trixie**. The trixie binary is `kde-spectacle` 4:6.3.5-2 (graphics, main). | **Contract name: no. `kde-spectacle`: yes.** | **INCLUDE-NOW as `kde-spectacle`.** Substitution. | Screenshot utility. | `https://packages.debian.org/trixie/kde-spectacle`; `IDX_TRIXIE_MAIN` | Accept the rename. |
| K13 | Plasma System Monitor (or KSysGuard) | `plasma-systemmonitor` 6.3.6-1 (utils, main) | **yes, main.** | **INCLUDE-NOW.** No separate `ksysguard` binary package was found in trixie. | Resource monitor. | `https://packages.debian.org/trixie/plasma-systemmonitor` | None. |
| K14 | Fonts (Recommends) | `fonts-noto` 20201225-2 (fonts, main) | **yes, main.** | **INCLUDE-NOW (Recommends).** | | `IDX_TRIXIE_MAIN` | None. |
| K15 | qqc2-desktop-style (Recommends) | Contract name `qqc2-desktop-style` is **not in trixie**. The Plasma style module is `qml-module-org-kde-qqc2desktopstyle` 5.116.1-1 (kde, main). | **Contract name: no. Real package: yes.** | **INCLUDE-NOW as `qml-module-org-kde-qqc2desktopstyle`** (Recommends). Substitution. | | `IDX_TRIXIE_MAIN` | Accept the rename. Confirm which Qt major version the panel QML requires. |

### 2j. Duplicate utilities to exclude

Each row must be absent from the built image's package closure. Presence in the trixie archive is noted, because it means a conflict or accidental pull-in is possible.

| # | Redundant package | Replaces | Present in trixie? | Disposition |
|---|---|---|---|---|
| D1 | `thunar` | Dolphin | yes (main) | **EXCLUDE** |
| D2 | `pcmanfm` | Dolphin | yes (main) | **EXCLUDE** |
| D3 | `qterminal` | Konsole | yes (main) | **EXCLUDE** |
| D4 | `xfce4-terminal` | Konsole | yes (main) | **EXCLUDE** |
| D5 | `tilix` | Konsole | yes (main) | **EXCLUDE** |
| D6 | `mousepad` | Kate/KWrite | yes (main) | **EXCLUDE** |
| D7 | `leafpad` | Kate/KWrite | **no** (absent from trixie index) | **EXCLUDE** (already absent) |
| D8 | `xarchiver` | Ark | yes (main) | **EXCLUDE** |
| D9 | `file-roller` | Ark | yes (main) | **EXCLUDE** |
| D10 | `evince` | Okular | yes (main) | **EXCLUDE** |
| D11 | `atril` | Okular | yes (main) | **EXCLUDE** |
| D12 | `ristretto` | Gwenview | yes (main) | **EXCLUDE** |
| D13 | `firefox-esr` (as stock browser) | LibreWolf/Tor Browser roles | yes (main) | **EXCLUDE** as stock browser (see P3) |

Note: `kwrite` (K8) is a KDE editor and is not in this exclusion list.

### 2k. Packages named in the contract but not in trixie

| Contract name | Trixie result | Real package used |
|---|---|---|
| `plasma-workspace-wayland` | not present | `plasma-workspace` (K2) |
| `spectacle` | not present | `kde-spectacle` (K12) |
| `qqc2-desktop-style` | not present | `qml-module-org-kde-qqc2desktopstyle` (K15) |
| `dnsutils` | virtual only | `bind9-dnsutils` (I5) |
| `exiftool` | not a package name | `libimage-exiftool-perl` (RE5) |

## 3. Resolutions

### 3.1 LibreWolf: distribution and update path (GATED)

**Finding:** LibreWolf is not in Debian 13 and not in the Kali archive index (`IDX_KALI_MAIN` and `IDX_KALI_NONFREE` have no `librewolf` package). The Debian-native mechanism for a curated third-party repository is `extrepo`, which is in trixie (`extrepo` 0.14, admin, main; `https://packages.debian.org/trixie/extrepo`). Debian's own description says extrepo lists third-party repositories and that Debian "does not guarantee the security or quality of software in these repositories."

**What extrepo supplies for LibreWolf** (from `extrepo-offline-data` 1.0.6~deb13u1, `usr/share/extrepo/offline-data/debian/trixie/index.yaml`, retrieved from `https://deb.debian.org/debian/pool/main/e/extrepo-data/extrepo-offline-data_1.0.6~deb13u1_all.deb`):

- `policy: main`, `contact: maltejur@dismail.de`, `description: LibreWolf Browser`.
- Source: `Types: deb`, `URIs: https://repo.librewolf.net`, `Suites: librewolf`, `Components: main`, `Architectures: amd64 arm64`.
- Key file `librewolf.asc`, `gpg-key-checksum sha256: cd1c45ebf33b42191af369b5ea901508eb7dff3dd5086abcb601b25121bc70ad` (verified in this research: the file's sha256 matches).
- Key fingerprint (from `gpg --show-keys --with-fingerprint`): `662E 3CDD 6FE3 2900 2D0C A5BB 4033 9DD8 2B12 EF16`, `LibreWolf Maintainers <gpg@librewolf.net>`, RSA 4096, created 2022-05-14. Signing subkey `4DD4 BBEC ABD3 2E79 8CC7 7A40 4798 1EA7 3D0E 1C63`.

**Upstream install instructions** (`https://librewolf.net/installation/debian/`, retrieved 2026-10-11): the page directs users to `sudo extrepo enable librewolf && sudo extrepo update librewolf`, then `sudo apt install librewolf`. The page itself does not publish a key fingerprint; the only key reference found is the local `/var/lib/extrepo/keys/librewolf.asc`.

**Authentication model:**

- APT verifies the repository `Release`/`InRelease` signature against the key named in the generated sources file (extrepo writes it under `/etc/apt/sources.list.d/`).
- Trust in that key comes from extrepo's shipped copy plus the sha256 in extrepo's data. This is a chain through extrepo's curation, not an independent check by cham3. The `Release.gpg` for `repo.librewolf.net` was not fetched in this research (the upstream repository was not queried).
- LibreWolf's own key fingerprint should be cross-checked against the LibreWolf project (not done here; the LibreWolf page does not state it).

**Narrowly pinned source that would be proposed** (design only, NOT validated, NOT installed):

```
# /etc/apt/sources.list.d/cham3_librewolf.sources  (deb822, build-time only)
Types: deb
URIs: https://repo.librewolf.net
Suites: librewolf
Components: main
Architectures: amd64
Signed-By: /usr/share/keyrings/cham3_librewolf_archive_keyring.gpg
```

```
# /etc/apt/preferences.d/cham3_librewolf   (proposed)
Package: *
Pin: origin repo.librewolf.net
Pin-Priority: -1

Package: librewolf
Pin: origin repo.librewolf.net
Pin-Priority: 500
```

Points for review:

- Ship the key file inside the cham3 packaging with its fingerprint `662E3CDD6FE329002D0CA5BB40339DD82B12EF16` checked at build time. Do not run `extrepo update` at install time.
- Build-time-only extrepo use is a possible alternative. It would still need a fingerprint check.
- The `Pin-Priority: -1` default blocks every other package from this origin, so the repository only supplies `librewolf` and its explicitly listed dependencies. Dependency closure is not computed (the repository index was not downloaded).
- Upgrades would move `librewolf` only. Its update cadence and version-rollback behavior are untested.

**Decision state:** GATED. Owner approval of a third-party repository is required, then separate review. Not in the M1 build's dependency list.

### 3.2 Official Tor Browser path (INCLUDE-NOW candidate)

**Finding:** `torbrowser-launcher` 0.3.7-3 is in trixie `contrib` (`https://packages.debian.org/trixie/torbrowser-launcher`). This is the Debian-native authenticated path to the official bundle. No Debian package of Tor Browser itself exists in `IDX_TRIXIE_*`.

**Verification of the downloaded bundle** (from the upstream `security_design.md` in the Debian source package `torbrowser-launcher` 0.3.7-3, `https://sources.debian.org/src/torbrowser-launcher/0.3.7-3/security_design.md/`):

- Tor Browser bundles and updates are verified with OpenPGP signatures (`.asc`) against trusted keys the launcher ships with.
- The Tor Browser Developers signing key fingerprint is `EF6E286DDA85EA2A4BA7DE684E2C6E8793298290` (`https://support.torproject.org/tbb/how-to-verify-signature/`).
- The launcher uses HTTPS to `check.torproject.org` and `www.torproject.org` and pins a bundled certificate for those hosts. Mirrors rely on CA trust.
- The launcher refuses an update that claims a version older than the installed one (downgrade protection).
- Upstream's own trust model: an attacker holding the bundled keys and write access to Tor's files, or the TLS key, could run code as the user. The author presents this as intended.

**Privilege:** the launcher runs as the desktop user, installs to `~/.local/share/torbrowser`, and needs no root after installation. The Debian package does not grant privilege.

**Update path:** the browser updates itself through Tor's updater, outside apt. This is a deliberate design, not a Debian-managed update. The owner must accept it or require a pinned, apt-managed alternative (none exists in trixie).

**Recommends:** `tor`. Not a hard dependency.

**Decision state:** INCLUDE-NOW candidate. Requires `contrib` in the image archive areas and an owner acceptance of the self-update path.

### 3.3 Not-in-Debian tools: equivalents and Kali candidates

For each tool below, no Debian-native equivalent exists in trixie. Each is a Kali-origin candidate only. None is included. The Kali review requirements are in section 5 and in the readiness packet.

| Tool | Debian-native equivalent in trixie? | Kali candidate (version / area / index) | Status |
|---|---|---|---|
| ghidra | none | 12.1.4+ds-0kali1 (misc, main) | GATED-KALI |
| metasploit-framework | none | 6.5.3-0kali1 (net, main) | GATED-KALI |
| searchsploit / exploitdb | none | exploitdb 20260904-0kali1 (utils, all) | GATED-KALI |
| OWASP ZAP (zaproxy) | none | 2.17.0-0kali1 (utils, all) | GATED-KALI |
| Burp Suite Community | none | burpsuite 2026.8-0kali1 (net, main) | GATED-KALI (licence review blocking) |
| wpscan | none | 4.1.0-0kali1 (utils, non-free) | GATED-KALI |
| Cutter | none (rizin absent too) | rizin-cutter 2.5.0-0kali1 + rizin 0.9.1-0kali1 | GATED-KALI |
| radare2 | none in trixie (unstable only, not allowed) | 6.0.4+dfsg-1 (devel, main) | GATED-KALI |
| mitmproxy | none in trixie (testing/sid only, not allowed) | 12.2.3-0kali5 (net, all) | GATED-KALI |
| amass | none | 5.1.1-0kali3 (net, main) | GATED-KALI |

LibreWolf is absent from the Kali index as well, so Kali is not a fallback source for it.

**Kali candidates are not equivalent to Debian packages.** Kali's versions are newer than trixie's and built against Kali's own library set. Running them on trixie requires a dependency-closure check to rule out mixed-release library use. That check has not been done.

**Required Kali allowlist and pinning design** (proposal, not validated):

- **Source:** `Types: deb`, `URIs: http://http.kali.org/kali/`, `Suites: kali-rolling`, `Components: main non-free` (contrib only if needed), `Signed-By: /usr/share/keyrings/kali-archive-keyring.gpg` (the path the Kali apt documentation uses; the key package name was not verified). Kali's own docs describe this layout at `https://www.kali.org/docs/general-use/kali-apt-sources/`.
- **Authentication:** the Kali key fingerprint `827C 8569 F251 8CC6 77FE CA1A ED65 462E C8D5 E4C5` (verified in this research against `kali-rolling/Release.gpg`). How the target obtains this key must be reviewed: a Kali-packaged keyring obtained over HTTPS and verified once, or a key installed at build time by hash. Not decided.
- **Pinning:** deny Kali origin by default (`Pin-Priority: -1`, origin `Kali`), then allow only the allowlisted names at a chosen priority. Exact priorities are not validated.
- **Dependency closure:** for each allowlisted package, resolve the full Kali closure against trixie, list every Kali-origin dependency that would replace a trixie library, and review each.
- **Upgrade evidence:** demonstrate that `apt upgrade` on a trixie system does not move Debian packages to Kali versions, and that allowlisted Kali packages upgrade only within their pinned set. Not performed.
- **Licence:** per package (Burp EULA, wpscan non-free, Ghidra/Metasploit/ZAP/Amass upstream licences). Not checked.

### 3.4 Wireshark and dumpcap capture privilege

**Mechanism** (from the Debian source package `wireshark` 4.4.18-0+deb13u1, `debian/wireshark-common.postinst`, `https://sources.debian.org/src/wireshark/4.4.18-0+deb13u1/debian/wireshark-common.postinst/`, and `debian/README.Debian`):

- The postinst creates a system group named **`wireshark`** (`addgroup --quiet --system wireshark`).
- `dumpcap` is owned `root:wireshark`. Permissions are set to `u=rwx,g=rx,o=r` and then `setcap cap_net_raw,cap_net_admin=eip` is applied.
- Setuid is used only as a fallback: if `setcap` fails, the postinst applies `u=rwxs` (setuid). This branch is controlled by the debconf question `wireshark-common/install-setuid`.
- Re-run the choice with `sudo dpkg-reconfigure wireshark-common` and answer Yes to the question. The README states this is the supported way to change the mechanism.
- Adding a user is manual: `sudo usermod -a -G wireshark <username>`. The README states "no user will be added to group wireshark automatically." A new login session is needed for the group to take effect (standard Unix behavior, not separately verified).

**Privilege boundary:**

- Group members can run `dumpcap` with file capabilities `cap_net_raw` and `cap_net_admin`. They can capture on interfaces. They do not get a root shell.
- `cap_net_admin` is broader than capture alone. It is required for some capture modes (for example, promiscuous mode). Membership in `wireshark` should be treated as a capture authority, not a benign convenience.
- Setuid fallback would give members root-owned execution of `dumpcap` on the file system. Avoid this. The image should set `wireshark-common/install-setuid` to the non-setuid answer.

**Verification status:**

- **Confirmed from source:** group name, `setcap` call, and the owner/permission sequence.
- **NOT_RUN:** the default answer of `wireshark-common/install-setuid` (the template file fetch returned 404 and could not be read). The image must set it explicitly, and the disposable-VM check must confirm that `getcap /usr/bin/dumpcap` shows `cap_net_admin,cap_net_raw=eip` and that a non-root group member can capture.

**Group name decision:** the task text says "a dedicated `dumpcap` group." Debian's mechanism uses the group `wireshark`. This research recommends using Debian's `wireshark` group rather than inventing a second group, since the postinst is written around it. Owner decision.

## 4. Verification status summary

| Check | Result | Command / environment |
|---|---|---|
| trixie archive index presence for listed packages | PASS (index read) | `curl` of `IDX_TRIXIE_*` on 2026-10-11; decompressed and searched with awk |
| Kali archive index presence for candidates | PASS (index read) | `curl` of `IDX_KALI_*` (gz) on 2026-10-11 |
| trixie Release signature | NOT_RUN | Signer key `4CB50190207B4758A3F73A796ED0E7B82643E131` not retrieved |
| Kali Release signature | PASS | `gpg --verify kali-Release.gpg kali-Release` with key from `archive.kali.org/archive-key.asc` |
| LibreWolf key fingerprint and sha256 | PASS (from extrepo data) | `ar`/`tar` unpack of `extrepo-offline-data_1.0.6~deb13u1`; `gpg --show-keys`; `shasum -a 256` |
| LibreWolf repository reachability and signature | NOT_RUN | Upstream repo not queried |
| Tor Browser bundle signature | NOT_RUN | Not downloaded (no bundle fetched) |
| Dumpcap group and setcap | PASS (source) / NOT_RUN (runtime) | Read from Debian source; not run in a VM |
| Any package installed or executed | NOT_RUN | Out of scope for this task |
| Dependency closure of any Kali candidate | NOT_RUN | Not computed |
| Licences of each candidate | NOT_RUN (except as noted) | Not checked |

## 5. Unresolved decisions

1. Owner: approve or reject the third-party `librewolf` repository via extrepo (section 3.1).
2. Owner: accept torbrowser-launcher's self-update path for M1 (section 3.2).
3. Owner: decide whether the clearnet privacy browser role falls back to Debian `firefox-esr` while LibreWolf is gated (P3).
4. Owner: choose ZAP or Burp Suite, or neither. Burp licence review is blocking if chosen.
5. Owner: decide whether to accept the Debian `wireshark` group name for capture authority (section 3.4).
6. Owner: decide whether M1 ships without the GATED tools (recommended by the task's gating rule).
7. Owner: confirm contract renames (`plasma-workspace` for `plasma-workspace-wayland`; `kde-spectacle` for `spectacle`; `qml-module-org-kde-qqc2desktopstyle` for `qqc2-desktop-style`).
8. Owner: confirm `zenmap` (GTK) is acceptable on the KDE desktop, or specify a KDE alternative (I2).
9. Owner: decide on radare2 vs Cutter (both Kali-only in this research).
10. Reviewer: verify trixie Release signature in the build environment (NOT_RUN above).
11. Reviewer: confirm `wireshark-common/install-setuid` default and the post-install `getcap` result in a disposable VM.
12. Reviewer: all Kali candidates require the section 3.3 review, including dependency closure and licences.
13. Confirm `searchsploit` and `exiftool` binary file paths against the built package contents (not verified).
