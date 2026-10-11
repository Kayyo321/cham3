# M0-001 rework evidence (R1/R2), implementation commit `526bacc`

These are curated copies of evidence for the R1/R2 rework. The rework implementation commit is `526bacccdc3c38e8d61a15cb8e50af85d4cc7557`, on branch `task/M0-repository-build-baseline-rework`. These files are records, not build inputs. The full ISO and raw logs are not committed. Scratch paths are redacted as `<coordinator-scratch>` and `<home>`. The evidence for the original target `5ae0dc5` is in the parent directory and is unchanged.

**Environments:** none of these is a native Debian 13 x86-64 machine.
- **Container:** a disposable `debian:trixie` linux/amd64 container (Debian 13.3) under Docker Desktop 29.2.1 on macOS arm64, using x86-64 user-mode emulation. It was unprivileged. The source was a fresh clone of the commit. Every check ran as the non-root user `builder` (uid 1000).
- **VM:** a disposable Debian 13.7 amd64 VM, booted from a fresh overlay of the same SHA512-verified `debian-13-genericcloud-amd64.qcow2` used for `5ae0dc5`. It ran under QEMU 10.2.1 with TCG (emulated CPU), 6 vCPUs and 4 GiB, on macOS 26.3.1 on Apple M2 Pro. The source arrived as a git bundle, followed by `git clone` and `git checkout --detach 526bacc…`. The flow followed `docs/engineering/developer_setup.md`.
- **Host-QEMU boot:** Homebrew QEMU 10.2.1, TCG, SeaBIOS, 1024 MiB, 2 CPUs, no disk, no network.

| File | Content |
|---|---|
| `regression_sensitivity_before_fix_ac36295.txt` | The new R1/R2 tests plus the existing scanner tests, run in the container against `ac36295`. Its `build/` is identical to the reviewed `82b0cfc`. 30 tests: **8 failures**, which are the 6 R1 fault controls and the 2 R2 fault controls. All positive controls pass. |
| `regression_sensitivity_after_fix_526bacc.txt` | The same 30 tests at `526bacc`: all pass. |
| `container_checks.txt` | `run_static_checks.sh`, `tests/run_tests.sh static` (82 tests, 0 skipped), `build_packages.sh` (lintian exit 0) and `tests/run_tests.sh packages` (8 tests), run as non-root at `526bacc`. The first static-test and package-test invocations failed to start because of a quoting error in the coordinator's harness (`runuser: failed to execute tests/run_tests.sh static`). They were rerun correctly, and both results are shown. |
| `vm_build_transcript.txt`, `vm_step_outputs.txt` | The clean-checkout VM flow: steps, exit codes, durations and output summaries. |
| `provenance.json` | Written by `build_image.sh` in the VM. `git_commit` is `526bacc…`, `git_tree_dirty` is false and `reproducibility` is `not_claimed`. |
| `artifact_sha256sums.txt` | `SHA256SUMS` for `out/image` and `out/packages` from the VM. ISO sha256: `ed1165d57926f2521d629d0b10430f4bf4e2938410ea2bdfb433849c9e3da71d`, 313,524,224 bytes. |
| `image_package_manifest.txt` | The live-build manifest: 159 packages. Its package names and versions are identical to the `5ae0dc5` manifest. |
| `boot_result.json`, `boot_serial_console.txt` | The `build/vm/run_boot_test.py` result for that ISO (host QEMU, PASS in 44.6 s at `debian login:`) and the serial console with ANSI escapes removed. |

**What these results do not show:**
- The R2 regression tests use stub `id`, `lb` and `debootstrap` executables and synthetic outputs. They test only the wrapper's control flow; they are not image-build evidence. The real image build in the VM is the image-build evidence.
- The ISO hash differs from the `5ae0dc5` build. Bit-for-bit reproducibility is not claimed.
- Neither a build nor a boot demonstrates any cham3 security property.
