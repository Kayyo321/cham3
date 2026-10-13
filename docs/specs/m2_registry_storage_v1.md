# M2 registryd storage formats v1 (journal, anchor, restriction ledger)

- **Status:** implementation record of M2-005 and M2-006 stage A (revision 2, after the independent stage-A review M2-006-A-R01–R08), written by the implementer; the architecture is [ADR 0006](../decisions/0006_m2_storage_anchor_recovery.md) (owner-approved), which controls where this document and the ADR disagree. Byte layouts that the ADR leaves open (the journal header, the anchor's version field and `unverifiable` marker state, the ledger's files) are author placements open to review.
- **Scope:** the files registryd writes and reads, their byte layouts, the invariants replay and recovery enforce, and the restrictive write order. The IPC vocabulary is [service IPC v1](m2_service_ipc_v1.md); the record payloads' signed objects are [wire format v1](m2_wire_format_v1.md).
- **Implementation:** `core/registryd/src/{record,journal,anchor,ledger,registry}.rs`. Every format is little more than a framed byte string; nothing here is a signed object.

## 1. Directory layout

| Path (defaults) | Owner / mode | Content |
|---|---|---|
| `<root>/` | registryd, `0700` | The store root (`/var/lib/cham3/registry` in deployment). Its entry is synced in its parent at every start. |
| `<root>/users/<uid>/journal.c3j` | `0600` | The user's append-only journal (§2). |
| `<root>/users/<uid>/reserve` | `0600` | The preallocated 1 MiB restrictive reserve of written zeros (ADR 0006 §1; M2-005 R03). |
| `<root>/anchor/slot_a`, `slot_b` | `0700` / `0600` | The two alternating anchor slots (§3). `registry_config_t::anchor_path` moves the directory; on another filesystem (`st_dev` differs from the root's) the anchor is `independent`, otherwise `degraded` (owner choice §9.1). |
| `<root>/ledger/validity`, `entries` | `0700` / `0600` | The restriction ledger of the current run (§4). `registry_config_t::ledger_path` moves it; deployment puts it on a tmpfs (`/run/cham3/registryd/restriction_ledger`, M2-007) so that it vanishes with the boot. |

Every path is checked before use: a regular file (directory) of the owning UID with no group or other bits, never a symbolic link; a slot or ledger file over its size limit is invalid. A failing check refuses the store (`unsafe_path`), never repairs it. Every directory registryd creates is synced in its parent at every start and on every reopen, never inferred durable from its existence (M2-005 R01, R2-01).

## 2. Journal (ADR 0006 §1)

- **Header** (42 bytes): `"C3JH" ‖ u16_be(version = 1) ‖ u32_be(uid) ‖ user_id (32)`.
- **Frame:** `"C3JR" ‖ u32_be(length) ‖ record ‖ chain (32)`; `length ≤ 65 536`.
- **Chain:** `chain_n = domain_hash("cham3/journal-chain/v1", chain_{n−1} ‖ u64_be(n) ‖ record)` for the record at 0-based position `n`, `chain_{−1} = domain_hash("cham3/journal-genesis/v1", user_id ‖ u32_be(uid))`.
- **Record:** one closed `cham3_dcbor_v1` map `{ "kind", "seq", "time", … }` of at most 65 536 bytes embedding at most one signed object. `seq` is the 1-based sequence `n + 1`; `time` is the registry's UTC text. Kinds: `identity` (a validated identity object: enrollment, certificate, invalidation), `commit_admitted`, the narrowing lifecycle records `expired`, `session_ended`, `restriction`, and the registry-written user-state records `containment`, `quarantine`, `admin_recover`, `recovery_opened`, `recovery_closed`, `deletion_pending`. Any other kind, any extra or missing field, fails replay.
- **Parse of a file at start** (`parse_journal_prefix`): frames are verified in order (magic, length bound, chain). A complete frame that fails is a **defect** (the user is unusable: `storage_unavailable`). An incomplete trailing frame — fewer bytes than its declared, in-bounds length — is a **torn tail**; a declared length above the bound is a defect, never a torn tail. The valid prefix length and the torn flag feed the recovery table (§5).
- **Append** (M2-006 stage A): a failed write (before any byte, after some bytes, or at its `fdatasync`) is rolled back by truncating the file to its last durable length; the caller gets `storage_unavailable`, the in-memory state is unchanged and the user continues. Only a failed rollback marks the journal failed, which denies the user until the next start (the bytes' state is unknown). A restrictive record that meets `ENOSPC` is written from the reserve (truncate, release the reserve, rewrite, `fdatasync`); the reserve is refilled by the next non-restrictive append. **Nothing is appended beyond an unanchored record** (§3): while the journal's last record is durable but not anchored, a restrictive record stays pending — in memory and in the ledger — until that record is anchored, so that a start finds at most one unanchored record (§5 rolls it forward).

## 3. Anchor (ADR 0006 §2)

Two slots, `slot_a` and `slot_b`; every update writes the slot that does not hold the current counter (`replace_file`: truncate, write, `fsync`; the directory is synced after the first write to each slot in a run). Startup reads both and takes the valid slot with the higher counter; an invalid slot is ignored while the other is valid; two invalid slots (or none while journals exist) quarantine every user (§5).

Slot bytes (big-endian), followed by the SHA-256 of everything before it:

```text
"C3AN" ‖ u16 version (1) ‖ u64 counter
‖ run marker: u8 0 (none) | u8 1 (running) | u8 2 (clean) | u8 3 (unverifiable) ‖ run_id (16) ‖ u8 boot_id_length (1..=64) ‖ boot_id
‖ policy: u64 epoch ‖ u64 min_valid_epoch ‖ u8 emergency_restriction (0/1)
‖ u32 user_count ‖ per user: u32 uid ‖ u64 seq ‖ chain (32) ‖ user_id (32) ‖ u8 state (0 normal, 1 contained, 2 quarantined) ‖ u64 identity_seq ‖ u64 min_valid_policy_epoch
‖ u32 tombstone_count ‖ per tombstone: user_id (32) ‖ u32 uid ‖ u8 state (1 pending, 2 deleted)
‖ sha256 (32)
```

Users and tombstones are sorted by key (the map is a `BTreeMap`); duplicates, unknown enumerators, trailing bytes and a checksum mismatch make the slot invalid. A slot is at most 64 MiB. The writer alternates the slots with strictly increasing counters, so two valid slots with the same counter never come from it: neither is trusted and the anchor is invalid (§5, `anchor_invalid` for every user).

The in-memory map is authoritative while registryd runs; disk is read only at start. The map holds, per user, the sequence and chain of the last record whose reply was acknowledged (`seq`, `chain`), the user's state, the user id, the number of identity records up to `seq` (`identity_seq`, so a lost tail that held key-state changes is visible, ADR 0006 §§5, 10; at start it must equal the identity records among the first `seq` records of the journal, else the entry does not describe the journal and the user is quarantined, §5 `identity_mismatch`) and the per-user minimum valid policy epoch raised by `recover_user` (stage C). It also holds the **run marker** (§4), the **policy state** (`set_policy_epoch`, `set_emergency_restriction`, raised at start by the configuration and never lowered) and the deletion **tombstones** (stage C writes them). Every slot write — a user's anchor update, the run marker, the shutdown — publishes the policy state in force, so a policy change whose own slot write failed becomes durable with the next successful one; the policy lock is held across every slot write, so no older copy of the policy state overwrites a newer publication.

A user whose anchor entry says `quarantined` or `contained` starts in that state whatever the journal's records say: every grant of a live state ends (`quarantined` / `suspended`) and every branch ends, exactly as the `quarantine` / `containment` record ends them when it is replayed.

## 4. Run marker and restriction ledger (ADR 0006 §3)

- **Run marker** (in the anchor): `{run_id, boot_id, state}`. Written `running` after replay and recovery, before anything is served (a failed write fails the start); written `clean` by `shutdown` once everything owed is durable — every pending restrictive write, every unanchored record, the policy state in force (published in the same slot write) — and the ledger has not failed; written `unverifiable` when the ledger fails (below), and never replaced by `clean` afterwards. The boot id is `/proc/sys/kernel/random/boot_id` or `registry_config_t::boot_id`.
- **Ledger files:** `validity` = `"C3RV" ‖ run_id (16) ‖ boot_id` (written once at start, after the old one is removed and the entries file emptied — the entries file first, so a validity file without an entries file is no complete ledger and is invalid); `entries` = a sequence of frames `"C3RL" ‖ u32 body_length ‖ body ‖ sha256(body)` with `body = run_id (16) ‖ u8 kind (1 journal_record, 2 policy) ‖ u32 uid ‖ u32 payload_length ‖ payload`. A `journal_record` payload is the encoded record the restrictive path intended to write; a `policy` payload is `u64 epoch ‖ u64 min_valid_epoch ‖ u8 emergency`. Entries are appended with `fdatasync`; the file is at most 64 MiB and an entry at most 70 000 bytes. A release of the emergency restriction is permissive and is not ledgered.
- **Ledger failure:** when an append fails, the ledger is marked failed, its validity file is removed (emptied if it cannot be removed: an empty validity file is invalid) and the run marker is written `unverifiable` (so that the next start contains whatever the ledger files hold, even when the validity file could not be removed). For the rest of the run no eligible decision is served (`lineage_head`, `admit`, every slot operation: `storage_unavailable`); restrictive changes still reach the journal; `shutdown` refuses (`restriction_not_durable`) and the next start sees an unverifiable run.
- **Start decision** (before anything is served):

| Marker | Previous ledger | Decision |
|---|---|---|
| none or `clean` | any | `resumed`; the ledger is replaced |
| `unverifiable` | any | `contained` |
| `running`, marker boot = this boot | `validity` valid for the marker's run id and this boot, entries file present and valid | `ledger_replayed`: every `journal_record` entry whose record the journal does not hold is re-journaled through the restrictive path — its in-memory effect applied as the ledgering operation applied it, then this run's ledger first (so that the intent survives a further unclean restart in this boot), then the journal and the anchor; an entry for a user the store does not know is dropped, a quarantined user's journal is left as found; `policy` entries raise the policy state |
| `running`, same boot | missing, invalid, entries file missing, or another run's | `contained` |
| `running`, another boot | — | `contained` |

- **Containment:** every usable user is contained: all live grants end `suspended` (permanently), temporary branches end, the slot state is `contained`, a `containment` record goes through the restrictive path (reserve-backed) and the anchor records `contained`. Until the record is durable the denial holds in memory and `rollback_status` reports the user pending. A contained user admits nothing (`contained`), every key is denied (identityd's scope), and only the recovery ceremony of ADR 0006 §10 (stage C) lifts it.
- **Restrictive write path** (`restrict`, key invalidation, containment, `set_policy_epoch`, `set_emergency_restriction(true)`): in-memory denial → ledger entry → journal record (reserve-backed) → anchor update → reply. A journal or anchor failure leaves the write pending: the caller gets `restriction_not_durable`, admissions of the user wait (`restriction_not_durable`), every later operation of the user retries what the slot owes in order — the anchor update of an unanchored record first, then the pending records — and `shutdown` refuses. An anchor failure after a durable record denies the user, reads included (`lineage_head`, `identity_state`: `storage_unavailable`, after a retry of the anchor update), until that record is anchored by a retry or by the next start, which rolls the anchor forward (§5); a restrictive change requested meanwhile is denied in memory, ledgered and kept pending — never dropped, never appended beyond the unanchored record. A global policy change whose anchor update fails stays in force and pending (`rollback_status.policy_pending`) and is published by the next successful slot write of any user and by `shutdown`.
- **Permissive policy change** (`set_emergency_restriction(false)`): anchor update first, in force only once it is durable; a failed update changes nothing (`restriction_not_durable`). Releasing restores no grant by itself.

## 5. Per-user recovery at start (ADR 0006 §4)

With `m` the number of chain-verified records (after a torn tail beyond the anchor is truncated) and `a` the anchored sequence:

| Observation | Result | `quarantine_reason_t` / state |
|---|---|---|
| `m == a`, chain at `a` matches | resume | — |
| `m == a + 1`, chain extends | resume; the anchor is rolled forward | — |
| torn tail beyond `a` | the torn bytes are truncated; then the rows above | — |
| torn tail inside the anchored range, or `m < a` | quarantine; the file is kept | `stale_journal` |
| `m > a + 1` | quarantine | `unexplained_records` |
| chain at `a` differs, or the user id differs | quarantine | `chain_mismatch` |
| a record fails replay | quarantine | `replay_defect` |
| journal missing (or the user directory gone) while the anchor knows the user | quarantine | `lost_store` |
| the anchor's `identity_seq` differs from the identity records among the first `a` | quarantine | `identity_mismatch` |
| both slots invalid, two valid slots with the same counter, or no anchor while journals exist | quarantine every user; a fresh anchor is written for this run | `anchor_invalid` |
| tombstone `pending` for the journal's user id | quarantine | `deletion_pending` |
| tombstone `deleted` for the journal's user id | quarantine (removal is stage C) | `deleted_user_files` |
| the anchor says `quarantined` / `contained` | that state is authoritative over the records'; every grant of a live state ends (`quarantined` / `suspended`) and every branch ends, as the record would end them | `anchored` / contained |
| a complete frame fails its chain, a cut header, a frame longer than any record | unusable (a store defect, not a quarantine) | `storage_unavailable` |

A quarantined user with a readable journal keeps a slot in the quarantined state (the verified prefix that `recover_user` of stage C accepts): `lineage_head` reports `user_state: quarantined` with every grant ended `quarantined`; `identity_state`, `admit`, `append_identity` and `restrict` answer `quarantined`; identityd, told `quarantined` instead of records, denies every key of the user (`key_not_certified` from a fresh index, `quarantined` once it knows the user). A user quarantined from the bytes or from a disagreeing anchor entry (stale, unexplained, chain mismatch, identity mismatch, replay defect, lost store) has no slot and every operation answers the reason's code (`quarantined`, or `storage_unavailable` for a store defect). `rollback_status` lists quarantined users with their reason under `quarantined`, with or without a slot, and store defects under `unusable`.

## 6. Status

`rollback_status` (service IPC §3): `anchor_mode` (`independent`/`degraded`), `run_decision` (`resumed`/`ledger_replayed`/`contained`), `quarantined` (uid, recovery-table reason), `unusable` (uid, store reason: a defect or an unreadable file, denied but not quarantined), `contained` (uids), `open_ceremonies` (stage C), `pending_restrictions` (users with a restrictive write not yet durable or an unanchored record), `ledger_failed`, `depleted_reserves` (users whose reserve is below 1 MiB; reported here until the status operations of stage B), `policy` (the state in force: epoch, minimum valid epoch, emergency restriction) and `policy_pending` (the state in force is not yet in the anchor). policyd's `policy_status` reports the policy state in force and the anchor mode as its rollback-protection mode.

## 7. Limits (stage A)

- The anchor has no reserve: on a full filesystem a restrictive record is durable in the journal (reserve-backed) while its anchor update may fail; the change is then pending (`restriction_not_durable`, denial in force, retried by every later operation of the user, reads included) and the next start rolls the anchor forward; a second restrictive change meanwhile is ledgered and pending, not journaled. The full-filesystem probe (`cham3_reserve_probe full-disk`) shows the journal and the ledger on a full tmpfs; the anchor's behaviour there depends on the slot size and is not probed.
- A ledger failure whose validity file can neither be removed nor emptied, while the anchor cannot be written either (no `unverifiable` marker), leaves the ledger files looking complete: a same-boot restart could then replay an incomplete ledger. Every restrictive record written after the failure is still durable in the journal; only a restriction whose journal write failed as well, in that triple failure, is lost to a same-boot restart (a reboot contains).
- A whole-volume restore that includes a `degraded` anchor is not detected (ADR 0006 §2); an `independent` anchor detects it as `stale_journal`.
- Power loss, kernel crashes and process death are simulated by faults at the write boundaries and by restarts with another boot id; no real crash test exists (A08/A10 remain NOT_RUN).
- The `recovery` exit of quarantine, tombstone creation, the ceremony records and `history` are stages B–C; the ledger's tmpfs placement is M2-007.
