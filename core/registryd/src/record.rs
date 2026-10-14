//! Journal records and frames (ADR 0006 §1).
//!
//! A user's journal file is a header followed by frames
//! `"C3JR" ‖ u32_be(length) ‖ record ‖ chain`. Each record is one closed
//! `cham3_dcbor_v1` map of at most 65 536 bytes (wire format §2.3 container budget)
//! that embeds at most one signed object. The chain value of the record at 0-based
//! position `n` is
//! `chain_n = domain_hash("cham3/journal-chain/v1", chain_{n−1} ‖ u64_be(n) ‖ record)`
//! with `chain_{−1} = domain_hash("cham3/journal-genesis/v1", user_id ‖ u32_be(uid))`.
//! The record's `seq` field is its 1-based journal sequence `n + 1`.
//!
//! The header (`"C3JH" ‖ u16_be(version) ‖ u32_be(uid) ‖ user_id`, 42 bytes) is this
//! implementation's encoding of ADR 0006 §4's "header + enrollment record": it names
//! the format version and the user the chain is bound to.
//!
//! Record kinds: `identity` (a validated identity object), `commit_admitted`, the
//! narrowing lifecycle records `expired`, `session_ended` and `restriction` (M2-005),
//! and the registry-written user-state records of ADR 0006 §§3, 4, 7 and 10 —
//! `containment`, `quarantine`, `admin_recover`, `recovery_opened`, `recovery_closed`,
//! `deletion_pending` (M2-006; the oracle's `RecordKinds`), and the registry events
//! of M2-006 stage B1: `clock_anomaly` (ADR 0007 §5 rule 2, erratum E6) and
//! `branch_terminated` (§5 rule 6: a new boot ends every live temporary branch).
//! Every one of them only narrows authority. Any other kind fails replay (fail
//! closed).
//!
//! `commit_admitted` carries the observed time at admission (§8 option B); its
//! `time` is the trusted time at admission.

use cham3_contract_format::dcbor::{MaxContainerBytes, decode_container, encode, value_t};
use cham3_contract_format::hash::{digest_t, domain_hash};
use cham3_contract_format::layer_t;
use cham3_contract_format::object_kind_t;
use cham3_contract_format::schema::random_id_t;
use cham3_contract_format::text::is_valid_time;
use cham3_identityd::index::{ceremony_mode_t, record_role_t, user_record_t};

/// Magic of a journal frame.
pub const FrameMagic: [u8; 4] = *b"C3JR";
/// Magic of the journal header.
pub const HeaderMagic: [u8; 4] = *b"C3JH";
/// Journal format version written in the header.
pub const JournalVersion: u16 = 1;
/// Header length: magic, version, uid, user_id.
pub const HeaderBytes: usize = 4 + 2 + 4 + 32;
/// Frame overhead: magic, length and chain value.
pub const FrameOverheadBytes: usize = 4 + 4 + 32;
/// Largest record (wire format §2.3, R08).
pub const MaxRecordBytes: usize = MaxContainerBytes;
/// Domain tags of the chain (ADR 0006 §1).
pub const ChainDomain: &str = "cham3/journal-chain/v1";
pub const GenesisDomain: &str = "cham3/journal-genesis/v1";
/// Most grant ids one restriction record names.
pub const MaxRestrictionGrants: usize = 64;

/// Where an admitted commit was placed (wire format §6.11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum placement_t {
    trunk,
    temporary,
}

impl placement_t {
    pub fn as_str(self) -> &'static str {
        match self {
            placement_t::trunk => "trunk",
            placement_t::temporary => "temporary",
        }
    }

    fn parse(text: &str) -> Option<placement_t> {
        match text {
            "trunk" => Some(placement_t::trunk),
            "temporary" => Some(placement_t::temporary),
            _ => None,
        }
    }
}

/// A narrowing lifecycle record (ADR 0007 §§2–3; wire format §6.12). Each ends the
/// grant ids it names permanently.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum lifecycle_record_t {
    /// A grant observed at or after its expiration.
    expired { grant_id: random_id_t },
    /// Every grant bound to the session.
    session_ended { session_id: random_id_t },
    /// The restrictive path of an explicit revocation (ascending, unique, 1..=64).
    restriction { grant_ids: Vec<random_id_t> },
}

