# Task: M2 — Cryptographic contract core and Eternal Contract Repository

- **State:** DRAFT for full backend implementation; M2-001 and M2-002 below are READY preparation work. Dependent subtasks become READY only after their design gates are independently reviewed and owner-approved.
- **Owner / role / provider:** Project owner retains architecture/risk authority. Claude Code is the intended design/implementation author; Codex is the independent architecture/security reviewer. Coordinator tracks gates; integrator admits exact reviewed commits.
- **Created / updated:** 2026-10-12.
- **Base commit:** aa843f7ac19398f580f3577ba047300971ba6095, clean local main including approved ADR 0001 and locally merged M1.
- **Branch / checkout:** codex/m2-cryptographic-contract-core, /Users/jessicabruce/cham3. One writer at a time; preparation hands this branch to Claude after its own commit/review. Do not use an old M1 worktree.
- **Original preparation target / review:** Earlier preparation commits/verdict are in the [historical preparation handoff](../handoffs/M2-task-preparation.md). They do not approve this language revision and are not product implementation commits.
- **Owner scope change, 2026-10-12:** The owner subsequently requested a memory-safe compiled implementation language and removal of the Stasha submodule for now. Rust is the selected implementation language under that instruction, superseding the earlier Stasha constraint/dependency request. This changes the implementation assignment, not PROJECT/ADR 0001 architecture or unresolved detailed design gates.
- **Language-change baseline:** c84de12d280454cba680d731f907355134a91e30, clean existing M2 branch after unrelated main updates. Current preparation evidence is in the [Rust transition handoff](../handoffs/M2-rust-transition.md); earlier preparation/review evidence is historical.

## Objective

Deliver PROJECT Phase 2 / engineering M2: authenticated per-user key authorities, encrypted signing vault, canonical signed contracts, current-authorization evaluation, and ECR as the foundational signed contract DAG and durable grant-state system. Implement cham3 domain logic in safe, compiled Rust. Demonstrate correctness, denial behavior, native boundaries and durable recovery on Debian 13 amd64, then obtain independent security review.

Signature validity, current authorization, durable ECR admission and confirmed runtime enforcement are separate facts. M2 supplies tested core/services/interfaces to future consumers. Its harness cannot establish M3 application isolation, M4 networking, M5 trusted KDE or M6 active-resource revocation.

## Approved requirements and authority

- PROJECT.md §§2.3–2.4, 3–5 including §5.7, 6.1, 10–15, Phase 2 and §18; [roadmap M2](../engineering/ROADMAP.md).
- [ADR 0001](../decisions/0001_eternal_contract_repository.md), APPROVED scoped architecture: immutable persistent/temporary DAG; canonical signed transactions; protected objects/indexes; current-state/anti-replay recovery; no direct app access; fresh authenticated approval; forward restore; immediate denial independent of signing/history.
- [Security invariants](../engineering/SECURITY_INVARIANTS.md): SEC-01–06, SEC-10–16, SEC-18. Preserve default-deny, mandatory-policy precedence, immutable revisions, appropriate user/key authority and narrow trusted IPC.
- Required primitives: Ed25519; COSE_Sign1; deterministic CBOR; XChaCha20-Poly1305; Argon2id; SHA-256; Linux cryptographic randomness. Use maintained established libraries; no custom signing, encryption, KDF or hash primitive.
- Internal naming: snake_case; PascalCase only constants/macros; snake_case_t types. Required external C/COSE/service spellings stay at documented boundaries.
- [Rust research](../research/M2_rust_readiness.md) is evidence/recommendations, not approval of a wire format, backend, library or security guarantee. The earlier Stasha research and receipts are historical and cannot qualify the Rust implementation.

## Scope and exclusions

In scope:

