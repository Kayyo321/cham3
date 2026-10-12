# Independent Review: M1 — Bootable desktop distribution

- **Reviewer / role / provider:** Codex / independent REVIEWER and SECURITY REVIEWER / OpenAI.
- **Task specification:** `docs/tasks/M1-bootable-desktop-distribution.md`; implementation review, including package readiness and security.
- **Base commit:** `401c6cf9a550c81decf832cbc346f6908b9c95ef`.
- **Reviewed commit:** `b82919c6ddbdb1f84f8db35e3ded8559c40b242f`.
- **Supplemental evidence revision:** `848b72ac16fe656fe3c6730b42f1482fdd3ca95c`. Its diff from the reviewed target contains documentation only; handoff, STATUS, container record and revised acceptance matrix were read as supplemental evidence, not substituted for the implementation target.
- **Worktree / environment:** `/workspace/M1-review`, detached at the target during verification, then `review/M1-codex-b82919c` for review documents. Codex Remote `Kayyo321/cham3`, Debian 13 amd64, native x86-64. Disposable privileged Docker container `codex-m1-review`, image `cham3-cloud-dev:trixie` (`fa9efb35525a`); fast checks ran as uid 1000, live-build and chroot probes as root inside that container. A `/proc` mount probe passed. `/dev/kvm` was absent; TCG would be available, but no ISO was produced. Review date: 2026-10-11.
- **Review scope:** Actual baseline-to-target diff, surrounding M0 build/test code, package definitions and runtime dependencies, all eight QML plasmoids, branding/layout, source packet, failure paths, CI workflows/results, and all fifteen acceptance criteria. Read `AGENTS.md`, relevant `PROJECT.md` §§8/12/13/18, workflow/testing/security guidance, role/orchestration guidance, STATUS, task, ADR 0002 (**APPROVED**) and ADR 0001 (**PROPOSED**). Product implementation was not modified. Claude Code retains implementation ownership; Codex retains independent review ownership.

## Findings

### F1 — High: the new package set breaks the default M0 image build

