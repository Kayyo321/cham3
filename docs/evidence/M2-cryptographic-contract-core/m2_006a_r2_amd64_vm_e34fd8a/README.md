# M2-006 stage A revision 2 — native amd64 evidence at e34fd8a (author, owner's home computer)

- **Candidate:** e34fd8a0b97f5a606656ab87914d4d1b19517d7c (range a9e47e4..e34fd8a, one commit: the stage-A revision 2 answering review 344c372 — the anchor-failure contract, the ledger replay, the policy publication, the identity-count and anchored-state rows, the unverifiable run marker, the new tests and seeded defects; 10 files, none new).
- **Environment:**
  - The owner's home computer (the same machine the independent reviewer uses), native x86_64, Debian 13, kernel 7.1.13+deb13-amd64, Docker 26.1.5+dfsg1.
  - `cham3-m2-dev` built on this machine from the unchanged Dockerfile (`docker build --network host -f build/m2/dev_container.dockerfile -t cham3-m2-dev build/m2`; the default build network had no DNS here, the host network was used for the package downloads; every layer was found in the daemon's build cache from the reviewer's earlier builds of the same Dockerfile, so the image id is `4f4f684d76f7`, identical to the reviewer's `cham3-m2-independent-r3`).
  - Run as this host's unprivileged UID/GID 1001 (`--user 1001:1001`; the clone is owned by it), with the container-only `--ulimit nofile=1024:1024` the reviewer also needs here for Valgrind. No `--privileged`, no added capabilities, no `--record`.
  - Clean clone at `/home/dev/cham3-m2006a-r2/qual`, `CHAM3_M2_REQUIRE_CLEAN=1`.
  - The full-filesystem check (journal reserve and ledger) and the five wrapper cases ran in the same clone with `--tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700`.
- **Classification:** author development evidence; CI is the second run ([CI evidence](../m2_006a_r2_amd64_ci_e34fd8a/)). Not an independent review.

Clone and run (exact; `uid`/`gid` are 1001):

~~~sh
git clone --no-hardlinks /home/dev/cham3 /home/dev/cham3-m2006a-r2/qual
git -C /home/dev/cham3-m2006a-r2/qual checkout --detach e34fd8a0b97f5a606656ab87914d4d1b19517d7c
docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
  -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
  -v /home/dev/cham3-m2006a-r2/qual:/work -w /work cham3-m2-dev bash -c \
  'bash build/scripts/qualify_m2_rust.sh; bash build/scripts/run_static_checks.sh; sh tests/run_tests.sh static'
docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home \
  --tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700 \
  -v /home/dev/cham3-m2006a-r2/qual:/work -w /work cham3-m2-dev sh tests/m2/storage/check_reserve_full_disk.sh out/m2/storage
docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home \
  --tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700 \
  -e CHAM3_M2_RESERVE_PROBE=/work/out/m2/storage/target/release/cham3_reserve_probe -e CHAM3_M2_WRAPPER_DISK=/full-disk \
  -v /home/dev/cham3-m2006a-r2/qual:/work -w /work cham3-m2-dev \
  python3 -I -m unittest discover -s tests/m2/harness -p test_m2_reserve_probe.py -k wrapper -v
docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/work/out/m2/differential_target \
  -v /home/dev/cham3-m2006a-r2/qual:/work -w /work cham3-m2-dev \
  cargo test --frozen -p cham3_registryd --test admission_scenarios -- --nocapture
~~~

| File | Content |
|---|---|
| `summary.txt`, `qualification_console.txt` | Every required check PASS, exit 0 (`m2 qualification: PASS`). Miri, sanitizers and coverage-guided fuzzing are NOT_AVAILABLE (NOT_RUN, never PASS). The console also holds the static checks (PASS) and the 82 static tests (OK) |
| `identity.txt` | `dirty_tracked 0`; commit e34fd8a0b97f5a606656ab87914d4d1b19517d7c; `Cargo.lock` unchanged (same digest as at f4b0628); EUID 1001 |
| `unit/` | **202 + 202** cargo tests, inventory-exact: 14 more than at f4b0628 (`storage_integrity` 11 new and one renamed, `current_authorization` +2, the anchor unit test +1). Provider oracle 7 PASS. ECR oracle 19/96/18 with 23 scenarios / 352 steps (unchanged). `format_differential.json`: 5 955 cases, 0 mismatches, 56 distinct outcomes, corpus `0ad8bc2b…d395e` (unchanged). `vault_object_oracle.log`: 38 checks PASS (unchanged) |
| `admission_scenarios_nocapture.log` | The layer-7 differential (debug): 11 scenarios, 59 steps, 105 comparisons, 0 disagreements (unchanged) |
| `harness/` | **147 tests** (368.6 s): `test_m2_registry_policy` 2 over **81** seeded registry/policy/contractd defects (19 new, the stage-A review findings and open fault cases), `test_m2_identity_vault` 2 over 21, `test_m2_reserve_probe` 12, 131 carried |
| `storage/reserve_full_disk_{release,debug}.txt` | The probe on an actually full 2 MiB tmpfs: reserve preallocated, the ledger started, restrictive append durable from the reserve (seq 35), admission refused while depleted, second restrictive append (36), the ledger append refused and the ledger failed with its validity removed, refilled after space returned (37), journal chained, the ledger still failed; `restrictive reserve under a full filesystem: PASS` (unchanged from f4b0628: the probe and its wrapper are untouched) |
| `storage/wrapper_preservation_on_tmpfs.txt` | The five wrapper cases against fresh subdirectories of the tmpfs after the positive path: 5/5 OK, exit 0, the tmpfs empty afterwards |
| `reviewer_reproductions/` | The reviewer's two test sources, unchanged (digests as in the review record), at f4b0628 (`reviewer_storage` 0/8 pass, `policy_review` adverse 2/2 pass: the defects present) and at e34fd8a (8/8 pass, adverse 0/2: gone), debug and release; the commands and log digests in its README |
| `unchanged_inputs.txt` | Diffstat a9e47e4..e34fd8a (10 files). `git diff --exit-code` per path: exit 0 for the workspace manifest and lock, `.cargo`, build, PROJECT, the ADRs, the oracle and vectors, the harness library and dispatcher, the protection check, the storage probe and wrapper, the static tests, the workflows, `contract_format`, `identityd`, `vaultd`, `contractd`, the native crates, the probe binary, `record.rs`, `lineage.rs`, `policyd/src`, `registry_operations.rs`, `admission_scenarios.rs` and the builders |
| static checks and static tests | in `qualification_console.txt`: `static checks: PASS`; `Ran 82 tests … OK`; the clone was clean afterwards |
| `valgrind_*`, `profile_settings.log`, `rust_naming.log`, `crate_sources.log`, `dependency_inventory.md`, `toolchain.txt`, lock diffs | memcheck 0 errors; empty package lock diffs; crate source manifest PASS |

Limits: stage A of M2-006 (see the handoff and [storage formats §7](../../../specs/m2_registry_storage_v1.md)): the anchor has no reserve; no real crash tests (faults are injected at the io boundaries, restarts and reboots are simulated); the recorded residual of a ledger failure whose validity file can neither be unlinked nor emptied while the anchor cannot be written either; `recover_user`, the ceremony, deletion, trusted time and the lifecycle writers are stages B–C; the ledger's tmpfs placement, transport and processes are M2-007. The vault protection positive path is not run here (it needs a raised memlock limit; CI covers it).
