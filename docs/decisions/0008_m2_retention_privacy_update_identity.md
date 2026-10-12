# ADR 0008: M2 retention, privacy and update identity (G6)

- **Status:** PROPOSED — awaiting independent review and owner approval. Nothing here is approved. Contains one explicit owner choice (§3).
- **Date / decision owner:** Proposed 2026-10-12 by Claude Code (M2 implementer) / cham3 project owner.
- **Related task/spec sections:** [M2 task](../tasks/M2-cryptographic-contract-core.md) G6, M2-004, M2-006, A14; PROJECT.md §§4.3, 4.7, 6.1, 10, 12.4; [ADR 0001](0001_eternal_contract_repository.md) retention and privacy gate; SEC-15, SEC-18.

`PROJECT.md` is the canonical product specification. This ADR records a scoped decision or proposal and does not silently replace that document.

## Problem and context

"Eternal" is an append-only design goal, not infinite storage or secrecy against privileged compromise (ADR 0001). G6 must define quotas and full-disk behavior, trusted viewers and redaction, the privacy of permanently retained messages and temporary history, account and key deletion, backup/export/access control, migrations, executable/package update identity, and restore when resources or keys no longer exist.

## Decision (proposed)

### 1. Capacity and full disk

Per-user quota 64 MiB (configurable 16 MiB–1 GiB) and the 1 MiB restrictive reserve from ADR 0006. When the quota or free space is exhausted: new or expanded grants are refused (`quota_exhausted`/`storage_unavailable`); restrictive records still use the reserve; beyond that, denial continues in memory and audit gaps are reported (ADR 0007). Nothing is silently pruned. History compaction or archival needs a later decision.

### 2. Access to history

- `history` returns a user's lineages, commits, lifecycle records, gaps and statuses only to that user's `trusted_client` and to `admin`. Services read only what an operation needs. No other user, application or role reads it through cham3 interfaces.
- Not secret against privileged compromise: root, a compromised registryd and offline disk access can read the store (stated limit, PROJECT §3.4).
- Retained content: structured state (including `resource_id` values such as paths), the bounded `message`, identities and times, for every committed transaction including temporary grants. Messages are restricted at the schema (ADR 0004) and clients must not put secrets or unnecessary sensitive details in them; the trusted client warns that messages are permanent.

### 3. Owner choice: deletion granularity

- **Option A (recommended): whole-account deletion only.** No per-entry redaction or erasure; append-only history stays verifiable. Deleting the account removes it all.
- **Option B: per-lineage deletion** (all history of one application installation) with a tombstone record; history of the remaining lineages stays verifiable. More privacy control, more recovery complexity.

Per-commit erasure is rejected in both options because it breaks lineage verification and could hide revocations.

### 4. Account and key deletion

- `admin` `delete_user`: deletes the user's vault key objects (overwrite then unlink; SSD/journaling filesystems may retain copies — stated limit), registry store and reserve, and identityd records; writes a tombstone (`user_id`, UID, time) to the global anchor so a restored copy of that store is quarantined, never resurrected. A later account reusing the UID gets a new identity and new lineages; tombstoned `user_id`s never become valid again.
- Invalidated or rotated keys: the encrypted private key object is deleted once no longer needed for signing; public keys, certificates and invalidations stay in identityd so retained history remains verifiable.

### 5. Backup and export

- **Export** (owner `trusted_client`): envelopes, lifecycle records and gaps as `cham3_dcbor_v1`, with the public keys needed to verify them, for inspection or archival. Exports grant nothing and cannot be imported as authority.
- **Backup/restore** (admin): full-store backups are not authority; restore triggers stale/lost-store detection and the conservative recovery ceremony (ADR 0006). Vault key backups are encrypted key objects only; restoring them never revives invalidated keys (identityd state wins).

### 6. Migrations

- Journal and key-object formats carry versions; upgrades migrate forward only, after the replay check, writing a `recovery`-class migration record and re-anchoring; an older package refuses newer formats (fail closed). Signed objects are never rewritten.
- Wire schema versions follow ADR 0004 (new forward commits under fresh approval). A package update never silently expands authority: authority changes only through admitted user-signed commits.

### 7. Executable and package updates

- Commits bind `package_identity` and `executable_sha256`. After an update that changes either, policyd reports `unknown_identity` (denied) for that lineage until the user approves an `identity_transition` commit rebinding the existing state to the new identity (ADR 0004). M2 supplies these values from fixtures; M3 supplies verified attribution.
- Automatic continuity for updates from the same authenticated package origin (for example the signed cham3/Debian archive) is a possible later policy; it requires trusted package-origin attribution and its own owner decision.

### 8. Restore when resources or keys are gone

- Forward restore recomputes the target under current identity and policy, signs with the current key and new grant ids. Grants for resources that current policy forbids or that a future resource registry reports missing are excluded and shown; nothing is silently dropped or kept.
- History signed by deleted, rotated or invalidated keys remains verifiable through retained public keys; it never confers current authority by itself.

## Alternatives considered

- **Unlimited retention:** rejected; disks fill and denial must stay possible.
- **Rolling deletion of old history:** rejected for M2; it would weaken audit and anti-replay (grant id uniqueness depends on retained history) without a reviewed design.
- **Encrypting history per user at rest:** protects against offline disk theft when the system is off, but registryd must read it to enforce, so keys would sit on the same machine; deferred to a later full-disk/home encryption decision rather than a partial in-app scheme.

## Tradeoffs and consequences

- Users cannot remove individual embarrassing or sensitive history entries under Option A.
- Application updates require re-approval of the identity transition, which is visible friction until package-origin continuity exists.

## Security implications

- Deletion tombstones prevent stale-backup resurrection of deleted identities (SEC-15).
- Privacy statements are limited to the stated threat model (SEC-18).

## Verification plan (M2-004, M2-006, A14)

Quota and full-disk denial with reserve-backed restrictions; history access by owner, other user, ordinary same-UID process and admin; message restrictions and hostile text escaping; account deletion followed by restore of the deleted store (quarantine) and UID reuse (new identity); export verification with public keys only; migration forward/refusal of newer formats; identity mismatch after an executable hash change and its `identity_transition` positive control; restore with forbidden resources.

## Approval and supersession

- **Owner approval evidence:** none yet. Requires independent review, then an explicit owner decision including §3.
- **Supersedes / is superseded by:** none.
