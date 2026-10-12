# ADR 0001: Eternal Contract Repository and audit commit pipeline

- **Status:** PROPOSED
- **Date / decision owner:** Proposed 2026-10-11; adoption candidate revised 2026-10-12 / cham3 project owner (requesting user); scoped approval pending.
- **Related task/spec sections:** Owner-requested ECR architecture proposal; `PROJECT.md` §§4, 5, 8, 11.3–11.4, 14, 18; roadmap M2, M5, M6; SEC-01–06, SEC-11–16.

`PROJECT.md` is the canonical product specification. This ADR records a scoped decision or proposal and does not silently replace that document.

## Problem and context

Contracts are immutable signed authorization statements, but the specification does not yet select a history-storage format or an audit transaction schema. The Eternal Contract Repository (ECR) would make contract lineage and authorization-change history one verifiable, append-only record owned by `cham3-registryd`. KDE would expose that history through the trusted Contract Inspector and audit dashboard.

This adoption candidate makes ECR a foundational M2 deliverable, with trusted desktop integration in M5 and runtime transition/restore verification in M6. Architecture adoption and implementation readiness are separate gates; this document reports no implementation and authorizes no privileged-service work before the detailed design gates below pass. The Git analogy describes lineage and forward revisions; it does not require Git, executable hooks, blockchain, or application access to a repository.

## Decision

The following architecture is **proposed for scoped owner approval**. Approval adopts the contract DAG, canonical signed change transactions, protected registry ownership, trusted approval/history workflow, forward restore, and the mandatory-denial rule below. It does not select a database, freeze a wire format, approve a background signing authority, or pass implementation gates. Detailed choices listed below need reviewed, owner-approved design decisions before the affected work becomes READY.

The accompanying `PROJECT.md` amendments add §5.7 and the Phase 2 ECR deliverables. They are part of this approval candidate; until owner approval, those amendments remain proposed and cannot authorize implementation.

### Contract DAG and lifetimes

- Record contract transactions in an append-only directed acyclic graph, partitioned by owning user and verified application/installation identity. Each immutable node links to its immediate predecessors by SHA-256 commit hashes; their recursive links establish ancestor lineage.
- Persistent contracts form the main trunk, which may be linear or contain explicitly authorized merges. A merge is a new signed transaction with multiple parents and an explicit resulting state; it must never implicitly union permissions or override revocations.
- Session, timed, and one-shot grants form temporary branches rooted at the persistent state active when approved. Their signed scope, lifetime, session binding, usage limits, and temporary authority remain mandatory. A branch cannot promote temporary authority into persistent authority.
- Temporary authority terminates on session exit, expiration, consumption, or other invalidation. Ephemeral describes its authority and live index entries, not deletion of historical signed nodes. Later persistent revisions must cause affected branches to be reevaluated; a historical branch root cannot preserve superseded authority.
- Historical nodes remain immutable and inspectable by authorized trusted components. No history edit, branch reset, or head-pointer rollback may reactivate an old grant.

### Canonical commit and cryptographic audit entry

User-authorized contract additions, modifications, explicit revocations, merges, and historical restores require a new durable signed ECR transaction. No new or expanded authority may be activated before admission. Every such transaction is the canonical cryptographic audit entry for that authorized change. Operational diagnostics and enforcement outcomes supplement it and cannot replace it.

Mandatory expiration, session termination, one-shot consumption, key/policy invalidation, emergency restriction, and fail-closed denial execute immediately under existing validity limits or mandatory policy. Restriction may precede transaction admission when signing, authentication, storage, or enforcement confirmation is unavailable. A failed explicit-revocation commit must never justify continuing access: deny/quarantine immediately, report the unsigned denial separately, and complete the signed history transaction only through a fresh valid authorization workflow. Neither restriction nor its lifecycle evidence can grant, extend, or restore authority. The lifecycle evidence requirements below preserve that distinction.

Every commit must contain or cryptographically bind the following structured metadata:

