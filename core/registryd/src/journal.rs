//! Journal files on disk (ADR 0006 §1): `<root>/users/<uid>/journal.c3j`.
//!
//! Directories are `0700` and journal files `0600`, owned by registryd's effective
//! UID; any other owner, mode, file type or a symbolic link is refused (fail closed).
//! An append writes one complete frame at the end of the file and returns only after
//! `fdatasync`; creating a journal also syncs its directory (and the `users`
//! directory when it was created). A failed append leaves the journal unusable for
//! the rest of the run: the caller denies the user until a restart replays the file
//! (torn-tail recovery and the anchor are M2-006).
//!
//! [`journal_io_t`] is the boundary for fault injection: tests substitute writes and
//! syncs that fail, or write only part of a frame.

use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use cham3_contract_format::hash::digest_t;

use crate::record::{frame, genesis_chain, header, next_chain, parse_journal, parsed_journal_t};

/// Name of a user's journal file.
pub const JournalFileName: &str = "journal.c3j";

/// The writes and syncs of the journal.
pub trait journal_io_t: Send + Sync {
    /// Writes all of `bytes` at the current end of `file`.
    fn write_all(&self, file: &mut File, bytes: &[u8]) -> io::Result<()>;
    /// `fdatasync`.
    fn sync_data(&self, file: &File) -> io::Result<()>;
    /// `fsync` of a directory.
    fn sync_directory(&self, path: &Path) -> io::Result<()>;
}

/// The real file system.
#[derive(Clone, Copy, Debug, Default)]
pub struct system_io_t;

impl journal_io_t for system_io_t {
    fn write_all(&self, file: &mut File, bytes: &[u8]) -> io::Result<()> {
        file.write_all(bytes)
    }

    fn sync_data(&self, file: &File) -> io::Result<()> {
        file.sync_data()
    }

    fn sync_directory(&self, path: &Path) -> io::Result<()> {
        File::open(path).and_then(|directory| directory.sync_all())
    }
}

/// A store failure. Contains no record content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum store_error_t {
    /// A path has the wrong type, owner or mode, or is a symbolic link.
    unsafe_path,
    /// The `users` directory holds an entry that is not a canonical UID.
    unexpected_entry,
    /// The journal already exists (creation) or is larger than the read limit.
    exists,
    too_large,
    /// An earlier append failed in this run.
    failed_earlier,
    /// An I/O operation failed.
    io,
}

fn io_error(_: io::Error) -> store_error_t {
    store_error_t::io
}

fn check_directory(path: &Path, owner: u32) -> Result<(), store_error_t> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if !metadata.file_type().is_dir() || metadata.uid() != owner || metadata.mode() & 0o077 != 0 {
        return Err(store_error_t::unsafe_path);
    }
    Ok(())
}

/// Creates `path` (mode `0700`) if needed and checks it; reports whether it was new.
fn ensure_directory(path: &Path, owner: u32) -> Result<bool, store_error_t> {
    let created = match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => true,
        Err(error) if error.kind() == ErrorKind::AlreadyExists => false,
        Err(error) => return Err(io_error(error)),
    };
    check_directory(path, owner)?;
    Ok(created)
}

/// Checks the journal path (a regular `0600` file of `owner`, not a symbolic link)
/// and returns its (device, inode).
fn check_journal_file(path: &Path, owner: u32) -> Result<(u64, u64), store_error_t> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if !metadata.file_type().is_file() || metadata.uid() != owner || metadata.mode() & 0o077 != 0 {
        return Err(store_error_t::unsafe_path);
    }
    Ok((metadata.dev(), metadata.ino()))
}

/// The opened file must be the file that was checked (no path replacement between
/// the check and the open).
fn check_same_file(file: &File, checked: (u64, u64)) -> Result<(), store_error_t> {
    let metadata = file.metadata().map_err(io_error)?;
    if (metadata.dev(), metadata.ino()) != checked {
        return Err(store_error_t::unsafe_path);
    }
    Ok(())
}

/// The registry's directory tree.
#[derive(Clone, Debug)]
pub struct journal_store_t {
    root: PathBuf,
    owner: u32,
}

/// Canonical decimal UID of a directory name (no sign, no leading zeros).
fn parse_uid(name: &str) -> Option<u32> {
    let canonical = !name.is_empty()
        && name.bytes().all(|byte| byte.is_ascii_digit())
        && (name == "0" || !name.starts_with('0'));
    canonical.then(|| name.parse().ok()).flatten()
}

impl journal_store_t {
    /// Opens (creating if needed) `root` and `root/users`, both `0700` directories of
    /// `owner` (registryd's effective UID).
    pub fn open(root: &Path, owner: u32) -> Result<journal_store_t, store_error_t> {
        ensure_directory(root, owner)?;
        ensure_directory(&root.join("users"), owner)?;
        Ok(journal_store_t {
            root: root.to_path_buf(),
            owner,
        })
    }

    fn users_directory(&self) -> PathBuf {
        self.root.join("users")
    }

    fn user_directory(&self, uid: u32) -> PathBuf {
        self.users_directory().join(uid.to_string())
    }

