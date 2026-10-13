# M2-005 revision 4 — native amd64 evidence at 5860f27 (author VM)

- **Candidate:** 5860f276ab593da34771f704691ea35649324d6e (range ad18d8b..5860f27, one commit: M2-005 R3-01 — the full-disk wrapper owns nothing on the disk; five wrapper preservation cases; the CI wrapper step; inventory +5). Four files, no Rust.
- **Environment:**
  - Anthropic cloud VM, native x86_64, kernel 6.18.44.
  - `cham3-m2-dev` built in this VM from the unchanged Dockerfile (same image as the revision-3 evidence), UID/GID 1000.
  - Clean clone at `/tmp/cham3_m2005r4`, `CHAM3_M2_REQUIRE_CLEAN=1`; no `--privileged`, no added capabilities, no `--record`.
  - The full-filesystem check and the five wrapper cases ran in the same clone with `--tmpfs /full-disk:rw,size=2m,uid=1000,gid=1000,mode=0700`.
- **Classification:** author development evidence; CI is the second run ([CI evidence](../m2_005_amd64_ci_5860f27/)). Not an independent review.

Clone and run (exact):

~~~sh
git clone --no-hardlinks /home/user/cham3 /tmp/cham3_m2005r4
git -C /tmp/cham3_m2005r4 checkout --detach 5860f276ab593da34771f704691ea35649324d6e
chown -R 1000:1000 /tmp/cham3_m2005r4
docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
  -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
  -v /tmp/cham3_m2005r4:/work -w /work cham3-m2-dev bash -c \
  'bash build/scripts/qualify_m2_rust.sh; bash build/scripts/run_static_checks.sh; sh tests/run_tests.sh static'
docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home \
  --tmpfs /full-disk:rw,size=2m,uid=1000,gid=1000,mode=0700 \
  -v /tmp/cham3_m2005r4:/work -w /work cham3-m2-dev sh tests/m2/storage/check_reserve_full_disk.sh out/m2/storage
docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home \
  --tmpfs /full-disk:rw,size=2m,uid=1000,gid=1000,mode=0700 \
  -e CHAM3_M2_RESERVE_PROBE=/work/out/m2/storage/target/release/cham3_reserve_probe -e CHAM3_M2_WRAPPER_DISK=/full-disk \
  -v /tmp/cham3_m2005r4:/work -w /work cham3-m2-dev \
  python3 -I -m unittest discover -s tests/m2/harness -p test_m2_reserve_probe.py -k wrapper -v
docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/work/out/m2/differential_target \
  -v /tmp/cham3_m2005r4:/work -w /work cham3-m2-dev \
  cargo test --frozen -p cham3_registryd --test admission_scenarios -- --nocapture
~~~

| File | Content |
|---|---|
| `summary.txt`, `qualification_console.txt` | Every required check PASS, exit 0 (`m2 qualification: PASS`). Miri, sanitizers and coverage-guided fuzzing are NOT_AVAILABLE (NOT_RUN, never PASS). The console also holds the static checks (PASS), the 82 static tests (OK), the full-filesystem check and the layer-7 differential |
| `identity.txt` | `dirty_tracked 0`; commit 5860f276ab593da34771f704691ea35649324d6e; Cargo.lock unchanged (`364e9c58…256a`); EUID 1000 |
| `unit/` | **169 + 169** cargo tests, inventory-exact (unchanged from be24152: no Rust changed). Provider oracle 7 PASS. ECR oracle 19/96/18 with 23 scenarios / 352 steps (unchanged). `format_differential.json`: 5 955 cases, 0 mismatches, 56 distinct outcomes, corpus `0ad8bc2b…d395e` (unchanged). `vault_object_oracle.log`: 38 checks PASS |
| `admission_scenarios_nocapture.log` | The layer-7 differential (debug): 11 scenarios, 59 steps, 105 comparisons, 0 disagreements (unchanged) |
| `harness/` | **147 tests** (859.8 s), 5 more than at be24152: `test_m2_reserve_probe` **12** (`full_disk_wrapper_t` 3 and `seeded_wrapper_defects_t` 2 new, against the dispatcher's probe build on the overlay), `test_m2_registry_policy` 2 over 41 seeded registry/policy/contractd defects, `test_m2_identity_vault` 2 over 21, 131 carried |
| `storage/reserve_full_disk_{release,debug}.txt` | The probe on an actually full 2 MiB tmpfs — preallocated, restrictive append durable from the reserve (seq 35), admission refused while depleted, second restrictive append (36), refilled after space returned (37), journal chained; the leftover checks (now including symbolic links) passed; `restrictive reserve under a full filesystem: PASS` |
| `storage/wrapper_preservation_on_tmpfs.txt` | The five wrapper cases run against fresh subdirectories of the 2 MiB tmpfs after the positive path, in the clean clone: 5/5 OK, exit 0, the tmpfs empty afterwards (`leftovers:` lists nothing) |
| `reviewer_reproductions/` | The re-review's wrapper scenario (`wrapper_run.py`, as a shell script of the same steps; `commands.txt`) with the candidate's probe build against a synthetic disk holding an existing root with a sentinel and an existing filler: at be24152 **both paths deleted** (wrapper exit 1 after `refusing_to_fill`); at 5860f27 **`refusing: … exists`, wrapper exit 1, both paths preserved**, nothing built |
| `unchanged_inputs.txt` | Diffstat ad18d8b..5860f27 (four files). `git diff --exit-code` per path: exit 0 for the manifests, `Cargo.lock`, `.cargo`, build, PROJECT, `docs`, `core` (all Rust), the oracle and vectors, the harness library and dispatcher, the protection check, the static tests and the m0/m1 workflows |
| static checks and static tests | in `qualification_console.txt`: `static checks: PASS`; `Ran 82 tests … OK`; the clone was clean afterwards |
| `valgrind_*`, `profile_settings.log`, `rust_naming.log`, `crate_sources.log`, `dependency_inventory.md`, `toolchain.txt`, lock diffs | memcheck 0 errors; empty package lock diffs; crate source manifest PASS |

Limits: unchanged from revision 3 (in-process composition; no anchor, restriction ledger, crash recovery, lifecycle-record writers, trusted-time clock, IPC transport or service processes until M2-006 and M2-007; lazy reserve replenishment not yet reported by a status operation). The vault protection positive path is BLOCKED in this VM (the memlock limit cannot be raised) and is covered by CI.
