//! registryd: the per-user journals, the global anchor, the restriction ledger, their
//! derived indexes and the operations of service IPC §3 that M2-005 and M2-006 stages
//! A–B2 implement (`append_identity`, `identity_state`, `admit`, `lineage_head`,
//! `restrict`, `rollback_status`, `set_policy_epoch`, `set_emergency_restriction`,
//! `time_sample`, `reserve_use`, `complete_use`, `session_event`,
//! `confirm_enforcement`, `history`).
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
//! **Trusted time (ADR 0007 §5, §8 option B, erratum E6; stage B1).** Registry time
//! is trusted time ([`crate::clock`]). Every IPC operation samples the clock right
//! after its role check: an offset increase is ledgered before the clock adopts it,
//! a sample more than 300 s behind is recorded once per episode as a
//! `clock_anomaly` in the journal of the user the operation concerns, and the
//! anchor's floor is refreshed at least every 60 s. Every anchor write publishes the
//! floor. Admission checks the commit-timestamp rule against trusted now and records
//! the observed time; `time_sample` takes policyd's offset increases. A new boot ends
//! every live temporary branch (`branch_terminated`). Lock order with the clock:
//! `users` → user → `policy` → `clock` → `ledger` → `anchor`; the clock lock is never
//! held while another lock is taken, except the ledger's for a raise. A decision that
//! depends on the present (admission, a reservation) takes trusted now from a second
//! sample under the user lock, after the flush of what the slot owed (M2-006-B2-01).
//!
//! **Lifecycle and consumption (ADR 0007 §§3, 4, 6, 7; ADR 0008 §2; erratum E7;
//! stage B2).** `reserve_use` (policyd only, after its full evaluation) re-checks the
//! rules this snapshot decides under the user lock, consumes the one-shot grant in
//! memory and writes `use_reserved` through the restrictive path; `complete_use`
//! records the outcome once. Every start reports a reservation without an outcome as
//! `consumed_uncertain`. `session_event` records a start and ends a session for good
//! (`session_ended`, or a `session_event` end when no grant is bound to it); a new
//! boot ends every session a journal knows (`sessions_ended`, M2-006-B2-03).
//! `confirm_enforcement` records an outcome per domain; a commit's status counts only
//! this run's outcomes. An evidence record (session start, use outcome, enforcement
//! outcome) whose write fails leaves an `audit_gap` through the restrictive path, and
//! a contained start writes one per contained user. `restrict` takes policyd's
//! `identity_mismatch` once per lineage and executable identity. No writer persists
//! a record the strict decoder would not accept back (`malformed_request`,
//! M2-006-B2-02). `history` pages the durable records for the owner's trusted client
//! and admin.
//!
//! Limits (stage B2): in-process composition; the socket transport and process
//! boundary are M2-007. The recovery ceremony, `recover_user`, `replace_identity` and
//! deletion cleanup are stage C (a `pending` tombstone keeps its user denied; a
//! `deleted` tombstone's files are quarantined and never loaded, not yet removed).
//! Each lost evidence record takes one `audit_gap` from the restrictive reserve.

use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use cham3_contract_format::hash::digest_t;
use cham3_contract_format::schema::{
    commit_payload_t, contract_state_t, grant_t, invalidation_reason_t, random_id_t, subject_t,
};
use cham3_contract_format::{key_resolver_t, object_kind_t, payload_t, verify_object};
use cham3_identityd::caller::{caller_role_t, service_name_t};
use cham3_identityd::directory::{identity_directory_t, identity_effect_t, journal_record_t};
use cham3_identityd::index::{
    check_commit_authority, check_signing_window, user_record_t, user_state_t, utc_seconds,
    utc_text,
};
use cham3_identityd::journal::{
    MaxPageRecords, append_receipt_t, identity_journal_t, journal_entry_t, journal_page_t,
};
use cham3_identityd::outcome::{
    authority, authority_code_t, ipc, ipc_code_t, rule, rule_code_t, service_error_t,
    service_result_t,
};

use crate::anchor::{
    anchor_map_t, anchor_mode_t, anchor_start_t, anchor_t, anchored_state_t, anchored_user_t,
    policy_state_t, run_marker_t, run_state_t, tombstone_state_t,
};
use crate::clock::{
    FloorIntervalSeconds, anomaly_state_t, merge_floor, time_floor_t, time_source_t, time_view_t,
    trusted_clock_t,
};
use crate::journal::{journal_file_t, journal_io_t, journal_store_t, store_error_t};
use crate::ledger::{
    boot_id_source_t, entry_kind_t, kernel_boot_id_t, ledger_entry_t, ledger_t,
    parse_policy_payload, parse_time_payload, policy_payload, previous_ledger_t, time_payload,
};
use crate::lineage::{admission_t, admitted_at_t, end_reason_t, lineage_index_t, standing_t};
use crate::record::{
    enforcement_domain_t, enforcement_result_t, journal_defect_t, lifecycle_record_t,
    lost_evidence_t, parse_journal_prefix, placement_t, record_body_t, registry_event_t,
    registry_record_t, session_change_t, use_outcome_t,
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
    /// The clocks cannot be sampled: trusted time cannot be established.
    clock_unavailable,
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
    /// Every registry event the journal holds (the same purpose).
    events: Vec<registry_event_t>,
    /// The clock-anomaly episode this run last recorded for the user (0: none).
    anomaly_episode: u64,
    /// Every durable record except identity objects, in journal order (`history`).
    history: Vec<history_item_t>,
    /// The latest enforcement outcome per commit and domain written in this run
    /// (ADR 0007 erratum E7.2): a commit's status counts only this run's reports.
    enforcement: BTreeMap<(digest_t, enforcement_domain_t), enforcement_result_t>,
    /// Restrictive records denied in memory whose durable write is still owed.
    pending: Vec<pending_write_t>,
    /// The journal's last record is durable but its anchor update failed: the user is
    /// denied, reads included, and nothing is appended, until a retry anchors it (or
    /// the next start rolls the anchor forward).
    anchor_failed: bool,
}

/// One durable record as `history` serves it: a commit by its id (the index holds
/// its envelope), anything else as it was written.
#[derive(Clone, Debug, PartialEq, Eq)]
enum history_body_t {
    commit {
        commit_id: digest_t,
        lineage_id: digest_t,
    },
    record(record_body_t),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct history_item_t {
    sequence: u64,
    time: String,
    body: history_body_t,
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
    /// Trusted and observed time when the commit was admitted (ADR 0007 §8 option B).
    pub admitted: admitted_at_t,
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
    /// The shared trusted-time offset of this boot and the observed counter's offset
    /// (ADR 0007 §5 rule 4, §8 option B).
    pub time: time_view_t,
}

/// Receipt of a one-shot reservation (ADR 0007 §4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct reservation_receipt_t {
    pub reservation_id: random_id_t,
    pub journal_seq: u64,
}

/// One record of a `history` page (ADR 0008 §2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct history_record_t {
    pub sequence: u64,
    /// Registry time of the append (a claim, never ordering or freshness).
    pub time: String,
    pub body: record_body_t,
    /// For a commit: this run's enforcement outcome per domain (ADR 0007 erratum
    /// E7.2); a domain without one is pending.
    pub enforcement: Vec<(enforcement_domain_t, enforcement_result_t)>,
}

