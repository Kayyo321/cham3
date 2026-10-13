//! The vault's files (ADR 0005 §4): `<root>/<uid>/<key_id hex>.key` key objects,
//! plus per-user records (`pending.reg`, `attempts.rec`).
//!
//! Directories are `0700` and files `0600`, owned by the vault's effective UID; any
//! other owner, mode, file type or a symbolic link is refused (fail closed). Writes go
//! to a temporary file that is fsynced, renamed into place and followed by a directory
//! fsync. Erasure overwrites the file with zeros, fsyncs, unlinks and fsyncs the
//! directory; on SSDs and journaling or copy-on-write file systems the old blocks may
//! survive (stated limit, ADR 0008 §4).

use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use cham3_contract_format::hash::digest_t;
use cham3_os_adapter::effective_uid;

use crate::key_object::{MaxObjectBytes, key_object_t};

/// Largest per-user record read (pending registration bundle, attempt counter).
pub const MaxRecordBytes: usize = 16 * 1024;
/// Names of the per-user records.
pub const PendingRecord: &str = "pending.reg";
pub const AttemptsRecord: &str = "attempts.rec";

/// Store failure. Contains no secret data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum store_error_t {
    /// A path has the wrong type, owner or mode, or is a symbolic link.
    unsafe_path,
    /// An object or record is malformed or too large.
    malformed,
    /// A key object for this key id already exists.
    exists,
    /// An I/O operation failed.
    io,
}

fn io_error(_: std::io::Error) -> store_error_t {
    store_error_t::io
}

/// The vault directory tree.
#[derive(Clone, Debug)]
pub struct key_store_t {
    root: PathBuf,
    owner: u32,
}

fn hex(bytes: &[u8]) -> String {
    const Digits: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::new();
    for byte in bytes {
        for nibble in [byte >> 4, byte & 0x0f] {
            if let Some(digit) = Digits.get(usize::from(nibble)) {
                text.push(char::from(*digit));
            }
        }
    }
    text
}

fn check_directory(path: &Path, owner: u32) -> Result<(), store_error_t> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if !metadata.file_type().is_dir() || metadata.uid() != owner || metadata.mode() & 0o077 != 0 {
        return Err(store_error_t::unsafe_path);
    }
    Ok(())
}

fn ensure_directory(path: &Path, owner: u32) -> Result<(), store_error_t> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => return Err(io_error(error)),
    }
    check_directory(path, owner)
}

fn sync_directory(path: &Path) -> Result<(), store_error_t> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(io_error)
}

impl key_store_t {
    /// Opens (creating if needed) the vault root, which must be a `0700` directory of
    /// the vault's effective UID.
    pub fn open(root: &Path) -> Result<key_store_t, store_error_t> {
        let owner = effective_uid();
        ensure_directory(root, owner)?;
        Ok(key_store_t {
            root: root.to_path_buf(),
            owner,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn user_directory(&self, uid: u32) -> Result<PathBuf, store_error_t> {
        let directory = self.root.join(uid.to_string());
        ensure_directory(&directory, self.owner)?;
        Ok(directory)
    }

    fn key_path(&self, uid: u32, key_id: &digest_t) -> Result<PathBuf, store_error_t> {
        Ok(self
            .user_directory(uid)?
            .join(format!("{}.key", hex(key_id))))
    }

    /// Reads a file of at most `limit` bytes after checking it is a regular `0600`
    /// file of the vault UID; `None` if it does not exist.
    fn read_file(&self, path: &Path, limit: usize) -> Result<Option<Vec<u8>>, store_error_t> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(io_error(error)),
        };
        if !metadata.file_type().is_file()
            || metadata.uid() != self.owner
            || metadata.mode() & 0o777 != 0o600
        {
            return Err(store_error_t::unsafe_path);
        }
        let mut file = File::open(path).map_err(io_error)?;
        let opened = file.metadata().map_err(io_error)?;
        if (opened.dev(), opened.ino()) != (metadata.dev(), metadata.ino()) {
            return Err(store_error_t::unsafe_path);
        }
        let mut bytes = Vec::new();
        let limit_plus_one = u64::try_from(limit)
            .ok()
            .and_then(|limit| limit.checked_add(1))
            .ok_or(store_error_t::malformed)?;
        Read::take(&mut file, limit_plus_one)
            .read_to_end(&mut bytes)
            .map_err(io_error)?;
        if bytes.len() > limit {
            return Err(store_error_t::malformed);
        }
        Ok(Some(bytes))
    }

    /// Writes `bytes` at `path` atomically (temp, fsync, rename, directory fsync).
    fn write_file(&self, directory: &Path, path: &Path, bytes: &[u8]) -> Result<(), store_error_t> {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(store_error_t::unsafe_path)?;
        let temporary = directory.join(format!(".{name}.tmp"));
        match fs::remove_file(&temporary) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(error)),
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(io_error)?;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(io_error)?;
        file.write_all(bytes).map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        drop(file);
        fs::rename(&temporary, path).map_err(io_error)?;
        sync_directory(directory)
    }

