# ADR 0008: M2 retention, privacy and update identity (G6)

- **Status:** PROPOSED (revision 3) — awaiting independent re-review and owner approval. Nothing here is approved. Contains one explicit owner choice (§3).
- **Date / decision owner:** Proposed 2026-10-12 by Claude Code (M2 implementer); revised 2026-10-12 after the independent review published at 9cf882503848f8c6cc4938a588cc9fc062a9392d (CHANGES_REQUESTED: R05, R06, R11); revision 3 on 2026-10-12 after the re-review at 336f13be6a19bb871ea956d64f9cacb0936fe028 (CHANGES_REQUESTED: R12 restore validation, R13 coordinated recovery path) / cham3 project owner. Finding resolutions: [revision 2](../reviews/M2-001-002-finding-resolutions.md), [revision 3](../reviews/M2-001-002-finding-resolutions-r2.md).
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
- **Option B: per-lineage deletion** (all history of one application installation); history of the remaining lineages stays verifiable. More privacy control, more recovery complexity: it needs a per-lineage variant of the tombstone-first ceremony (§4) and lineage tombstones in the anchor so that a deleted lineage's restored records are never replayed, and its grant ids stay reserved.

Per-commit erasure is rejected in both options because it breaks lineage verification and could hide revocations.

### 4. Account and key deletion (R11)

- `admin` `delete_user` follows the **tombstone-first** ceremony of [ADR 0006 §7](0006_m2_storage_anchor_recovery.md): immediate in-memory denial of all the user's grants and keys plus a restriction-ledger entry; a durable `deletion_pending` journal record **and** a durable anchor tombstone `pending` before anything is destroyed (if either write fails, deletion fails, nothing is destroyed and the denial stays); idempotent per-service cleanup (vaultd erases key objects — overwrite then unlink, with the stated SSD/journaling-filesystem limit — and registryd removes the journal, which also holds the identity records); then the tombstone becomes `deleted` durably and only then is success reported.
- Crash or failure at any step leaves the user denied with a `pending` tombstone; startup resumes cleanup. A `deletion_pending` user cannot open, continue or close a recovery ceremony; deletion requested during an open ceremony aborts it (ADR 0006 §10). A store, vault file or backup of a tombstoned `user_id` is never loaded again (quarantined and removed). Tombstones are kept permanently (one small entry per deleted user). A later account reusing the UID enrolls a new `user_id`; tombstoned identities never become valid again. Restoring an old full system **including** a same-volume anchor defeats tombstones (degraded-mode limit, ADR 0006 §2).
- Invalidated or rotated keys: the encrypted private key object is deleted once no longer needed for signing; public keys, certificates and invalidations stay in the journal so retained history remains verifiable.

### 5. Backup, restore and export (R05)

- **One coordinated rule:** all authority state — identity enrollments, certificates, invalidations, replacements, commits, restrictions, lifecycle and deletion records — lives in the per-user journals under one global anchor (ADR 0006 §1, §5). There is no separately restorable identity store, so a restore of older identity or key state is detected exactly like any other stale journal.
- **Backup/restore** (admin): a restored registry store is never authority. With an independent anchor, restore is detected as stale or lost and quarantined; when the anchor is restored too, `recover_user` (ADR 0006 §4) is mandatory. Either way the user then re-authorizes only through the complete recovery ceremony of ADR 0006 §10 (revision 3, R13): fresh recovery keys at a new key epoch, per-lineage freshly approved ceremony-bound restore commits with new grant ids; every grant and contract key from the restored state stays denied, and a restored vault key file is below the minimum key epoch. A quarantined store whose anchor shows identity records missing from the journal can only be replaced (`replace_identity`). Vault key-object backups are encrypted keys only; restoring them never revives a key whose invalidation, replacement or deletion is in the journal, because vaultd checks key status with identityd before every signature.
- **Export** (owner `trusted_client`): envelopes, lifecycle records and gaps as `cham3_dcbor_v1`, with the public keys needed to verify them, delivered in pages of at most 65 536 bytes. Exports grant nothing and cannot be imported as authority.

### 6. Migrations

- Journal and key-object formats carry versions; upgrades migrate forward only, after the replay check, writing a `recovery`-class migration record and re-anchoring; an older package refuses newer formats (fail closed). Signed objects are never rewritten.
- Wire schema versions follow ADR 0004 (new forward commits under fresh approval). A package update never silently expands authority: authority changes only through admitted user-signed commits.

### 7. Executable and package updates

- Commits bind `package_identity` and `executable_sha256`. The lineage's authoritative subject changes **only** through an `identity_transition` commit whose `delta.identity_change {from, to}` is displayed and approved; every other commit type must restate the authoritative subject exactly (ADR 0004 §7a, R06). After an update that changes either value, policyd reports `unknown_identity` (denied) until such a transition is admitted. M2 supplies these values from fixtures; M3 supplies verified attribution.
- Automatic continuity for updates from the same authenticated package origin (for example the signed cham3/Debian archive) is a possible later policy; it requires trusted package-origin attribution and its own owner decision.

### 8. Restore when resources or keys are gone

- Forward restore recomputes the target under current identity and policy, signs with the current key and new grant ids. It matches only **persistent** grants of the parent or an ancestor (revision 3, R12, wire format §6.7): historical timed, session and one-shot grants are never restored, and making temporary scope persistent is a separate freshly approved revision. Grants for resources that current policy forbids or that a future resource registry reports missing are excluded and shown; nothing is silently dropped or kept.
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

Restore of historical session/timed/one-shot scope (rejected) versus persistent scope of the same ancestor (accepted), oracle vectors published (R12); recovery after a full-store restore through the ADR 0006 §10 ceremony, with restored vault key files refused; deletion during an open ceremony; deletion crash at every ceremony step including tombstone/journal write failure, restore of a deleted user's store and vault files, UID reuse; independent restore of identity-bearing journals versus vault files; ordinary revision/merge/restore/temporary grant attempting an identity substitution; quota and full-disk denial with reserve-backed restrictions; history access by owner, other user, ordinary same-UID process and admin; message restrictions and hostile text escaping; account deletion followed by restore of the deleted store (quarantine) and UID reuse (new identity); export verification with public keys only; migration forward/refusal of newer formats; identity mismatch after an executable hash change and its `identity_transition` positive control; restore with forbidden resources.

## Approval and supersession

- **Independent review:** revision 1 CHANGES_REQUESTED at 9cf8825 (R05, R06, R11). Revision 2 CHANGES_REQUESTED at 336f13b (R12, R13; R05, R06, R11 passed at design level). Revision 3 requires re-review.
- **Owner approval evidence:** none. Requires independent re-review, then an explicit owner decision including §3.
- **Supersedes / is superseded by:** revision 3 supersedes revision 2 (commit a2bbb370c867da57fdc72274fb17ca19a23426e4), which superseded revision 1 (commit f1437f0d254af2968c3598bf14d1a9b2b8e791cd).
