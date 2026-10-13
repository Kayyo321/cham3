//! registryd: the per-user journals, the global anchor, the restriction ledger, their
//! derived indexes and the operations of service IPC §3 that M2-005 and M2-006 stage A
//! implement (`append_identity`, `identity_state`, `admit`, `lineage_head`,
//! `restrict`, `rollback_status`, `set_policy_epoch`, `set_emergency_restriction`).
//!
//! **One snapshot.** A user's identity records and commits live in one journal (ADR
//! 0006 §1). The slot of a user holds that journal, the key-authority index rebuilt
//! from its identity records (identityd's own code, so layer 6 at admission reads the
//! same snapshot as the commit) and the ECR index. Admission re-verifies layers 1–6
//! against that snapshot, then layer 7, then the policy epoch and quotas, all under
//! the user's lock immediately before the append; the indexes change only after the
//! record is durable.
//!
//! **Anchor (ADR 0006 §2).** Every durable append is followed by an anchor update
//! naming the user's new sequence and chain before the caller is answered; every
//! anchor update also publishes the policy state in force. A failed anchor update
//! after a durable admission reports the admission failed and denies the user —
//! writes and reads alike, so that nothing of the unanchored state is served — until
//! that record is anchored: by the retry every later operation of the user makes
//! first, or by the roll-forward of the next start (§4). Nothing is appended beyond
//! an unanchored record.
//!
//! **Restrictive write path (ADR 0006 §3).** A restriction (a lifecycle record, a key
//! invalidation, containment, a policy raise) is denied in memory first, then written
//! to the restriction ledger, then to the journal (reserve-backed), then anchored. If
//! the journal or anchor write fails the denial stays, the change is kept pending,
//! the caller gets `restriction_not_durable`, non-restrictive appends of that user are
//! refused and every later operation retries the pending writes first.
//!
//! **Start (ADR 0006 §§3–4).** The previous run marker and ledger decide: a clean
//! marker resumes; a `running` marker of this boot with a valid ledger re-journals the
//! ledger's entries and resumes (`ledger_replayed`); a `running` marker without a
//! usable ledger, or of another boot, is an unverifiable run: every user is contained.
//! Per user, the journal is compared with the anchor: a torn frame beyond the anchor is
//! truncated; one unanchored complete record rolls the anchor forward; a stale journal,
//! unexplained records, a chain mismatch, a lost store, invalid anchor slots, a deleted
//! user's files or a replay defect quarantine the user. The run marker `running` is
//! written before anything is served; `shutdown` writes `clean` once every pending
//! change is durable.
//!
//! **Locks.** `users` (the map of slots) is held only to find or create a slot; a
//! user lock is never taken while `users` is held, except for a slot created under
//! it. Lock order: `users` → user → `policy` → `ledger` → `anchor`. The `policy`
//! lock is held across a policy publication (its ledger entry and anchor update) and
//! across every slot's anchor update, so that no older copy of the policy state can
//! overwrite a newer publication; `ledger` is released before `anchor` is taken.
//!
//! Limits (stage A): in-process composition; the socket transport and process
//! boundary are M2-007. Registry time is the system clock; ADR 0007's trusted-time
//! clock, the lifecycle writers, one-shot consumption and `history` are stage B; the
//! recovery ceremony, `recover_user`, `replace_identity` and deletion cleanup are
//! stage C (a `pending` tombstone keeps its user denied; a `deleted` tombstone's files
//! are quarantined and never loaded, not yet removed).

use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use cham3_contract_format::hash::digest_t;
use cham3_contract_format::schema::{
    commit_payload_t, contract_state_t, invalidation_reason_t, random_id_t, subject_t,
};
use cham3_contract_format::{key_resolver_t, object_kind_t, payload_t, verify_object};
use cham3_identityd::caller::{caller_role_t, service_name_t};
use cham3_identityd::directory::{identity_directory_t, identity_effect_t, journal_record_t};
use cham3_identityd::index::{
    check_commit_authority, check_signing_window, user_record_t, user_state_t, utc_text,
};
use cham3_identityd::journal::{
    MaxPageRecords, append_receipt_t, identity_journal_t, journal_entry_t, journal_page_t,
};
use cham3_identityd::outcome::{
    authority, authority_code_t, ipc, ipc_code_t, service_error_t, service_result_t,
};

use crate::anchor::{
    anchor_map_t, anchor_mode_t, anchor_start_t, anchor_t, anchored_state_t, anchored_user_t,
    policy_state_t, run_marker_t, run_state_t, tombstone_state_t,
};
use crate::journal::{journal_file_t, journal_io_t, journal_store_t, store_error_t};
use crate::ledger::{
    boot_id_source_t, entry_kind_t, kernel_boot_id_t, ledger_entry_t, ledger_t,
    parse_policy_payload, policy_payload, previous_ledger_t,
};
use crate::lineage::{admission_t, end_reason_t, lineage_index_t, standing_t};
use crate::record::{
    journal_defect_t, lifecycle_record_t, parse_journal_prefix, placement_t, record_body_t,
    registry_record_t,
};

/// Default per-user journal quota (ADR 0006 §1): 64 MiB, configurable 16 MiB–1 GiB.
pub const DefaultQuotaBytes: u64 = 64 * 1024 * 1024;
pub const MinQuotaBytes: u64 = 16 * 1024 * 1024;
pub const MaxQuotaBytes: u64 = 1024 * 1024 * 1024;
/// Reserve kept for restrictive records only (ADR 0006 §1): the quota's logical
/// reserve here, and the preallocated reserve file of the same size in the store
/// (`journal.rs`, R03).
pub const ReserveBytes: u64 = crate::journal::ReserveBytes;
/// Most lineages per user (ADR 0006 §1).
pub const MaxLineages: usize = 4096;
/// Directory names under the store root used when no other path is configured.
pub const DefaultAnchorDirectory: &str = "anchor";
pub const DefaultLedgerDirectory: &str = "ledger";

/// Registry configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct registry_config_t {
    pub quota_bytes: u64,
    pub max_lineages: usize,
    /// The policy epoch at genesis, and a raise at start: the persisted epoch (anchor)
    /// never goes down. A commit's `policy.epoch` must equal the current epoch at
    /// admission (`stale_epoch`).
    pub policy_epoch: u64,
    /// Grants admitted under a lower epoch are no longer eligible (ADR 0007 §1 rule
    /// 4); likewise a floor that only rises.
    pub min_valid_policy_epoch: u64,
    /// Anchor directory (ADR 0006 §2). `None`: `<root>/anchor`, degraded mode. A path
    /// on another filesystem makes the anchor independent (owner choice §9.1).
    pub anchor_path: Option<PathBuf>,
    /// Restriction-ledger directory (ADR 0006 §3; tmpfs in deployment). `None`:
    /// `<root>/ledger` — a development default that does not vanish with the boot.
    pub ledger_path: Option<PathBuf>,
    /// The boot identifier to use instead of the kernel's (tests simulate reboots).
    pub boot_id: Option<String>,
}

impl Default for registry_config_t {
    fn default() -> registry_config_t {
        registry_config_t {
            quota_bytes: DefaultQuotaBytes,
            max_lineages: MaxLineages,
            policy_epoch: 1,
            min_valid_policy_epoch: 1,
            anchor_path: None,
            ledger_path: None,
            boot_id: None,
        }
    }
}

/// Why a configuration or start was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum registry_start_error_t {
    /// Quota outside 16 MiB–1 GiB, lineages outside 1..=4096 or an epoch order error.
    invalid_config,
    /// The store root, its `users` directory, the anchor or the ledger is unusable, or
    /// the run marker could not be made durable.
    store(store_error_t),
    /// No boot identifier is available.
    boot_id_unavailable,
}

/// Registry time (seconds since 1970-01-01T00:00:00Z).
pub trait registry_clock_t: Send + Sync {
    fn utc_seconds(&self) -> i64;
}

/// The system clock (ADR 0007's offset rule is stage B).
#[derive(Clone, Copy, Debug, Default)]
pub struct system_registry_clock_t;

impl registry_clock_t for system_registry_clock_t {
    fn utc_seconds(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|elapsed| i64::try_from(elapsed.as_secs()).ok())
            .unwrap_or(0)
    }
}

/// Why a user is quarantined (ADR 0006 §4): the row of the decision table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum quarantine_reason_t {
    /// The journal holds fewer records than the anchor names.
    stale_journal,
    /// The journal holds more than one record beyond the anchor.
    unexplained_records,
    /// The chain at the anchored sequence differs from the anchored chain.
    chain_mismatch,
    /// The anchor knows the user but no journal exists.
    lost_store,
    /// No valid anchor slot while journals exist.
    anchor_invalid,
    /// A record fails replay verification.
    replay_defect,
    /// Files of a `deleted` user (tombstone) exist.
    deleted_user_files,
    /// A `pending` tombstone: the user stays denied until deletion completes.
    deletion_pending,
    /// The anchor recorded the user as quarantined in an earlier run.
    anchored,
    /// The anchor's identity count for the user disagrees with the identity records
    /// of the anchored prefix: the entry does not describe this journal.
    identity_mismatch,
}

/// Why a user's journal is unusable in this run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum unusable_reason_t {
    /// The file could not be read safely.
    store(store_error_t),
    /// Header, frame or chain defect.
    defect(journal_defect_t),
    /// A record does not decode, has the wrong sequence or fails re-verification.
    replay { sequence: u64 },
    /// The decision table of ADR 0006 §4 quarantined the user.
    quarantine(quarantine_reason_t),
}

