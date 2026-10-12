# M2-001 / M2-002 revision 4: native amd64 evidence at 9b1aa2b (author VM)

- **Candidate:** 9b1aa2b9d1435fb33c303ca7fe01ce7afcb2964a, tree cb9f15101455f6c670a5f901f1e829544d45ef7d (parent 42397304663bff6057e26118014c18e7b90a2040).
- **Environment:**
  - Host: Anthropic cloud VM, Ubuntu 24.04, native x86_64, kernel 6.18.44, Docker 29.8.2 (overlayfs).
  - Image: `cham3-m2-dev`, built from the unchanged `build/m2/dev_container.dockerfile`. Every build step was CACHED from the identical context (`docker_build_candidate.txt`). Debian 13.7 amd64.
  - Run: as UID/GID 1000, in a clean clone owned by UID 1000. No `--privileged`, no added capabilities, no `--record`.
- **Classification:** this is **author development evidence on amd64**. The second run is GitHub Actions ([CI evidence](../m2_001_amd64_ci_9b1aa2b/)). Neither run is an independent review.

Clone and run (exact):

~~~sh
git clone --no-hardlinks /home/user/cham3 /tmp/cham3_r4_run
git -C /tmp/cham3_r4_run checkout --detach 9b1aa2b9d1435fb33c303ca7fe01ce7afcb2964a
chown -R 1000:1000 /tmp/cham3_r4_run
docker build -f /tmp/cham3_r4_run/build/m2/dev_container.dockerfile -t cham3-m2-dev /tmp/cham3_r4_run/build/m2
docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
  -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
  -v /tmp/cham3_r4_run:/work -w /work cham3-m2-dev bash build/scripts/qualify_m2_rust.sh
# same container settings:
bash build/scripts/run_static_checks.sh
sh tests/run_tests.sh static
~~~

| File | Content |
|---|---|
| `summary.txt`, `qualification_console.txt` | Qualification: every required check PASS, exit 0. Miri, sanitizers and fuzzing are NOT_AVAILABLE (= NOT_RUN, never PASS). The summary differs from revision 3 only in the commit line, and the CI log's summary is identical |
| `identity.txt` | Clean candidate identity: `dirty_tracked 0`, Cargo.lock `fc87901c…10f6`, EUID 1000 |
| `installed_packages_diff.log`, `downloaded_debs_diff.log` | Empty diffs against the committed amd64 locks |
| `unit/` | Unit suite: 36 debug + 36 release cargo tests (inventory-exact), provider oracle 7/7, ECR oracle 18 valid / 90 invalid / 18 admission / **22 journal scenarios (346 steps)** |
| `harness/` | Harness suite: **119 tests** (102 carried + 17 new: 9 R14 and 8 R15) |
| `valgrind_*_summary.txt` | The release probe and the real secure-memory test executable (10 tests, 0 errors) under memcheck |
| `static_checks.log`, `static_tests.log` | shellcheck, py_compile, yamllint and the secret scan PASS; 82 static tests OK; the clone was still clean afterwards |
| `unchanged_inputs.txt` | `git diff --exit-code 84fa6ef 9b1aa2b -- core Cargo.toml Cargo.lock build .github PROJECT.md ADR-0001 tests/m2/run_m2_tests.sh tests/m2/lib` exit 0. Also a per-section comparison of `vectors.json`: only `journal_scenarios` changed — the 10 network/restore scenarios are identical, each of the 9 recovery scenarios differs in one step (key naming), and there are 3 new scenarios. Also the stat of the candidate commit |
| `review_tools/` | Preserved reviewer tools at the candidate:<br>• `independent_vectors.py`: exit 0 (its final line still hardcodes 17).<br>• `schema_holes.py`: exit 1 at `subject_binding_mismatch`.<br>• `r2_probes.py`: exit 1 at `network_grant_conflict`.<br>• `ecr_oracle.py check`: PASS.<br>Same results as revision 3 |
| `reviewer_r3_reproductions/` | The re-review f451f2e's own R14 static script and R15 `time_limits_probe.py`, extracted unchanged from the review record (`r14_static.sh` SHA-256 `0f548fe2…6460`, `time_limits_probe.py` `06ea9e62…0038`). Each was run at baseline 84fa6ef and at the candidate:<br>• **R14:** at 84fa6ef it prints `GAP …` (exit 0); at 9b1aa2b its first assertion fails (exit 1), so the gap is closed.<br>• **R15:** both commits print the two reproduced behaviors (exit 0). The time rule is intentionally unchanged; the false bounds were withdrawn and these behaviors are now stated limits (ADR 0007 §5.7) |

Limits: the oracle journal and interface models and the trusted-time model are executable statements of PROPOSED designs, not services. Every A04–A15 mechanism is NOT_RUN: vault, IPC, `name_recovery_keys` over real sockets, registry, anchors, ceremony, clock and enforcement.
