# ADR 0006: M2 ECR storage, anchors and recovery (G4)

- **Status:** PROPOSED (revision 2) — awaiting independent re-review and owner approval. Nothing here is approved. Contains two explicit owner choices (§9).
- **Date / decision owner:** Proposed 2026-10-12 by Claude Code (M2 implementer); revised 2026-10-12 after the independent review published at 9cf882503848f8c6cc4938a588cc9fc062a9392d (CHANGES_REQUESTED: R04, R05, R08, R10, I01; R11 shared with ADR 0008) / cham3 project owner.
- **Related task/spec sections:** [M2 task](../tasks/M2-cryptographic-contract-core.md) G4, M2-005, M2-006, A07, A08, A10; PROJECT.md §§5.4, 5.7, Phase 2; [ADR 0001](0001_eternal_contract_repository.md) registry storage and recovery; SEC-04, SEC-14. Finding resolutions: [M2-001-002-finding-resolutions.md](../reviews/M2-001-002-finding-resolutions.md).

`PROJECT.md` is the canonical product specification. This ADR records a scoped decision or proposal and does not silently replace that document.

## Problem and context

ADR 0001 requires immutable objects, transactional current-state indexes (heads, epochs, revocations, one-shot state, request ids, signing keys), atomic admission before any activation, serialized writers, separate pending/confirmed outcomes, crash recovery that never grants from incomplete writes, and a rollback-resistant mechanism for authoritative state: hash-valid old history is not freshness. Mandatory denial must not be undone by restarts, failed writes or missing lifecycle evidence. Another file on the same restorable disk is not independent freshness proof, and TPM or external hardware must not become an accidental foundational requirement.

Revision 2 closes the review's gaps: a restriction lost to failed writes plus restart (R04), identity/key state outside the anchored store (R05), records too small for the largest valid object (R08), a shared anchor updated under per-user locks (R10) and an unprotected deletion sequence (R11). Nothing described here is implemented; every mechanism is an M2-005/M2-006 obligation (I01).

## Decision (proposed)

### 1. One authoritative store: per-user append-only journal

- One journal per enrolled user: `/var/lib/cham3/registry/users/<uid>/journal.c3j`, owned by `_cham3-registryd`, directory `0700`, file `0600`; only registryd writes it.
- **It holds all authority state for that user** (R05): identity enrollment, key certificates, key invalidations and identity replacement records (the signed objects of ADR 0004, validated by identityd and persisted by registryd), ECR commits, enforcement outcomes, restrictions, lifecycle records, one-shot reservations/outcomes, session events, policy epochs, time-floor records and deletion records. identityd keeps no store of its own (ADR 0005 §6).
- Frame: `"C3JR"` ‖ length (u32 BE) ‖ record (`cham3_dcbor_v1` map) ‖ `chain` (32 bytes), `chain_n = domain_hash("cham3/journal-chain/v1", chain_{n−1} ‖ u64_be(n) ‖ record)`, `chain_{−1} = domain_hash("cham3/journal-genesis/v1", user_id ‖ u32_be(uid))`.
- **Record size (R08):** the record map is at most 65 536 bytes and embeds at most one signed object (≤ 61 440 bytes, ADR 0004) plus at most 4 096 bytes of other fields. The published maximum object yields a 61 619-byte worst-case `commit_admitted` record.
- **Atomicity:** one logical change = one record, durable after `fdatasync` (plus directory `fsync` at creation). Key invalidation is a single record, so key status and authorization are always read from the same snapshot: no cross-service crash window exists between identity and registry state.
- **Indexes are derived, not stored:** startup replays and re-verifies every record (chain, byte layers, key authority, admission rules) into in-memory indexes. No persisted index file exists.
- **Serialization:** one writer lock per user; checks of expected head/epoch/base/uniqueness/delegation/subject binding run under it immediately before the append; conflicts return `stale_head`/`stale_epoch`.
- **Pending → confirmed:** commits are admitted `pending`; `enforcement_outcome` records bound to commit, runtime id, domain and epoch move them to confirmed/failed per domain. Signed history is never mutated. An admission reported to the caller as failed (for example, anchor write failure after a durable record) may be found admitted after recovery; it remains pending and clients reconcile by `request_id`.
- **Capacity:** per-user quota 64 MiB (configurable 16 MiB–1 GiB); ≤ 4 096 lineages; a preallocated 1 MiB reserve is released only for restrictive records (restriction, lifecycle, use reservation/outcome, audit gap, key invalidation, deletion, recovery), never for admissions.

