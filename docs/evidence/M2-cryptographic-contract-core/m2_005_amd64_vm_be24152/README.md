# M2-005 revision 3 — native amd64 evidence at be24152 (author VM)

- **Candidate:** be2415291bf5bcf9dad8df323bf971bd193d5120 (range 959524c..be24152, one commit: M2-005 R2-01, R2-02, R2-03 with their tests, seeded defects and harness).
- **Environment:**
  - Anthropic cloud VM, native x86_64, kernel 6.18.44.
  - `cham3-m2-dev` built in this VM from the unchanged Dockerfile (same image as the revision-2 evidence), UID/GID 1000.
  - Clean clone at `/tmp/cham3_m2005r3`, `CHAM3_M2_REQUIRE_CLEAN=1`; no `--privileged`, no added capabilities, no `--record`.
  - The full-filesystem check ran in the same clone with `--tmpfs /full-disk:rw,size=2m,uid=1000,gid=1000,mode=0700`.
- **Classification:** author development evidence; CI is the second run ([CI evidence](../m2_005_amd64_ci_be24152/)). Not an independent review.

Clone and run (exact):

~~~sh
git clone --no-hardlinks /home/user/cham3 /tmp/cham3_m2005r3
git -C /tmp/cham3_m2005r3 checkout --detach be2415291bf5bcf9dad8df323bf971bd193d5120
chown -R 1000:1000 /tmp/cham3_m2005r3
docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
  -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
  -v /tmp/cham3_m2005r3:/work -w /work cham3-m2-dev bash -c \
  'bash build/scripts/qualify_m2_rust.sh; bash build/scripts/run_static_checks.sh; sh tests/run_tests.sh static'
docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home \
  --tmpfs /full-disk:rw,size=2m,uid=1000,gid=1000,mode=0700 \
  -v /tmp/cham3_m2005r3:/work -w /work cham3-m2-dev sh tests/m2/storage/check_reserve_full_disk.sh out/m2/storage
docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/work/out/m2/differential_target \
  -v /tmp/cham3_m2005r3:/work -w /work cham3-m2-dev \
  cargo test --frozen -p cham3_registryd --test admission_scenarios -- --nocapture
~~~

| File | Content |
|---|---|
| `summary.txt`, `qualification_console.txt` | Every required check PASS, exit 0 (`m2 qualification: PASS`). Miri, sanitizers and coverage-guided fuzzing are NOT_AVAILABLE (NOT_RUN, never PASS). The console also holds the static checks (PASS), the 82 static tests (OK), the full-filesystem check and the layer-7 differential |
| `identity.txt` | `dirty_tracked 0`; commit be2415291bf5bcf9dad8df323bf971bd193d5120; Cargo.lock unchanged (`364e9c58…256a`); EUID 1000 |
| `unit/` | **169 + 169** cargo tests, inventory-exact: 3 more than at 051b4c8 (registryd `registry_operations` +3). Provider oracle 7 PASS. ECR oracle 19/96/18 with 23 scenarios / 352 steps (unchanged). `format_differential.json`: 5 955 cases, 0 mismatches, 56 distinct outcomes, corpus `0ad8bc2b…d395e` (unchanged). `vault_object_oracle.log`: 38 checks PASS |
| `admission_scenarios_nocapture.log` | The layer-7 differential (debug): 11 scenarios, 59 steps, 105 comparisons, 0 disagreements (unchanged) |
| `harness/` | **142 tests** (831.7 s): `test_m2_registry_policy` 2 over **41** seeded registry/policy/contractd defects (5 new), `test_m2_identity_vault` 2 over 21, `test_m2_reserve_probe` **7** (5 new, including the two seeded probe defects and their control), 131 carried |
| `storage/` | `reserve_full_disk_release.txt`, `reserve_full_disk_debug.txt`: the probe on an actually full 2 MiB tmpfs — preallocated, restrictive append durable from the reserve (seq 35), admission refused while depleted, second restrictive append (36), refilled after space returned (37), journal chained; the script's new leftover check passed; `restrictive reserve under a full filesystem: PASS` |
| `reviewer_reproductions/` | The re-review's ten tests (`edge_`) and probe matrix, byte-identical to the record's appendix (digests in `source_digests_and_commands.txt`): at 051b4c8 **4 passed / 6 failed** and `sentinel=deleted` (debug and release); at be24152 **10 passed / 0 failed** and `sentinel=preserved`, no root left behind by the unknown mode, the overlay refusal or the 32 MiB tmpfs refusal. The first review's nine `own_` tests and `filesystem_full_reserve` at be24152: 9/0 and 1/0, debug and release |
| `unchanged_inputs.txt` | Diffstat 959524c..be24152 (seven files). `git diff --exit-code` per path: exit 0 for the manifests, `Cargo.lock`, `.cargo`, build, PROJECT, the specifications and ADRs, `registry.rs`, `lineage.rs`, `record.rs`, the admission differential and builders, policyd, vaultd, identityd, contractd, contract_format, the native crates, the oracle and vectors, the harness library and dispatcher, the other harness modules, the protection check and the workflows |
| static checks and static tests | in `qualification_console.txt`: `static checks: PASS`; `Ran 82 tests … OK`; the clone was clean afterwards |
| `valgrind_*`, `profile_settings.log`, `rust_naming.log`, `crate_sources.log`, `dependency_inventory.md`, `toolchain.txt`, lock diffs | memcheck 0 errors; empty package lock diffs; crate source manifest PASS |

Limits: in-process composition; no anchor, restriction ledger, crash recovery, lifecycle-record writers, trusted-time clock, IPC transport or service processes (M2-006, M2-007). The reserve's lazy replenishment is not yet reported by a status operation; after a restart `reserve_bytes` is 0 until the start-time sync or the next ordinary append succeeds.