- Pinned Rust/Cargo/dependency qualification, build integration and unsafe/native boundary inventory.
- Reviewed detailed design records, interface specifications and vectors for all gates below.
- Strict schema/envelope verification, identity/key-class validation, current-authorization and mandatory-policy evaluation.
- Encrypted key objects, restricted vault operations, independently validated approval binding, enrollment/invalidation/rotation primitives and only approved recovery transitions.
- Immutable ECR objects, graph/admission, pending/current indexes, lifetime/revision/delegation/revocation, authorized trusted history queries, lifecycle evidence and audit gaps.
- Durable atomic one-shot handling, stale-head/epoch rejection, concurrency, interrupted writes and rollback-resistant recovery.
- Authenticated service boundaries, development CLI/harness, Debian packages/service confinement, strict CI and evidence.
- Core forward-restore proposal/validation and current-parented admission; actual resource effects remain M6.

Out of scope:

- Launcher/application sandbox, real window/runtime attribution, network/Tor/firewalls, portals, trusted KDE workflow/history UI, global kill implementation, production resource enforcement/release/keys or host administration.
- Full recovery UX, hardware/FIDO2/biometric support and multi-user desktop hardening before later designs/tests. M2 still enforces user separation internally and rejects unauthenticated key transitions.
- Replacing the safe Rust core with an unsafe/native policy implementation; compiler redesign; unrelated M0/M1 edits; changing the separate Stasha checkout or reintroducing its submodule.
- Calling OS UID separation, mock approval, fixture acknowledgements or configuration checks full-system isolation or real human-presence proof.

## Rust implementation contract

Implement the core as a Cargo workspace of substantive .rs crates/modules under core/. Rust owns schema/contract semantics, COSE/profile validation, identity/key-class and mandatory-policy checks, exact proposal/approval binding, graph/delta/result-state checks, transaction state machines, lifecycle/recovery decisions and service orchestration. Use safe Rust and crate-level #![forbid(unsafe_code)] for these domain crates. Do not wrap a second C or unsafe Rust cham3 policy/ECR core.

Use stable Rust, edition 2024, with an exact compiler/Cargo/standard-library and target baseline qualified on disposable Debian 13 amd64. Debian's Rust 1.85.1 package is the initial compatibility candidate, not an already qualified pin. G1 freezes actual package versions/hashes, linker/native inputs, features and minimum supported Rust version. No floating stable/nightly installation or compiler-source submodule; any toolchain change needs recorded justification, repeated affected qualification and independent review.

Commit Cargo.lock for the complete workspace and pin/check transitive dependency sources and checksums. Review crate provenance, supported compiler versions, default/optional features, build.rs scripts, procedural macros, native link inputs, maintenance/security status and actual unsafe code. Freeze dependencies before offline/frozen builds; lockfiles alone do not prove soundness, provenance or reproducibility. Prefer maintained safe Rust APIs where they meet PROJECT requirements; established native providers remain possible through reviewed adapters. No custom signing, encryption, KDF or hash primitive.

Keep unavoidable unsafe/FFI only in small separately reviewed adapter crates under core/native/, exposing safe checked APIs. Enable #![deny(unsafe_op_in_unsafe_fn)] there; document each safety invariant, lifetimes, null/length/overflow validation, thread safety, allocator pairing and all error paths. Audit unsafe and native internals in dependencies even when the local API is safe. Rust's memory-safety claim depends on those boundaries being sound; it does not prove cryptographic correctness, authorization, durable storage or secret erasure.

Secret types must not expose generic Debug/Display/Clone, logging or unrestricted serialization. Use reviewed zeroization and guarded/locked memory mechanisms, minimize copies/lifetimes, and test protection failures and cleanup. Ordinary Vec or String storage, Drop alone, and a Rust implementation do not satisfy PROJECT §4.5. Freeze panic/unwind/abort behavior and recovery: abort or process death can bypass destructors, and no authority or consumption guarantee may depend solely on Drop.

Preserve repository snake_case_t types and PascalCase constants with narrow documented allowances for Rust naming lints (non_camel_case_types and non_upper_case_globals). Keep required upstream trait/type/interface spelling at documented boundaries; do not globally suppress warnings or weaken unsafe restrictions to satisfy naming checks.

## Dependencies and mandatory design gates

M0/M1 are present in Git ancestry; historical status may predate the merge and is not new evidence. M2 does not require completed KDE/network backends; it requires reviewed interfaces future milestones can implement.

