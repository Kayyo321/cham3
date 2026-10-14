//! The global anchor (ADR 0006 §2): one map, held authoritatively in memory and
//! written as two alternating slots, each `"C3AN"` ‖ version ‖ slot counter ‖ map ‖
//! SHA-256 of the preceding bytes. Startup takes the valid slot with the higher
//! counter. Every update serializes the whole map to the older slot and makes it
//! durable before the caller is answered; the in-memory map changes only when the
//! slot holding the change is durable, so a failed slot write leaves the change
//! non-durable and the map as it was.
//!
//! The map holds, per user, `(seq, chain, state, user_id, identity_seq,
//! min_valid_policy_epoch)`; the deleted-user tombstones (§7); the run marker (§3);
//! the policy state that restrictive administrative changes persist (policy
//! epoch, minimum valid epoch, emergency restriction; ADR 0007 §1 rule 4); and the
//! trusted-time floor (ADR 0007 §5 rule 3, erratum E6: format version 2). The
//! serialization is a fixed binary layout rather than `cham3_dcbor_v1`, whose profile
//! caps a container at 256 items: an anchor names every enrolled user (author
//! decision, revisable).
//!
//! Lock order (ADR 0006 §2): user lock → anchor lock; no code takes a user lock while
//! holding the anchor lock.
//!
//! Backends (§2, owner choice §9.1 option A): the anchor lives under the store root
//! unless a separate path is configured; it is `independent` when that path is on a
//! different filesystem (`st_dev` differs from the store's), else `degraded` (a
//! whole-volume restore cannot be detected). `tpm2_nv` is not in M2.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, ErrorKind};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use cham3_contract_format::hash::{digest_t, sha256};

use crate::clock::time_floor_t;
use crate::journal::{journal_io_t, store_error_t};

/// Magic of an anchor slot.
pub const AnchorMagic: [u8; 4] = *b"C3AN";
/// Version 2 adds the trusted-time floor (ADR 0007 erratum E6).
pub const AnchorVersion: u16 = 2;
/// The two slot files under the anchor directory.
pub const SlotFileNames: [&str; 2] = ["slot_a", "slot_b"];
/// Longest boot identifier stored (the kernel's UUID text is 36 bytes).
pub const MaxBootIdBytes: usize = 64;
/// Longest slot read (every user costs 93 bytes; this allows far more than the
/// user-ID space in use on one machine).
pub const MaxSlotBytes: u64 = 64 * 1024 * 1024;

/// State of the registryd run marker (ADR 0006 §3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum run_state_t {
    /// Replay finished; restrictive changes of this run may be non-durable.
    running,
    /// An orderly shutdown made every restrictive change durable.
    clean,
    /// The restriction ledger of this run failed (ADR 0006 §3 mechanism 2): whatever
    /// the ledger files hold, the run cannot be verified, and the next start contains
    /// every user.
    unverifiable,
}

/// The run marker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct run_marker_t {
    pub run_id: [u8; 16],
    pub boot_id: String,
    pub state: run_state_t,
}

/// Global policy state persisted through the restrictive path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct policy_state_t {
    pub epoch: u64,
    pub min_valid_epoch: u64,
    pub emergency_restriction: bool,
}

/// A user's state as the anchor records it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum anchored_state_t {
    normal,
    contained,
    quarantined,
}

/// One user's anchored position (ADR 0006 §2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct anchored_user_t {
    pub seq: u64,
    pub chain: digest_t,
    pub user_id: digest_t,
    pub state: anchored_state_t,
    /// Identity records (enrollment, certificates, invalidations) in the journal up to
    /// `seq`, so a lost tail that held key-state changes is visible (§§5, 10).
    pub identity_seq: u64,
    /// Raised by `recover_user` (§4); grants admitted under a lower epoch are ended.
    pub min_valid_policy_epoch: u64,
}

/// A deleted-user tombstone (§7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum tombstone_state_t {
    pending,
    deleted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct tombstone_t {
    pub uid: u32,
    pub state: tombstone_state_t,
}

/// The anchor map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct anchor_map_t {
    pub run_marker: Option<run_marker_t>,
    pub policy: policy_state_t,
    pub users: BTreeMap<u32, anchored_user_t>,
    /// Keyed by `user_id`.
    pub tombstones: BTreeMap<digest_t, tombstone_t>,
    /// The trusted-time floor (ADR 0007 §5 rule 3, erratum E6), published with every
    /// write that carries one.
    pub time_floor: Option<time_floor_t>,
}

