# ADR 0006: M2 ECR storage, anchors and recovery (G4)

- **Status:** PROPOSED — awaiting independent review and owner approval. Nothing here is approved. Contains one explicit owner choice (§5).
- **Date / decision owner:** Proposed 2026-10-12 by Claude Code (M2 implementer) / cham3 project owner.
- **Related task/spec sections:** [M2 task](../tasks/M2-cryptographic-contract-core.md) G4, M2-005, M2-006, A07, A08, A10; PROJECT.md §§5.4, 5.7, Phase 2; [ADR 0001](0001_eternal_contract_repository.md) registry storage and recovery; SEC-04, SEC-14.

`PROJECT.md` is the canonical product specification. This ADR records a scoped decision or proposal and does not silently replace that document.

## Problem and context

ADR 0001 requires immutable objects, transactional current-state indexes (heads, epochs, revocations, one-shot state, request ids), atomic admission of objects plus pending state before any activation, serialized writers, separate pending/confirmed outcomes, crash recovery that never grants from incomplete writes, and a rollback-resistant mechanism for authoritative state: hash-valid old history is not freshness. The task adds: another file on the same restorable disk is not independent freshness proof, and TPM or external hardware must not become an accidental foundational requirement.

## Decision (proposed)

### 1. Backend: per-user append-only journal (safe Rust)

- One journal per user: `/var/lib/cham3/registry/users/<uid>/journal.c3j`, owned by `_cham3-registryd`, directory `0700`, file `0600`. No other process writes it (DAC; ProtectSystem; dedicated user).
- Record frame: `"C3JR"` (4 bytes) ‖ length (u32 big-endian) ‖ record (`cham3_dcbor_v1` map) ‖ `chain` (32 bytes) where `chain_n = domain_hash("cham3/journal-chain/v1", chain_{n−1} ‖ u64_be(n) ‖ record)`, and `chain_0` is computed over the header record with `chain_{−1} = domain_hash("cham3/journal-genesis/v1", user_id ‖ u32_be(uid))`.
- Record kinds (closed set, all carry `seq` and trusted `time`): `header`, `commit_admitted` (envelope bytes, commit id, lineage, branch role, admitted as pending), `enforcement_outcome`, `restriction`, `lifecycle`, `use_reserved`, `use_completed`, `session_event`, `policy_epoch`, `audit_gap`, `startup`, `clean_shutdown`, `recovery`. Semantics of lifecycle kinds are in ADR 0007.
- **Atomicity:** one admission = one record. The record is durable once its bytes and `fdatasync` complete (plus directory `fsync` when the file is created). An incomplete trailing frame is, by construction, unacknowledged and is truncated during recovery only if it lies beyond the anchored sequence (§3).
- **Indexes are derived, not stored:** on start, registryd replays and re-verifies every record (chain, envelope byte layers, admission rules) into in-memory indexes: trunk head and live temporary heads per lineage, every grant id ever added per lineage, per-user request ids, revocations/restrictions, one-shot reservations and outcomes, ended sessions, pending/confirmed/failed outcomes per commit and domain, policy epochs, time floor. No persisted index file exists, so a stale index file cannot be restored; history search indexes for M5 may be rebuilt the same way.
- **Serialization:** one writer lock per user. Expected head, expected policy epoch, base state, uniqueness and delegation checks run under the lock immediately before the append; a conflicting second writer gets `stale_head`/`stale_epoch` and must obtain fresh approval.
- **Pending → confirmed:** commits are admitted `pending`. `enforcement_outcome` records (from authenticated consumer roles; fixture in M2) move a commit to `confirmed` or `failed` per domain, bound to `commit_id`, runtime id, domain and the current epoch; wrong, stale or reused confirmations are rejected. Signed history is never mutated.
- **Capacity:** per-user journal quota 64 MiB (configurable 16 MiB–1 GiB), record ≤ 65 536 bytes, ≤ 4 096 lineages per user. Admission beyond quota or free space fails (`quota_exhausted`). A 1 MiB per-user reserve file (`reserve.bin`, preallocated) is released only to write restrictive records (restriction, lifecycle, use reservation/outcome, audit gap, recovery), never admissions.

### 2. Durability boundaries and write order

1. Validate under the user lock (admission) or decide the restrictive change.
2. For restrictive changes, apply the in-memory denial **first** (ADR 0007).
3. Append the frame; `fdatasync`.
4. Update the anchor (§3) to `(seq, chain)`; make it durable.
5. Reply.

Fault-injection points (A08/A10): before 3; partial frame (torn write at every byte boundary class: inside magic, length, record, chain); after 3 before 4; partial anchor write; after 4 before 5; process death at each point; `ENOSPC`/`EIO` at 3 and 4.

### 3. Anchor

The anchor records, per user, `(seq, chain)` of the last durable record, plus a global record of known users and deleted-user tombstones. It is written as two alternating slots, each `"C3AN"` ‖ slot counter ‖ map ‖ SHA-256 checksum; recovery takes the valid slot with the higher counter.

Anchor backends behind one interface:

