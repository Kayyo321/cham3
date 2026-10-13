# M2-003 revision 2 — native amd64 evidence at 8b0ccf3 (author VM)

- **Candidate:** 8b0ccf37f2437cae0a9668e012ddf55e9952e2aa (parent f87111d021a743b6937dcc5a6c0efab4310f135b). It answers the independent review at 1cb93c0 with owner errata E1–E3.
- **Environment:** the same as for [4841039](../m2_003_amd64_vm_4841039/):
  - Anthropic cloud VM, native x86_64, kernel 6.18.44.
  - `cham3-m2-dev` built from the unchanged Dockerfile, every step CACHED (`docker_build_candidate.txt`).
  - UID/GID 1000, clean clone at `/tmp/cham3_m2003r2_run`, `CHAM3_M2_REQUIRE_CLEAN=1`.
  - No `--privileged`, no added capabilities, no `--record`.
  - Commands as in the [4841039 README](../m2_003_amd64_vm_4841039/README.md), with this clone and commit.
- **Classification:** author development evidence; CI is the second run ([CI evidence](../m2_003_amd64_ci_8b0ccf3/)). Not an independent review.

| File | Content |
|---|---|
| `summary.txt`, `qualification_console.txt` | Every required check PASS, exit 0. Miri, sanitizers and coverage-guided fuzzing are NOT_AVAILABLE (NOT_RUN). |
| `identity.txt` | `dirty_tracked 0`; Cargo.lock `49ab393a…4518`, unchanged since 4841039; EUID 1000. |
| `unit/` | **64 + 64** cargo tests: `ecr_vectors` 11, `schema_mutations` 7, crate unit tests 11, 35 carried. Provider oracle 7/7. ECR oracle **19/96/18** with 22 scenarios / 346 steps. `format_differential.json`: **5 950 cases, 0 mismatches**, 56 distinct outcomes, corpus SHA-256 `080d0c86…257d`. |
| `harness/` | **130 tests**: 128 carried plus `test_short_container_bodies_reserve_no_declared_capacity` and `test_count_driven_reservation_is_detected` (Massif). 8 seeded defects are detected. |
| `reviewer_reproductions/` | The review's own `adversarial.py` and `allocations.py`, extracted unchanged from the review record (SHA-256 values in `script_sha256.txt` match the record), run at 4841039 and 8b0ccf3. **adversarial:** 266 cases, 0 Rust/oracle mismatches and 6 prose mismatches at 4841039; 270 cases, 0 mismatches and 2 prose mismatches at 8b0ccf3. The remaining 2 are the script's hard-coded pre-erratum `list_too_long` expectations, superseded by owner erratum E1. **allocations (Massif peak heap):** 25 629 / 33 821 bytes at 4841039 become 17 477 / 17 477 bytes at 8b0ccf3, the same as an empty container. |
| `unchanged_inputs.txt` | Diffstat f87111d..8b0ccf3. `git diff --exit-code` of the manifests, Cargo.lock, build, CI, PROJECT, other crates and the IPC spec: exit 0. Dependency inventory identical to 4841039. Section comparison of `vectors.json`: 7 added vectors, `typ_unknown`'s expected code and description changed with identical bytes, every other section identical. |
| `static_checks.log`, `static_tests.log`, `valgrind_*`, `profile_settings.log`, `rust_naming.log`, `crate_sources.log`, `dependency_inventory.md`, lock diffs | Static checks PASS; 82 static tests OK; the clone was clean afterwards. memcheck 0 errors; empty lock diffs. |

Limits: layers 1–5 only. Bounded seeded tests are not coverage-guided fuzzing. Key authority, admission, authorization and services are NOT_RUN.