    /// Overwrites a file with zeros, fsyncs, unlinks and fsyncs the directory.
    fn erase_file(&self, directory: &Path, path: &Path) -> Result<(), store_error_t> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(io_error(error)),
        };
        if !metadata.file_type().is_file() || metadata.uid() != self.owner {
            return Err(store_error_t::unsafe_path);
        }
        let length = usize::try_from(metadata.len()).map_err(|_| store_error_t::malformed)?;
        let mut file = OpenOptions::new()
            .write(true)
            .open(path)
            .map_err(io_error)?;
        file.write_all(&vec![0u8; length]).map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        drop(file);
        fs::remove_file(path).map_err(io_error)?;
        sync_directory(directory)
    }

    /// Writes a new key object; an existing object for the key is never replaced.
    pub fn write_key(&self, object: &key_object_t) -> Result<(), store_error_t> {
        let uid = object.metadata.owner_uid;
        let path = self.key_path(uid, &object.metadata.key_id)?;
        if fs::symlink_metadata(&path).is_ok() {
            return Err(store_error_t::exists);
        }
        self.write_file(&self.user_directory(uid)?, &path, &object.encode())
    }

    /// Reads the key object of `key_id` in `uid`'s directory and checks that it names
    /// that owner and key.
    pub fn read_key(
        &self,
        uid: u32,
        key_id: &digest_t,
    ) -> Result<Option<key_object_t>, store_error_t> {
        let Some(bytes) = self.read_file(&self.key_path(uid, key_id)?, MaxObjectBytes)? else {
            return Ok(None);
        };
        let object = key_object_t::decode(&bytes).map_err(|_| store_error_t::malformed)?;
        if object.metadata.owner_uid != uid || object.metadata.key_id != *key_id {
            return Err(store_error_t::malformed);
        }
        Ok(Some(object))
    }

    /// Key ids with an object in `uid`'s directory.
    pub fn list_keys(&self, uid: u32) -> Result<Vec<digest_t>, store_error_t> {
        let directory = self.user_directory(uid)?;
        let mut keys = Vec::new();
        for entry in fs::read_dir(&directory).map_err(io_error)? {
            let name = entry.map_err(io_error)?.file_name();
            let Some(stem) = name.to_str().and_then(|name| name.strip_suffix(".key")) else {
                continue;
            };
            if let Some(key_id) = parse_hex_digest(stem) {
                keys.push(key_id);
            }
        }
        keys.sort_unstable();
        Ok(keys)
    }

    pub fn erase_key(&self, uid: u32, key_id: &digest_t) -> Result<(), store_error_t> {
        let directory = self.user_directory(uid)?;
        self.erase_file(&directory, &self.key_path(uid, key_id)?)
    }

    pub fn read_record(&self, uid: u32, name: &str) -> Result<Option<Vec<u8>>, store_error_t> {
        self.read_file(&self.user_directory(uid)?.join(name), MaxRecordBytes)
    }

    pub fn write_record(&self, uid: u32, name: &str, bytes: &[u8]) -> Result<(), store_error_t> {
        if bytes.len() > MaxRecordBytes {
            return Err(store_error_t::malformed);
        }
        let directory = self.user_directory(uid)?;
        self.write_file(&directory, &directory.join(name), bytes)
    }

    pub fn remove_record(&self, uid: u32, name: &str) -> Result<(), store_error_t> {
        let directory = self.user_directory(uid)?;
        self.erase_file(&directory, &directory.join(name))
    }
}

fn parse_hex_digest(text: &str) -> Option<digest_t> {
    if text.len() != 64 {
        return None;
    }
    let mut digest = [0u8; 32];
    for (slot, pair) in digest.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        let pair = std::str::from_utf8(pair).ok()?;
        if pair.bytes().any(|byte| byte.is_ascii_uppercase()) {
            return None;
        }
        *slot = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(digest)
}
