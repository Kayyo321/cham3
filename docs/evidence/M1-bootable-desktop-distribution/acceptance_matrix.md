# M1 acceptance-to-evidence matrix (repair cycle 2)

Task: `docs/tasks/M1-bootable-desktop-distribution.md`. Baseline `401c6cf`.
Implementation target `01bf7a1f1066fae15ab51a56a5212c0a69780ab8`. This matrix holds
only evidence actually produced. Statuses are PASS, FAIL, NOT_RUN or BLOCKED. A
build/boot/VM result is never described as proof of M2–M4 security enforcement. All
evidence below was produced by the implementation owner and awaits independent Codex
review (A15).

Evidence stages: **static** (repository content) · **native** (x86-64 Debian 13
containers on GitHub-hosted runners; real archives) · **image** (the built ISO) ·
**VM** (QEMU/KVM: live session, installation, installed system). Native evidence for
the target: [native_01bf7a1/](native_01bf7a1/README.md) (runs 38207816363,
38207816376, 38207816428). Environment and re-run recipe:
[codex_remote_verification.md](codex_remote_verification.md).

| # | Acceptance criterion (abridged) | Stage / method | Status | Evidence |
|---|---|---|---|---|
| A1 | Dedicated task branch on the recorded baseline; exact branch and target recorded | static | PASS | `task/M1-bootable-desktop` from `401c6cf`; target recorded in the handoff |
| A2 | Documented Debian 13 amd64 live-build path produces a bootable, installable ISO from declared inputs; no production credentials or undocumented host state | native + image | PASS | `ci_m1_heavy.sh iso` in a privileged `debian:trixie` container: ISO `f25bf75e…fd16`, manifest, provenance and checksums; image-m1 54/54; no firmware fetch in the build log; inputs: `build/live-desktop`, built packages, official Debian mirrors and the two pinned sources |
| A3 | In a disposable VM the ISO boots into Plasma 6 Wayland and completes the documented install path | VM | PASS | live: autologin session `Type=wayland`, `kwin_wayland` (37/37); install: the ISO's own debian-installer completed and powered off (330 s); installed: SDDM login → `Type=wayland` (65/65) |
| A4 | Installed desktop uses cham3 branding defaults; 56–64 logical-px vertical panel on the right edge | VM | PASS | running shell (plasmashell `evaluateScript`): location right, vertical, thickness 60; wallpaper `Cham3`; `LookAndFeelPackage=org.cham3.desktop`, `ColorScheme=Cham3Dark`; screenshots `installed_02_desktop.png`, `live_02_desktop.png` |
| A5 | Panel contains the nine components in ADR 0002 order/presentation (M1 = visual/package foundation) | static + VM | PASS | live and installed panel widgets in order: the eight `org.cham3.*` components, system tray, digital clock |
| A6 | M2–M4-dependent functions visibly report unavailable/disconnected/unverified; no implied active contracts/aliases/modes/kill/identity | static + VM | PASS | honest-state static suite; on screen: panel captions read as unavailable, and alias wheel, tag pager, contract dock, network protocol launcher and kill switch each show an unavailable state when opened (clean baseline), launcher `key_id: unavailable`, launches "not contract-verified: unverified" (screenshots `*_widget_*.png`, `*_launcher_categories.png`) |
| A7 | Desired/signed/pending/enforced distinction preserved; absent/stale/failed backend → visible unavailable, no false success | static + VM | PASS | inert-state static suite; no backend exists, and every backend-dependent widget renders unavailable in the live and installed sessions; no widget claims an active state (OCR text in `vm/*/ocr_*.txt`) |
| A8 | `cham3-desktop`/`cham3-full` valid metadata and dependency relationships; curated suite in default `cham3-full` | native + image | PASS | package build, lintian exit 0, package tests 8; F2 closure installs all six foreign roles; image manifest required/forbidden checks; the suite is installed in the live and installed systems. Deferred by owner amendment: mitmproxy, wpscan, metasploit-framework |
| A9 | Debian 13 preferred; Debian-native equivalents where unavailable; Kali only via reviewed narrow pinning; substitutions documented; no unreviewed foreign repository | static + native + image | PASS | host-bound pins (F6) with the 8-name Kali and `librewolf` allowlists; F2 `policy` (0 escaped of 9 945), `closure_uri` (8 Kali downloads = allowlist), retained-index origin audit in the image; no other foreign source (static and image policy) |
| A10 | Source configuration authenticates origin/signatures; Kali limited to the allowlist and its closure; update behaviour documented without silent origin change | native + VM | PASS | F2 `signatures`, `upgrade`; F6 synthetic signed-identity controls; installed system: authenticated `apt-get update`, upgrade origin by download host clean, unlisted Kali package not installable |
| A11 | Installed `.desktop` entries in correct categories, pass validation; launches resolve to installed applications | static + VM | PASS | desktop-file suite; launcher shows seven menu categories; search for Wireshark and Enter started a real `wireshark` process in the live and installed sessions |
| A12 | Debian KDE utilities present; named redundant alternatives absent (including transitive) | static + image | PASS | relation tests; image manifest has the KDE core utilities and none of the excluded duplicates, gated tools or desktop conflicts |
| A13 | Evidence recorded with exact commands/environment and PASS/FAIL/NOT_RUN/BLOCKED; build/boot not called M2–M4 proof | all | PASS | `native_01bf7a1/` (commands, environment, run URLs, artifact hashes, `SHA256SUMS`), this matrix, the handoff |
| A14 | `docs/STATUS.md` updated only with evidence-backed results | n/a | PASS | STATUS records the target and these results as owner-produced evidence pending review |
| A15 | Independent reviewer examines the exact target; security review required before READY_TO_MERGE/DONE | review | NOT_RUN (re-review pending) | Codex returned CHANGES_REQUESTED on `b82919c` and `0fec3a0`; re-review of `01bf7a1` pending. F2 and F3 security approval is Codex's |

