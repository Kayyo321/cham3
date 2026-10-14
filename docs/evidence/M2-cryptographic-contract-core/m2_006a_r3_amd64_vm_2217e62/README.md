# M2-006 stage A revision 3 — native amd64 evidence at 2217e62 (author, owner's home computer)

- **Candidate:** 2217e62ddf42c42573346155727c983d937addc0 (range fc147e0..2217e62, three commits: 9633ce3 the stage-A revision 3 answering review 7d570c1 — the anchor counter continuation and the anchored quarantine, the start marker after a ledger failure, the owed ledger invalidation and marker with their retries, the queue drained one anchored record at a time, the new tests and seeded defects, 8 files; a2d0ebd the owner-approved amd64 lock refresh for libcurl deb13u7; 2217e62 ADR 0006 §3 erratum E5; 13 files, none new).
- **Environment:**
  - The owner's home computer (the same machine the independent reviewer uses), native x86_64, Debian 13, kernel 7.1.13+deb13-amd64, Docker 26.1.5+dfsg1 (reached through the `docker` group with `sg docker`).
  - `cham3-m2-dev` rebuilt without cache from the unchanged Dockerfile (`docker build --no-cache --network host -f build/m2/dev_container.dockerfile -t cham3-m2-dev-lock20261014 build/m2`, tagged `cham3-m2-dev`), image id `2a0bcf342c57`. Its installed set and `.deb` digests are byte-identical to the image of GitHub Actions m2_checks 38577923955 and are the refreshed locks. The earlier image `4f4f684d76f7` (the reviewer's `cham3-m2-independent-r3`) qualified 9633ce3 here with the old locks before the refresh; that run is superseded by this one.
  - Run as this host's unprivileged UID/GID 1001 (`--user 1001:1001`; the clone is owned by it), with the container-only `--ulimit nofile=1024:1024` the reviewer also needs here for Valgrind. No `--privileged`, no added capabilities, no `--record`.
  - Clean clone at `/home/dev/cham3-m2006a-r3/qual`, `CHAM3_M2_REQUIRE_CLEAN=1`.
  - The full-filesystem check (journal reserve and ledger) and the five wrapper cases ran in the same clone with `--tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700`.
- **Classification:** author development evidence; CI is the second run ([CI evidence](../m2_006a_r3_amd64_ci_2217e62/)). Not an independent review.

Clone and run (exact; `uid`/`gid` are 1001):

~~~sh
git clone --no-hardlinks /home/dev/cham3 /home/dev/cham3-m2006a-r3/qual
git -C /home/dev/cham3-m2006a-r3/qual checkout --detach 2217e62ddf42c42573346155727c983d937addc0
docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
  -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
  -v /home/dev/cham3-m2006a-r3/qual:/work -w /work cham3-m2-dev bash -c \
  'bash build/scripts/qualify_m2_rust.sh; bash build/scripts/run_static_checks.sh; sh tests/run_tests.sh static'
docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home \
  --tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700 \
  -v /home/dev/cham3-m2006a-r3/qual:/work -w /work cham3-m2-dev sh tests/m2/storage/check_reserve_full_disk.sh out/m2/storage
docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home \
  --tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700 \
  -e CHAM3_M2_RESERVE_PROBE=/work/out/m2/storage/target/release/cham3_reserve_probe -e CHAM3_M2_WRAPPER_DISK=/full-disk \
  -v /home/dev/cham3-m2006a-r3/qual:/work -w /work cham3-m2-dev \
  python3 -I -m unittest discover -s tests/m2/harness -p test_m2_reserve_probe.py -k wrapper -v
docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/work/out/m2/differential_target \
  -v /home/dev/cham3-m2006a-r3/qual:/work -w /work cham3-m2-dev \
  cargo test --frozen -p cham3_registryd --test admission_scenarios -- --nocapture
~~~

| File | Content |
|---|---|
| `summary.txt`, `qualification_console.txt` | Every required check PASS, exit 0 (`m2 qualification: PASS`). Miri, sanitizers and coverage-guided fuzzing are NOT_AVAILABLE (NOT_RUN, never PASS). The console also holds the static checks (PASS) and the 82 static tests (OK) |
| `identity.txt` | `dirty_tracked 0`; commit 2217e62ddf42c42573346155727c983d937addc0, tree `5f82bac0…`; `Cargo.lock` unchanged (`9801453a…`, the same digest as at e34fd8a); EUID 1001 |
| `unit/` | **206 + 206** cargo tests, inventory-exact: 4 more than at e34fd8a (the four new `storage_integrity` tests). Provider oracle PASS. ECR oracle 19/96/18 with 23 scenarios / 352 steps (unchanged). `format_differential.json`: 5 955 cases, 0 mismatches, 56 distinct outcomes, corpus `0ad8bc2b…d395e` (unchanged). `vault_object_oracle.log`: 38 checks PASS (unchanged) |
| `admission_scenarios_nocapture.log` | The layer-7 differential (debug): 11 scenarios, 59 steps, 105 comparisons, 0 disagreements (unchanged) |
| `harness/` | **147 tests** (384.6 s): `test_m2_registry_policy` 2 over **87** seeded registry/policy/contractd defects (6 new for R2-01–R2-04, two anchors re-pointed), `test_m2_identity_vault` 2 over 21, `test_m2_reserve_probe` 12, 131 carried |
| `storage/reserve_full_disk_{release,debug}.txt` | The probe on an actually full 2 MiB tmpfs, with the ledger: `restrictive reserve under a full filesystem: PASS`. The release report is byte-identical to the e34fd8a run (the probe's ledger invalidation succeeds at its first attempt, so the new retry is not exercised there) |
| `storage/wrapper_preservation_on_tmpfs.txt` | The five wrapper cases against fresh subdirectories of the tmpfs after the positive path: 5/5 OK, exit 0 |
| `reviewer_reproductions/` | The reviewer's three test sources, unchanged (digests as in the review record; the queue tests appended to the e34fd8a `storage_integrity.rs` as the reviewer ran them), at e34fd8a (storage 3/6, queue 1/4, matrix 4/4: R2-01, R2-02, R2-03 and R2-04 present) and at 9633ce3 and 2217e62 (storage 5/6: only the case erratum E5 excludes; queue 3/4: only the partial-flush test's premise assertion; matrix 4/4), debug and release; the commands and log digests in its README |
| `unchanged_inputs.txt` | The three commits; diffstat fc147e0..2217e62 (13 files) and 9633ce3..2217e62 (the lock lines and the ADR text, no code); the lock diff itself. `git diff --exit-code` per path: exit 0 for the workspace manifest and lock, `.cargo`, the Dockerfile, the pinned package list, the crate sources, the arm64 locks, the build scripts, PROJECT, every other ADR, the oracle and vectors, the harness library and dispatcher, the protection check, the storage probe and wrapper, the static tests, the workflows, `contract_format`, `identityd`, `vaultd`, `contractd`, the native crates, `policyd`, the probe binary, `record.rs`, `lineage.rs`, `journal.rs`, the registryd manifest, `registry_operations.rs`, `admission_scenarios.rs` and the builders |
| static checks and static tests | in `qualification_console.txt`: `static checks: PASS`; `Ran 82 tests … OK`; the clone was clean afterwards (0 dirty paths) |
| `valgrind_*`, `profile_settings.log`, `rust_naming.log`, `crate_sources.log`, `dependency_inventory.md`, `toolchain.txt`, lock diffs | memcheck 0 errors; both package lock checks PASS against the refreshed locks (empty diffs); crate source manifest PASS; `checked 63 Rust files, 72 paths: PASS` |

Limits: stage A of M2-006 (see the handoff and [storage formats §7](../../../specs/m2_registry_storage_v1.md)): the anchor has no reserve; no real crash tests (faults are injected at the io boundaries, restarts and reboots are simulated); the R2-03 window (a process death after a simultaneous failure of the ledger append, both invalidations, the anchor and the journal, before any retry succeeds, followed by a same-boot restart) is the owner-accepted limit of ADR 0006 §3 erratum E5; the arm64 development locks are not refreshed; `recover_user`, the ceremony, deletion, trusted time and the lifecycle writers are stages B–C; the ledger's tmpfs placement, transport and processes are M2-007. The vault protection positive path is not run here (it needs a raised memlock limit; CI covers it).
