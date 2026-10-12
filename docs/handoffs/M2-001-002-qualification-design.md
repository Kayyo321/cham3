# Handoff: M2-001 / M2-002 — Rust qualification and G1–G6 design package

- **State:** M2-001 and M2-002 IMPLEMENTED → **IN_REVIEW**. M2-003..M2-008 remain DRAFT (no gate approved). Full M2 is not DONE.
- **Role:** IMPLEMENTER (and ARCHITECT for the PROPOSED ADRs). The author does not approve this work.
- **Model/provider:** Claude Code (Opus 5.5). Independent reviewer requested: Codex, in a cloud session.
- **Working directory / branch:** /Users/jessicabruce/cham3, `codex/m2-cryptographic-contract-core`.
- **Task specification:** [docs/tasks/M2-cryptographic-contract-core.md](../tasks/M2-cryptographic-contract-core.md) (packages M2-001, M2-002; gates G1–G6), [Claude prompt](../prompts/m2_claude_implementation.md).
- **Baseline commit:** a4bc0c4371f2298787f0876d81c0c6fd21150ea6 (clean, after the Rust-transition review). Earlier preparation baselines are recorded in [M2-rust-transition](M2-rust-transition.md).
- **Implementation commits:** 5d07ee13cb6bb032a347bfb6ca034b6742fb5189 (M2-001), f1437f0d254af2968c3598bf14d1a9b2b8e791cd (M2-002 design package and M2-001 refinements; **exact review candidate for code, specs and ADRs**). The commit that adds this handoff and the evidence directory changes documentation only.

Product authority: PROJECT.md, approved ADR 0001. ADRs 0003–0008 are PROPOSED and authorize nothing.

## Completed work

### M2-001 — qualification (code)

- **Workspace:** `Cargo.toml` (edition 2024, rust-version 1.85, resolver 3, exact dependency pins, overflow checks in every profile, release `panic = "abort"`), `Cargo.lock`, `.cargo/config.toml` (offline Debian registry source replacement; fiat curve25519 backend; portable chacha20/poly1305; sha2 `force-soft`).
- **Pinned environment:** `build/m2/dev_container.dockerfile` (base by digest, exact pins in `build/m2/debian_packages.txt`, downloaded `.deb` digests), `build/m2/debian_lock_arm64.txt`, `build/m2/debian_debs_arm64.sha256`, `build/m2/crate_sources.sha256` (1 454 files).
- **Crates:** domain skeletons `core/{contract_format,identityd,vaultd,contractd,policyd,registryd}` (only a name function and test each; `#![forbid(unsafe_code)]` + workspace lint); `core/native/secure_memory` (the only unsafe code: guarded, mlocked, `MADV_DONTDUMP`/`MADV_WIPEONFORK` secret regions; fail-closed allocation; subprocess guard-page and memlock-exhaustion tests; compile-time trait-absence checks); `core/native/os_adapter` (safe: `getrandom(2)`, core limit + non-dumpable, `mlockall`, `SO_PEERCRED`); `core/probes/provider_probe` (RFC 8032, XChaCha draft A.3.1, RFC 9106, FIPS 180 vectors with tamper/sensitivity controls; panic/overflow/harden modes).
- **Harness and tools:** `tests/m2/run_m2_tests.sh` (suites `unit`, `harness`; `integration`/`recovery`/`packaging` FAIL as NOT_RUN), `tests/m2/lib/m2_harness.py` (inventory-exact validation; rejects zero/failed/ignored/filtered/measured/doc-test suites, compile-only output, stale run directories, non-zero exits), `tests/m2/test_inventory.txt`, `tests/m2/harness/` (57 tests: parser controls, real-cargo canaries, lint controls, deployed-profile probes, dispatcher failures, naming checker, ECR oracle sensitivity), `tests/m2/oracle/provider_oracle.py`, `build/scripts/{qualify_m2_rust.sh,build_m2_core.sh,check_rust_naming.py,m2_dependency_inventory.py}`, `.github/workflows/m2_checks.yml`.

### M2-002 — design gates (documents and vectors)

- PROPOSED ADRs: [0003 G1](../decisions/0003_m2_rust_toolchain_dependencies.md), [0004 G2](../decisions/0004_m2_wire_schema_hash.md), [0005 G3](../decisions/0005_m2_identity_vault_ipc.md), [0006 G4](../decisions/0006_m2_storage_anchor_recovery.md), [0007 G5](../decisions/0007_m2_lifecycle_time_consumption.md), [0008 G6](../decisions/0008_m2_retention_privacy_update_identity.md).
- Specifications: [wire format v1](../specs/m2_wire_format_v1.md), [CDDL](../specs/m2_ecr_v1.cddl), [service IPC v1](../specs/m2_service_ipc_v1.md).
- Vectors: [ECR v1](../../tests/m2/vectors/ecr_v1/vectors_readme.md) — 17 valid, 80 invalid (six layers, exact codes), 7 lineage-admission failures — produced and re-verified by an independent Python implementation with a cbor2 canonical cross-check.

### Decisions made within approved scope

Branch kept as assigned; ADR numbers 0003–0008 allocated against the index; local Rust domain logic deliberately not started (gated). Decisions that need the owner are listed below, not taken.

## Verification

