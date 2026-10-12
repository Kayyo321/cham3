# Handoff: M1 — Bootable desktop distribution (repair cycle)

- **State:** IN_PROGRESS (repair cycle). Repaired candidate for independent Codex **re-review** after CHANGES_REQUESTED. Not approved; not merged; M1 is not complete.
- **Role:** Claude Code (Opus 5.5) as COORDINATOR / INTEGRATOR and implementation owner, with three subagents: `m1_worker` (Sonnet 5.5, IMPLEMENTER), `m1_test_reviewer` (Haiku 5.5, TEST / internal REVIEWER), `m1_docs_research` (Haiku 5.5, docs/research).
- **Model/provider:** Claude Opus 5.5 / Anthropic. Independent reviewer: Codex / OpenAI.
- **Working directory / branch:** `task/M1-bootable-desktop` in `/Users/jessicabruce/cham3-worktrees/M1-integration` (integration). Contributor worktrees: `task/M1-worker`, `test/M1`, `docs/M1-research`.
- **Task specification:** `docs/tasks/M1-bootable-desktop-distribution.md`.
- **Baseline commit:** `401c6cf9a550c81decf832cbc346f6908b9c95ef` (`main`; includes approved ADR 0002 and branding assets).
- **Prior reviewed commit:** `b82919c6ddbdb1f84f8db35e3ded8559c40b242f` — Codex verdict **CHANGES_REQUESTED** (`docs/reviews/M1-bootable-desktop-distribution-codex.md`).
- **Repaired implementation target (resubmit):** recorded at the bottom of this handoff and in `docs/STATUS.md` once the candidate is pushed.

Product authority: `PROJECT.md` §§8, 12, 13, 18; approved ADR 0002; SEC-01–18. ADR 0001 remains PROPOSED (ECR Explorer reserved, shown unavailable).

## Finding dispositions (Codex review of `b82919c`)

| # | Codex finding | Disposition | Key changes |
|---|---|---|---|
| F1 | Default M0 image build broken by the new package set | FIXED | `build/scripts/build_image.sh` stages only the locally built packages named in `${CHAM3_LIVE_DIR}/cham3-local-packages` (M0 → `cham3-base`; M1 → `cham3-desktop`, `cham3-full`), matched by each deb's real `Package` field, fail-closed, with a `CHAM3_STAGE_DRYRUN` hook. Regression suite (`staging`, 16) + CI gate. Real M0 build+boot → Codex Remote §2 |
| F2 | Full metapackage omits required application roles | RESOLVED per **owner scope amendment** (2026-10-11) | Six roles added via narrowly pinned authenticated sources: LibreWolf (own repo) + `zaproxy`, `exploitdb`, `ghidra`, `amass`, `radare2` from a Kali source at Pin-Priority `-1` with an 8-package allowlist; keys trusted only via `Signed-By` (0 in `trusted.gpg.d`, verified in a chroot run); strict pin admits no Debian base package. `mitmproxy`, `wpscan`, `metasploit-framework` deferred to a separate reviewed task (base-library skew). Matrix A8/A9/A10 corrected. Native closure/origin → Codex Remote §7 |
| F3 | Advertised non-setuid capture silently allowed a setuid-root fallback | FIXED | `cham3-desktop` pins a `dpkg-statoverride` (root root 0750) on `/usr/bin/dumpcap` so wireshark-common never applies its setuid fallback (across install/upgrade/`dpkg-reconfigure`), and a postinst + file trigger set group+capabilities or fail closed (no caps, no setuid — never root). Preseed keeps `install-setuid true`; comment corrected. Failure-path + member/nonmember tests; privileged capture → Codex Remote §6 |
| F4 | Image gate expected two files not shipped | FIXED | `tests/desktop/test_m1_image_contents.py` now checks `usr/share/wayland-sessions/plasma.desktop` and `usr/share/wallpapers/Cham3/contents/images/3840x2160.png` (derived from packaging), negative controls retained |
| F5 | Launcher could not present the curated suite under categories | FIXED | `org.cham3.user_launcher` uses the maintained KDE kicker model (`org.kde.plasma.private.kicker` RootModel + search) for categorized installed apps; no hardcoded list; contract badges remain M5. plasmoidviewer confirmed categories + a real launch. Live/installed → Codex Remote §4 |
| — | Firmware/proxy build blocker | FIXED | `--firmware-chroot false` + an explicit 42-package self-contained firmware list; no build-time fetch of Broadcom blobs. Legacy b43/b43legacy SoftMAC Wi-Fi dropped by recorded owner decision; `brcm80211` and all other firmware kept |

## Agent assignments

| Agent | Model / effort | Owned paths |
|---|---|---|
| coordinator (this) | Opus 5.5 | `build/scripts/*`, `build/live/cham3-local-packages`, `build/host_packages.txt`, `docs/`, `.github/workflows/m1_checks.yml`, M0-test reconciliation, integration merges |
| `m1_worker` | Sonnet 5.5 / high | `packaging/cham3-desktop`, `packaging/cham3-full`, `desktop/`, `build/live-desktop/` |
| `m1_test_reviewer` | Haiku 5.5 / high | `tests/desktop/`, `tests/container/`, `tests/lib/*`, `tests/run_tests.sh` |
| `m1_docs_research` | Haiku 5.5 / medium | `docs/research/` |

Persistent definitions: `.claude/agents/m1_*.md`. Internal review prepared the candidate; it does not replace independent approval.

## Verification