/// `history`: one page (service IPC §2: at most 16 records or one signed object).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct history_page_t {
    pub records: Vec<history_record_t>,
    /// Cursor of the next page (a journal sequence), if any.
    pub next: Option<u64>,
    /// Restrictive records in force but not yet durable: not in the history yet.
    pub pending_restrictions: u64,
    /// The policy state in force: commits below its minimum valid epoch are invalid
    /// (ADR 0007 erratum E7.4: `policy_invalidated` is not a separate record).
    pub policy: policy_state_t,
}

/// The clock in `rollback_status` (ADR 0007 §5; erratum E6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct clock_status_t {
    pub boot_id: String,
    pub trusted_now: u64,
    pub observed_now: u64,
    /// The current or last clock-anomaly episode.
    pub anomaly: anomaly_state_t,
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
    /// The failed ledger's validity file is still intact or the `unverifiable` run
    /// marker is not yet written: both are retried by every operation; until one
    /// succeeds, a same-boot restart would trust the failed ledger (M2-006-A-R2-03).
    pub unverifiable_owed: bool,
    /// Users whose restrictive reserve is not full (R03; reported here until the
    /// status operations of stage B).
    pub depleted_reserves: Vec<u32>,
    /// The policy state in force (`policy`) is not yet in the anchor: a restrictive
    /// change whose anchor update failed, published by the next successful anchor
    /// write.
    pub policy_pending: bool,
    pub policy: policy_state_t,
    pub clock: clock_status_t,
}

/// registryd.
pub struct registry_t {
    store: journal_store_t,
    config: registry_config_t,
    io: Box<dyn journal_io_t>,
    time_source: Box<dyn time_source_t>,
    /// Trusted time of this boot (ADR 0007 §5).
    clock: Mutex<trusted_clock_t>,
    users: Mutex<BTreeMap<u32, user_entry_t>>,
    anchor: Mutex<anchor_t>,
    ledger: Mutex<ledger_t>,
    /// The policy state in force (restrictive changes apply here first).
    policy: Mutex<policy_state_t>,
    run_decision: run_decision_t,
    /// This run's id (the ledger's and the run marker's): enforcement outcomes count
    /// only in the run that wrote them (ADR 0007 erratum E7.2).
    run_id: random_id_t,
    owner: u32,
    /// The ledger failed but the run marker could not be written `unverifiable`:
    /// the marker write is still owed and retried by every later operation
    /// (M2-006-A-R2-03).
    unverifiable_owed: Mutex<bool>,
}

fn storage_unavailable() -> service_error_t {
    ipc(ipc_code_t::storage_unavailable)
}

/// Publishes the trusted-time floor in an anchor image (erratum E6): never below the
/// one the anchor already holds.
fn publish_floor(map: &mut anchor_map_t, floor: Option<time_floor_t>) {
    if let Some(floor) = floor {
        map.time_floor = Some(merge_floor(map.time_floor.take(), floor));
    }
}

/// What an operation's clock sample found (ADR 0007 §5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct time_observation_t {
    /// The sample was taken: trusted now is current, not only the last known value.
    fresh: bool,
    /// The sample is more than 300 s behind trusted time: its anomaly episode.
    episode: Option<u64>,
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
const FixtureConsumer: caller_role_t = caller_role_t::fixture_consumer;

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