- **Location:** `build/scripts/build_image.sh:74` and `:99`, together with the new `packaging/cham3-full/debian/control:11` dependencies. The wildcard staging existed before M1 but becomes a regression when this change adds the two desktop packages.
- **Trigger:** Run the documented unparameterized M0 build after `build/scripts/build_packages.sh`, as `.github/workflows/m0_checks.yml` does.
- **Violated requirement:** Preserve the M0 build baseline and regressions; the implementation explicitly claims the default M0 path remains unchanged.
- **Confirmed evidence:** All three `.deb` files are copied to `config/packages.chroot`. Live-build selects local packages for installation even if only `cham3-base` is named in the M0 package list. M0 exposes `main`, while `cham3-full` requires `nikto` from non-free and `torbrowser-launcher` from contrib. The independent native build fails with `cham3-full : Depends: nikto but it is not installable` and the same error for `torbrowser-launcher`; `lb build` exits 123 and the wrapper exits 1. No M0 ISO is published. See `codex_b82919c/m0_image_attempt.log`.
- **Reproduction:** Build packages, then run `build/scripts/build_image.sh` as root in a disposable Debian 13 amd64 environment with no M1 selectors. The review used a separate output directory containing the same three built `.deb` files to prevent collisions with the M1 build.
- **Required correction:** Select and stage the appropriate local packages per image profile. Do not solve this by broadening M0's archive areas or admitting the desktop suite to the baseline image. Add a meaningful regression check and rerun the real M0 image/boot gates.
- **CI corroboration:** [m0_checks run 38181193243](https://github.com/Kayyo321/cham3/actions/runs/38181193243) at documentation tip `848b72a` failed in `Build live image` with exit 1. The public detailed-log endpoint returned 404, so its exact root cause was not inferred. The independent failure above establishes the defect. [m1_checks run 38181193291](https://github.com/Kayyo321/cham3/actions/runs/38181193291) succeeded.

### F2 — High: the full metapackage omits required M1 application roles

- **Location:** `packaging/cham3-full/debian/control:42–48`; `docs/research/M1_readiness_packet.md` §§2–3; `tests/lib/m1_support.py:80` and its required/gated sets.
- **Trigger:** Install the default `cham3-full` set, or build the default M1 image.
- **Violated requirement:** Task built-in suite, A8/A9, and ROADMAP M1 deliverables require the complete requested suite, with approved compatible packages or equivalents. Documenting an omission is necessary but does not approve a reduction in milestone scope.
- **Confirmed evidence:** There is no LibreWolf clearnet browser, mitmproxy/mitmweb, ZAP or Burp, Amass, WPScan, Metasploit, searchsploit, Ghidra, or radare2/Cutter dependency or selected equivalent. The package description acknowledges these gaps. Tests explicitly require absence of many requested tools, so their success cannot establish full-suite acceptance. The supplemental acceptance matrix marks A8 PASS although its full-suite part is unsatisfied.
- **Required correction:** Resolve each requested role through the approved sourcing sequence. Provide compatible, authenticated sources and complete closure/pinning/update evidence before adding foreign packages; alternatively obtain an explicit owner-approved scope amendment. Do not enable a general Kali repository or add unreviewed dependencies merely to clear this finding. Correct the acceptance matrix so unresolved suite coverage cannot pass.

### F3 — Medium, security: the advertised non-setuid capture configuration silently permits setuid-root fallback

- **Location:** `build/live-desktop/config/preseed/cham3_wireshark.cfg.chroot:6`; `docs/evidence/M1-bootable-desktop-distribution/codex_remote_verification.md` §5; `docs/research/M1_package_disposition.md` §3.4.
- **Trigger:** `setcap` is unavailable or fails during `wireshark-common` installation/reconfiguration, including a filesystem without working capability xattrs.
- **Violated requirement:** Task capture privilege boundary, no unnecessary root execution, and the candidate's explicit claim that the preseed selects a non-setuid configuration. Its source packet also explicitly says to avoid setuid fallback.
- **Confirmed evidence:** `install-setuid boolean true` enables Debian's non-root capture mechanism, including its fallback. The authenticated Debian `wireshark-common` 4.4.19 postinst executes `chmod u=rwxs,g=rx,o=r` when `setcap` fails or is absent. An independent disposable-file reproduction using that postinst with the same debconf answer returns success and mode `04754` on capability failure; its positive control produces `0754` and `cap_net_admin,cap_net_raw=eip`. See `capture_fallback.log`, the retained postinst, and `capture_fallback_probe.py`.
- **Boundary of the claim:** This is group-restricted setuid execution of dumpcap, not a demonstrated root-shell exploit. The partial M1 chroot's actual successful path was `root:wireshark`, `0754`, with the expected capabilities, and the group-member/nonmember probes passed. That does not validate the failure path or preservation through installation/upgrade.
- **Required correction:** Make capability setup fail closed without setuid fallback, verify no setuid bit, document intentional membership, and test missing/failing capability setup plus package reconfiguration/upgrade and installed-system behavior. Setting the debconf answer to false alone is insufficient: it also removes the desired non-root group capture mechanism.

### F4 — Medium: the required image-content gate expects two files the candidate does not ship

- **Location:** `tests/desktop/test_m1_image_contents.py:25` and `:28`.
- **Trigger:** Run `tests/run_tests.sh image-m1` against an otherwise correctly populated candidate ISO.
- **Violated requirement:** Accurate image/desktop verification and the required heavy-gate recipe; checks must exercise the actual accepted packaging interface.
- **Confirmed evidence:** The test expects `usr/share/wayland-sessions/plasmawayland.desktop`, but Debian Plasma 6.3.6 installs `plasma.desktop`, which the candidate's SDDM component also selects. It expects wallpaper `contents/images/cham3_minimal_charcoal.png`, but `packaging/cham3-desktop/debian/rules:22` installs `contents/images/3840x2160.png`. Both actual files were found in the downloaded/built packages and partial chroot. Running the existing `contents_problems()` on those real chroot paths reports precisely these two missing files. Fixtures repeat the wrong names, so fixture success misses the inconsistency. See `partial_chroot_contents.log`.
- **Required correction:** Align the gate with the actual Debian session and packaged wallpaper, retain meaningful missing-file negative controls, and run it against the real ISO. Do not change valid packaging names to satisfy a defective fixture.

### F5 — Medium: the default launcher cannot present the curated suite under menu categories

- **Location:** `desktop/plasma/plasmoids/org.cham3.user_launcher/contents/ui/main.qml:23–33` and its final label deferring categories to M5; `desktop/plasma/layout/cham3_panel_layout.js:25–36`.
- **Trigger:** A new user opens the default panel's application launcher to locate Wireshark, Zenmap, Tor Browser, or another installed suite application.
- **Violated requirement:** M1's built-in suite and A11 require categorized application-menu entries; this requirement is separate from M5's trusted badges and backend interactions.
- **Confirmed evidence:** The flyout contains only nine hardcoded KDE utilities, without categories or a path to the full application menu. The panel supplies no upstream application launcher. Upstream desktop entries can exist on disk yet remain inaccessible from this flyout. The implementation text explicitly defers categories to M5, although M1 requires them. Static desktop-file tests validate zero cham3-authored `.desktop` entries and do not establish curated-suite menu access. The partial chroot has valid upstream entries, but that does not add them to this launcher.
- **Required correction:** Provide ordinary categorized application-menu access to the installed suite through the default desktop, using maintained KDE facilities if appropriate. Keep future contract badges/security launch authority unavailable. Validate actual visible menu entries and launches in the live and installed desktop; add no fake entries for absent tools.

## Verification and limits

Commands and full independent transcripts are preserved in [codex_b82919c](../evidence/M1-bootable-desktop-distribution/codex_b82919c/README.md). Package checksums identify the unsigned development packages. There is **no M1 ISO checksum, graphical screenshot, installation log, or installed-system boot result** because the exact build did not finish.

| Independent check | Result | Evidence and scope |
|---|---|---|
| Exact target/base and documentation-only tip diff | PASS | Base is an ancestor; checkout was `b82919c`; supplemental diff changes five documentation files only. |
| Static runner; M0 static tests; M1 desktop tests | PASS | Four static checks; 82 M0 tests; 122 M1 tests. These establish static/package scaffolding only. |
| Three Debian package builds and lintian; package tests | PASS | Lintian exit 0, eight package tests. This is not dependency installation or full-suite coverage. |
| Native privileged mount/chroot prerequisites | PASS | Root `/proc` mount/unmount probe succeeded. Host is native amd64 Debian 13; no architecture override. |
| M1 real ISO build | BLOCKED (command failed) | Exact profile; debootstrap reports a valid Release signature; APT resolves and configures selected desktop dependencies. Broadcom installer postinsts attempt GitHub/OpenWrt downloads, fail DNS in the chroot, and leave dpkg failed. Wrapper exits 1 / `lb build` 123. Supported-proxy probe of OpenWrt returns CONNECT 403. No network-policy bypass, firmware omission, or implementation repair was attempted. |
| M0 real default image build | FAIL | Selected `cham3-full` cannot resolve contrib/non-free dependencies under M0 main-only sources (F1). |
| Required `image-m1` tests | FAIL due to missing build artifacts | 15 tests: 11 fixture tests pass, four artifact-dependent tests fail. Independently established F4 is separate from artifact absence. |
| Selected dependencies/duplicate exclusion in partial dpkg status | PASS, partial only | 1,323 packages marked installed; existing manifest checker reports no missing selected dependencies or named excluded utilities. Full requested suite remains incomplete (F2). No ISO manifest/transitive-image result. |
| Desktop-file syntax and the nine fixed launcher IDs in partial chroot | PASS, partial only | 113 upstream entries validate; all nine referenced files exist. No live/installed application execution claim. |
| Capture privileges in partial chroot | PASS, partial only | `root:wireshark`, mode `0754`, capabilities; uid 1000/group 104 captured one loopback packet; nonmember execution denied (126). No automatic members. No installed-system or upgrade result. Failure-path test confirms F3. |
| Graphical Plasma 6 Wayland boot and rendered panel/branding/state | BLOCKED | No ISO; a serial marker or QML-load check cannot substitute. KVM absence would permit a recorded TCG attempt, not justify assuming verification impossible. |
| Disk installation, installed-system boot, launcher execution, installed dumpcap | BLOCKED | Depend on a successful exact-target ISO. |

Package/source readiness is **not approved for complete M1**. Ordinary selected Debian package origins are authenticated through the observed signed Release/APT path. The configured image has no enabled foreign/Kali source. The documented package-name replacements are reasonable Debian names; Zenmap is supplied by Debian. However, foreign candidates remain research only: no completed allowlist, authenticated-source configuration/key-provenance record, full closure, tested preferences, or upgrade/origin-preservation evidence. LibreWolf repository verification and required role substitutions remain unresolved. Signing a Kali Release in a research session is insufficient to approve mixed-distribution installation. The readiness packet's own open decisions prevent treating it as implementation readiness; keeping those sources disabled is appropriate until resolved.

Acceptance disposition: A1 PASS; A2/A3/A4 BLOCKED; A5/A6/A7 static foundation assessed, runtime BLOCKED; A8 FAIL for complete-suite coverage (metadata/build portion PASS); A9/A10 partial Debian-only sourcing evidence with unresolved readiness; A11 FAIL for menu coverage and runtime BLOCKED; A12 partial chroot PASS, final image BLOCKED; A13 evidence recorded with explicit limits; A14 supplemental STATUS correctly withholds completion, but A8 in its acceptance matrix needs correction; A15 this independent implementation/security review completed with CHANGES_REQUESTED.

Security inspection found no cham3 signing backend, privileged firewall operation, fabricated active contract, Tor-active claim, or simulated kill enforcement in the QML. Backend-dependent state is statically unavailable and explanatory popups do not activate authority. ADR 0001 is still proposed and the ECR reservation says unavailable. These are source-review conclusions, not rendered-UI or M2–M4 enforcement approval. Build safety/secret/static regression checks passed within their stated coverage; production signing credentials were not used.

## Verdict

**CHANGES_REQUESTED** for exact commit `b82919c6ddbdb1f84f8db35e3ded8559c40b242f`.

Required actions: repair F1–F5, resolve full-suite readiness without unreviewed sources or an implicit scope reduction, and provide all required successful ISO/graphical/install/installed-system evidence at a new exact implementation target. Recover the declared remote build's supported proxy/CA configuration and approved firmware-host access, or use another declared disposable environment; do not bypass the network policy or drop firmware silently. Rerun both M0 and M1 affected gates. No final milestone approval, merge approval, or readiness approval of an unimplemented milestone is granted.

### Ready-to-paste Claude Code repair prompt

```text
You are Claude Code, implementation owner for cham3 M1. Codex remains the independent REVIEWER and SECURITY REVIEWER. Repair and resubmit the findings in docs/reviews/M1-bootable-desktop-distribution-codex.md, reviewed implementation commit b82919c6ddbdb1f84f8db35e3ded8559c40b242f, baseline 401c6cf9a550c81decf832cbc346f6908b9c95ef. Verdict: CHANGES_REQUESTED. Supplemental documentation tip was 848b72ac16fe656fe3c6730b42f1482fdd3ca95c. Review transcripts are in docs/evidence/M1-bootable-desktop-distribution/codex_b82919c/.

Use your Opus coordinator and the same three subagents: m1_worker, m1_test_reviewer, and m1_docs_research, in separate owned worktrees. Read AGENTS.md, PROJECT.md, the task, approved ADR 0002, workflow/testing/security guidance and this review. ADR 0001 remains PROPOSED. Leave independent review ownership with Codex; internal tests/review cannot approve the repaired candidate.

Required corrections:
1. F1: select/stage local packages per image profile so unparameterized M0 builds only its intended baseline. All three current debs cause live-build to select cham3-full under main-only M0 sources, failing nikto/torbrowser-launcher dependencies. Do not broaden M0 or insert the desktop suite to mask the regression. Add a sensitive regression check.
2. F2: resolve all required application roles: LibreWolf, mitmproxy/mitmweb, ZAP or Burp, Amass, WPScan, Metasploit, searchsploit, Ghidra and radare2/Cutter, with reviewed compatible sources/equivalents or an explicit owner-approved scope amendment. Finish authenticated provenance, key/source configuration, allowlist, dependency closure, narrow pinning and update/origin-preservation tests before including foreign packages. No general Kali repo. Documented omissions alone do not satisfy A8; correct the matrix and readiness status.
3. F3: prevent Debian dumpcap's setuid-root fallback when capability setup is missing/fails; preserve intended group-restricted capability capture. Test both controls, no setuid bit, membership, package reconfiguration/upgrade and installed-system behavior. A false debconf answer alone disables the required capture mechanism. Correct misleading non-setuid wording.
4. F4: fix image tests to use the real Plasma session plasma.desktop and packaged wallpaper 3840x2160.png. Keep missing-file negative controls and run against the real ISO; do not rename valid interfaces to match wrong fixtures.
5. F5: expose the actual installed suite in a categorized default application menu. Ordinary menu access is M1; future trusted contract badges remain M5. Verify live and installed launcher execution and categories. Do not create fake launchers for absent tools.

Required evidence/checks to rerun at the new exact target:
- build/scripts/run_static_checks.sh; tests/run_tests.sh static; build/scripts/build_packages.sh (lintian exit 0); tests/run_tests.sh packages; tests/run_tests.sh desktop.
- Unparameterized real M0 ISO build, tests/run_tests.sh image, throwaway VM boot and tests/run_tests.sh boot; native m0_checks and m1_checks must succeed, with failing CI dispositions recorded.
- Real Debian 13 amd64 M1 ISO: CHAM3_LIVE_DIR=build/live-desktop CHAM3_IMAGE_NAME=cham3-m1-desktop-amd64 build/scripts/build_image.sh, then tests/run_tests.sh image-m1. Record inputs, complete manifest, provenance, checksums, source/signature/update evidence and transitive duplicate exclusions.
- Graphical Plasma 6 Wayland boot with screenshots, session/version evidence, branding and right-edge 56–64 logical-pixel panel in ADR order; inert/unavailable security state and ordinary KDE positive controls.
- Disk installation and installed-system boot without the ISO, screenshots, installed launchers/categories, dumpcap capabilities/group/no-setuid checks and member/nonmember capture controls. No serial-marker inference or presentation-as-enforcement claim.

Codex Remote's native privileged mount/chroot worked, but the exact M1 build failed in firmware-b43-installer and firmware-b43legacy-installer postinst downloads. Chroot wget reported DNS failure for GitHub/OpenWrt; the supported proxy explicitly denied downloads.openwrt.org (CONNECT 403). Configure the supported proxy/CA and approved network access or use another declared disposable build environment. Do not bypass policy, disable TLS verification, silently omit firmware, or count the failed build's partial chroot as ISO/install evidence. KVM was absent; record a TCG attempt if needed rather than claiming architecture makes verification impossible.

Update the task, handoff, STATUS and acceptance matrix with exact PASS/FAIL/NOT_RUN/BLOCKED evidence, your new immutable implementation target, and finding dispositions. Push the candidate and resubmit that exact target to Codex for independent implementation/security rereview. Do not merge or declare M1 complete before required evidence and independent approval.
```