    /// UIDs with a user directory. Any other entry refuses the whole store.
    pub fn users(&self) -> Result<Vec<u32>, store_error_t> {
        let mut uids = Vec::new();
        for entry in fs::read_dir(self.users_directory()).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            let name = entry.file_name();
            let uid = name
                .to_str()
                .and_then(parse_uid)
                .ok_or(store_error_t::unexpected_entry)?;
            check_directory(&entry.path(), self.owner)?;
            uids.push(uid);
        }
        uids.sort_unstable();
        Ok(uids)
    }

    /// Reads a user's journal file of at most `limit` bytes; `None` if the user
    /// directory has no journal file.
    pub fn read(&self, uid: u32, limit: u64) -> Result<Option<Vec<u8>>, store_error_t> {
        let directory = self.user_directory(uid);
        check_directory(&directory, self.owner)?;
        let path = directory.join(JournalFileName);
        match fs::symlink_metadata(&path) {
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(io_error(error)),
        }
        let checked = check_journal_file(&path, self.owner)?;
        let file = OpenOptions::new()
            .read(true)
            .open(&path)
            .map_err(io_error)?;
        check_same_file(&file, checked)?;
        let mut bytes = Vec::new();
        file.take(limit.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(io_error)?;
        if u64::try_from(bytes.len()).map_or(true, |length| length > limit) {
            return Err(store_error_t::too_large);
        }
        Ok(Some(bytes))
    }

    /// Opens an existing journal for appending after its replay parsed it.
    pub fn open_journal(
        &self,
        uid: u32,
        parsed: &parsed_journal_t,
        length: u64,
    ) -> Result<journal_file_t, store_error_t> {
        let path = self.user_directory(uid).join(JournalFileName);
        let checked = check_journal_file(&path, self.owner)?;
        let file = OpenOptions::new()
            .append(true)
            .open(&path)
            .map_err(io_error)?;
        check_same_file(&file, checked)?;
        let metadata = file.metadata().map_err(io_error)?;
        if metadata.len() != length {
            return Err(store_error_t::io);
        }
        Ok(journal_file_t {
            file,
            length,
            chain: parsed.chain,
            count: u64::try_from(parsed.records.len()).map_err(|_| store_error_t::too_large)?,
            failed: false,
        })
    }

    /// Creates a user's journal with its header and first record (the enrollment)
    /// and makes both durable. Refuses an existing journal.
    pub fn create_journal(
        &self,
        uid: u32,
        user_id: &digest_t,
        first_record: &[u8],
        io: &dyn journal_io_t,
    ) -> Result<journal_file_t, store_error_t> {
        let directory = self.user_directory(uid);
        let new_directory = ensure_directory(&directory, self.owner)?;
        if new_directory {
            io.sync_directory(&self.users_directory())
                .map_err(io_error)?;
        }
        let path = directory.join(JournalFileName);
        let mut file = match OpenOptions::new()
            .append(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                return Err(store_error_t::exists);
            }
            Err(error) => return Err(io_error(error)),
        };
        let chain = next_chain(&genesis_chain(user_id, uid), 0, first_record);
        let mut bytes = header(uid, user_id);
        bytes.extend(frame(first_record, &chain).ok_or(store_error_t::too_large)?);
        io.write_all(&mut file, &bytes).map_err(io_error)?;
        io.sync_data(&file).map_err(io_error)?;
        io.sync_directory(&directory).map_err(io_error)?;
        Ok(journal_file_t {
            file,
            length: u64::try_from(bytes.len()).map_err(|_| store_error_t::too_large)?,
            chain,
            count: 1,
            failed: false,
        })
    }
}

/// An open journal file positioned for appends.
#[derive(Debug)]
pub struct journal_file_t {
    file: File,
    length: u64,
    chain: digest_t,
    count: u64,
    failed: bool,
}

impl journal_file_t {
    /// Sequence the next record must carry.
    pub fn next_sequence(&self) -> Option<u64> {
        self.count.checked_add(1)
    }

    /// Records in the journal.
    pub fn record_count(&self) -> u64 {
        self.count
    }

    /// Bytes in the journal file.
    pub fn length(&self) -> u64 {
        self.length
    }

    /// Whether an append failed in this run.
    pub fn failed(&self) -> bool {
        self.failed
    }

    /// Bytes an append of `record_length` would add.
    pub fn frame_length(record_length: usize) -> Option<u64> {
        u64::try_from(record_length.checked_add(crate::record::FrameOverheadBytes)?).ok()
    }

    /// Appends one record (whose `seq` is [`journal_file_t::next_sequence`]) and makes
    /// it durable. On any failure the journal refuses every later append.
    pub fn append(&mut self, record: &[u8], io: &dyn journal_io_t) -> Result<u64, store_error_t> {
        if self.failed {
            return Err(store_error_t::failed_earlier);
        }
        let sequence = self.next_sequence().ok_or(store_error_t::too_large)?;
        let chain = next_chain(&self.chain, self.count, record);
        let bytes = frame(record, &chain).ok_or(store_error_t::too_large)?;
        let added = u64::try_from(bytes.len()).map_err(|_| store_error_t::too_large)?;
        let written = io
            .write_all(&mut self.file, &bytes)
            .and_then(|()| io.sync_data(&self.file));
        if written.is_err() {
            self.failed = true;
            return Err(store_error_t::io);
        }
        self.length = self
            .length
            .checked_add(added)
            .ok_or(store_error_t::too_large)?;
        self.chain = chain;
        self.count = sequence;
        Ok(sequence)
    }
}

/// Parses and chain-verifies a journal file (re-exported for the replay).
pub fn parse(bytes: &[u8], uid: u32) -> Result<parsed_journal_t, crate::record::journal_defect_t> {
    parse_journal(bytes, uid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uid_names_are_canonical() {
        assert_eq!(parse_uid("1001"), Some(1001));
        assert_eq!(parse_uid("0"), Some(0));
        for name in ["", "01001", "+1001", "-1", "10a", "4294967296", " 1001"] {
            assert_eq!(parse_uid(name), None, "{name:?}");
        }
    }
}
