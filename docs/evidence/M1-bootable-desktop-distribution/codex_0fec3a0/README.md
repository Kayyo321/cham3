# Independent native M1 re-review evidence — 0fec3a0

Reviewer: Codex / OpenAI, REVIEWER and SECURITY REVIEWER. Implementation owner:
Claude Code. Reviewed implementation `0fec3a01c18b95fc238d2168ed0d97e0d3831865`,
baseline `401c6cf9a550c81decf832cbc346f6908b9c95ef`. Review date 2026-10-11
America/New_York; raw tool timestamps are UTC on 2026-10-12.

Verdict and findings: [independent review](../../../reviews/M1-bootable-desktop-distribution-codex-rereview.md).

## Environment and artifact identity

- Source checkout `/workspace/M1-rereview`, clean at the target during all checks;
  branch `review/M1-codex-0fec3a0` holds documentation only after verification.
- Host: Debian 13.6 x86-64, kernel 6.18.44; Docker 28.4.0, vfs.
- Container `codex-m1-rereview`: native amd64 Debian 13.7,
  `cham3-cloud-dev:trixie`, image
  `sha256:fa9efb35525a10a9855860b454d01228d904c044f9ce643bc36f9338729c1b24`.
  Started with `--privileged`, `/workspace` bound at the same path and the outer
  public CA bundle mounted read-only at `/run/session-ca.crt`. Fast/package gates
  ran as builder uid 1000; elevated operations ran through `docker exec -u 0`.
- Container `codex-m1-f2`: native amd64 `debian:trixie`, image
  `sha256:99339bdd341ab316eeae3dcf870b98bc7243d99269d01666b0f49a565c1399da`;
  `/workspace/M1-rereview-evidence` bound at `/out`, same public CA mount.
- Used Docker's configured proxy environment. Session APT config uses inherited
  HTTP/HTTPS proxy settings and `Acquire::https::CaInfo`; TLS checks remain enabled.
  `m1_build_session.py` supplies only that session proxy/CA config to the disposable
  chroot; no candidate source, package selector or firmware input was edited.
  `network-policy.json` is the startup snapshot; actual upstream requests return
  proxy HTTP 403. The status connector reported running/connected but policy state
  unknown, so enforcement readiness is not inferred from that connector.
- Versions: `native_versions.log`. No architecture override. `/proc` mount/unmount
  probe passed; `/dev/kvm` absent. Actual M0 boot uses TCG/SeaBIOS.
- M0 ISO: **3074795cc19effff09d86bca0f71a6d101d981e6e83184c37dda664f2cccbb08**.
  Preserved at `/workspace/M1-rereview-evidence/m0_image/cham3-m0-dev-amd64.iso`
  (about 299 MiB; not committed). Manifest, raw-log identity, provenance, boot
  result and serial transcript are preserved here.
- No M1 ISO, completed manifest, screenshot, installation or installed-system
  image exists. The allocated `m1_install.qcow2` is an empty disposable 25 GiB
  virtual disk, not an installed artifact.
- Built package checksums: `package_SHA256SUMS_and_f3_state.txt` (unsigned
  development packages retained locally under `/workspace/M1-rereview/out/packages`).

Committed terminal `.log` transcripts and HTTP header transcripts normalize terminal carriage returns and trailing
whitespace for readable diffs. `raw_log_SHA256SUMS` records the original bytes in
`/workspace/M1-rereview-evidence`, including the raw M0 build/serial logs and CI HTML.
`m0_image/provenance.json` and `m0_image/SHA256SUMS` bind the **raw** build log, not
its normalized committed transcript. `SHA256SUMS` in this directory binds the
committed evidence files. No substantive output was removed. Disposable synthetic
private keys remain outside the repository and are removed with the containers.

## Exact commands and outcomes

All candidate commands use `/workspace/M1-rereview` as cwd inside the declared
container unless stated otherwise. Transcripts and script sources sit beside this
README. Commands requiring unavailable artifacts are recorded as BLOCKED even
when their process exits nonzero; a test runner's observed failures are also given.