| Field | Required meaning and validation |
|---|---|
| `timestamp` | ISO-8601 UTC creation time, supplied by a trusted component. Time alone is not evidence of ordering, freshness, or current validity. |
| `actor_uid` | Human user authorizing the change, verified through the authenticated workflow rather than an application claim. |
| `key_id` | Authenticated user signing-key identifier, bound to the public key, owner, and permitted authority class. |
| `app_id` / `package_identity` | Stable verified application identifier plus verified package/installation identity and executable SHA-256 hash. Both identity aspects must be bound to the transaction. |
| `initiating_program` | Specific trusted initiating context, such as `cham3_authui`, `cham3_inspector`, or a trusted portal broker. Verify caller credentials and role; a caller-supplied name is insufficient. |
| `parent_commit_hashes` | Array of immediate predecessor hashes in the same authorized lineage; empty only for a validated genesis node. Parent order must be defined by the approved encoding profile. |
| `commit_message` | Human-readable description and rationale for the grant, change, revocation, or historical restore request. |
| `delta` | Exact capability changes relative to a designated current state: additions/removals, resource and operation scope, lifetime changes, and Direct/Tor/Offline transitions. A merge must identify the state against which its delta is evaluated. |
| `signature` | Ed25519 signature produced by `cham3-vaultd` under the authenticated user's appropriate signing authority, carried in a COSE_Sign1 envelope. |

Use a fixed versioned deterministic CBOR profile. The payload must also bind the contract schema version, policy epoch, authorization-request/session references, authority class, grant identifiers, and complete resulting contract state (or immutable content hashes of that state). A delta or prose description alone is insufficient to reconstruct and verify authorization.

