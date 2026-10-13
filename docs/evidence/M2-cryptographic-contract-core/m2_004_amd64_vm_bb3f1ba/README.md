# M2-004 — native amd64 evidence at bb3f1ba (author VM)

- **Candidate:** bb3f1bad77080da34069af62c8d2cc390f2fcf7e on base 20c849b22ff7cb56c9266d447a28d6c692cbe532: implementation 0e4f993 plus fixes e5be206 and bb3f1ba ([attempts.md](attempts.md)).
- **Environment:**
  - Anthropic cloud VM, native x86_64, kernel 6.18.44.
  - `cham3-m2-dev` built from the unchanged Dockerfile, every step CACHED (`docker_build_candidate.txt`).
  - UID/GID 1000, clean clone at `/tmp/cham3_m2004_run`, `CHAM3_M2_REQUIRE_CLEAN=1`.
  - No `--privileged`, no added capabilities, no `--record`.
- **Classification:** author development evidence; CI is the second run ([CI evidence](../m2_004_amd64_ci_bb3f1ba/)). Not an independent review.

Clone and run (exact):

~~~sh
git clone --no-hardlinks /home/user/cham3 /tmp/cham3_m2004_run
git -C /tmp/cham3_m2004_run checkout --detach bb3f1bad77080da34069af62c8d2cc390f2fcf7e
chown -R 1000:1000 /tmp/cham3_m2004_run
docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
  -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
  -v /tmp/cham3_m2004_run:/work -w /work cham3-m2-dev bash -c \
  'bash build/scripts/qualify_m2_rust.sh; bash build/scripts/run_static_checks.sh; sh tests/run_tests.sh static'
~~~

| File | Content |
|---|---|
| `summary.txt`, `qualification_console.txt` | Every required check PASS, exit 0 (`m2 qualification: PASS`). Miri, sanitizers and coverage-guided fuzzing are NOT_AVAILABLE (NOT_RUN, never PASS) |
| `identity.txt` | `dirty_tracked 0`; Cargo.lock `da1a17c2…e28f`; EUID 1000 |
| `unit/` | **114 + 114** cargo tests, inventory-exact: 50 more than at M2-003 (identityd 21 new, 22 with its existing crate-name test, including `journal_scenarios` 2; vaultd 28 new, 29 in all; os_adapter 1). Provider oracle 7/7. ECR oracle 19/96/18 with 22 scenarios / 346 steps. `format_differential.json`: 5 950 cases, 0 mismatches, 56 distinct outcomes, corpus `080d0c86…257d` (unchanged). **`vault_object_oracle.log`: 38 checks plus the oracle's own AEAD control, all PASS** |
| `harness/` | **133 tests**: 130 carried, `test_m2_identity_vault` 2 (14 seeded defects, each caught by its named Rust test; the unmodified copy passes) and `test_m2_vault_protection` 1 (refusal under an 8 MiB memlock limit) |
| `protection.txt` | The refusal here; the positive path BLOCKED here (memlock cannot be raised in this VM) and PASS in CI with its report; the validation of the probe's mapping check with a negative control |
| `attempts.md` | What failed at 0e4f993 and e5be206 and how it was fixed |
| `unchanged_inputs.txt` | Diffstat 20c849b..bb3f1ba. `git diff --exit-code` of the workspace manifest, build inputs, PROJECT, docs, vectors, the ECR/format/provider oracles, the M2 harness library and every other crate: exit 0. Cargo.lock delta: only the dependency edges of identityd and vaultd |
| `static_checks.log`, `static_tests.log`, `valgrind_*`, `profile_settings.log`, `rust_naming.log`, `crate_sources.log`, `dependency_inventory.md`, lock diffs | Static checks PASS; 82 static tests OK; the clone was clean afterwards. memcheck 0 errors. Empty lock diffs; crate source manifest PASS |

Limits: in-process composition with labeled registry fixtures. IPC transport, service processes, registryd admission and real human presence are NOT_RUN (M2-005..M2-007, M5). Bounded seeded tests are not coverage-guided fuzzing.