| Check | Environment / command | Result | Evidence / limits |
|---|---|---|---|
| Full qualification at clean `f1437f0` | arm64 Debian 13.7 container (`cham3-m2-dev`), UID 1000, `CHAM3_M2_REQUIRE_CLEAN=1 bash build/scripts/qualify_m2_rust.sh` | PASS (development) | [evidence](../evidence/M2-cryptographic-contract-core/m2_001_arm64_f1437f0/): pins, arm64 locks, source manifest, Debian-only inventory (33 compiled crates), rustfmt, clippy `-D warnings`, naming, debug/release builds, backend flags, valgrind 0 errors |
| `tests/m2/run_m2_tests.sh unit` | same | PASS | cargo test debug 32 and release 32 (inventory-exact), provider probe vs independent oracle (Argon2id case equal), ECR vector oracle 17/80/7 |
| `tests/m2/run_m2_tests.sh harness` | same | PASS | 57 tests incl. canaries proving assertion/compile/link/panic/zero/ignored/filtered/`--no-run` rejection; unsafe insertion fails in every domain crate with each enforcement layer alone; release panic and overflow abort (SIGABRT), dev unwinds (101) |
| M0/M1 static gates | same container: `build/scripts/run_static_checks.sh`; `tests/run_tests.sh static` | PASS | shellcheck/py_compile/yamllint/secret scan; 82 static tests |
| amd64 target qualification | amd64 Debian 13 | **NOT_RUN** | Owner instruction: no x86 emulation on the development Mac; assigned to the reviewer (commands below) |
| GitHub Actions `m2_checks` | — | NOT_RUN | Branch not pushed (needs owner authorization) |
| M1 desktop/packaging suites | — | NOT_RUN | No desktop/package files changed |
| Miri / sanitizers / coverage-guided fuzzing | — | NOT_AVAILABLE | Pinned toolchain limit (ADR 0003 §9); never PASS |
| M2 security behavior (A03–A15) | — | NOT_RUN | Gated domain packages M2-003..M2-008 not started |

## Review request for Codex (cloud)

Review the exact range `a4bc0c4..f1437f0` (and the documentation-only evidence/handoff commit that follows) against the M2 task, PROJECT.md and ADR 0001. Please return **APPROVE** or **CHANGES_REQUESTED**, per package (M2-001) and per gate (G1–G6), with findings (severity, location, trigger, rationale).

### amd64 qualification (cloud, disposable, unprivileged)

~~~sh
git fetch origin codex/m2-cryptographic-contract-core   # after the owner authorizes pushing
git checkout f1437f0d254af2968c3598bf14d1a9b2b8e791cd
docker build -f build/m2/dev_container.dockerfile -t cham3-m2-dev build/m2
docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp/cham3_home \
  -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
  -v "$PWD:/work" -w /work cham3-m2-dev bash build/scripts/qualify_m2_rust.sh --record
~~~

Expected on amd64 at this commit: every check PASS **except** `installed_packages_lock` and `downloaded_debs_lock`, which FAIL as "unpinned for amd64" and write `proposed_debian_lock_amd64.txt` / `proposed_debian_debs_amd64.sha256` into the run directory. Please return those two files, `summary.txt`, `identity.txt`, `toolchain.txt`, `dependency_inventory.md` and the `out/m2/runs/*/summary.txt` + JSON results as evidence. The implementer commits the reviewed amd64 locks; a rerun must then PASS fully. Running as root or with `CAP_IPC_LOCK` invalidates the memlock test (the harness refuses root). The same commands run under GitHub Actions via `.github/workflows/m2_checks.yml` once the branch is pushed.

### Suggested independent checks

- Inspect `core/native/secure_memory/src/lib.rs` invariants and every `unsafe` block; confirm no other cham3 crate can contain unsafe code.
- Audit focus from ADR 0003 §5: rustix `mm`/`net`/`process`/`rand` paths, zeroize, argon2 AVX2 compression and cpufeatures, generic-array/inout casts; check Debian security tracker status of the pinned packages.
- Confirm backend selection in a verbose amd64 build (fiat, `chacha20_force_soft`, `poly1305_force_soft`, sha2 `force-soft`) and that `cargo tree --target x86_64-unknown-linux-gnu` matches `crate_sources.sha256`.
- Re-verify a sample of the ECR vectors with a third implementation of your choice (any COSE/CBOR/Ed25519 library) and try to find a schema/semantic hole the vectors miss.
- Probe the harness: introduce a controlled defect (e.g. delete a test, mark one `#[ignore]`, break the release profile's `panic = "abort"`) in a disposable copy and confirm the suite fails.
- For each ADR: threat-model gaps, contradictions with PROJECT.md/ADR 0001, unverifiable claims, and whether the verification plan is sufficient for A01–A15.

## Owner decisions required after review (not taken)

1. Approve, amend or reject each of G1–G6 (ADRs 0003–0008).
2. ADR 0006 §5 — default anchor mode: degraded same-volume by default (recommended) vs requiring a separate volume/TPM for new grants.
3. ADR 0007 §4 — one-shot semantics: reservation consumes (recommended) vs release on confirmed failure-before-effect.
4. ADR 0008 §3 — deletion granularity: whole-account only (recommended) vs per-lineage deletion.
5. Authorization to push `codex/m2-cryptographic-contract-core` to `origin` so the cloud reviewer and `m2_checks` can use the exact commits.

## Remaining

- **Known issues/risks:** amd64 locks not yet recorded; Debian's crate checksum placeholder means content pinning relies on the committed source manifest; passphrase approval in M2 is a development terminal path and not a trusted display/input path; app/session identities are fixtures until M3; anchors are degraded unless a separate rollback domain is configured; Debian security updates will require re-qualification.
- **Uncommitted paths or dirty state:** none after the evidence/handoff commit (`out/` is ignored).
- **Required next action and owner:** owner authorizes push (or provides another transfer path) → Codex reviews the exact commits and runs amd64 qualification → implementer repairs findings and commits amd64 locks → owner decides G1–G6 and the three explicit choices → only then do M2-003 onward become READY.