Gate IDs describe required design packages, not approved decisions. Allocate unique ADR numbers against the current index, use the ADR template, compare alternatives and obtain owner approval after independent review. Any canonical-spec amendment must be explicit. Feasibility probes do not themselves approve architecture.

| Gate | Required reviewed, owner-approved design before affected implementation |
|---|---|
| G1 — toolchain/dependencies/unsafe | Exact stable Rust/Cargo/std/target/linker pins, edition/MSRV, workspace/lockfile and offline input recipe, dependency features/provenance/build scripts/macros, transitive unsafe/native inventory and sound safe adapter contracts; secret ownership/cleanup, panic/abort policy, reproducible inputs and Debian build/deployment recipe. Missing support needs bounded dependency/toolchain proposals, qualification and review. |
| G2 — wire/schema/hash | Exact versioned contract/commit fields/types; deterministic CBOR ordering, numbers, text, tags, lengths and resource limits; duplicate/unknown/malformed rejection; COSE_Sign1 header/tag/algorithm/key ID/external-AAD/domain separation; signing structure, hash preimage, parent order, genesis, merge/base-state and linked state; evolution/migrations and published valid/invalid vectors. |
| G3 — identity/auth/vault/IPC | Trusted user/app/install/executable/session identity source; authenticated key IDs/certificates/enrollment/rotation/recovery; available fresh authenticated approval before M5; exact bytes/head/epoch/request binding, approval expiration/replay/one-use signing; service credentials/roles/protocol; encrypted key format, KDF limits/calibration, memory/core/ptrace/lock policy. No arbitrary signing or trust in bare UID/display names. |
| G4 — storage/anchors/recovery | Backend, atomic objects/index boundaries, heads/epochs/revocations/one-shot reservations, serialized writers, pending/confirmed freshness, crash points/integrity; actual rollback-resistant anchor and assumptions; genesis, lost/corrupt anchor, stale backup/tail/index restoration and safe recovery. Another file on the same restorable disk is not independent freshness proof. Do not accidentally require TPM/external hardware foundationally. |
| G5 — lifecycle/time/consumption | Immediate expiry/session/key/policy restriction; non-grant authenticated lifecycle provenance and audit gaps; time/clock rollback model; reservation/success/failure/crash consumption semantics, durable ordering and reconciliation without duplicated or restored authority. Denial never waits for a new signature. |
| G6 — retention/privacy/update identity | Quotas/full-disk denial; trusted viewers/redaction, permanent message privacy, account/key deletion, backup/export/access control, migrations and executable/package updates; restore with missing resources/keys. Append-only history is not infinite storage or secrecy against privileged compromise. |

libsodium, libcbor/COSE handling, SQLite, Unix sockets and systemd are research candidates. Select/freeze actual providers, versions, IPC and backend through these gates; this task does not implicitly approve them.

## Logical service interfaces

These are required responsibilities, not final wire names. Freeze protocol, limits, errors, credentials and freshness through G2–G5. Canonical services follow PROJECT §14; process consolidation needs explicit review and owner approval.

| Owner | Required operations / checks |
|---|---|
| cham3-identityd | Authenticated registered user keys/authority classes/certificates/epochs, enrollment/rotation/invalidation. Caller-supplied key ID is not authentication. |
| cham3-vaultd | Create/load encrypted keys, authorized unlock/lock, sign independently approved exact proposals under correct authority. No arbitrary message API or raw secret export; deny wrong/stale/missing approval, enforce short secret lifetime and protection failure policy. |
| cham3-contractd | Strict versioned parse/canonicalization, validated proposals, provider-backed exact COSE signing/verification, separate current validity; immutable revise/merge/restore without replay. |
| cham3-policyd | Mandatory policy, user/resource ownership, verified app/install/runtime scope, validity/time/session/usage, key class, lineage/revocation/delegation and epoch checks. Structured deny/eligible/unknown outcomes, never a resource-enforcement claim. |
| cham3-registryd | Validated immutable graph/delta/state admission; transactional heads/epochs/request IDs and usage/lifecycle; pending/failed/confirmed references; restricted read-only history; protected authoritative state and crash/rollback recovery. |
| Future launcher/net/broker/session consumers | Authenticated attribution/confirmation bound to commit, runtime, domain and freshness. Reject unauthorized/stale confirmations; M2 fixture consumers do not establish product enforcement. |