impl anchor_map_t {
    pub fn new(policy: policy_state_t) -> anchor_map_t {
        anchor_map_t {
            run_marker: None,
            policy,
            users: BTreeMap::new(),
            tombstones: BTreeMap::new(),
            time_floor: None,
        }
    }
}

/// Rollback domain of the anchor (ADR 0006 §2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum anchor_mode_t {
    /// A configured path on a different filesystem than the store.
    independent,
    /// Same filesystem as the store: partial restores, torn tails and journal-only
    /// substitution are detected; a whole-volume restore is not.
    degraded,
}

impl anchor_mode_t {
    pub fn as_str(self) -> &'static str {
        match self {
            anchor_mode_t::independent => "independent",
            anchor_mode_t::degraded => "degraded",
        }
    }
}

/// What startup found in the slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum anchor_start_t {
    /// No slot exists: a store without an anchor (genesis, or a store that lost it).
    absent,
    /// A valid slot was loaded.
    loaded,
    /// Slots exist but none is valid (§4: quarantine every user).
    invalid,
}

// ------------------------------------------------------------------ serialization

struct writer_t {
    bytes: Vec<u8>,
}

impl writer_t {
    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn fixed(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }
}

struct reader_t<'data> {
    data: &'data [u8],
    position: usize,
}

impl reader_t<'_> {
    fn take(&mut self, count: usize) -> Option<&[u8]> {
        let end = self.position.checked_add(count)?;
        let chunk = self.data.get(self.position..end)?;
        self.position = end;
        Some(chunk)
    }

    fn u8(&mut self) -> Option<u8> {
        self.take(1).and_then(|chunk| chunk.first().copied())
    }

    fn u16(&mut self) -> Option<u16> {
        self.take(2)
            .and_then(|chunk| <[u8; 2]>::try_from(chunk).ok())
            .map(u16::from_be_bytes)
    }

    fn u32(&mut self) -> Option<u32> {
        self.take(4)
            .and_then(|chunk| <[u8; 4]>::try_from(chunk).ok())
            .map(u32::from_be_bytes)
    }

    fn u64(&mut self) -> Option<u64> {
        self.take(8)
            .and_then(|chunk| <[u8; 8]>::try_from(chunk).ok())
            .map(u64::from_be_bytes)
    }

    fn digest(&mut self) -> Option<digest_t> {
        self.take(32)
            .and_then(|chunk| digest_t::try_from(chunk).ok())
    }

    fn finished(&self) -> bool {
        self.position == self.data.len()
    }
}

fn state_byte(state: anchored_state_t) -> u8 {
    match state {
        anchored_state_t::normal => 0,
        anchored_state_t::contained => 1,
        anchored_state_t::quarantined => 2,
    }
}

fn parse_state(byte: u8) -> Option<anchored_state_t> {
    match byte {
        0 => Some(anchored_state_t::normal),
        1 => Some(anchored_state_t::contained),
        2 => Some(anchored_state_t::quarantined),
        _ => None,
    }
}

/// The bytes of a slot holding `map` at `counter`.
pub fn encode_slot(counter: u64, map: &anchor_map_t) -> Option<Vec<u8>> {
    let mut writer = writer_t { bytes: Vec::new() };
    writer.fixed(&AnchorMagic);
    writer.u16(AnchorVersion);
    writer.u64(counter);
    match &map.run_marker {
        None => writer.u8(0),
        Some(marker) => {
            writer.u8(match marker.state {
                run_state_t::running => 1,
                run_state_t::clean => 2,
                run_state_t::unverifiable => 3,
            });
            writer.fixed(&marker.run_id);
            let boot_id = marker.boot_id.as_bytes();
            writer.u8(u8::try_from(boot_id.len())
                .ok()
                .filter(|_| boot_id.len() <= MaxBootIdBytes)?);
            writer.fixed(boot_id);
        }
    }
    writer.u64(map.policy.epoch);
    writer.u64(map.policy.min_valid_epoch);
    writer.u8(u8::from(map.policy.emergency_restriction));
    writer.u32(u32::try_from(map.users.len()).ok()?);
    for (uid, user) in &map.users {
        writer.u32(*uid);
        writer.u64(user.seq);
        writer.fixed(&user.chain);
        writer.fixed(&user.user_id);
        writer.u8(state_byte(user.state));
        writer.u64(user.identity_seq);
        writer.u64(user.min_valid_policy_epoch);
    }
    writer.u32(u32::try_from(map.tombstones.len()).ok()?);
    for (user_id, tombstone) in &map.tombstones {
        writer.fixed(user_id);
        writer.u32(tombstone.uid);
        writer.u8(match tombstone.state {
            tombstone_state_t::pending => 1,
            tombstone_state_t::deleted => 2,
        });
    }
    match &map.time_floor {
        None => writer.u8(0),
        Some(floor) => {
            writer.u8(1);
            let boot_id = floor.boot_id.as_bytes();
            writer.u8(u8::try_from(boot_id.len())
                .ok()
                .filter(|length| (1..=MaxBootIdBytes).contains(&usize::from(*length)))?);
            writer.fixed(boot_id);
            writer.u64(floor.boottime);
            writer.u64(floor.trusted);
            writer.u64(floor.observed);
            writer.u64(floor.anomaly);
        }
    }
    let checksum = sha256(&writer.bytes);
    writer.fixed(&checksum);
    Some(writer.bytes)
}