### 2. Global anchor and its serialization (R10)

- The anchor is one map: per user `(seq, chain, state)`, plus deleted-user tombstones and the registryd **run marker** (§3). It is written as two alternating slots (`"C3AN"` ‖ slot counter ‖ map ‖ SHA-256 checksum); startup takes the valid slot with the higher counter.
- registryd holds the authoritative anchor map **in memory** and never re-reads disk during operation. One global anchor lock protects it. **Lock order: user lock → anchor lock**; no code acquires a user lock while holding the anchor lock.
- Update protocol: under the anchor lock, apply the caller's change to the in-memory map, increment the slot counter, serialize the whole map, write it to the older slot, `fdatasync`, release. Callers reply only after the slot containing their change is durable. Concurrent updates may be coalesced (group commit) but each caller waits for a durable slot that contains its change; no snapshot taken outside the lock is ever written, so one user's update cannot overwrite another's.
- A failed slot write leaves the change non-durable: restrictive changes stay enforced in memory and in the restriction ledger (§3) and are retried; admissions are reported failed.

Anchor backends behind one interface (all **planned**, none implemented or tested yet — I01):

| Backend | Rollback domain | Planned status |
|---|---|---|
| `separate_volume` | A configured path on a different filesystem (`st_dev` must differ from the store), intended for a separate disk/partition/removable medium the administrator declares is not restored with `/var/lib/cham3` | To be implemented and tested in M2-006 (A10) with a distinct filesystem in a disposable environment |
| `same_volume` | Same filesystem as the store | Degraded mode: detects partial restores, torn tails and journal-only substitution; **cannot** detect whole-volume restoration; reported as `rollback_protection: degraded` |
| `tpm2_nv` | TPM 2.0 NV counter + NV index | Interface reserved; not in M2 (needs a reviewed TSS adapter and owner decision) |

Rollback resistance holds only if the anchor's storage is not rolled back together with the store. No software-only design detects an offline restore of every storage domain; that is outside PROJECT §3.4's baseline unless hardware is configured.

### 3. Restart safety for restrictions (R04)

A restriction must survive any write failure followed by a restart. Three mechanisms make every unclean run either precisely recoverable or conservatively contained:

1. **Run marker** (anchor): `{run_id, boot_id, state}` with `state = running` written durably after replay and **before** any `eligible` decision is served, and `state = clean` written only by an orderly shutdown after every restrictive change is durable. If the marker cannot be written at startup, registryd serves no `eligible` decisions.
2. **Restriction ledger** (memory-backed, outlives the process, not the boot): `/run/cham3/registryd/restriction_ledger` on tmpfs (`RuntimeDirectory`, `RuntimeDirectoryPreserve=yes`, `0700`, size-capped). Every restrictive change (restriction, revocation intent, key invalidation, expiry observation, consumption reservation, deletion intent, emergency restriction) appends `{run_id, user, targets, kind, time}` here **before** any disk write is attempted; disk failures cannot prevent it. If the ledger itself cannot be written, registryd removes the ledger's validity file and stops serving `eligible` decisions (the next start then sees an unverifiable run).
3. **Restrictive write path:** in-memory denial → ledger entry → journal record (reserve-backed) → anchor update → reply. If the journal or anchor write fails, denial continues in memory, status reports `restriction_not_durable`, and registryd retries; it does not need to exit.

Restart decisions (before serving anything):

| Previous run marker | Restriction ledger | Decision |
|---|---|---|
| `clean` | any | Resume; clear ledger |
| `running`, same `boot_id` | valid for that `run_id` | Re-journal every ledger entry not yet in the journal (restrictive records first, durable), then resume |
| `running`, same `boot_id` | missing or invalid | **Unverifiable run → containment** |
| `running`, different `boot_id` (reboot, power loss, kernel crash) | gone with the boot | **Unverifiable run → containment** |
| unreadable | — | Quarantine (§4) |