/// What the previous run marker and ledger decided at start (ADR 0006 §3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum run_decision_t {
    /// A clean marker, or a store without one: nothing to recover.
    resumed,
    /// A `running` marker of this boot with a valid ledger: its entries were
    /// re-journaled where the journals lacked them.
    ledger_replayed,
    /// An unverifiable run: every user was contained.
    contained,
}

impl run_decision_t {
    pub fn as_str(self) -> &'static str {
        match self {
            run_decision_t::resumed => "resumed",
            run_decision_t::ledger_replayed => "ledger_replayed",
            run_decision_t::contained => "contained",
        }
    }
}

/// A restrictive change whose journal record or anchor update is not yet durable.
#[derive(Clone, Debug, PartialEq, Eq)]
struct pending_write_t {
    time: String,
    body: record_body_t,
}

/// A user's state in this run (the anchor's, plus this run's containment).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum slot_state_t {
    normal,
    contained,
    quarantined(quarantine_reason_t),
}

/// One user's journal and derived indexes.
#[derive(Debug)]
pub struct user_slot_t {
    uid: u32,
    user_id: digest_t,
    journal: journal_file_t,
    directory: identity_directory_t,
    lineages: lineage_index_t,
    identity_entries: Vec<journal_entry_t>,
    state: slot_state_t,
    /// Identity records in the journal (the anchor's `identity_seq`).
    identity_count: u64,
    /// Every lifecycle record the journal holds (replayed or written durably), so
    /// that a ledger entry the journal already holds is never re-journaled.
    lifecycle_records: Vec<lifecycle_record_t>,
    /// Restrictive records denied in memory whose durable write is still owed.
    pending: Vec<pending_write_t>,
    /// The journal's last record is durable but its anchor update failed: the user is
    /// denied, reads included, and nothing is appended, until a retry anchors it (or
    /// the next start rolls the anchor forward).
    anchor_failed: bool,
}

enum user_entry_t {
    usable(Arc<Mutex<user_slot_t>>),
    unusable(unusable_reason_t),
}

/// Receipt of an admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct admission_receipt_t {
    pub commit_id: digest_t,
    pub journal_seq: u64,
    pub placement: placement_t,
    /// Always `pending` in M2: enforcement confirmation is separate (ADR 0006 §1).
    pub status: crate::lineage::commit_status_t,
}

/// A live head and its state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct head_view_t {
    pub commit_id: digest_t,
    pub placement: placement_t,
    pub state: contract_state_t,
}

/// The commit that added a grant, and its signing key's state in the same snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct grant_origin_t {
    pub commit_id: digest_t,
    pub key_id: digest_t,
    pub policy_epoch: u64,
    /// The signing key's invalidation in this journal, if any.
    pub key_invalidated: Option<invalidation_reason_t>,
    /// The key is unknown to this snapshot (never for an admitted commit).
    pub key_unknown: bool,
}

/// `lineage_head`: one lineage from one consistent snapshot of the user's journal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct lineage_view_t {
    pub owner_uid: u32,
    pub owner_user_id: digest_t,
    pub user_state: user_state_t,
    /// Journal sequence of the snapshot.
    pub journal_seq: u64,
    pub lineage_id: digest_t,
    pub trunk_head: Option<head_view_t>,
    pub temporary_heads: Vec<head_view_t>,
    pub authoritative_subject: Option<subject_t>,
    /// Origin of every grant in a live state.
    pub origins: BTreeMap<random_id_t, grant_origin_t>,
    /// Lifecycle records that ended grants of a live state.
    pub ended: BTreeMap<random_id_t, end_reason_t>,
    /// Sessions that bind grants of a live state and that a durable `session_ended`
    /// record ended (ADR 0007 §6; M2-005 review R02). Admission refuses such grants,
    /// so this is empty unless the index and the records disagree; evaluation denies
    /// them regardless of any liveness attestation.
    pub ended_sessions: BTreeSet<random_id_t>,
    /// Network grants occupying the lineage's slot (wire format §6.12).
    pub occupying_network: Vec<random_id_t>,
    pub policy_epoch: u64,
    pub min_valid_policy_epoch: u64,
    /// The emergency restriction of everyone (ADR 0007 §1 rule 1, persisted).
    pub emergency_restriction: bool,
}

/// `rollback_status` (service IPC §3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct rollback_status_t {
    pub anchor_mode: anchor_mode_t,
    pub run_decision: run_decision_t,
    /// Quarantined users and the row of the recovery table that quarantined them
    /// (ADR 0006 §4), with or without a usable slot.
    pub quarantined: Vec<(u32, quarantine_reason_t)>,
    /// Users whose journal cannot be used for another reason (a store defect, an
    /// unreadable file): denied, not quarantined.
    pub unusable: Vec<(u32, unusable_reason_t)>,
    pub contained: Vec<u32>,
    /// Ceremonies are stage C: always empty here.
    pub open_ceremonies: Vec<u32>,
    /// Users with a restrictive change not yet durable.
    pub pending_restrictions: Vec<u32>,
    /// The ledger can no longer be written: no eligible decision is served.
    pub ledger_failed: bool,
    /// Users whose restrictive reserve is not full (R03; reported here until the
    /// status operations of stage B).
    pub depleted_reserves: Vec<u32>,
    /// The policy state in force (`policy`) is not yet in the anchor: a restrictive
    /// change whose anchor update failed, published by the next successful anchor
    /// write.
    pub policy_pending: bool,
    pub policy: policy_state_t,
}

/// registryd.
pub struct registry_t {
    store: journal_store_t,
    config: registry_config_t,
    io: Box<dyn journal_io_t>,
    clock: Box<dyn registry_clock_t>,
    users: Mutex<BTreeMap<u32, user_entry_t>>,
    anchor: Mutex<anchor_t>,
    ledger: Mutex<ledger_t>,
    /// The policy state in force (restrictive changes apply here first).
    policy: Mutex<policy_state_t>,
    run_decision: run_decision_t,
    owner: u32,
}

fn storage_unavailable() -> service_error_t {
    ipc(ipc_code_t::storage_unavailable)
}

fn not_durable() -> service_error_t {
    ipc(ipc_code_t::restriction_not_durable)
}

fn require(caller: caller_role_t, allowed: &[caller_role_t]) -> service_result_t<()> {
    if allowed.contains(&caller) {
        Ok(())
    } else {
        Err(ipc(ipc_code_t::role_denied))
    }
}

const Identityd: caller_role_t = caller_role_t::service(service_name_t::identityd);
const Contractd: caller_role_t = caller_role_t::service(service_name_t::contractd);
const Policyd: caller_role_t = caller_role_t::service(service_name_t::policyd);
const Admin: caller_role_t = caller_role_t::admin;

fn lock<item_t>(mutex: &Mutex<item_t>) -> service_result_t<MutexGuard<'_, item_t>> {
    // A panic while holding the lock poisons it: fail closed.
    mutex.lock().map_err(|_| storage_unavailable())
}

/// Resolves only the keys of one user's snapshot (layer 3 for commits).
struct snapshot_resolver_t<'directory> {
    directory: &'directory identity_directory_t,
}

impl key_resolver_t for snapshot_resolver_t<'_> {
    fn resolve(&self, kid: &digest_t) -> Option<[u8; 32]> {
        self.directory
            .resolve(kid)
            .map(|resolution| resolution.public_key)
    }
}

/// Layers 1–7 of a commit envelope against one user's snapshot.
fn check_commit_envelope(
    uid: u32,
    directory: &identity_directory_t,
    lineages: &lineage_index_t,
    envelope: &[u8],
) -> service_result_t<(admission_t, Box<commit_payload_t>)> {
    let verified = verify_object(
        envelope,
        object_kind_t::commit,
        &snapshot_resolver_t { directory },
    )?;
    let payload_t::commit(payload) = verified.payload else {
        return Err(ipc(ipc_code_t::malformed_request));
    };
    let resolution = directory
        .resolve(&payload.key_id)
        .filter(|resolution| resolution.owner_uid == uid)
        .ok_or(authority(authority_code_t::key_not_certified))?;
    check_commit_authority(&resolution, &payload)?;
    let admission = lineages.check_commit(&verified.object_id, &payload)?;
    Ok((admission, payload))
}

impl user_slot_t {
    fn user_state(&self) -> user_state_t {
        match self.state {
            slot_state_t::quarantined(_) => user_state_t::quarantined,
            slot_state_t::contained => user_state_t::contained,
            slot_state_t::normal => self
                .directory
                .user(self.uid)
                .map_or(user_state_t::deleted, |user| user.state()),
        }
    }

    fn anchored_state(&self) -> anchored_state_t {
        match self.state {
            slot_state_t::normal => match self.user_state() {
                user_state_t::contained => anchored_state_t::contained,
                user_state_t::quarantined => anchored_state_t::quarantined,
                user_state_t::normal | user_state_t::deleted => anchored_state_t::normal,
            },
            slot_state_t::contained => anchored_state_t::contained,
            slot_state_t::quarantined(_) => anchored_state_t::quarantined,
        }
    }

    fn anchored(&self) -> anchored_user_t {
        anchored_user_t {
            seq: self.journal.record_count(),
            chain: self.journal.chain(),
            user_id: self.user_id,
            state: self.anchored_state(),
            identity_seq: self.identity_count,
            min_valid_policy_epoch: 0,
        }
    }