/// A registry event (M2-006 stage B1): evidence the registry writes itself, never
/// through `restrict`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum registry_event_t {
    /// A wall-clock sample more than 300 s behind trusted time, first seen for this
    /// user in this episode (ADR 0007 §5 rule 2, erratum E6). No effect on authority.
    clock_anomaly {
        boot_id: String,
        trusted: u64,
        behind_seconds: u64,
    },
    /// A new boot (`boot_id`) ended every live temporary branch of the user (§5 rule
    /// 6): replay ends them at this point.
    branch_terminated { boot_id: String },
}

/// Longest boot identifier a record names (the anchor's limit).
pub const MaxRecordBootIdBytes: usize = 64;

/// The content of one record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum record_body_t {
    /// A validated identity object persisted through `append_identity`.
    identity {
        kind: object_kind_t,
        object: Vec<u8>,
    },
    /// An admitted ECR commit (pending until enforcement confirms it).
    commit_admitted {
        commit_id: digest_t,
        lineage_id: digest_t,
        placement: placement_t,
        envelope: Vec<u8>,
        /// The observed-time counter at admission (ADR 0007 §8 option B).
        observed: u64,
    },
    lifecycle(lifecycle_record_t),
    event(registry_event_t),
    /// A registry-written user-state record (identityd's `user_record_t`: the same
    /// value both indexes apply).
    user(user_record_t),
}

fn role_text(role: record_role_t) -> &'static str {
    match role {
        record_role_t::trusted_client => "trusted_client",
        record_role_t::admin => "admin",
    }
}

fn parse_role(text: &str) -> Option<record_role_t> {
    match text {
        "trusted_client" => Some(record_role_t::trusted_client),
        "admin" => Some(record_role_t::admin),
        _ => None,
    }
}

fn mode_text(mode: ceremony_mode_t) -> &'static str {
    match mode {
        ceremony_mode_t::c1 => "c1",
        ceremony_mode_t::c2 => "c2",
    }
}

fn parse_mode(text: &str) -> Option<ceremony_mode_t> {
    match text {
        "c1" => Some(ceremony_mode_t::c1),
        "c2" => Some(ceremony_mode_t::c2),
        _ => None,
    }
}

/// One journal record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct registry_record_t {
    /// 1-based journal sequence.
    pub sequence: u64,
    /// Registry time of the append (a claim, never ordering or freshness).
    pub time: String,
    pub body: record_body_t,
}

/// Why a record could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum record_error_t {
    /// Not a canonical record of a known kind with exactly its fields.
    malformed,
    /// Longer than the container budget.
    too_large,
}

fn uint(value: u64) -> value_t {
    value_t::unsigned(value)
}