**Containment** (unverifiable unclean run): every user's grants admitted before the restart are `suspended` (denied), live temporary grants end, and one-shots without a recorded success are consumed. Suspension is lifted only by fresh authorization (owner choice §9.2). This includes the review's case — journal and anchor both unwritable, restart after storage recovers — which is either same-boot (ledger re-applies the restriction precisely) or cross-boot (containment); in no case does access resume from missing evidence. The cost is that a power loss or kernel crash requires users to reconfirm their permissions.

### 4. Recovery decision table (per user, after the run-marker decision)

| Observation (journal last valid seq `m`, anchor `a`) | Decision |
|---|---|
| `m == a`, chain at `a` matches | Resume |
| `m == a + 1`, chain extends the anchored chain, frame complete | Roll the anchor forward (durable but unacknowledged tail), resume |
| Torn frame after the last complete frame, beyond `a` | Truncate the torn bytes, then apply the rows above |
| `m < a` | **Stale journal**: quarantine |
| `m > a + 1` | **Unexplained records**: quarantine |
| Chain mismatch at `a` | Quarantine |
| Anchor slots both invalid/missing while journals exist | Quarantine every user |
| Journal missing while the anchor knows the user | **Lost store**: quarantine |
| Both absent and user unknown | Genesis at enrollment: header + enrollment record + anchor |
| Tombstone `pending` | Keep the user denied; resume idempotent deletion (§7) |
| Tombstone `deleted` and files for that `user_id` exist | Never load them; quarantine and remove |
| Any record fails replay verification | Quarantine |

**Quarantine** denies every grant and key of that user (`quarantined`). Exit only through the `admin` `recover_user` ceremony: append a `recovery` record, end temporary grants, consume unconfirmed one-shots, mark every persistent grant admitted before the recovery point `requires_reauthorization`, raise the user's minimum valid policy epoch and re-anchor. Re-authorization follows §9.2. Recovery never infers validity, undoes consumption or claims complete history.

### 5. Identity and key freshness (R05)

- Identity records live in the anchored journal (§1), so restoring older identity state is a stale-journal/lost-store detection like any other rollback; no separate identityd store exists to restore.
- identityd rebuilds its index from registryd at start and answers `resolve_key` with the journal sequence of its snapshot; if registryd is unavailable, the user is contained/quarantined, or the snapshot is older than a restrictive change it knows of, the answer is `unknown` (deny).
- Key invalidation, identity replacement and deletion are restrictive: in-memory denial and ledger entry first, then one durable journal record, then reply. Until durable, the key is denied everywhere (vaultd checks key status with identityd before every signature).
- Vault key files are not authority: restoring an older encrypted key object cannot revive a key whose invalidation or replacement is in the journal. The vault's failed-attempt counter is not rollback-protected (an offline restore resets delays; Argon2 cost and the passphrase remain) — stated limit.

### 6. Durability boundaries and fault points

Admission: validate under the user lock → append → `fdatasync` → anchor update → reply. Restrictive change: §3 order. Fault injection (A08/A10) at every boundary: before append; torn frame (inside magic, length, record, chain); after append before anchor; torn/failed anchor slot write; after anchor before reply; ledger write; run-marker write at start and at clean shutdown; process death (release binaries, panic=abort) at each point; `ENOSPC`/`EIO` on journal and anchor; concurrent users updating the anchor; deletion steps (§7).

### 7. Account deletion: tombstone first (R11, shared with ADR 0008)

1. `admin` `delete_user(uid)`: deny all of the user's grants and keys in memory; ledger entry.
2. Under the user lock and anchor lock: append a durable `deletion_pending` record to the user's journal and set the anchor tombstone `(user_id, uid, pending)` durably. If either fails, deletion is reported failed, **nothing is destroyed**, and the denial stays (ledger, §3).
3. Idempotent cleanup, each step retried until done: vaultd erases the user's key objects (it refuses to sign for a pending/deleted user from step 1); registryd removes the user's journal and reserve (identity records go with it).
4. Set the tombstone to `deleted` durably; only then report success.

