# Task: M2 — Cryptographic contract core and Eternal Contract Repository

- **State:** IN_PROGRESS. M2-001, M2-002, M2-003 and M2-004 are DONE; G1–G6 were owner-approved on 2026-10-12 after independent review (ADRs 0003–0008). M2-005 is DONE (independent re-review APPROVE of revision 4, 2026-10-13). M2-006 is IN_PROGRESS with its staged scope recorded (2026-10-13, below); stage A is DONE (independent re-review APPROVE of revision 4, 2026-10-14) and stage B1 is being implemented; M2-007 and M2-008 become READY as their package dependencies complete (see the progress table). The milestone is not DONE until every package has its independent review, M2-008 and the final review below.
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

G1–G6 are owner-approved (2026-10-12, ADRs 0003–0008, with the owner choices recorded there). Historical Stasha/Ubuntu receipts qualify neither Rust nor M2. Open later questions: authenticated time source (not adopted for M2; would need its own ADR) and recovery from a large accepted forward clock error (deferred; M2 never lowers trusted time).

Next: Claude implements M2-003, then continues each package in dependency order through M2-008 (historical first instruction: Claude begins M2-001/M2-002 on this branch, preserving preparation commits; produce useful qualification/probes/designs, present exact independently reviewed choices for owner approval, then continue each admitted dependency through M2-008 without inventing approval or reducing the full milestone).

## Implementation progress (Claude Code, 2026-10-12)

Implementation baseline: a4bc0c4371f2298787f0876d81c0c6fd21150ea6 (clean branch head after the Rust-transition review). Exact candidate commits and evidence are in the [M2-001/M2-002 handoff](../handoffs/M2-001-002-qualification-design.md).