/// Parses a slot: `None` for any defect (magic, version, length, checksum, field).
pub fn decode_slot(bytes: &[u8]) -> Option<(u64, anchor_map_t)> {
    let body_length = bytes.len().checked_sub(32)?;
    let (body, checksum) = bytes.split_at(body_length);
    if sha256(body) != checksum {
        return None;
    }
    let mut reader = reader_t {
        data: body,
        position: 0,
    };
    if reader.take(4)? != AnchorMagic || reader.u16()? != AnchorVersion {
        return None;
    }
    let counter = reader.u64()?;
    let run_marker = match reader.u8()? {
        0 => None,
        state @ 1..=3 => {
            let run_id = <[u8; 16]>::try_from(reader.take(16)?).ok()?;
            let length = usize::from(reader.u8()?);
            if length > MaxBootIdBytes {
                return None;
            }
            let boot_id = String::from_utf8(reader.take(length)?.to_vec()).ok()?;
            Some(run_marker_t {
                run_id,
                boot_id,
                state: match state {
                    1 => run_state_t::running,
                    2 => run_state_t::clean,
                    _ => run_state_t::unverifiable,
                },
            })
        }
        _ => return None,
    };
    let policy = policy_state_t {
        epoch: reader.u64()?,
        min_valid_epoch: reader.u64()?,
        emergency_restriction: match reader.u8()? {
            0 => false,
            1 => true,
            _ => return None,
        },
    };
    let user_count = reader.u32()?;
    let mut users = BTreeMap::new();
    let mut previous_uid: Option<u32> = None;
    for _ in 0..user_count {
        let uid = reader.u32()?;
        // Strictly ascending: one encoding per map.
        if previous_uid.is_some_and(|previous| previous >= uid) {
            return None;
        }
        previous_uid = Some(uid);
        let user = anchored_user_t {
            seq: reader.u64()?,
            chain: reader.digest()?,
            user_id: reader.digest()?,
            state: parse_state(reader.u8()?)?,
            identity_seq: reader.u64()?,
            min_valid_policy_epoch: reader.u64()?,
        };
        users.insert(uid, user);
    }
    let tombstone_count = reader.u32()?;
    let mut tombstones = BTreeMap::new();
    let mut previous_id: Option<digest_t> = None;
    for _ in 0..tombstone_count {
        let user_id = reader.digest()?;
        if previous_id.is_some_and(|previous| previous >= user_id) {
            return None;
        }
        previous_id = Some(user_id);
        let tombstone = tombstone_t {
            uid: reader.u32()?,
            state: match reader.u8()? {
                1 => tombstone_state_t::pending,
                2 => tombstone_state_t::deleted,
                _ => return None,
            },
        };
        tombstones.insert(user_id, tombstone);
    }
    let time_floor = match reader.u8()? {
        0 => None,
        1 => {
            let length = usize::from(reader.u8()?);
            if !(1..=MaxBootIdBytes).contains(&length) {
                return None;
            }
            let boot_id = String::from_utf8(reader.take(length)?.to_vec()).ok()?;
            Some(time_floor_t {
                boot_id,
                boottime: reader.u64()?,
                trusted: reader.u64()?,
                observed: reader.u64()?,
                anomaly: reader.u64()?,
            })
        }
        _ => return None,
    };
    if !reader.finished() {
        return None;
    }
    Some((
        counter,
        anchor_map_t {
            run_marker,
            policy,
            users,
            tombstones,
            time_floor,
        },
    ))
}