| Command | User/environment | Result | Transcript |
|---|---|---|---|
| `apt-get update; apt-get install -y --no-install-recommends $(sed '/^#/d' build/host_packages.txt) desktop-file-utils socat curl libcap2-bin` | root, privileged native container | PASS | `prerequisites.log` |
| `build/scripts/run_static_checks.sh` | uid 1000 | PASS | `static_checks.log` |
| `tests/run_tests.sh static` | uid 1000 | PASS, 82 | `static.log` |
| `tests/run_tests.sh staging` | uid 1000 | PASS, 16 | `staging.log` |
| `tests/run_tests.sh desktop` | uid 1000 | PASS, 224 | `desktop.log` |
| `build/scripts/build_packages.sh` | uid 1000 | PASS, lintian 0 | `package_build.log` |
| `tests/run_tests.sh packages` | uid 1000 | PASS, 8 | `packages.log` |
| `mkdir -p /tmp/cham3-mount-probe; mount -t proc proc /tmp/cham3-mount-probe; umount /tmp/cham3-mount-probe; build/scripts/build_image.sh` | root; no CHAM3 selectors | PASS, M0 | `m0_build_attempt.log`, `m0_image/` |
| `tests/run_tests.sh image` | uid 1000 | PASS, 17 | `m0_image_tests.log` |
| `python3 build/vm/run_boot_test.py --iso out/image/cham3-m0-dev-amd64.iso --out-dir out/boot_test --accel tcg --timeout-seconds 600` | uid 1000 | PASS | `m0_boot_attempt.log`, `m0_boot/` |
| `tests/run_tests.sh boot` | uid 1000 | PASS, 18 | `m0_boot_tests.log` |
| `tests/run_tests.sh container` | outer native host, wrapper's ordinary Docker container | PASS with 4 skips, 51 | `container_default.log` |
| `CHAM3_REPO_ROOT=/workspace/M1-rereview CHAM3_OUT_DIR=/workspace/M1-rereview/out sh tests/container/in_container.sh` | root, privileged native container | FAIL, 1 failure/2 skips, 51 | `container_privileged.log` |
| `python3 /workspace/M1-rereview-evidence/initial_install_probe.py` | root; actual Debian dpkg plus exact candidate script harness | FAIL policy: transient/interrupted 04754 observed | `initial_install_probe.log` |
| `python3 /workspace/M1-rereview-evidence/harness_diagnostics.py` | root | FAIL harness controls | `harness_diagnostics.log` |
| `apt-get install -y --allow-downgrades --no-install-recommends wireshark-common=4.4.18-0+deb13u1; stat -c '%U %G %a' /usr/bin/dumpcap; getcap /usr/bin/dumpcap; apt-get install -y --no-install-recommends wireshark-common=4.4.19-0+deb13u1; stat -c '%U %G %a' /usr/bin/dumpcap; getcap /usr/bin/dumpcap; dpkg-reconfigure -f noninteractive wireshark-common; stat -c '%U %G %a' /usr/bin/dumpcap; getcap /usr/bin/dumpcap; dpkg-statoverride --list /usr/bin/dumpcap` | root, override established | PASS lifecycle | `f3_real_upgrade.log` |
| `python3 /workspace/M1-rereview-evidence/independent_capture.py` | root starts shell after dropping credentials to member/nonmember | PASS, actual packet / denial | `independent_capture.log`, `independent_capture_after_upgrade.log`, `member_capture.pcapng` |
| `python3 /workspace/M1-rereview-evidence/pin_metadata_probe.py` | root, isolated signed synthetic APT roots; no network/install | FAIL default-deny policy | `pin_metadata_probe.log` |
| `sh /out/f2_closure_gate.sh` with `CURL_CA_BUNDLE=/run/session-ca.crt` | separate native F2 container | BLOCKED, exit 22 at archive.kali.org 403 | `f2_closure_attempt.log` |
| `sh /out/f2_origin_gate.sh` | separate native F2 container | BLOCKED, authenticated-index prerequisite exit 100 | `f2_origin_attempt.log` |
| `python3 /workspace/M1-rereview-evidence/m1_build_session.py` invoking `CHAM3_LIVE_DIR=build/live-desktop CHAM3_IMAGE_NAME=cham3-m1-desktop-amd64 build/scripts/build_image.sh` | root, native privileged container | BLOCKED, exit 1/lb 100, repository proxy 403 | `m1_build_attempt.log` |
| `tests/run_tests.sh image-m1` | uid 1000 | FAIL, 28 fixture passes/8 missing-artifact failures | `m1_image_tests.log` |
| `qemu-system-x86_64 -machine q35 -accel tcg -cpu max -smp 4 -m 4096 -device virtio-vga -display vnc=127.0.0.1:0 -nic none -cdrom out/image/cham3-m1-desktop-amd64.iso -serial file:/workspace/M1-rereview-evidence/m1_serial.log -monitor unix:/workspace/M1-rereview-evidence/m1_monitor.sock,server,nowait` | uid 1000 | BLOCKED, missing ISO | `m1_graphical_attempt.log` |
| `apt-get install -y --no-install-recommends qemu-utils` | root | PASS, required qemu-img prerequisite | `install_prerequisite.log` |
| `qemu-img create -f qcow2 /workspace/M1-rereview-evidence/m1_install.qcow2 25G; qemu-system-x86_64 -machine q35 -accel tcg -cpu max -smp 4 -m 4096 -device virtio-vga -display vnc=127.0.0.1:0 -drive file=/workspace/M1-rereview-evidence/m1_install.qcow2,if=virtio,format=qcow2 -cdrom out/image/cham3-m1-desktop-amd64.iso -boot d -nic none` | uid 1000 | BLOCKED, missing ISO | `m1_install_attempt.log` |
| Installed-system boot/session/panel/branding/inert state/launch/capture | no installed system | BLOCKED; not executed | no artifact |
| Native F2 full closure/context/control/naive origin and final-image trust | authenticated foreign indices unavailable; no M1 ISO | BLOCKED; dependent steps not executed | above F2 attempts |
| `sh /workspace/M1-rereview-evidence/firmware_binary_probe.sh` | root, native installed live-build selector | FAIL firmware exclusion: selects 44 including both b43 installers | `firmware_binary_probe.log`, `installer_firmware_packages.txt` |
| For each of `firmware-b43-installer firmware-b43legacy-installer`: `chroot out/live-build-work/chroot apt-get --yes -o Dir::Cache=/binary.deb -o Dir::State::status=/dev/null -o APT::Install-Recommends=false -o Debug::NoLocking=true --download-only install "$package"`; then `ls -l out/live-build-work/chroot/binary.deb/archives/firmware-b43*.deb; sha256sum out/live-build-work/chroot/binary.deb/archives/firmware-b43*.deb` | root; authenticated Debian index, no installer executed | FAIL exclusion: both excluded debs downloaded | `firmware_binary_download_probe.log` |
| `git diff --check 401c6cf 0fec3a0` | outer host | FAIL, exit 2 (retained evidence whitespace) | `candidate_diff_check.log` |

