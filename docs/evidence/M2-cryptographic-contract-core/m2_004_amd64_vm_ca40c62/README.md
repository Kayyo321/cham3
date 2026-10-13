# M2-004 revision 2 — native amd64 evidence at ca40c62 (author VM)

- **Candidate:** ca40c62ed7a5fc29ba8910fef514f3ce29ea4fba (parent d2c29ad7a09b2b1d49a23e9fa92c9656aa74c711). It answers the independent review at f2d82b7 ([finding resolutions](../../../reviews/M2-004-finding-resolutions.md)).
- **Environment:** the same as for [bb3f1ba](../m2_004_amd64_vm_bb3f1ba/):
  - Anthropic cloud VM, native x86_64, kernel 6.18.44.
  - `cham3-m2-dev` rebuilt from the unchanged Dockerfile in the clone, every step CACHED (`docker_build_candidate.txt`).
  - UID/GID 1000, clean clone at `/tmp/cham3_m2004r2_run`, `CHAM3_M2_REQUIRE_CLEAN=1`.
  - No `--privileged`, no added capabilities, no `--record`.
- **Classification:** author development evidence; CI is the second run ([CI evidence](../m2_004_amd64_ci_ca40c62/)). Not an independent review.

Clone and run (exact):

~~~sh
git clone --no-hardlinks /home/user/cham3 /tmp/cham3_m2004r2_run
git -C /tmp/cham3_m2004r2_run checkout --detach ca40c62ed7a5fc29ba8910fef514f3ce29ea4fba
chown -R 1000:1000 /tmp/cham3_m2004r2_run
docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
  -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
  -v /tmp/cham3_m2004r2_run:/work -w /work cham3-m2-dev bash -c \
  'bash build/scripts/qualify_m2_rust.sh; bash build/scripts/run_static_checks.sh; sh tests/run_tests.sh static'
~~~

| File | Content |
|---|---|
| `summary.txt`, `qualification_console.txt` | Every required check PASS, exit 0 (`m2 qualification: PASS`). Miri, sanitizers and coverage-guided fuzzing are NOT_AVAILABLE (NOT_RUN, never PASS) |
| `identity.txt` | `dirty_tracked 0`; tree `f3ae3093…`; Cargo.lock `da1a17c2…e28f`, unchanged since bb3f1ba; EUID 1000 |
| `unit/` | **121 + 121** cargo tests, inventory-exact: 7 more than at bb3f1ba (identityd `service_operations` 1, vaultd `vault_operations` 3, `vault_recovery` 3). Provider oracle 7 PASS (the libargon2 RFC 9106 secret/AD case NOT_CHECKED, as before). ECR oracle 19/96/18 with 22 scenarios / 346 steps. `format_differential.json`: 5 950 cases, 0 mismatches, 56 distinct outcomes, corpus `080d0c86…257d` (unchanged). `vault_object_oracle.log`: 38 checks plus the oracle's own AEAD control, all PASS (key-object format unchanged) |
| `harness/` | **133 tests**: `test_m2_identity_vault` 2, now over **20** seeded defects (6 new, 2 re-anchored or redefined), each caught by its named Rust test while the unmodified copy passes; `test_m2_vault_protection` 1; 130 carried |
| `protection.txt` | Refusal PASS here; positive path BLOCKED here and PASS in CI with its report |
| `reviewer_reproductions/` | The review's own 17 tests (`review.rs` and its `Cargo.toml`, extracted unchanged from the review record; SHA-256 values in `script_sha256.txt` match the record), run against read-only clean clones in debug and release. **At bb3f1ba: 11 passed, 6 failed** in both profiles, as the review reported. **At ca40c62: 16 passed, 1 failed** in both: the five finding reproducers pass; the one failure is I01's stricter assertion (`defect_recovery_retry_authenticates_passphrase_before_naming`), which the review classifies as stronger than R14 and which this revision answers with documentation only. `journal_scenarios_nocapture.log`: 12 scenarios, 318 comparisons, 0 disagreements at ca40c62 |
| `unchanged_inputs.txt` | Diffstat bb3f1ba..ca40c62 (9 files; d2c29ad..ca40c62 the same 9). `git diff --exit-code` of the manifests, Cargo.lock, `.cargo`, CI, build, PROJECT, vectors, every oracle, the harness library, the protection script, every other crate, the unchanged identityd/vaultd modules (including the key-object, KDF, store, protection and probe sources), the specifications and the ADRs: exit 0 |
| `static_checks.log`, `static_tests.log`, `valgrind_*`, `profile_settings.log`, `rust_naming.log`, `crate_sources.log`, `dependency_inventory.md`, `toolchain.txt`, lock diffs | Static checks PASS; 82 static tests OK; the clone was clean afterwards. memcheck 0 errors. Dependency inventory and toolchain identical to bb3f1ba. Empty lock diffs; crate source manifest PASS |

Limits: in-process composition with labeled registry fixtures; the interleaving tests are in-process schedules, not socket races. IPC transport, service processes, registryd admission, the restriction ledger and real human presence are NOT_RUN (M2-005..M2-007, M5). Bounded seeded tests are not coverage-guided fuzzing.
