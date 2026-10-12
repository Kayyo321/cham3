# M2-001 / M2-002 revision 3 — native amd64 evidence at 84fa6ef (author VM)

- **Candidate:** 84fa6ef172b3fad21bb079549776252bb65624d6, tree 7e3bc1a4cedfff60ad437deca05428c45c26dcdc (parent 6b4ef9d87d69185ff7205b51fa417df1fed5c6f6).
- **Environment:** Anthropic cloud VM, Ubuntu 24.04 host, native x86_64, kernel 6.18.44, Docker 29.8.2 (overlayfs). Image `cham3-m2-dev` built from the unchanged `build/m2/dev_container.dockerfile` (every layer of the candidate build CACHED from the baseline build of the identical context, `docker_build_candidate.txt`), Debian 13.7 amd64. Runs as UID/GID 1000 in a clean clone owned by UID 1000; no `--privileged`, no added capabilities, no `--record`. This is **author development evidence on amd64**; the second run is GitHub Actions ([CI evidence](../m2_001_amd64_ci_84fa6ef/)). Neither is independent review.

Clone and run (exact):

~~~sh
git clone --no-hardlinks /home/user/cham3 /tmp/cham3_run
git -C /tmp/cham3_run checkout --detach 84fa6ef172b3fad21bb079549776252bb65624d6
chown -R 1000:1000 /tmp/cham3_run
docker build -f /tmp/cham3_run/build/m2/dev_container.dockerfile -t cham3-m2-dev /tmp/cham3_run/build/m2
docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
  -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
  -v /tmp/cham3_run:/work -w /work cham3-m2-dev bash build/scripts/qualify_m2_rust.sh
# same container settings:
bash build/scripts/run_static_checks.sh
sh tests/run_tests.sh static
~~~

| File | Content |
|---|---|
| `summary.txt`, `qualification_console.txt` | Qualification: every required check PASS, exit 0; Miri/sanitizers/fuzzing NOT_AVAILABLE (= NOT_RUN, never PASS). Byte-identical to the CI summary |
| `identity.txt` | clean candidate identity (`dirty_tracked 0`, Cargo.lock `fc87901c…10f6`, EUID 1000) |
| `installed_packages_diff.log`, `downloaded_debs_diff.log` | empty diffs against the committed amd64 locks |
| `unit/` | unit suite: 36 debug + 36 release cargo tests (inventory-exact), provider oracle 7/7, ECR oracle 18 valid / 90 invalid / 18 admission / 19 journal scenarios (256 steps) |
| `harness/` | harness suite: 102 tests (70 carried + 32 new: 7 R01-r2 canaries, 12 trusted-time, 11 review regressions/defect sensitivity, 2 vector sensitivity) |
| `valgrind_*_summary.txt` | release probe and the real secure-memory test executable (10 tests, 0 errors) under memcheck |
| `static_checks.log`, `static_tests.log` | shellcheck/py_compile/yamllint/secret scan PASS; 82 static tests OK |
| `review_tools/` | reviewers' tools at the candidate: `independent_vectors.py` exit 0 (all 18 valid objects); `schema_holes.py` exit 1 at its first case (`subject_binding_mismatch`); `r2_probes.py` exit 1 at the sibling case (`network_grant_conflict`) after its four original rejections; `ecr_oracle.py check` PASS |
| `r01_r2/` | the re-review's failing-`sha256sum` reproduction at the candidate (exit 1, `cargo_lock_sha256 FAILED`); the new canaries (test module byte-identical to the candidate's) run in the same image against the 6b4ef9d `m2_source_identity.sh` and `m2_harness.py` (5 failures + 1 error = the 6 negatives fail as required; the positive control passes) |
| `baseline_6b4ef9d/` | the same reproductions on the unchanged baseline before any change: identity PASS with an empty digest; `r2_probes.py` ACCEPTED both new cases |

Limits: the oracle journal model and the trusted-time model are executable statements of PROPOSED designs, not services; every A04–A15 mechanism (vault, IPC, registry, anchors, ceremony, clock, enforcement) is NOT_RUN.