impl registry_record_t {
    /// The record's canonical encoding.
    pub fn encode(&self) -> Vec<u8> {
        let mut pairs = vec![
            ("seq", uint(self.sequence)),
            ("time", value_t::text_of(&self.time)),
        ];
        match &self.body {
            record_body_t::identity { kind, object } => {
                pairs.push(("kind", value_t::text_of("identity")));
                pairs.push(("object_kind", value_t::text_of(kind.name())));
                pairs.push(("object", value_t::bytes_of(object)));
            }
            record_body_t::commit_admitted {
                commit_id,
                lineage_id,
                placement,
                envelope,
                observed,
            } => {
                pairs.push(("kind", value_t::text_of("commit_admitted")));
                pairs.push(("commit_id", value_t::bytes_of(commit_id)));
                pairs.push(("lineage_id", value_t::bytes_of(lineage_id)));
                pairs.push(("branch", value_t::text_of(placement.as_str())));
                pairs.push(("envelope", value_t::bytes_of(envelope)));
                pairs.push(("observed", uint(*observed)));
            }
            record_body_t::event(registry_event_t::clock_anomaly {
                boot_id,
                trusted,
                behind_seconds,
            }) => {
                pairs.push(("kind", value_t::text_of("clock_anomaly")));
                pairs.push(("boot_id", value_t::text_of(boot_id)));
                pairs.push(("trusted", uint(*trusted)));
                pairs.push(("behind_seconds", uint(*behind_seconds)));
            }
            record_body_t::event(registry_event_t::branch_terminated { boot_id }) => {
                pairs.push(("kind", value_t::text_of("branch_terminated")));
                pairs.push(("boot_id", value_t::text_of(boot_id)));
            }
            record_body_t::lifecycle(lifecycle_record_t::expired { grant_id }) => {
                pairs.push(("kind", value_t::text_of("expired")));
                pairs.push(("grant_id", value_t::bytes_of(grant_id)));
            }
            record_body_t::lifecycle(lifecycle_record_t::session_ended { session_id }) => {
                pairs.push(("kind", value_t::text_of("session_ended")));
                pairs.push(("session_id", value_t::bytes_of(session_id)));
            }
            record_body_t::lifecycle(lifecycle_record_t::restriction { grant_ids }) => {
                pairs.push(("kind", value_t::text_of("restriction")));
                pairs.push((
                    "grant_ids",
                    value_t::array(grant_ids.iter().map(|id| value_t::bytes_of(id)).collect()),
                ));
            }
            record_body_t::user(user_record_t::containment) => {
                pairs.push(("kind", value_t::text_of("containment")));
            }
            record_body_t::user(user_record_t::quarantine) => {
                pairs.push(("kind", value_t::text_of("quarantine")));
            }
            record_body_t::user(user_record_t::admin_recover { role }) => {
                pairs.push(("kind", value_t::text_of("admin_recover")));
                pairs.push(("role", value_t::text_of(role_text(*role))));
            }
            record_body_t::user(user_record_t::recovery_opened {
                recovery_id,
                key_epoch,
                mode,
                role,
            }) => {
                pairs.push(("kind", value_t::text_of("recovery_opened")));
                pairs.push(("recovery_id", value_t::bytes_of(recovery_id)));
                pairs.push(("key_epoch", uint(*key_epoch)));
                pairs.push(("mode", value_t::text_of(mode_text(*mode))));
                pairs.push(("role", value_t::text_of(role_text(*role))));
            }
            record_body_t::user(user_record_t::recovery_closed { recovery_id }) => {
                pairs.push(("kind", value_t::text_of("recovery_closed")));
                pairs.push(("recovery_id", value_t::bytes_of(recovery_id)));
            }
            record_body_t::user(user_record_t::deletion_pending) => {
                pairs.push(("kind", value_t::text_of("deletion_pending")));
            }
        }
        encode(&value_t::text_map(pairs))
    }

