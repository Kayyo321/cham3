# Developer Setup: M0 Build Baseline

> **Note:** Commands in this document reflect the M0-001 implementation on branch `task/M0-repository-build-baseline`. Verification evidence for M0-001 is recorded in the task handoff, `docs/handoffs/M0-repository-build-baseline.md`. No command in this document is claimed as verified here; check the handoff for results.

## Scope

This document covers the **M0 repository and build baseline only**.

The image produced by this baseline is a minimal Debian 13 live development image plus the `cham3-base` M0 skeleton package. It is **not** an installable or complete cham3 distribution. It contains no KDE desktop, no installer, no cham3 services, no enforcement, no sandboxing, no network modes, and no signed APT repository.

A successful build or VM boot demonstrates **no security property**. It shows only that packaging, live-build configuration, and the boot path run.

## Supported build host

For M0, the only documented and validated build host is:

- **Debian 13 (trixie), x86-64 (amd64)**, on a clean machine or disposable VM.

All other hosts are **unsupported and unverified** for M0. This includes other distributions, arm64 hosts, containers on other hosts, and x86-64 emulation. Do not assume a build that works elsewhere is representative.

This is a narrow development convention for the M0 task. It is not a product decision about cham3's runtime hardware or supported platforms.

## Root and disposable environment

`build/scripts/build_image.sh` refuses to run unless it is root. The check is `[ "$(id -u)" -eq 0 ]`.

The live-build tool enforces the same rule. The `lb` command in live-build `1:20250505+deb13u1` (Debian 13) exits with `Root privileges needed!` for every command except `config` when the effective user ID is not 0. Live-build also mounts `/proc` inside the build chroot (`chroot_proc` helper), so the image build needs root and mount access.

**Run the image build in a disposable Debian 13 amd64 VM.** Do not run it on a workstation you care about. The build runs as root and mounts filesystems inside its chroot.

Host disk and RAM requirements are not established by a primary source. Provide ample free space for `out/` and the live-build work directory. This figure is unverified.

## Step order

Work through these steps in order on a clean Debian 13 amd64 host.

### 1. Install git and CA certificates

```sh
sudo apt-get update
sudo apt-get install --no-install-recommends git ca-certificates
```

### 2. Clone the repository

```sh
git clone <repository-url> cham3
cd cham3
git log -1 --format='%H %ct'
```

Record the commit you build. The image build uses the commit time of `HEAD` as `SOURCE_DATE_EPOCH` unless you set it yourself.

### 3. Install the build-host packages

The single source of truth for host packages is `build/host_packages.txt`:

```sh
sudo apt-get install --no-install-recommends $(grep -v '^#' build/host_packages.txt)
```

The list covers `git`, `ca-certificates`, `python3`, `file`, `shellcheck`, `yamllint`, `build-essential`, `debhelper`, `dpkg-dev`, `fakeroot`, `lintian`, `live-build`, `debootstrap`, `squashfs-tools`, `xorriso`, `cpio`, and `qemu-system-x86`. `cpio` is listed because live-build declares it as a hard `Depends` (checked with `apt-cache depends live-build` on trixie, 2026-10-11).