| Package | State | Output | Gate / next |
|---|---|---|---|
| M2-001 | DONE (owner-approved G1, 2026-10-12) | Cargo workspace on Debian 13 Rust 1.85.1 with Debian-packaged offline crates; pinned qualification container and arm64 locks; per-file crate source manifest; domain skeletons with `#![forbid(unsafe_code)]`; `core/native/secure_memory` (only unsafe) and safe `core/native/os_adapter`; provider probe with published KATs and an independent OpenSSL/libsodium/libargon2 oracle; strict dispatcher with inventory-exact validation, harness canaries, lint controls, deployed-profile probes, naming checker, dependency/unsafe inventory, `qualify_m2_rust.sh`, `build_m2_core.sh`, `m2_checks.yml`. arm64 development qualification PASS; **amd64 qualification NOT_RUN** (owner instruction: reviewer runs it in the cloud). | Done: amd64 qualification PASS (author VM, CI and the reviewer's own run); independent re-review r5 APPROVE; G1 approved with a pre-M2-007 advisory re-check condition (ADR 0003). |
| M2-002 | DONE (owner-approved G1–G6, 2026-10-12) | APPROVED ADRs [0003 (G1)](../decisions/0003_m2_rust_toolchain_dependencies.md), [0004 (G2)](../decisions/0004_m2_wire_schema_hash.md), [0005 (G3)](../decisions/0005_m2_identity_vault_ipc.md), [0006 (G4)](../decisions/0006_m2_storage_anchor_recovery.md), [0007 (G5)](../decisions/0007_m2_lifecycle_time_consumption.md), [0008 (G6)](../decisions/0008_m2_retention_privacy_update_identity.md); [wire format](../specs/m2_wire_format_v1.md), [CDDL](../specs/m2_ecr_v1.cddl), [service IPC](../specs/m2_service_ipc_v1.md); [ECR v1 vectors](../../tests/m2/vectors/ecr_v1/vectors_readme.md) from an independent oracle. | Done: independent re-review r5 APPROVE for every gate; owner decisions ADR 0006 §9.1 A, §9.2 C1; ADR 0007 §4 A, §8 B (trusted time never lowered; forward-error recovery deferred); ADR 0008 §3 A. |
| M2-003 | DONE (independent re-review APPROVE, 2026-10-13) | `cham3_contract_format`: strict `cham3_dcbor_v1` decoder/encoder, COSE_Sign1 envelope with strict Ed25519 verification before payload parsing, closed typed schemas with lossless re-encoding, layer-5 commit semantics, hashes; vector, mutation and oracle-differential evidence ([handoff](../handoffs/M2-003-format-verifier.md)). Review 1cb93c0 CHANGES_REQUESTED; revision 2 **8b0ccf3** with owner errata E1–E3 approved by re-review 87e9b2a ([completion handoff](../handoffs/M2-003-completion.md)). | Done. M2-004 obligation: identityd calls `public_key_is_valid` on enrolled/certified keys. |
| M2-004 | DONE (independent re-review APPROVE for the staged scope, 2026-10-13) | `cham3_identityd`: caller roles from peer credentials, layer-6 key-authority index over the journal with scoped `resolve_key`, registration with the M2-003 key-validation obligation, restrictive invalidation; `cham3_vaultd`: encrypted key objects (Argon2id, XChaCha20-Poly1305, secret regions), restricted store, exact-proposal approval and signing, attempt limiting, restartable creation/rotation/invalidation/recovery keys, process protection ([handoff](../handoffs/M2-004-identity-vault.md)). Candidate bb3f1ba; review f2d82b7 CHANGES_REQUESTED (R01–R05; I01); revision 2 ca40c62 ([resolutions](../reviews/M2-004-finding-resolutions.md), [handoff](../handoffs/M2-004-rereview-r2.md)); its two revision-2 records disagreed (fd6de58 CHANGES_REQUESTED with M2-004-R2-01; the combined f9f3b82 APPROVE); **revision 3 051b4c8** fixes M2-004-R2-01 ([resolutions r2](../reviews/M2-004-finding-resolutions-r2.md), [handoff r3](../handoffs/M2-004-rereview-r3.md)) and is **approved** by the re-review d300b60, which reconciles the records (the earlier CHANGES_REQUESTED stood for revision 2) ([completion handoff](../handoffs/M2-004-completion.md)). | Done for the staged scope. Carried: `replace_identity` and vault-side deletion cleanup (M2-006); restriction ledger across restarts and cross-service serialization (M2-006/007); transport and binaries (M2-007); duplicate request ids at admission (done in M2-005). |
| M2-005 | DONE (independent re-review APPROVE of revision 4, 2026-10-13) | `cham3_registryd`: per-user chained journal with durable appends, directory durability and a preallocated restrictive reserve, fail-closed replay through layers 1–7, layer-7 admission (placement, state, uniqueness including ended sessions, restore, subject binding, branch scope, network slot), `admit` under a per-user lock with the signing-key window, epoch and quotas, the identity store identityd runs over, `lineage_head`; `cham3_policyd`: `evaluate`/`network_mode` (ADR 0007 §1) and service operations; `cham3_contractd`: `submit_signed`, `verify_object` ([handoff](../handoffs/M2-005-admission-authorization.md)). Candidate b06d7cb; review f9f3b82 CHANGES_REQUESTED (R01–R03, Medium); revision 2 051b4c8 answers them ([resolutions](../reviews/M2-005-finding-resolutions.md), [handoff](../handoffs/M2-005-rereview-r2.md)); re-review d300b60 CHANGES_REQUESTED (R2-01, R2-02, R2-03: failure and restart paths of R01 and R03, the probe's cleanup); revision 3 be24152 answers them ([resolutions r3](../reviews/M2-005-finding-resolutions-r3.md), [handoff r3](../handoffs/M2-005-rereview-r3.md)); re-review 4865d7b CHANGES_REQUESTED (R3-01: the full-disk wrapper's pre-deletion); **revision 4 5860f276ab593da34771f704691ea35649324d6e** answers it ([resolutions r4](../reviews/M2-005-finding-resolutions-r4.md), [handoff r4](../handoffs/M2-005-rereview-r4.md)); re-review 165e149 **APPROVE** ([completion handoff](../handoffs/M2-005-completion.md)). | Delegation admission, lifecycle writers, anchor/ledger/recovery, ceremony, trusted time, one-shot reservations, confirmation, `prepare_proposal`, reserve status reporting move to M2-006; transport to M2-007. |
| M2-006 | IN_PROGRESS — stage A DONE (review 06a9423 APPROVE of revision 4, db5bb59, 2026-10-14, after 344c372, 7d570c1 and f0d6eb0 CHANGES_REQUESTED; R2-03 limit owner-accepted as ADR 0006 erratum E5, corrected wording owner-confirmed 2026-10-14); stage B split into B1 and B2 (owner, 2026-10-14), B1 IN_PROGRESS; stages C–D not started | Staged candidates, each independently reviewed before the next: A storage integrity (anchor, run marker, restriction ledger, recovery, quarantine, status); B lifecycle and time, delivered as B1 (trusted-time clock, `time_sample`, `time_floor`, `clock_anomaly`, the option-B observed counter, `timestamp_rule`) and B2 (lifecycle writers, one-shot reservations, confirmation, reconciliation, history); C ceremony and deletion; D proposals, restore and delegation. See the scope record below. | Built on M2-005 revision 3 (in review); an M2-005 finding may require rework. |
| M2-007, M2-008 | DRAFT | — | Gates approved; each waits for its package dependencies (M2-004–006, M2-003–007). |

### Independent review and revision 2 (2026-10-12)

- Independent Codex review of f1437f0d254af2968c3598bf14d1a9b2b8e791cd / 3c35aac0e8d93fef334fc4f208e1365857b1e84a, published on `main` at 9cf882503848f8c6cc4938a588cc9fc062a9392d (PR #1): **CHANGES_REQUESTED** for M2-001 and for each of G1–G6 (findings R01–R11, notes I01–I03). Native amd64 qualification reproduced all expected passes; only the two then-missing amd64 lock checks failed.
- Revision 2 on this branch addresses every finding (author response: [finding resolutions](../reviews/M2-001-002-finding-resolutions.md); exact commits and evidence: [re-review handoff](../handoffs/M2-001-002-rereview.md)). M2-001 and M2-002 return to **IN_REVIEW**. No gate is approved; M2-003..M2-008 remain DRAFT. A new owner choice (lifting containment after an unverifiable unclean run, ADR 0006 §9.2) joins the three earlier ones.

### Re-review and revision 3 (2026-10-12)

- Independent re-review of a2bbb370c867da57fdc72274fb17ca19a23426e4 / 6b4ef9d87d69185ff7205b51fa417df1fed5c6f6 on `review/m2-001-002-r2-security` at 336f13be6a19bb871ea956d64f9cacb0936fe028: **CHANGES_REQUESTED** for M2-001 and G1–G6 (R01-r2 failed lock hashing still reported identity PASS; R07-r2 network uniqueness per state only; R09-r2 trusted time not rebased/persisted; R12 restore of ancestral session scope as persistent; R13 containment blocked recovery signing). Items R02–R06, R08, R10, R11 and I01–I03 passed within their stated scope and are unchanged.
- Revision 3 (base 6b4ef9d, candidate **84fa6ef172b3fad21bb079549776252bb65624d6**) answers all five ([author response](../reviews/M2-001-002-finding-resolutions-r2.md); [handoff](../handoffs/M2-001-002-rereview-r3.md)). Native amd64 qualification in the author's disposable VM container and GitHub Actions m2/m0/m1 checks PASS at the candidate. M2-001 and M2-002 return to **IN_REVIEW**; every proposal is PROPOSED (revision 3); no gate is approved; M2-003..M2-008 remain DRAFT. ADR 0006 §9.2 is rewritten to refer to the complete recovery ceremony (§10); owner decisions G1–G6, §9.1, §9.2, ADR 0007 §4 and ADR 0008 §3 remain open.

### Re-review and revision 4 (2026-10-12)

- Independent re-review of 84fa6ef172b3fad21bb079549776252bb65624d6 / 42397304663bff6057e26118014c18e7b90a2040 on `review/m2-001-002-r3-security` at f451f2e346e6981b24802a7e9070ba09303c40d3:
  - **CHANGES_REQUESTED** for M2-001, G1, G3, G4, G5 and G6; **APPROVE for owner consideration** for G2.
  - New findings:
    - R14: recovery key naming had no authenticated registry IPC route; R13 was FAIL only through it.
    - R15: the cross-boot and future-start time limits were overstated.
  - G1 additionally needs the reviewer's own Debian tracker check, which was BLOCKED by the reviewer's network (HTTP 403).
- Revision 4 (base 4239730, candidate **9b1aa2b9d1435fb33c303ca7fe01ce7afcb2964a**) answers R14 and R15 ([author response](../reviews/M2-001-002-finding-resolutions-r3.md); [handoff](../handoffs/M2-001-002-rereview-r4.md)).
- Native amd64 qualification in the author's disposable VM container PASS at the candidate; GitHub Actions m2/m0/m1 checks PASS at the candidate.
- M2-001 and M2-002 return to **IN_REVIEW**. Every proposal is PROPOSED; no gate is approved; M2-003..M2-008 remain DRAFT.
- Owner decisions, all open:
  - G1–G6;
  - ADR 0006 §9.1 and §9.2;
  - ADR 0007 §4 and the new **§8** (time-bound strength);
  - ADR 0008 §3;
  - how G1's independent Debian check is to be obtained.

### Re-review of revision 4 and revision 4.1 (2026-10-12)

- Independent re-review of 9b1aa2b9d1435fb33c303ca7fe01ce7afcb2964a / 70c431c13a1c76fd2948e6c8f0ae33b720618043 on `review/m2-001-002-r4-security` at 3a3b5d17ddc44b96c3fcc8265c5b337066aff0fb:
  - **APPROVE for owner consideration** for G2, G3, G4, G5 and G6. These are design recommendations, not owner approvals.
  - **CHANGES_REQUESTED** for M2-001 and G1, only because the reviewer's own Debian tracker retrieval was BLOCKED.
  - R13, R14 and R15 PASS. Low, nonblocking note I04 on G4 model-test coverage.
- Revision 4.1 (candidate **28a8595e627bcd092acaff4b8161522cf5d38f7f**, test/oracle-only) answers I04 ([author response](../reviews/M2-001-002-finding-resolutions-r4.md); [handoff](../handoffs/M2-001-002-rereview-r5.md)).
- The owner now offers a desktop environment where the independent reviewer can perform the Debian check with its own retrieval.
- No gate is owner-approved; M2-003..M2-008 remain DRAFT.

### Re-review of revision 4.1 and owner decisions (2026-10-12)

- Independent re-review of 28a8595e627bcd092acaff4b8161522cf5d38f7f (docs-only ed2ba0d, 87c9cbc) on `review/m2-001-002-r5-security` at bd081df26bc31636b01bf470bd9b1fea9be7d20d (`docs/reviews/M2-001-002-codex-security-rereview-r5.md`, SHA-256 `77ec6d5f…b178af`, verified; not merged), run on the owner's desktop:
  - **APPROVE** for M2-001 and for each of G1–G6. I04 PASS; R13/R14/R15 and every earlier item intact.
  - The reviewer's own amd64 qualification PASS, static checks and static tests PASS, CI comparison PASS (artifact downloaded and verified).
  - The reviewer's own advisory check PASS: Debian tracker 2026-10-12T22:00:46Z (SHA-256 `3464ca13…0e0848`), RustSec 57dcc0f; 0 behind, 23 open not-unimportant items with the author's triage agreed, 0 RustSec affected. Before M2-007: re-check the loader/setgid and systemd items and re-qualify any changed pin.
- **Owner decisions (2026-10-12, answered by the owner to explicit questions):**
  - G1–G6: **approve all six**.
  - ADR 0006 §9.1: **Option A**, `same_volume` degraded default anchor with visible status.
  - ADR 0006 §9.2: **Option C1**, the user opens the recovery ceremony; after quarantine an admin `recover_user` first.
  - ADR 0007 §4: **Option A**, the reservation consumes.
  - ADR 0007 §8: **Option B**, observed-time start eligibility (M2-006).
  - ADR 0007 §8 open question: **defer; never lower trusted time** in M2.
  - ADR 0008 §3: **Option A**, whole-account deletion only.
- M2-001 and M2-002 are **DONE** ([completion handoff](../handoffs/M2-001-002-completion.md)). M2-003 is READY and started.

### M2-003 implementation (2026-10-12)

- Candidate **4841039d772ca923e09a55f1505f119317e73a91** on base ecc8213: verification layers 1–5 of wire format §1 in `core/contract_format` (safe Rust; no new dependency, Cargo.lock gains only edges to the pinned `ed25519-dalek` and `sha2`). Details, evidence and the review prompt: [M2-003 handoff](../handoffs/M2-003-format-verifier.md).
- The differential check against the independent oracle found two oracle defects (trailing newline and non-ASCII digits accepted by its patterns), fixed in test code only; the vectors are unchanged.
- Owner decision requested: erratum E1. Wire format §§6.1/7 say `list_too_long`, while the approved vectors use `list_length`, for too short as well as too long.
- M2-004 is not started (one writer on this branch); it starts after the M2-003 review.

### M2-003 review and revision 2 (2026-10-13)

- Independent review of 4841039 / f87111d on `review/m2-003-security` at 1cb93c0454583311b04fbbb9e90f01e18aab5ca3 (SHA-256 `e33e8ad4…1f98`, verified; not merged): **CHANGES_REQUESTED**.
  - Scope, boundaries, qualification, static gates, the oracle fix and the CI comparison PASS.
  - Findings: R01 (protected-header decode precedence), R02 (declared-capacity reservation), R03 (unknown `typ` code contradicted the prose), R04 (text-list comparator unspecified), all Low; I01 (expiration-equality coverage), Info.
  - The reviewer's Codex Security plugin workflow was BLOCKED in its environment.
- **Owner decisions, 2026-10-13** (answers to explicit questions, recorded in ADR 0004):
  - E1: `list_length`;
  - E2: any text `typ` other than the expected one is `wrong_object_type`;
  - E3: text lists use deterministic-encoding order.
- **Revision 2** candidate **8b0ccf37f2437cae0a9668e012ddf55e9952e2aa** answers every finding ([resolutions](../reviews/M2-003-finding-resolutions.md), [handoff](../handoffs/M2-003-rereview-r2.md)).
  - Qualification PASS: unit 64+64, ECR 19/96/18, differential 5 950 / 0 mismatches, harness 130.
  - GitHub Actions m2/m1/m0 PASS.
- **M2-004 obligation:** identityd must call `envelope::public_key_is_valid` on enrolled and certified public keys before registration.

### M2-003 re-review and completion (2026-10-13)

- Independent re-review of 8b0ccf3 / ce83f28 on `review/m2-003-r2-security` at 87e9b2a888671ab9b7cd86272d34e8a1c48cd5ac (`docs/reviews/M2-003-codex-security-rereview-r2.md`, SHA-256 `55605db4…2518`, verified; parent 9cf8825; adds only that file; not merged): **APPROVE** for M2-003.
  - R01–R04 and I01 PASS; earlier PASS items intact; errata E1–E3 recorded and applied consistently; no new findings.
  - The reviewer's own amd64 qualification, static checks/tests, scripts and CI comparison PASS.
  - The Codex Security plugin skill was BLOCKED again in the reviewer's environment; it remains required at the final M2-008 review.
- M2-003 is **DONE** ([completion handoff](../handoffs/M2-003-completion.md)). M2-004 is READY and started.

### M2-004 implementation (2026-10-13)

- Candidate **bb3f1bad77080da34069af62c8d2cc390f2fcf7e** on base 20c849b (implementation 0e4f993, static-gate fix e5be206, protection-probe fix bb3f1ba): identityd and vaultd in safe Rust (details, decisions, evidence and the review prompt: [M2-004 handoff](../handoffs/M2-004-identity-vault.md)).
  - Cargo.lock gains only dependency edges; no new crate or version.
  - os_adapter adds a safe memlock limit query and raise.
- Evidence:
  - The published key-authority journal scenarios replay through the Rust index with 0 disagreements (318 comparisons).
  - An independent libargon2/libsodium oracle checks the key-object format (38 checks).
  - 14 seeded identity/vault defects are each detected.
  - The vault's process protection fails closed without a sufficient memlock limit; its positive path runs as a CI step with a raised limit.
- **Scope sequencing (recorded before the work):** `replace_identity` (no journal record format and no layer-6 code for a replaced identity's keys yet) and vault-side deletion cleanup (no vaultd IPC route yet) move to M2-006, with `recover_user` and deletion. registryd's journal and the ceremony operations are labeled fixtures until M2-005/006; transport, binaries, the setgid client and systemd units are M2-007.
- M2-005 is not started (one writer on this branch); it starts after the M2-004 review. (Superseded on 2026-10-13 by the owner instruction below.)

### M2-004 review and revision 2 (2026-10-13)

- Independent review of bb3f1ba / d2c29ad on `review/m2-004-security` at f2d82b7a663c1579582bc5c88fe2832a6f97e061 (SHA-256 `eb0b011f…596a`, verified; not merged): **CHANGES_REQUESTED**.
  - Scope, boundaries, secret custody, qualification, the protection positive path, static gates and the CI comparison PASS; author decisions 1–8 accepted in intent (decision 4's implementation failed R01).
  - Findings: R01 (an older invalidation retry cleared a pending compromise), R02 (a live proposal extended a temporary key's window), R03 (no authority recheck after unlock), R05 (a definitive naming error erased still-nameable recovery keys), all Medium; R04 (used proposal ids reusable through cleanup), Low; I01 (recovery completion is not fresh signing), observation.
  - The reviewer's Codex Security plugin workflow was BLOCKED again.
- No owner decision was needed; every change follows the approved ADRs and the IPC specification.
- **Revision 2** candidate **ca40c62ed7a5fc29ba8910fef514f3ce29ea4fba** (one commit on d2c29ad) answers every finding: the [finding resolutions](../reviews/M2-004-finding-resolutions.md) and the [re-review handoff](../handoffs/M2-004-rereview-r2.md) have the details.
  - Qualification PASS: unit 121+121, harness 133 with 20 seeded identity/vault defects; static gates PASS.
  - The review's own 17 tests: 11/6 at bb3f1ba, 16/1 at ca40c62 (the remaining failure is I01's stricter assertion).
  - CI m2 38347259191 (including the protection positive path), m1 38347259188, m0 38347259258 PASS.
- **Carried obligations recorded:** M2-006 restriction ledger (a not-yet-durable denial across an identityd restart); M2-006/M2-007 serialization of restrictions with signatures across services; M2-005 admission rejects duplicate request ids.

### Owner instruction: deferred reviews, M2-005 started (2026-10-13)

- **Owner instruction (2026-10-13), relayed with a copy of the already recorded M2-003 re-review verdict:** "If things are unable to be checked now, move on and mark them necessary for review later, I'll let yall know when I'm back on my main computer."
  - The relayed verdict is the M2-003 re-review already applied in 20c849b. Its record was verified again: `review/m2-003-r2-security` at 87e9b2a888671ab9b7cd86272d34e8a1c48cd5ac, SHA-256 `55605db4…2518`. Nothing changes.
  - No independent review can run now. The implementer therefore continues with the next package and marks every unreviewed package as **requiring independent review later**. No approval is inferred: M2-004 stays **IN_REVIEW**, with its revision-2 re-review of ca40c62 pending, and is not DONE.
  - M2-005 starts on top of the unreviewed M2-004 revision 2. Consequences: an M2-004 finding may require M2-005 rework, and M2-005's review follows M2-004's re-review or is combined with it. The owner decides the order when back.
- **M2-005 scope, recorded before the work:**
  - **registryd: per-user journal** (ADR 0006 §1).
    - Frames and chain as specified: `"C3JR"`, length, record, chain.
    - Records are closed `cham3_dcbor_v1` maps of at most 65 536 bytes (the wire format §2.3 container budget).
    - Durable append: `fdatasync`, plus a directory `fsync` at creation.
    - Replay re-verifies the chain and every record through layers 1–7. Any defect makes the user's journal unusable (fail closed).
    - Indexes are derived in memory, never stored.
  - **registryd: identity and admission operations.**
    - `append_identity` and `identity_state` through identityd's journal interface, so identityd runs over the real store.
    - Commit admission: layers 1–6 from the same journal snapshot, then layer 7 in wire format §7 order. That covers parents, lineage, placement and `stale_head` (§6.11), base and result state, delta, `grant_id` and `request_id` uniqueness (including the M2-004 obligation: duplicate request ids rejected), restore (§6.7), subject binding (§6.9), branch rule and network slot (§6.12).
    - Policy epoch equal to the current epoch (`stale_epoch`).
    - A per-user writer lock serializes checks and append.
    - Quotas (`quota_exhausted`): journal size and lineages.
    - `lineage_head`.
    - Commits are admitted `pending`.
  - **Lifecycle records** `expired`, `session_ended` and `restriction`: their record format, validation (`lifecycle_rule`) and effect on the index (ended grant ids, network slot) are M2-005. The operations that write them are M2-006: `restrict` with the restriction ledger, the expiry latch and `session_event`.
  - **Interim restriction, fail closed:** a grant with `delegation.parent_grant_id` is refused at admission (`delegation_rule`) until M2-006 implements the delegation checks of wire format §6.4 (no published vector delegates).
  - **policyd:** `evaluate` and `network_mode` (ADR 0007 §1, rules 1–7), as pure functions of a consistent registry snapshot, identity key resolution, mandatory policy, policy epochs, trusted now, session liveness and one-shot reservations.
    - The mandatory-policy representation is an M2 configuration model (category/resource/operation prohibitions, forbidden network modes, emergency restriction), not an administrator policy language. It is an author decision for review.
    - The admin operations `set_policy_epoch` and `set_emergency_restriction` are restrictive changes and move to M2-006 with the restrictive write path. M2-005 configures policy state at construction.
  - **contractd:** `submit_signed` (layers 1–6 through identityd, then registryd `admit`) and `verify_object`.
    - `prepare_proposal` and `check_proposal` move to M2-006 with forward-restore and the recovery proposals.
    - `request_revocation` moves to M2-006 with the restriction path.
  - **M2-006:**
    - anchor, run marker, restriction ledger, torn-tail and crash recovery, quarantine;
    - containment and the recovery ceremony operations, and `recovery_rule` at admission;
    - delegation admission;
    - `reserve_use`/`complete_use`, `confirm_enforcement`, `session_event`, `history`, `time_sample`;
    - the trusted-time service clock, and with it the ±300 s commit-timestamp admission check of ADR 0007 §5.
  - **Open question for the owner, no change made:** the specifications assign no error code to the ±300 s commit-timestamp check (wire format §7 and the IPC error vocabulary). It needs an erratum before M2-006 implements it. (Answered 2026-10-13: `timestamp_rule`, ADR 0007 erratum E4; see the M2-006 scope record.)
  - **M2-007:** transport, binaries and service processes. Until then M2-005 is an in-process composition.

### M2-005 implementation (2026-10-13)

- Candidate **b06d7cb21e1e3e42bbcd025abd9cea131574751d** on base 6968bd6 (the scope record): the implementation 6e9d6d3 and the fix b06d7cb. Details, decisions, evidence and the review prompt: [M2-005 handoff](../handoffs/M2-005-admission-authorization.md).
  - registryd, policyd and contractd in safe Rust. Cargo.lock gains only dependency edges; no new crate or version.
  - Shared changes: `contract_format::dcbor::decode_container` (the wire format §2.3 container limit) and identityd's admission codes.
- Evidence:
  - The published admission scenarios replay through the Rust index with 0 disagreements (10 scenarios, 53 steps, 90 comparisons); all 18 admission-invalid vectors and the published lineage match.
  - Real journal files: replay, tampering, durability faults, roles, epochs, quotas and their reserve, 16 rounds of conflicting concurrent writers, and an end-to-end vault-signed commit that is admitted and evaluated eligible across a restart.
  - 28 seeded registry/policy/contractd defects are each detected.
  - Qualification PASS (unit 157+157, harness 135); CI m2 38404018473, m1 38404018403, m0 38404018497 PASS. The first attempt 6e9d6d3 failed on rustfmt only.
- **Review:** required, deferred by the owner. The owner reports that the Codex cloud environment now has unrestricted internet domain access, so the items earlier reviews recorded as BLOCKED (artifact download, the Codex Security plugin, upstream retrievals) are expected to run; the review prompt asks for them.
- **Open owner question (unchanged):** no error code is assigned to the ±300 s commit-timestamp check.
- M2-006 is next under the owner's instruction; its scope is recorded before its work.

### M2-004 revision-2 re-reviews and M2-005 review; revision 2 of M2-005 and revision 3 of M2-004 (2026-10-13)

- **Two independent records for M2-004 revision 2 (ca40c62) disagree.** `review/m2-004-r2-security` at fd6de588bbee575d936fde448e823bff08f44e7a (`docs/reviews/M2-004-codex-security-rereview-r2.md`, SHA-256 `8736aee2…abae7`, verified; parent 9cf8825; adds only that file; not relayed by the owner, found on the remote): **CHANGES_REQUESTED** — R01, R02, R03, R05 PASS, I01 documentation corrected, R04 FAIL through one new Low finding, **M2-004-R2-01** (pending-to-retired proposal transitions exceeded the retained-id bounds 32/256). The later combined record below: **APPROVE for the staged scope**, "no new findings", without mentioning the first record. No approval is inferred; **M2-004 stays IN_REVIEW**, the finding is fixed as revision 3, and the owner is asked which revision-2 verdict governs ([resolutions](../reviews/M2-004-finding-resolutions-r2.md)).
- **Independent review of M2-005** (b06d7cb / 24a35c1) combined with the M2-004 revision-2 re-review, on `review/m2-005-security` at f9f3b828fcd117ebcaa4179fa6ae05e782381db1 (`docs/reviews/M2-005-codex-security-review.md`, SHA-256 `8b36e400…a018`, verified; parent 9cf8825; adds only that file; not merged): **CHANGES_REQUESTED** for M2-005.
  - Scope, boundaries, the journal format, paths, replay validation, the frozen differential, the admission snapshot/locking/identity store/roles, the remaining authorization rules and contractd PASS; author decisions 1, 3–10 and 12 accepted (2 and 11 subject to R01, 7 insufficient without R02, the reserve shortfall not a decision).
  - Findings, all Medium: **R01** (directory durability skipped on an enrollment retry and at store creation), **R02** (a new grant for a durably ended session regained eligibility with a still-fresh attestation; the oracle shared the gap), **R03** (the restrictive reserve was a quota allowance, not preallocated blocks: actual filesystem exhaustion prevented a key invalidation).
  - The reviewer's qualification, protection positive path, static gates, artifact digests and CI comparison PASS; the Codex Security plugin was BLOCKED again (preflight resources unreadable) despite network access.
- No owner decision was needed for the changes; every change follows ADR 0006 §1, ADR 0007 §6, ADR 0008 §1 and the IPC specification as written.
- **Revision candidate 051b4c8aacdeecfabb23625f93dd16615aec39a0** (range 24a35c1..051b4c8: the implementation 4f7080b and the R03 seeded defects 051b4c8) answers R01–R03 and M2-004-R2-01: [M2-005 resolutions](../reviews/M2-005-finding-resolutions.md), [M2-005 handoff](../handoffs/M2-005-rereview-r2.md), [M2-004 resolutions r2](../reviews/M2-004-finding-resolutions-r2.md), [M2-004 handoff r3](../handoffs/M2-004-rereview-r3.md).
  - Directory entries durable in their parents on every attempt (R01); ended sessions retained in the index, refused at admission and replay (`duplicate_identifier`, ADR 0007 §6), denied at evaluation before any attestation, with the same rule and a new published scenario in the oracle (R02); a preallocated per-user 1 MiB reserve of written zeros released only by restrictive appends that meet a full filesystem, refilled before every admission, shown on an actually full 2 MiB tmpfs by `cham3_reserve_probe` as a new CI step (R03); pending proposal ids counted against the retained-id bounds (M2-004-R2-01).
  - Every reviewer test reproduced failing at the old candidates (M2-005: 5/4 and the tmpfs test, debug and release; M2-004 R2: 10/2) and passing at the candidate.
  - Qualification PASS: unit 166+166, ECR 19/96/18 with 23 scenarios / 352 steps, harness 137 (36 registry/policy and 21 identity/vault seeded defects); static gates PASS; the full-filesystem check PASS (VM and CI); CI m2 38422383429, m1 38422383455, m0 38422383430 PASS.
  - `Cargo.lock` is unchanged; the vectors change only by the added scenario.
- M2-005 returns to **IN_REVIEW** (revision 2); M2-004 is **IN_REVIEW** (revision 3). M2-006 follows under the owner's standing instruction after its scope record; the ±300 s commit-timestamp error code remains an open owner question.

### Re-review of M2-005 revision 2 and M2-004 revision 3; M2-004 DONE; M2-005 revision 3 (2026-10-13)

- **Independent re-review** on `review/m2-005-r2-security` at d300b609ccde3eaf1fe8e909f99e78707c4b6c5a (`docs/reviews/M2-005-codex-security-rereview-r2.md`, SHA-256 `28b46231…7435`, verified; parent 9cf8825; adds only that file; not merged), run on the owner's home computer with the Codex Security plugin available for the first time (`codex-security:security-diff-scan` 0.1.32 on 24a35c1..051b4c8 and bb3f1ba..ca40c62: zero exploit-oriented findings).
  - **M2-004 revision 3: APPROVE** for the recorded staged scope. The reviewer reconciles the two revision-2 records: both reviewed ca40c62; the later APPROVE did not dispose of M2-004-R2-01, so CHANGES_REQUESTED stood for revision 2 and is closed at 051b4c8. **M2-004 is DONE** ([completion handoff](../handoffs/M2-004-completion.md)); the owner question which revision-2 verdict governs is answered by that reconciliation.
  - **M2-005 revision 2: CHANGES_REQUESTED.** R02 PASS (`duplicate_identifier` accepted); R01 and R03 improved but open in failure and restart paths: **R2-01** (Medium: a failed creation-directory sync was forgotten across a restart — root, users, the journal's and a recreated reserve's directory entries), **R2-02** (Low: a reserve of full length bypassed the retry of a failed reserve sync at creation and replenishment), **R2-03** (Medium: the probe removed an existing root even when it refused to run); the revision-2 scope list omitted two paths. The reviewer's qualification (with a container-only `nofile` limit for Valgrind), protection positive path, full-filesystem run, static gates, artifact digest and CI comparison PASS.
- **Revision 3 candidate be2415291bf5bcf9dad8df323bf971bd193d5120** (one commit on 959524c) answers R2-01–R2-03: [resolutions r3](../reviews/M2-005-finding-resolutions-r3.md), [handoff r3](../handoffs/M2-005-rereview-r3.md).
  - The existence of a directory entry is never taken for a completed sync: `root` in its parent and `users` in `root` are synced at every start, each user directory at every reopen (R2-01). The reserve's durable size is tracked apart from its length: creation allocates (syncs) on every attempt, replenishment counts the reserve full only after a successful allocation including its sync, a full-length reserve found at start is synced before it counts (R2-02). The probe owns only what it creates: mode and filesystem decided first, root and filler created exclusively, an existing path refused untouched, only a created root removed (R2-03). No owner decision was needed.
  - The reviewer's ten new tests reproduced 4/6 at 051b4c8 and pass 10/10 at the candidate (debug and release); the probe matrix went from `sentinel=deleted` to `sentinel=preserved`; the original nine and the tmpfs test still pass. Three new Rust tests, five new registry/policy seeded defects (41) and two seeded probe defects; seven probe harness tests.
  - Qualification PASS: unit 169+169, ECR 19/96/18 with 23 scenarios / 352 steps, differential 5 955 / 0, harness 142; static gates PASS (82 static tests); the full-filesystem check PASS (VM and CI); CI m2 38435225455, m1 38435225444, m0 38435225490 PASS (artifact `sha256:4168e919…eef6`). `Cargo.lock`, the vectors and the oracle are unchanged; the change is exactly seven files.
- M2-005 is **IN_REVIEW** (revision 3). M2-006 follows under the owner's standing instruction after its scope record; the ±300 s commit-timestamp error code remains an open owner question.

### Owner decision on the commit-timestamp error code; M2-006 scope record (2026-10-13)

- **Owner decision (2026-10-13), answering the explicit question with the recommended option:** the ±300 s commit-timestamp check of ADR 0007 §5 returns the new layer-7 code **`timestamp_rule`** (`admission/timestamp_rule` through the IPC). The parent-minus-300 s clause uses the same code: both clauses are the one commit-timestamp rule and test temporal validity at admission. The refusal is admission-only: fail closed, an error to the caller, no journal record and no mutation of durable state; `clock_anomaly` remains strictly a trusted-time engine event under §5 rule 2 (a wall-clock sample more than 300 s behind trusted time). Recorded as **erratum E4** in ADR 0007 (§5 and the approval section), in wire format §7 (admission codes, after `stale_head` and before the state checks: parents known, state not yet compared — an author placement open to review) and in the IPC vocabulary (§4). No type, byte format or vector change; the published vectors cannot reach the code before a trusted clock exists.
- **M2-006 scope, recorded before the work (docs only; this is the scope commit the owner's standing instruction requires).** M2-006 is delivered as **four staged candidates**, each with its own qualification, CI and independent review before the next starts, on `codex/m2-cryptographic-contract-core`, fast-forward only. Every stage keeps the M2-005 invariants: safe Rust, fail closed, no weakening of a check, design-only mechanisms NOT_RUN until implemented, in-process composition until M2-007.
  - **Stage A — storage integrity (ADR 0006 §2, §3, §4, §6; ADR 0008 §1):** the global anchor and its serialization, the run marker and the recovery decision table at start (`resumed`/`ledger_replayed`/`contained`), the restriction ledger with `restrict` (`restriction_not_durable` until durable), torn-tail and crash recovery of the per-user journal (today a torn tail denies the user until restart), quarantine and containment states (`quarantined`, `contained` at admission, identity operations and evaluation), `rollback_status` (anchor mode, run-marker decision, quarantined/contained users, open ceremonies) and the reserve status that M2-005 left unreported; `set_policy_epoch` and `set_emergency_restriction` as restrictive writes through the ledger. Verification: the ADR 0006 decision table as an oracle, fault injection at every durability boundary of §6, seeded defects per rule, the full-filesystem probe extended to the ledger.
  - **Stage B — lifecycle and time (ADR 0007 §1–§5, §7; ADR 0008 §2):** the lifecycle-record writers (the expiry latch → `expired`, `session_event` → `session_ended`, `restrict` → `restriction`), the trusted-time service clock (`time_sample`, floors in journal and ledger, `clock_anomaly`, the §8 option-B start eligibility) with `tests/m2/oracle/trusted_time_model.py` as its oracle, the ±300 s commit-timestamp admission check with `timestamp_rule` (E4) in the admission differential and the published scenarios, one-shot `reserve_use`/`complete_use` (§4 option A: reservation consumes), `confirm_enforcement`, startup reconciliation (§7), `history` (ADR 0008 §2). The admission oracle `ecr_oracle.py` gains trusted now and the timestamp rule; vectors change only by added scenarios.
  - **Stage C — ceremony and deletion (ADR 0006 §7, §10; ADR 0008 §3, §4):** the recovery ceremony (`recover_user`, `begin_recovery`, `name_recovery_keys`, `recovery_status`, `close_recovery`) with `recovery_rule` and `recovery_key_required` at admission, `replace_identity` (carried from M2-004), tombstone-first `delete_user` with the vault-side deletion cleanup (carried from M2-004) and `deletion_pending`, the owner's deletion granularity (ADR 0008 §3).
  - **Stage D — proposals, restore and delegation (ADR 0005 §3; ADR 0006 §8; ADR 0008 §5, §8; wire format §6.4):** `prepare_proposal` and `check_proposal` (carried from M2-005) including the recovery proposals of ceremony step 4, `request_revocation`, coordinated backup and forward restore, restore when resources or keys are gone (ADR 0008 §8), and delegation admission (wire format §6.4, ADR 0007), which lifts the interim `delegation_rule` refusal of M2-005 and adds published delegation scenarios.
  - **Carried into M2-007:** transport, binaries, the setgid client, systemd units, packaging and the cross-service serialization of restrictions with signatures (ADR 0005 §6, ADR 0006 §3); until then every stage is an in-process composition.
  - **Open owner questions, none blocking stage A:** none. The authenticated time source and recovery from a large accepted forward clock error stay deferred (ADR 0007 §8, "defer; never lower").
  - **Dependencies and risk:** M2-006 builds on M2-005 revision 3, whose independent re-review is pending; an M2-005 finding may require rework of a stage. Each stage's handoff names the exact base and candidate commits.

### Re-review of M2-005 revision 3; revision 4 (2026-10-13)

- **Independent re-review** on `review/m2-005-r3-security` at 4865d7b7a4c885fddf71b3e0f0b14a361c3c609e (`docs/reviews/M2-005-codex-security-rereview-r3.md`, SHA-256 `6ce7449c…8c50`, verified; parent 9cf8825; adds only that file; not merged): **CHANGES_REQUESTED** with one Medium finding, **M2-005-R3-01** — `check_reserve_full_disk.sh` removed the probe's root and filler paths before the probe ran, so a pre-existing root or filler on an ordinary filesystem was deleted before the probe refused that filesystem. R2-01, R2-02, R2-03 (binary), R01, R03 and every earlier PASS item PASS; the `<uid>` entry argument accepted (cost restated as 2 + N syncs for N users); the pending reserve at start accepted; the M2-004 completion handoff confirmed; qualification, protection, full filesystem, static gates, CI comparison and the Codex Security plugin (zero introduced findings) PASS.
- **Revision 4 candidate 5860f276ab593da34771f704691ea35649324d6e** (one commit on ad18d8b; four files, no Rust) answers R3-01: the wrapper owns nothing on the disk — no pre-deletion, an existing root or filler path (file, directory or symbolic link) refuses the whole check untouched, leftover checks cover links; five harness cases (existing paths, symbolic links and their targets, a path-free disk; the baseline pre-deletion seeded in a copy is caught) run on an ordinary filesystem in the harness and on the 2 MiB tmpfs in a new CI step. The reviewer's scenario went from both paths deleted (be24152) to both preserved (5860f27). [Resolutions r4](../reviews/M2-005-finding-resolutions-r4.md), [handoff r4](../handoffs/M2-005-rereview-r4.md). Qualification PASS (unit 169+169, harness 147 with 12 reserve-probe tests; 23 scenarios / 352 steps; differential 5 955 / 0); static gates PASS; the full-filesystem check and the five wrapper cases on the 2 MiB tmpfs PASS; CI m2 38444270032, m1 38444270097, m0 38444270083 PASS, including the new wrapper preservation step (5/5 on the CI tmpfs).
- M2-005 is **IN_REVIEW** (revision 4). M2-006 stage A continues after the re-review.

### Re-review of M2-005 revision 4; M2-005 DONE (2026-10-13)

- **Independent bounded re-review** on `review/m2-005-r4-security` at 165e149d91e7ddaf167c3a37177275af70ae14e8 (`docs/reviews/M2-005-codex-security-rereview-r4.md`, SHA-256 `87a61a90e36e768b4f782e28c2543b57c125afb6f656da3c5b77540449127817`, verified; parent 9cf8825; adds only that file; not merged): **APPROVE** for M2-005 at 5860f27. R3-01 PASS (the original wrapper scenario and 25 extended preservation checks on the overlay and at the 2 MiB tmpfs mount root); the five wrapper cases on the tmpfs PASS with no leftovers; the seeded wrapper defect discriminates; every earlier item intact (no Rust changed; revision-3 evidence carried forward); qualification (same counts), static gates, CI comparison and the artifact digest PASS; the Codex Security plugin (`security-diff-scan` 0.1.32) ran with zero findings. Documented coverage limit, not a defect: the harness wrapper cases run in per-case subdirectories, so filler-only, dangling-filler and mount-root cleanup are proven by the reviewer's matrix, not by the harness alone. The approval does not extend to M2-006.
- **M2-005 is DONE** ([completion handoff](../handoffs/M2-005-completion.md)). M2-006 is **IN_PROGRESS**: stage A (storage integrity) is being implemented on top of 5860f27 and is offered for independent review as its own candidate.

### M2-006 stage A implementation (2026-10-13)

- **Candidate f4b062826ae54ba4e79c5abd08eeb9fe21d48510** (one commit on 22b6b5f, the M2-005 completion docs; code base 5860f27): `cham3_registryd` gains the global anchor (`anchor.rs`: two alternating slots, the valid higher counter wins, the in-memory map authoritative, `independent`/`degraded` by filesystem), the restriction ledger (`ledger.rs`: validity and entries files, started fresh every run, appended before any disk write of a restrictive change, failure removes the validity file), the restart decision of ADR 0006 §3 (`resumed`/`ledger_replayed`/`contained`, the `running`/`clean` run marker in the anchor, re-journaling of ledger entries, containment of every user with a durable `containment` record), the per-user recovery table of §4 (torn-tail truncation beyond the anchor; stale, unexplained, chain-mismatch, replay-defect, lost-store, anchor-invalid and tombstone quarantines; roll-forward of one durable unanchored record), the restrictive write path (`restrict`, key invalidation, containment, `set_policy_epoch`, `set_emergency_restriction`: denial → ledger → journal from the reserve → anchor → reply; `restriction_not_durable` and retries while pending; `shutdown` refuses with pending writes or a failed ledger), `rollback_status` (anchor mode, run decision, quarantined/unusable/contained users, pending restrictions, ledger failure, depleted reserves, policy state) and the rollback of a failed ordinary append (the user continues; a failed rollback denies until restart). `cham3_policyd` reads the persisted policy state and anchor mode, denies on the persisted emergency restriction and forwards the two admin operations. The full-filesystem probe starts a ledger on the full tmpfs. Formats and decision tables: [registryd storage formats v1](../specs/m2_registry_storage_v1.md); IPC rows updated. Details and the author placements open to review: [stage-A handoff](../handoffs/M2-006-stage-a.md).
- **Tests:** `storage_integrity` (12 new), `registry_operations` rewritten to the anchor-first semantics (25; two rollback tests), policyd +3 (12); 21 new seeded defects (62); inventory 188 cargo tests (+19). Qualification PASS (unit 188+188, harness 147 with 62 seeded registry/policy defects; 23 scenarios / 352 steps; differential 5 955 / 0); static gates PASS; the full-filesystem check with the ledger and the five wrapper cases on the 2 MiB tmpfs PASS; CI m2 38459079087, m1 38459079018, m0 38459079003 PASS, including the full-filesystem step with the ledger lines and the wrapper step.
- Stage A is **IN_REVIEW**. Stages B–D start after its approval, each on its own candidate.

### Stage A review (344c372): CHANGES_REQUESTED; revision 2 (2026-10-13)

- **Independent review** on `review/m2-006-a-security` at 344c37210ff15f297cf015ee047d8e147efe2fbd (`docs/reviews/M2-006-stage-a-codex-security.md`, SHA-256 `f7c3d57e9e90c11e6a6726571cef51579346c13f7bc1c174097a8344fd4bd37e`, verified; two commits on `main` 9cf8825, the branch adds only that file; not merged): **CHANGES_REQUESTED** for stage A at f4b0628 — six Medium findings (R01 `lineage_head` served an unanchored admission, which policyd evaluated eligible; R02 a restriction after an anchor failure was dropped after the in-memory denial; R03 a re-journaled entry that failed again was lost by a second same-boot restart; R04 a pending policy raise was lost to a clean shutdown; R05 a failed emergency release lifted the denial; R06 a validity file without an entries file counted as a valid empty ledger) and two Low observations (R07 the anchor's identity count was not reconciled; R08 an anchored quarantine or containment left grants current in the index), each reproduced by a reviewer test failing on the unchanged candidate; the happy paths, harness, wrapper, qualification, static gates, CI comparison and artifact digest PASS; the Codex Security plugin (`security-diff-scan` 0.1.32) promoted the same six Medium findings; fault cases left NOT_RUN (the validity-unlink failure, the malformed/foreign-anchor and ledger-entry matrix, concurrent policy publication) and one informational correction (the handoff's file count). No owner question.
- **Revision 2, candidate e34fd8a0b97f5a606656ab87914d4d1b19517d7c** (one commit on a9e47e4; ten files, no dependency, lock, ADR, oracle, vector, workflow, Dockerfile or pin): reads refuse an unanchored state and retry its anchor update, denial lasts until the record is anchored (retry or roll-forward) and nothing is appended beyond it; restrictions and key invalidations after an anchor failure are ledgered and pending; replayed entries are re-ledgered before the journal write and applied as the live operations apply them; the policy state in force is published with every slot write and the clean marker (`policy_pending`), a release is anchored before it is in force; a validity file without entries is invalid; the identity count is reconciled (`identity_mismatch`); anchored states end grants. The open fault cases: run marker state `unverifiable` on a ledger failure (with the validity file emptied when it cannot be unlinked), equal slot counters invalidate the anchor, foreign and malformed slots, the ledger-entry matrix, concurrent policy publication under the policy lock. The reviewer's tests, unchanged, go from 0/8 to 8/8 (their two adverse-behaviour tests from 2/2 to 0/2). Tests: `storage_integrity` 23 (+11, one rewritten), `current_authorization` 14 (+2), one anchor unit test; 81 seeded defects (+19); inventory 202. [Resolutions r2](../reviews/M2-006-a-finding-resolutions-r2.md), [handoff r2](../handoffs/M2-006-a-rereview-r2.md). Qualification PASS (unit 202+202, harness 147 with 81 seeded registry/policy defects; 23 scenarios / 352 steps; differential 5 955 / 0; layer-7 differential 0 disagreements); static gates PASS; the full-filesystem check with the ledger and the five wrapper cases on the 2 MiB tmpfs PASS; CI m2 38481185159, m1 38481185180, m0 38481185314 PASS (every job and step), including the vault protection positive path, the full-filesystem step with the ledger lines and the wrapper step; the artifact (11857892996, 44 files) downloaded and its digest verified.
- Stage A revision 2 was re-reviewed (below).

### Stage A re-review (7d570c1): CHANGES_REQUESTED; revision 3 (2026-10-14)

- **Independent re-review** on `review/m2-006-a-r2-security` at 7d570c1ce5b14ce3b37836775881c2013aa051a4 (`docs/reviews/M2-006-stage-a-r2-codex-security.md`, SHA-256 `d3ecee78270a58c82bdc0c1d137f1cd2a6eb8417066cfb37cc85b12f9ccea7e3`, verified; one commit on `main` 9cf8825, the branch adds only that file; not merged): **CHANGES_REQUESTED** for stage A revision 2 at e34fd8a — R01–R08 PASS (the eight original reviewer tests 8/8, the adverse controls 0/2, extended retry/drop, repeated-restart, policy and foreign-anchor matrices), four new findings each reproduced by a reviewer test: R2-01 (Medium) two equal-counter slots were rejected once but the fresh anchor restarted its counter at zero and left them on disk, so the next unclean start trusted the surviving slot and revived the quarantined grants; R2-02 (Medium) a ledger failure during the start's own replay was followed by a fresh `running` marker, so a further same-boot restart found a complete-looking empty ledger; R2-03 (Medium) the documented compound-failure residual (ledger append, both validity invalidations, anchor and journal all failing) is not an owner-approved exception to ADR 0006 §3; R2-04 (Low) the pending queue was appended in full before anchoring (two unanchored records; a view beyond the anchor served after a partial drain). Harness, static gates, full-filesystem wrapper and CI comparison PASS; the local pinned qualification BLOCKED in the reviewer's runtime (no Docker access) and replaced by a component fallback plus CI; the Codex Security plugin (0.1.32) promoted no exploit finding and retained the four acceptance findings. Owner question raised by the reviewer: what narrower guarantee, if any, the owner approves for the compound failure.
- **Owner decisions (2026-10-14):** R2-03 **Option A** — the limit recorded as ADR 0006 §3 erratum E5 (drafted by the implementer; the owner confirms the wording); and the amd64 Debian locks refreshed after GitHub Actions m2_checks 38577923955 at 9633ce3 failed only its two lock checks on Debian's libcurl `8.14.1-2+deb13u7` (ADR 0003 §2).
- **Revision 3, candidate 2217e62ddf42c42573346155727c983d937addc0** (three commits on fc147e0: 9633ce3 the code, eight files; a2d0ebd the lock refresh, four lines of the two amd64 lock files; 2217e62 erratum E5 and storage formats §7; no dependency, `Cargo.lock`, pinned version, oracle, vector, workflow or Dockerfile): the fresh anchor's counter continues above the rejected slots so this run's writes outrank them, and the quarantine they leave is anchored at the same start (R2-01); a run whose ledger failed during the start publishes `unverifiable`, or refuses to start (R2-02); the failed ledger's invalidation and the `unverifiable` marker are owed until they succeed and retried by every operation, reported as `rollback_status.unverifiable_owed`, and the remaining window (a process death before any write succeeds) is accepted by the owner as ADR 0006 §3 erratum E5 (R2-03); the pending queue is drained one record at a time, each anchored before the next append (R2-04). The reviewer's tests, unchanged: storage 3/6 → 5/6 (the case erratum E5 excludes), queue 1/4 → 3/4 (the partial-flush test's premise assertion), matrix 4/4 → 4/4. Tests: `storage_integrity` 27 (+4); 87 seeded defects (+6); inventory 206. [Resolutions r3](../reviews/M2-006-a-finding-resolutions-r3.md), [handoff r3](../handoffs/M2-006-a-rereview-r3.md). Qualified at 2217e62 in a clean clone as UID 1001 in the no-cache rebuilt pinned container (image 2a0bcf342c57): `m2 qualification: PASS` (both lock checks PASS), unit 206 + 206, harness 147 (87 registry/policy seeded defects), static checks PASS, 82 static tests OK, full-filesystem probe and the five wrapper cases PASS ([VM evidence](../evidence/M2-cryptographic-contract-core/m2_006a_r3_amd64_vm_2217e62/)); GitHub Actions m2_checks 38584362113, m1_checks 38584362062, m0_checks 38584362077 success at 2217e62 ([CI evidence](../evidence/M2-cryptographic-contract-core/m2_006a_r3_amd64_ci_2217e62/)).
- Stage A revision 3 was re-reviewed (below).

### Stage A re-review (f0d6eb0): CHANGES_REQUESTED; revision 4 (2026-10-14)

- **Independent re-review** on `review/m2-006-a-r3-security` at f0d6eb01aeec7858fe67a034f42db09ca04f0177 (`docs/reviews/M2-006-stage-a-r3-codex-security.md`, SHA-256 `a04041838560744814bd2c9f078155497032a8009b693cefc2b5c95ad1e60c92`, verified; one commit on `main` 9cf8825, the branch adds only that file; not merged): **CHANGES_REQUESTED** for revision 3 at 2217e62 — R2-01, R2-02, R2-04, the lock refresh (reproduced by the reviewer's own no-cache build), harness, qualification, CI and the Codex Security plugin (0.1.32, no promoted exploit finding) PASS; the E5 exclusion holds over all 32 failure combinations; R3-01 (Medium) `policy_state`, `append_identity` (idempotent and new-user returns), `journal_users` and the emergency release skipped the owed retries after storage recovered, so a same-boot restart resumed without the restriction; R3-02 (Low) E5 said every successful retry contains, which a journal-only retry does not. Owner question: confirm the corrected E5 wording.
- **Revision 4, candidate db5bb59fcf461034461d9e0d2f21f5816986134c** (one commit on 908c391; eight files, no dependency, lock, pin, oracle, vector, workflow or Dockerfile): every IPC operation retries the owed invalidation and marker right after its role check and before any early return, a refused caller writes nothing; from the first ledger failure the anchor holds the run unverifiable, so every later anchor image carries the marker whichever write it belongs to, and a published marker is not rewritten; the start's explicit marker branch is that same flag (R3-01); E5 and storage formats §§4, 7 distinguish containment after a successful invalidation or marker from precise roll-forward and replay after a journal-only retry, and state that policy restrictions always contain (R3-02, no mechanism change). The reviewer's R3 tests 7/12 → 12/12; its R2 tests unchanged (5/6, 3/4, 4/4). Tests: `storage_integrity` 30 (+3); 97 seeded defects (+10, one re-pointed); inventory 209. [Resolutions r4](../reviews/M2-006-a-finding-resolutions-r4.md), [handoff r4](../handoffs/M2-006-a-rereview-r4.md). Qualified at db5bb59 in a clean clone as UID 1001 in the pinned container (image 2a0bcf342c57): `m2 qualification: PASS`, unit 209 + 209, harness 147 (97 registry/policy seeded defects), static checks PASS, 82 static tests OK, full-filesystem probe and the five wrapper cases PASS ([VM evidence](../evidence/M2-cryptographic-contract-core/m2_006a_r4_amd64_vm_db5bb59/)); GitHub Actions m2_checks 38592632636, m1_checks 38592632234, m0_checks 38592632553 success at db5bb59 ([CI evidence](../evidence/M2-cryptographic-contract-core/m2_006a_r4_amd64_ci_db5bb59/)).
- Stage A revision 4 was re-reviewed (below).

### Stage A re-review (06a9423): APPROVE; stage A DONE (2026-10-14)

- **Independent bounded re-review** on `review/m2-006-a-r4-security` at 06a9423258ecd84fa823c4817d3fba615d49ff6d (`docs/reviews/M2-006-stage-a-r4-codex-security.md`, SHA-256 `327149fa5ffec50df535cd17c1cec4ededcb8d997150d7a12618d493685a4121`, verified; one commit on `main` 9cf8825, the branch adds only that file; not merged): **APPROVE** for stage A revision 4 at db5bb59, no new findings. R3-01 PASS (all twelve IPC entry points audited, the sticky anchor, refused callers write nothing, lock order intact); R3-02 PASS (the corrected E5 wording matches the behaviour, read in its failed-ledger context: a policy restriction whose ledger append succeeded can replay precisely). The reviewer's tests at the candidate: R3 12/12; R2 storage 5/6 and queue 3/4 (the E5-excluded case and the obsolete queue premise, as expected); matrix 4/4; three new R4 tests 3/3 (nine refused-caller operations and the accessors, 32 policy partial-recovery cases, journal-only persistence then marker recovery); debug and release. All eleven changed seeded defects fail at their intended assertions. Qualification in its own no-cache image (209 + 209; harness 147, 97 registry/policy defects; both locks PASS), static checks, 82 static tests, the full-filesystem wrapper (5/5), the CI comparison and artifact digest PASS; Codex Security plugin (`security-diff-scan` 0.1.32) zero findings. Physical crashes and deployed enforcement remain NOT_RUN. Owner question: confirm the corrected E5 wording.
- **Stage A is DONE** ([completion handoff](../handoffs/M2-006-a-completion.md)). The owner's confirmation of the corrected E5 wording is open and does not block the stage. **Stage B** (lifecycle and time) starts on top of the completion commit as its own candidate.

### Owner decisions after stage A: E5 wording confirmed; stage B split (2026-10-14)

- **E5 confirmed:** the owner confirmed the corrected wording of ADR 0006 §3 erratum E5 as written (the re-review at 06a9423 found it technically accurate). ADR 0006, its index and storage formats §7 record the confirmation; no mechanism, record or format changes.
- **Stage B split, recorded before the work (owner, answering the explicit question with the recommended option):** stage B is delivered as **two candidates**, each with its own qualification, CI and independent review before the next starts, on `codex/m2-cryptographic-contract-core`, fast-forward only. Nothing is added to or removed from stage B's scope.
  - **Stage B1 — trusted time (ADR 0007 §5, §8 option B; erratum E4):** registryd's trusted-time clock under the maximum-offset rule (rebase on every accepted forward sample; never lower within a boot; same-boot resume from the maximum over journal floors and ledger entries; new-boot start from the last persisted trusted value), the `time_floor` journal record and the ledger time entry `{boot_id, boottime, trusted, observed}`, the `clock_anomaly` record, `time_sample` (`service:policyd`) and the offset in `lineage_head`, policyd's offset clock (`max(own, snapshot)`; reported through `time_sample` before relying on it), the option-B observed-time counter with the trusted and observed time at admission in `commit_admitted` and the option-B start rule in evaluation, the ±300 s commit-timestamp check with `timestamp_rule` at admission (both clauses), registry record time from trusted time, and the new-boot end of every live temporary grant (`branch_terminated`). Oracles: `tests/m2/oracle/trusted_time_model.py` (the service clock against the model) and `ecr_oracle.py` gaining trusted now and the timestamp rule; vectors change only by added scenarios.
  - **Stage B2 — lifecycle and consumption (ADR 0007 §§2–4, 6, 7; ADR 0008 §2):** the lifecycle-record writers (the expiry latch → `expired`, `session_event` → `session_ended`, the remaining narrowing records of ADR 0007 §3 that stage B owns), one-shot `reserve_use`/`complete_use` (§4 option A: reservation consumes; `consumed_uncertain` at restart), `confirm_enforcement`, startup reconciliation (§7), audit gaps and `history`.
  - **Dependencies and risk:** B2 builds on B1's clock; a B1 finding may require rework of B2. Each stage's handoff names the exact base and candidate commits.

### Final M2 review requirement (owner instruction, 2026-10-12)

- When M2 implementation is complete (M2-003..M2-008, before M2 is declared DONE), the owner will run the final independent security review with the **Codex Security plugin skill**.
- The implementer preparing the final M2 handoff must:
  1. remind the owner of this;
  2. write the final review prompt so that the reviewer uses that skill in addition to the standard review steps, and records the skill's exact name/version, the commands it ran and every finding in its review record.
- Findings from the skill are handled like any other independent-review finding (PASS/FAIL/NOT_RUN/BLOCKED, no self-approval).