Peer UID alone does not establish trusted role or human approval. Validate service security context/role, authenticated user/session, request/bytes and approval as designed. A compromised trusted service remains a stated threat-model limitation.

## Deliverable layout

Follow PROJECT §13 ownership; freeze practical module composition under G1.

- core/contract_format/: schema/profile/envelope and vectors (snake_case directory; canonical external service names remain hyphenated).
- core/identityd/, core/vaultd/, core/contractd/, core/policyd/, core/registryd/: substantive safe Rust logic and approved process boundaries.
- core/native/: narrow safe provider/OS adapter APIs, isolated reviewed unsafe/FFI and no policy duplication.
- Cargo.toml, Cargo.lock and qualified toolchain/package manifests: workspace and fixed inputs; generated binaries/target directories remain outside tracked source. Stasha is no longer a submodule.
- build/scripts/qualify_m2_rust.sh, build/scripts/build_m2_core.sh, tests/m2/run_m2_tests.sh: proposed new entry points Claude must implement/document.
- tests/m2/: unit/vector, independent native/encoding oracle, IPC/process, fault-injection and harness-sensitivity tests.
- packaging/ and security/: core package definitions, approved users/roles and confined service deployment inside disposable environments only.
- .github/workflows/m2_checks.yml: pinned inputs/actions, Debian amd64 checks, strict missing-tool/test/evidence failures, bounded caches and no production secrets.
- docs/evidence/M2-cryptographic-contract-core/, docs/reviews/, docs/handoffs/: exact implementation evidence/review/handoff.

These are organizational deliverables, not existing implementations or frozen privileged interfaces. Fixtures use disposable keys and synthetic identities/resources.

## Ordered work packages

Record state, owner, base/target, gates, evidence and reviewer for each package. Claude can progress independent READY work while another dependency awaits owner input.

| ID / initial state | Dependencies | Output and exit |
|---|---|---|
| M2-001 / READY | Memory-safe compiled-language request, branch/Rust research | Pinned Rust/Cargo/workspace/dependency qualification on disposable Debian 13 amd64; safe-core lint controls, real candidate-provider/adapter probes, unsafe/native inventory and test-oracle controls; hashes, supported features and gaps. Probes are unprivileged feasibility work, not service deployment or schema approval. |
| M2-002 / READY | ADR 0001; M2-001 informs G1 | Concrete G1–G6 ADR/interface/vector/verification packages, alternatives, independent review and owner decisions. Can start while qualification proceeds. Exit only after required approvals; staged approval cannot waive another gate. |
| M2-003 / DRAFT | G1/G2 and relevant G3 | Safe Rust strict schema/COSE verifier, deterministic bytes/hash/signature vectors, provider integration and hostile parser evidence; signature validity is separate from current authorization. |
| M2-004 / DRAFT | G1/G3 and key retention under G6 | Identity/authority registry, encrypted vault and exact-proposal signing. Real-provider protection, error, user separation, temporary-authority and approval-replay tests through an approved trusted development auth path. |
| M2-005 / DRAFT | G2–G5, M2-003/004 interfaces | Current authorization and ECR genesis/successor/temporary/explicit-merge admission, immutable objects, exact delta/state, atomic pending indexes and expected head/epoch serialization. |
| M2-006 / DRAFT | G4–G6, M2-005 | Lifetime/one-shot/delegation/revocation, lifecycle gaps, clock/key invalidation, crash/tail/index/anchor rollback recovery, locked/unavailable/full/unknown denial, core fresh forward-restore. |
| M2-007 / DRAFT | G3–G6, M2-004–006 | Real authenticated IPC/confined core processes; Debian packaging/install/upgrade/migration/remove/restart. Authorized path works; unapproved CLI/app/callers fail; missing consumers remain unavailable. |
| M2-008 / DRAFT | M2-003–007 | Complete A01–A15 evidence, strict CI, independent exact-commit architecture/security review, repairs/reruns, integration/handoff/post-merge regression under workflow. |