/// Layers 1–7 of a commit envelope against one user's snapshot; `trusted_now` at
/// admission (the commit-timestamp rule), `None` at replay.
fn check_commit_envelope(
    uid: u32,
    directory: &identity_directory_t,
    lineages: &lineage_index_t,
    envelope: &[u8],
    trusted_now: Option<i64>,
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
    let admission = lineages.check_commit(&verified.object_id, &payload, trusted_now)?;
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
                observed,
            } => {
                let (admission, payload) = check_commit_envelope(
                    self.uid,
                    &self.directory,
                    &self.lineages,
                    &envelope,
                    None,
                )?;
                if admission.commit_id != commit_id
                    || admission.lineage_id != lineage_id
                    || admission.placement != placement
                {
                    return Err(storage_unavailable());
                }
                let trusted = utc_seconds(&record.time).ok_or(storage_unavailable())?;
                self.lineages.apply_commit(
                    &admission,
                    payload,
                    envelope,
                    record.sequence,
                    admitted_at_t { trusted, observed },
                );
                self.history.push(history_item_t {
                    sequence: record.sequence,
                    time: record.time,
                    body: history_body_t::commit {
                        commit_id,
                        lineage_id,
                    },
                });
            }
            record_body_t::event(event) => {
                self.lineages.check_event(&event)?;
                self.lineages.apply_event(&event);
                self.note_durable(
                    record.sequence,
                    record.time,
                    record_body_t::event(event.clone()),
                );
                self.events.push(event);
            }
            record_body_t::lifecycle(lifecycle) => {
                self.lineages.check_record(&lifecycle, &record.time)?;
                self.lineages.apply_record(&lifecycle);
                self.note_durable(
                    record.sequence,
                    record.time,
                    record_body_t::lifecycle(lifecycle.clone()),
                );
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
                self.note_durable(
                    record.sequence,
                    record.time,
                    record_body_t::user(user_record.clone()),
                );
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

    /// Keeps a durable record for `history` (identity objects and commits are kept
    /// elsewhere).
    fn note_durable(&mut self, sequence: u64, time: String, body: record_body_t) {
        self.history.push(history_item_t {
            sequence,
            time,
            body: history_body_t::record(body),
        });
    }

    /// The sequence of a durable record equal to `body`, if the journal holds one.
    fn durable_sequence(&self, body: &record_body_t) -> Option<u64> {
        self.history
            .iter()
            .find(|item| item.body == history_body_t::record(body.clone()))
            .map(|item| item.sequence)
    }

    /// Whether the journal already holds the record of a ledger entry (journaled
    /// before the crash): the same identity object, user record or lifecycle record.
    fn holds(&self, body: &record_body_t) -> bool {
        match body {
            record_body_t::lifecycle(lifecycle) => self.lifecycle_records.contains(lifecycle),
            record_body_t::event(event) => self.events.contains(event),
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
        events: Vec::new(),
        anomaly_episode: 0,
        history: Vec::new(),
        enforcement: BTreeMap::new(),
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

/// Rule 5 of ADR 0007 §1 as registryd's snapshot decides it for a reservation
/// (erratum E7.1): started — a start later than the admission is reached only once
/// as much observed time has passed (§8 option B) — not expired, and a bound
/// session not ended. Session liveness is policyd's.
fn lifetime_open(
    lineages: &lineage_index_t,
    grant: &grant_t,
    admitted: admitted_at_t,
    now: i64,
    observed_now: u64,
) -> bool {
    let lifetime = &grant.lifetime;
    let Some(start) = utc_seconds(&lifetime.effective_start) else {
        return false;
    };
    if now < start {
        return false;
    }
    if let Some(needed) = start
        .checked_sub(admitted.trusted)
        .filter(|needed| *needed > 0)
    {
        let elapsed = observed_now
            .checked_sub(admitted.observed)
            .and_then(|elapsed| i64::try_from(elapsed).ok());
        if elapsed.is_none_or(|elapsed| elapsed < needed) {
            return false;
        }
    }
    if let Some(expiration) = &lifetime.expiration {
        match utc_seconds(expiration) {
            Some(end) if now < end => {}
            _ => return false,
        }
    }
    if let Some(session_id) = &lifetime.session_id {
        if lineages.session_ended(session_id) {
            return false;
        }
    }
    true
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
    /// Trusted time starts from a sample of `time_source` and the persisted floors:
    /// the anchor's, and on a same-boot ledger replay the ledger's time entries (ADR
    /// 0007 §5 rules 3 and 6). After a reboot every live temporary branch ends.
    pub fn open(
        root: &Path,
        owner: u32,
        config: registry_config_t,
        io: Box<dyn journal_io_t>,
        time_source: Box<dyn time_source_t>,
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

        // Trusted time (ADR 0007 §5): the anchor's floor and, when the previous run's
        // ledger is trusted, its time entries; the maximum of this boot wins.
        let mut floors: Vec<time_floor_t> = anchor.map().time_floor.iter().cloned().collect();
        floors.extend(
            replay_entries
                .iter()
                .filter(|entry| entry.kind == entry_kind_t::time)
                .filter_map(|entry| parse_time_payload(&entry.payload)),
        );
        let sample = time_source
            .sample()
            .ok_or(registry_start_error_t::clock_unavailable)?;
        let clock = trusted_clock_t::start(&boot_id, sample, &floors)
            .ok_or(registry_start_error_t::clock_unavailable)?;
        // The boot of the anchor's last run: an unverifiable run's audit gap names it.
        let previous_boot = anchor
            .map()
            .run_marker
            .as_ref()
            .map(|marker| marker.boot_id.clone());
        // A reboot since the anchor's last run (§5 rule 6). An unverifiable run's
        // containment ends the branches anyway.
        let new_boot = anchor
            .map()
            .run_marker
            .as_ref()
            .is_some_and(|marker| marker.boot_id != boot_id);

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
        let mut anchor_quarantined: Vec<u32> = Vec::new();
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
                            } else if anchor_invalid || anchor_lost {
                                anchor_quarantined.push(uid);
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
            time_source,
            clock: Mutex::new(clock),
            users: Mutex::new(users),
            anchor: Mutex::new(anchor),
            ledger: Mutex::new(ledger),
            policy: Mutex::new(policy),
            run_decision: decision,
            run_id,
            owner,
            unverifiable_owed: Mutex::new(false),
        };
        // Roll the anchor forward for the durable but unanchored records (§4), and
        // make the quarantine of every user the unusable anchor quarantined durable
        // in the fresh anchor, so that it holds at every later start
        // (M2-006-A-R2-01).
        for uid in roll_forward.into_iter().chain(anchor_quarantined) {
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
        // Containment of every user after an unverifiable run (§3); otherwise, after
        // a reboot, the end of every live temporary branch (ADR 0007 §5 rule 6).
        if decision == run_decision_t::contained {
            registry.contain_everyone(previous_boot.as_deref());
        } else if new_boot {
            registry.terminate_branches();
        }
        // After a reboot every session ends, contained or not (ADR 0007 §6).
        if new_boot {
            registry.end_sessions();
        }
        // Reservations without a reported outcome are uncertain (ADR 0007 §7).
        registry.reconcile_reservations();
        // The policy state and the run marker, durable before anything is served. A
        // ledger that failed during the start (a re-journaled intent that could not
        // enter this run's ledger) has already made the anchor hold this run
        // unverifiable (`mark_run_unverifiable`): this write then publishes
        // `unverifiable`, never `running` (M2-006-A-R2-02, R3-01).
        let marker = run_marker_t {
            run_id,
            boot_id,
            state: run_state_t::running,
        };
        let policy_now = *lock(&registry.policy)
            .map_err(|_| registry_start_error_t::store(store_error_t::io))?;
        let floor = registry.floor();
        let mut anchor =
            lock(&registry.anchor).map_err(|_| registry_start_error_t::store(store_error_t::io))?;
        anchor
            .update(registry.io.as_ref(), |map| {
                map.policy = policy_now;
                map.run_marker = Some(marker);
                publish_floor(map, floor);
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
            record_body_t::event(event) => {
                if slot.lineages.check_event(event).is_err() {
                    return;
                }
                slot.lineages.apply_event(event);
            }
            record_body_t::commit_admitted { .. } => return,
        }
        let _ = self.restrictive_write(&mut slot, record.time.clone(), record.body.clone());
    }

    /// Contains every usable, not yet contained or quarantined user (§3), and records
    /// for each one that records of the unverifiable run of boot `previous_boot` may
    /// be missing (`audit_gap`, ADR 0007 erratum E7.3).
    fn contain_everyone(&self, previous_boot: Option<&str>) {
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
                time.clone(),
                record_body_t::user(user_record_t::containment),
            );
            if let Some(boot_id) = previous_boot {
                let _ = self.restrictive_write(
                    &mut slot,
                    time,
                    record_body_t::event(registry_event_t::audit_gap {
                        boot_id: boot_id.to_owned(),
                        lost: lost_evidence_t::unverifiable_run,
                    }),
                );
            }
        }
    }

    /// Startup reconciliation (ADR 0007 §7): every reservation of a usable,
    /// unquarantined user without a reported outcome gets `consumed_uncertain`
    /// through the restrictive path. Its grant stays consumed (§4 option A); a
    /// completion reported after this is refused.
    fn reconcile_reservations(&self) {
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
            if let slot_state_t::quarantined(_) = slot.state {
                continue;
            }
            for reservation_id in slot.lineages.open_reservations() {
                let Ok(time) = self.now_text() else {
                    continue;
                };
                let event = registry_event_t::consumed_uncertain { reservation_id };
                slot.lineages.apply_event(&event);
                let _ = self.restrictive_write(&mut slot, time, record_body_t::event(event));
            }
        }
    }

    /// A new boot (ADR 0007 §5 rule 6): every live temporary branch of every usable,
    /// normal user ends, in memory at once, then durably through the restrictive path
    /// (`branch_terminated`). A failure leaves the record pending, retried by every
    /// later operation of the user, and in the ledger for a same-boot restart.
    fn terminate_branches(&self) {
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
        let boot_id = match lock(&self.clock) {
            Ok(clock) => clock.boot_id().to_owned(),
            Err(_) => return,
        };
        for slot in slots {
            let Ok(mut slot) = lock(&slot) else {
                continue;
            };
            if slot.state != slot_state_t::normal || !slot.lineages.has_live_branches() {
                continue;
            }
            let Ok(time) = self.now_text() else {
                continue;
            };
            slot.lineages.end_branches();
            let _ = self.restrictive_write(
                &mut slot,
                time,
                record_body_t::event(registry_event_t::branch_terminated {
                    boot_id: boot_id.clone(),
                }),
            );
        }
    }

    /// A new boot (ADR 0007 §6; M2-006-B2-03): every session that the journal of a
    /// usable, unquarantined user knows — started by a `session_event` or bound to an
    /// admitted grant, live branch or not — ends, in memory at once, then durably
    /// through the restrictive path (`sessions_ended`, one record per user). A
    /// failure leaves the record pending, retried by every later operation of the
    /// user, and in the ledger for a same-boot restart. A contained user's sessions
    /// end too; a quarantined user's journal is left as found.
    fn end_sessions(&self) {
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
        let boot_id = match lock(&self.clock) {
            Ok(clock) => clock.boot_id().to_owned(),
            Err(_) => return,
        };
        for slot in slots {
            let Ok(mut slot) = lock(&slot) else {
                continue;
            };
            if matches!(slot.state, slot_state_t::quarantined(_))
                || !slot.lineages.has_live_sessions()
            {
                continue;
            }
            let Ok(time) = self.now_text() else {
                continue;
            };
            let event = registry_event_t::sessions_ended {
                boot_id: boot_id.clone(),
            };
            slot.lineages.apply_event(&event);
            let _ = self.restrictive_write(&mut slot, time, record_body_t::event(event));
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
        self.retry_unverifiable();
        self.observe_time();
        Ok(*lock(&self.policy)?)
    }

    /// Users whose journal is unusable in this run (unreadable, defective or
    /// quarantined without a usable slot), with the reason. Quarantined users with a
    /// readable journal are in `rollback_status`.
    pub fn unusable_users(&self) -> service_result_t<Vec<(u32, unusable_reason_t)>> {
        self.retry_unverifiable();
        self.observe_time();
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
        self.retry_unverifiable();
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

    /// Trusted now as record time: the value at the latest observation.
    fn now_text(&self) -> service_result_t<String> {
        utc_text(self.trusted_now()?).ok_or(storage_unavailable())
    }

    fn trusted_now(&self) -> service_result_t<i64> {
        let now = lock(&self.clock)?.now().ok_or(storage_unavailable())?;
        i64::try_from(now).map_err(|_| storage_unavailable())
    }

    /// Trusted now for a decision that depends on the present (admission, an expiry
    /// record): only when this operation's own sample was taken.
    fn fresh_now(&self, observed: &time_observation_t) -> service_result_t<i64> {
        if !observed.fresh {
            return Err(storage_unavailable());
        }
        self.trusted_now()
    }

    /// Trusted now for a decision under the user lock, from a sample taken after
    /// the lock was acquired and after everything that may block before the
    /// decision (`ready` flushing what the slot owes), immediately before it
    /// (M2-006-B2-01): the operation's first sample may be older by the time spent
    /// waiting for the lock or for those writes. This sample is observed like any
    /// other (offset increase ledgered, anomaly recorded); when it fails, or leaves
    /// an anomaly record owed, the decision is refused and nothing is consumed.
    fn decision_now(&self, slot: &mut user_slot_t) -> service_result_t<i64> {
        let observed = self.observe_time();
        self.record_anomaly(slot, &observed);
        if !slot.pending.is_empty() {
            return Err(not_durable());
        }
        self.fresh_now(&observed)
    }

    /// The floor this clock persists now (erratum E6).
    fn floor(&self) -> Option<time_floor_t> {
        lock(&self.clock).ok()?.floor()
    }

    /// Samples the clock (ADR 0007 §5 rules 2 and 3). An offset increase is appended
    /// to the ledger before the clock adopts it (a raise that cannot be ledgered is
    /// not relied on, and a failed ledger append makes the run unverifiable); the
    /// anomaly episode opens or ends; the anchor's floor is refreshed when it is 60 s
    /// old or older. Every IPC operation calls this right after its role check.
    fn observe_time(&self) -> time_observation_t {
        let unavailable = time_observation_t {
            fresh: false,
            episode: None,
        };
        let Some(sample) = self.time_source.sample() else {
            return unavailable;
        };
        let mut ledger_failed = false;
        let (episode, boottime) = {
            let Ok(mut clock) = lock(&self.clock) else {
                return unavailable;
            };
            let Some(assessment) = clock.assess(sample) else {
                return unavailable;
            };
            let mut raised = false;
            if let Some(offset) = assessment.raise {
                let payload = clock.floor_with(offset).as_ref().and_then(time_payload);
                if let (Some(payload), Ok(mut ledger)) = (payload, lock(&self.ledger)) {
                    match ledger.append(self.io.as_ref(), entry_kind_t::time, 0, &payload) {
                        Ok(()) => raised = true,
                        Err(store_error_t::io) => ledger_failed = true,
                        Err(_) => {}
                    }
                }
            }
            let before = clock.anomaly().mark();
            let episode = clock.observe(sample, raised);
            // An episode that opened or ended is persisted at once, so that a restart
            // in this boot resumes it (erratum E6; M2-006-B1-R02). The anchor floor
            // publishes it too.
            if clock.anomaly().mark() != before {
                let payload = clock.floor().as_ref().and_then(time_payload);
                if let (Some(payload), Ok(mut ledger)) = (payload, lock(&self.ledger)) {
                    if ledger.append(self.io.as_ref(), entry_kind_t::time, 0, &payload)
                        == Err(store_error_t::io)
                    {
                        ledger_failed = true;
                    }
                }
            }
            (episode, clock.boottime())
        };
        if ledger_failed {
            self.mark_run_unverifiable();
        }
        self.refresh_floor(boottime);
        time_observation_t {
            fresh: true,
            episode,
        }
    }

    /// Writes the floor when the anchor's is 60 s old or older, or of another boot
    /// (§5 rule 3). A failure is ignored: the ledger holds every offset increase.
    fn refresh_floor(&self, boottime: u64) {
        let Some(boot_id) = lock(&self.clock)
            .ok()
            .map(|clock| clock.boot_id().to_owned())
        else {
            return;
        };
        let stale = match lock(&self.anchor) {
            Ok(anchor) => anchor.map().time_floor.as_ref().is_none_or(|floor| {
                floor.boot_id != boot_id
                    || boottime.saturating_sub(floor.boottime) >= FloorIntervalSeconds
            }),
            Err(_) => false,
        };
        if !stale {
            return;
        }
        let Ok(policy) = lock(&self.policy) else {
            return;
        };
        let policy_now = *policy;
        let floor = self.floor();
        let Ok(mut anchor) = lock(&self.anchor) else {
            return;
        };
        let _ = anchor.update(self.io.as_ref(), |map| {
            map.policy = policy_now;
            publish_floor(map, floor);
        });
    }

    /// Records a clock anomaly for the user the operation concerns, once per episode
    /// (erratum E6): a narrowing `clock_anomaly` record through the restrictive path.
    /// "Once" holds across same-boot restarts: the episode is resumed from the
    /// persisted mark, and a record of this boot and episode that the journal already
    /// holds (replayed or written this run) counts (M2-006-B1-R02). A quarantined
    /// user's journal is left as found.
    fn record_anomaly(&self, slot: &mut user_slot_t, observed: &time_observation_t) {
        let Some(episode) = observed.episode else {
            return;
        };
        if let slot_state_t::quarantined(_) = slot.state {
            return;
        }
        let event = {
            let Ok(clock) = lock(&self.clock) else {
                return;
            };
            let Some(trusted) = clock.now() else {
                return;
            };
            let boot_id = clock.boot_id().to_owned();
            let reported = slot.events.iter().any(|event| {
                matches!(
                    event,
                    registry_event_t::clock_anomaly {
                        boot_id: recorded_boot,
                        episode: recorded,
                        ..
                    } if *recorded_boot == boot_id && *recorded == episode
                )
            });
            if slot.anomaly_episode == episode || reported {
                return;
            }
            registry_event_t::clock_anomaly {
                boot_id,
                episode,
                trusted,
                behind_seconds: clock.anomaly().behind,
            }
        };
        let Ok(time) = self.now_text() else {
            return;
        };
        slot.anomaly_episode = episode;
        let _ = self.restrictive_write(slot, time, record_body_t::event(event));
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
        let floor = self.floor();
        let mut anchor = lock(&self.anchor)?;
        entry.min_valid_policy_epoch = anchor
            .map()
            .users
            .get(&uid)
            .map_or(0, |existing| existing.min_valid_policy_epoch);
        match anchor.update(self.io.as_ref(), |map| {
            map.policy = policy_now;
            map.users.insert(uid, entry);
            publish_floor(map, floor);
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
        // Never a record that replay would refuse (M2-006-B2-02): nothing is
        // ledgered or queued.
        if !body.well_formed() {
            return Err(ipc(ipc_code_t::malformed_request));
        }
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
        let Ok(mut owed) = lock(&self.unverifiable_owed) else {
            return;
        };
        *owed = true;
        drop(owed);
        self.retry_unverifiable();
    }

    /// Retries what a failed ledger still owes (M2-006-A-R2-03): the invalidation of
    /// its validity file and the `unverifiable` run marker. Both are owed until they
    /// succeed. Every IPC operation of the registry retries them right after its role
    /// check, before anything else it does (M2-006-A-R3-01), so that the window in
    /// which a same-boot restart could trust the failed ledger closes at the first
    /// operation after storage recovers. The anchor holds the run unverifiable from
    /// the first failure on: any anchor write of this run, this retry's or another
    /// operation's, publishes the marker `unverifiable`. Lock order: ledger, then
    /// anchor; called with neither held.
    fn retry_unverifiable(&self) {
        if let Ok(mut ledger) = lock(&self.ledger) {
            ledger.retry_invalidation(self.io.as_ref());
        }
        let Ok(mut owed) = lock(&self.unverifiable_owed) else {
            return;
        };
        if !*owed {
            return;
        }
        let floor = self.floor();
        let Ok(mut anchor) = lock(&self.anchor) else {
            return;
        };
        anchor.hold_unverifiable();
        if anchor.publishes_unverifiable()
            || anchor
                .update(self.io.as_ref(), |map| publish_floor(map, floor))
                .is_ok()
        {
            *owed = false;
        }
    }

    /// Whether a failed ledger still owes its invalidation or the `unverifiable`
    /// marker (`rollback_status`).
    fn unverifiable_owed(&self) -> service_result_t<bool> {
        let ledger_owed = lock(&self.ledger)?.invalidation_owed();
        Ok(ledger_owed || *lock(&self.unverifiable_owed)?)
    }

    /// Makes what the slot owes durable, in order (ADR 0006 §§3, 6): a durable record
    /// whose anchor update failed is anchored first — nothing is appended beyond it
    /// until it is, so that a start finds at most one unanchored record (§4 rolls it
    /// forward) and nothing of the unanchored state is served — then every pending
    /// restrictive record is appended and anchored, one at a time. A failure
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
                record_body_t::event(event) => slot.events.push(event),
                record_body_t::commit_admitted { .. } => {}
            }
            if !matches!(
                pending.body,
                record_body_t::identity { .. } | record_body_t::commit_admitted { .. }
            ) {
                slot.note_durable(sequence, pending.time.clone(), pending.body.clone());
            }
            slot.pending.remove(0);
            last_sequence = sequence;
            // Each durable record is anchored before the next one is appended, so
            // that at most one record is ever unanchored, whatever fails next
            // (M2-006-A-R2-04). With the anchor update failed the record is durable
            // and unanchored: the user is denied (`anchor_failed`), the rest of the
            // queue stays owed, and every later operation of the user retries the
            // anchor first (or the next start rolls it forward).
            if self.anchor_slot(slot).is_err() {
                return Err(not_durable());
            }
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
        self.retry_unverifiable();
        let observed = self.observe_time();
        if kind == object_kind_t::commit {
            return Err(ipc(ipc_code_t::malformed_request));
        }
        if let Some(slot) = self.slot_any(owner_uid)? {
            let mut slot = lock(&slot)?;
            self.record_anomaly(&mut slot, &observed);
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
            events: Vec::new(),
            anomaly_episode: 0,
            history: Vec::new(),
            enforcement: BTreeMap::new(),
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
        self.retry_unverifiable();
        let observed = self.observe_time();
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
        self.record_anomaly(&mut slot, &observed);
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
        self.retry_unverifiable();
        self.observe_time();
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
        self.retry_unverifiable();
        let observed = self.observe_time();
        let slot = self
            .slot_any(owner_uid)?
            .ok_or(authority(authority_code_t::key_not_certified))?;
        let mut slot = lock(&slot)?;
        self.record_anomaly(&mut slot, &observed);
        match slot.state {
            slot_state_t::quarantined(_) => return Err(authority(authority_code_t::quarantined)),
            slot_state_t::contained => return Err(authority(authority_code_t::contained)),
            slot_state_t::normal => {}
        }
        self.ready(&mut slot)?;
        if !slot.pending.is_empty() {
            return Err(not_durable());
        }
        // Trusted now from this operation's own sample, taken after `ready` (ADR 0007
        // §5; M2-006-B2-01): the commit-timestamp rule (`timestamp_rule`, erratum E4)
        // and the signing window.
        let now = self.decision_now(&mut slot)?;
        let (admission, payload) = check_commit_envelope(
            owner_uid,
            &slot.directory,
            &slot.lineages,
            envelope,
            Some(now),
        )?;
        // (beyond the model) The signing key's certificate window must also contain
        // trusted now, not only the commit's own timestamp: an envelope signed in a
        // temporary key's window cannot be admitted after that window closed.
        // Admission-time only: replay reproduces historical admissions.
        let resolution = slot
            .directory
            .resolve(&payload.key_id)
            .ok_or(authority(authority_code_t::key_not_certified))?;
        check_signing_window(&resolution, now)?;
        let policy = *lock(&self.policy)?;
        if payload.policy.epoch != policy.epoch {
            return Err(ipc(ipc_code_t::stale_epoch));
        }
        if admission.new_lineage && slot.lineages.lineage_count() >= self.config.max_lineages {
            return Err(ipc(ipc_code_t::quota_exhausted));
        }
        let sequence = slot.journal.next_sequence().ok_or(storage_unavailable())?;
        // The record's time is trusted now; with the observed counter it is when the
        // commit was admitted (ADR 0007 §8 option B).
        let observed_now = lock(&self.clock)?
            .observed_now()
            .ok_or(storage_unavailable())?;
        let record = registry_record_t {
            sequence,
            time: utc_text(now).ok_or(storage_unavailable())?,
            body: record_body_t::commit_admitted {
                commit_id: admission.commit_id,
                lineage_id: admission.lineage_id,
                placement: admission.placement,
                envelope: envelope.to_vec(),
                observed: observed_now,
            },
        }
        .encode();
        // Not restrictive: the full reserve must exist before the append (R03).
        self.ordinary_append(&mut slot, &record)?;
        slot.lineages.apply_commit(
            &admission,
            payload,
            envelope.to_vec(),
            sequence,
            admitted_at_t {
                trusted: now,
                observed: observed_now,
            },
        );
        slot.history.push(history_item_t {
            sequence,
            time: utc_text(now).ok_or(storage_unavailable())?,
            body: history_body_t::commit {
                commit_id: admission.commit_id,
                lineage_id: admission.lineage_id,
            },
        });
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
        // Evaluation reports identity mismatches (ADR 0007 erratum E7.4).
        let mismatch = matches!(record, lifecycle_record_t::identity_mismatch { .. });
        if mismatch && caller != Policyd {
            return Err(ipc(ipc_code_t::role_denied));
        }
        // A record the strict decoder refuses (an identity mismatch naming an empty,
        // overlong or non-printable package identity) has no effect at all: nothing
        // is denied, ledgered or journaled, and the journal stays replayable
        // (M2-006-B2-02).
        if !record_body_t::lifecycle(record.clone()).well_formed() {
            return Err(ipc(ipc_code_t::malformed_request));
        }
        self.retry_unverifiable();
        let observed = self.observe_time();
        let slot = self
            .slot_any(owner_uid)?
            .ok_or(authority(authority_code_t::key_not_certified))?;
        let mut slot = lock(&slot)?;
        if let slot_state_t::quarantined(_) = slot.state {
            return Err(authority(authority_code_t::quarantined));
        }
        self.record_anomaly(&mut slot, &observed);
        // Once per lineage, package identity and executable hash: a mismatch the
        // journal holds, or still owes, is not written again.
        if mismatch {
            let body = record_body_t::lifecycle(record.clone());
            if slot.pending.iter().any(|pending| pending.body == body) {
                self.flush_pending(&mut slot)?;
            }
            if let Some(sequence) = slot.durable_sequence(&body) {
                return Ok(sequence);
            }
        }
        // The record's time is the last trusted value, also when this operation's own
        // sample failed: trusted time never decreases, so an expiration it already
        // shows is proven (a premature `expired` record stays `lifecycle_rule`), and a
        // clock failure never blocks a restriction.
        let time = self.now_text()?;
        slot.lineages.check_record(record, &time)?;
        // Denial first, whatever the writes do; the write joins the queue behind
        // whatever the slot owes, an unanchored record included (M2-006-A-R02).
        slot.lineages.apply_record(record);
        self.restrictive_write(&mut slot, time, record_body_t::lifecycle(record.clone()))
    }

    // ------------------------------------------------- lifecycle and consumption

    /// `reserve_use` (`service:policyd`; ADR 0007 §4 option A, erratum E7.1). policyd
    /// has evaluated the request completely and found the one-shot grant `grant_id`
    /// eligible for the runtime `subject`. Under the user lock registryd re-checks
    /// every rule its own snapshot decides: the user state and the persisted
    /// emergency restriction; the grant current, one-shot, not ended and not
    /// reserved; the lineage's authoritative subject; the signing key and the policy
    /// epoch; the lifetime at this operation's own trusted now (§8 option B) and a
    /// bound session not ended — the lifetime at a sample taken after the lock and
    /// any flush it waited for (M2-006-B2-01). It then consumes the grant in memory
    /// and writes
    /// `use_reserved` through the restrictive path before it replies. A failed write
    /// leaves the grant consumed and reports `restriction_not_durable`: the use is
    /// denied.
    pub fn reserve_use(
        &self,
        caller: caller_role_t,
        owner_uid: u32,
        grant_id: &random_id_t,
        subject: &subject_t,
        operation_id: &random_id_t,
    ) -> service_result_t<reservation_receipt_t> {
        require(caller, &[Policyd])?;
        self.retry_unverifiable();
        let observed = self.observe_time();
        let slot = self
            .slot_any(owner_uid)?
            .ok_or(authority(authority_code_t::key_not_certified))?;
        let mut slot = lock(&slot)?;
        self.record_anomaly(&mut slot, &observed);
        match slot.user_state() {
            user_state_t::normal => {}
            user_state_t::contained => return Err(authority(authority_code_t::contained)),
            user_state_t::quarantined => return Err(authority(authority_code_t::quarantined)),
            user_state_t::deleted => return Err(authority(authority_code_t::user_deleted)),
        }
        self.ready(&mut slot)?;
        if !slot.pending.is_empty() {
            return Err(not_durable());
        }
        // A sample after the lock and the flush, immediately before the lifetime
        // check and the consumption (M2-006-B2-01).
        let now = self.decision_now(&mut slot)?;
        let observed_now = lock(&self.clock)?
            .observed_now()
            .ok_or(storage_unavailable())?;
        let policy = *lock(&self.policy)?;
        let user_floor = lock(&self.anchor)?
            .map()
            .users
            .get(&owner_uid)
            .map_or(0, |entry| entry.min_valid_policy_epoch);
        let refused = || rule(rule_code_t::lifecycle_rule);
        if policy.emergency_restriction {
            return Err(refused());
        }
        {
            let lineages = &slot.lineages;
            let entry = lineages.grant(grant_id).ok_or_else(refused)?;
            let subject_matches = lineages
                .lineage(&entry.lineage_id)
                .and_then(|lineage| lineage.authoritative_subject.as_ref())
                .is_some_and(|authoritative| authoritative == subject)
                && subject.owner_uid == owner_uid
                && subject.owner_user_id == slot.user_id;
            if !subject_matches {
                return Err(rule(rule_code_t::subject_binding_mismatch));
            }
            let commit = lineages.commit(&entry.commit_id).ok_or_else(refused)?;
            let key = slot
                .directory
                .resolve(&commit.payload.key_id)
                .ok_or(authority(authority_code_t::key_not_certified))?;
            if key.invalidated == Some(invalidation_reason_t::compromised) {
                return Err(authority(authority_code_t::key_invalidated));
            }
            if commit.payload.policy.epoch < policy.min_valid_epoch.max(user_floor) {
                return Err(ipc(ipc_code_t::stale_epoch));
            }
            if !lifetime_open(lineages, &entry.grant, commit.admitted, now, observed_now) {
                return Err(refused());
            }
        }
        let mut reservation_id = [0u8; 16];
        cham3_os_adapter::fill_random(&mut reservation_id).map_err(|_| storage_unavailable())?;
        let event = registry_event_t::use_reserved {
            grant_id: *grant_id,
            reservation_id,
            operation_id: *operation_id,
        };
        // One-shot, current, not ended and not reserved, under a fresh id.
        slot.lineages.check_event(&event)?;
        // Consumed in memory first, whatever the writes do (option A).
        slot.lineages.apply_event(&event);
        let time = utc_text(now).ok_or(storage_unavailable())?;
        let sequence = self.restrictive_write(&mut slot, time, record_body_t::event(event))?;
        Ok(reservation_receipt_t {
            reservation_id,
            journal_seq: sequence,
        })
    }

    /// `complete_use` (`fixture_consumer`): the outcome of a reserved use, at most
    /// once per reservation and never after `consumed_uncertain` (ADR 0007 §§4, 7).
    /// The grant stays consumed whatever the outcome. Evidence: a record that cannot
    /// be written leaves an `audit_gap` (erratum E7.3).
    pub fn complete_use(
        &self,
        caller: caller_role_t,
        owner_uid: u32,
        reservation_id: &random_id_t,
        outcome: use_outcome_t,
    ) -> service_result_t<u64> {
        require(caller, &[FixtureConsumer])?;
        self.retry_unverifiable();
        let observed = self.observe_time();
        let slot = self
            .slot_any(owner_uid)?
            .ok_or(authority(authority_code_t::key_not_certified))?;
        let mut slot = lock(&slot)?;
        if let slot_state_t::quarantined(_) = slot.state {
            return Err(authority(authority_code_t::quarantined));
        }
        self.record_anomaly(&mut slot, &observed);
        let event = registry_event_t::use_completed {
            reservation_id: *reservation_id,
            outcome,
        };
        slot.lineages.check_event(&event)?;
        self.write_evidence(&mut slot, event, lost_evidence_t::use_completed)
    }

    /// `session_event` (`fixture_consumer`, later sessiond; ADR 0007 §6). A start is
    /// evidence: refused for an ended session (session ids are single-use), and a
    /// repeated start returns the first one's sequence. An end takes the restrictive
    /// path: `session_ended` when grants of the journal are bound to the session,
    /// otherwise a `session_event` end; either way the session never becomes live
    /// again, and admission refuses grants bound to it. A repeated end returns the
    /// first one's sequence, and the end of a session that a new boot ended returns
    /// that boot's `sessions_ended` (M2-006-B2-03).
    pub fn session_event(
        &self,
        caller: caller_role_t,
        owner_uid: u32,
        session_id: &random_id_t,
        change: session_change_t,
    ) -> service_result_t<u64> {
        require(caller, &[FixtureConsumer])?;
        self.retry_unverifiable();
        let observed = self.observe_time();
        let slot = self
            .slot_any(owner_uid)?
            .ok_or(authority(authority_code_t::key_not_certified))?;
        let mut slot = lock(&slot)?;
        if let slot_state_t::quarantined(_) = slot.state {
            return Err(authority(authority_code_t::quarantined));
        }
        self.record_anomaly(&mut slot, &observed);
        match change {
            session_change_t::started => {
                let event = registry_event_t::session_event {
                    session_id: *session_id,
                    change,
                };
                // An ended session never starts again, even when its start is durable.
                if slot.lineages.session_ended(session_id) {
                    return Err(rule(rule_code_t::lifecycle_rule));
                }
                if let Some(sequence) = slot.durable_sequence(&record_body_t::event(event.clone()))
                {
                    return Ok(sequence);
                }
                slot.lineages.check_event(&event)?;
                self.write_evidence(&mut slot, event, lost_evidence_t::session_event)
            }
            session_change_t::ended => {
                let mut ends = vec![
                    record_body_t::lifecycle(lifecycle_record_t::session_ended {
                        session_id: *session_id,
                    }),
                    record_body_t::event(registry_event_t::session_event {
                        session_id: *session_id,
                        change,
                    }),
                ];
                // A session a new boot ended: its end is that boot's record.
                if let Some(boot_id) = slot.lineages.session_ended_by_boot(session_id) {
                    ends.push(record_body_t::event(registry_event_t::sessions_ended {
                        boot_id: boot_id.to_owned(),
                    }));
                }
                if slot.lineages.session_ended(session_id) {
                    if slot
                        .pending
                        .iter()
                        .any(|pending| ends.contains(&pending.body))
                    {
                        self.flush_pending(&mut slot)?;
                    }
                    return ends
                        .iter()
                        .find_map(|body| slot.durable_sequence(body))
                        .ok_or_else(not_durable);
                }
                let time = self.now_text()?;
                let body = if slot.lineages.session_bound(session_id) {
                    let record = lifecycle_record_t::session_ended {
                        session_id: *session_id,
                    };
                    slot.lineages.check_record(&record, &time)?;
                    slot.lineages.apply_record(&record);
                    record_body_t::lifecycle(record)
                } else {
                    let event = registry_event_t::session_event {
                        session_id: *session_id,
                        change,
                    };
                    slot.lineages.check_event(&event)?;
                    slot.lineages.apply_event(&event);
                    record_body_t::event(event)
                };
                self.restrictive_write(&mut slot, time, body)
            }
        }
    }

    /// `confirm_enforcement` (`fixture_consumer`, later launcherd, netd and brokers;
    /// ADR 0006 §1, ADR 0007 erratum E7.2): one domain's outcome for a commit that is
    /// a live head of the user's journal (`stale_head` otherwise), at the policy
    /// epoch in force (`stale_epoch` otherwise), bound to the runtime id. It counts
    /// for the commit's status only in this run, the latest per domain winning; the
    /// status is never an eligibility input. Evidence: a record that cannot be
    /// written leaves an `audit_gap`.
    #[allow(clippy::too_many_arguments)]
    pub fn confirm_enforcement(
        &self,
        caller: caller_role_t,
        owner_uid: u32,
        commit_id: &digest_t,
        runtime_id: &random_id_t,
        domain: enforcement_domain_t,
        policy_epoch: u64,
        result: enforcement_result_t,
    ) -> service_result_t<u64> {
        require(caller, &[FixtureConsumer])?;
        self.retry_unverifiable();
        let observed = self.observe_time();
        let slot = self
            .slot_any(owner_uid)?
            .ok_or(authority(authority_code_t::key_not_certified))?;
        let mut slot = lock(&slot)?;
        self.record_anomaly(&mut slot, &observed);
        match slot.state {
            slot_state_t::quarantined(_) => return Err(authority(authority_code_t::quarantined)),
            slot_state_t::contained => return Err(authority(authority_code_t::contained)),
            slot_state_t::normal => {}
        }
        if !slot.lineages.is_live_head(commit_id) {
            return Err(ipc(ipc_code_t::stale_head));
        }
        if policy_epoch != lock(&self.policy)?.epoch {
            return Err(ipc(ipc_code_t::stale_epoch));
        }
        let event = registry_event_t::enforcement_outcome {
            commit_id: *commit_id,
            runtime_id: *runtime_id,
            domain,
            policy_epoch,
            result,
            run_id: self.run_id,
        };
        slot.lineages.check_event(&event)?;
        self.write_evidence(&mut slot, event, lost_evidence_t::enforcement_outcome)
    }

    /// Writes an evidence record with an ordinary append (quota; anchored before the
    /// reply) and applies it once durable. When it cannot be written — a write still
    /// owed, a failed ledger or journal, an exhausted quota or a failed append — the
    /// evidence is lost: an `audit_gap` naming its kind takes the restrictive path
    /// (ledger at once, journal once storage recovers; ADR 0007 erratum E7.3) and
    /// the caller gets the error. A durable record whose anchor update failed is not
    /// lost: the next retry or the next start's roll-forward anchors it.
    fn write_evidence(
        &self,
        slot: &mut user_slot_t,
        event: registry_event_t,
        lost: lost_evidence_t,
    ) -> service_result_t<u64> {
        let time = self.now_text()?;
        let body = record_body_t::event(event.clone());
        let written = self.ready(slot).and_then(|()| {
            if !slot.pending.is_empty() {
                return Err(not_durable());
            }
            let sequence = slot.journal.next_sequence().ok_or(storage_unavailable())?;
            let record = registry_record_t {
                sequence,
                time: time.clone(),
                body: body.clone(),
            }
            .encode();
            self.ordinary_append(slot, &record)
        });
        let sequence = match written {
            Ok(sequence) => sequence,
            Err(error) => {
                let boot_id = lock(&self.clock)?.boot_id().to_owned();
                let gap = registry_event_t::audit_gap { boot_id, lost };
                let _ = self.restrictive_write(slot, time, record_body_t::event(gap));
                return Err(error);
            }
        };
        slot.lineages.apply_event(&event);
        if let registry_event_t::enforcement_outcome {
            commit_id,
            domain,
            result,
            run_id,
            ..
        } = &event
        {
            if *run_id == self.run_id {
                slot.enforcement.insert((*commit_id, *domain), *result);
            }
        }
        slot.note_durable(sequence, time, body);
        slot.events.push(event);
        self.anchor_slot(slot)?;
        Ok(sequence)
    }

    /// `history` (`trusted_client` for its own UID, `admin`; ADR 0008 §2): one page of
    /// the user's journal after the sequence `after`, identity objects left out
    /// (identityd pages them). A page holds at most 16 records, or one record that
    /// embeds a signed object (a commit, service IPC §2); `next` is the cursor of
    /// the following page. Each commit carries this run's enforcement status per
    /// domain (erratum E7.2). The page also reports the restrictive records not yet
    /// durable and the policy state in force: the history is never complete (ADR
    /// 0007 §3). Refused for a quarantined user and while the user's last record is
    /// unanchored.
    pub fn history(
        &self,
        caller: caller_role_t,
        owner_uid: u32,
        after: u64,
    ) -> service_result_t<history_page_t> {
        match caller {
            caller_role_t::trusted_client { uid } if uid == owner_uid => {}
            caller_role_t::trusted_client { .. } => return Err(ipc(ipc_code_t::owner_mismatch)),
            caller_role_t::admin => {}
            _ => return Err(ipc(ipc_code_t::role_denied)),
        }
        self.retry_unverifiable();
        let observed = self.observe_time();
        let policy = *lock(&self.policy)?;
        let Some(slot) = self.slot_any(owner_uid)? else {
            return Ok(history_page_t {
                records: Vec::new(),
                next: None,
                pending_restrictions: 0,
                policy,
            });
        };
        let mut slot = lock(&slot)?;
        if let slot_state_t::quarantined(_) = slot.state {
            return Err(authority(authority_code_t::quarantined));
        }
        self.record_anomaly(&mut slot, &observed);
        if slot.journal.failed() {
            return Err(storage_unavailable());
        }
        self.anchored_for_reading(&mut slot)?;
        let mut records: Vec<history_record_t> = Vec::new();
        let mut remaining = slot
            .history
            .iter()
            .filter(|item| item.sequence > after)
            .peekable();
        while let Some(item) = remaining.peek() {
            let signed = matches!(item.body, history_body_t::commit { .. });
            if records.len() >= MaxPageRecords || (signed && !records.is_empty()) {
                break;
            }
            let (body, enforcement) = match &item.body {
                history_body_t::commit {
                    commit_id,
                    lineage_id,
                } => {
                    let entry = slot
                        .lineages
                        .commit(commit_id)
                        .ok_or(storage_unavailable())?;
                    let status = slot
                        .enforcement
                        .iter()
                        .filter(|((commit, _), _)| commit == commit_id)
                        .map(|((_, domain), result)| (*domain, *result))
                        .collect();
                    (
                        record_body_t::commit_admitted {
                            commit_id: *commit_id,
                            lineage_id: *lineage_id,
                            placement: entry.placement,
                            envelope: entry.envelope.clone(),
                            observed: entry.admitted.observed,
                        },
                        status,
                    )
                }
                history_body_t::record(body) => (body.clone(), Vec::new()),
            };
            records.push(history_record_t {
                sequence: item.sequence,
                time: item.time.clone(),
                body,
                enforcement,
            });
            remaining.next();
            if signed {
                break;
            }
        }
        let next = match remaining.peek() {
            Some(_) => records.last().map(|record| record.sequence),
            None => None,
        };
        Ok(history_page_t {
            records,
            next,
            pending_restrictions: u64::try_from(slot.pending.len()).unwrap_or(u64::MAX),
            policy,
        })
    }

    /// A commit's enforcement status in this run, per domain (diagnostics and tests;
    /// `history` reports it to clients).
    pub fn enforcement_status(
        &self,
        owner_uid: u32,
        commit_id: &digest_t,
    ) -> service_result_t<Vec<(enforcement_domain_t, enforcement_result_t)>> {
        let Some(slot) = self.slot_any(owner_uid)? else {
            return Ok(Vec::new());
        };
        let slot = lock(&slot)?;
        Ok(slot
            .enforcement
            .iter()
            .filter(|((commit, _), _)| commit == commit_id)
            .map(|((_, domain), result)| (*domain, *result))
            .collect())
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
        self.retry_unverifiable();
        self.observe_time();
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
        self.retry_unverifiable();
        self.observe_time();
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
        let floor = self.floor();
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
            .update(self.io.as_ref(), |map| {
                map.policy = next;
                publish_floor(map, floor);
            })
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
        let floor = self.floor();
        let mut anchor = lock(&self.anchor)?;
        anchor
            .update(self.io.as_ref(), |map| {
                map.policy = next;
                publish_floor(map, floor);
            })
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
        self.retry_unverifiable();
        self.observe_time();
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
        let clock = {
            let clock = lock(&self.clock)?;
            clock_status_t {
                boot_id: clock.boot_id().to_owned(),
                trusted_now: clock.now().ok_or(storage_unavailable())?,
                observed_now: clock.observed_now().ok_or(storage_unavailable())?,
                anomaly: clock.anomaly(),
            }
        };
        let ledger_failed = lock(&self.ledger)?.failed();
        let unverifiable_owed = self.unverifiable_owed()?;
        Ok(rollback_status_t {
            anchor_mode,
            run_decision: self.run_decision,
            quarantined,
            unusable,
            contained,
            open_ceremonies: Vec::new(),
            pending_restrictions: pending,
            ledger_failed,
            unverifiable_owed,
            depleted_reserves: depleted,
            policy_pending: policy != anchored_policy,
            policy,
            clock,
        })
    }

    /// Orderly shutdown (ADR 0006 §3): once everything owed is durable — every pending
    /// restrictive write, every unanchored record, the policy state in force — the run
    /// marker becomes `clean`, in the same slot write as the policy state, so the next
    /// start resumes without the ledger. With anything still owed, or a failed ledger,
    /// the marker stays as it is and `restriction_not_durable` is reported.
    pub fn shutdown(&self) -> service_result_t<()> {
        self.retry_unverifiable();
        self.observe_time();
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
        let floor = self.floor();
        let mut anchor = lock(&self.anchor)?;
        anchor
            .update(self.io.as_ref(), |map| {
                map.policy = policy_now;
                if let Some(marker) = &mut map.run_marker {
                    marker.state = run_state_t::clean;
                }
                publish_floor(map, floor);
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
        self.retry_unverifiable();
        let observed = self.observe_time();
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
        self.record_anomaly(&mut slot, &observed);
        self.anchored_for_reading(&mut slot)?;
        Ok(Some(self.view(&slot, lineage_id)?))
    }

    /// `time_sample` (`service:policyd`): an offset increase policyd observed (ADR 0007
    /// §5 rule 4; `boot_id`, `boottime`, `trusted`). registryd takes the maximum and
    /// appends it to the ledger before it adopts it and replies; policyd relies on the
    /// increase only after an `ok` reply. Another boot's sample, or a CLOCK_BOOTTIME
    /// ahead of registryd's own, is `malformed_request`. Returns the shared offsets.
    pub fn time_sample(
        &self,
        caller: caller_role_t,
        boot_id: &str,
        boottime: u64,
        trusted: u64,
    ) -> service_result_t<time_view_t> {
        require(caller, &[Policyd])?;
        self.retry_unverifiable();
        self.observe_time();
        let malformed = || ipc(ipc_code_t::malformed_request);
        let offset = i64::try_from(trusted)
            .ok()
            .zip(i64::try_from(boottime).ok())
            .and_then(|(trusted, boottime)| trusted.checked_sub(boottime))
            .ok_or_else(malformed)?;
        let mut ledger_failed = false;
        let view = {
            let mut clock = lock(&self.clock)?;
            if boot_id != clock.boot_id() || boottime > clock.boottime().saturating_add(1) {
                return Err(malformed());
            }
            if offset > clock.offset() {
                let payload = clock
                    .floor_with(offset)
                    .as_ref()
                    .and_then(time_payload)
                    .ok_or_else(malformed)?;
                let appended =
                    lock(&self.ledger)?.append(self.io.as_ref(), entry_kind_t::time, 0, &payload);
                match appended {
                    Ok(()) => clock.adopt(offset),
                    Err(store_error_t::io) => ledger_failed = true,
                    Err(_) => return Err(storage_unavailable()),
                }
            }
            clock.view()
        };
        if ledger_failed {
            self.mark_run_unverifiable();
            return Err(storage_unavailable());
        }
        Ok(view)
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
                    admitted: commit.admitted,
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
        let time = lock(&self.clock)?.view();
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
            time,
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
