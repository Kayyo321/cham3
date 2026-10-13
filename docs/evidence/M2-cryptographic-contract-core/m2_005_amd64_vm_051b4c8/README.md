# M2-005 revision 2 and M2-004 revision 3 — native amd64 evidence at 051b4c8 (author VM)

- **Candidate:** 051b4c8aacdeecfabb23625f93dd16615aec39a0 (range 24a35c1914d787b936d14b37b500226dde51722f..051b4c8: the implementation 4f7080b with M2-005 R01–R03 and M2-004-R2-01, and 051b4c8 with the R03 seeded defects).
- **Environment:**
  - Anthropic cloud VM, native x86_64, kernel 6.18.44.
  - `cham3-m2-dev` built in this VM from the unchanged Dockerfile (pinned base `debian@sha256:913f67…`; see `attempts.md`), UID/GID 1000.
  - Clean clone at `/tmp/cham3_m2005r2final`, `CHAM3_M2_REQUIRE_CLEAN=1`; no `--privileged`, no added capabilities, no `--record`.
  - The full-filesystem check ran in the same clone with `--tmpfs /full-disk:rw,size=2m,uid=1000,gid=1000,mode=0700`.
- **Classification:** author development evidence; CI is the second run ([CI evidence](../m2_005_amd64_ci_051b4c8/)). Not an independent review.

Clone and run (exact):

~~~sh
git clone --no-hardlinks /home/user/cham3 /tmp/cham3_m2005r2final
git -C /tmp/cham3_m2005r2final checkout --detach 051b4c8aacdeecfabb23625f93dd16615aec39a0
chown -R 1000:1000 /tmp/cham3_m2005r2final
docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
  -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
  -v /tmp/cham3_m2005r2final:/work -w /work cham3-m2-dev bash -c \
  'bash build/scripts/qualify_m2_rust.sh; bash build/scripts/run_static_checks.sh; sh tests/run_tests.sh static'
docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home \
  --tmpfs /full-disk:rw,size=2m,uid=1000,gid=1000,mode=0700 \
  -v /tmp/cham3_m2005r2final:/work -w /work cham3-m2-dev sh tests/m2/storage/check_reserve_full_disk.sh out/m2/storage
~~~

| File | Content |
|---|---|
| `summary.txt`, `qualification_console.txt` | Every required check PASS, exit 0 (`m2 qualification: PASS`). Miri, sanitizers and coverage-guided fuzzing are NOT_AVAILABLE (NOT_RUN, never PASS) |
| `identity.txt` | `dirty_tracked 0`; commit 051b4c8aacdeecfabb23625f93dd16615aec39a0; Cargo.lock unchanged (`364e9c58…256a`); EUID 1000 |
| `unit/` | **166 + 166** cargo tests, inventory-exact: 9 more than at b06d7cb (registryd `registry_operations` +6, policyd `current_authorization` +1, vaultd `vault_operations` +2). Provider oracle 7 PASS. ECR oracle 19/96/18 with **23 scenarios / 352 steps**. `format_differential.json`: 5 955 cases (5 950 before: the five signed objects of the new scenario join the corpus, seed 3, corpus `0ad8bc2b…d395e`), 0 mismatches, 56 distinct outcomes. `vault_object_oracle.log`: 38 checks PASS |
| `admission_scenarios_nocapture.log` | The layer-7 differential (debug): **11 scenarios, 59 steps, 105 comparisons, 0 disagreements**; the 18 admission-invalid vectors and the published lineage |
| `harness/` | **137 tests**: `test_m2_registry_policy` 2 over **33** seeded registry/policy/contractd defects (8 new), `test_m2_identity_vault` 2 over **21** (1 new), `test_m2_reserve_probe` 2 (new), 131 carried |
| `storage/` | `reserve_full_disk_release.txt`, `reserve_full_disk_debug.txt`: the reserve probe on an actually full 2 MiB tmpfs — preallocated, restrictive append durable from the reserve, admission refused while depleted, second restrictive append, refilled after space returned, journal chained; `restrictive reserve under a full filesystem: PASS` |
| `reviewer_reproductions/` | The reviewers' own tests, reconstructed byte for byte from the records (digests in the resolutions): the nine M2-005 tests and `filesystem_full_reserve` at b06d7cb (5/4; FAIL) and at the candidate (9/0; PASS), debug and release; the twelve M2-004 revision-2 tests at ca40c62 (10/2) and at the candidate (12/0); the original seventeen at the candidate (16/1, I01) |
| `unchanged_inputs.txt` | Diffstat 24a35c1..051b4c8aacdeecfabb23625f93dd16615aec39a0. `git diff --exit-code` per path: exit 0 for the workspace manifests except `core/registryd/Cargo.toml`, `.cargo`, `Cargo.lock`, build, PROJECT, the specifications and ADRs, every oracle but `ecr_oracle.py`, the harness library, identityd, contract_format sources, contractd, the native crates, probes, vaultd except `service.rs`; the vectors differ only by the added scenario |
| `static_checks.log`, `static_tests.log`, `valgrind_*`, `profile_settings.log`, `rust_naming.log`, `crate_sources.log`, `dependency_inventory.md`, `toolchain.txt`, lock diffs | Static checks PASS; 82 static tests OK; the clone was clean afterwards (only ignored files). memcheck 0 errors. Empty package lock diffs; crate source manifest PASS |
| `attempts.md` | The superseded qualification run at the implementation commit 4f7080b (stopped before its suites when the candidate moved to 051b4c8; every earlier gate PASS) and the image build |

Limits: in-process composition; no anchor, restriction ledger, crash recovery, lifecycle-record writers, trusted-time clock, IPC transport or service processes (M2-006, M2-007). The reserve's lazy replenishment is not yet reported by a status operation.