    /// Parses a record with the strict decoder: exactly the fields of its kind, each
    /// of the right type and size.
    pub fn decode(bytes: &[u8]) -> Result<registry_record_t, record_error_t> {
        if bytes.len() > MaxRecordBytes {
            return Err(record_error_t::too_large);
        }
        let value = decode_container(bytes, layer_t::payload_decode)
            .map_err(|_| record_error_t::malformed)?;
        let value_t::map(pairs) = &value else {
            return Err(record_error_t::malformed);
        };
        let field = |name: &str| value.get(name).ok_or(record_error_t::malformed);
        let sequence = match field("seq")? {
            value_t::unsigned(sequence) if *sequence >= 1 => *sequence,
            _ => return Err(record_error_t::malformed),
        };
        let time = text(field("time")?)?;
        if !is_valid_time(&time) {
            return Err(record_error_t::malformed);
        }
        let kind = text(field("kind")?)?;
        let (body, names): (record_body_t, &[&str]) = match kind.as_str() {
            "identity" => {
                let kind = object_kind_t::from_name(&text(field("object_kind")?)?)
                    .filter(|kind| *kind != object_kind_t::commit)
                    .ok_or(record_error_t::malformed)?;
                (
                    record_body_t::identity {
                        kind,
                        object: bytes_field(field("object")?)?,
                    },
                    &["object", "object_kind"],
                )
            }
            "commit_admitted" => (
                record_body_t::commit_admitted {
                    commit_id: fixed(field("commit_id")?)?,
                    lineage_id: fixed(field("lineage_id")?)?,
                    placement: placement_t::parse(&text(field("branch")?)?)
                        .ok_or(record_error_t::malformed)?,
                    envelope: bytes_field(field("envelope")?)?,
                    observed: unsigned(field("observed")?)?,
                },
                &["branch", "commit_id", "envelope", "lineage_id", "observed"],
            ),
            "clock_anomaly" => (
                record_body_t::event(registry_event_t::clock_anomaly {
                    boot_id: boot_id(field("boot_id")?)?,
                    trusted: unsigned(field("trusted")?)?,
                    behind_seconds: unsigned(field("behind_seconds")?)?,
                }),
                &["behind_seconds", "boot_id", "trusted"],
            ),
            "branch_terminated" => (
                record_body_t::event(registry_event_t::branch_terminated {
                    boot_id: boot_id(field("boot_id")?)?,
                }),
                &["boot_id"],
            ),
            "expired" => (
                record_body_t::lifecycle(lifecycle_record_t::expired {
                    grant_id: fixed(field("grant_id")?)?,
                }),
                &["grant_id"],
            ),
            "session_ended" => (
                record_body_t::lifecycle(lifecycle_record_t::session_ended {
                    session_id: fixed(field("session_id")?)?,
                }),
                &["session_id"],
            ),
            "restriction" => {
                let value_t::array(items) = field("grant_ids")? else {
                    return Err(record_error_t::malformed);
                };
                let grant_ids = items
                    .iter()
                    .map(fixed::<16>)
                    .collect::<Result<Vec<random_id_t>, record_error_t>>()?;
                let ascending = grant_ids.windows(2).all(|pair| match pair {
                    [first, second] => first < second,
                    _ => false,
                });
                if grant_ids.is_empty() || grant_ids.len() > MaxRestrictionGrants || !ascending {
                    return Err(record_error_t::malformed);
                }
                (
                    record_body_t::lifecycle(lifecycle_record_t::restriction { grant_ids }),
                    &["grant_ids"],
                )
            }
            "containment" => (record_body_t::user(user_record_t::containment), &[]),
            "quarantine" => (record_body_t::user(user_record_t::quarantine), &[]),
            "admin_recover" => (
                record_body_t::user(user_record_t::admin_recover {
                    role: parse_role(&text(field("role")?)?).ok_or(record_error_t::malformed)?,
                }),
                &["role"],
            ),
            "recovery_opened" => (
                record_body_t::user(user_record_t::recovery_opened {
                    recovery_id: fixed(field("recovery_id")?)?,
                    key_epoch: match field("key_epoch")? {
                        value_t::unsigned(epoch) => *epoch,
                        _ => return Err(record_error_t::malformed),
                    },
                    mode: parse_mode(&text(field("mode")?)?).ok_or(record_error_t::malformed)?,
                    role: parse_role(&text(field("role")?)?).ok_or(record_error_t::malformed)?,
                }),
                &["key_epoch", "mode", "recovery_id", "role"],
            ),
            "recovery_closed" => (
                record_body_t::user(user_record_t::recovery_closed {
                    recovery_id: fixed(field("recovery_id")?)?,
                }),
                &["recovery_id"],
            ),
            "deletion_pending" => (record_body_t::user(user_record_t::deletion_pending), &[]),
            _ => return Err(record_error_t::malformed),
        };
        // Closed map: "kind", "seq", "time" and exactly the kind's own fields.
        let expected = names
            .len()
            .checked_add(3)
            .ok_or(record_error_t::malformed)?;
        if pairs.len() != expected {
            return Err(record_error_t::malformed);
        }
        Ok(registry_record_t {
            sequence,
            time,
            body,
        })
    }
}

fn text(value: &value_t) -> Result<String, record_error_t> {
    match value {
        value_t::text(text) => Ok(text.clone()),
        _ => Err(record_error_t::malformed),
    }
}

fn unsigned(value: &value_t) -> Result<u64, record_error_t> {
    match value {
        value_t::unsigned(value) => Ok(*value),
        _ => Err(record_error_t::malformed),
    }
}

fn boot_id(value: &value_t) -> Result<String, record_error_t> {
    let text = text(value)?;
    if text.is_empty() || text.len() > MaxRecordBootIdBytes {
        return Err(record_error_t::malformed);
    }
    Ok(text)
}

fn bytes_field(value: &value_t) -> Result<Vec<u8>, record_error_t> {
    match value {
        value_t::bytes(bytes) => Ok(bytes.clone()),
        _ => Err(record_error_t::malformed),
    }
}

fn fixed<const Count: usize>(value: &value_t) -> Result<[u8; Count], record_error_t> {
    match value {
        value_t::bytes(bytes) => {
            <[u8; Count]>::try_from(bytes.as_slice()).map_err(|_| record_error_t::malformed)
        }
        _ => Err(record_error_t::malformed),
    }
}

