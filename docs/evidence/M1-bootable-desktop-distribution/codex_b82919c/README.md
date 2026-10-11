# Codex independent verification at b82919c

This is reviewer evidence for implementation commit
`b82919c6ddbdb1f84f8db35e3ded8559c40b242f`, baseline
`401c6cf9a550c81decf832cbc346f6908b9c95ef`, collected on 2026-10-11.
See [the review](../../../reviews/M1-bootable-desktop-distribution-codex.md)
for findings, acceptance dispositions and CHANGES_REQUESTED verdict.
Committed log transcripts normalize terminal carriage returns and trailing whitespace for readable text. `raw_log_SHA256SUMS` records the original log bytes; original copies remain in `/workspace/M1-review-evidence/`. No substantive output was removed.

No product implementation was changed. All elevated work ran in a disposable
Docker container on the native x86-64 Debian 13 Codex Remote environment.

## Setup and commands

The host user was uid 1000. `sudo` was absent; the managed Docker daemon was
available at its local socket. A privileged container provided the documented
disposable build alternative. Its public system CA bundle was mounted read-only
for diagnostics; inherited proxy settings and TLS verification were preserved.
The working directory was `/workspace/M1-review` throughout.

```sh
git fetch origin task/M1-bootable-desktop
git worktree add --detach /workspace/M1-review b82919c6ddbdb1f84f8db35e3ded8559c40b242f
env -u DOCKER_HOST -u DOCKER_CONTEXT -u DOCKER_TLS \
  -u DOCKER_TLS_VERIFY -u DOCKER_CERT_PATH \
  docker --host=unix:///var/run/docker.sock info
docker run -d --name codex-m1-review --privileged \
  --mount type=bind,src=/workspace,dst=/workspace \
  --mount type=bind,src=/etc/ssl/certs/ca-certificates.crt,dst=/run/codex-ca-bundle.pem,readonly \
  -e SSL_CERT_FILE=/run/codex-ca-bundle.pem \
  -w /workspace/M1-review cham3-cloud-dev:trixie sleep infinity
docker exec -u 0 codex-m1-review sh -c \
  'mkdir -p /tmp/mount-probe; mount -t proc proc /tmp/mount-probe && umount /tmp/mount-probe'
docker exec codex-m1-review build/scripts/run_static_checks.sh
docker exec codex-m1-review tests/run_tests.sh static
docker exec codex-m1-review tests/run_tests.sh desktop
docker exec codex-m1-review build/scripts/build_packages.sh
docker exec codex-m1-review tests/run_tests.sh packages
docker exec -u 0 -e CHAM3_LIVE_DIR=build/live-desktop \
  -e CHAM3_IMAGE_NAME=cham3-m1-desktop-amd64 \
  codex-m1-review build/scripts/build_image.sh
tests/run_tests.sh image-m1
```

The M0 reproduction used a separate output tree to avoid interfering with M1:

```sh
docker exec -u 0 codex-m1-review sh -c \
  'mkdir -p /workspace/M1-review/out_m0/packages; cp /workspace/M1-review/out/packages/*.deb /workspace/M1-review/out_m0/packages/; CHAM3_OUT_DIR=/workspace/M1-review/out_m0 build/scripts/build_image.sh'
```

That generated tree was moved to `/workspace/M1-review-evidence/m0-output`
after failure. No change to either image profile was made. Both image-build
processes finished with failed status; the container was subsequently removed.

## Results and artifacts

