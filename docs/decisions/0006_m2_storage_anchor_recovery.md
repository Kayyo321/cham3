# ADR 0006: M2 ECR storage, anchors and recovery (G4)

- **Status:** APPROVED (revision 4) — owner-approved 2026-10-12 (G4) after the independent revision-5 re-review, with the §9 owner choices **9.1 Option A** and **9.2 Option C1**.
- **Date / decision owner:** Proposed 2026-10-12 by Claude Code (M2 implementer); revised 2026-10-12 after the independent review published at 9cf882503848f8c6cc4938a588cc9fc062a9392d (CHANGES_REQUESTED: R04, R05, R08, R10, I01; R11 shared with ADR 0008); revision 3 on 2026-10-12 after the re-review at 336f13be6a19bb871ea956d64f9cacb0936fe028 (CHANGES_REQUESTED: R13 — containment blocked the key authority its own recovery needs); revision 4 on 2026-10-12 after the re-review at f451f2e346e6981b24802a7e9070ba09303c40d3 (CHANGES_REQUESTED: R14 — no authenticated registry route for the step-3 key-naming record) / cham3 project owner. Revision-3 responses: [M2-001-002-finding-resolutions-r2.md](../reviews/M2-001-002-finding-resolutions-r2.md); revision-4 responses: [M2-001-002-finding-resolutions-r3.md](../reviews/M2-001-002-finding-resolutions-r3.md).
- **Related task/spec sections:** [M2 task](../tasks/M2-cryptographic-contract-core.md) G4, M2-005, M2-006, A07, A08, A10; PROJECT.md §§5.4, 5.7, Phase 2; [ADR 0001](0001_eternal_contract_repository.md) registry storage and recovery; SEC-04, SEC-14. Finding resolutions: [M2-001-002-finding-resolutions.md](../reviews/M2-001-002-finding-resolutions.md).

`PROJECT.md` is the canonical product specification. This ADR records a scoped decision or proposal and does not silently replace that document.

## Problem and context

ADR 0001 requires immutable objects, transactional current-state indexes (heads, epochs, revocations, one-shot state, request ids, signing keys), atomic admission before any activation, serialized writers, separate pending/confirmed outcomes, crash recovery that never grants from incomplete writes, and a rollback-resistant mechanism for authoritative state: hash-valid old history is not freshness. Mandatory denial must not be undone by restarts, failed writes or missing lifecycle evidence. Another file on the same restorable disk is not independent freshness proof, and TPM or external hardware must not become an accidental foundational requirement.

Revision 2 closes the review's gaps: a restriction lost to failed writes plus restart (R04), identity/key state outside the anchored store (R05), records too small for the largest valid object (R08), a shared anchor updated under per-user locks (R10) and an unprotected deletion sequence (R11). Revision 3 (R13) separates **grant containment** from the **key authority** a tightly bound recovery needs, and freezes the complete staged recovery ceremony (§10) that the owner options of §9.2 now refer to. Revision 4 (R14) gives step 3 its authenticated route: the vaultd-only registryd operation `name_recovery_keys`, with its durability and reconciliation rules. Nothing described here is implemented; every mechanism is an M2-005/M2-006 obligation (I01).

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

- The anchor is one map: per user `(seq, chain, state, user_id, identity_seq)` — `identity_seq` counts that user's identity records (enrollment, certificates, invalidations, replacement) so a lost journal tail that held key-state changes is visible (§10) — plus deleted-user tombstones and the registryd **run marker** (§3). It is written as two alternating slots (`"C3AN"` ‖ slot counter ‖ map ‖ SHA-256 checksum); startup takes the valid slot with the higher counter.
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

