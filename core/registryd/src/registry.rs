//! registryd: the per-user journals, their derived indexes and the operations of
//! service IPC §3 that M2-005 implements (`append_identity`, `identity_state`,
//! `admit`, `lineage_head`).
//!
//! **One snapshot.** A user's identity records and commits live in one journal (ADR
//! 0006 §1). The slot of a user holds that journal, the key-authority index rebuilt
//! from its identity records (identityd's own code, so layer 6 at admission reads the
//! same snapshot as the commit) and the ECR index. Admission re-verifies layers 1–6
//! against that snapshot, then layer 7, then the policy epoch and quotas, all under
//! the user's lock immediately before the append; the indexes change only after the
//! record is durable.
//!
//! **Locks.** `users` (the map of slots) is held only to find or create a slot; a
//! user lock is never taken while `users` is held, except for a slot created under
//! it. Lock order: `users` → user. The anchor lock of ADR 0006 §2 joins in M2-006
//! (user → anchor).
//!
//! **Replay.** At start every journal is parsed, chain-verified and replayed through
//! the same checks as at admission (except the current policy epoch and quotas,
//! which are admission-time facts). A user whose journal fails is unusable: every
//! operation for that user fails `storage_unavailable` (fail closed). The
//! quarantine decision table, anchor and torn-tail recovery are M2-006.
//!
//! Limits (M2-005): in-process composition; the socket transport and process
//! boundary are M2-007. Registry time is the system clock; ADR 0007's trusted-time
//! offset rule is M2-006.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use cham3_contract_format::hash::digest_t;
use cham3_contract_format::schema::{
    commit_payload_t, contract_state_t, invalidation_reason_t, random_id_t, subject_t,
};
use cham3_contract_format::{key_resolver_t, object_kind_t, payload_t, verify_object};
use cham3_identityd::caller::{caller_role_t, service_name_t};
use cham3_identityd::directory::{identity_directory_t, identity_effect_t, journal_record_t};
use cham3_identityd::index::{
    check_commit_authority, check_signing_window, user_state_t, utc_text,
};
use cham3_identityd::journal::{
    MaxPageRecords, append_receipt_t, identity_journal_t, journal_entry_t, journal_page_t,
};
use cham3_identityd::outcome::{
    authority, authority_code_t, ipc, ipc_code_t, service_error_t, service_result_t,
};

use crate::journal::{journal_file_t, journal_io_t, journal_store_t, parse, store_error_t};
use crate::lineage::{admission_t, end_reason_t, lineage_index_t, standing_t};
use crate::record::{placement_t, record_body_t, registry_record_t};

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

/// Registry configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct registry_config_t {
    pub quota_bytes: u64,
    pub max_lineages: usize,
    /// The current policy epoch: a commit's `policy.epoch` must equal it at
    /// admission (`stale_epoch`). Raising it is policyd's admin operation (M2-006).
    pub policy_epoch: u64,
    /// Grants admitted under a lower epoch are no longer eligible (ADR 0007 §1 rule
    /// 4). Per-user raises (`recover_user`) are M2-006.
    pub min_valid_policy_epoch: u64,
}

impl Default for registry_config_t {
    fn default() -> registry_config_t {
        registry_config_t {
            quota_bytes: DefaultQuotaBytes,
            max_lineages: MaxLineages,
            policy_epoch: 1,
            min_valid_policy_epoch: 1,
        }
    }
}

/// Why a configuration or start was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum registry_start_error_t {
    /// Quota outside 16 MiB–1 GiB, lineages outside 1..=4096 or an epoch order error.
    invalid_config,
    /// The store root or its `users` directory is unusable.
    store(store_error_t),
}

/// Registry time (seconds since 1970-01-01T00:00:00Z).
pub trait registry_clock_t: Send + Sync {
    fn utc_seconds(&self) -> i64;
}

/// The system clock (ADR 0007's offset rule is M2-006).
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

/// Why a user's journal is unusable in this run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum unusable_reason_t {
    /// The file could not be read safely.
    store(store_error_t),
    /// Header, frame, torn tail or chain defect.
    defect(crate::record::journal_defect_t),
    /// A record does not decode, has the wrong sequence or fails re-verification.
    replay { sequence: u64 },
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
}

/// registryd.
pub struct registry_t {
    store: journal_store_t,
    config: registry_config_t,
    io: Box<dyn journal_io_t>,
    clock: Box<dyn registry_clock_t>,
    users: Mutex<BTreeMap<u32, user_entry_t>>,
}

