# Developer Setup: M0 Build Baseline

> **Note:** Commands in this document reflect the M0-001 interface. Verification evidence is recorded in the task handoff, not here. Do not treat any command in this document as verified until the handoff says so.

## Scope

This document covers the **M0 repository and build baseline only**.

The image produced by this baseline is a minimal Debian 13 live development image plus the `cham3-base` M0 skeleton package. It is **not** an installable or complete cham3 distribution. It contains no KDE desktop, no installer, no cham3 services, no enforcement, no sandboxing, no network modes, and no signed APT repository.

A successful build or VM boot demonstrates **no security property**. It shows only that packaging, live-build configuration, and the boot path run.

## Supported build host

For M0, the only documented and validated build host is:

- **Debian 13 (trixie), x86-64 (amd64)**, on a clean machine or disposable VM.

All other hosts are **unsupported and unverified** for M0. This includes other distributions, arm64 hosts, macOS, containers on other hosts, and x86-64 emulation. Do not assume a build that works elsewhere is representative.

This is a narrow development convention for the M0 task. It is not a product decision about cham3's runtime hardware or supported platforms.

## Warning: build runs as root

The build script requires root. The M0 interface states that `build/scripts/build_image.sh` refuses to run without root because live-build needs it. The live-build documentation I retrieved did not confirm this requirement, so treat it as an interface requirement rather than a sourced fact.

The build also mounts filesystems inside the build chroot. **Run the image build in a disposable Debian 13 amd64 VM, not on a workstation you care about.**

Host disk and RAM requirements are not established by a source here. Plan for ample free disk space in the output directory and the work directory (unverified figure).

## 1. Install build-host packages

The single source of truth for host packages is `build/host_packages.txt`:

```sh
sudo apt-get install --no-install-recommends $(grep -v '^#' build/host_packages.txt)
```

The list covers `git`, static-check tools, package build and lintian tools, live-build, image inspection (`xorriso`), Python 3 tests, and `qemu-system-x86` for the boot test.

Debian 13 trixie package versions for key tools, as shown on packages.debian.org on 2026-10-11 (the versions your host installs may differ):

| Package | trixie version shown |
|---|---|
| live-build | 1:20250505+deb13u1 |
| debootstrap | 1.0.141 |
| debhelper | 13.24.2 |
| dpkg-dev | 1.22.22 |
| lintian | 2.122.0 |
| xorriso | 1.5.6-1.2 (binary 1.5.6-1.2+b1) |
| squashfs-tools | 1:4.6.1-1 |
| qemu-system-x86 | 1:10.0.13+ds-0+deb13u1 |
| shellcheck | 0.10.0-1 |
| yamllint | 1.37.1-1 |

The version recorded in `provenance.json` for each build is authoritative for that build.

## 2. Clone the repository

```sh
git clone <repository-url> cham3
cd cham3
git log -1 --format='%H %ct'
```

Record the commit you build. The image build uses the commit time of `HEAD` as `SOURCE_DATE_EPOCH` unless you set it yourself.

## 3. Run static checks (no root, no network)

```sh
build/scripts/run_static_checks.sh
```

This runs shellcheck on tracked shell scripts (including `build/live/auto/*`), `python3 -m py_compile` on tracked Python files, yamllint on `.github/workflows/*.yml` using `.yamllint.yml`, and `build/scripts/scan_secrets.sh`. It prints a per-check summary and exits 0 only if every check passes.

To run the test suite for this stage:

```sh
tests/run_tests.sh static
```

## 4. Build the packages (no root)

```sh
build/scripts/build_packages.sh
```

This builds each package under `packaging/*/` with `dpkg-buildpackage -us -uc -b` in a temporary copy, so the source tree is not modified. Outputs go to `$CHAM3_OUT_DIR/packages/`:

- `.deb`, `.buildinfo`, `.changes` files
- `SHA256SUMS`
- `lintian.txt`

Lintian errors fail the build. The warning policy is defined by the script and its output; warnings are recorded in `lintian.txt`.

To run the test suite for this stage:

```sh
tests/run_tests.sh packages
```