The signature is the COSE envelope signature, not a field recursively included in its own signed payload. Sign the standard COSE signing structure containing the deterministic commit payload and protected headers, with OS contract domain separation. This follows [RFC 9052 §§4.2–4.4](https://www.rfc-editor.org/rfc/rfc9052.html#section-4.2); deterministic serialization must be profiled against [RFC 8949 §4.2](https://www.rfc-editor.org/rfc/rfc8949.html#section-4.2). Required external spellings such as `COSE_Sign1` remain at the standards boundary.

The proposed commit identifier is SHA-256 of the complete stored deterministic COSE envelope. Before implementation, freeze its exact encoding, protected-header/domain-separation rules, contract-to-commit relationship, and hash preimage in a reviewed schema decision and test vectors. Use maintained cryptographic libraries, never a custom primitive.

### Registry storage and indexing outline

`cham3-registryd` owns a protected, durable store of immutable commit objects and referenced contract objects, addressed by hash. Storage is per user/application lineage with enforced cross-user boundaries; a specific database or on-disk layout remains unselected.

Maintain transactional indexes for current persistent heads, live temporary branch heads, parents/children, grant revisions and revocations, policy epochs, signing keys, application/package identity, session/expiry/usage state, and authorization-request identifiers. Indexes are trusted registry state, never application-writable references. Hashes and ancestry alone do not identify which head is currently authorized.

Admission must verify canonical encoding, signature and key authority, ownership, verified subject, mandatory policy, parent existence, acyclicity, allowed lineage, and precise state/delta consistency. Serialize conflicting changes against the expected current head and policy epoch; reject or reauthorize stale proposals. Bind approval to the exact bytes shown, not just a request identifier. Reject duplicate transaction activation and one-shot reuse.

For signed contract transactions, durably commit all required objects and the pending transaction/index update atomically before attempting activation; mandatory denial is never blocked on this write. Track pending, failed, and confirmed enforcement outcomes separately, bound to the commit and runtime identity; never mutate signed history to record a later outcome. Promote active state only after the appropriate enforcement services confirm success. Recovery must reconcile pending transactions without granting authority from an incomplete write or blindly retrying a consumed grant.

Derived history-search indexes may be rebuilt from validated objects. The authoritative active-head, epoch, revocation, and one-shot state must additionally have a rollback-resistant recovery mechanism: rebuilding arbitrary old signed history is not sufficient. Its durable anchors, backup/restore semantics, and crash behavior require a separate approved design. Corruption, unavailable storage, or unknown current state must fail closed. No adoption claim of immutable or complete history extends beyond the threat model, especially privileged-service/kernel compromise or deletion of an unanchored repository tail.

### Trusted access and commit-description pipeline

Sandboxed applications must have **no direct read or write access** to ECR files, history/search indexes, signing interfaces, or administrative IPC. Enforce this through filesystem isolation, OS credentials/security contexts, and narrowly authenticated service APIs, including child/helper processes. Same Unix UID is not sufficient authorization (SEC-01, SEC-02, SEC-05, SEC-15, SEC-16).

Applications may submit permission requests only through system brokers. The broker treats requests and proposed rationale as untrusted input; it resolves the actual application identity independently. A broker or inspector may initiate a proposal but cannot authorize or sign it.

For user-requested ECR modifications, only the trusted `cham3-authui` workflow may obtain explicit authenticated human presence: password, PIN, or an approved biometric/FIDO2 touch mechanism. Hardware is optional, not a foundational requirement. A previously unlocked vault, application-synthesized input, or ordinary UI setting is not approval of this transaction.

1. Trusted contract/policy services compute the exact validated before/after state and capability delta.
2. `cham3-authui` generates a plain-language description from that structured delta, verified app identity, lifetime, and old/new network mode. Optional user rationale is distinct from application-supplied text. Escape untrusted text, bound its length, and avoid secrets or unnecessary sensitive resource details in permanent messages.
3. Display the description and all security-relevant structured contents. Examples include “Grant microphone access to the verified application for this session” and “Change network mode from Offline to Direct.” The message cannot substitute for the delta.
4. Authenticate the user and bind approval to the exact canonical proposal, expected current head, policy epoch, and request/session scope. Any security-relevant change requires renewed approval.
5. `cham3-vaultd` validates the authorized signing workflow; `cham3-contractd` verifies the revision; `cham3-registryd` durably admits the transaction. Technical enforcement follows through the existing service boundaries and safe-transition rules.

Canonical service names retain the external hyphenated spelling defined in `PROJECT.md` §14. New internal fields, functions, variables, and files use `snake_case`; constants/macros use `PascalCase`; types use `snake_case_t`; no `camelCase`.

### KDE history presentation and forward restore

The trusted inspector/audit dashboard presents a time-ordered graph with persistent trunk, temporary branches, parent/merge links, and terminated branch markers. Each node shows the verified application/user, time, initiating context, message, exact delta, lifetime, signing/verification status, and policy epoch. Show separate committed, pending, failed, superseded, invalidated, and confirmed-enforcement states; status must be bound to the correct commit and runtime, never inferred from a graph highlight or signature.

Historical inspection is read-only and does not unlock signing keys. A selected node exposes its snapshot and a comparison with the current state. A “Restore permissions from this revision” action is a proposal, routed through trusted authorization; expired/session-specific grants and obsolete application identities cannot be silently resurrected.

Restore uses a forward-signed revision (`git revert` model), parented to the current authorized head rather than resetting it to the selected historical node. Bind the selected historical hash as a restore reference, recompute the delta and target state under the current policy epoch, ownership, identity, key authority, and mandatory restrictions, then obtain fresh explicit approval. Show any historical permissions that are now forbidden or unavailable; reject or require approval of a clearly displayed permitted target rather than silently granting them. Temporary grants require fresh valid scope and lifetime. Old signatures, keys, sessions, and one-shot grants are not reused. Follow quarantine/restart and resource-specific revocation requirements before reporting success (SEC-04, SEC-06, SEC-13, SEC-14).

### Required invariant, lifecycle decision, and implementation gates

**A durable signed ECR commit is necessary before new or expanded contract authority can be activated, but is not technical proof of active enforcement until confirmed by `cham3-launcherd` / `cham3-netd` for their enforcement domains (SEC-03, SEC-11). Mandatory denial never depends on a new commit.** Relevant resource brokers must also confirm their own domains. Policy decisions, authenticated user approval, durable commit admission, and technical enforcement remain distinct.

**Lifecycle decision included in scoped approval:** `PROJECT.md` §§5.3, 5.5, 11.4 and ADR 0002's emergency deny boundary take precedence over history admission. Trusted services enforce already authorized validity limits and mandatory restrictions immediately, including with the vault locked, user presence absent, ECR unavailable, or current state uncertain. They record closure/lifecycle evidence through a narrowly authenticated service path when possible; that evidence is not a human-authorized contract commit and conveys no signing or grant authority. When durable evidence cannot be written, report an audit gap and keep affected access denied/unknown; recovery cannot infer continuing validity, undo consumption, or claim complete history. Re-enabling access requires justified current authorization and confirmed enforcement, with fresh approval for a new or expanded grant. This decision grants no unrestricted background vault access and changes no cryptographic primitive.

The following are implementation gates, not secretly selected details. The coordinator must create separate reviewed, owner-approved design records and task acceptance criteria for each before dependent implementation:

- **Lifecycle evidence and denial recovery:** Define event encoding, authenticated service provenance, durable ordering/consumption state, audit-gap reporting, and replay-resistant reconciliation. A lifecycle event cannot be represented as a user-signed transaction unless the exact signing operation has independent user approval under `PROJECT.md` §14. No service signing key is selected by this ADR; any such proposal needs its own owner decision.
- **Wire schema and storage:** Approve exact CBOR/COSE/hash profile, schema evolution, linked contract representation, merge semantics, backend, atomicity, concurrent writers, and durable rollback-resistant state anchors.
- **Retention and privacy:** Define capacity, access by trusted viewers, backup/export/recovery, account/key deletion, and treatment of permanently retained temporary history and sensitive metadata. “Eternal” is an append-only design goal, not a guarantee of infinite retention or resistance to every privileged attacker.
- **Authentication and identity:** Define approved authentication mechanisms, identity/hash handling across package updates, and safe restore when historical resources or keys no longer exist.

### Staged delivery and acceptance evidence

- **M2 / PROJECT Phase 2 — key pillar:** Adopt ECR as the contract registry and canonical signed change-history foundation. First pass the lifecycle-evidence, wire schema/storage, retention/privacy, authentication/identity, and rollback-resistant recovery design gates; then implement DAG storage, transaction admission, protected current-state indexes, registry/vault interfaces, and cryptographic/negative/recovery tests. M2 cannot exit with only signatures or a mutable basic registry.
- **M5 / PROJECT Phase 5:** Add trusted message generation, fresh approval, read-only history tree, snapshot comparison, and accurate enforcement status. Verify trusted attribution and spoof resistance.
- **M6 / PROJECT Phase 6:** Verify forward restore, revocation, lifecycle termination, quarantine/restart, failed writes/transitions, and recovery under current policy and anti-replay rules. Later multi-user hardening must verify isolated histories and key lifecycle.

Required implementation acceptance specifications are listed in `../engineering/TESTING.md`. Implementation acceptance needs unit vectors, positive/negative component tests, and actual Debian 13/KDE Wayland enforcement tests in disposable environments, with independent security review. A draft ADR or passing documentation check does not satisfy implementation gates.

## Alternatives considered

- **Separate mutable registry and ordinary text audit log:** Simpler storage, but permits authorization history and audit records to diverge; not selected for this goal.
- **Literal Git repository:** Useful analogy and exploration tool, but Git access controls and branch pointers alone do not establish cham3 authorization, signing, isolation, or enforcement. Backend selection remains open.
- **Linear signed journal only:** Not selected as the logical architecture because it omits the adopted DAG, temporary-branch, and explicit-merge semantics. A physical journal may be evaluated as a storage backend only if it preserves those semantics and passes the same admission/recovery gates. Replacing the logical DAG requires a new scoped owner-approved amendment.
- **Replay a historical snapshot or reset its head:** Rejected because superseded authority could be restored without current policy and fresh approval (SEC-04).

## Tradeoffs and consequences

- Unified signed change history improves inspection and attribution; append-only DAG/index consistency, crash recovery, and anti-replay anchors add complexity and write latency.
- Temporary authority remains bounded while its audit objects persist, increasing storage and privacy obligations.
- Descriptions help people understand changes but structured state controls authorization. Untrusted text cannot influence the signing boundary.
- Existing applications continue to request permissions through brokers; they gain no repository capability. No migration support or implemented compatibility is claimed.
- Adoption includes the explicit canonical-spec amendments in `PROJECT.md` §5.7 and Phase 2. They preserve mandatory denial and signing boundaries; detailed implementation decisions remain gated rather than being implied by adoption.

## Security implications

- Preserve default-deny, fail-closed behavior, immutable revisions, no application signing authority, mandatory-policy precedence, user separation, and confirmed enforcement status.
- Tamper-evident objects are not proof of current active state, audit completeness, authenticated human intent, or protection against a compromised privileged service. Trusted admission, state anchors, and enforcement still matter.
- Signing keys remain in `cham3-vaultd`; no production secrets or privileged host changes are needed to draft or review this ADR.
- Independent architecture/security review is required before adoption; implementation also requires independent review and recorded verification at the actual enforcement boundaries.

## Approval and supersession

- **Owner approval evidence:** Pending approval of this reviewed adoption candidate. On 2026-10-12 the owner requested: “go through the approval motions for ADR 0001, once you're done make it a key pilar of M2 in the roadmap”. This authorizes preparation and review; final scoped adoption includes the lifecycle decision and `PROJECT.md` amendments above.
- **Independent adoption review:** [APPROVE for owner consideration](../reviews/ecr_adr_0001_adoption_review.md), independent `ecr_adoption_review` at `b0fdbc2f16bd4d5574d29532be9ef7bfd1051644`, 2026-10-12; no outstanding findings. The earlier [proposal review](../reviews/ecr_adr_0001_review.md) approved documentation landing only and is not reused as adoption approval.
- **Supersedes / is superseded by:** None. The scoped lifecycle decision reconciles this ADR's original universal commit wording with mandatory denial and ADR 0002; all other ADR 0002 boundaries remain unchanged.
- **Next owner action:** Approve or revise the reviewed scoped architecture and canonical-spec amendments. Following approval, the coordinator defines M2 design tasks for the remaining gates; adoption alone does not make backend implementation READY.
