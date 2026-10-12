# ADR 0003: M2 Rust toolchain, dependencies and unsafe boundary (G1)

- **Status:** PROPOSED (revision 2) — awaiting independent re-review and owner approval. Nothing here is approved.
- **Date / decision owner:** Proposed 2026-10-12 by Claude Code (M2 implementer); revised 2026-10-12 after the independent review published at 9cf882503848f8c6cc4938a588cc9fc062a9392d (CHANGES_REQUESTED: R01, R02, R03; I02) / cham3 project owner. Finding resolutions: [M2-001-002-finding-resolutions.md](../reviews/M2-001-002-finding-resolutions.md).
- **Related task/spec sections:** [M2 task](../tasks/M2-cryptographic-contract-core.md) G1, M2-001, A01, A02; PROJECT.md §§4.2, 4.5, 12, 13; [Rust research](../research/M2_rust_readiness.md); SEC-05, SEC-17.

`PROJECT.md` is the canonical product specification. This ADR records a scoped decision or proposal and does not silently replace that document.

## Problem and context

M2 must be implemented as safe, compiled Rust with exactly pinned, offline, reviewable build inputs, isolated unsafe/FFI, and harness evidence that tests really execute. The owner selected a memory-safe compiled language; Rust was selected under that instruction. G1 must freeze the compiler/std/target/linker, edition and MSRV, the workspace and lockfile, dependency sources/features/build scripts/macros, the transitive unsafe/native inventory, secret ownership, panic policy and the Debian build recipe.

Revision 1 evidence (`f1437f0`) was produced natively on arm64 Debian 13.7 (development evidence) and reproduced natively on amd64 by the independent reviewer and by GitHub Actions run 38276640049: every check passed except the two then-uncommitted amd64 locks. Revision 2 commits the reviewed amd64 locks and is qualified natively on amd64 in disposable CI infrastructure (never x86 emulation on the development Mac, owner instruction 2026-10-12).

## Decision (proposed)

### 1. Toolchain

| Input | Pin |
|---|---|
| Compiler, Cargo, std | Debian 13 `rustc`, `cargo`, `libstd-rust-dev` 1.85.1+dfsg1-1+deb13u1 (`rustc 1.85.1 (4eb161250 2025-03-15)`); `rust-clippy`, `rustfmt` same version |
| Target | `x86_64-unknown-linux-gnu` (amd64). arm64 is a development host only |
| Linker / C runtime | `gcc-14` 14.2.0-19 as `cc`, `binutils` 2.44-3, `libc6-dev` 2.41-12+deb13u4 |
| Language | edition 2024, `rust-version = "1.85"`, resolver 3 |
| Excluded | rustup, nightly, floating `stable`, compiler source submodules |

Rationale: 1.85 is the first release with edition 2024; Debian's package is APT-authenticated, security-supported for Debian 13 and identical to what a Debian build of cham3 packages uses. Its age means some newer crates/features are unavailable; G1 accepts that rather than introducing a second toolchain.

### 2. Reproducible environment

- [build/m2/dev_container.dockerfile](../../build/m2/dev_container.dockerfile): base `debian@sha256:913f6706…c2d5` (multi-arch index), exact versions from [build/m2/debian_packages.txt](../../build/m2/debian_packages.txt); a version Debian has superseded fails the build instead of upgrading.
- Per-architecture locks of the full installed set and of every downloaded `.deb` SHA-256: `build/m2/debian_lock_<arch>.txt`, `build/m2/debian_debs_<arch>.sha256`. arm64 (development) is committed from the author's native run. **amd64 (target)** is committed from the independent reviewer's native reproduction, accepted by that review: `debian_lock_amd64.txt` (612 lines, SHA-256 `6cd45590bdebd08f53c95a56112a97030b844959983284db44232b59baa11d3c`) and `debian_debs_amd64.sha256` (535 lines, `6ce06e450a8b91cd2c91c1d3aaffc360dfed7276f6622ac03dbd05fd35f0f43a`). The reviewer could not retrieve the earlier CI artifact (HTTP 403) and made no comparison with it; the author separately downloaded that artifact (run 38276640049, clean `3c35aac`, amd64) and found its two proposal files have the same SHA-256 digests — author evidence, not reviewed.
- Debian security updates to any pinned package are expected; each requires a recorded re-qualification (new locks, source-manifest diff review), independent review and, for toolchain/provider changes, owner approval. snapshot.debian.org is the fallback for reacquiring exact superseded versions.