fn storage_unavailable() -> service_error_t {
    ipc(ipc_code_t::storage_unavailable)
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
    fn usable(&self) -> service_result_t<()> {
        if self.journal.failed() {
            Err(storage_unavailable())
        } else {
            Ok(())
        }
    }

    fn user_state(&self) -> user_state_t {
        self.directory
            .user(self.uid)
            .map_or(user_state_t::deleted, |user| user.state())
    }

    /// Applies one replayed record at `sequence` (position `sequence − 1`).
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
            }
        }
        Ok(())
    }
}

/// Replays one user's journal file.
fn replay_user(
    store: &journal_store_t,
    uid: u32,
    bytes: &[u8],
    io: &dyn journal_io_t,
) -> Result<user_slot_t, unusable_reason_t> {
    let parsed = parse(bytes, uid).map_err(unusable_reason_t::defect)?;
    let length = u64::try_from(bytes.len())
        .map_err(|_| unusable_reason_t::store(store_error_t::too_large))?;
    let journal = store
        .open_journal(uid, &parsed, length, io)
        .map_err(unusable_reason_t::store)?;
    let mut slot = user_slot_t {
        uid,
        user_id: parsed.user_id,
        journal,
        directory: identity_directory_t::new(),
        lineages: lineage_index_t::new(),
        identity_entries: Vec::new(),
    };
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
    }
    if parsed.records.is_empty() {
        return Err(unusable_reason_t::replay { sequence: 1 });
    }
    Ok(slot)
}

