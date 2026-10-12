# Independent Codex verification — M1 at 01bf7a1

Reviewer: Codex (OpenAI), REVIEWER and SECURITY REVIEWER; implementation owner remains Claude Code. Date: 2026-10-12. Base `401c6cf9a550c81decf832cbc346f6908b9c95ef`; implementation `01bf7a1f1066fae15ab51a56a5212c0a69780ab8`; documentation/evidence tip `058f839`. Local worktree `/workspace/cham3-review-m1`, branch `review/M1-codex-01bf7a1`. No product code modified and no merge performed.

Review: `docs/reviews/M1-bootable-desktop-distribution-codex-01bf7a1.md`.

## Local checks

Native amd64 Debian 13, prepared `cham3-cloud-dev:trixie` toolchain (image ID `sha256:fa9efb35525a10a9855860b454d01228d904c044f9ce643bc36f9338729c1b24`, architecture amd64) under managed Docker 28.4.0, outer kernel `Linux 6.18.44 x86_64`, uid 1000 for repository checks/builds. Read-only source and parent Git metadata mounts; only gitignored `out/` writable. Initial invocation:

```sh
mkdir -p out/review-evidence
docker run --rm --network none --user 1000:1000 \
  -e PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
  -e PYTHONDONTWRITEBYTECODE=1 \
  -e CHAM3_REPO_ROOT=/workspace/cham3-review-m1 \
  -e CHAM3_OUT_DIR=/workspace/cham3-review-m1/out \
  --mount type=bind,src=/workspace/cham3,dst=/workspace/cham3,readonly \
  --mount type=bind,src=/workspace/cham3-review-m1,dst=/workspace/cham3-review-m1,readonly \
  --mount type=bind,src=/workspace/cham3-review-m1/out,dst=/workspace/cham3-review-m1/out \
  -w /workspace/cham3-review-m1 cham3-cloud-dev:trixie bash -c \
  'set -e; build/scripts/run_static_checks.sh; tests/run_tests.sh static; tests/run_tests.sh desktop; tests/run_tests.sh staging; tests/run_tests.sh apt; build/scripts/build_packages.sh; tests/run_tests.sh packages' \
  > out/review-evidence/local_gates.log 2>&1
```

PASS: static checks, 82 static, 261 desktop, 16 staging tests, no skips. The prepared image lacked gpg-agent, so the F6 suite could not generate disposable signing keys; package checks after it were not reached. That is an environment block, not an implementation finding. Retried in a fresh disposable container with the same mounts/environment and `--user 0`, default managed Docker proxy settings (no `--network none`), adding `-e DEBIAN_FRONTEND=noninteractive`; exact shell body:

```sh
set -e
apt-get update
apt-get install -y --no-install-recommends gnupg desktop-file-utils
runuser -u builder -- tests/run_tests.sh apt
runuser -u builder -- build/scripts/build_packages.sh
runuser -u builder -- tests/run_tests.sh packages
```

Output: `local_apt_packages.log`. PASS: 22 real-APT signed synthetic identity tests, package build, lintian exit 0, 8 package tests; no skips. No elevated changes made to the outer workspace host.

Also executed:

```sh
git diff --quiet 01bf7a1 058f839 -- ':!docs'
git diff --check 401c6cf
(cd docs/evidence/M1-bootable-desktop-distribution/native_01bf7a1 && sha256sum -c SHA256SUMS)
python3 docs/evidence/M1-bootable-desktop-distribution/codex_01bf7a1/auto_build_failure_probe.py .
```

All PASS. Native checksum output: `native_hash_check.log`. The reviewer-only auto/build probe substitutes lb and apt-get in a temporary tree: successful stage ordering, Debian-only installer stage, source/index removal, propagation of stage failures, and restoration of the four stashed foreign files on installer failure. It is **mocked orchestration evidence**, not image/firmware/signature proof; source and output are preserved here.

## Independent GitHub Actions reruns

Local proxy policy returned HTTP 403 for `http://http.kali.org/kali/dists/kali-rolling/InRelease`, `https://repo.librewolf.net`, and `gh api repos/Kayyo321/cham3/actions/runs/38207816363`. No policy bypass or configuration change attempted. Connected GitHub tools could read jobs/logs and request reruns; those ran in the recipe's disposable native environment. Each rerun checkout log confirms the exact implementation SHA, irrespective of the newer branch tip.

