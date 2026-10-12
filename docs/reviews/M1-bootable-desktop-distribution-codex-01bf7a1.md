# Independent Review: M1 — Bootable desktop distribution, 01bf7a1

- **Reviewer / role / provider:** Codex / REVIEWER and SECURITY REVIEWER / OpenAI. Independent of Claude Code, who remains implementation owner.
- **Task specification:** `docs/tasks/M1-bootable-desktop-distribution.md`, including the recorded sourcing/deferral and development-image firmware licence decisions; `PROJECT.md` §§8, 12, 13, 15, 16 Phase 1, 18; approved ADR 0002; security invariants SEC-11, SEC-16–18. ADR 0001 remains proposed.
- **Base commit:** `401c6cf9a550c81decf832cbc346f6908b9c95ef`.
- **Reviewed commit:** `01bf7a1f1066fae15ab51a56a5212c0a69780ab8`. Evidence revision `058f839` adds only `docs/**`; `git diff --quiet 01bf7a1 058f839 -- ':!docs'` passed. Supersedes the review of `0fec3a0`.
- **Worktree / environment:** `/workspace/cham3-review-m1`, `review/M1-codex-01bf7a1`; prepared native amd64 Debian 13 container for local checks; GitHub-hosted disposable runners with Debian 13 containers for independent native/privileged/heavy reruns. No implementation edits, merge, or production credentials.
- **Review scope:** Baseline-to-target distribution changes, especially F6 host-bound APT pins, F3 dumpcap lifecycle, F7 installer firmware payload, F8 capture controls, and the native-gate repairs. Inspected source and failure paths, not only the handoff. Read the acceptance matrix, verification recipe, native evidence README, workflows, test scripts, manifests, provenance, serial records, result JSON, and representative screenshots.

## Findings

1. None — no confirmed blocking implementation defect found in the reviewed scope. F6, F3, F7 and F8 are closed; the native-gate repairs passed independent reruns.

### Previous findings

| Finding | Assessment and evidence |
|---|---|
| F6 — default deny depends on signed Release identity | Repaired. Both deny and allow stanzas use `Pin: origin` with the configured source hostname. Static/image checks reject a source moved to a different hostname, additional allowances, or identity-dependent pins. Independently ran all 22 real-APT signed synthetic tests: changed/removed identity still denies, the allowed candidate proves the index loaded, the superseded rules expose the defect, and a foreign signing key is rejected. Real-archive F2 rerun also passed. |
| F3 — first-install setuid gap and warning-only override failure | Repaired for the approved image/package lifecycle. Preinst installs the `root root 750` override before configuration after unpack, clears a found set-id bit, and refuses failed registration or a set-id override. Postinst clears set-id bits before subsequent work and fails configuration if protection is absent. Image preseed is false, covering upstream configuration before cham3 unpack. The override persists on remove/abort-install and is removed only on purge when it matches cham3's entry. Capture requires deliberate group membership and restricted file capabilities; setcap failure never introduces a root fallback. Independent privileged rerun passed 77 tests without skips, including both unpack orders, interrupted runs, apt ordering, failed override registration, upstream setuid sensitivity, actual version change, member packet capture/nonmember execution denial, capability failure and recovery. The dependency-free stub exercises shipped control scripts with real dpkg/wireshark-common, not the KDE payload. |
| F7 — b43 installers remain in the installer payload | Repaired. `contrib` is outside live-build archive areas; authenticated persistent Debian contrib sources supply torbrowser-launcher. Explicit live firmware and the installer's selector therefore exclude both fetch-at-install b43 packages. The independently rebuilt ISO passed all 54 image-m1 checks. Its payload gate inspects the actual ISO pool, firmware links, Contents-firmware and root manifest, requires all 42 reviewed packages, and has defect controls. |
| F8 — capture harness has invalid controls | Repaired. setpriv starts a shell under dropped credentials before that shell executes dumpcap; output lives in a member-owned directory. Output-path errors fail the harness. Privileged mode fails when capture prerequisites are absent, and the completed independent rerun executed capture checks without skips. The guest probe uses the same credential-drop boundary. |

The earlier F1/F4/F5 dispositions remain reasonable: local per-profile staging passed independently; packaged session/wallpaper paths match image tests; launcher categories/search use the upstream application model and ordinary launches explicitly remain unverified.

### Native-gate repairs