**Containment** (unverifiable unclean run): every user's grants admitted before the restart are `suspended` (denied), live temporary grants end, and one-shots without a recorded success are consumed. A durable `containment` record per user (reserve-backed, restrictive path) names the containment point; until it is durable the user stays denied in memory and no ceremony can open. A suspended grant id is ended **permanently**: suspension is never lifted by un-suspending a grant, only by re-authorizing its scope with a new grant id through the recovery ceremony of §10 (owner choice §9.2 decides who may open it). Containment restricts **grants**, not the user's identity key; the ceremony's narrowly scoped key authority (§10) is what lets the user produce fresh signatures while every old grant and old contract key stays denied. This includes the review's case — journal and anchor both unwritable, restart after storage recovers — which is either same-boot (ledger re-applies the restriction precisely) or cross-boot (containment); in no case does access resume from missing evidence. The cost is that a power loss or kernel crash requires users to reconfirm their permissions.

**Erratum E5 (owner, 2026-10-14; wording corrected 2026-10-14 after M2-006-A-R3-02 and confirmed by the owner on 2026-10-14): the guarantee's limit under a simultaneous failure of every write.** The rule "a restriction must survive any write failure followed by a restart" holds when at least one durable mark of the restriction is made before the process dies:

- its **ledger entry**;
- its **own durable record**: for a per-user restriction (restriction, key invalidation, containment) the record in the user's journal; for a policy restriction (`set_policy_epoch`, `set_emergency_restriction(true)`), which has no per-user journal record, the anchor image that carries the policy state;
- once its ledger append has failed, the **removal or emptying of the ledger's validity file**, or the **`unverifiable` run marker** in the anchor.

Every failure of persistent storage alone (journal, anchor or both) is covered by the ledger; every failure of the ledger alone is covered by the invalidation, the marker or the restriction's own record. Outside the guarantee is the one case in which every mark fails at the same moment — the memory-backed ledger append, both ways of invalidating its validity file, the anchor write and, for a per-user restriction, the journal write — **and** the process then dies before any retry succeeds, **and** registryd restarts in the same boot. Stated consequences:

- **What is lost:** only the restrictions none of whose marks became durable; at a same-boot restart the ledger looks complete, the marker reads `running`, and the run is resumed without them. Every restriction with any durable mark is unaffected.
- **The window:** from that simultaneous failure to the first successful retry. While it lasts registryd holds the denial in memory and the restriction in the user's pending queue, retries the invalidation and the marker at every IPC operation (after its role check, before anything else the operation does), retries the restriction's own record whenever the user's slot is written or shut down, serves no `eligible` decision, refuses an orderly shutdown and reports `rollback_status.unverifiable_owed`. Once the ledger has failed, every anchor image the run writes carries the marker `unverifiable`.
- **How the window ends** depends on which write succeeds first:
  - the **invalidation or the `unverifiable` marker** (for a policy restriction, any anchor write, which carries both the policy and the marker): the next same-boot start finds an unverifiable run and **contains** (§3 table; the recovery ceremony of §10);
  - the restriction's **journal record** while the ledger and the anchor still cannot be written: the restriction survives **precisely**. The next same-boot start finds a valid-looking ledger and a `running` marker, rolls the one unanchored journal record forward (§4) and replays the ledger (`ledger_replayed`); the restriction stays in force and the user is **not** contained by this alone.
- **Recovery:** a reboot contains (the ledger is gone with the boot); an operator who sees `unverifiable_owed` or `restriction_not_durable` can force containment by rebooting before restarting the service. No other recovery path depends on this limit.
- **Why it is accepted, not closed:** no durable mark can be made while every write fails, so no mechanism inside registryd can close the window; an external witness (the supervisor, M2-007) or containment on every unclean same-boot restart were considered and not chosen. The window narrows further once the ledger has a preallocated reserve (stage B or M2-007), after which its appends cannot fail for space.