| Check | Environment / command | Result | Evidence / limits |
|---|---|---|---|
| Static checks | fresh clone of the integrated tree, amd64 `debian:trixie`, uid 1000: `build/scripts/run_static_checks.sh` + `tests/run_tests.sh static` | PASS | shellcheck (`-x`) / py_compile / yamllint / scan_secrets; 82 M0 static tests |
| M1 desktop suite | same: `tests/run_tests.sh desktop` | PASS | 224 tests (incl. foreign-source narrow-pin + key policy, categorized launcher, dumpcap static, firmware list, F4 packaged paths) |
| F1 staging suite | same: `tests/run_tests.sh staging` | PASS | 16 tests (per-profile selection: M0→cham3-base only, M1→desktop+full; fail-closed cases; real-build guards) |
| Package build + lintian | implementation branch, amd64 container: `build/scripts/build_packages.sh` | PASS | cham3-base + cham3-desktop + cham3-full; lintian exit 0, no tags. Re-run natively in CI on push |
| Foreign-source resolution | worker, build-phase chroot: `apt-get -s install cham3-full` + `--print-uris` | PASS | 1133 pkgs; only the 8 allowlisted Kali + `librewolf` are non-Debian; keyrings land only at `/usr/share/keyrings` (0 in `trusted.gpg.d`); GOODSIG for Kali + LibreWolf in a real `lb chroot_archives` run |
| dumpcap no-setuid | worker, dpkg harness + real wireshark-common 4.4.18/4.4.19 | PASS | no setuid on install/upgrade/reconfigure; fail-closed when setcap fails; group restriction. Live member/nonmember capture → Codex Remote §6 |
| Plasmoid QML load | worker: `plasmoidviewer` under Xvfb, Plasma 6.3.6 | PASS (load) | categorized launcher enumerates installed apps; search "wire" → Wireshark launches. Rendered panel → Codex Remote |
| Real M0 default build + boot (F1 regression) | native x86-64 | NOT_RUN → Codex Remote §2 | default build must stage `cham3-base` only |
| Real M1 ISO build (A2) + `image-m1` | native x86-64, privileged | NOT_RUN → Codex Remote §3 | keyring built at `lb config`; apt indices retained for origin audit |
| Graphical boot / install / launcher / capture (A3–A7, A11) | QEMU + VNC | NOT_RUN → Codex Remote §4–§6 | screendump recipe provided |
| F2 native closure / origin / trust (A9/A10) | native solver | NOT_RUN → Codex Remote §7 | `docs/research/M1_readiness_packet.md` §6 scripts |

Environment note: the coordinator host is macOS arm64 (Apple M2 Pro); fast gates ran in an amd64 container and (for the Python suites) native arm64. Per owner instruction, the long emulated ISO/boot/install runs were NOT performed locally and are assigned to Codex Remote (native x86-64); `m1_checks.yml` also exercises the fast jobs on native x86-64 on push.

## Review
- **Reviewer and prior commit:** Codex returned CHANGES_REQUESTED on `b82919c`; re-review of the repaired target is pending.
- **Internal review (not approval):** `m1_test_reviewer` probed the real wireshark-common postinst and established that a trigger-only dumpcap design is insufficient (reconfigure gap), which drove the statoverride design; it also built the foreign-source narrow-pin + key policy tests with negative controls. The author does not approve its own work.

## Owner scope amendment (2026-10-11)
Recorded in `docs/tasks/M1-bootable-desktop-distribution.md`: include six foreign roles via narrowly pinned, authenticated, separately-reviewed sources; defer `mitmproxy`, `wpscan`, `metasploit-framework`; build firmware without the fetch-at-install b43 installers (dropping legacy b43 SoftMAC Wi-Fi). Inclusion of the six is contingent on native closure confirmation on Codex Remote and independent Codex security review before merge.

## Remaining
- **Known limitations / risks:** Heavy + native gates NOT_RUN locally (assigned to Codex Remote). The three deferred tools are absent pending a separate reviewed source task. The Kali archive key expires 2028-04-17 (fail-closed). LibreWolf has no independent second trust channel (NOT_RUN). Package maintainer/copyright/license placeholders remain owner decisions before publication. `--firmware-binary true` interaction with the b43 installers in the installer payload is NOT_RUN. Honest-state checks are text heuristics; rendered-UI confirmation is a Codex Remote item.
- **Required next action and owner:**
  1. **Codex (REVIEWER/SECURITY):** independently re-review the repaired target (F1–F5 dispositions + the F2 pinned-source security design) and run the Codex Remote heavy/native gates per `codex_remote_verification.md`; record the verdict with `docs/templates/REVIEW.md`.
  2. On APPROVE + gate evidence: integrator merges through the owner-approved process; post-merge checks recorded; STATUS updated; M1 advances only then.
  3. On CHANGES_REQUESTED/BLOCKED: coordinator returns the task to the same three subagents against the exact reviewed commit.

## Repaired implementation target
- **Target (resubmit to Codex):** `0fec3a01c18b95fc238d2168ed0d97e0d3831865` on `origin/task/M1-bootable-desktop`. This commit is the complete repaired implementation (F1–F5 + firmware + all tests and docs; fast gates PASS on a fresh clone). The branch HEAD is this target plus a single documentation-only commit that records this target line (analogous to the prior cycle's docs tip on `b82919c`).
- **Baseline:** `401c6cf9a550c81decf832cbc346f6908b9c95ef`. **Supersedes** prior candidate `b82919c`.