- Firmware licence answers match the recorded owner decision for the development image. They do not silently restore either b43 installer.
- `auto/build` runs live-build stages explicitly, stashes foreign sources and preferences only during the installer stage, and restores them on failure before binary creation. This keeps the installer's alternative-package selection Debian-only while preserving the final live root's reviewed sources/pins/indices.
- The unused unsigned live-medium source and its indices are removed before the binary stage. Image tests check final source configuration and permit only Debian archive key files owned by `debian-archive-keyring` in global trust; foreign keys remain in dedicated Signed-By keyrings.
- `user-setup` and the live packages are in the live-only pass. The ISO's own d-i installer uses an authenticated Debian `apt-mirror-setup` udeb supplied by the staged build. Recorded installed-system evidence shows Debian main/security/updates sources, live packages removed, authenticated update, and the persistent narrow foreign policy.
- Inert desktop behavior is supported by source inspection and visual evidence: the kill switch states unavailable and makes no enforcement request; launcher identity and launch authority remain unavailable/unverified. VM checks do not demonstrate M2–M4 enforcement.

## Verification and limits

Evidence and exact commands: `docs/evidence/M1-bootable-desktop-distribution/codex_01bf7a1/README.md`. Original author-produced native evidence remains separate under `native_01bf7a1/`.

| Check | Independent result |
|---|---|
| Static lint/syntax/secret scan | PASS |
| Static / desktop / staging | PASS: 82 / 261 / 16; no skips |
| F6 real-APT signed synthetic suite | PASS: 22; no skips |
| Package build / lintian / package tests | PASS: lintian exit 0; 8 tests |
| Committed native evidence SHA256SUMS | PASS |
| Staged auto/build failure-path probe | PASS: six mocked stage/success cases; original script, temporary files, no root/network or real image proof |
| F2 real-archive source gates, Actions rerun | PASS: 7/7; 9,947 foreign-only packages, 0 escaped; closure exactly 8 Kali + 1 LibreWolf; 1,159 unlisted Kali downloads without pins; actual radare2/LibreWolf installation, clean upgrade plan |
| F3/F8 privileged suite, Actions rerun | PASS: 77, no skips; actual version change and capture controls ran |
| M1 ISO/image-m1, Actions rerun | PASS: independently rebuilt from 01bf7a1, 54 tests; payload, origin, final trust, capture configuration and build-log firmware-fetch gate |
| Live/install/installed VM rerun | PASS: 37 / 1 / 65 checks; installation powered off successfully in 320 seconds. The VM job downloaded the rebuilt ISO artifact 11748821992 and verified its ZIP digest, rather than using the original artifact. |
| M0 image/boot regression | Recorded PASS reviewed from run 38207816428; not independently rerun |

Local initial F6 execution was blocked by missing `gpg-agent` in the prepared toolchain, then passed after installing the declared gnupg dependency inside a disposable container. Local Kali/LibreWolf and GitHub API access is blocked by proxy policy (HTTP 403); connected GitHub Actions tools successfully reran gates in the declared environment without changing or bypassing that policy.

Limits: BIOS/q35 QEMU evidence, not UEFI or physical hardware compatibility. No M2–M4 authorization/isolation/network guarantees. APT origin controls constrain the reviewed repositories and allowances, not the safety of arbitrary third-party application code. Real-archive metadata rolls between runs; no reproducible ISO claim. Key rotation, public-release licence/publication readiness and the three deferred tools remain separate work. Installed-system upgrade at the original snapshot had no newer wireshark-common version; real version change ran in the privileged suite. The installed phase does not independently record `iso_sha256` despite the native README's broad wording: its identity is traced through the install phase and shared disk in the same job, whose live/install hashes and workflow ISO checksum agree.

## Verdict

- **APPROVE** `01bf7a1f1066fae15ab51a56a5212c0a69780ab8` for the approved M1 development scope. This supersedes CHANGES_REQUESTED on `0fec3a0`; F6, F3, F7 and F8 are closed.
- **F2 security approval:** GRANTED for the reviewed narrow authenticated sources, host-bound pinning, dependency closure and upgrade policy at `01bf7a1`, within the approved M1 development scope.
- **F3 security approval:** GRANTED for the reviewed dumpcap policy and tested lifecycle at `01bf7a1`.
- **Required actions:** None for this re-review. Claude Code retains implementation ownership and the coordinator/integrator owns any subsequent task/status updates and integration. No merge performed or authorized by this review assignment; public release and future security milestones remain outside this approval.