impl registry_t {
    /// Starts registryd over the store at `root` (owned by `owner`, registryd's
    /// effective UID) and replays every journal.
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
        let mut users = BTreeMap::new();
        for uid in store.users().map_err(registry_start_error_t::store)? {
            let entry = match store.read(uid, config.quota_bytes) {
                // A user directory without a journal holds no user (a creation that
                // never completed its first durable record).
                Ok(None) => continue,
                Ok(Some(bytes)) => match replay_user(&store, uid, &bytes, io.as_ref()) {
                    Ok(slot) => user_entry_t::usable(Arc::new(Mutex::new(slot))),
                    Err(reason) => user_entry_t::unusable(reason),
                },
                Err(error) => user_entry_t::unusable(unusable_reason_t::store(error)),
            };
            users.insert(uid, entry);
        }
        Ok(registry_t {
            store,
            config,
            io,
            clock,
            users: Mutex::new(users),
        })
    }

    pub fn config(&self) -> registry_config_t {
        self.config
    }

    /// Users whose journal is unusable in this run, with the reason.
    pub fn unusable_users(&self) -> service_result_t<Vec<(u32, unusable_reason_t)>> {
        Ok(lock(&self.users)?
            .iter()
            .filter_map(|(uid, entry)| match entry {
                user_entry_t::unusable(reason) => Some((*uid, *reason)),
                user_entry_t::usable(_) => None,
            })
            .collect())
    }

    fn slot(&self, uid: u32) -> service_result_t<Option<Arc<Mutex<user_slot_t>>>> {
        match lock(&self.users)?.get(&uid) {
            None => Ok(None),
            Some(user_entry_t::usable(slot)) => Ok(Some(Arc::clone(slot))),
            Some(user_entry_t::unusable(_)) => Err(storage_unavailable()),
        }
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

    // ---------------------------------------------------------------- identity

    /// `append_identity` (`service:identityd`): persists a validated identity object
    /// in `owner_uid`'s journal before replying. registryd re-validates it against
    /// the user's snapshot (it must replay it later). An identical object already
    /// durable returns its sequence without a second record.
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
        if let Some(slot) = self.slot(owner_uid)? {
            let mut slot = lock(&slot)?;
            slot.usable()?;
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
        let verified = slot.directory.verify_identity_object(kind, object)?;
        let effect = slot.directory.check_identity_object(slot.uid, &verified)?;
        if let identity_effect_t::already_present(sequence) = effect {
            return Ok(append_receipt_t {
                journal_seq: sequence,
                already_present: true,
            });
        }
        let sequence = slot.journal.next_sequence().ok_or(storage_unavailable())?;
        let record = registry_record_t {
            sequence,
            time: self.now_text()?,
            body: record_body_t::identity {
                kind,
                object: object.to_vec(),
            },
        }
        .encode();
        // A key invalidation is restrictive and may use the reserve (quota and blocks).
        let restrictive = kind == object_kind_t::key_invalidation;
        self.check_quota(&slot.journal, record.len(), restrictive)?;
        slot.journal
            .append(&record, restrictive, self.io.as_ref())
            .map_err(|_| storage_unavailable())?;
        slot.directory
            .apply_identity_object(slot.uid, &verified, sequence)?;
        slot.identity_entries.push(journal_entry_t {
            sequence,
            record: journal_record_t::identity {
                kind,
                object: object.to_vec(),
            },
        });
        Ok(append_receipt_t {
            journal_seq: sequence,
            already_present: false,
        })
    }

    /// Genesis at enrollment (ADR 0006 §4): header and enrollment record, durable,
    /// under the `users` lock so that two first enrollments of a UID cannot race.
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
                slot.usable()?;
                return self.append_identity_to(&mut slot, kind, object);
            }
            Some(user_entry_t::unusable(_)) => return Err(storage_unavailable()),
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
        let slot = user_slot_t {
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
        };
        users.insert(owner_uid, user_entry_t::usable(Arc::new(Mutex::new(slot))));
        Ok(append_receipt_t {
            journal_seq: 1,
            already_present: false,
        })
    }

    /// `identity_state` (`service:identityd`): the user's identity records with a
    /// sequence above `after`, one page at a time.
    pub fn identity_state(
        &self,
        caller: caller_role_t,
        owner_uid: u32,
        after: u64,
    ) -> service_result_t<journal_page_t> {
        require(caller, &[Identityd])?;
        let Some(slot) = self.slot(owner_uid)? else {
            return Ok(journal_page_t {
                entries: Vec::new(),
                next: None,
            });
        };
        let slot = lock(&slot)?;
        slot.usable()?;
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
    /// before the reply; admitted `pending`.
    pub fn admit(
        &self,
        caller: caller_role_t,
        owner_uid: u32,
        envelope: &[u8],
    ) -> service_result_t<admission_receipt_t> {
        require(caller, &[Contractd])?;
        let slot = self
            .slot(owner_uid)?
            .ok_or(authority(authority_code_t::key_not_certified))?;
        let mut slot = lock(&slot)?;
        slot.usable()?;
        let (admission, payload) =
            check_commit_envelope(owner_uid, &slot.directory, &slot.lineages, envelope)?;
        // (beyond the model) The signing key's certificate window must also contain
        // registry time now, not only the commit's own timestamp: an envelope signed
        // in a temporary key's window cannot be admitted after that window closed.
        // Admission-time only: replay reproduces historical admissions. The ±300 s
        // commit-timestamp check of ADR 0007 §5 joins in M2-006.
        let resolution = slot
            .directory
            .resolve(&payload.key_id)
            .ok_or(authority(authority_code_t::key_not_certified))?;
        check_signing_window(&resolution, self.clock.utc_seconds())?;
        if payload.policy.epoch != self.config.policy_epoch {
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
        self.check_quota(&slot.journal, record.len(), false)?;
        // Not restrictive: the full reserve must exist before the append (R03).
        slot.journal
            .append(&record, false, self.io.as_ref())
            .map_err(|_| storage_unavailable())?;
        slot.lineages
            .apply_commit(&admission, payload, envelope.to_vec(), sequence);
        Ok(admission_receipt_t {
            commit_id: admission.commit_id,
            journal_seq: sequence,
            placement: admission.placement,
            status: crate::lineage::commit_status_t::pending,
        })
    }

    /// `lineage_head` (`service:contractd`, `service:policyd`): one lineage from a
    /// consistent snapshot. `None` if the user has no journal.
    pub fn lineage_head(
        &self,
        caller: caller_role_t,
        owner_uid: u32,
        lineage_id: &digest_t,
    ) -> service_result_t<Option<lineage_view_t>> {
        require(caller, &[Contractd, Policyd])?;
        let Some(slot) = self.slot(owner_uid)? else {
            return Ok(None);
        };
        let slot = lock(&slot)?;
        slot.usable()?;
        Ok(Some(self.view(&slot, lineage_id)))
    }

    fn view(&self, slot: &user_slot_t, lineage_id: &digest_t) -> lineage_view_t {
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
        lineage_view_t {
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
            policy_epoch: self.config.policy_epoch,
            min_valid_policy_epoch: self.config.min_valid_policy_epoch,
        }
    }

    /// Bytes allocated to a user's restrictive reserve (diagnostics and tests).
    pub fn reserve_bytes(&self, owner_uid: u32) -> service_result_t<u64> {
        let Some(slot) = self.slot(owner_uid)? else {
            return Err(authority(authority_code_t::key_not_certified));
        };
        let slot = lock(&slot)?;
        Ok(slot.journal.reserve_bytes())
    }

    /// The standing of a grant id in a user's index (diagnostics and tests).
    pub fn standing(&self, owner_uid: u32, grant_id: &random_id_t) -> service_result_t<standing_t> {
        let Some(slot) = self.slot(owner_uid)? else {
            return Ok(standing_t::unknown);
        };
        let slot = lock(&slot)?;
        slot.usable()?;
        Ok(slot.lineages.standing(grant_id))
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
