//! The run marker's companion: the restriction ledger (ADR 0006 §3), memory-backed
//! storage that outlives the process but not the boot (`RuntimeDirectory` on tmpfs in
//! deployment; any directory in tests, where the boot identifier is injected).
//!
//! Layout under the ledger directory: `validity` (`"C3RV"` ‖ run_id ‖ boot_id) names
//! the run whose entries follow; `entries` holds frames `"C3RL"` ‖ length ‖ entry ‖
//! SHA-256(entry). Each entry is a restrictive change written **before** any disk
//! write is attempted: `{run_id, kind, uid, payload}`, where a journal entry's payload
//! is the exact record registryd intends to append (its sequence and time included) so
//! that a restart in the same boot can tell which entries the journal already holds and
//! re-journal the rest; a policy entry carries the global policy state.
//!
//! If the ledger cannot be written, the validity file is removed (emptied, if it cannot
//! be removed: an empty validity file is invalid), registryd records the run as
//! unverifiable in the anchor's run marker and stops serving eligible decisions; the
//! next start then sees an unverifiable run. A validity file without an entries file
//! is no complete ledger either: `start` writes the entries file first.

use std::fs;
use std::io::{self, ErrorKind};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use cham3_contract_format::hash::sha256;

use crate::anchor::MaxBootIdBytes;
use crate::journal::{ensure_directory, journal_io_t, store_error_t};

pub const ValidityMagic: [u8; 4] = *b"C3RV";
pub const EntryMagic: [u8; 4] = *b"C3RL";
pub const ValidityFileName: &str = "validity";
pub const EntriesFileName: &str = "entries";
/// Largest entry payload (a journal record plus its envelope).
pub const MaxEntryBytes: usize = 70_000;
/// Largest ledger read at start.
pub const MaxLedgerBytes: u64 = 64 * 1024 * 1024;

/// What an entry records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum entry_kind_t {
    /// A journal record of `uid` (the payload is its exact encoding).
    journal_record,
    /// The global policy state (payload: epoch ‖ min_valid_epoch ‖ emergency byte).
    policy,
}

/// One ledger entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ledger_entry_t {
    pub run_id: [u8; 16],
    pub kind: entry_kind_t,
    pub uid: u32,
    pub payload: Vec<u8>,
}

/// The previous run's ledger as startup finds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum previous_ledger_t {
    /// No validity file: the ledger is gone (a new boot) or was never written.
    missing,
    /// A validity file or an entry is unreadable or corrupt.
    invalid,
    /// Valid for `run_id` in `boot_id`, with its entries in order.
    valid {
        run_id: [u8; 16],
        boot_id: String,
        entries: Vec<ledger_entry_t>,
    },
}

/// Source of the boot identifier (the kernel's in deployment; injected in tests).
pub trait boot_id_source_t: Send + Sync {
    fn boot_id(&self) -> Option<String>;
}

/// `/proc/sys/kernel/random/boot_id`.
#[derive(Clone, Copy, Debug, Default)]
pub struct kernel_boot_id_t;

impl boot_id_source_t for kernel_boot_id_t {
    fn boot_id(&self) -> Option<String> {
        let text = fs::read_to_string("/proc/sys/kernel/random/boot_id").ok()?;
        let trimmed = text.trim();
        (!trimmed.is_empty() && trimmed.len() <= MaxBootIdBytes).then(|| trimmed.to_owned())
    }
}

fn io_error(_: io::Error) -> store_error_t {
    store_error_t::io
}

fn encode_entry(entry: &ledger_entry_t) -> Option<Vec<u8>> {
    if entry.payload.len() > MaxEntryBytes {
        return None;
    }
    let mut body = Vec::with_capacity(entry.payload.len().checked_add(25)?);
    body.extend_from_slice(&entry.run_id);
    body.push(match entry.kind {
        entry_kind_t::journal_record => 1,
        entry_kind_t::policy => 2,
    });
    body.extend_from_slice(&entry.uid.to_be_bytes());
    body.extend_from_slice(&u32::try_from(entry.payload.len()).ok()?.to_be_bytes());
    body.extend_from_slice(&entry.payload);
    let mut frame = Vec::with_capacity(body.len().checked_add(40)?);
    frame.extend_from_slice(&EntryMagic);
    frame.extend_from_slice(&u32::try_from(body.len()).ok()?.to_be_bytes());
    frame.extend_from_slice(&body);
    frame.extend_from_slice(&sha256(&body));
    Some(frame)
}

