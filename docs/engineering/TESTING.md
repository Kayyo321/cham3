# Testing Strategy

Testing verifies the observable requirements in `PROJECT.md`; a configuration file or successful build alone is not evidence of enforcement. This strategy does not assert that the repository currently has a test harness.

## Result labels

- **PASS:** stated requirement demonstrated in the stated environment with artifacts.
- **FAIL:** observed behavior violates acceptance criteria.
- **NOT_RUN:** not executed; give reason.
- **BLOCKED:** execution prevented by a dependency or unsafe/unavailable environment; identify it.

Always record commit, environment, command, result, artifacts, and limits. Distinguish unit, mocked, namespace/container, VM, and physical-hardware evidence.

## Layers

1. **Static checks:** formatting, lint, type checks, dependency/license checks, secret scanning and configuration validation as each is introduced.
2. **Unit tests:** deterministic CBOR/COSE handling, signature and key-authority rules, contract parsing, policy decisions, expiration, revocation, revision and delegation.
3. **Component integration:** vault authorization, registry consistency, authenticated service IPC, launcher attribution, brokers, namespaces, AppArmor/seccomp and lifecycle behavior.
4. **Negative security tests:** malformed/tampered contracts; unknown keys; wrong owner/app/resource; expired/revoked/stale grants; unauthorized IPC/signing; helper process attempts; policy conflicts.
5. **Network tests:** Direct policy; Tor-only egress with packet observation; gateway unavailable/restart; no direct fallback; DNS, IPv4, IPv6, UDP and unsupported protocols; Offline external and proxy/host-service attempts; firewall reload and mode-transition socket teardown. Include a positive network control.
6. **Desktop tests:** KDE/Wayland Super+Click attribution, spoofed titles/windows, inspector read-only behavior, authenticated approval, cancel behavior, and display of confirmed backend state.
7. **Distribution tests:** Debian package build/install/upgrade, image build, boot in a VM, application launch, update signature and recovery. A VM result is not a hardware compatibility result.
8. **Compatibility tests:** representative Debian GTK/Qt/KDE packages, Flatpak, standalone executables, helper processes, browsers, communications apps and portals; document unsupported behavior instead of relaxing mandatory restrictions.
9. **Fuzzing/reliability:** parsers, IPC boundaries, protocol handling, crash/restart, interrupted transitions, power/session changes. Fuzz only where the interface and harness exist.
10. **Release verification:** clean source revision, reproducible-build evidence where supported, artifact signatures, security findings, supported hardware, upgrade/recovery, and release gate.

## Test quality

Pair security negative controls with positive controls proving the target path works absent the restriction. Prefer external observation for routing claims. Test failure and recovery as well as steady state. Validate test sensitivity by introducing a controlled defect in a disposable copy and confirming the test fails. Never call a simulated result full-system proof. Review false positives, environment dependencies, and blind spots independently.

Security-sensitive changes require independent review of the test design and results. Required tests that cannot run remain NOT_RUN or BLOCKED and prevent gates that require them from passing.

## Proposed ECR test specifications

These specifications support [ADR 0001](../decisions/0001_eternal_contract_repository.md), which is **PROPOSED**. They are future implementation acceptance work for M2, M5, and M6, not existing tests or passing evidence. Lifecycle/closure, schema, and recovery cases must use owner-approved semantics before implementation. Each negative case needs a valid approved transaction or authorized trusted caller as its positive control.

| Specification | Required positive and negative cases | Intended environment / evidence |
|---|---|---|
| Tamper evidence and canonical encoding | Accept published deterministic CBOR/COSE/Ed25519 vectors; independently alter each metadata field, contract reference/state, parent, payload, protected header, and signature and reject it. Reject disallowed encoding, duplicate fields, unknown keys, wrong authority class, and cross-user/app substitution. Verify the exact reviewed hash preimage. | M2 unit vectors and verifier/vault integration; serialized bytes, hashes, and rejection results. |
| Commit-chain and DAG verification | Admit a valid genesis, persistent successor, temporary branch, and explicitly authorized merge. Reject missing/substituted parents, illegal cross-lineage edges, cycles, inconsistent deltas/result states, stale expected heads, and implicit permission unions. Detect truncated/substituted historical stores using the approved current-state anchors; hash verification alone is insufficient. | M2 registry integration with corruption/concurrency cases; object/index state and admission results. |
| Commit-before-authorization and enforcement separation | A freshly approved durable commit may proceed to enforcement; absent, unsigned, incomplete, duplicate, or failed-write transactions cannot grant authority. Inject transition failure and verify the UI never marks committed/pending/failed state as enforced. Confirmation must identify the correct commit and runtime from launch/network services and affected brokers. | M2/M5/M6 component and disposable Debian 13/KDE Wayland VM tests; transaction traces, backend status, UI capture, and observed resource/network behavior. |
| Forward restore and anti-replay | Restore an eligible historical permission state through a new current-parented revision, fresh approval, and current policy epoch. Reject old-byte replay, head reset, superseded/revoked grants, invalid keys, expired sessions, consumed one-shot grants, forbidden current-policy permissions, and wrong application identity. Verify canceled/failed restore and a policy/head change during approval cannot increase authority. | M2 verifier/registry plus M6 disposable VM transition tests; old/new hashes, policy state, confirmation, and actual access probes. |
| Unauthorized process access denial | Authorized inspector/service can read only its allowed history and authorized auth workflow can request a commit. Sandboxed applications and same-user child/helper processes cannot read/write/delete ECR objects/indexes, enumerate history through APIs, use admin IPC, or invoke arbitrary vault signing. Reject forged `actor_uid`, `initiating_program`, request IDs, synthetic approval input, and broker confused-deputy attempts; test cross-user boundaries. | M3/M5 and later multi-user disposable Debian 13/KDE Wayland VM; authenticated caller controls, filesystem/IPC denial evidence, and signing/commit absence. Mocks alone cannot prove isolation. |
| Temporary branches and mandatory denial | Approved temporary grants work within scope then cease on session exit, expiry, consumption, key invalidation, and persistent-state supersession. Preserve historical objects without retaining authority. Under locked vault, missing user presence, full/unavailable ECR storage, and service restart, mandatory denial still occurs; closure/audit behavior follows the approved lifecycle decision. | M2 lifecycle integration and M6/M7 disposable VM with resource-specific positive controls; access probes and retained history/current-state evidence. |
| Atomicity, crash recovery, and replay-resistant indexes | Interrupt object writes, index updates, commit admission, enforcement, and confirmation; recover to a justified current state without unsigned authority, duplicate activation, or repeated one-shot use. Exercise concurrent proposals, backup/restore of stale indexes and repository tails, corrupt anchors, and unavailable storage; quarantine/deny unresolved state. | M2/M6 fault injection and disposable VM restart tests; durable object/index/anchor snapshots and observed enforcement. |
| Trusted descriptions and history display | Show verified identity, exact delta, lifetime, parent/merge/termination markers, and separate signing/authorization/enforcement status. Validate generated messages against structured state, safely render hostile rationale, inspect without signing, and obtain fresh approval for any changed payload. Restore preview exposes current-policy exclusions and cannot silently reactivate a branch. | M5 unit/component tests and KDE Wayland desktop tests; UI captures and exact approved versus signed payloads. |

Record PASS, FAIL, NOT_RUN, or BLOCKED with exact commits, commands, environment, controls, and artifacts. Require independent security review of schema, boundary tests, and recovery/anti-replay evidence before declaring an ECR implementation complete.
