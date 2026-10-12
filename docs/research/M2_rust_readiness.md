# M2 Rust readiness research

Research date: 2026-10-12. Scope-change baseline: c84de12d280454cba680d731f907355134a91e30. Authority: PROJECT.md, approved ADR 0001 and the owner's request to replace Stasha with a memory-safe compiled language and remove its submodule for now. This research supplies implementation inputs, not M2 or toolchain verification.

## Language selection and safety boundary

Rust is selected for M2 under the owner's new language constraint. Its ownership/type rules provide memory safety for safe code; the [Rust Reference](https://doc.rust-lang.org/reference/unsafety.html) identifies operations that require unsafe code. That protection depends on sound unsafe implementations behind safe interfaces, including dependencies and foreign libraries. Rust does not by itself prove authorization, crypto correctness, secure erasure or rollback resistance.

Require substantive safe Rust domain crates with #![forbid(unsafe_code)]. Isolate unavoidable unsafe/FFI in separate reviewed adapter crates, document soundness invariants and expose checked safe APIs. Inventory transitive native/unsafe code, build scripts and procedural macros; a safe-looking dependency API does not establish soundness. Maintained safe Rust APIs are preferred where they satisfy PROJECT's required algorithms and memory protections. Established native providers remain candidates, subject to G1/G3 review.

## Toolchain and build inputs

[Rust 1.85 introduced edition 2024](https://blog.rust-lang.org/2025/02/20/Rust-1.85.0.html). The primary Debian 13 package pages currently list [rustc](https://packages.debian.org/trixie/rustc) and [Cargo](https://packages.debian.org/trixie/cargo) at 1.85.1+dfsg1-1+deb13u1, including amd64 availability. This is an initial compatibility candidate only: exact authenticated package versions/hashes, security updates, target/std/linker and dependency MSRVs must be checked in disposable Debian before freezing G1. No compiler installation, Rust workspace or product build was performed for this research.

Commit the workspace Cargo.lock; record the exact dependency feature/source/checksum graph and native inputs. [Cargo build](https://doc.rust-lang.org/cargo/commands/cargo-build.html) documents locked/offline/frozen operation. Acquire and verify dependency sources separately before frozen builds. Review executable build inputs and network behavior; a lockfile does not eliminate unpinned build-script inputs. Generated targets belong under ignored out/, and Rust naming lints need narrow allowances for the repository's required type/constant spellings.

[Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html) make overflow checking and panic handling explicit build choices. Enable overflow checks for debug, release and custom profiles; use checked operations for security-relevant arithmetic. Freeze actual profile settings and test failure/recovery. Panic abort or process termination may bypass cleanup; vault protection, admission and one-shot recovery cannot rely solely on destructors.

The ordinary Cargo test harness ignores panic=abort. Exercise deployed panic/abort handling using subprocess fault-injection against binaries built with the actual approved profile, alongside unit tests; this is required implementation evidence, not a test performed during preparation.

## Verification and dependency decisions

[Cargo test](https://doc.rust-lang.org/cargo/commands/cargo-test.html) normally builds and runs tests; --no-run only builds. A successful command can still select zero tests. The M2 harness must validate actual suite inventory, nonzero expected counts, required ignored/filtered/skipped cases, fresh output and process results. Test debug/release, controlled unsafe insertion in domain crates, assertion/compile/link failure, missing tools and zero-test rejection. Fuzz malformed inputs; record optional Miri/sanitizer tooling scope rather than implying universal dependency coverage.

The earlier libsodium, CBOR/COSE and SQLite research remains a list of candidates, not frozen providers or backend choices. Compare maintained Rust APIs and native options under G1–G6 using the required Ed25519, deterministic CBOR/COSE_Sign1, XChaCha20-Poly1305, Argon2id, SHA-256 and Linux cryptographic randomness. Check versions, features, security/maintenance evidence, exact vectors, canonicalization and actual protected-memory support before selection. No new primitive implementation or unreviewed library substitution is authorized.

Safe Rust does not remove PROJECT §4.5's guarded allocation, successful locking, explicit clearing, trace/core restrictions, short lifetime and swap/hibernation requirements. Secret types must prevent casual copying/formatting/serialization, and tests must exercise actual protection failures. Durable storage, current authorization, authenticated approval, lifecycle evidence, privacy and rollback-resistant anchors remain unchanged design gates.

## Verification classification

- PASS — primary-source documentation/package availability research and source-tree inspection on the macOS arm64 host; no host installation or privileged operations.
- NOT_RUN — pinned Debian Rust/dependency qualification, provider probes, crate lint controls, product builds, M2 tests and full-system enforcement. M2-001/M2-002 remain the useful starting assignment.
- Historical only — [Stasha research](M2_stasha_readiness.md) and [upstream receipts](../evidence/M2-planning/README.md). The removed dependency's qualification cannot be reused as Rust evidence.
