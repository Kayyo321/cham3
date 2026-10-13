//! identityd's persistence boundary: registryd's per-user journal (ADR 0006 §§1, 5).
//!
//! identityd keeps no store (ADR 0005 §6, R05). It appends validated identity objects
//! through registryd `append_identity` and rebuilds its index by paging through
//! `identity_state`, which returns the user's identity objects and the registry
//! records that change key authority, in journal order, with their journal sequence.
//!
//! [`memory_journal_t`] is a labeled **M2-004 fixture** standing in for registryd so
//! that identityd and vaultd can be exercised before the registry exists (M2-005/006).
//! It has no files, anchor, ledger or crash points and establishes no durability or
//! rollback-protection claim.

use std::collections::BTreeMap;

use cham3_contract_format::hash::{digest_t, sha256};
use cham3_contract_format::object_kind_t;

use crate::directory::journal_record_t;
use crate::index::{naming_t, user_record_t};
use crate::outcome::{ipc, ipc_code_t, service_result_t};

/// Records per `identity_state` page (service IPC §2).
pub const MaxPageRecords: usize = 16;

/// Receipt of an `append_identity`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct append_receipt_t {
    pub journal_seq: u64,
    /// The identical object was already durable; nothing was appended.
    pub already_present: bool,
}

/// One journal entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct journal_entry_t {
    pub sequence: u64,
    pub record: journal_record_t,
}

/// One `identity_state` page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct journal_page_t {
    pub entries: Vec<journal_entry_t>,
    /// Cursor for the next page, if any.
    pub next: Option<u64>,
}

/// registryd's identity operations as identityd uses them.
pub trait identity_journal_t {
    /// Persists a validated identity object in `owner_uid`'s journal before replying.
    /// An identical, already durable object returns its sequence.
    fn append_identity(
        &mut self,
        owner_uid: u32,
        kind: object_kind_t,
        object: &[u8],
    ) -> service_result_t<append_receipt_t>;

    /// Identity-relevant records of `owner_uid` with a sequence above `after`.
    fn identity_state(&self, owner_uid: u32, after: u64) -> service_result_t<journal_page_t>;

    /// UIDs that have a journal (for the index rebuild at start).
    fn journal_users(&self) -> service_result_t<Vec<u32>>;
}

/// Fault injected into the fixture.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct journal_faults_t {
    /// `append_identity` fails with `storage_unavailable` and appends nothing.
    pub fail_appends: bool,
    /// Every operation fails with `storage_unavailable`.
    pub unavailable: bool,
}

#[derive(Clone, Debug, Default)]
struct user_journal_t {
    entries: Vec<journal_entry_t>,
    objects: BTreeMap<digest_t, u64>,
}

impl user_journal_t {
    fn push(&mut self, record: journal_record_t) -> service_result_t<u64> {
        let sequence = u64::try_from(self.entries.len())
            .ok()
            .and_then(|count| count.checked_add(1))
            .ok_or(ipc(ipc_code_t::storage_unavailable))?;
        self.entries.push(journal_entry_t { sequence, record });
        Ok(sequence)
    }
}

/// M2-004 test fixture for registryd's journal (not the registry; see module docs).
#[derive(Clone, Debug, Default)]
pub struct memory_journal_t {
    users: BTreeMap<u32, user_journal_t>,
    pub faults: journal_faults_t,
}

impl memory_journal_t {
    pub fn new() -> memory_journal_t {
        memory_journal_t::default()
    }

    /// Stands in for a record registryd writes on its own authority.
    pub fn push_user_record(
        &mut self,
        owner_uid: u32,
        record: user_record_t,
    ) -> service_result_t<u64> {
        self.users
            .entry(owner_uid)
            .or_default()
            .push(journal_record_t::user(record))
    }

    /// Stands in for an anchored `recovery_keys` record built by registryd
    /// `name_recovery_keys` (callable only by vaultd).
    pub fn push_naming(&mut self, owner_uid: u32, naming: naming_t) -> service_result_t<u64> {
        self.users
            .entry(owner_uid)
            .or_default()
            .push(journal_record_t::recovery_keys(naming))
    }

    /// Number of records in a user's journal.
    pub fn record_count(&self, owner_uid: u32) -> usize {
        self.users
            .get(&owner_uid)
            .map_or(0, |journal| journal.entries.len())
    }

    /// Keeps only a user's first `length` records (stand-in for a stale restore).
    pub fn truncate(&mut self, owner_uid: u32, length: usize) {
        if let Some(journal) = self.users.get_mut(&owner_uid) {
            journal.entries.truncate(length);
            let kept: BTreeMap<digest_t, u64> = journal
                .entries
                .iter()
                .filter_map(|entry| match &entry.record {
                    journal_record_t::identity { object, .. } => {
                        Some((sha256(object), entry.sequence))
                    }
                    _ => None,
                })
                .collect();
            journal.objects = kept;
        }
    }
}

impl identity_journal_t for memory_journal_t {
    fn append_identity(
        &mut self,
        owner_uid: u32,
        kind: object_kind_t,
        object: &[u8],
    ) -> service_result_t<append_receipt_t> {
        if self.faults.unavailable || self.faults.fail_appends {
            return Err(ipc(ipc_code_t::storage_unavailable));
        }
        if kind == object_kind_t::commit {
            return Err(ipc(ipc_code_t::malformed_request));
        }
        let journal = self.users.entry(owner_uid).or_default();
        let digest = sha256(object);
        if let Some(sequence) = journal.objects.get(&digest) {
            return Ok(append_receipt_t {
                journal_seq: *sequence,
                already_present: true,
            });
        }
        let sequence = journal.push(journal_record_t::identity {
            kind,
            object: object.to_vec(),
        })?;
        journal.objects.insert(digest, sequence);
        Ok(append_receipt_t {
            journal_seq: sequence,
            already_present: false,
        })
    }

    fn identity_state(&self, owner_uid: u32, after: u64) -> service_result_t<journal_page_t> {
        if self.faults.unavailable {
            return Err(ipc(ipc_code_t::storage_unavailable));
        }
        let Some(journal) = self.users.get(&owner_uid) else {
            return Ok(journal_page_t {
                entries: Vec::new(),
                next: None,
            });
        };
        let mut remaining = journal
            .entries
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

    fn journal_users(&self) -> service_result_t<Vec<u32>> {
        if self.faults.unavailable {
            return Err(ipc(ipc_code_t::storage_unavailable));
        }
        Ok(self.users.keys().copied().collect())
    }
}