| Workflow / rerun target | Result / evidence |
|---|---|
| [m1_image 38207816363](https://github.com/Kayyo321/cham3/actions/runs/38207816363), rerun original F2 job 114675140633 → new job 114814798886 | PASS 7/7, original configuration against real archives. `remote_f2_native_excerpt.log` contains exact-commit and gate lines. |
| [m1_checks 38207816376](https://github.com/Kayyo321/cham3/actions/runs/38207816376), rerun original privileged job 114675112454 → new job 114814860911 | PASS 77, no skips, including actual version change. `remote_privileged_capture_excerpt.log` contains exact-commit and test lines. |
| m1_image, rerun ISO job 114814848088 → new job 114816826997 | PASS: ISO built and 54 image-m1 tests passed. `remote_iso_excerpt.log` preserves check results and upload identities. |
| m1_image, dependent VM job 114823605947 | PASS: live 37/37, install 1/1 (320 seconds, clean poweroff), installed 65/65. `remote_vm_excerpt.log` and `remote_vm_checks_from_console.json` preserve console results. `remote_heavy_jobs.json` records final successful job/step conclusions. |

The F2 rerun saw 9,947 foreign-only packages (rolling metadata changed since the author's 9,945), zero escapes, and the same exact eight Kali names plus LibreWolf in the 1,045-package closure. Without the pins the control leaked 1,159 unlisted Kali packages. radare2 and LibreWolf were actually installed, the upgrade plan contained no unlisted foreign downloads, and ghidra-data remained unavailable. Authentication stayed enabled. The privileged rerun exercised real wireshark-common/dpkg with a dependency-free stub containing the shipped cham3-desktop control scripts; it does not install the KDE payload.

Stable artifact identities independently fetched from GitHub:

- F2 rerun artifact `11747183560`, `m1_f2_native`, created 2026-10-12T12:43:27Z, workflow head SHA `01bf7a1…`, ZIP digest `sha256:68fca4f0abd07bdab836f62297ac9a3c295cf73999bc4e9a2af9498e6d803f03`.
- Privileged rerun artifact `11747454098`, `m1_privileged_capture`, created 2026-10-12T12:42:02Z, workflow head SHA `01bf7a1…`, ZIP digest `sha256:2ee5eb7fb0a7324c1b8bf8c6593fabde2edfd167e5b6a98c6414f9cde841b5a7`.
- Rebuilt ISO artifact `11748821992`, `m1_iso`, created 2026-10-12T13:02:36Z, workflow head SHA `01bf7a1…`, ZIP digest `sha256:b04e03601b5b47f517d337241d03cd7f580db63e61eff1028f8f97025d2c6646` (the ZIP digest, not the raw ISO checksum). Image checks verify the raw ISO against its newly generated checksums/provenance inside the job. Those files are in evidence artifact `11749106921`, `m1_iso_evidence`, ZIP digest `sha256:c6ddfeabe91fbdb9174d0c8295d8bd88e6c4da0b9299ef0c141cab0c81ff2d7b`. The raw rebuilt ISO was not downloaded or independently rehashed locally.
- VM rerun artifact `11749592083`, `m1_vm_evidence`, created 2026-10-12T13:24:40Z, workflow head SHA `01bf7a1…`, ZIP digest `sha256:36d25e1337912bc49a9030f13b19e37b64d63f4150ce99757c240b688057b746`. The VM download log selects ISO artifact `11748821992` and confirms the downloaded ZIP digest `b04e0360…6646`, binding this VM run to the independently rebuilt image. All 103 console checks passed, including Wayland/panel/launcher/unavailable UI, installed Debian sources, signed update, upgrade origin, live package removal, capture denial, missing-setcap fail-closed behavior and recovery.

Full downloaded job logs are retained in the local gitignored `out/review-evidence/`; the committed excerpts omit routine dependency installation and preserve the check results. The original author-produced runs and full image/VM evidence were inspected separately: `native_01bf7a1/`, ISO `f25bf75eb6357f76b787cd7a0630a457f7e30519438ba139ea1fa4e7ca4afd16`, live/install hash records, installed serial/results, original job logs and representative desktop/launcher/kill-switch screenshots. Original M0 run [38207816428](https://github.com/Kayyo321/cham3/actions/runs/38207816428) jobs were independently queried and confirmed successful; its image/boot was not independently rerun.

## Limits

Mocked/static, container, image and VM evidence have their stated scopes. BIOS/q35 QEMU does not establish UEFI or real hardware support, application safety or M2–M4 enforcement. Real-archive inputs can change; reproducibility is not claimed. The original installed VM had no newer wireshark-common version, so it tested reinstall/reconfigure/failure/recovery; the privileged rerun exercised a real version change. The installed-phase results do not contain an ISO hash: identity comes from the preceding install and shared disk in the same job.

## Review outcome

APPROVE the exact implementation target for the approved M1 development scope. F2 and F3 security approval explicitly GRANTED. F6/F3/F7/F8 closed. No implementation fixes or merge performed. Claude Code remains implementation owner. Review/evidence files in this directory are bound by `SHA256SUMS`; full transient logs remain in the local gitignored `out/review-evidence/`.
