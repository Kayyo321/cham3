# M0-001 evidence files

These are curated copies of evidence from the coordinator's clean-checkout evidence run of implementation commit `5ae0dc526336b7f3c6dec10b94ff4b78f135d593`. They are records, not build inputs. The full ISO and raw logs are not committed. The commit that adds this directory changes only documentation and evidence; it does not change implementation.

**Build environment:** a disposable Debian 13.7 (trixie) amd64 virtual machine. It booted from a fresh overlay of the official `debian-13-genericcloud-amd64.qcow2` cloud image, last modified 2026-10-01. The image's SHA512 matches cloud.debian.org `SHA512SUMS`: `f46f0671…b468c3d`. Debian publishes no detached signature for that file, so the checksum is not signature-verified.

**VM configuration:**
- Kernel `6.12.111+deb13-cloud-amd64`, 6 vCPUs, 4 GiB RAM.
- QEMU 10.2.1 with TCG (full x86-64 emulation), running on macOS 26.3.1 on Apple M2 Pro (arm64).
- The source was transferred as a git bundle of the target commit, then `git clone` and `git checkout --detach`.

**Boot environment:** the macOS host, Homebrew QEMU 10.2.1, TCG, SeaBIOS, no disk, no network.

| File | Content |
|---|---|
| `vm_build_transcript.txt` | Step-by-step commands with exit codes and durations for the documented developer flow inside the VM. Scratch paths are redacted as `<coordinator-scratch>`. |
| `vm_step_outputs.txt` | Output from the VM: environment, clone, tool versions, static checks, test summaries, the build log head and tail, and the artifact secret scan. |
| `provenance.json` | The `provenance.json` that `build/scripts/build_image.sh` wrote in the VM. |
| `artifact_sha256sums.txt` | `SHA256SUMS` for `out/image` and `out/packages`. ISO sha256: `060382a0daba662d6f2354ce9abcbdde77c9337743f699ffeef2dea0e0bf1478`. |
| `image_package_manifest.txt` | The live-build package manifest of the image (159 packages). It includes `cham3-base 0.0.0~m0` and no `sudo` package. |
| `boot_result.json`, `boot_serial_console.txt` | The `build/vm/run_boot_test.py` result for that ISO, and the serial console with ANSI escapes removed. |

These results come from a virtual machine and from x86-64 emulation. They are not native-hardware or hardware-compatibility results. A build or boot result demonstrates no cham3 security property.