At startup a `pending` tombstone keeps the user denied and resumes step 3; a `deleted` tombstone causes any surviving or restored file of that `user_id` to be quarantined and removed, never loaded. Tombstones are retained permanently (one small entry per deleted user). A later account with the same UID enrolls a new `user_id`. Restoring an old full system **including** the anchor defeats tombstones only in `same_volume` degraded mode (§2 limit).

### 8. Backup and restore (coordinated)

All authority state is in the journals plus the anchor, so there is one coordinated rule: a restored registry store is never authority. Restore triggers stale/lost-store detection (independent anchor) or must be followed by the recovery ceremony (§4) when the anchor was restored with it. Vault key-object backups are encrypted keys only and confer nothing (§5). Exports for inspection are separate (ADR 0008).

### 9. Owner choices

1. **Default anchor mode.** **Option A (recommended):** `same_volume` degraded mode by default with visible status; administrators may configure `separate_volume`; `tpm2_nv` later. **Option B:** refuse new or expanded grants unless an independent anchor is configured (makes a second volume or TPM a de facto requirement).
2. **Lifting containment/quarantine.** **Option C1 (recommended):** the user re-authorizes with freshly approved, signed restore commits that remove the suspended grant ids and re-add the grants they still want with new ids (bulk-offered by the trusted client, one approval per lineage); quarantine additionally needs the admin ceremony first. **Option C2:** containment is lifted only by the admin ceremony followed by the same signed re-authorization.

Mandatory denial, quarantine, ledger and recovery rules are identical under every option.

## Alternatives considered

- **SQLite (WAL, synchronous=FULL):** mature engine; rejected for M2 because it adds a large C FFI dependency to the most security-critical store and coarser fault injection.
- **Separate identityd store with its own anchor entry:** keeps storage with the owning service; rejected because a cross-service write ordering would be needed for every invalidation and restore, which is exactly the gap R05 found. identityd keeps ownership of validation and policy, not of bytes.
- **Accepting the double-failure residual (revision 1):** rejected by review R04; contradicts ADR 0001's rule that a failed revocation never justifies continuing access.
- **Containment after every unclean shutdown without a ledger:** simpler, but makes every service crash within a boot cost a user reconfirmation; the tmpfs ledger limits containment to reboots and genuinely unverifiable runs.
- **Persisted index files, one global journal, EFI-variable anchors, same-disk duplicates presented as rollback-resistant, service-signed anchors:** rejected as in revision 1 (stale index restoration, failure containment, flash wear, misleading claims, no freshness from a same-disk key).

## Tradeoffs and consequences

- Reboots after an unclean run require users to reconfirm grants (§3, §9.2). A clean shutdown avoids it.
- Startup replay re-verifies signatures; bounded by quota.
- Degraded mode is the default under Option A and must be visible in M5.

## Security implications

- Mandatory restrictions survive write failures and restarts: same-boot precisely, cross-boot conservatively. Missing evidence never resumes access.
- Identity, key and authorization freshness share one anchored store; there is no restore of one without detection by the other.
- Concurrent anchor updates are serialized; acknowledged changes cannot be lost to another user's write.
- Rollback detection depends on the anchor's independence, stated as an explicit assumption (SEC-18).

## Verification plan (M2-005, M2-006, A07, A08, A10, A14)

All planned; nothing has run. Atomic admission under concurrent writers; every fault point in §6; the review's case (journal and anchor unwritable, restart after recovery) in both same-boot and new-boot variants; ledger missing/invalid; run marker unwritable; concurrent users and tombstones racing the anchor; independent restore of each storage domain (journal only, anchor only, vault files only, all but the independent anchor); key invalidation crash points; deletion crash at every step including tombstone write failure; `separate_volume` detection on a distinct filesystem; `same_volume` reporting degraded; quota/reserve. Release binaries under subprocess fault injection.

## Approval and supersession

- **Independent review:** revision 1 CHANGES_REQUESTED at 9cf8825 (R04, R05, R08, R10, R11 via G6, I01). Revision 2 requires re-review.
- **Owner approval evidence:** none. Requires independent re-review, then an explicit owner decision including §9.
- **Supersedes / is superseded by:** revision 2 supersedes revision 1 of this proposal (commit f1437f0d254af2968c3598bf14d1a9b2b8e791cd).