// ------------------------------------------------------------------------ runtime

/// The anchor in memory with its slot files.
#[derive(Debug)]
pub struct anchor_t {
    directory: PathBuf,
    mode: anchor_mode_t,
    map: anchor_map_t,
    counter: u64,
    /// Index of the slot that holds `counter`; the next write goes to the other.
    current_slot: usize,
    /// Per slot: whether this run has synced the directory after writing it. A slot's
    /// existence is no proof that its entry is durable (M2-005 R2-01), so the first
    /// write to each slot in a run syncs the directory.
    entry_synced: [bool; 2],
    /// Set once this run is unverifiable (a failed ledger, M2-006-A-R3-01): every
    /// later image of the anchor carries the run marker `unverifiable`, whatever the
    /// write it belongs to, so that no anchor write of this run can publish `running`
    /// or `clean` again.
    unverifiable: bool,
}

fn io_error(_: io::Error) -> store_error_t {
    store_error_t::io
}

/// Reads one slot file: `Ok(None)` when it does not exist, `Err` for a slot that
/// exists but is unsafe or invalid.
fn read_slot(path: &Path, owner: u32) -> Result<Option<(u64, anchor_map_t)>, store_error_t> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error(error)),
    };
    if !metadata.file_type().is_file() || metadata.uid() != owner || metadata.mode() & 0o077 != 0 {
        return Err(store_error_t::unsafe_path);
    }
    if metadata.len() > MaxSlotBytes {
        return Err(store_error_t::too_large);
    }
    let bytes = fs::read(path).map_err(io_error)?;
    decode_slot(&bytes).map(Some).ok_or(store_error_t::io)
}

impl anchor_t {
    /// Opens the anchor under `directory` (created `0700` if missing, synced in its
    /// parent) for a store whose root is on device `store_device`. Returns the anchor
    /// and what the slots held; with nothing or only invalid slots the map is a fresh
    /// one over `policy`, and the caller decides what that means for the store.
    /// The fresh anchor's counter continues above the highest counter any rejected
    /// slot (invalid, or two valid ones with equal counters) carried, so that the
    /// first write of this run — the run marker at the latest — outranks every
    /// rejected image and none of them can become authoritative at a later start
    /// (M2-006-A-R2-01).
    pub fn open(
        directory: &Path,
        owner: u32,
        store_device: u64,
        policy: policy_state_t,
        io: &dyn journal_io_t,
    ) -> Result<(anchor_t, anchor_start_t), store_error_t> {
        crate::journal::ensure_directory(directory, owner)?;
        let parent = directory.parent().ok_or(store_error_t::unsafe_path)?;
        io.sync_directory(parent).map_err(io_error)?;
        let mode = if fs::metadata(directory).map_err(io_error)?.dev() == store_device {
            anchor_mode_t::degraded
        } else {
            anchor_mode_t::independent
        };
        let mut best: Option<(usize, u64, anchor_map_t)> = None;
        let mut present = 0usize;
        // The highest counter any checksum-valid slot carries, trusted or not: a
        // fresh anchor continues above it, so that no rejected slot can outrank the
        // slots this run writes (M2-006-A-R2-01).
        let mut highest = 0u64;
        // The writer alternates the slots with strictly increasing counters: two valid
        // slots with the same counter never come from it, and neither is trusted.
        let mut equal_counters = false;
        for (index, name) in SlotFileNames.iter().enumerate() {
            match read_slot(&directory.join(name), owner) {
                Ok(None) => {}
                Ok(Some((counter, map))) => {
                    present = present.saturating_add(1);
                    highest = highest.max(counter);
                    if best
                        .as_ref()
                        .is_some_and(|(_, current, _)| counter == *current)
                    {
                        equal_counters = true;
                    }
                    if best
                        .as_ref()
                        .is_none_or(|(_, current, _)| counter > *current)
                    {
                        best = Some((index, counter, map));
                    }
                }
                // An unsafe slot (owner, mode, symbolic link) refuses the store.
                Err(store_error_t::unsafe_path) => return Err(store_error_t::unsafe_path),
                // An invalid slot counts as present but unusable.
                Err(_) => present = present.saturating_add(1),
            }
        }
        if equal_counters {
            best = None;
        }
        let anchor = match best {
            Some((slot, counter, map)) => (
                anchor_t {
                    directory: directory.to_path_buf(),
                    mode,
                    map,
                    counter,
                    current_slot: slot,
                    entry_synced: [false; 2],
                    unverifiable: false,
                },
                anchor_start_t::loaded,
            ),
            None => (
                anchor_t {
                    directory: directory.to_path_buf(),
                    mode,
                    map: anchor_map_t::new(policy),
                    counter: highest,
                    current_slot: 1,
                    entry_synced: [false; 2],
                    unverifiable: false,
                },
                if present == 0 {
                    anchor_start_t::absent
                } else {
                    anchor_start_t::invalid
                },
            ),
        };
        Ok(anchor)
    }

