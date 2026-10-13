# M2-006 stage A — native amd64 evidence at f4b0628 (author VM)

- **Candidate:** f4b062826ae54ba4e79c5abd08eeb9fe21d48510 (range 22b6b5f..f4b0628, one commit: the anchor, the restriction ledger, the restart and recovery decisions, the restrictive write path, the policy state, the probe's ledger scenario, the stage-A tests and seeded defects; 22 files, 5 new).
- **Environment:**
  - Anthropic cloud VM, native x86_64, kernel 6.18.44.
  - `cham3-m2-dev` built in this VM from the unchanged Dockerfile (same image as the M2-005 evidence), UID/GID 1000.
  - Clean clone at `/tmp/cham3_m2006a`, `CHAM3_M2_REQUIRE_CLEAN=1`; no `--privileged`, no added capabilities, no `--record`.
  - The full-filesystem check (journal reserve and ledger) and the five wrapper cases ran in the same clone with `--tmpfs /full-disk:rw,size=2m,uid=1000,gid=1000,mode=0700`.
- **Classification:** author development evidence; CI is the second run ([CI evidence](../m2_006a_amd64_ci_f4b0628/)). Not an independent review.

Clone and run (exact):

~~~sh
git clone --no-hardlinks /home/user/cham3 /tmp/cham3_m2006a
git -C /tmp/cham3_m2006a checkout --detach f4b062826ae54ba4e79c5abd08eeb9fe21d48510
chown -R 1000:1000 /tmp/cham3_m2006a
docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
  -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
  -v /tmp/cham3_m2006a:/work -w /work cham3-m2-dev bash -c \
  'bash build/scripts/qualify_m2_rust.sh; bash build/scripts/run_static_checks.sh; sh tests/run_tests.sh static'
docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home \
  --tmpfs /full-disk:rw,size=2m,uid=1000,gid=1000,mode=0700 \
  -v /tmp/cham3_m2006a:/work -w /work cham3-m2-dev sh tests/m2/storage/check_reserve_full_disk.sh out/m2/storage
docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home \
  --tmpfs /full-disk:rw,size=2m,uid=1000,gid=1000,mode=0700 \
  -e CHAM3_M2_RESERVE_PROBE=/work/out/m2/storage/target/release/cham3_reserve_probe -e CHAM3_M2_WRAPPER_DISK=/full-disk \
  -v /tmp/cham3_m2006a:/work -w /work cham3-m2-dev \
  python3 -I -m unittest discover -s tests/m2/harness -p test_m2_reserve_probe.py -k wrapper -v
docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/work/out/m2/differential_target \
  -v /tmp/cham3_m2006a:/work -w /work cham3-m2-dev \
  cargo test --frozen -p cham3_registryd --test admission_scenarios -- --nocapture
~~~

| File | Content |
|---|---|
| `summary.txt`, `qualification_console.txt` | Every required check PASS, exit 0 (`m2 qualification: PASS`). Miri, sanitizers and coverage-guided fuzzing are NOT_AVAILABLE (NOT_RUN, never PASS). The console also holds the static checks (PASS), the 82 static tests (OK), the full-filesystem check and the layer-7 differential |
| `identity.txt` | `dirty_tracked 0`; commit f4b062826ae54ba4e79c5abd08eeb9fe21d48510; Cargo.lock with the one added edge (`cham3_os_adapter` for registryd); EUID 1000 |
| `unit/` | **188 + 188** cargo tests, inventory-exact: 19 more than at 5860f27 (`storage_integrity` 12 new, `current_authorization` +3, `registry_operations` +1, the anchor and ledger unit tests +3). Provider oracle 7 PASS. ECR oracle 19/96/18 with 23 scenarios / 352 steps (unchanged). `format_differential.json`: 5 955 cases, 0 mismatches, 56 distinct outcomes, corpus `0ad8bc2b…d395e` (unchanged). `vault_object_oracle.log`: 38 checks PASS (unchanged) |
| `admission_scenarios_nocapture.log` | The layer-7 differential (debug): 11 scenarios, 59 steps, 105 comparisons, 0 disagreements (unchanged) |
| `harness/` | **147 tests** (997.2 s): `test_m2_registry_policy` 2 over **62** seeded registry/policy/contractd defects (21 new, stage A), `test_m2_identity_vault` 2 over 21, `test_m2_reserve_probe` 12, 131 carried |
| `storage/reserve_full_disk_{release,debug}.txt` | The probe on an actually full 2 MiB tmpfs — reserve preallocated, a ledger started on the same filesystem (`ledger_started`), restrictive append durable from the reserve (seq 35), admission refused while depleted, second restrictive append (36), the ledger append refused (`ledger_append_on_full_filesystem refused`, `ledger_failed`, `ledger_validity_removed`, `previous_ledger missing`), refilled after space returned (37), journal chained, the ledger still failed (`ledger_stays_failed`); `restrictive reserve under a full filesystem: PASS` |
| `storage/wrapper_preservation_on_tmpfs.txt` | The five wrapper cases against fresh subdirectories of the tmpfs after the positive path: 5/5 OK, exit 0, the tmpfs empty afterwards |
| `unchanged_inputs.txt` | Diffstat 22b6b5f..f4b0628 (22 files). `git diff --exit-code` per path: exit 0 for the workspace manifest, `.cargo`, build, PROJECT, the ADRs, the oracle and vectors, the harness library and dispatcher, the protection check, the static tests, the workflows, `contract_format`, `identityd`, `vaultd`, `contractd` and the native crates; the lock's one added line |
| static checks and static tests | in `qualification_console.txt`: `static checks: PASS`; `Ran 82 tests … OK`; the clone was clean afterwards |
| `valgrind_*`, `profile_settings.log`, `rust_naming.log`, `crate_sources.log`, `dependency_inventory.md`, `toolchain.txt`, lock diffs | memcheck 0 errors; empty package lock diffs; crate source manifest PASS |

Limits: stage A of M2-006 (see the handoff and [storage formats §7](../../../specs/m2_registry_storage_v1.md)): the anchor has no reserve; no real crash tests; `recover_user`, the ceremony, deletion, trusted time and the lifecycle writers are stages B–C; the ledger's tmpfs placement, transport and processes are M2-007. The vault protection positive path is BLOCKED in this VM (the memlock limit cannot be raised) and is covered by CI.
