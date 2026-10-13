# M2-005 — native amd64 evidence at b06d7cb (author VM)

- **Candidate:** b06d7cb21e1e3e42bbcd025abd9cea131574751d (range 6968bd6..b06d7cb: the implementation 6e9d6d3 and the fix b06d7cb). Built on M2-004 revision 2, whose re-review is pending.
- **Environment:**
  - Anthropic cloud VM, native x86_64, kernel 6.18.44.
  - `cham3-m2-dev` rebuilt from the unchanged Dockerfile in the clone, every step CACHED (`docker_build_candidate.txt`).
  - UID/GID 1000, clean clone at `/tmp/cham3_m2005_final`, `CHAM3_M2_REQUIRE_CLEAN=1`.
  - No `--privileged`, no added capabilities, no `--record`.
- **Classification:** author development evidence; CI is the second run ([CI evidence](../m2_005_amd64_ci_b06d7cb/)). Not an independent review.

Clone and run (exact):

~~~sh
git clone --no-hardlinks /home/user/cham3 /tmp/cham3_m2005_final
git -C /tmp/cham3_m2005_final checkout --detach b06d7cb21e1e3e42bbcd025abd9cea131574751d
chown -R 1000:1000 /tmp/cham3_m2005_final
docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
  -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
  -v /tmp/cham3_m2005_final:/work -w /work cham3-m2-dev bash -c \
  'bash build/scripts/qualify_m2_rust.sh; bash build/scripts/run_static_checks.sh; sh tests/run_tests.sh static'
~~~

| File | Content |
|---|---|
| `summary.txt`, `qualification_console.txt` | Every required check PASS, exit 0 (`m2 qualification: PASS`), including rustfmt (the only failure at 6e9d6d3). Miri, sanitizers and coverage-guided fuzzing are NOT_AVAILABLE (NOT_RUN, never PASS) |
| `identity.txt` | `dirty_tracked 0`; commit b06d7cb; Cargo.lock `364e9c58…256a`; EUID 1000 |
| `unit/` | **157 + 157** cargo tests, inventory-exact: 36 more than at ca40c62 (registryd unit 3, `admission_scenarios` 4, `registry_operations` 15; policyd unit 1, `current_authorization` 8, `end_to_end` 1; contractd `submission` 3; contract_format unit 1). Provider oracle 7 PASS (the libargon2 RFC 9106 secret/AD case NOT_CHECKED, as before). ECR oracle 19/96/18 with 22 scenarios / 346 steps. `format_differential.json`: 5 950 cases, 0 mismatches, 56 distinct outcomes, corpus `080d0c86…257d` (unchanged). `vault_object_oracle.log`: 38 checks PASS |
| `admission_scenarios_nocapture.log` | The layer-7 differential at b06d7cb (read-only clone, debug): **10 scenarios, 53 steps, 90 comparisons, 0 disagreements**; the 18 admission-invalid vectors and the published lineage |
| `harness/` | **135 tests**: `test_m2_registry_policy` 2, over **28** seeded registry/policy/contractd defects, each caught by its named Rust test while the unmodified copy passes; `test_m2_identity_vault` 2 (20 defects); 131 carried |
| `unchanged_inputs.txt` | Diffstat 6968bd6..b06d7cb (25 files). `git diff --exit-code` of the manifests, `.cargo`, CI, build, PROJECT, docs, vectors, every oracle, the harness library, the protection script, vaultd, the native crates, probes, every identityd module but `outcome.rs`, identityd's tests, and every contract_format module but `dcbor.rs`: exit 0 (29 paths). The full Cargo.lock diff: only dependency edges of workspace crates |
| `static_checks.log`, `static_tests.log`, `valgrind_*`, `profile_settings.log`, `rust_naming.log`, `crate_sources.log`, `dependency_inventory.md`, `toolchain.txt`, lock diffs | Static checks PASS; 82 static tests OK; the clone was clean afterwards (only ignored files). memcheck 0 errors. Empty package lock diffs; crate source manifest PASS |
| `attempts.md` | The first candidate 6e9d6d3: qualification and m2_checks failed only on rustfmt |

Limits: in-process composition; the concurrency test runs threads in one process, not separate service processes. No anchor, restriction ledger, crash recovery, lifecycle-record writers, trusted-time clock, IPC transport or service processes: NOT_RUN (M2-006, M2-007).