    pub fn mode(&self) -> anchor_mode_t {
        self.mode
    }

    pub fn map(&self) -> &anchor_map_t {
        &self.map
    }

    pub fn counter(&self) -> u64 {
        self.counter
    }

    /// Makes this run unverifiable for the rest of its life: every later write
    /// carries the run marker `unverifiable` (M2-006-A-R3-01). Nothing is written
    /// here; the durable map shows whether a write has published it yet.
    pub fn hold_unverifiable(&mut self) {
        self.unverifiable = true;
    }

    /// Whether the durable anchor already says this run is `unverifiable`.
    pub fn publishes_unverifiable(&self) -> bool {
        self.map
            .run_marker
            .as_ref()
            .is_some_and(|marker| marker.state == run_state_t::unverifiable)
    }

    /// Applies `change` to a copy of the map, writes it to the older slot and makes it
    /// durable — the slot's entry in the directory included, the first time this run
    /// writes that slot; only then does the in-memory map take the change (ADR 0006
    /// §2). A failure leaves the map, the counter and the slots as they were, except
    /// that the older slot may now hold a torn or invalid image, which startup ignores.
    pub fn update(
        &mut self,
        io: &dyn journal_io_t,
        change: impl FnOnce(&mut anchor_map_t),
    ) -> Result<(), store_error_t> {
        let mut next = self.map.clone();
        change(&mut next);
        if self.unverifiable {
            if let Some(marker) = &mut next.run_marker {
                marker.state = run_state_t::unverifiable;
            }
        }
        let counter = self
            .counter
            .checked_add(1)
            .ok_or(store_error_t::too_large)?;
        let bytes = encode_slot(counter, &next).ok_or(store_error_t::too_large)?;
        let slot = 1usize.saturating_sub(self.current_slot);
        let name = SlotFileNames.get(slot).ok_or(store_error_t::io)?;
        let path = self.directory.join(name);
        io.replace_file(&path, &bytes).map_err(io_error)?;
        if let Some(synced) = self.entry_synced.get_mut(slot) {
            if !*synced {
                io.sync_directory(&self.directory).map_err(io_error)?;
                *synced = true;
            }
        }
        self.map = next;
        self.counter = counter;
        self.current_slot = slot;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> anchor_map_t {
        let mut map = anchor_map_t::new(policy_state_t {
            epoch: 3,
            min_valid_epoch: 2,
            emergency_restriction: true,
        });
        map.run_marker = Some(run_marker_t {
            run_id: [7; 16],
            boot_id: "0f3c1e2a-0000-4000-8000-000000000001".to_owned(),
            state: run_state_t::running,
        });
        map.users.insert(
            1001,
            anchored_user_t {
                seq: 12,
                chain: [1; 32],
                user_id: [2; 32],
                state: anchored_state_t::contained,
                identity_seq: 3,
                min_valid_policy_epoch: 1,
            },
        );
        map.users.insert(
            1002,
            anchored_user_t {
                seq: 1,
                chain: [3; 32],
                user_id: [4; 32],
                state: anchored_state_t::normal,
                identity_seq: 1,
                min_valid_policy_epoch: 2,
            },
        );
        map.tombstones.insert(
            [9; 32],
            tombstone_t {
                uid: 1003,
                state: tombstone_state_t::deleted,
            },
        );
        map.time_floor = Some(time_floor_t {
            boot_id: "0f3c1e2a-0000-4000-8000-000000000001".to_owned(),
            boottime: 4_000,
            trusted: 1_791_882_000,
            observed: 9_000,
            anomaly: 3,
        });
        map
    }

    #[test]
    fn the_time_floor_round_trips_and_malformed_floors_are_rejected() {
        let map = sample();
        let bytes = encode_slot(3, &map).expect("encodable");
        assert_eq!(decode_slot(&bytes), Some((3, map.clone())));
        let body = &bytes[..bytes.len() - 32];
        let sealed = |body: &[u8]| {
            let mut sealed = body.to_vec();
            sealed.extend_from_slice(&sha256(body));
            sealed
        };
        // The floor is the last field: flag, boot id length, boot id (36), four u64.
        let floor_at = body.len() - (1 + 1 + 36 + 32);
        let mut flag = body.to_vec();
        flag[floor_at] = 2;
        assert_eq!(decode_slot(&sealed(&flag)), None, "unknown floor flag");
        let mut length = body.to_vec();
        length[floor_at + 1] = 0;
        assert_eq!(decode_slot(&sealed(&length)), None, "empty boot id");
        let mut version = body.to_vec();
        version[4..6].copy_from_slice(&1u16.to_be_bytes());
        assert_eq!(decode_slot(&sealed(&version)), None, "format version 1");
        let mut empty_floor = map;
        empty_floor.time_floor = Some(time_floor_t {
            boot_id: String::new(),
            boottime: 0,
            trusted: 0,
            observed: 0,
            anomaly: 0,
        });
        assert_eq!(
            encode_slot(1, &empty_floor),
            None,
            "a floor needs a boot id"
        );
    }

    #[test]
    fn slots_round_trip_and_reject_every_corruption() {
        let map = sample();
        let bytes = encode_slot(42, &map).expect("encodable");
        assert_eq!(decode_slot(&bytes), Some((42, map.clone())));
        // Any flipped byte fails the checksum; a truncation fails too.
        for position in [0usize, 5, 20, 60, bytes.len() - 40, bytes.len() - 1] {
            let mut corrupt = bytes.clone();
            corrupt[position] ^= 0x01;
            assert_eq!(decode_slot(&corrupt), None, "byte {position}");
        }
        assert_eq!(decode_slot(&bytes[..bytes.len() - 1]), None);
        assert_eq!(decode_slot(&[]), None);
        // A map without a run marker and without users encodes too.
        let empty = anchor_map_t::new(policy_state_t {
            epoch: 1,
            min_valid_epoch: 1,
            emergency_restriction: false,
        });
        let bytes = encode_slot(1, &empty).expect("encodable");
        assert_eq!(decode_slot(&bytes), Some((1, empty)));
    }

    #[test]
    fn the_unverifiable_marker_round_trips_and_malformed_slots_are_rejected() {
        let mut map = sample();
        if let Some(marker) = &mut map.run_marker {
            marker.state = run_state_t::unverifiable;
        }
        let bytes = encode_slot(7, &map).expect("encodable");
        assert_eq!(decode_slot(&bytes), Some((7, map.clone())));
        // Malformed bodies behind a valid checksum are rejected by the parser itself:
        // an unknown marker byte, a boot id longer than the limit, a user count beyond
        // the data and a trailing byte. Offsets: magic 4, version 2, counter 8, marker
        // byte at 14, run id 15..31, boot id length at 31, boot id (36) 32..68, policy
        // 68..85, user count 85..89.
        let body = &bytes[..bytes.len() - 32];
        let sealed = |body: &[u8]| {
            let mut sealed = body.to_vec();
            sealed.extend_from_slice(&sha256(body));
            sealed
        };
        assert_eq!(decode_slot(&sealed(body)), Some((7, map)));
        let mut marker = body.to_vec();
        marker[14] = 4;
        assert_eq!(decode_slot(&sealed(&marker)), None, "unknown marker state");
        let mut boot = body.to_vec();
        boot[31] = 65;
        assert_eq!(decode_slot(&sealed(&boot)), None, "boot id of 65 bytes");
        let mut count = body.to_vec();
        count[85..89].copy_from_slice(&u32::MAX.to_be_bytes());
        assert_eq!(
            decode_slot(&sealed(&count)),
            None,
            "user count beyond the data"
        );
        let mut trailing = body.to_vec();
        trailing.push(0);
        assert_eq!(decode_slot(&sealed(&trailing)), None, "trailing byte");
    }

    #[test]
    fn an_overlong_boot_id_is_not_encodable() {
        let mut map = sample();
        if let Some(marker) = &mut map.run_marker {
            marker.boot_id = "x".repeat(MaxBootIdBytes + 1);
        }
        assert_eq!(encode_slot(1, &map), None);
    }
}