The F2 closure script is the exact shell block extracted from readiness packet
section 6.1. F2-ORIGIN first requires a successful authenticated update using the
candidate's real keyring/sources/pins; it stops before meaningless simulations
when indices are denied. The native signed synthetic pin probe is a separate
negative design test, not a replacement for either real-source gate. Its fixture
key is generated locally and does not impersonate any actual upstream signature.

The capture scripts use the dependency-free harness specified by the candidate's
own test approach: exact built control scripts/triggers, no KDE payload. The
first-install probe configures real wireshark-common while the candidate harness
is only unpacked, records the unsafe state, then configures cham3-desktop and
records recovery. Full installed-system acceptance remains blocked.

`livebuild_installer_debian_installer.sh` and `livebuild_firmwarelists.sh` are copies
of the actual native tool sources used to evaluate installer firmware selection.
The scheduling probe contacts only deb.debian.org; the download probe downloads
Debian packages only, never executes their postinsts or fetches Broadcom blobs.

Public GitHub run pages inspected with verified TLS:

- `curl -L --fail --max-time 20 https://github.com/Kayyo321/cham3/actions/runs/38195543252`
- `curl -L --fail --max-time 20 https://github.com/Kayyo321/cham3/actions/runs/38195543207`

Both report Success for documentation tip `6f38fc3`, whose implementation is
identical to the reviewed target. `ci_observed.json` preserves the observed status
text and URLs; raw page hashes are recorded. GitHub job artifacts were not fetched.

No build/boot/container result establishes M2–M4 enforcement, anonymity,
contract-verified launch, or installed-system acceptance. No merge/milestone
approval or independent F2/F3 security approval is granted.