The owner chose this option (Option A of the M2-006 stage-A revision-3 handoff) on 2026-10-14, after the independent re-review `review/m2-006-a-r2-security` at 7d570c1 (M2-006-A-R2-03) found that the residual was not an owner-approved exception. The text was drafted by the implementer under that choice. The re-review `review/m2-006-a-r3-security` at f0d6eb0 (M2-006-A-R3-02) found that the first wording made every successful retry end in containment, which a journal-only retry does not; the corrected wording above distinguishes the two outcomes and the policy restrictions. The re-review `review/m2-006-a-r4-security` at 06a9423 found it technically accurate (its "policy restrictions" sentence read in E5's failed-ledger context), and the owner confirmed the corrected wording as written on 2026-10-14. No mechanism, record or format of this ADR changes; storage formats §7 records the limit. Implemented by M2-006 stage A revisions 3 and 4.

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

**Quarantine** denies every grant and key of that user (`quarantined`); no ceremony can open and `resolve_key` answers `quarantined` for every key. Exit only through `admin` `recover_user`: verify and accept the journal prefix, append a `recovery` record (temporary grants end, unconfirmed one-shots are consumed, every grant admitted before the recovery point is suspended), raise the user's minimum valid policy epoch and re-anchor. The user is then **contained**, never active: re-authorization is the §10 ceremony under either §9.2 option. If the anchor's `identity_seq` for the user exceeds the identity records in the accepted prefix (a lost tail held key-state changes, possibly an identity replacement or a compromise report), `recover_user` refuses: the only exit is `replace_identity` (new `user_id`, ADR 0005 §2). Recovery never infers validity, undoes consumption or claims complete history.

### 5. Identity and key freshness (R05)

- Identity records live in the anchored journal (§1), so restoring older identity state is a stale-journal/lost-store detection like any other rollback; no separate identityd store exists to restore.
- identityd rebuilds its index from registryd at start and answers `resolve_key` with the journal sequence of its snapshot and a **scope** (IPC spec): if registryd is unavailable or the snapshot is older than a restrictive change it knows of, the answer is `unknown` (deny); for a quarantined or deleted user every key is denied; for a contained user every key is denied **except** the narrowly scoped ceremony authority of §10 (revision 3, R13).
- Key invalidation, identity replacement and deletion are restrictive: in-memory denial and ledger entry first, then one durable journal record, then reply. Until durable, the key is denied everywhere (vaultd checks key status and scope with identityd before every signature, and signs nothing the scope does not permit).
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
2. **Who opens the recovery ceremony (rewritten in revision 3, R13).** Both options use the **complete ceremony of §10** unchanged — fresh recovery keys at a new key epoch, per-lineage freshly approved ceremony-bound restore/revocation commits, the same journal/anchor ordering, crash handling and permanent denial of every suspended grant and pre-ceremony key. They differ only in step 2. **Option C1 (recommended):** after containment the user (`trusted_client`, own UID) opens the ceremony; after quarantine an `admin` `recover_user` comes first, then the user opens it. **Option C2:** only `admin` opens the ceremony, after containment and after quarantine; the user then performs steps 3–5. C2 adds an administrator to every reboot after an unclean run; C1 relies on the passphrase-authenticated user alone (with the ADR 0005 §8 limits).

Mandatory denial, quarantine, ledger and ceremony rules are identical under every option.

**Owner decision (2026-10-12):** 9.1 **Option A** — `same_volume` degraded mode by default with visible status; `separate_volume` by administrator configuration; `tpm2_nv` later. 9.2 **Option C1** — after containment the user opens the ceremony; after quarantine an `admin` `recover_user` comes first, then the user opens it. Options B and C2 are not adopted.

### 10. Recovery ceremony after containment or quarantine (revision 3, R13)

**Separation.** Containment is a fact about **grants** (every grant admitted before the containment point is permanently ended as `suspended`). Key authority is evaluated separately with a **scope** (§5): while a user is contained, nothing signs except (a) the identity key for key invalidations (always permitted: they only narrow) and for the ceremony's own certificates, and (b) the ceremony's **named recovery persistent key**, only for trunk `restore`/`revocation` commits bound to the open ceremony. Every pre-ceremony contract key is denied throughout and permanently retired at step 3, so a contract key compromised in an unverifiable window — whose invalidation may have been lost with that window's ledger — can never sign recovery or later commits.

**Steps** (each record is one durable journal record followed by an anchor update before the reply, per §§1–2; each step is idempotent):

| Step | Actor / operation | Journal record | Checks |
|---|---|---|---|
| 1. Contain | registryd at start (§3) or `admin` `recover_user` after quarantine (§4) | `containment` / `recovery` | All pre-point grants suspended (terminal), live temporary branches end, unconfirmed one-shots consumed; per lineage the **contained trunk head** is recorded. Denial is in memory before anything is served. |
| 2. Open | `begin_recovery`: owner under C1, `admin` under C2 (§9.2) | `recovery_opened {recovery_id, key_epoch E, mode, role}` | User contained (not quarantined, not deletion-pending), no ceremony open, `E` greater than every key epoch and every earlier ceremony epoch of the user. |
| 3. Fresh keys | vaultd `recover_keys(recovery_id, passphrase)` (owner) → identityd `register_certificate` (×2) → registryd **`name_recovery_keys`** (caller role `service:vaultd` only; revision 4, R14) | two `key_certificate` records (identity-key-signed, epoch `E`, `supersedes_key_id`) then `recovery_keys {recovery_id, persistent_key_id, temporary_key_id}`, built by registryd from the operation's closed arguments | Identity key current: user not replaced, deleted or tombstoned; anchor `identity_seq` equals the journal's (else only `replace_identity`). New keys generated in locked memory, never existed before, their encrypted objects and certificates durable in the vault before registration. `name_recovery_keys` names exactly two certified, non-invalidated epoch-`E` keys of this user (persistent, temporary) for the open ceremony that has named none; once the record **and its anchor update** are durable it raises the user's **minimum signing key epoch** to `E` and **retires every other contract key**, including epoch-`E` keys of an abandoned attempt. No other role and no other operation can produce this record: `append_identity` refuses it, and no registryd operation appends caller-supplied records. |
| 4. Re-authorize, per lineage | contractd `prepare_proposal(recovery_id)` → fresh approval (passphrase, exact digest, ADR 0005 §3) → vaultd signs with the named key → `submit_signed` → `admit` | ordinary `commit_admitted` | Commit type `restore` or `revocation`, trunk position, parent = current trunk head, `restore_reference` = the **contained trunk head**, `authorization.session_id` = `recovery_id`, removes **every** suspended grant id of the lineage's state, adds only grants matching persistent grants of the contained head (wire §6.7) with new ids, current policy epoch (≥ the user's minimum valid epoch), network slot (wire §6.12); one recovery commit per lineage per ceremony. Anything else is `recovery_rule` (or the layer-6 / `restore_rule` code). New grants are pending until enforcement confirms them, like every admission. |
| 5. Close | `close_recovery` (owner or `admin`) | `recovery_closed {recovery_id}` | Requires step 3 complete. The user leaves containment; lineages not re-authorized keep their suspended grants (still ended) and need ordinary freshly approved commits later. The recovery keys become the user's current keys; pre-ceremony keys never sign again. |