| Evidence | Result |
|---|---|
| `static_checks.log`, `static_tests.log`, `desktop_tests.log` | PASS: four static checks, 82 M0 static tests, 122 M1 desktop tests |
| `package_build.log`, `package_tests.log`, `package_SHA256SUMS` | PASS: three unsigned package builds, lintian exit 0, eight package tests |
| `image_attempt.log` | BLOCKED: real M1 build failed on firmware downloads, wrapper exit 1 / lb build exit 123 |
| `m0_image_attempt.log` | FAIL: M0 incorrectly selects cham3-full and cannot resolve nikto / torbrowser-launcher |
| `image_tests.log` | FAIL: four required-artifact tests fail; eleven fixture controls pass; no ISO produced |
| `firmware_network_probe.log` | Supported proxy denies `https://downloads.openwrt.org/sources/wl_apsta-3.130.20.0.o`, CONNECT 403 |
| `partial_chroot_contents.log` | Selected dependencies/exclusions pass on partial dpkg status; real paths expose two erroneous test expectations |
| `partial_desktop_validation.log` | Partial chroot only: 113 upstream entries validate, nine fixed launcher IDs exist |
| `partial_chroot_capture.log`, `partial_chroot_packet_capture.log` | Partial chroot only: member opens capture and captures one packet; nonmember gets permission denied |
| `capture_fallback.log`, `capture_fallback_probe.py`, `wireshark-common_4.4.19_postinst.txt` | Controlled upstream postinst reproduction: successful capability path 0754, failed setcap path 04754, both exit 0 |
| `ci_snapshot.json` | Supplemental tip 848b72a: m1_checks success; m0_checks failed at Build live image; CI detailed log unavailable |
| `tool_versions.log` | Declared native environment and tool versions |

The complete M1 log records `I: Valid Release signature`, actual APT dependency
resolution and the precise firmware failure messages. Package checksums cover
development packages, buildinfo and changes files; they are not release
signatures. There is no completed ISO, ISO provenance/checksum/manifest,
graphical boot screenshot, installation log, or installed-system evidence.
No partial-chroot result establishes those gates or M2–M4 enforcement.

## Additional independent probes

The firmware policy probe was:

```sh
curl -I --max-time 20 https://downloads.openwrt.org/sources/wl_apsta-3.130.20.0.o
```

The capture-fallback source was obtained with authenticated Debian APT:

```sh
# Inside the disposable container after apt-get update:
mkdir -p /tmp/review-debs
cd /tmp/review-debs
apt-get download wireshark-common qml6-module-org-kde-plasma-plasma5support plasma-workspace
dpkg-deb -e wireshark-common_4.4.19-0+deb13u1_amd64.deb /tmp/review-control-wireshark-common
python3 /workspace/M1-review-evidence/capture_fallback_probe.py
```

The retained Python script changes only a disposable copy of that upstream
postinst: it supplies the candidate's true answer, targets a disposable test
file, and puts a failing setcap stub on PATH for the negative control.
It does not alter candidate packaging or the real partial-chroot dumpcap.

The partial-chroot member/nonmember test used:

```sh
docker exec -u 0 codex-m1-review sh -c \
  'chroot --userspec=1000:1000 --groups=104 /workspace/M1-review/out/live-build-work/chroot /usr/bin/dumpcap -i lo -a duration:1 -w /tmp/codex-member.pcapng; result=$?; echo "group_member_exit=$result"; chroot --userspec=1000:1000 --groups=1000 /workspace/M1-review/out/live-build-work/chroot /usr/bin/dumpcap -D; result=$?; echo "nonmember_exit=$result"'
docker exec -u 0 codex-m1-review sh -c \
  'chroot --userspec=1000:1000 --groups=104 /workspace/M1-review/out/live-build-work/chroot /usr/bin/dumpcap -i lo -a duration:3 -c 1 -w /tmp/codex-member-packet.pcapng & capture_pid=$!; python3 -c "import socket,time; time.sleep(0.5); s=socket.socket(socket.AF_INET,socket.SOCK_DGRAM); s.sendto(b\"codex review loopback capture\",(\"127.0.0.1\",48123))"; wait "$capture_pid"; echo "capture_exit=$?"'
```

The partial status check called the existing `installed_package_names()`,
`contents_problems()` and `manifest_problems()` on actual paths and
`out/live-build-work/chroot/var/lib/dpkg/status`. Desktop validation ran
`desktop-file-validate` on each real file under that chroot's
`usr/share/applications`. Capture ownership was `0:104`, mode `0754`, and
the chroot's group file identifies gid 104 as `wireshark` with no members.
These probes reuse the disposable container's network namespace; they are
not a live or installed VM.
