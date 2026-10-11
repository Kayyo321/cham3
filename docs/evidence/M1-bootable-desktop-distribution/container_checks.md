# M1 fast-gate verification (container), integrated candidate

- **Commit:** `b82919c6ddbdb1f84f8db35e3ded8559c40b242f` on `task/M1-bootable-desktop`.
- **Environment:** a disposable, unprivileged `debian:trixie` `linux/amd64` container
  (Docker Desktop with x86-64 emulation on a macOS arm64 host, Apple M2 Pro). The
  repository was a fresh `git clone` of a bundle of the exact commit. Every check
  ran as the non-root user `builder` (uid 1000). This is emulated amd64, not native
  hardware, and exercises only packaging/static behavior — no cham3 security property.
- **Tooling:** host packages from `build/host_packages.txt` plus `desktop-file-utils`
  (shellcheck 0.10.0, lintian 2.122.0, live-build 1:20250505+deb13u1, dpkg-dev
  1.22.22, debhelper 13.24.2, python3 3.13).

| Check | Command | Result | Evidence |
|---|---|---|---|
| Static checks | `build/scripts/run_static_checks.sh` | PASS | shellcheck / py_compile / yamllint / scan_secrets all PASS (incl. the new `build/live-desktop` shell and `m1_checks.yml`) |
| M0 static tests | `tests/run_tests.sh static` | PASS | 82 tests, 0 failures (includes the hygiene exemption for `build/live-desktop` and the generalized built-deb check) |
| Package build + lintian | `build/scripts/build_packages.sh` | PASS | builds `cham3-base_0.0.0~m0_all.deb`, `cham3-desktop_0.0.0~m1_all.deb`, `cham3-full_0.0.0~m1_all.deb`; lintian exit status 0, no tags |
| M0 package tests | `tests/run_tests.sh packages` | PASS | 8 tests, 0 failures |
| M1 desktop tests | `tests/run_tests.sh desktop` | PASS | 122 tests, 0 failures — package metadata, curated-suite presence, duplicate + GATED exclusion, `.desktop`/launcher rules, panel geometry (right edge, height 60 in [56,64]) and nine-component order, plasmoid metadata/ids, honest inert-state wording, naming convention, and the build-script extension/profile behavior |

All five gates PASS. These are static/container checks only. The real desktop
ISO build, graphical Plasma 6 Wayland boot, disk install and installed-system
boot are NOT run here (they need a privileged x86-64 build and a graphical VM);
they are assigned to Codex Remote per `codex_remote_verification.md`, and are
also partly covered by the `m1_checks.yml` CI workflow on push (fast jobs only).