`build-essential` is listed although `cham3-base` declares only `debhelper-compat (= 13)` in `Build-Depends`. It is listed because it is an implicit build dependency. Debian Policy section 4.2 ("Package relationships") says it is not necessary to declare relationships on a minimal set of packages, and that the required packages are called build-essential ([Debian Policy 4.2](https://www.debian.org/doc/debian-policy/ch-source.html#package-relationships)). The dpkg tools add it themselves: on Debian, `Dpkg::Vendor::Debian` returns `build-essential:native` from the `builtin-build-depends` hook, and `dpkg-checkbuilddeps` and `dpkg-genbuildinfo` prepend that hook to the declared Build-Depends. Without build-essential installed, `dpkg-checkbuilddeps` on the `cham3-base` control file fails with `unmet build dependencies: build-essential:native` (reproduced on dpkg-dev 1.22.22, trixie, 2026-10-11). Keep it in the host package list.

Debian 13 trixie package versions shown on packages.debian.org on 2026-10-11:

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

Versions change over time and depend on your mirror. Each build's `provenance.json` records the versions actually used. Treat that file as authoritative for the build.

### 4. Run the static checks (no root, no network)

```sh
build/scripts/run_static_checks.sh
```

This runs shellcheck on tracked shell scripts (including `build/live/auto/*`), `python3 -m py_compile` on tracked Python files, yamllint on `.github/workflows/*.yml` using `.yamllint.yml`, and `build/scripts/scan_secrets.sh`. It prints a per-check summary and exits 0 only if every check passes. A missing tool is a FAIL, not a skip.

If `git ls-files` fails, the script prints `FAIL file_discovery`, reports `static checks: FAIL`, and exits 1 without reporting per-check results.

```sh
tests/run_tests.sh static
```

#### Secret scan (`build/scripts/scan_secrets.sh`)

With no arguments, the scanner checks all git-tracked files. With path arguments, it scans those files and directories recursively, which is how build outputs and logs are checked.

- Exit codes: `0` clean, `1` finding, `2` usage or scan error.
- Symlinks are followed. A dangling symlink is skipped and is not an error.
- An unreadable file is a scan error (exit 2). The scanner never reports clean for input it could not read. If a finding was also made, it exits 1.
- Findings print `FINDING <rule>: <path>:<line>`. The matched value is never printed.
- Rules: PEM/OpenSSH/PGP private key blocks, GitHub tokens (`ghp_`, `gho_`, `ghs_`, `ghu_`, `ghr_`, and `github_pat_`), AWS access key IDs, and Slack tokens.
- `-h` or `--help` prints usage.

**Limit:** this is pattern matching only. It is not a complete secret scanner and does not prove the absence of secrets. Binary files are skipped.

### 5. Build the packages (no root)

```sh
build/scripts/build_packages.sh
```

This builds each package under `packaging/*/` with `dpkg-buildpackage -us -uc -b` in a temporary copy, so the source tree is not modified. Outputs go to `$CHAM3_OUT_DIR/packages/` (default `out/packages/`):

- `.deb`, `.buildinfo`, and `.changes` files
- `SHA256SUMS`
- `lintian.txt`

Lintian policy: **lintian errors fail the build**. Warnings, info, and pedantic tags are recorded in `lintian.txt` and do not fail the build. The last line of `lintian.txt` is `# lintian exit status: N`.

```sh
tests/run_tests.sh packages
```

The only M0 package is `cham3-base`. It installs documentation under `/usr/share/doc/cham3-base/` and nothing else. It has no maintainer scripts, conffiles, or systemd units. It is an M0 development skeleton that installs no services, security policy, enforcement, or system configuration.

### 6. Build the image (root, disposable environment only)

```sh
sudo build/scripts/build_image.sh
```

The script:

- first refuses an empty `CHAM3_OUT_DIR` or `CHAM3_WORK_DIR`, a path that resolves to `/`, a work directory equal to the output directory, and a work directory equal to the repository root. These checks run before the root check, so they can be tested without root;
- refuses to run unless it is root;
- checks that the host is Debian 13 amd64 (`VERSION_ID=13` and `dpkg --print-architecture` = `amd64`); to proceed on another host, set `CHAM3_ALLOW_UNVERIFIED_HOST=1`, which is recorded in provenance as an unverified host;
- refuses to run if `$CHAM3_OUT_DIR/packages/*.deb` is missing (run step 5 first);
- refuses to reuse an existing work directory that does not contain the marker file `.cham3_live_build_work`;
- copies `build/live` into the work directory, copies the built `.deb` files into `config/packages.chroot/` there, and runs `lb clean --purge`, `lb config`, and `lb build` in the work directory only;
- sets `SOURCE_DATE_EPOCH` from the commit time of `HEAD` if it is not already set.

The build reads from `build/live` and writes only to the work directory and the output directory. Because it runs as root, `out/image/` is root-owned after a build.

Network access to both official mirrors is required during the build:

- `http://deb.debian.org/debian/`
- `http://security.debian.org/`

Outputs in `$CHAM3_OUT_DIR/image/`:

- `cham3-m0-dev-amd64.iso`
- `cham3-m0-dev-amd64.packages` (live-build package manifest)
- `build.log`
- `provenance.json`
- `SHA256SUMS`

```sh
tests/run_tests.sh image
```

#### live-build configuration (from `build/live/auto/config`)

- Distribution `trixie`, architecture `amd64`, archive area `main` only, no installer (`--debian-installer none`).
- Image type: `--binary-image iso-hybrid`.
- Bootloaders: `syslinux,grub-efi`. **Only the BIOS path (SeaBIOS) is exercised by the boot test. UEFI boot is untested.**
- Firmware is off: `--firmware-chroot false`, `--firmware-binary false`. No `non-free-firmware`.
- Security and updates are enabled (`--security true`, `--updates true`).
- APT authentication is not disabled (`--apt-secure true`).
- `--apt-recommends false` and `--cache false`.
- Kernel parameters: `boot=live components console=tty0 console=ttyS0,115200n8`. The serial console is for VM testing.
- Mirrors stay on plain HTTP (`http://deb.debian.org/debian/` and `http://security.debian.org/`). APT verifies the signed InRelease files with `debian-archive-keyring`, and authentication is not disabled (`--apt-secure true`). HTTPS would add only transport confidentiality. Switching to HTTPS is a possible follow-up.
- The only package list is `build/live/config/package-lists/m0_base.list.chroot`, which names `cham3-base`. The local `.deb` is supplied at build time from `config/packages.chroot/`. No `.deb` files are committed.

### 7. Boot the image in a disposable VM (no root)

```sh
build/vm/run_boot_test.py --iso out/image/cham3-m0-dev-amd64.iso
```

The script boots the ISO read-only in a throwaway QEMU x86-64 VM with no attached disk and no networking. It writes the serial console to `<out-dir>/serial.log` and a summary to `<out-dir>/result.json`. The default output directory is `$CHAM3_OUT_DIR/boot_test`.

Defaults:

- machine: `pc` (SeaBIOS)
- memory: 1024 MiB
- CPUs: 2
- timeout: 900 seconds
- accelerator: `--accel auto` uses KVM when `/dev/kvm` is readable and writable on an x86-64 Linux host, otherwise TCG (software emulation)

The syslinux boot menu has no timeout. The script therefore presses Enter through the QEMU monitor (`sendkey ret`) until the kernel writes to the serial console.

Classification of the serial console:

- **PASS** only when a login prompt is reached, or a non-root autologin shell prompt (ending in `$`) appears.
- **FAIL** for any of these markers: `Kernel panic`, the initramfs shell `(initramfs)`, `Unable to find a medium`, `You are in emergency mode`, `You are in rescue mode`, the maintenance password prompt `Give root password for maintenance`, or an unexpected root shell prompt (ending in `#`) on the serial console.
- **PENDING** otherwise. The script keeps waiting until the timeout, then reports FAIL.

The script does not type credentials, so runtime checks inside the VM are not performed when only a login prompt is shown. A PASS says nothing about any cham3 behavior.

Exit codes: `0` PASS, `1` FAIL, `2` BLOCKED (for example, QEMU is missing). The script always terminates QEMU.

Options: `--timeout-seconds`, `--memory-mib`, `--cpus`, `--accel auto|kvm|tcg`, `--qemu PATH`.

```sh
tests/run_tests.sh boot
```

`tests/run_tests.sh all` runs every suite.

## Limitations of the image

The image keeps Debian live-config defaults. The live-config package at trixie (`11.0.5`) uses the username `user` and sets the password to `live` when no override is given (`/usr/lib/live/config/0030-user-setup`, comment "Default password is: live"). Anyone who knows these defaults can log in. The image is a development artifact. **Do not run it on or expose it to untrusted networks.**

The image is not hardened and is not a cham3 system. The `sudo` package is not installed in the M0 image: the manifest of the M0-001 evidence build (`cham3-m0-dev-amd64.packages`, 159 packages) lists no `sudo` package. Each build's `.packages` manifest is the authority for what that build contains.

## Output directory and cleanup

All build outputs go under `out/` by default. `out/` is gitignored. Override the root with `CHAM3_OUT_DIR`. The live-build work directory defaults to `$CHAM3_OUT_DIR/live-build-work` and can be overridden with `CHAM3_WORK_DIR`.

Do not commit `.deb`, `.iso`, or logs.

`build_image.sh` runs as root, so `out/image/` and the work directory are root-owned after a build. Clean them with:

```sh
sudo rm -rf out/
```

If you set `CHAM3_OUT_DIR` or `CHAM3_WORK_DIR`, remove those paths instead. The script's `lb clean --purge` applies only to the work directory, and only if it carries the marker file.

## Provenance

`build/scripts/write_provenance.py` writes `provenance.json` with `provenance_format` set to `cham3_m0_provenance_v1`. Its top-level fields are:

- `reproducibility`: `not_claimed`
- `source`: `git_commit`, `git_tree_dirty`
- `build_host`: `os_id`, `os_version_id`, `os_version_codename`, `kernel_release`, `machine`, `dpkg_architecture`, `in_container`, `unverified_host_override`
- `tools`: installed versions of `live-build`, `debootstrap`, `dpkg-dev`, `debhelper`, `lintian`, `xorriso`, `squashfs-tools`, `debian-archive-keyring`, and `qemu-system-x86`
- `inputs`: `distribution`, `architecture`, `archive_areas`, `mirrors` (bootstrap, chroot, chroot_security, binary, binary_security), `security_updates_enabled`, `updates_enabled`, `lb_config_command`, `source_date_epoch`, `apt_release_files` (or null), `apt_release_files_unavailable_reason`, `live_config_file_sha256`, and `package_versions_note`
- `local_packages`: each built `.deb` with its file name, package, version, and SHA-256
- `outputs`: each output file with its size and SHA-256
- `build_started_utc`, `build_finished_utc`, and `written_utc`

Provenance records inputs and tool versions for traceability. It does **not** make the build reproducible.

## Reproducibility

Bit-for-bit reproducibility is **not claimed** for M0. `SOURCE_DATE_EPOCH` is set from the commit time of `HEAD` so that later reproducibility work has a fixed input. Setting it does not show that a rebuild produces identical output.

## Credentials and signing

No credentials are needed.

- Packages are built unsigned (`-us -uc`).
- No signing keys are used or required.
- The image is not signed and is not published to an APT repository.
- Do not add secrets, tokens, or private keys to the repository, to CI, or to build artifacts.

## Continuous integration

`.github/workflows/m0_checks.yml` runs on `push`, `pull_request`, and `workflow_dispatch`.

- Top-level `permissions: contents: read`. No `secrets.*` and no `pull_request_target`.
- Jobs run on `ubuntu-latest` GitHub-hosted runners, each inside a `debian:trixie` container.
- `persist-credentials: false` on checkout.
- Third-party actions are pinned by full commit SHA: `actions/checkout` v5.0.0 (`08c6903cd8c0fde910a37f88322edcfb5dd907a8`) and `actions/upload-artifact` v5.0.0 (`330a01c490aca151604b8cf639adc76d48f6c5d4`).

Jobs:

1. `static_checks`: runs `build/scripts/run_static_checks.sh` and `tests/run_tests.sh static` as an unprivileged `builder` user (uid 1000), so the root-sensitive tests run rather than skip.
2. `package_build`: `build/scripts/build_packages.sh`, then `tests/run_tests.sh packages`. Uploads `out/packages` as artifact `m0_packages` with 7-day retention.
3. `image_build`: runs in a **`--privileged`** container, with a 120-minute timeout. It runs, in order: `build/scripts/build_packages.sh`; `build/scripts/build_image.sh`; `tests/run_tests.sh image`; `build/vm/run_boot_test.py --iso out/image/cham3-m0-dev-amd64.iso --out-dir out/boot_test --accel auto --timeout-seconds 1800`; then `tests/run_tests.sh boot` as the "Boot result tests" step. Uploads `out/image` and `out/boot_test` as artifact `m0_image` with 3-day retention.

The `--privileged` option is needed because live-build mounts filesystems in its chroot. The job runs with a read-only token and no secrets, but it is still a privileged job. The workflow comments state that `--privileged` is acceptable only on GitHub-hosted ephemeral runners. Do not move this job to self-hosted runners without a security review.

**This workflow has not yet been executed on GitHub as part of M0-001.** Any CI result is recorded in the task handoff after it runs.

## Troubleshooting

- **`build_image.sh` refuses to run:** run it as root, on Debian 13 amd64, after `build/scripts/build_packages.sh` has produced `out/packages/*.deb`. For a different host, set `CHAM3_ALLOW_UNVERIFIED_HOST=1` only for a disposable investigation; the build is not validated on that host.
- **`build_image.sh` refuses the work directory:** the directory exists without `.cham3_live_build_work`. Remove it or point `CHAM3_WORK_DIR` to a new path.
- **Lintian fails:** read `$CHAM3_OUT_DIR/packages/lintian.txt`. Errors fail the build; warnings do not.
- **Boot test reports BLOCKED:** check that `qemu-system-x86_64` is installed, or pass `--qemu PATH`.
- **Boot test reports FAIL:** read `<out-dir>/serial.log` and `result.json`. Failure markers are listed under step 7.
- **Secret scan reports exit 2:** a file could not be read. Fix the permissions or remove the file from the scanned path; the scanner does not report clean in this case.

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
- Debian 13 release information (trixie; amd64 listed as a supported architecture): https://www.debian.org/releases/trixie/
- live-build root check and chroot mounts: `/usr/bin/lb` (lines 61-66, "Root privileges needed!") and `/usr/lib/live/build/chroot_proc` (mounts `/proc` in the chroot), from live-build `1:20250505+deb13u1` installed in `docker run --rm --platform linux/amd64 debian:trixie`, 2026-10-11. This is an emulated amd64 container on an arm64 host, not a native Debian 13 host.
- live-config default user and password: `/usr/lib/live/init-config.sh` (`LIVE_USERNAME="user"`) and `/usr/lib/live/config/0030-user-setup` ("Default password is: live"), from live-config `11.0.5` installed in the same container, 2026-10-11.
- reproducible-builds.org, SOURCE_DATE_EPOCH: https://reproducible-builds.org/docs/source-date-epoch/
- GitHub Docs, Security hardening for GitHub Actions (pinning actions to full commit SHAs; least-privilege `GITHUB_TOKEN`): https://docs.github.com/en/actions/security-for-github-actions/security-guides/security-hardening-for-github-actions
- GitHub Docs, Using secrets in GitHub Actions (secrets are not passed to workflows triggered from a forked repository): https://docs.github.com/en/actions/security-for-github-actions/security-guides/using-secrets-in-github-actions
- Action tag commits, via `git ls-remote`, 2026-10-11: `actions/checkout` `refs/tags/v5.0.0` = `08c6903cd8c0fde910a37f88322edcfb5dd907a8`; `actions/upload-artifact` `refs/tags/v5.0.0` = `330a01c490aca151604b8cf639adc76d48f6c5d4` (lightweight tags, so the tag SHA is the commit).

- Debian Policy, section 4.2 "Package relationships" (build-essential is an implicit build dependency): https://www.debian.org/doc/debian-policy/ch-source.html#package-relationships (retrieved 2026-10-11).
- dpkg-dev `1.22.22` behavior, in `docker run --rm --platform linux/amd64 debian:trixie`, 2026-10-11: `/usr/share/perl5/Dpkg/Vendor/Debian.pm` (`builtin-build-depends` returns `build-essential:native`); `/usr/bin/dpkg-checkbuilddeps` line 104 (prepends the hook to Build-Depends); reproduced failure `dpkg-checkbuilddeps: error: unmet build dependencies: build-essential:native` with the `cham3-base` control file and build-essential not installed. Note: `dpkg-dev` only Recommends build-essential; the reproduction used `--no-install-recommends`.
- M0-001 evidence build image manifest `cham3-m0-dev-amd64.packages` (159 packages): contains no `sudo` entry (`grep -i sudo` returned no match), 2026-10-11.
- live-build package dependencies: `apt-cache depends live-build` in `debian:trixie`, 2026-10-11, shows `Depends: cpio`.

Not established by a primary source (unverified): host disk and RAM requirements; the Debian Live Manual pages at live-team.pages.debian.net, which returned HTTP 404 for the URLs tried.