## Finding dispositions (Codex re-review of `0fec3a0`)

| Finding | Disposition | Native evidence at the target |
|---|---|---|
| F6 — foreign default-deny fails open with changed signed Release identity | FIXED: pins bound to the configured source host | `apt` suite 22/22 (changed Origin/Label/Suite/Codename and removed identity still deny; superseded pins fail open as control); F2 7/7 on real archives; image and VM apt policy |
| F3 — first-install setuid gap; override failure only warned | FIXED: preinst registers the override at unpack and fails if it cannot; postinst fails instead of warning; image preseed `install-setuid false` | privileged suite 77/77, 0 skipped (both unpack orders with interrupted runs, apt order, preseed-false order, found setuid, preinst/postinst override failures, set-id override, lifecycle, real version change); image squashfs state; installed lifecycle with fail-closed reinstall and recovery |
| F7 — installer payload selects the b43 installers | FIXED: contrib out of the lb archive areas, supplied by a signed source file | image-m1 payload audit: no b43 installer in the ISO pool, `/firmware` or `Contents-firmware`; all 42 reviewed firmware packages present and linked; installed and live systems have no b43 installer |
| F8 — invalid privileged capture controls | FIXED: execution after credential drop, member-owned output, no skips in privileged mode | privileged suite 0 skipped; VM member capture (436-byte pcapng) and nonmember exit 126 in live and installed systems |
| F1, F4, F5 | Preserved | M0 build/boot regression PASS; image paths PASS; live/installed categories, search and real launch PASS |

Defects the native gates found and this cycle repaired (detail in the handoff):
firmware licence answers for `firmware-ipw2x00`/`firmware-ivtv` (owner decision
2026-10-12), Kali index confusing live-build's installer stage, the installer stage's
`trusted=yes` live-medium entry, the missing live account (`user-setup`), live
packages kept on installed systems, and installed systems without Debian sources
(`apt-mirror-setup`).

## Notes

- The six foreign roles come only from the two narrowly pinned, authenticated
  sources (owner scope amendment, 2026-10-11). The deferred roles (mitmproxy, wpscan,
  metasploit-framework) and the omitted alternatives (Cutter/rizin, Burp) are
  recorded scope decisions.
- `dumpcap` capture privilege is the Debian `wireshark` group plus file capabilities
  with no set-user-id fallback on any tested path; the image adds no group members.
- VM evidence covers q35/BIOS/virtio-vga in QEMU with KVM, not real hardware or UEFI.