The only M0 package is `cham3-base`. It installs documentation under `/usr/share/doc/cham3-base/` and nothing else. It has no maintainer scripts, conffiles, or systemd units. It is an M0 development skeleton that installs no services, security policy, enforcement, or system configuration.

## 5. Build the image (root required)

```sh
sudo build/scripts/build_image.sh
```

Run this in a disposable VM. The script:

- refuses to run without root;
- checks that the host is Debian 13 amd64 (`/etc/os-release` `VERSION_ID=13` and `dpkg --print-architecture` = `amd64`);
- allows an override with `CHAM3_ALLOW_UNVERIFIED_HOST=1`, which is recorded in provenance as an unverified host;
- refuses to run if `$CHAM3_OUT_DIR/packages/*.deb` are missing (run step 4 first);
- copies `build/live` to a work directory, copies the built `.deb` files into `config/packages.chroot/` there, and runs `lb clean --purge`, `lb config`, and `lb build` in the work directory only;
- exports `SOURCE_DATE_EPOCH` from `git log -1 --format=%ct` if it is not already set.

The build must run from a clean checkout or copy. Live-build does not work on a macOS bind mount, and the source tree is not written to.

Outputs:

- `$CHAM3_OUT_DIR/image/cham3-m0-dev-amd64.iso`
- `$CHAM3_OUT_DIR/image/cham3-m0-dev-amd64.packages` (live-build package manifest)
- `$CHAM3_OUT_DIR/image/build.log`
- `$CHAM3_OUT_DIR/image/provenance.json`
- `$CHAM3_OUT_DIR/image/SHA256SUMS`

Run the image test suite:

```sh
tests/run_tests.sh image
```

## 6. Boot the image in a disposable VM

```sh
build/vm/run_boot_test.py --iso out/image/cham3-m0-dev-amd64.iso
```

The script boots the ISO read-only in a throwaway QEMU x86-64 VM with SeaBIOS, no attached disk, and no networking. It writes the serial console to `<out-dir>/serial.log` and a summary to `<out-dir>/result.json`. The default output directory is `$CHAM3_OUT_DIR/boot_test`.

Exit codes: `0` PASS, `1` FAIL, `2` BLOCKED (for example, QEMU is missing). The script always terminates QEMU.

Useful options: `--timeout-seconds`, `--memory-mib`, `--cpus`, `--accel auto|kvm|tcg`, `--qemu PATH`.

The script never types credentials. A boot result is an observation of the serial console. It does not demonstrate any cham3 security behavior.

To run the boot test suite:

```sh
tests/run_tests.sh boot
```

`tests/run_tests.sh all` runs every suite.

## Output directory

All build outputs go under `out/` by default. `out/` is gitignored. Override the root with `CHAM3_OUT_DIR`. The live-build work directory defaults to `$CHAM3_OUT_DIR/live-build-work` and can be overridden with `CHAM3_WORK_DIR`.

Do not commit `.deb`, `.iso`, or logs.

To clean outputs, remove the output directory:

```sh
rm -rf out/
```

If you used `CHAM3_OUT_DIR` or `CHAM3_WORK_DIR`, remove those paths instead. The image build's `lb clean --purge` affects only the work directory.

## Provenance

`build/scripts/write_provenance.py` writes `provenance.json` with `provenance_format` set to `cham3_m0_provenance_v1`. Its fields include:

- `reproducibility`: `not_claimed`
- `source`: `git_commit`, `git_tree_dirty`
- `build_host`: OS ID and version, codename, kernel release, machine, dpkg architecture, whether the build ran in a container, and the unverified-host override flag
- `tools`: installed versions of live-build, debootstrap, dpkg-dev, debhelper, lintian, xorriso, and squashfs-tools
- `inputs`: distribution, architecture, archive areas, mirrors, the `lb config` command, `SOURCE_DATE_EPOCH`, and APT release-file hashes (or an explicit null with a reason)
- `local_packages`: the built `.deb` files with their package, version, and SHA-256
- `outputs`: each output file with size and SHA-256
- `build_started_utc` and `build_finished_utc`

Provenance records inputs and tool versions for traceability. It does **not** make the build reproducible.