/// `chain_{−1}` of a user's journal.
pub fn genesis_chain(user_id: &digest_t, uid: u32) -> digest_t {
    let mut data = Vec::with_capacity(36);
    data.extend_from_slice(user_id);
    data.extend_from_slice(&uid.to_be_bytes());
    domain_hash(GenesisDomain, &data)
}

/// `chain_n` of the record at 0-based position `index`.
pub fn next_chain(previous: &digest_t, index: u64, record: &[u8]) -> digest_t {
    let mut data = Vec::with_capacity(record.len().saturating_add(40));
    data.extend_from_slice(previous);
    data.extend_from_slice(&index.to_be_bytes());
    data.extend_from_slice(record);
    domain_hash(ChainDomain, &data)
}

/// The journal header.
pub fn header(uid: u32, user_id: &digest_t) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(HeaderBytes);
    bytes.extend_from_slice(&HeaderMagic);
    bytes.extend_from_slice(&JournalVersion.to_be_bytes());
    bytes.extend_from_slice(&uid.to_be_bytes());
    bytes.extend_from_slice(user_id);
    bytes
}

/// One frame. `record` is at most [`MaxRecordBytes`] (checked by the caller).
pub fn frame(record: &[u8], chain: &digest_t) -> Option<Vec<u8>> {
    if record.len() > MaxRecordBytes {
        return None;
    }
    let length = u32::try_from(record.len()).ok()?;
    let mut bytes = Vec::with_capacity(record.len().checked_add(FrameOverheadBytes)?);
    bytes.extend_from_slice(&FrameMagic);
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(record);
    bytes.extend_from_slice(chain);
    Some(bytes)
}

/// Why a journal file does not parse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum journal_defect_t {
    /// The header is missing, has the wrong magic or version, or names another uid.
    header,
    /// The file ends inside a frame (a torn tail; recovery is M2-006).
    torn_tail,
    /// A frame has the wrong magic or an oversized length.
    frame,
    /// A chain value does not match.
    chain,
}

/// A journal file split into header fields and verified frames.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct parsed_journal_t {
    pub user_id: digest_t,
    /// Record bytes in journal order; position `n` has sequence `n + 1`.
    pub records: Vec<Vec<u8>>,
    /// Chain value after the last record.
    pub chain: digest_t,
    /// Chain value after each record (position `n` → `chain_n`), for the anchor's
    /// comparison at any sequence (ADR 0006 §4).
    pub chains: Vec<digest_t>,
}

impl parsed_journal_t {
    /// The chain value after `sequence` records (`sequence` 0 is the genesis chain).
    pub fn chain_at(&self, sequence: u64, uid: u32) -> Option<digest_t> {
        if sequence == 0 {
            return Some(genesis_chain(&self.user_id, uid));
        }
        usize::try_from(sequence)
            .ok()
            .and_then(|position| position.checked_sub(1))
            .and_then(|position| self.chains.get(position).copied())
    }
}

/// The complete frames of a journal file and what follows them (ADR 0006 §4: a torn
/// frame after the last complete frame is truncated; any other defect quarantines).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct journal_prefix_t {
    pub parsed: parsed_journal_t,
    /// Length of the header and the complete frames.
    pub valid_length: u64,
    /// Bytes follow the last complete frame without forming one.
    pub torn: bool,
}

fn take<'data>(data: &'data [u8], position: &mut usize, count: usize) -> Option<&'data [u8]> {
    let end = position.checked_add(count)?;
    let chunk = data.get(*position..end)?;
    *position = end;
    Some(chunk)
}

/// Parses a journal file of user `uid` and verifies every chain value. The records
/// themselves are decoded and re-verified by the replay. A torn tail is a defect here;
/// [`parse_journal_prefix`] reports it with the valid prefix instead.
pub fn parse_journal(data: &[u8], uid: u32) -> Result<parsed_journal_t, journal_defect_t> {
    let prefix = parse_journal_prefix(data, uid)?;
    if prefix.torn {
        return Err(journal_defect_t::torn_tail);
    }
    Ok(prefix.parsed)
}

