# M2-006 stage B1 — native amd64 evidence at c9129bc (author, owner's home computer)

- **Candidate:** c9129bca0e6c4b1e6caf72774e05a27e09a4d542 (range e7803ef..c9129bc, one commit): the stage-B1 trusted-time clock, the option-B observed start, the commit-timestamp rule, registry record time from trusted time, and the end of temporary branches at a new boot. 34 files, 5 of them new.
- **Environment:**
  - The owner's home computer (the same machine the independent reviewer uses), native x86_64, Debian 13.7, kernel 7.1.13+deb13-amd64, Docker reached through the `docker` group with `sg docker`.
  - Image `cham3-m2-dev`, id `2a0bcf342c57`: the no-cache rebuild with the refreshed amd64 locks, unchanged since stage A revision 3.
  - Run as UID/GID 1001 with the container-only `--ulimit nofile=1024:1024`. No `--privileged` and no added capabilities.
  - Clean clone at `/home/dev/cham3-m2006b1/qual`, run with `CHAM3_M2_REQUIRE_CLEAN=1`; the clone had 0 dirty paths before and after.
- **Classification:** author development evidence; CI is the second run ([CI evidence](../m2_006b1_amd64_ci_c9129bc/)). Not an independent review.

The commands are those of the [stage-A revision-4 evidence](../m2_006a_r4_amd64_vm_db5bb59/), with the clone path `/home/dev/cham3-m2006b1/qual`, the candidate c9129bca0e6c4b1e6caf72774e05a27e09a4d542, and one more differential run:

~~~sh
docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/work/out/m2/differential_target \
  -v /home/dev/cham3-m2006b1/qual:/work -w /work cham3-m2-dev \
  cargo test --frozen -p cham3_registryd --test trusted_time_model -- --nocapture
~~~

| File | Content |
|---|---|
| `summary.txt`, `qualification_console.txt` | Every required check PASS, exit 0 (`m2 qualification: PASS`). Miri, sanitizers and coverage-guided fuzzing are NOT_AVAILABLE, so NOT_RUN and never PASS. The console also holds the static checks (PASS) and the 82 static tests (OK). |
| `identity.txt` | `dirty_tracked 0`; commit c9129bc, tree `03f9d0dd…`; `Cargo.lock` unchanged (`9801453a…`); EUID 1001. |
| `unit/` | Cargo tests: **234 + 234**, inventory-exact (25 more than at db5bb59). |
| `unit/` | Provider oracle: PASS. |
| `unit/` | ECR oracle: 19 valid / 96 invalid / 18 admission vectors, with **27 scenarios / 365 steps** (4 new E4 scenarios). |
| `unit/` | `trusted_time_vectors.log`: the 165 published cases equal the model's output, PASS. |
| `unit/` | `format_differential.json`: 5 968 cases, 0 mismatches, 56 distinct outcomes, corpus `44ea2aa4…426c`. The corpus is derived from the vectors, so the 13 signed objects of the new scenarios add 13 cases. |
| `unit/` | `vault_object_oracle.log`: 38 checks PASS. |
| `admission_scenarios_nocapture.log` | The layer-7 differential: 15 scenarios, 72 steps, 121 comparisons, 0 disagreements. |
| `trusted_time_model_nocapture.log` | registryd's clock against the model's vectors: 165 cases, 4 813 steps, 0 disagreements. |
| `harness/` | **149 tests** (417.3 s). `test_m2_registry_policy`: 2 tests over **119** seeded defects (22 new). `test_m2_identity_vault`: 2 over 21. `test_m2_trusted_time`: 22 (2 new). `test_m2_reserve_probe`: 12. |
| `harness/defect_reasons_b1.log` | Each of the 22 new defects applied alone in a copy of the candidate, with its named test run. All 22 fail with exit 101, each at the intended assertion; the panic line and assertion are recorded. |
| `storage/` | The full-filesystem probe on an actually full 2 MiB tmpfs: PASS. Its reports are byte-identical to db5bb59's. The five wrapper cases: 5/5 OK. |
| `unchanged_inputs.txt` | The diffstat e7803ef..c9129bc. `git diff --exit-code` returns 0 for the manifests, the lock, `.cargo`, `build`, PROJECT, `docs/decisions`, the workflows, the sources of `contract_format`, `contractd`, `vaultd` and the native crates, `journal.rs`, the probe binary, the harness library and the storage wrapper. `trusted_time_model.py` differs in docstrings only. |
| `valgrind_*`, `profile_settings.log`, `rust_naming.log`, `crate_sources.log`, `dependency_inventory.md`, `toolchain.txt`, lock diffs | Memcheck: 0 errors. Both package lock checks PASS (empty diffs). Crate sources PASS. Naming: `checked 66 Rust files, 75 paths: PASS`. |

**Limits:**
- The clocks in every test come from a settable source. The kernel source (`/proc/uptime` plus the standard library) runs in one unit test only.
- NOT_RUN: real hardware clocks, suspend, time synchronization, and reboots with the RTC set back.
- The ADR 0007 §5.7 limits stand.
- The vault protection positive path needs a raised memlock limit, so it is not run here; CI covers it.