fn decode_entries(data: &[u8]) -> Option<Vec<ledger_entry_t>> {
    let mut entries = Vec::new();
    let mut position = 0usize;
    while position < data.len() {
        let head = data.get(position..position.checked_add(8)?)?;
        let (magic, length) = head.split_at(4);
        if magic != EntryMagic {
            return None;
        }
        let length = usize::try_from(u32::from_be_bytes(<[u8; 4]>::try_from(length).ok()?)).ok()?;
        if length > MaxEntryBytes.checked_add(25)? {
            return None;
        }
        let body_start = position.checked_add(8)?;
        let body_end = body_start.checked_add(length)?;
        let body = data.get(body_start..body_end)?;
        let checksum = data.get(body_end..body_end.checked_add(32)?)?;
        if sha256(body) != checksum {
            return None;
        }
        let run_id = <[u8; 16]>::try_from(body.get(..16)?).ok()?;
        let kind = match body.get(16)? {
            1 => entry_kind_t::journal_record,
            2 => entry_kind_t::policy,
            _ => return None,
        };
        let uid = u32::from_be_bytes(<[u8; 4]>::try_from(body.get(17..21)?).ok()?);
        let payload_length = usize::try_from(u32::from_be_bytes(
            <[u8; 4]>::try_from(body.get(21..25)?).ok()?,
        ))
        .ok()?;
        let payload = body.get(25..25usize.checked_add(payload_length)?)?;
        if payload.len() != length.checked_sub(25)? {
            return None;
        }
        entries.push(ledger_entry_t {
            run_id,
            kind,
            uid,
            payload: payload.to_vec(),
        });
        position = body_end.checked_add(32)?;
    }
    Some(entries)
}

fn read_private_file(path: &Path, owner: u32) -> Result<Option<Vec<u8>>, store_error_t> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error(error)),
    };
    if !metadata.file_type().is_file() || metadata.uid() != owner || metadata.mode() & 0o077 != 0 {
        return Err(store_error_t::unsafe_path);
    }
    if metadata.len() > MaxLedgerBytes {
        return Err(store_error_t::too_large);
    }
    fs::read(path).map(Some).map_err(io_error)
}

/// Reads the previous run's ledger under `directory` (which may not exist yet).
pub fn read_previous(directory: &Path, owner: u32) -> previous_ledger_t {
    let validity = match read_private_file(&directory.join(ValidityFileName), owner) {
        Ok(None) => return previous_ledger_t::missing,
        Ok(Some(bytes)) => bytes,
        Err(_) => return previous_ledger_t::invalid,
    };
    let Some((magic, rest)) = validity.split_at_checked(4) else {
        return previous_ledger_t::invalid;
    };
    let Some((run_id, boot_id)) = rest.split_at_checked(16) else {
        return previous_ledger_t::invalid;
    };
    let (Ok(run_id), Ok(boot_id)) = (
        <[u8; 16]>::try_from(run_id),
        String::from_utf8(boot_id.to_vec()),
    ) else {
        return previous_ledger_t::invalid;
    };
    if magic != ValidityMagic || boot_id.is_empty() || boot_id.len() > MaxBootIdBytes {
        return previous_ledger_t::invalid;
    }
    // `start` writes the entries file before the validity file, so a validity file
    // without one is no complete ledger (M2-006-A-R06).
    let entries = match read_private_file(&directory.join(EntriesFileName), owner) {
        Ok(None) => return previous_ledger_t::invalid,
        Ok(Some(bytes)) => match decode_entries(&bytes) {
            Some(entries) if entries.iter().all(|entry| entry.run_id == run_id) => entries,
            _ => return previous_ledger_t::invalid,
        },
        Err(_) => return previous_ledger_t::invalid,
    };
    previous_ledger_t::valid {
        run_id,
        boot_id,
        entries,
    }
}

/// The ledger of the current run.
#[derive(Debug)]
pub struct ledger_t {
    directory: PathBuf,
    run_id: [u8; 16],
    boot_id: String,
    /// Writing failed once: the validity file is gone and nothing is written again.
    failed: bool,
}

impl ledger_t {
    /// Starts the ledger of a new run: an empty `entries` file and a `validity` file
    /// naming `run_id` and `boot_id`, both durable. The directory is created `0700`
    /// if missing. Replaces whatever the previous run left.
    pub fn start(
        directory: &Path,
        owner: u32,
        run_id: [u8; 16],
        boot_id: &str,
        io: &dyn journal_io_t,
    ) -> Result<ledger_t, store_error_t> {
        if boot_id.is_empty() || boot_id.len() > MaxBootIdBytes {
            return Err(store_error_t::unsafe_path);
        }
        ensure_directory(directory, owner)?;
        // The directory's entry in its parent is synced at every start, never inferred
        // from its existence (M2-005 R01, R2-01).
        let parent = directory.parent().ok_or(store_error_t::unsafe_path)?;
        io.sync_directory(parent).map_err(io_error)?;
        let validity_path = directory.join(ValidityFileName);
        // The old validity file goes first: a crash between the two writes leaves no
        // validity file (an unverifiable run), never a stale one over new entries.
        io.remove_file(&validity_path).map_err(io_error)?;
        io.replace_file(&directory.join(EntriesFileName), &[])
            .map_err(io_error)?;
        let mut validity = Vec::with_capacity(20usize.saturating_add(boot_id.len()));
        validity.extend_from_slice(&ValidityMagic);
        validity.extend_from_slice(&run_id);
        validity.extend_from_slice(boot_id.as_bytes());
        io.replace_file(&validity_path, &validity)
            .map_err(io_error)?;
        io.sync_directory(directory).map_err(io_error)?;
        Ok(ledger_t {
            directory: directory.to_path_buf(),
            run_id,
            boot_id: boot_id.to_owned(),
            failed: false,
        })
    }