**Old grants denied throughout.** Suspended ids are terminal from step 1 onward, before, during and after the ceremony, whatever its outcome; `grant_id` uniqueness (wire §6.6) forbids their reuse, so only new ids can carry re-authorized scope. Containment never resumes access from missing evidence.

**Crashes and failures** (registryd decides from durable records only; the ledger rules of §3 apply to every restrictive record):

- Before the `containment` record is durable: the next start still sees an unverifiable run and contains again (idempotent).
- After step 2: the ceremony stays open across clean or same-boot restarts and resumes at the first incomplete step.
- During step 3 (revision 4, R14): certificates without `recovery_keys` grant nothing. vaultd writes the key objects and certificates durably before registering them and names them last. A `recovery_keys` record that is durable but not yet anchored is the user's **pending tail**: it has no effect, registryd completes its anchor update before any later operation for the user (or fails that operation with `storage_unavailable`; nothing is admitted or served past the tail) and startup rolls it forward (§4, `m == a + 1`). After any failure, timeout or restart vaultd reconciles from `recovery_status` (IPC spec): an unavailable status → keep everything and retry; the held keys named → keep them (step 3 done); other keys named → erase the held objects; nothing named while the ceremony is open → resume naming the **held** keys (identical certificate re-registration and identical naming are idempotent: `already_named`, no second record); ceremony no longer open → erase. Because vaultd erases only on a positive answer, a durably named key is never destroyed, and because it resumes with held keys instead of generating new ones, a naming request still in flight from before a vaultd restart names the same keys. New keys are generated only when no held object is usable; once a naming record is durable and anchored, every unnamed key is retired.
- During step 4: each lineage's commit is atomic; lineages already re-authorized stay re-authorized (pending enforcement), the others stay suspended. A head or epoch change between approval and admission rejects the commit (`stale_head`/`stale_epoch`); the user re-approves.
- A new unverifiable run while a ceremony is open: a new containment suspends everything admitted before it, **including grants re-authorized in the open ceremony**, and aborts the ceremony; a new ceremony needs a key epoch above `E`.
- Deletion requested during a ceremony: `deletion_pending` denies every key and closes the ceremony (§7).
- Anchor write failure at any step: the step is reported failed; a durable-but-unanchored record follows the roll-forward rule of §4.