/// Parses the complete frames of a journal file and reports whether a torn frame
/// follows them. Header, frame and chain defects are still refused.
pub fn parse_journal_prefix(data: &[u8], uid: u32) -> Result<journal_prefix_t, journal_defect_t> {
    let mut position = 0usize;
    let header_bytes = take(data, &mut position, HeaderBytes).ok_or(journal_defect_t::header)?;
    let (magic, rest) = header_bytes.split_at(4);
    let (version, rest) = rest.split_at(2);
    let (file_uid, user_id) = rest.split_at(4);
    if magic != HeaderMagic
        || version != JournalVersion.to_be_bytes()
        || file_uid != uid.to_be_bytes()
    {
        return Err(journal_defect_t::header);
    }
    let user_id: digest_t = user_id.try_into().map_err(|_| journal_defect_t::header)?;
    let mut chain = genesis_chain(&user_id, uid);
    let mut records = Vec::new();
    let mut chains = Vec::new();
    let mut index = 0u64;
    let mut valid_length = position;
    let mut torn = false;
    while position < data.len() {
        let Some(head) = take(data, &mut position, 8) else {
            torn = true;
            break;
        };
        let (magic, length) = head.split_at(4);
        if magic != FrameMagic {
            return Err(journal_defect_t::frame);
        }
        let length: [u8; 4] = length.try_into().map_err(|_| journal_defect_t::frame)?;
        let length =
            usize::try_from(u32::from_be_bytes(length)).map_err(|_| journal_defect_t::frame)?;
        if length > MaxRecordBytes {
            return Err(journal_defect_t::frame);
        }
        let (Some(record), Some(stored)) = (
            take(data, &mut position, length),
            take(data, &mut position, 32),
        ) else {
            torn = true;
            break;
        };
        let expected = next_chain(&chain, index, record);
        if stored != expected {
            return Err(journal_defect_t::chain);
        }
        chain = expected;
        records.push(record.to_vec());
        chains.push(chain);
        index = index.checked_add(1).ok_or(journal_defect_t::frame)?;
        valid_length = position;
    }
    Ok(journal_prefix_t {
        parsed: parsed_journal_t {
            user_id,
            records,
            chain,
            chains,
        },
        valid_length: u64::try_from(valid_length).map_err(|_| journal_defect_t::frame)?,
        torn,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(body: record_body_t) -> registry_record_t {
        registry_record_t {
            sequence: 7,
            time: "2026-10-13T08:00:00Z".to_owned(),
            body,
        }
    }

    #[test]
    fn records_round_trip_and_are_closed() {
        let bodies = [
            record_body_t::identity {
                kind: object_kind_t::key_certificate,
                object: vec![1, 2, 3],
            },
            record_body_t::commit_admitted {
                commit_id: [3; 32],
                lineage_id: [4; 32],
                placement: placement_t::temporary,
                envelope: vec![9; 10],
                observed: 4_000,
            },
            record_body_t::event(registry_event_t::clock_anomaly {
                boot_id: "boot".to_owned(),
                trusted: 1_791_882_000,
                behind_seconds: 3_600,
            }),
            record_body_t::event(registry_event_t::branch_terminated {
                boot_id: "boot".to_owned(),
            }),
            record_body_t::lifecycle(lifecycle_record_t::expired { grant_id: [5; 16] }),
            record_body_t::lifecycle(lifecycle_record_t::session_ended {
                session_id: [6; 16],
            }),
            record_body_t::lifecycle(lifecycle_record_t::restriction {
                grant_ids: vec![[1; 16], [2; 16]],
            }),
            record_body_t::user(user_record_t::containment),
            record_body_t::user(user_record_t::quarantine),
            record_body_t::user(user_record_t::admin_recover {
                role: record_role_t::admin,
            }),
            record_body_t::user(user_record_t::recovery_opened {
                recovery_id: [7; 16],
                key_epoch: 3,
                mode: ceremony_mode_t::c1,
                role: record_role_t::trusted_client,
            }),
            record_body_t::user(user_record_t::recovery_closed {
                recovery_id: [7; 16],
            }),
            record_body_t::user(user_record_t::deletion_pending),
        ];
        for body in bodies {
            let record = sample(body);
            let bytes = record.encode();
            assert_eq!(registry_record_t::decode(&bytes), Ok(record.clone()));
            // An extra field, a missing field and a wrong type are refused.
            let value_t::map(mut pairs) =
                decode_container(&bytes, layer_t::payload_decode).expect("record")
            else {
                panic!("record is a map")
            };
            let mut extra = pairs.clone();
            extra.push((value_t::text_of("zz"), value_t::unsigned(1)));
            assert_eq!(
                registry_record_t::decode(&encode(&value_t::map(extra))),
                Err(record_error_t::malformed)
            );
            pairs.retain(|(key, _)| *key != value_t::text_of("time"));
            assert_eq!(
                registry_record_t::decode(&encode(&value_t::map(pairs))),
                Err(record_error_t::malformed)
            );
        }
        let commit_as_identity = sample(record_body_t::identity {
            kind: object_kind_t::commit,
            object: vec![1],
        });
        assert_eq!(
            registry_record_t::decode(&commit_as_identity.encode()),
            Err(record_error_t::malformed)
        );
        let unsorted = sample(record_body_t::lifecycle(lifecycle_record_t::restriction {
            grant_ids: vec![[2; 16], [1; 16]],
        }));
        assert_eq!(
            registry_record_t::decode(&unsorted.encode()),
            Err(record_error_t::malformed)
        );
        let empty = sample(record_body_t::lifecycle(lifecycle_record_t::restriction {
            grant_ids: Vec::new(),
        }));
        assert_eq!(
            registry_record_t::decode(&empty.encode()),
            Err(record_error_t::malformed)
        );
        // A registry event names a boot id of 1..=64 bytes.
        for boot_id in [String::new(), "b".repeat(MaxRecordBootIdBytes + 1)] {
            let event = sample(record_body_t::event(registry_event_t::branch_terminated {
                boot_id,
            }));
            assert_eq!(
                registry_record_t::decode(&event.encode()),
                Err(record_error_t::malformed)
            );
        }
    }

    #[test]
    fn frames_chain_and_detect_defects() {
        let user_id = [7u8; 32];
        let first = sample(record_body_t::lifecycle(lifecycle_record_t::expired {
            grant_id: [1; 16],
        }))
        .encode();
        let second = sample(record_body_t::lifecycle(lifecycle_record_t::expired {
            grant_id: [2; 16],
        }))
        .encode();
        let chain_0 = next_chain(&genesis_chain(&user_id, 1001), 0, &first);
        let chain_1 = next_chain(&chain_0, 1, &second);
        let mut file = header(1001, &user_id);
        file.extend(frame(&first, &chain_0).expect("frame"));
        file.extend(frame(&second, &chain_1).expect("frame"));
        let parsed = parse_journal(&file, 1001).expect("journal");
        assert_eq!(parsed.records, vec![first.clone(), second.clone()]);
        assert_eq!(parsed.chain, chain_1);
        assert_eq!(parse_journal(&file, 1002), Err(journal_defect_t::header));
        // Every truncation inside a frame is a torn tail; inside the header, a header
        // defect.
        for length in HeaderBytes.saturating_add(1)..file.len() {
            let boundary = length
                == HeaderBytes
                    .saturating_add(first.len())
                    .saturating_add(FrameOverheadBytes);
            let result = parse_journal(&file[..length], 1001);
            if boundary {
                assert!(result.is_ok());
            } else {
                assert_eq!(result, Err(journal_defect_t::torn_tail), "length {length}");
            }
        }
        assert_eq!(
            parse_journal(&file[..10], 1001),
            Err(journal_defect_t::header)
        );
        // A flipped record byte breaks the chain; a swapped frame order too.
        let mut flipped = file.clone();
        if let Some(byte) = flipped.get_mut(HeaderBytes + 10) {
            *byte ^= 1;
        }
        assert_eq!(parse_journal(&flipped, 1001), Err(journal_defect_t::chain));
        let mut swapped = header(1001, &user_id);
        swapped.extend(frame(&second, &chain_1).expect("frame"));
        swapped.extend(frame(&first, &chain_0).expect("frame"));
        assert_eq!(parse_journal(&swapped, 1001), Err(journal_defect_t::chain));
        // Another user's header (same uid) does not verify the chain.
        let mut other = header(1001, &[8u8; 32]);
        other.extend(frame(&first, &chain_0).expect("frame"));
        assert_eq!(parse_journal(&other, 1001), Err(journal_defect_t::chain));
        assert!(frame(&vec![0u8; MaxRecordBytes + 1], &chain_0).is_none());
    }
}