    pub fn run_id(&self) -> [u8; 16] {
        self.run_id
    }

    pub fn boot_id(&self) -> &str {
        &self.boot_id
    }

    /// Whether the ledger can no longer be written (ADR 0006 §3: no eligible
    /// decisions are served).
    pub fn failed(&self) -> bool {
        self.failed
    }

    /// Appends one entry durably. On failure the ledger is marked failed and its
    /// validity file is removed — emptied, if it cannot be removed (an empty validity
    /// file is invalid); the registry also records the run as unverifiable in the
    /// anchor's run marker. The error is returned: `io` for the failure itself,
    /// `failed_earlier` for every later append.
    pub fn append(
        &mut self,
        io: &dyn journal_io_t,
        kind: entry_kind_t,
        uid: u32,
        payload: &[u8],
    ) -> Result<(), store_error_t> {
        if self.failed {
            return Err(store_error_t::failed_earlier);
        }
        let entry = ledger_entry_t {
            run_id: self.run_id,
            kind,
            uid,
            payload: payload.to_vec(),
        };
        let frame = encode_entry(&entry).ok_or(store_error_t::too_large)?;
        match io.append_file(&self.directory.join(EntriesFileName), &frame) {
            Ok(()) => Ok(()),
            Err(_) => {
                self.failed = true;
                let validity = self.directory.join(ValidityFileName);
                if io.remove_file(&validity).is_err() {
                    let _ = io.replace_file(&validity, &[]);
                }
                Err(store_error_t::io)
            }
        }
    }

    /// Reads back this run's entries (tests and the restart path of the same process).
    pub fn entries(&self, owner: u32) -> Result<Vec<ledger_entry_t>, store_error_t> {
        match read_private_file(&self.directory.join(EntriesFileName), owner)? {
            None => Ok(Vec::new()),
            Some(bytes) => decode_entries(&bytes).ok_or(store_error_t::io),
        }
    }
}

/// The policy payload of a `policy` entry.
pub fn policy_payload(epoch: u64, min_valid_epoch: u64, emergency: bool) -> Vec<u8> {
    let mut payload = Vec::with_capacity(17);
    payload.extend_from_slice(&epoch.to_be_bytes());
    payload.extend_from_slice(&min_valid_epoch.to_be_bytes());
    payload.push(u8::from(emergency));
    payload
}

/// Parses a `policy` payload.
pub fn parse_policy_payload(payload: &[u8]) -> Option<(u64, u64, bool)> {
    let (epoch, rest) = payload.split_at_checked(8)?;
    let (min_valid, rest) = rest.split_at_checked(8)?;
    let emergency = match rest {
        [0] => false,
        [1] => true,
        _ => return None,
    };
    Some((
        u64::from_be_bytes(<[u8; 8]>::try_from(epoch).ok()?),
        u64::from_be_bytes(<[u8; 8]>::try_from(min_valid).ok()?),
        emergency,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_round_trip_and_reject_corruption() {
        let entries = vec![
            ledger_entry_t {
                run_id: [1; 16],
                kind: entry_kind_t::journal_record,
                uid: 1001,
                payload: vec![9; 100],
            },
            ledger_entry_t {
                run_id: [1; 16],
                kind: entry_kind_t::policy,
                uid: 0,
                payload: policy_payload(2, 1, true),
            },
        ];
        let mut bytes = Vec::new();
        for entry in &entries {
            bytes.extend(encode_entry(entry).expect("encodable"));
        }
        assert_eq!(decode_entries(&bytes), Some(entries.clone()));
        let mut corrupt = bytes.clone();
        corrupt[30] ^= 1;
        assert_eq!(decode_entries(&corrupt), None);
        // A torn last frame invalidates the ledger (fail closed: containment).
        assert_eq!(decode_entries(&bytes[..bytes.len() - 5]), None);
        assert_eq!(decode_entries(&[]), Some(Vec::new()));
        assert_eq!(
            parse_policy_payload(&policy_payload(2, 1, true)),
            Some((2, 1, true))
        );
        assert_eq!(parse_policy_payload(&[0; 16]), None);
    }
}