| Backend | Rollback domain | Status in M2 |
|---|---|---|
| `separate_volume` | A configured path on a **different filesystem** from the registry store (checked: different `st_dev`); intended for a separate disk/partition or removable medium declared by the administrator as not restored together with `/var/lib/cham3`. | Implemented and tested (A10) with a distinct filesystem in a disposable environment. |
| `tpm2_nv` | TPM 2.0 NV monotonic counter plus NV index holding the anchor hash; survives disk replacement/restore. | Interface only; not implemented in M2 (requires a reviewed TSS FFI adapter and owner decision). |
| `same_volume` | Same filesystem as the store. | Implemented as **degraded mode**: detects partial restores, torn tails and in-place substitution of the journal alone; **cannot** detect restoration of the whole volume. Status `rollback_protection: degraded` is reported by policyd and must be shown, never described as rollback-resistant. |

Assumption stated precisely: rollback resistance holds only if the anchor's storage is not rolled back together with the registry store. No software-only design can detect an offline restore of every storage domain the machine has; that is a physical/privileged attack outside PROJECT §3.4's baseline unless hardware (`tpm2_nv`) is configured.

### 4. Recovery decision table (per user, at start)

| Observation (journal last valid seq `m`, anchor `a`) | Decision |
|---|---|
| `m == a`, chain at `a` matches | Resume |
| `m == a + 1`, chain extends anchored chain, last frame complete | Roll anchor forward (unacknowledged durable tail), resume |
| Torn frame after the last complete frame, beyond `a` | Truncate the torn bytes, then apply the rows above |
| `m < a` (journal older than anchor) | **Stale**: quarantine user |
| `m > a + 1` | **Unexplained records**: quarantine |
| Chain mismatch at `a` (substituted history) | Quarantine |
| Anchor slots both invalid/missing while journal exists | Quarantine |
| Journal missing while anchor has `a > 0` or global anchor knows the user | **Lost store**: quarantine |
| Both absent and user unknown to global anchor | Genesis: create header + anchor |
| Tombstoned user reappears (restored deleted store) | Quarantine; never resurrect |
| Any record fails replay verification | Quarantine |

**Quarantine** denies every grant of that user (policyd reports `quarantined`); mandatory denial and lifecycle restriction continue. Exit only through the `admin` `recover_user` ceremony, which appends a `recovery` record and re-anchors after making state conservative: all live temporary grants terminated, every one-shot without a recorded success treated as consumed, every persistent grant admitted before the recovery point marked `requires_reauthorization` (denied until the user approves a fresh current-parented restore/revision with new grant ids), and the user's minimum valid policy epoch raised. Recovery never infers continuing validity, undoes consumption or claims complete history.

### 5. Owner choice: default anchor mode

- **Option A (recommended):** ship `same_volume` degraded mode by default with visible status; administrators may configure `separate_volume`; `tpm2_nv` later. Does not make extra hardware foundational; honest about limits.
- **Option B:** refuse new or expanded grants unless `separate_volume` or `tpm2_nv` is configured. Stronger, but makes a second volume or TPM a de facto requirement for usable authorization, which the task warns against.

Either option keeps mandatory denial, quarantine and the recovery rules identical.

### 6. Backup and restore

Backups of `/var/lib/cham3/registry` must not be treated as authority. Restoring a backup produces a stale or lost-store detection (if the anchor is independent) and requires the recovery ceremony with the conservative semantics above. Exports for history inspection are separate (ADR 0008).

## Alternatives considered

- **SQLite (WAL, synchronous=FULL):** mature transactional engine and indexes; rejected for M2 because it adds a large C FFI dependency to the most security-critical store, and fault injection at its internal durability boundaries is coarser than at a single append/fsync boundary we control. Could be revisited for history search indexes, which are derivable.
- **Persisted index files with checkpoints:** faster startup; rejected for v1 because stale index files become a restorable rollback vector; replay is cheap at the quota sizes.
- **One global journal for all users:** simpler anchoring; rejected to keep per-user deletion, quotas and failure containment.
- **Anchor in an EFI variable:** independent of disk restore on UEFI systems, but flash wear and firmware fragility at per-commit write rates and absence on BIOS VMs make it unsuitable as a default.
- **Same-disk duplicate anchor file presented as rollback-resistant:** rejected as misleading; kept only as explicitly degraded mode.
- **Service-signed anchors:** a signature by a key stored on the same disk does not establish freshness.

## Tradeoffs and consequences

- Startup replay cost grows with history; bounded by quota. Verification on replay re-checks signatures (Ed25519 verify per commit).
- Recovery ceremonies cost the user re-approval of persistent grants; this is the price of fail-closed recovery.
- Degraded mode is the default under Option A; status reporting must make that visible in M5.

## Security implications

- Admission and activation stay separate; nothing becomes active without confirmation and current authorization.
- Torn or unacknowledged writes never grant: only complete, chained, anchored (or exactly-one-ahead) records are replayed.
- Rollback detection depends on the anchor's independence, stated as an explicit assumption; there is no claim beyond it (SEC-18).

## Verification plan (M2-005, M2-006, A07, A08, A10)

Atomic admission under concurrent writers; kill and torn-write injection at every boundary in §2; replay determinism; stale journal, stale anchor, swapped anchor slots, corrupted checksum, missing anchor, missing journal, substituted record with valid signature, extra injected record, deleted-user restore; `separate_volume` on a distinct filesystem detecting full-store rollback; `same_volume` reporting degraded and not claiming detection; `ENOSPC` with reserve; recovery ceremony semantics. Process death uses real release binaries (panic=abort) in subprocesses.

## Approval and supersession

- **Owner approval evidence:** none yet. Requires independent review, then an explicit owner decision including §5.
- **Supersedes / is superseded by:** none.