M2-001/M2-002 are the starting assignment. This does not authorize dependent privileged backend implementation before its design approvals. Compiler blockers require a minimal reproduction and bounded upstream/task/pin review; no silent language replacement or scope reduction.

## Acceptance criteria and required evidence

All criteria require exact source/compiler/runtime/dependency identities, environment, command, artifacts and PASS/FAIL/NOT_RUN/BLOCKED. Pair denial with positive controls. Mandatory NOT_RUN/BLOCKED prevents gate completion.

| ID | Positive / negative controls and exit evidence |
|---|---|
| A01 — pin/build/language | Fresh locked workspace builds/runs on Debian 13 amd64 with exact Rust/Cargo/std/dependency/native hashes. Representative core executes in debug/release profiles, both with overflow checks enabled; record actual optimization/panic settings. Domain crates contain substantive .rs with forbid(unsafe_code); controlled unsafe insertion must fail compilation. Reviewed dependency/adapter inventory contains no shadow policy/ECR core. |
| A02 — test oracle | Fresh isolated Cargo target directory; tests actually executed with nonzero expected counts and no required ignored/filtered/skipped tests. Syntax/link/assertion failure, zero tests, compile-only (--no-run) and stale evidence fail. Independent provider/vector controls, malformed-input fuzzing and unsafe adapter tooling have exact versions and limits; tooling absence cannot count as a safety PASS. |
| A03 — canonical envelope | Published valid contract/COSE/signature/hash bytes match independent oracle. Reject changed fields, duplicate/unknown keys, wrong headers/algo/domain, disallowed/nonminimal/indefinite encoding, malformed length/depth/text, trailing data and oversized inputs under G2. No prose-only signing. |
| A04 — identity/key class | Two synthetic user/key hierarchies work within scope; reject unauthenticated/substituted keys, wrong owner/app/install/executable/session/resource, uncertified rotation, temporary persistent grant and delegated expansion. New key never silently becomes old identity. |
| A05 — vault/approval | Real-provider encrypted key round trip and authorized signing; metadata/ciphertext/tag tampering, wrong credentials, KDF bounds, locked vault, canceled/stale/forged/wrong-byte/head/epoch/reused approval and unauthorized caller deny. Test memory/allocator/lock/core/trace behavior and protection errors under G3; no secrets in evidence. |
| A06 — current policy | Eligible grants pass; mandatory prohibitions win. Wrong scope, expired/session-ended/consumed/revoked/superseded/key-invalid/epoch-invalid/parent-invalid grants deny despite valid signatures. Missing network policy evaluates Offline eligibility, not actual isolation. |
| A07 — DAG/revisions/merge | Valid genesis, successor, temporary branch and explicit multi-parent result; reject missing/foreign parents, illegal lineage, cycles, mismatched delta/state, stale heads, implicit permission union and authority promotion. Historical bytes stay immutable. |
| A08 — atomicity/concurrency | Objects/required pending indexes commit atomically; interrupt every durability boundary, race conflicting writers, replay requests and fail writes. No incomplete authority or duplicate activation; recover justified state or denied/unknown state under G4. |
| A09 — lifetime/consumption/denial | Race one-shot consumers; kill around reservation/success/failure writes. Restart cannot reuse consumed/uncertain grants. Expiry/session/key/policy invalidation and failed explicit-revocation commits deny with locked vault, absent user, unavailable/full storage and restart. Old objects retain history, not validity. |
| A10 — rollback resistance | Fresh anchored state resumes justified operation; stale full store/index/tail/epochs/usage/backups, lost/corrupt/replaced anchor and interrupted anchor updates detect/quarantine. Demonstrate actual anchor assumptions and recovery. Hash-valid ancestry or same-disk duplicate files cannot pass. |
| A11 — restore/delegation | Fresh current-parented target needs current policy/identity/authority and approval. Fresh allowed reauthorization of formerly revoked scope works; old-byte replay, head reset, obsolete identity/resources/keys, ended sessions and consumed one-shots reject. Delegation narrows; parent revocation invalidates dependent descendants. |
| A12 — IPC/boundaries | Approved trusted clients work with real credentials/roles; unauthenticated clients, same-human-UID ordinary callers, forged identity/program/request, unauthorized signing/admin/history and malformed/flooded IPC deny. Attribute tested OS boundaries precisely; same-user application sandboxing remains M3. |
| A13 — confirmation separation | Eligible durable commit remains pending until correctly attributed acknowledgement. Wrong/stale/reused runtime, commit, epoch/domain confirmations fail. Fixture consumers are labeled; absent backend cannot report active resource enforcement. Failed/partial transitions never expand authority. |
| A14 — privacy/quotas/migrations | Trusted history/view/export only, bounded escaped rationale and no secrets. Full disk, deletion, backup/migration/update identity follow G6 and cannot restore stale authority or claim complete audit history. |
| A15 — package/regression/review | Clean Debian packages install/upgrade/restart/remove with approved confinement/lifecycle and dependency/lint evidence; no production keys. Affected M0/M1 checks pass. Independent reviewers approve exact implementation and required evidence; status/handoff distinguish later runtime obligations. |