### 3. Dependency source and provenance

- Dependencies come **only** from Debian 13 `librust-*-dev` packages installed under `/usr/share/cargo/registry`, through Cargo directory source replacement in [.cargo/config.toml](../../.cargo/config.toml) with `net.offline = true`. crates.io is never contacted; `cargo metadata --locked --offline` and `cargo build --frozen` are required.
- Debian's registry entries carry a placeholder package checksum (`"Could not get crate checksum"`), so `Cargo.lock` pins versions but not content. Content is pinned by [build/m2/crate_sources.sha256](../../build/m2/crate_sources.sha256): SHA-256 of all 1 454 files of every compiled dependency for x86_64 and aarch64, checked by `build/scripts/m2_dependency_inventory.py sources --check`.
- Debian's dfsg repacks and patches are accepted as Debian's maintained provenance; byte equality with crates.io is not claimed. A reviewer may diff selected crates against upstream releases.

### 4. Dependency set

Compiled graph (from `cargo tree --target <triple>`; `cargo metadata`'s resolve over-approximates and lists never-built optional crates): 33 external crates, all Debian-packaged.

| Purpose | Crate (Debian version) | Features / configuration |
|---|---|---|
| Ed25519 | ed25519-dalek 2.1.1 (`2.1.1+4.1.3+20240618+dfsg-9`) on curve25519-dalek 4.1.3 | `std`, `zeroize`; no `rand_core`/`serde`/`pkcs8`; `verify_strict` only; **fiat-crypto backend** (formally verified field arithmetic, no SIMD/derive macro) |
| SHA-256 | sha2 0.10.8 | `std`, **`force-soft`** (no SHA-NI/ARM intrinsics) |
| XChaCha20-Poly1305 | chacha20poly1305 0.10.1 (chacha20 0.9.1, poly1305 0.8.0) | no default features (no allocation API, no getrandom); in-place detached AEAD; **`chacha20_force_soft`, `poly1305_force_soft`** |
| Argon2id | argon2 0.5.3 (blake2 0.10.6) | `zeroize`, no default features; caller-supplied memory (`hash_password_into_with_memory`); retains a runtime-detected AVX2 compression path (2 `unsafe` occurrences) with no switch |
| Constant-time compare | subtle 2.6.1 | `std` |
| Zeroization | zeroize 1.8.1 | volatile writes |
| Linux interfaces | rustix 0.38.37 (linux-raw-sys 0.4.14; libc 0.2.169 on aarch64 only) | `std`, `use-libc-auxv`, `mm`, `param`, `process`, `net`, `rand` |
| Build-time only | rustc_version 0.4.0, semver 1.0.23, version_check 0.9.5 | run by curve25519-dalek/generic-array build scripts |
| Test-only (declared for M2-003) | proptest 1.6.0, ciborium 0.2.2 | property tests; ciborium as differential CBOR oracle, never linked into services |

Deliberately not used: the `getrandom` crate (rustix `getrandom(2)` with no fallback), libsodium FFI, serde, any async runtime, and a COSE crate (none is packaged in Debian 13; the COSE_Sign1 profile is data formatting over deterministic CBOR implemented in safe Rust — signing remains ed25519-dalek; no primitive is implemented).

### 5. Unsafe and native boundary

- **Domain crates** (`core/contract_format`, `identityd`, `vaultd`, `contractd`, `policyd`, `registryd`, plus `core/native/os_adapter` and the qualification probe) carry `#![forbid(unsafe_code)]` in every crate root **and** the workspace lint `unsafe_code = "forbid"`; each layer alone rejects inserted unsafe code (tests/m2/harness/test_m2_lint_controls.py).
- **The only cham3 unsafe code** is [core/native/secure_memory](../../core/native/secure_memory/src/lib.rs): 11 documented `unsafe` blocks (mmap, two guard `mprotect`, `mlock`, two `madvise`, two slice constructions, drop-time slice, `munlock`, `munmap`) behind one safe type. `unsafe_op_in_unsafe_fn`, `clippy::undocumented_unsafe_blocks` and `clippy::multiple_unsafe_ops_per_block` are denied. Its invariants (exclusive ownership, layout, checked arithmetic, borrow-bound slices, not Send/Sync) are stated in the crate docs.
- **No FFI** to C libraries in the deployed graph. rustix performs raw syscalls (linux_raw backend, inline assembly); linux-raw-sys is generated bindings.
- **Dependency unsafe inventory** (`build/scripts/m2_dependency_inventory.py`, keyword occurrences outside comments; a locator, not a soundness verdict): rustix 1 432 (145 files), linux-raw-sys 1 636 (86, mostly generated constants/types), libc 405 (aarch64 only), generic-array 78, poly1305 50 and chacha20 23 (SIMD backends compiled out by force_soft), curve25519-dalek 33 (SIMD backend compiled out by fiat), sha2 29 (intrinsics compiled out), blake2 26 (`simd` feature off), zeroize 24, inout 22, cpufeatures 9, base64ct 4, block-buffer 4, others ≤ 2. **Review focus for G1:** rustix `mm`/`net`/`process`/`rand` paths that cham3 calls, zeroize, argon2's AVX2 compression and cpufeatures, generic-array/inout pointer casts. Reviewers record findings; known advisories are checked against Debian's security tracker for the pinned versions.
- No shadow policy/ECR logic exists in any native or dependency crate; domain logic stays in safe crates.

### 6. Profiles and panic policy

- `overflow-checks = true` in dev, release, test and bench; security arithmetic additionally uses checked operations (clippy `arithmetic_side_effects` denied in domain crates).
- Deployment (release) profile: `panic = "abort"`, `codegen-units = 1`, `lto = false`, `incremental = false`, debug assertions off. Dev profile unwinds (Cargo's test harness always unwinds).
- No authority, consumption, revocation or confidentiality guarantee may depend on unwinding or `Drop`. Services persist restrictive state before acknowledging (G4/G5), are restarted by systemd and recover from durable state. Secret confidentiality after abort relies on process-wide `mlockall`, `PR_SET_DUMPABLE=0` and `RLIMIT_CORE=0`, so pages are neither swapped nor dumped and are freed by the kernel.
- Verified by subprocess tests against real binaries: release panic and overflow abort with SIGABRT; dev unwinds with status 101; positive controls (tests/m2/harness/test_m2_deployed_profile.py).

### 7. Secret handling (G1 part; vault protocol is G3)

- Secret types implement no `Debug`, `Display`, `Clone`, `Copy` or serialization (compile-time trait-absence assertions); secrets are held in `secret_region_t` (guard pages, `mlock`, `MADV_DONTDUMP`, `MADV_WIPEONFORK`, volatile zeroization; allocation fails closed when locking fails).
- The vault process additionally calls `mlockall(MCL_CURRENT|MCL_FUTURE)`, `PR_SET_DUMPABLE=0`, `RLIMIT_CORE=0` at start and refuses to run if any fails, because ed25519-dalek and argon2 make transient copies outside guarded regions. `mlockall` prevents swapping; it does **not** erase freed memory, so every secret-bearing buffer has a named clearing owner (R03):
  - ed25519-dalek 2.1.1 `SigningKey` and `ExpandedSecretKey` zeroize their secret fields on drop (`zeroize` feature; verified in the pinned source).
  - argon2 0.5.3 clears its internal hashes but **not** the caller-supplied `Block` workspace (confirmed by the independent review and by the regression test `argon2_library_leaves_caller_workspace_populated`). cham3 owns that workspace: allocated once at its exact block count, explicitly zeroized after every derivation on success and every error path and again on drop (`argon2_workspace_t` in the probe demonstrates the pattern with tests; the vault implementation is gated, ADR 0005 §4).
  - Unnamed stack temporaries inside the libraries are not erased; they are covered only by process locking, non-dumpable state and core limits, and abort/death bypasses every destructor. This is a documented limitation (PROJECT §4.5: no claim that keys never enter RAM).

### 8. Lints and naming

`#![forbid(unsafe_code)]` as above; clippy `-D warnings` plus denied `unwrap_used`, `expect_used`, `panic`, `indexing_slicing`, `arithmetic_side_effects`, `as_conversions`, `dbg_macro` in domain crates (tests may allow them locally). Repository naming (snake_case, `snake_case_t` types, PascalCase constants) conflicts with rustc's `non_camel_case_types` and `non_upper_case_globals`; exactly those two lints are allowed, `non_snake_case` is denied, and [build/scripts/check_rust_naming.py](../../build/scripts/check_rust_naming.py) enforces the repository rule (with controlled-violation tests). External spellings (`Drop`, `Vec`, `COSE_Sign1`, `Sig_structure`, hyphenated service names) stay at boundaries.

### 9. Verification tooling and limits

Source identity is evidence, not decoration (R01): `build/scripts/m2_source_identity.sh` records commit, tree, tracked-dirty count, Debian version and the Cargo.lock digest, and **fails** when any identity command fails or yields a malformed value; the harness environment collector fails likewise. Qualification and every suite record `source_identity` as PASS/FAIL; `CHAM3_M2_REQUIRE_CLEAN=1` additionally fails dirty trees. Missing identity is never reported as a clean or dirty state (canaries in `tests/m2/harness/test_m2_source_identity.py`).


| Tool | Status on the pinned toolchain |
|---|---|
| `cargo test` debug and release, inventory-exact, no ignored/filtered/zero suites | available, required |
| Harness sensitivity canaries (assertion, compile, link, panic, zero, ignored, filtered, `--no-run`) | available, required |
| valgrind 3.24 memcheck | available, required: the release provider probe (`valgrind_memcheck_probe`) **and** the actual `cham3_secure_memory` release test executable, all 10 tests in process (`valgrind_memcheck_secure_memory`, I02). Limit: the guard-page and memlock child processes are not instrumented (no `--trace-children`); their parents and the in-process tests are |
| Bounded property and mutation tests (proptest 1.6.0, deterministic mutators) | available; used for parser/DAG/IPC fuzzing |
| Miri, `-Zsanitizer`, cargo-fuzz/libFuzzer | NOT_AVAILABLE (nightly-only / not packaged); never counted as PASS. An optional supplementary Miri run of `core/native` on a separately recorded nightly is non-qualifying evidence |

### 10. Debian build and deployment recipe (frame for M2-007)

Packages are built with `dpkg-buildpackage` in the same pinned environment; `debian/rules` runs `cargo build --release --frozen` with the repository `.cargo/config.toml`, Build-Depends pin the toolchain and `librust-*-dev` versions above, and produced binaries are recorded by SHA-256. Package contents, users, units and confinement are G3/M2-007 decisions.

### 11. Dependency security status (advisory review requested by the review)

Author-run, against the pinned inputs; the independent reviewer's own advisory check was BLOCKED (HTTP 403) and remains to be repeated before approval. Evidence: `docs/evidence/M2-cryptographic-contract-core/advisory_review_20261012/`.

- **Debian security tracker** (`tracker/data/json` fetched 2026-10-12 17:07 UTC, SHA-256 `52d2faea…5dc6`): for the sources of every compiled crate, the toolchain (rustc, gcc-14, binutils, glibc) and the oracle libraries, **no** resolved trixie fix is newer than a pinned version (0 behind), and **no** open trixie issue exists for `rustc` or any `rust-*` source of the 33 compiled crates. Open, not-yet-triaged trixie issues exist in glibc (`ld.so` and libc functions), gcc-14 libstdc++, systemd and the test-only oracles (cbor2, python-cryptography). None is in code the M2 Rust binaries call today; the `ld.so` items are recorded as a precondition for the setgid client in M2-007 (re-check before packaging).
- **RustSec advisory-db** (commit `25a288f6…2569`, 2026-10-12): 37 crate versions (33 compiled + 4 test-only) checked against affected/patched ranges: 6 advisories exist for these crates (blake2 RUSTSEC-2019-0019, chacha20 RUSTSEC-2019-0029, curve25519-dalek RUSTSEC-2024-0344, ed25519-dalek RUSTSEC-2022-0093, generic-array RUSTSEC-2020-0146, sha2 RUSTSEC-2021-0100); **all are patched** in the pinned versions; **0 affected**.
- Debian point/security updates will change these results; each requires re-qualification (§2).

## Alternatives considered

- **rustup-pinned upstream stable (for example 1.88) with crates.io:** newer language/library features and access to coset/minicbor/cargo-fuzz; rejected as primary because it introduces a second, non-Debian toolchain and unauthenticated-by-APT dependency acquisition, and Debian packaging would diverge from qualification.
- **crates.io sources vendored into the repository (`cargo vendor`):** real `.crate` checksums in Cargo.lock and newer versions; rejected as primary (large tracked third-party tree, no Debian security tracking, duplicate provenance review). Remains the fallback if a required crate is missing from Debian, subject to review.
- **libsodium via FFI (`libsodium-dev` 1.0.18-1+deb13u1):** one mature native provider with `sodium_malloc` guarded memory and all required primitives; rejected as primary because it adds a C FFI boundary and a 2019-era API line, while the safe Rust providers cover every primitive and cham3 can provide guarded memory with a small reviewed adapter. libsodium remains the independent test oracle (python3-nacl).
- **SIMD/intrinsic backends (default):** faster; rejected for M2 because performance is irrelevant at these volumes and each backend is additional unsafe code to audit.
- **Separate C/unsafe policy core:** forbidden by the task.

## Tradeoffs and consequences

- Debian's crate versions trail upstream (ed25519-dalek 2.1.1, rustix 0.38) and some crates are absent; cham3 implements the narrow COSE profile itself and uses proptest instead of libFuzzer. Toolchain/crate upgrades follow Debian 13 point releases and security updates with re-qualification.
- Placeholder registry checksums require the separate source manifest; any Debian update changes it and must be reviewed.
- Portable backends cost CPU time (not measurable at M2 scale) and remove most SIMD unsafe code from the audited path.

## Security implications

- Memory safety of cham3 domain logic rests on safe Rust plus soundness of rustix, zeroize, generic-array/inout, argon2's AVX2 path and the cham3 secure_memory adapter; these are the audit targets. Rust does not prove cryptographic correctness, authorization, durable storage or erasure; those are G2–G6 and test obligations.
- No production keys, no host deployment, no privileged operations are involved in qualification; tests run unprivileged and refuse to run as root.

## Verification

| Check | Environment / command | Result | Evidence / limits |
|---|---|---|---|
| Revision 1 qualification | arm64 (author) at `f1437f0`; amd64 (independent reviewer, GitHub Actions 38276640049) at `3c35aac` | PASS except the two uncommitted amd64 locks on amd64 | Review 9cf8825; `docs/evidence/M2-cryptographic-contract-core/m2_001_arm64_f1437f0/` |
| Revision 2 qualification with committed amd64 locks, source identity, secure-memory valgrind | amd64 GitHub Actions (disposable, unprivileged container) and arm64 author run at the revision-2 commit | see the [revision-2 handoff](../handoffs/M2-001-002-rereview.md) | Must PASS fully on amd64 before approval |
| Miri / sanitizers / coverage-guided fuzzing | — | NOT_AVAILABLE | Toolchain limit, recorded, not PASS |

## Approval and supersession

- **Independent review:** revision 1 CHANGES_REQUESTED at 9cf8825 (R01–R03; I02). Revision 2 requires re-review, including the reviewer's own advisory check.
- **Owner approval evidence:** none. Requires independent re-review of this exact proposal and full amd64 qualification, then an explicit owner decision.
- **Supersedes / is superseded by:** revision 2 supersedes revision 1 of this proposal (commit f1437f0d254af2968c3598bf14d1a9b2b8e791cd). The historical Stasha research and receipts do not qualify this toolchain.
