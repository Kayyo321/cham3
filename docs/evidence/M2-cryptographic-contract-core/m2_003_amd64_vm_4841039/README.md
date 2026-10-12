# M2-003 — native amd64 evidence at 4841039 (author VM)

- **Candidate:** 4841039d772ca923e09a55f1505f119317e73a91, tree 9304e98acb96f5ec30a09831e10bb77f3a13e1bc (parent ecc821342a50668742d9f23f5065eeffe87a6a10).
- **Environment:** the same as the M2-001/M2-002 runs:
  - Anthropic cloud VM, native x86_64, kernel 6.18.44.
  - `cham3-m2-dev` built from the unchanged Dockerfile, every step CACHED (`docker_build_candidate.txt`), Debian 13.7 amd64.
  - UID/GID 1000, clean clone owned by UID 1000.
  - No `--privileged`, no added capabilities, no `--record`.
- **Classification:** author development evidence. The second run is GitHub Actions ([CI evidence](../m2_003_amd64_ci_4841039/)). Neither is an independent review.

Clone and run (exact):

~~~sh
git clone --no-hardlinks /home/user/cham3 /tmp/cham3_m2003_run
git -C /tmp/cham3_m2003_run checkout --detach 4841039d772ca923e09a55f1505f119317e73a91
chown -R 1000:1000 /tmp/cham3_m2003_run
docker build -f /tmp/cham3_m2003_run/build/m2/dev_container.dockerfile -t cham3-m2-dev /tmp/cham3_m2003_run/build/m2
docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
  -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
  -v /tmp/cham3_m2003_run:/work -w /work cham3-m2-dev bash build/scripts/qualify_m2_rust.sh
# same container settings:
bash build/scripts/run_static_checks.sh
sh tests/run_tests.sh static
~~~

| File | Content |
|---|---|
| `summary.txt`, `qualification_console.txt` | Every required check PASS, exit 0 (`m2 qualification: PASS`). Miri, sanitizers and coverage-guided fuzzing are NOT_AVAILABLE (NOT_RUN, never PASS). The CI log's summary lines are the same |
| `identity.txt` | `dirty_tracked 0`, Cargo.lock `49ab393a…4518`, EUID 1000 |
| `installed_packages_diff.log`, `downloaded_debs_diff.log`, `crate_sources.log`, `dependency_inventory.md` | Empty lock diffs; crate source manifest PASS; the dependency inventory is byte-identical to revision 4.1 (`unchanged_inputs.txt`) |
| `unit/` | **60 debug + 60 release** cargo tests, inventory-exact: `cham3_contract_format` 11, `ecr_vectors` 9, `schema_mutations` 5, plus the 35 carried tests. Provider oracle 7/7. ECR oracle 18/90/18 with 22 scenarios (346 steps). **`format_differential.json`**: seed 3, 300 cases per object, 5 642 cases, **0 mismatches**, 56 distinct outcomes; coverage ok 494, envelope_decode 610, envelope 775, signature 247, payload_decode 619, schema 2 789, intrinsic 108; corpus SHA-256 `b714ad73…477f` |
| `harness/` | **128 tests** (120 carried + 8 new in `test_m2_format_verifier.py`, including the 5 seeded verifier defects, each detected) |
| `differential_seeds_100_109.txt` | Author exploration with the qualified probe (SHA-256 `dbc4f00b…5324`): 10 more seeds, 300 cases per object, about 5 640 cases each, 0 mismatches each |
| `oracle_before/` | The pre-fix oracle (87c9cbc, SHA-256 in `prefix_oracle_sha256.txt`) against the candidate probe, default seed and corpus: **4 mismatches** in 5 642 cases (`format_differential_prefix_oracle.json`). The old oracle accepted 2 objects with a time containing non-ASCII digits or a trailing newline, and gave a later error for 2 others, while the probe reported `invalid_time`/`invalid_text` as wire format §§3.2/3.4 require. `oracle_fix.diff` is the fix (fullmatch, `re.ASCII`). With it, 0 mismatches (`unit/`) |
| `valgrind_*_summary.txt`, `profile_settings.log`, `rust_naming.log` | memcheck 0 errors (10 secure-memory tests); profiles and naming PASS |
| `static_checks.log`, `static_tests.log` | Static checks PASS; 82 static tests OK; the clone was still clean afterwards |
| `unchanged_inputs.txt` | Cargo.lock delta: only the dependency edges of `cham3_contract_format` on `ed25519-dalek` and `sha2`. `git diff --exit-code ecc8213 4841039 -- Cargo.toml build .github PROJECT.md docs tests/m2/vectors` and the other crates exits 0: no specification, ADR, vector, build input or other crate changed. Full diffstat |

Limits: layers 1–5 only. The seeded mutation and differential tests are bounded deterministic testing, not coverage-guided fuzzing. Key authority, admission, current authorization, IPC and services are NOT_RUN (M2-004..M2-007).