Fuzz bounded parsing/DAG/IPC and preserve reproductions. Verify sensitivity with controlled defects and independent provider/C/encoding oracles. Test ownership/error cleanup and supported sanitizer paths with explicit limits; unsupported instrumentation cannot become a general memory-safety claim. Actual Debian/KDE resource/network enforcement belongs to later integration.

## Build and verification entry points

No Rust workspace or M2 scripts exist at this scope-change baseline. Claude implements the proposed interfaces; this outline is not executed product verification. Install dependencies only inside a disposable Debian 13 amd64 environment, using authenticated exact package inputs frozen under G1. Record rustc -Vv, cargo -V, std/target/linker/native versions, package hashes, feature graph and Cargo.lock digest.

Once the workspace and dependencies are prepared, required Cargo operations include:

~~~sh
cargo metadata --locked --offline --format-version 1
cargo build --workspace --frozen
cargo build --workspace --release --frozen
cargo test --workspace --frozen
cargo test --workspace --release --frozen
~~~

Use an isolated CARGO_TARGET_DIR under ignored out/ for each verification run. Frozen requires both a fixed lockfile and already available dependency sources. Dependency acquisition/vendoring is a separate authenticated, recorded preparation step; do not let build scripts fetch uncontrolled inputs. Freeze and check overflow-checks = true for release and custom profiles too, and explicitly record panic strategy. Add format/lint checks with pinned tool versions and targeted naming allowances.

Cargo's ordinary test harness ignores panic=abort. Verify deployed panic/abort and process-death behavior using separate subprocess fault-injection tests against binaries built with the actual approved deployment profile; cargo test alone cannot establish abort cleanup or recovery.

Required future repository interfaces (create/document, not yet runnable):

~~~sh
sh build/scripts/qualify_m2_rust.sh
sh build/scripts/build_m2_core.sh
sh tests/m2/run_m2_tests.sh unit
sh tests/m2/run_m2_tests.sh integration
sh tests/m2/run_m2_tests.sh recovery
sh tests/m2/run_m2_tests.sh packaging
sh tests/m2/run_m2_tests.sh harness
~~~

Dispatcher must fail unknown suites, missing prerequisites, compile/link/runtime failures, stale output, required skips/zero tests, missing evidence and failed controls. Cargo test normally compiles and executes; --no-run does not execute and cannot satisfy acceptance. Validate suite inventory/counts and exit statuses rather than accepting a zero-test Cargo success. Use exact toolchain and native-provider fixtures; record Miri/sanitizer/fuzzer availability and limits without treating them as proof of the entire dependency graph.

## Security and failure behavior

Use disposable Debian environments and synthetic keys/users for service/package/credential/privileged tests. No production secret use, host deployment, key export or secret logging. Treat text, arguments, paths, imported objects and lengths as untrusted.

Mandatory denial does not wait for ECR closure evidence. Report gaps under G5 and keep unresolved authority denied; never issue background user signatures, infer complete history or reset uncertain consumption. New authority needs durable verified signed admission/current validity; real resources need enforcement confirmation.