    /// Quarantine (ADR 0006 §4): every grant of a live state ends `quarantined` and
    /// every branch ends, permanently; the slot keeps the verified prefix that
    /// `recover_user` (stage C) accepts.
    fn quarantine(&mut self, reason: quarantine_reason_t) {
        self.lineages.contain(end_reason_t::quarantined);
        self.state = slot_state_t::quarantined(reason);
    }

    /// Containment (§3): every grant admitted before this point ends `suspended`,
    /// permanently; only the recovery ceremony (stage C) re-authorizes scope.
    fn contain(&mut self) {
        self.lineages.contain(end_reason_t::suspended);
        self.state = slot_state_t::contained;
    }

    /// The sequence a restrictive record queued now will get: behind the pending ones.
    fn intended_sequence(&self) -> Option<u64> {
        let queued = u64::try_from(self.pending.len()).ok()?;
        self.journal.next_sequence()?.checked_add(queued)
    }

    /// Applies one replayed record at `sequence` (position `sequence − 1`); the same
    /// path applies a record this run writes, once it is durable.
    fn replay(&mut self, record: registry_record_t) -> service_result_t<()> {
        match record.body {
            record_body_t::identity { kind, object } => {
                let entry = journal_record_t::identity { kind, object };
                self.directory
                    .apply_journal_record(self.uid, &entry, record.sequence)?;
                self.identity_entries.push(journal_entry_t {
                    sequence: record.sequence,
                    record: entry,
                });
                self.identity_count = self.identity_count.saturating_add(1);
            }
            record_body_t::commit_admitted {
                commit_id,
                lineage_id,
                placement,
                envelope,
            } => {
                let (admission, payload) =
                    check_commit_envelope(self.uid, &self.directory, &self.lineages, &envelope)?;
                if admission.commit_id != commit_id
                    || admission.lineage_id != lineage_id
                    || admission.placement != placement
                {
                    return Err(storage_unavailable());
                }
                self.lineages
                    .apply_commit(&admission, payload, envelope, record.sequence);
            }
            record_body_t::lifecycle(lifecycle) => {
                self.lineages.check_record(&lifecycle, &record.time)?;
                self.lineages.apply_record(&lifecycle);
                self.lifecycle_records.push(lifecycle);
            }
            record_body_t::user(user_record) => {
                // identityd's index checks the record against the user's state
                // (recovery_rule) and applies it; the ECR index ends the grants.
                let entry = journal_record_t::user(user_record.clone());
                self.directory
                    .apply_journal_record(self.uid, &entry, record.sequence)?;
                self.identity_entries.push(journal_entry_t {
                    sequence: record.sequence,
                    record: entry,
                });
                self.lineages.apply_user_record(&user_record);
                match user_record {
                    user_record_t::containment => self.state = slot_state_t::contained,
                    user_record_t::quarantine => {
                        self.state = slot_state_t::quarantined(quarantine_reason_t::anchored)
                    }
                    user_record_t::recovery_closed { .. } => self.state = slot_state_t::normal,
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// Whether the journal already holds the record of a ledger entry (journaled
    /// before the crash): the same identity object, user record or lifecycle record.
    fn holds(&self, body: &record_body_t) -> bool {
        match body {
            record_body_t::lifecycle(lifecycle) => self.lifecycle_records.contains(lifecycle),
            record_body_t::commit_admitted { .. } => false,
            record_body_t::identity { .. } | record_body_t::user(_) => self
                .identity_entries
                .iter()
                .any(|entry| match (&entry.record, body) {
                    (
                        journal_record_t::identity { kind, object },
                        record_body_t::identity { kind: k, object: o },
                    ) => kind == k && object == o,
                    (journal_record_t::user(record), record_body_t::user(other)) => record == other,
                    _ => false,
                }),
        }
    }
}

/// Replays one user's parsed journal. Returns the slot and the identity records among
/// the first `anchored_seq` records, which the anchor's `identity_seq` must equal
/// (ADR 0006 §2).
fn replay_user(
    store: &journal_store_t,
    uid: u32,
    prefix: &crate::record::journal_prefix_t,
    io: &dyn journal_io_t,
    anchored_seq: u64,
) -> Result<(user_slot_t, u64), unusable_reason_t> {
    let parsed = &prefix.parsed;
    let journal = store
        .open_journal(uid, parsed, prefix.valid_length, io)
        .map_err(unusable_reason_t::store)?;
    let mut slot = user_slot_t {
        uid,
        user_id: parsed.user_id,
        journal,
        directory: identity_directory_t::new(),
        lineages: lineage_index_t::new(),
        identity_entries: Vec::new(),
        state: slot_state_t::normal,
        identity_count: 0,
        lifecycle_records: Vec::new(),
        pending: Vec::new(),
        anchor_failed: false,
    };
    let mut identity_at_anchor = 0u64;
    for (position, bytes) in parsed.records.iter().enumerate() {
        let expected = u64::try_from(position)
            .ok()
            .and_then(|position| position.checked_add(1))
            .ok_or(unusable_reason_t::replay { sequence: 0 })?;
        let failed = unusable_reason_t::replay { sequence: expected };
        let record = registry_record_t::decode(bytes).map_err(|_| failed)?;
        if record.sequence != expected {
            return Err(failed);
        }
        // The first record is the enrollment of the user the chain is bound to.
        if position == 0 {
            let first_is_enrollment = matches!(
                &record.body,
                record_body_t::identity {
                    kind: object_kind_t::identity_enrollment,
                    ..
                }
            );
            if !first_is_enrollment {
                return Err(failed);
            }
        }
        slot.replay(record).map_err(|_| failed)?;
        if position == 0
            && slot.directory.user(uid).map(|user| user.user_id()) != Some(parsed.user_id)
        {
            return Err(failed);
        }
        if expected == anchored_seq {
            identity_at_anchor = slot.identity_count;
        }
    }
    Ok((slot, identity_at_anchor))
}

/// The per-user recovery decision of ADR 0006 §4 for a parsed journal of `m` records
/// against the anchor's entry (`None`: the anchor does not know the user, which is a
/// genesis whose anchor update did not complete: `a = 0`).
fn recovery_decision(
    uid: u32,
    prefix: &crate::record::journal_prefix_t,
    anchored: Option<&anchored_user_t>,
) -> Result<bool, quarantine_reason_t> {
    let records = u64::try_from(prefix.parsed.records.len()).unwrap_or(u64::MAX);
    let (anchored_seq, anchored_chain) = match anchored {
        Some(entry) => (entry.seq, Some(entry.chain)),
        None => (0, None),
    };
    if records < anchored_seq {
        return Err(quarantine_reason_t::stale_journal);
    }
    if records > anchored_seq.saturating_add(1) {
        return Err(quarantine_reason_t::unexplained_records);
    }
    if let Some(chain) = anchored_chain {
        if prefix.parsed.chain_at(anchored_seq, uid) != Some(chain) {
            return Err(quarantine_reason_t::chain_mismatch);
        }
        if anchored.is_some_and(|entry| entry.user_id != prefix.parsed.user_id) {
            return Err(quarantine_reason_t::chain_mismatch);
        }
    }
    // `true`: one durable but unanchored record rolls the anchor forward.
    Ok(records == anchored_seq.saturating_add(1))
}

fn random_run_id() -> Result<[u8; 16], registry_start_error_t> {
    let mut run_id = [0u8; 16];
    cham3_os_adapter::fill_random(&mut run_id)
        .map_err(|_| registry_start_error_t::store(store_error_t::io))?;
    Ok(run_id)
}

impl registry_t {
    /// Starts registryd over the store at `root` (owned by `owner`, registryd's
    /// effective UID): opens the anchor and the previous ledger, decides the run
    /// (ADR 0006 §3), replays and recovers every journal (§4), contains or re-journals
    /// as decided, starts this run's ledger and writes the run marker `running`.
    pub fn open(
        root: &Path,
        owner: u32,
        config: registry_config_t,
        io: Box<dyn journal_io_t>,
        clock: Box<dyn registry_clock_t>,
    ) -> Result<registry_t, registry_start_error_t> {
        let valid = (MinQuotaBytes..=MaxQuotaBytes).contains(&config.quota_bytes)
            && (1..=MaxLineages).contains(&config.max_lineages)
            && config.min_valid_policy_epoch <= config.policy_epoch;
        if !valid {
            return Err(registry_start_error_t::invalid_config);
        }
        let store = journal_store_t::open(root, owner, io.as_ref())
            .map_err(registry_start_error_t::store)?;
        let store_device = std::fs::metadata(root)
            .map_err(|_| registry_start_error_t::store(store_error_t::io))?
            .dev();
        let anchor_directory = config
            .anchor_path
            .clone()
            .unwrap_or_else(|| root.join(DefaultAnchorDirectory));
        let ledger_directory = config
            .ledger_path
            .clone()
            .unwrap_or_else(|| root.join(DefaultLedgerDirectory));
        let boot_id = match &config.boot_id {
            Some(boot_id) => boot_id.clone(),
            None => kernel_boot_id_t
                .boot_id()
                .ok_or(registry_start_error_t::boot_id_unavailable)?,
        };
        let configured_policy = policy_state_t {
            epoch: config.policy_epoch,
            min_valid_epoch: config.min_valid_policy_epoch,
            emergency_restriction: false,
        };
        let (anchor, anchor_start) = anchor_t::open(
            &anchor_directory,
            owner,
            store_device,
            configured_policy,
            io.as_ref(),
        )
        .map_err(registry_start_error_t::store)?;

        // ADR 0006 §3: the previous run.
        let previous = crate::ledger::read_previous(&ledger_directory, owner);
        let (decision, replay_entries): (run_decision_t, Vec<ledger_entry_t>) = match &anchor
            .map()
            .run_marker
        {
            None => (run_decision_t::resumed, Vec::new()),
            Some(run_marker_t {
                state: run_state_t::clean,
                ..
            }) => (run_decision_t::resumed, Vec::new()),
            // The ledger failed in that run: unverifiable, whatever its files hold.
            Some(run_marker_t {
                state: run_state_t::unverifiable,
                ..
            }) => (run_decision_t::contained, Vec::new()),
            Some(run_marker_t {
                state: run_state_t::running,
                run_id,
                boot_id: marker_boot,
            }) => match previous {
                previous_ledger_t::valid {
                    run_id: ledger_run,
                    boot_id: ledger_boot,
                    entries,
                } if *marker_boot == boot_id && ledger_run == *run_id && ledger_boot == boot_id => {
                    (run_decision_t::ledger_replayed, entries)
                }
                _ => (run_decision_t::contained, Vec::new()),
            },
        };

        // The policy state: the anchor's, raised by the configuration (never lowered)
        // and by any policy entry the ledger holds.
        let mut policy = anchor.map().policy;
        policy.epoch = policy.epoch.max(config.policy_epoch);
        policy.min_valid_epoch = policy.min_valid_epoch.max(config.min_valid_policy_epoch);
        for entry in replay_entries
            .iter()
            .filter(|entry| entry.kind == entry_kind_t::policy)
        {
            if let Some((epoch, min_valid, emergency)) = parse_policy_payload(&entry.payload) {
                policy.epoch = policy.epoch.max(epoch);
                policy.min_valid_epoch = policy.min_valid_epoch.max(min_valid);
                policy.emergency_restriction |= emergency;
            }
        }

        // ADR 0006 §4: every journal against the anchor.
        let anchor_invalid = anchor_start == anchor_start_t::invalid;
        let journals_exist = !store
            .users()
            .map_err(registry_start_error_t::store)?
            .is_empty();
        let anchor_lost = anchor_start == anchor_start_t::absent && journals_exist;
        let mut users = BTreeMap::new();
        let mut roll_forward: Vec<u32> = Vec::new();
        for uid in store.users().map_err(registry_start_error_t::store)? {
            let anchored = anchor.map().users.get(&uid).copied();
            let entry = match store.read(uid, config.quota_bytes) {
                Ok(None) => match anchored {
                    // A user directory without a journal holds no user (a creation
                    // that never completed its first durable record)...
                    None => continue,
                    // ...unless the anchor knows the user: the store is lost.
                    Some(_) => user_entry_t::unusable(unusable_reason_t::quarantine(
                        quarantine_reason_t::lost_store,
                    )),
                },
                Ok(Some(bytes)) => {
                    match Self::recover_user(
                        &store,
                        uid,
                        &bytes,
                        anchored.as_ref(),
                        anchor.map(),
                        anchor_invalid || anchor_lost,
                        io.as_ref(),
                    ) {
                        Ok((slot, forward)) => {
                            if forward {
                                roll_forward.push(uid);
                            }
                            user_entry_t::usable(Arc::new(Mutex::new(slot)))
                        }
                        Err(reason) => user_entry_t::unusable(reason),
                    }
                }
                Err(error) => user_entry_t::unusable(unusable_reason_t::store(error)),
            };
            users.insert(uid, entry);
        }
        // Users the anchor knows whose directory is gone.
        for (uid, _) in anchor.map().users.iter() {
            users
                .entry(*uid)
                .or_insert(user_entry_t::unusable(unusable_reason_t::quarantine(
                    quarantine_reason_t::lost_store,
                )));
        }

        // This run's ledger, then the recovered state is made durable.
        let run_id = random_run_id()?;
        let ledger = ledger_t::start(&ledger_directory, owner, run_id, &boot_id, io.as_ref())
            .map_err(registry_start_error_t::store)?;
        let registry = registry_t {
            store,
            config,
            io,
            clock,
            users: Mutex::new(users),
            anchor: Mutex::new(anchor),
            ledger: Mutex::new(ledger),
            policy: Mutex::new(policy),
            run_decision: decision,
            owner,
        };
        // Roll the anchor forward for the durable but unanchored records (§4).
        for uid in roll_forward {
            registry
                .anchor_user(uid)
                .map_err(|_| registry_start_error_t::store(store_error_t::io))?;
        }
        // Re-journal the previous run's ledger entries (§3, same boot).
        for entry in &replay_entries {
            if entry.kind == entry_kind_t::journal_record {
                registry.rejournal(entry);
            }
        }
        // Containment of every user after an unverifiable run (§3).
        if decision == run_decision_t::contained {
            registry.contain_everyone();
        }
        // The policy state and the run marker, durable before anything is served.
        let marker = run_marker_t {
            run_id,
            boot_id,
            state: run_state_t::running,
        };
        let policy_now = *lock(&registry.policy)
            .map_err(|_| registry_start_error_t::store(store_error_t::io))?;
        let mut anchor =
            lock(&registry.anchor).map_err(|_| registry_start_error_t::store(store_error_t::io))?;
        anchor
            .update(registry.io.as_ref(), |map| {
                map.policy = policy_now;
                map.run_marker = Some(marker);
            })
            .map_err(registry_start_error_t::store)?;
        drop(anchor);
        Ok(registry)
    }

    /// The §4 decision for one journal: truncation of a torn tail, the decision table,
    /// replay, and the user's state from the anchor and the tombstones.
    fn recover_user(
        store: &journal_store_t,
        uid: u32,
        bytes: &[u8],
        anchored: Option<&anchored_user_t>,
        map: &anchor_map_t,
        anchor_unusable: bool,
        io: &dyn journal_io_t,
    ) -> Result<(user_slot_t, bool), unusable_reason_t> {
        let mut prefix = parse_journal_prefix(bytes, uid).map_err(unusable_reason_t::defect)?;
        if prefix.torn {
            // A torn frame beyond the anchor is truncated; one inside the anchored
            // range leaves a stale journal (the table's rows then apply).
            let records = u64::try_from(prefix.parsed.records.len()).unwrap_or(u64::MAX);
            if anchored.is_some_and(|entry| records < entry.seq) {
                return Err(unusable_reason_t::quarantine(
                    quarantine_reason_t::stale_journal,
                ));
            }
            store
                .truncate_tail(uid, prefix.valid_length, io)
                .map_err(unusable_reason_t::store)?;
            prefix.torn = false;
        }
        let quarantine = if anchor_unusable {
            Some(quarantine_reason_t::anchor_invalid)
        } else {
            match map.tombstones.get(&prefix.parsed.user_id) {
                Some(tombstone) if tombstone.state == tombstone_state_t::deleted => {
                    Some(quarantine_reason_t::deleted_user_files)
                }
                Some(_) => Some(quarantine_reason_t::deletion_pending),
                None => None,
            }
        };
        let forward = match (quarantine, recovery_decision(uid, &prefix, anchored)) {
            (Some(_), _) => false,
            (None, Err(reason)) => return Err(unusable_reason_t::quarantine(reason)),
            (None, Ok(forward)) => forward,
        };
        let anchored_seq = anchored.map_or(0, |entry| entry.seq);
        let (mut slot, identity_at_anchor) = replay_user(store, uid, &prefix, io, anchored_seq)
            .map_err(|reason| match reason {
                unusable_reason_t::replay { .. } => {
                    unusable_reason_t::quarantine(quarantine_reason_t::replay_defect)
                }
                other => other,
            })?;
        // The anchor's identity count must describe the anchored prefix exactly (§2);
        // an entry that disagrees is no anchor of this journal (M2-006-A-R07). The one
        // unanchored record of a roll-forward may itself be an identity record.
        if quarantine.is_none()
            && anchored.is_some_and(|entry| entry.identity_seq != identity_at_anchor)
        {
            return Err(unusable_reason_t::quarantine(
                quarantine_reason_t::identity_mismatch,
            ));
        }
        if let Some(reason) = quarantine {
            // Quarantine denies every grant and key of the user (§4); the slot keeps
            // the verified prefix for `recover_user` (stage C).
            slot.quarantine(reason);
        } else if let Some(entry) = anchored {
            // The anchor's state is authoritative over the records' (a containment or
            // quarantine whose record is not durable is still in force), and ends the
            // grants exactly as the record would (M2-006-A-R08).
            match entry.state {
                anchored_state_t::quarantined => {
                    if !matches!(slot.state, slot_state_t::quarantined(_)) {
                        slot.quarantine(quarantine_reason_t::anchored);
                    }
                }
                anchored_state_t::contained => {
                    if slot.state == slot_state_t::normal {
                        slot.contain();
                    }
                }
                anchored_state_t::normal => {}
            }
        }
        Ok((slot, forward))
    }

    /// Re-journals a ledger entry whose record the journal does not hold (§3, same
    /// boot): its in-memory effect is applied exactly as the operation that ledgered
    /// it applies it, then it takes the restrictive path again — this run's ledger
    /// first, so that the intent survives any further unclean restart in this boot
    /// (M2-006-A-R03), then the journal and the anchor; a failure leaves it pending,
    /// retried by every later operation of the user. An entry for a user the store
    /// does not know is dropped (nothing to deny); a quarantined user's journal is left
    /// as found (every grant and key of the user is denied already, and the verified
    /// prefix belongs to the ceremony of stage C).
    fn rejournal(&self, entry: &ledger_entry_t) {
        let Ok(record) = registry_record_t::decode(&entry.payload) else {
            return;
        };
        let Ok(Some(slot)) = self.slot_any(entry.uid) else {
            return;
        };
        let Ok(mut slot) = lock(&slot) else {
            return;
        };
        if let slot_state_t::quarantined(_) = slot.state {
            return;
        }
        if slot.holds(&record.body) {
            return;
        }
        let uid = slot.uid;
        match &record.body {
            record_body_t::lifecycle(lifecycle) => {
                if slot.lineages.check_record(lifecycle, &record.time).is_err() {
                    return;
                }
                slot.lineages.apply_record(lifecycle);
            }
            record_body_t::identity { kind, object } => {
                let Ok(verified) = slot.directory.verify_identity_object(*kind, object) else {
                    return;
                };
                let Some(intended) = slot.intended_sequence() else {
                    return;
                };
                if slot
                    .directory
                    .apply_identity_object(uid, &verified, intended)
                    .is_err()
                {
                    return;
                }
            }
            // Containment is applied by the restrictive path itself; no other user
            // record is ledgered in stage A.
            record_body_t::user(user_record) => {
                if *user_record != user_record_t::containment {
                    return;
                }
            }
            record_body_t::commit_admitted { .. } => return,
        }
        let _ = self.restrictive_write(&mut slot, record.time.clone(), record.body.clone());
    }

    /// Contains every usable, not yet contained or quarantined user (§3).
    fn contain_everyone(&self) {
        let slots: Vec<Arc<Mutex<user_slot_t>>> = match lock(&self.users) {
            Ok(users) => users
                .values()
                .filter_map(|entry| match entry {
                    user_entry_t::usable(slot) => Some(Arc::clone(slot)),
                    user_entry_t::unusable(_) => None,
                })
                .collect(),
            Err(_) => return,
        };
        for slot in slots {
            let Ok(mut slot) = lock(&slot) else {
                continue;
            };
            if slot.state != slot_state_t::normal {
                continue;
            }
            let Ok(time) = self.now_text() else {
                continue;
            };
            let _ = self.restrictive_write(
                &mut slot,
                time,
                record_body_t::user(user_record_t::containment),
            );
        }
    }

    pub fn config(&self) -> registry_config_t {
        self.config.clone()
    }

    /// The run marker decision of this start (ADR 0006 §3).
    pub fn run_decision(&self) -> run_decision_t {
        self.run_decision
    }

    /// The policy state in force.
    pub fn policy_state(&self) -> service_result_t<policy_state_t> {
        Ok(*lock(&self.policy)?)
    }

    /// Users whose journal is unusable in this run (unreadable, defective or
    /// quarantined without a usable slot), with the reason. Quarantined users with a
    /// readable journal are in `rollback_status`.
    pub fn unusable_users(&self) -> service_result_t<Vec<(u32, unusable_reason_t)>> {
        Ok(lock(&self.users)?
            .iter()
            .filter_map(|(uid, entry)| match entry {
                user_entry_t::unusable(reason) => Some((*uid, *reason)),
                user_entry_t::usable(_) => None,
            })
            .collect())
    }

    fn slot_any(&self, uid: u32) -> service_result_t<Option<Arc<Mutex<user_slot_t>>>> {
        match lock(&self.users)?.get(&uid) {
            None => Ok(None),
            Some(user_entry_t::usable(slot)) => Ok(Some(Arc::clone(slot))),
            Some(user_entry_t::unusable(unusable_reason_t::quarantine(_))) => {
                Err(authority(authority_code_t::quarantined))
            }
            Some(user_entry_t::unusable(_)) => Err(storage_unavailable()),
        }
    }

    /// A user's slot for a write: with a failed ledger or journal nothing is written;
    /// otherwise what the slot owes — the anchor update of an unanchored record first,
    /// then the pending restrictive records — is retried, and the operation proceeds
    /// only once nothing is owed.
    fn ready(&self, slot: &mut user_slot_t) -> service_result_t<()> {
        if lock(&self.ledger)?.failed() {
            return Err(storage_unavailable());
        }
        if slot.journal.failed() {
            return Err(storage_unavailable());
        }
        if slot.anchor_failed || !slot.pending.is_empty() {
            self.flush_pending(slot)?;
        }
        Ok(())
    }

    /// A user's slot for a read: a durable record whose anchor update failed is
    /// anchored first, and the read is refused while that keeps failing, so that
    /// nothing of the unanchored state is served (M2-006-A-R01).
    fn anchored_for_reading(&self, slot: &mut user_slot_t) -> service_result_t<()> {
        if slot.anchor_failed {
            self.anchor_slot(slot)?;
        }
        Ok(())
    }

    fn now_text(&self) -> service_result_t<String> {
        utc_text(self.clock.utc_seconds()).ok_or(storage_unavailable())
    }

    /// Whether a record of `record_length` bytes fits the user's quota. Restrictive
    /// records may use the quota's reserve; nothing else may (ADR 0006 §1). The
    /// preallocated reserve of filesystem blocks is the journal's (`journal.rs`).
    fn check_quota(
        &self,
        journal: &journal_file_t,
        record_length: usize,
        restrictive: bool,
    ) -> service_result_t<()> {
        let limit = if restrictive {
            self.config.quota_bytes
        } else {
            self.config.quota_bytes.saturating_sub(ReserveBytes)
        };
        let after = journal_file_t::frame_length(record_length)
            .and_then(|frame| journal.length().checked_add(frame))
            .ok_or(ipc(ipc_code_t::quota_exhausted))?;
        if after > limit {
            return Err(ipc(ipc_code_t::quota_exhausted));
        }
        Ok(())
    }

    // ------------------------------------------------------------------ durability

    /// Anchors the user's current sequence and chain (ADR 0006 §2) after a durable
    /// append; the slot's `min_valid_policy_epoch` is kept.
    fn anchor_user(&self, uid: u32) -> service_result_t<()> {
        let Some(slot) = self.slot_any(uid)? else {
            return Err(storage_unavailable());
        };
        let mut slot = lock(&slot)?;
        self.anchor_slot(&mut slot)
    }

    /// Anchors the slot's current sequence, chain and state, and publishes the policy
    /// state in force with it (a policy change whose own update failed becomes durable
    /// with the next successful slot write); the slot's `min_valid_policy_epoch` is
    /// kept. The policy lock is held across the write (lock order), so that no older
    /// copy of the policy state can overwrite a newer publication.
    fn anchor_slot(&self, slot: &mut user_slot_t) -> service_result_t<()> {
        let mut entry = slot.anchored();
        let uid = slot.uid;
        let policy = lock(&self.policy)?;
        let policy_now = *policy;
        let mut anchor = lock(&self.anchor)?;
        entry.min_valid_policy_epoch = anchor
            .map()
            .users
            .get(&uid)
            .map_or(0, |existing| existing.min_valid_policy_epoch);
        match anchor.update(self.io.as_ref(), |map| {
            map.policy = policy_now;
            map.users.insert(uid, entry);
        }) {
            Ok(()) => {
                slot.anchor_failed = false;
                Ok(())
            }
            Err(_) => {
                slot.anchor_failed = true;
                Err(storage_unavailable())
            }
        }
    }

    /// A non-restrictive append: journal, then anchor (ADR 0006 §6); on an anchor
    /// failure after a durable record the user is denied until restart.
    fn ordinary_append(&self, slot: &mut user_slot_t, record: &[u8]) -> service_result_t<u64> {
        self.check_quota(&slot.journal, record.len(), false)?;
        let sequence = slot
            .journal
            .append(record, false, self.io.as_ref())
            .map_err(|_| storage_unavailable())?;
        Ok(sequence)
    }

    /// The restrictive write path (ADR 0006 §3): the in-memory effect is already
    /// applied by the caller; this writes the ledger entry, the journal record and the
    /// anchor update, behind whatever the slot still owes. A ledger failure
    /// invalidates the ledger (no eligible decisions until restart; the run is marked
    /// unverifiable) but the record is still written; a journal or anchor failure
    /// leaves the change pending and reports `restriction_not_durable`. Returns the
    /// record's sequence.
    fn restrictive_write(
        &self,
        slot: &mut user_slot_t,
        time: String,
        body: record_body_t,
    ) -> service_result_t<u64> {
        // The in-memory denial for records the caller did not apply (containment).
        if let record_body_t::user(user_record) = &body {
            if *user_record == user_record_t::containment && slot.state == slot_state_t::normal {
                slot.contain();
            }
        }
        let sequence = slot.intended_sequence().unwrap_or(0);
        slot.pending.push(pending_write_t { time, body });
        let encoded = {
            let Some(last) = slot.pending.last() else {
                return Err(storage_unavailable());
            };
            registry_record_t {
                sequence,
                time: last.time.clone(),
                body: last.body.clone(),
            }
            .encode()
        };
        let appended = {
            let mut ledger = lock(&self.ledger)?;
            ledger.append(
                self.io.as_ref(),
                entry_kind_t::journal_record,
                slot.uid,
                &encoded,
            )
        };
        if appended == Err(store_error_t::io) {
            self.mark_run_unverifiable();
        }
        self.flush_pending(slot)
    }

    /// The ledger failed: besides the removal of its validity file, the run marker
    /// records the run as unverifiable, so that the next start contains even when the
    /// validity file could not be removed. A failure here is ignored: the ledger is
    /// marked failed and nothing eligible is served for the rest of the run.
    fn mark_run_unverifiable(&self) {
        let Ok(mut anchor) = lock(&self.anchor) else {
            return;
        };
        let _ = anchor.update(self.io.as_ref(), |map| {
            if let Some(marker) = &mut map.run_marker {
                marker.state = run_state_t::unverifiable;
            }
        });
    }

    /// Makes what the slot owes durable, in order (ADR 0006 §§3, 6): a durable record
    /// whose anchor update failed is anchored first — nothing is appended beyond it
    /// until it is, so that a start finds at most one unanchored record (§4 rolls it
    /// forward) and nothing of the unanchored state is served — then every pending
    /// restrictive record is appended, then the anchor names the result. A failure
    /// leaves the rest owed: `storage_unavailable` when only the anchor is owed,
    /// `restriction_not_durable` when a restrictive record is. Returns the sequence of
    /// the last record appended (the journal's count when none was).
    fn flush_pending(&self, slot: &mut user_slot_t) -> service_result_t<u64> {
        if slot.anchor_failed && self.anchor_slot(slot).is_err() {
            return Err(if slot.pending.is_empty() {
                storage_unavailable()
            } else {
                not_durable()
            });
        }
        let mut last_sequence = slot.journal.record_count();
        let mut appended = false;
        while let Some(pending) = slot.pending.first().cloned() {
            let sequence = slot.journal.next_sequence().ok_or(not_durable())?;
            let record = registry_record_t {
                sequence,
                time: pending.time.clone(),
                body: pending.body.clone(),
            };
            let encoded = record.encode();
            if self
                .check_quota(&slot.journal, encoded.len(), true)
                .is_err()
            {
                return Err(not_durable());
            }
            if slot
                .journal
                .append(&encoded, true, self.io.as_ref())
                .is_err()
            {
                return Err(not_durable());
            }
            // Durable: the indexes already hold the effect for lifecycle records the
            // caller applied; identity and user records enter the entries here.
            match record.body {
                record_body_t::identity { kind, object } => {
                    slot.identity_entries.push(journal_entry_t {
                        sequence,
                        record: journal_record_t::identity { kind, object },
                    });
                    slot.identity_count = slot.identity_count.saturating_add(1);
                }
                record_body_t::user(user_record) => {
                    slot.identity_entries.push(journal_entry_t {
                        sequence,
                        record: journal_record_t::user(user_record),
                    });
                }
                record_body_t::lifecycle(lifecycle) => slot.lifecycle_records.push(lifecycle),
                record_body_t::commit_admitted { .. } => {}
            }
            slot.pending.remove(0);
            last_sequence = sequence;
            appended = true;
        }
        if appended && self.anchor_slot(slot).is_err() {
            // The records are durable; the anchor is retried by every later operation
            // of the user, or rolled forward at the next start. Until then the user is
            // denied (anchor_failed).
            return Err(not_durable());
        }
        Ok(last_sequence)
    }

    // ---------------------------------------------------------------- identity

    /// `append_identity` (`service:identityd`): persists a validated identity object
    /// in `owner_uid`'s journal before replying. registryd re-validates it against
    /// the user's snapshot (it must replay it later). An identical object already
    /// durable returns its sequence without a second record. A key invalidation takes
    /// the restrictive path.
    pub fn append_identity(
        &self,
        caller: caller_role_t,
        owner_uid: u32,
        kind: object_kind_t,
        object: &[u8],
    ) -> service_result_t<append_receipt_t> {
        require(caller, &[Identityd])?;
        if kind == object_kind_t::commit {
            return Err(ipc(ipc_code_t::malformed_request));
        }
        if let Some(slot) = self.slot_any(owner_uid)? {
            let mut slot = lock(&slot)?;
            return self.append_identity_to(&mut slot, kind, object);
        }
        self.create_user(owner_uid, kind, object)
    }

    fn append_identity_to(
        &self,
        slot: &mut user_slot_t,
        kind: object_kind_t,
        object: &[u8],
    ) -> service_result_t<append_receipt_t> {
        let restrictive = kind == object_kind_t::key_invalidation;
        if let slot_state_t::quarantined(_) = slot.state {
            return Err(authority(authority_code_t::quarantined));
        }
        let verified = slot.directory.verify_identity_object(kind, object)?;
        let effect = slot.directory.check_identity_object(slot.uid, &verified)?;
        if let identity_effect_t::already_present(sequence) = effect {
            return Ok(append_receipt_t {
                journal_seq: sequence,
                already_present: true,
            });
        }
        let body = record_body_t::identity {
            kind,
            object: object.to_vec(),
        };
        if restrictive {
            // Denial first (the directory holds the invalidation), then the ledger,
            // the journal and the anchor — behind whatever the slot owes, an
            // unanchored record included (M2-006-A-R02).
            let time = self.now_text()?;
            let intended = slot.intended_sequence().ok_or(storage_unavailable())?;
            slot.directory
                .apply_identity_object(slot.uid, &verified, intended)?;
            let sequence = self.restrictive_write(slot, time, body)?;
            return Ok(append_receipt_t {
                journal_seq: sequence,
                already_present: false,
            });
        }
        self.ready(slot)?;
        if !slot.pending.is_empty() {
            return Err(not_durable());
        }
        let sequence = slot.journal.next_sequence().ok_or(storage_unavailable())?;
        let record = registry_record_t {
            sequence,
            time: self.now_text()?,
            body,
        }
        .encode();
        self.ordinary_append(slot, &record)?;
        slot.directory
            .apply_identity_object(slot.uid, &verified, sequence)?;
        slot.identity_entries.push(journal_entry_t {
            sequence,
            record: journal_record_t::identity {
                kind,
                object: object.to_vec(),
            },
        });
        slot.identity_count = slot.identity_count.saturating_add(1);
        self.anchor_slot(slot)?;
        Ok(append_receipt_t {
            journal_seq: sequence,
            already_present: false,
        })
    }

    /// Genesis at enrollment (ADR 0006 §4): header, enrollment record and anchor
    /// entry, durable in that order, under the `users` lock so that two first
    /// enrollments of a UID cannot race.
    fn create_user(
        &self,
        owner_uid: u32,
        kind: object_kind_t,
        object: &[u8],
    ) -> service_result_t<append_receipt_t> {
        let mut users = lock(&self.users)?;
        match users.get(&owner_uid) {
            None => {}
            // Created meanwhile: retry through the normal path.
            Some(user_entry_t::usable(slot)) => {
                let slot = Arc::clone(slot);
                drop(users);
                let mut slot = lock(&slot)?;
                return self.append_identity_to(&mut slot, kind, object);
            }
            Some(user_entry_t::unusable(unusable_reason_t::quarantine(_))) => {
                return Err(authority(authority_code_t::quarantined));
            }
            Some(user_entry_t::unusable(_)) => return Err(storage_unavailable()),
        }
        if lock(&self.ledger)?.failed() {
            return Err(storage_unavailable());
        }
        let mut directory = identity_directory_t::new();
        let verified = directory.verify_identity_object(kind, object)?;
        if directory.check_identity_object(owner_uid, &verified)? != identity_effect_t::enroll {
            return Err(authority(authority_code_t::key_not_certified));
        }
        let payload_t::identity_enrollment(enrollment) = &verified.payload else {
            return Err(authority(authority_code_t::key_not_certified));
        };
        let user_id = enrollment.user_id;
        // A deleted user's id never enrolls again; a pending deletion neither.
        if lock(&self.anchor)?.map().tombstones.contains_key(&user_id) {
            return Err(authority(authority_code_t::user_deleted));
        }
        let record = registry_record_t {
            sequence: 1,
            time: self.now_text()?,
            body: record_body_t::identity {
                kind,
                object: object.to_vec(),
            },
        }
        .encode();
        let journal = self
            .store
            .create_journal(owner_uid, &user_id, &record, self.io.as_ref())
            .map_err(|_| storage_unavailable())?;
        directory.apply_identity_object(owner_uid, &verified, 1)?;
        let mut slot = user_slot_t {
            uid: owner_uid,
            user_id,
            journal,
            directory,
            lineages: lineage_index_t::new(),
            identity_entries: vec![journal_entry_t {
                sequence: 1,
                record: journal_record_t::identity {
                    kind,
                    object: object.to_vec(),
                },
            }],
            state: slot_state_t::normal,
            identity_count: 1,
            lifecycle_records: Vec::new(),
            pending: Vec::new(),
            anchor_failed: false,
        };
        let anchored = self.anchor_slot(&mut slot);
        users.insert(owner_uid, user_entry_t::usable(Arc::new(Mutex::new(slot))));
        anchored?;
        Ok(append_receipt_t {
            journal_seq: 1,
            already_present: false,
        })
    }

    /// `identity_state` (`service:identityd`): the user's identity and user-state
    /// records with a sequence above `after`, one page at a time. A quarantined user
    /// with a readable journal is paged (its quarantine record included); one without
    /// is `quarantined`.
    pub fn identity_state(
        &self,
        caller: caller_role_t,
        owner_uid: u32,
        after: u64,
    ) -> service_result_t<journal_page_t> {
        require(caller, &[Identityd])?;
        let Some(slot) = self.slot_any(owner_uid)? else {
            return Ok(journal_page_t {
                entries: Vec::new(),
                next: None,
            });
        };
        let mut slot = lock(&slot)?;
        if let slot_state_t::quarantined(_) = slot.state {
            // Every key of a quarantined user is denied (ADR 0006 §4, §5): identityd
            // learns the quarantine instead of the records.
            return Err(authority(authority_code_t::quarantined));
        }
        if slot.journal.failed() {
            return Err(storage_unavailable());
        }
        self.anchored_for_reading(&mut slot)?;
        let mut remaining = slot
            .identity_entries
            .iter()
            .filter(|entry| entry.sequence > after);
        let entries: Vec<journal_entry_t> =
            remaining.by_ref().take(MaxPageRecords).cloned().collect();
        let next = match remaining.next() {
            Some(_) => entries.last().map(|entry| entry.sequence),
            None => None,
        };
        Ok(journal_page_t { entries, next })
    }

    /// UIDs with a journal, including unusable ones (identityd then fails them).
    pub fn journal_users(&self, caller: caller_role_t) -> service_result_t<Vec<u32>> {
        require(caller, &[Identityd])?;
        Ok(lock(&self.users)?.keys().copied().collect())
    }

    // ---------------------------------------------------------------- admission

    /// `admit` (`service:contractd`): layers 1–6 against the user's snapshot, layer 7,
    /// the signing key's certificate window at registry time, the current policy
    /// epoch and quotas, under the user lock immediately before the append; durable
    /// and anchored before the reply; admitted `pending`. A contained or quarantined
    /// user admits nothing (layer 6 refuses every key); a user with a pending
    /// restrictive write admits nothing until it is durable.
    pub fn admit(
        &self,
        caller: caller_role_t,
        owner_uid: u32,
        envelope: &[u8],
    ) -> service_result_t<admission_receipt_t> {
        require(caller, &[Contractd])?;
        let slot = self
            .slot_any(owner_uid)?
            .ok_or(authority(authority_code_t::key_not_certified))?;
        let mut slot = lock(&slot)?;
        match slot.state {
            slot_state_t::quarantined(_) => return Err(authority(authority_code_t::quarantined)),
            slot_state_t::contained => return Err(authority(authority_code_t::contained)),
            slot_state_t::normal => {}
        }
        self.ready(&mut slot)?;
        if !slot.pending.is_empty() {
            return Err(not_durable());
        }
        let (admission, payload) =
            check_commit_envelope(owner_uid, &slot.directory, &slot.lineages, envelope)?;
        // (beyond the model) The signing key's certificate window must also contain
        // registry time now, not only the commit's own timestamp: an envelope signed
        // in a temporary key's window cannot be admitted after that window closed.
        // Admission-time only: replay reproduces historical admissions. The ±300 s
        // commit-timestamp check of ADR 0007 §5 (`timestamp_rule`) is stage B.
        let resolution = slot
            .directory
            .resolve(&payload.key_id)
            .ok_or(authority(authority_code_t::key_not_certified))?;
        check_signing_window(&resolution, self.clock.utc_seconds())?;
        let policy = *lock(&self.policy)?;
        if payload.policy.epoch != policy.epoch {
            return Err(ipc(ipc_code_t::stale_epoch));
        }
        if admission.new_lineage && slot.lineages.lineage_count() >= self.config.max_lineages {
            return Err(ipc(ipc_code_t::quota_exhausted));
        }
        let sequence = slot.journal.next_sequence().ok_or(storage_unavailable())?;
        let record = registry_record_t {
            sequence,
            time: self.now_text()?,
            body: record_body_t::commit_admitted {
                commit_id: admission.commit_id,
                lineage_id: admission.lineage_id,
                placement: admission.placement,
                envelope: envelope.to_vec(),
            },
        }
        .encode();
        // Not restrictive: the full reserve must exist before the append (R03).
        self.ordinary_append(&mut slot, &record)?;
        slot.lineages
            .apply_commit(&admission, payload, envelope.to_vec(), sequence);
        self.anchor_slot(&mut slot)?;
        Ok(admission_receipt_t {
            commit_id: admission.commit_id,
            journal_seq: sequence,
            placement: admission.placement,
            status: crate::lineage::commit_status_t::pending,
        })
    }

    /// `restrict` (`service:contractd`, `service:policyd`, `service:identityd`,
    /// `admin`): a narrowing lifecycle record through the restrictive path (ADR 0006
    /// §3): validated (`lifecycle_rule`), denied in memory, ledgered, journaled from
    /// the reserve, anchored, then acknowledged with its sequence. Reports
    /// `restriction_not_durable` while the denial is held only in memory and ledger;
    /// the write is retried by every later operation of the user.
    pub fn restrict(
        &self,
        caller: caller_role_t,
        owner_uid: u32,
        record: &lifecycle_record_t,
    ) -> service_result_t<u64> {
        require(caller, &[Contractd, Policyd, Identityd, Admin])?;
        let slot = self
            .slot_any(owner_uid)?
            .ok_or(authority(authority_code_t::key_not_certified))?;
        let mut slot = lock(&slot)?;
        if let slot_state_t::quarantined(_) = slot.state {
            return Err(authority(authority_code_t::quarantined));
        }
        let time = self.now_text()?;
        slot.lineages.check_record(record, &time)?;
        // Denial first, whatever the writes do; the write joins the queue behind
        // whatever the slot owes, an unanchored record included (M2-006-A-R02).
        slot.lineages.apply_record(record);
        self.restrictive_write(&mut slot, time, record_body_t::lifecycle(record.clone()))
    }

    /// `set_policy_epoch` (`admin`): raises the policy epoch and the minimum valid
    /// epoch (never lowers either) through the restrictive path: in force at once,
    /// ledgered, anchored.
    pub fn set_policy_epoch(
        &self,
        caller: caller_role_t,
        epoch: u64,
        min_valid_epoch: u64,
    ) -> service_result_t<policy_state_t> {
        require(caller, &[Admin])?;
        if min_valid_epoch > epoch {
            return Err(ipc(ipc_code_t::malformed_request));
        }
        self.change_policy(|policy| {
            policy.epoch = policy.epoch.max(epoch);
            policy.min_valid_epoch = policy.min_valid_epoch.max(min_valid_epoch);
        })
    }

    /// `set_emergency_restriction` (`admin`): sets or releases the emergency
    /// restriction of everyone (ADR 0007 §1 rule 1). Setting it is restrictive (in
    /// force at once, ledgered, then anchored). Releasing it restores no grant by
    /// itself and is permissive: it takes effect only once the anchor holds it, and a
    /// release whose anchor update fails changes nothing (M2-006-A-R05).
    pub fn set_emergency_restriction(
        &self,
        caller: caller_role_t,
        restricted: bool,
    ) -> service_result_t<policy_state_t> {
        require(caller, &[Admin])?;
        if restricted {
            self.change_policy(|policy| policy.emergency_restriction = true)
        } else {
            self.release_policy(|policy| policy.emergency_restriction = false)
        }
    }

    /// A restrictive policy change (ADR 0006 §3): in force at once, ledgered, then
    /// anchored; with its anchor update failed it stays in force and pending, published
    /// by the next successful anchor write (`rollback_status` reports it). The policy
    /// lock is held throughout: publications never overtake each other.
    fn change_policy(
        &self,
        change: impl Fn(&mut policy_state_t),
    ) -> service_result_t<policy_state_t> {
        let mut policy = lock(&self.policy)?;
        change(&mut policy);
        let next = *policy;
        let appended = {
            let mut ledger = lock(&self.ledger)?;
            ledger.append(
                self.io.as_ref(),
                entry_kind_t::policy,
                0,
                &policy_payload(next.epoch, next.min_valid_epoch, next.emergency_restriction),
            )
        };
        if appended == Err(store_error_t::io) {
            self.mark_run_unverifiable();
        }
        let mut anchor = lock(&self.anchor)?;
        anchor
            .update(self.io.as_ref(), |map| map.policy = next)
            .map_err(|_| not_durable())?;
        Ok(next)
    }

    /// A permissive policy change: durable in the anchor first, in force only then; a
    /// failed anchor update leaves the state as it was (`restriction_not_durable`).
    fn release_policy(
        &self,
        change: impl Fn(&mut policy_state_t),
    ) -> service_result_t<policy_state_t> {
        let mut policy = lock(&self.policy)?;
        let mut next = *policy;
        change(&mut next);
        let mut anchor = lock(&self.anchor)?;
        anchor
            .update(self.io.as_ref(), |map| map.policy = next)
            .map_err(|_| not_durable())?;
        *policy = next;
        Ok(next)
    }

    /// `rollback_status` (`service:*`, `trusted_client`).
    pub fn rollback_status(&self, caller: caller_role_t) -> service_result_t<rollback_status_t> {
        match caller {
            caller_role_t::service(_) | caller_role_t::trusted_client { .. } => {}
            _ => return Err(ipc(ipc_code_t::role_denied)),
        }
        let mut quarantined = Vec::new();
        let mut unusable = Vec::new();
        let mut contained = Vec::new();
        let mut pending = Vec::new();
        let mut depleted = Vec::new();
        let slots: Vec<(u32, Arc<Mutex<user_slot_t>>)> = {
            let users = lock(&self.users)?;
            for (uid, entry) in users.iter() {
                match entry {
                    user_entry_t::unusable(unusable_reason_t::quarantine(reason)) => {
                        quarantined.push((*uid, *reason));
                    }
                    user_entry_t::unusable(reason) => unusable.push((*uid, *reason)),
                    user_entry_t::usable(_) => {}
                }
            }
            users
                .iter()
                .filter_map(|(uid, entry)| match entry {
                    user_entry_t::usable(slot) => Some((*uid, Arc::clone(slot))),
                    user_entry_t::unusable(_) => None,
                })
                .collect()
        };
        for (uid, slot) in slots {
            let slot = lock(&slot)?;
            match slot.state {
                slot_state_t::quarantined(reason) => quarantined.push((uid, reason)),
                slot_state_t::contained => contained.push(uid),
                slot_state_t::normal => {}
            }
            if !slot.pending.is_empty() || slot.anchor_failed {
                pending.push(uid);
            }
            if slot.journal.reserve_bytes() < ReserveBytes {
                depleted.push(uid);
            }
        }
        quarantined.sort_by_key(|(uid, _)| *uid);
        // One lock at a time, in lock order.
        let policy = *lock(&self.policy)?;
        let (anchor_mode, anchored_policy) = {
            let anchor = lock(&self.anchor)?;
            (anchor.mode(), anchor.map().policy)
        };
        let ledger_failed = lock(&self.ledger)?.failed();
        Ok(rollback_status_t {
            anchor_mode,
            run_decision: self.run_decision,
            quarantined,
            unusable,
            contained,
            open_ceremonies: Vec::new(),
            pending_restrictions: pending,
            ledger_failed,
            depleted_reserves: depleted,
            policy_pending: policy != anchored_policy,
            policy,
        })
    }

    /// Orderly shutdown (ADR 0006 §3): once everything owed is durable — every pending
    /// restrictive write, every unanchored record, the policy state in force — the run
    /// marker becomes `clean`, in the same slot write as the policy state, so the next
    /// start resumes without the ledger. With anything still owed, or a failed ledger,
    /// the marker stays as it is and `restriction_not_durable` is reported.
    pub fn shutdown(&self) -> service_result_t<()> {
        let slots: Vec<Arc<Mutex<user_slot_t>>> = lock(&self.users)?
            .values()
            .filter_map(|entry| match entry {
                user_entry_t::usable(slot) => Some(Arc::clone(slot)),
                user_entry_t::unusable(_) => None,
            })
            .collect();
        let mut clean = !lock(&self.ledger)?.failed();
        for slot in slots {
            let mut slot = lock(&slot)?;
            let owed = !slot.pending.is_empty() || slot.anchor_failed;
            if owed && self.flush_pending(&mut slot).is_err() {
                clean = false;
            }
        }
        if !clean {
            return Err(not_durable());
        }
        let policy = lock(&self.policy)?;
        let policy_now = *policy;
        let mut anchor = lock(&self.anchor)?;
        anchor
            .update(self.io.as_ref(), |map| {
                map.policy = policy_now;
                if let Some(marker) = &mut map.run_marker {
                    marker.state = run_state_t::clean;
                }
            })
            .map_err(|_| not_durable())
    }

    /// `lineage_head` (`service:contractd`, `service:policyd`): one lineage from a
    /// consistent snapshot. `None` if the user has no journal. Refused while the
    /// ledger cannot be written (no eligible decision is served, ADR 0006 §3) and
    /// while the user's last record is unanchored (M2-006-A-R01).
    pub fn lineage_head(
        &self,
        caller: caller_role_t,
        owner_uid: u32,
        lineage_id: &digest_t,
    ) -> service_result_t<Option<lineage_view_t>> {
        require(caller, &[Contractd, Policyd])?;
        if lock(&self.ledger)?.failed() {
            return Err(storage_unavailable());
        }
        let Some(slot) = self.slot_any(owner_uid)? else {
            return Ok(None);
        };
        let mut slot = lock(&slot)?;
        if slot.journal.failed() {
            return Err(storage_unavailable());
        }
        self.anchored_for_reading(&mut slot)?;
        Ok(Some(self.view(&slot, lineage_id)?))
    }

    fn view(&self, slot: &user_slot_t, lineage_id: &digest_t) -> service_result_t<lineage_view_t> {
        let lineages = &slot.lineages;
        let lineage = lineages.lineage(lineage_id);
        let head = |commit_id: &digest_t| {
            lineages.commit(commit_id).map(|entry| head_view_t {
                commit_id: *commit_id,
                placement: entry.placement,
                state: entry.payload.result_state.clone(),
            })
        };
        let trunk_head = lineage
            .and_then(|lineage| lineage.trunk_head.as_ref())
            .and_then(head);
        let temporary_heads: Vec<head_view_t> = lineage
            .map(|lineage| lineage.branches.keys().filter_map(head).collect())
            .unwrap_or_default();
        let mut origins = BTreeMap::new();
        let mut ended = BTreeMap::new();
        let mut ended_sessions = BTreeSet::new();
        let live = trunk_head.iter().chain(temporary_heads.iter());
        for grant in live.flat_map(|head| head.state.grants.iter()) {
            if let Some(reason) = lineages.ended(&grant.grant_id) {
                ended.insert(grant.grant_id, reason);
            }
            if let Some(session_id) = &grant.lifetime.session_id {
                if lineages.session_ended(session_id) {
                    ended_sessions.insert(*session_id);
                }
            }
            let Some(entry) = lineages.grant(&grant.grant_id) else {
                continue;
            };
            let Some(commit) = lineages.commit(&entry.commit_id) else {
                continue;
            };
            let resolution = slot.directory.resolve(&commit.payload.key_id);
            origins.insert(
                grant.grant_id,
                grant_origin_t {
                    commit_id: entry.commit_id,
                    key_id: commit.payload.key_id,
                    policy_epoch: commit.payload.policy.epoch,
                    key_invalidated: resolution
                        .as_ref()
                        .and_then(|resolution| resolution.invalidated),
                    key_unknown: resolution.is_none(),
                },
            );
        }
        let states = trunk_head
            .iter()
            .chain(temporary_heads.iter())
            .map(|head| &head.state);
        let occupying_network = lineages
            .occupying_network_grants(states)
            .into_iter()
            .collect();
        let policy = *lock(&self.policy)?;
        let user_floor = lock(&self.anchor)?
            .map()
            .users
            .get(&slot.uid)
            .map_or(0, |entry| entry.min_valid_policy_epoch);
        Ok(lineage_view_t {
            owner_uid: slot.uid,
            owner_user_id: slot.user_id,
            user_state: slot.user_state(),
            journal_seq: slot.journal.record_count(),
            lineage_id: *lineage_id,
            trunk_head,
            temporary_heads,
            authoritative_subject: lineage
                .and_then(|lineage| lineage.authoritative_subject.clone()),
            origins,
            ended,
            ended_sessions,
            occupying_network,
            policy_epoch: policy.epoch,
            min_valid_policy_epoch: policy.min_valid_epoch.max(user_floor),
            emergency_restriction: policy.emergency_restriction,
        })
    }

    /// Bytes allocated to a user's restrictive reserve (diagnostics and tests).
    pub fn reserve_bytes(&self, owner_uid: u32) -> service_result_t<u64> {
        let Some(slot) = self.slot_any(owner_uid)? else {
            return Err(authority(authority_code_t::key_not_certified));
        };
        let slot = lock(&slot)?;
        Ok(slot.journal.reserve_bytes())
    }

    /// The standing of a grant id in a user's index (diagnostics and tests).
    pub fn standing(&self, owner_uid: u32, grant_id: &random_id_t) -> service_result_t<standing_t> {
        let Some(slot) = self.slot_any(owner_uid)? else {
            return Ok(standing_t::unknown);
        };
        let slot = lock(&slot)?;
        Ok(slot.lineages.standing(grant_id))
    }

    /// The anchor's entry for a user (diagnostics and tests).
    pub fn anchored_user(&self, owner_uid: u32) -> service_result_t<Option<anchored_user_t>> {
        Ok(lock(&self.anchor)?.map().users.get(&owner_uid).copied())
    }

    /// The anchor's slot counter (diagnostics and tests).
    pub fn anchor_counter(&self) -> service_result_t<u64> {
        Ok(lock(&self.anchor)?.counter())
    }

    /// The owner UID the store belongs to.
    pub fn owner(&self) -> u32 {
        self.owner
    }
}

/// registryd's identity operations in the role `service:identityd`: the journal an
/// in-process identityd runs over (service IPC §3; M2-007 puts a socket between).
#[derive(Clone)]
pub struct registry_identity_store_t {
    pub registry: Arc<registry_t>,
}

impl identity_journal_t for registry_identity_store_t {
    fn append_identity(
        &mut self,
        owner_uid: u32,
        kind: object_kind_t,
        object: &[u8],
    ) -> service_result_t<append_receipt_t> {
        self.registry
            .append_identity(Identityd, owner_uid, kind, object)
    }

    fn identity_state(&self, owner_uid: u32, after: u64) -> service_result_t<journal_page_t> {
        self.registry.identity_state(Identityd, owner_uid, after)
    }

    fn journal_users(&self) -> service_result_t<Vec<u32>> {
        self.registry.journal_users(Identityd)
    }
}