## Reproducibility

Bit-for-bit reproducibility is **not claimed** for M0. Recording `SOURCE_DATE_EPOCH` and tool versions is intended to support later reproducibility work, but it is not evidence that a rebuild produces identical output.

## Credentials and signing

No credentials are needed.

- Packages are built unsigned (`-us -uc`).
- No signing keys are used or required.
- The image is not signed and is not published to an APT repository.
- Never add secrets, tokens, or private keys to the repository, to CI, or to build artifacts.

## Continuous integration

`.github/workflows/m0_checks.yml` runs on `push`, `pull_request`, and `workflow_dispatch`. Per the M0 interface, it:

- sets top-level `permissions: contents: read`;
- uses no `secrets.*` and no `pull_request_target`;
- pins third-party actions to full commit SHAs;
- runs jobs in `debian:trixie` containers:
  1. static checks and `tests/run_tests.sh static`;
  2. package build and `tests/run_tests.sh packages`;
  3. image build in a `--privileged` container, `tests/run_tests.sh image`, and the boot test with `--accel auto` and a generous timeout;
- uploads the ISO, provenance, and boot logs with short retention.

The image job uses a privileged container because live-build needs it. This is a privileged CI job. It is still limited to the read-only token and no secrets.

CI results are recorded in the task handoff, not in this document.

## Troubleshooting

- **Build refuses to run:** check that you are root, that the host is Debian 13 amd64, and that `$CHAM3_OUT_DIR/packages/*.deb` exist (run the package build first).
- **Lintian fails:** read `$CHAM3_OUT_DIR/packages/lintian.txt`.
- **Boot test reports BLOCKED:** check that `qemu-system-x86` is installed and that `--qemu` points to a valid binary.

Further troubleshooting guidance is not sourced yet.

## Sources

Retrieved 2026-10-11.

- Debian `live-build` package, trixie: https://packages.debian.org/trixie/live-build (version 1:20250505+deb13u1)
- Debian `xorriso` package, trixie: https://packages.debian.org/trixie/xorriso (version 1.5.6-1.2)
- Debian `debhelper` package, trixie: https://packages.debian.org/trixie/debhelper (version 13.24.2)
- Debian `lintian` package, trixie: https://packages.debian.org/trixie/lintian (version 2.122.0)
- Debian `qemu-system-x86` package, trixie: https://packages.debian.org/trixie/qemu-system-x86 (version 1:10.0.13+ds-0+deb13u1)
- Debian `debootstrap` package, trixie: https://packages.debian.org/trixie/debootstrap (version 1.0.141)
- Debian `shellcheck` package, trixie: https://packages.debian.org/trixie/shellcheck (version 0.10.0-1)
- Debian `yamllint` package, trixie: https://packages.debian.org/trixie/yamllint (version 1.37.1-1)
- Debian `squashfs-tools` package, trixie: https://packages.debian.org/trixie/squashfs-tools (version 1:4.6.1-1)
- Debian `dpkg-dev` package, trixie: https://packages.debian.org/trixie/dpkg-dev (version 1.22.22)
- Debian `live-build` man page, trixie: https://manpages.debian.org/trixie/live-build/lb_build.1.en.html (version 1:20250505+deb13u1, shown on packages.debian.org)
- Debian 13 release information (trixie, amd64 listed as a supported architecture): https://www.debian.org/releases/trixie/
- reproducible-builds.org, SOURCE_DATE_EPOCH: https://reproducible-builds.org/docs/source-date-epoch/
- GitHub Docs, Security hardening for GitHub Actions (pinning actions to commit SHAs; least-privilege `GITHUB_TOKEN`): https://docs.github.com/en/actions/security-for-github-actions/security-guides/security-hardening-for-github-actions
- GitHub Docs, Using secrets in GitHub Actions (secrets not passed to workflows triggered from forked repositories): https://docs.github.com/en/actions/security-for-github-actions/security-guides/using-secrets-in-github-actions

Not sourced (unverified): root requirement for live-build; host disk and RAM requirements; the Debian Live Manual at live-team.pages.debian.net was not retrievable (HTTP 404 for the URLs tried).