Panics/aborts/service crashes cannot leave continuing expanded authority or falsely confirmed state. Freeze supervisor/recovery/freshness behavior; do not weaken checks to accommodate unsafe/FFI/compiler limits.

## Completion, review and handoff

- Record each package's exact base/target, owner, changes, tests/controls/environment, gaps and next action under [WORKFLOW.md](../engineering/WORKFLOW.md), [ORCHESTRATION.md](../agents/ORCHESTRATION.md) and templates.
- Author cannot independently approve sensitive designs/code/tests. Route exact candidates to Codex, resolve findings and repeat affected review/checks.
- Backend work cannot be READY before gates; M2 cannot be DONE with mandatory NOT_RUN/BLOCKED or missing security review.
- Final handoff includes .rs/unsafe/native ownership inventory, provider/wire/storage/anchor decisions, A01–A15 evidence, package/runtime footprint, pin/provenance, exact implementation/review/integration commits and later-milestone obligations.
- Merge/push/release require applicable owner authorization and gates. This request prepares the task branch and packet, not publication.

## Current decisions / blockers / next action

Owner-authorized scope: memory-safe compiled core; Rust selected for the implementation assignment and Stasha submodule removed for now; ADR 0001 scoped architecture and immediate denial; repository workflow/security/naming.

Pending: G1–G6 detailed designs and Debian-specific Rust/dependency qualification. Historical Stasha/Ubuntu receipts qualify neither Rust nor M2.

Next: Claude begins M2-001/M2-002 on this branch, preserving preparation commits; produce useful qualification/probes/designs, present exact independently reviewed choices for owner approval, then continue each admitted dependency through M2-008 without inventing approval or reducing the full milestone.

## Implementation progress (Claude Code, 2026-10-12)

Implementation baseline: a4bc0c4371f2298787f0876d81c0c6fd21150ea6 (clean branch head after the Rust-transition review). Exact candidate commits and evidence are in the [M2-001/M2-002 handoff](../handoffs/M2-001-002-qualification-design.md).

| Package | State | Output | Gate / next |
|---|---|---|---|
| M2-001 | IMPLEMENTED → IN_REVIEW | Cargo workspace on Debian 13 Rust 1.85.1 with Debian-packaged offline crates; pinned qualification container and arm64 locks; per-file crate source manifest; domain skeletons with `#![forbid(unsafe_code)]`; `core/native/secure_memory` (only unsafe) and safe `core/native/os_adapter`; provider probe with published KATs and an independent OpenSSL/libsodium/libargon2 oracle; strict dispatcher with inventory-exact validation, harness canaries, lint controls, deployed-profile probes, naming checker, dependency/unsafe inventory, `qualify_m2_rust.sh`, `build_m2_core.sh`, `m2_checks.yml`. arm64 development qualification PASS; **amd64 qualification NOT_RUN** (owner instruction: reviewer runs it in the cloud). | Independent review + amd64 qualification; G1 approval. |
| M2-002 | IMPLEMENTED → IN_REVIEW | PROPOSED ADRs [0003 (G1)](../decisions/0003_m2_rust_toolchain_dependencies.md), [0004 (G2)](../decisions/0004_m2_wire_schema_hash.md), [0005 (G3)](../decisions/0005_m2_identity_vault_ipc.md), [0006 (G4)](../decisions/0006_m2_storage_anchor_recovery.md), [0007 (G5)](../decisions/0007_m2_lifecycle_time_consumption.md), [0008 (G6)](../decisions/0008_m2_retention_privacy_update_identity.md); [wire format](../specs/m2_wire_format_v1.md), [CDDL](../specs/m2_ecr_v1.cddl), [service IPC](../specs/m2_service_ipc_v1.md); [ECR v1 vectors](../../tests/m2/vectors/ecr_v1/vectors_readme.md) from an independent oracle. | Independent review of each gate; then owner decisions, including the explicit choices in ADR 0006 §5, 0007 §4, 0008 §3. |
| M2-003 … M2-008 | DRAFT | — | Not READY: every dependency gate is PROPOSED, none approved. |