**Bypass prevention.** No role can regain **grant** authority without fresh commits: closing (step 5) after step 3 ends the key-scope restriction even if the user approved no lineage, but revives no suspended grant — every lineage not re-authorized in step 4 stays ended until an ordinary freshly approved commit (wording corrected in revision 4 after the re-review's reading of revision 3). Only vaultd names ceremony keys, only through `name_recovery_keys`, and only exactly two epoch-`E` keys certified by the current identity key for the open ceremony; admin never signs; `fixture_consumer` cannot call ceremony operations; a pre-ceremony key, a key of an abandoned attempt, a compromised recovery key, a commit not bound to the `recovery_id`, a revision/merge/temporary grant/identity transition, a restore that keeps a suspended id, references an older state or adds scope that existed only on a temporary branch are all rejected. Restoring an older vault key file confers nothing: its key is below the minimum epoch.

**Published vectors** (`journal_scenarios`, oracle model of these rules): `recovery_c1_complete` (positive, with bypass, replay, stale-key and old-id negatives after close), `recovery_rejects_stale_key`, `recovery_rejects_compromised_key`, `recovery_rejects_abandoned_key`, `recovery_rejects_deleted_user`, `recovery_rejects_bypass`, `recovery_c2_requires_admin`, `recovery_aborted_by_new_containment`, `recovery_quarantine_requires_admin_first`; revision 4 (R14): `recovery_naming_interface` (every non-vaultd role denied, closed arguments, wrong user/ceremony/class/epoch/uncertified keys rejected, idempotent retry, one naming per ceremony, status after close), `recovery_naming_anchor_failure_restart` and `recovery_naming_anchor_failure_retry` (durable unanchored tail without effect, nothing served past it, startup roll-forward, reconciliation without a second record). Every scenario step now passes through the oracle's registry interface model (`registry_service_t`: caller role, closed arguments, registryd-built records, journal → anchor → reply); the step-3 naming record has no other path into the model. The oracle models admission, key-authority and interface decisions only; vault key handling, passphrase approval, real IPC, anchor writes and crash points are NOT_RUN until M2-004/M2-006.

## Alternatives considered

- **SQLite (WAL, synchronous=FULL):** mature engine; rejected for M2 because it adds a large C FFI dependency to the most security-critical store and coarser fault injection.
- **Separate identityd store with its own anchor entry:** keeps storage with the owning service; rejected because a cross-service write ordering would be needed for every invalidation and restore, which is exactly the gap R05 found. identityd keeps ownership of validation and policy, not of bytes.
- **Accepting the double-failure residual (revision 1):** rejected by review R04; contradicts ADR 0001's rule that a failed revocation never justifies continuing access.
- **Containment after every unclean shutdown without a ledger:** simpler, but makes every service crash within a boot cost a user reconfirmation; the tmpfs ledger limits containment to reboots and genuinely unverifiable runs.
- **Recovery signed by the pre-containment contract keys (revision 2's implicit path):** cannot run, because containment correctly denies those keys (R13), and would be unsafe if it could: an invalidation of a compromised contract key may have been lost with the unverifiable window. Rejected in favor of fresh ceremony keys at a new epoch.
- **Lifting containment by un-suspending grants after an admin attestation:** no fresh user signature and no new ids; contradicts ADR 0001 (no restoration from lifecycle evidence). Rejected.
- **A generic "recovery mode" that re-enables all signing until the user is done:** would let any signature (not just ceremony-bound re-authorization) through during the window; rejected for the step-scoped key authority of §10.
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

All planned; nothing has run except the oracle vectors of §10 and the network-slot/placement scenarios (model evidence, not service evidence). Atomic admission under concurrent writers; the §10 ceremony at every step with crashes and anchor failures, `name_recovery_keys` over real IPC from every role (only vaultd accepted) with journal/anchor faults, lost replies, vaultd and registryd restarts at each point of the step-3 order and a naming request in flight across a vaultd restart, abandoned key generation, a new unclean run mid-ceremony, deletion mid-ceremony and every bypass case, with vault signing refused outside the scope; every fault point in §6; the review's case (journal and anchor unwritable, restart after recovery) in both same-boot and new-boot variants; ledger missing/invalid; run marker unwritable; concurrent users and tombstones racing the anchor; independent restore of each storage domain (journal only, anchor only, vault files only, all but the independent anchor); key invalidation crash points; deletion crash at every step including tombstone write failure; `separate_volume` detection on a distinct filesystem; `same_volume` reporting degraded; quota/reserve. Release binaries under subprocess fault injection.

## Approval and supersession

- **Independent review:** revision 1 CHANGES_REQUESTED at 9cf8825 (R04, R05, R08, R10, R11 via G6, I01). Revision 2 CHANGES_REQUESTED at 336f13b (R13; R04, R05, R10, R11 and I01 passed at design level). Revision 3 CHANGES_REQUESTED at f451f2e (R14; the R13 state machine passed at model level, R04, R05, R10, R11 and I01 intact). Revision 4 APPROVE for owner consideration at 3a3b5d1 (R13, R14 PASS; Low note I04 answered in revision 4.1); revision 4.1 (candidate 28a8595e627bcd092acaff4b8161522cf5d38f7f, test/oracle-only; docs-only ed2ba0d, 87c9cbc) **APPROVE** at bd081df26bc31636b01bf470bd9b1fea9be7d20d on `review/m2-001-002-r5-security` (`docs/reviews/M2-001-002-codex-security-rereview-r5.md`, SHA-256 `77ec6d5f2c0c86102304b2227830b21f664fd78e6518cde3af502468bdb178af`, verified; parent 9cf8825, adds only that file).
- **Owner approval evidence:** On 2026-10-12, after the independent revision-5 re-review recommended APPROVE for M2-001 and every gate G1–G6, the project owner (requesting user) answered the concrete approval question for G1–G6 with **“Approve all six”**, and chose §9.1 **Option A** (same_volume degraded default) and §9.2 **Option C1** (user opens the ceremony; admin `recover_user` first after quarantine) — the recommended options, selected from both options of each choice.
- **Errata (owner, 2026-10-14):** **E5** — the limit of §3 under a simultaneous failure of the ledger append, both validity invalidations, the anchor and the journal, followed by a process death before any retry succeeds and a same-boot restart (M2-006-A-R2-03). The owner answered the explicit question with the recommended Option A: record the limit and its consequences (§3); the alternatives, an external supervisor witness and containment on every unclean same-boot restart, were not chosen. No mechanism, record or format change. Wording corrected after M2-006-A-R3-02 (a successful invalidation or marker contains; a journal-only retry preserves the restriction precisely without containing); the owner confirmed the corrected wording as written on 2026-10-14, after the re-review at 06a9423.
- **Supersedes / is superseded by:** revision 4 supersedes revision 3 (commit 84fa6ef172b3fad21bb079549776252bb65624d6), which superseded revision 2 (commit a2bbb370c867da57fdc72274fb17ca19a23426e4), which superseded revision 1 (commit f1437f0d254af2968c3598bf14d1a9b2b8e791cd).
