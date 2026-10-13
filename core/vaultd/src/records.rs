//! Per-user vault records: the pending registration bundle and the failed-attempt
//! counter, both `cham3_dcbor_v1` maps with strict decoding.
//!
//! A **pending bundle** makes key creation, rotation, invalidation and recovery
//! restartable: vaultd writes the new encrypted key objects and the signed identity
//! objects that register them durably **before** registering anything (ADR 0006 §10
//! step 3; ADR 0005 §4), and removes the bundle only after identityd (and, for a
//! ceremony, registryd) answered. Every later operation for the user first completes
//! or reconciles the bundle. The bundle holds public signed objects only.

use cham3_contract_format::dcbor::{decode, encode, value_t};
use cham3_contract_format::hash::digest_t;
use cham3_contract_format::schema::random_id_t;
use cham3_contract_format::{layer_t, object_kind_t};

pub const RecordVersion: u64 = 1;

/// What a pending bundle completes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum bundle_operation_t {
    enrollment,
    rotation,
    invalidation,
    recovery,
}

impl bundle_operation_t {
    fn as_str(self) -> &'static str {
        match self {
            bundle_operation_t::enrollment => "enrollment",
            bundle_operation_t::rotation => "rotation",
            bundle_operation_t::invalidation => "invalidation",
            bundle_operation_t::recovery => "recovery",
        }
    }

    fn parse(text: &str) -> Option<bundle_operation_t> {
        [
            bundle_operation_t::enrollment,
            bundle_operation_t::rotation,
            bundle_operation_t::invalidation,
            bundle_operation_t::recovery,
        ]
        .into_iter()
        .find(|operation| operation.as_str() == text)
    }
}

/// Signed identity objects awaiting registration, in registration order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct bundle_t {
    pub operation: bundle_operation_t,
    /// The ceremony a recovery bundle belongs to.
    pub recovery_id: Option<random_id_t>,
    pub objects: Vec<(object_kind_t, Vec<u8>)>,
    /// Key objects this bundle introduces (erased if the bundle can never complete).
    pub new_keys: Vec<digest_t>,
    /// Key objects to erase once the bundle completes (no longer needed for signing).
    pub erase_after: Vec<digest_t>,
}

/// Malformed record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct record_error_t;

fn text(value: &str) -> value_t {
    value_t::text_of(value)
}

fn digests(list: &[digest_t]) -> value_t {
    value_t::array(
        list.iter()
            .map(|digest| value_t::bytes_of(digest))
            .collect(),
    )
}

impl bundle_t {
    pub fn encode(&self) -> Vec<u8> {
        let mut pairs = vec![
            (text("format_version"), value_t::unsigned(RecordVersion)),
            (text("operation"), text(self.operation.as_str())),
            (
                text("objects"),
                value_t::array(
                    self.objects
                        .iter()
                        .map(|(kind, object)| {
                            value_t::array(vec![text(kind.name()), value_t::bytes_of(object)])
                        })
                        .collect(),
                ),
            ),
            (text("new_keys"), digests(&self.new_keys)),
            (text("erase_after"), digests(&self.erase_after)),
        ];
        if let Some(recovery_id) = &self.recovery_id {
            pairs.push((text("recovery_id"), value_t::bytes_of(recovery_id)));
        }
        encode(&value_t::map(pairs))
    }

    pub fn decode(bytes: &[u8]) -> Result<bundle_t, record_error_t> {
        let value = decode(bytes, layer_t::payload_decode, false).map_err(|_| record_error_t)?;
        let value_t::map(pairs) = value else {
            return Err(record_error_t);
        };
        let mut bundle = bundle_t {
            operation: bundle_operation_t::enrollment,
            recovery_id: None,
            objects: Vec::new(),
            new_keys: Vec::new(),
            erase_after: Vec::new(),
        };
        let mut seen = Vec::new();
        for (key, item) in pairs {
            let value_t::text(name) = key else {
                return Err(record_error_t);
            };
            match (name.as_str(), item) {
                ("format_version", value_t::unsigned(RecordVersion)) => {}
                ("operation", value_t::text(operation)) => {
                    bundle.operation =
                        bundle_operation_t::parse(&operation).ok_or(record_error_t)?;
                }
                ("recovery_id", value_t::bytes(id)) => {
                    bundle.recovery_id =
                        Some(<random_id_t>::try_from(id.as_slice()).map_err(|_| record_error_t)?);
                }
                ("objects", value_t::array(items)) => {
                    for item in items {
                        let value_t::array(pair) = item else {
                            return Err(record_error_t);
                        };
                        let [value_t::text(kind), value_t::bytes(object)] =
                            <[value_t; 2]>::try_from(pair).map_err(|_| record_error_t)?
                        else {
                            return Err(record_error_t);
                        };
                        let kind = object_kind_t::from_name(&kind).ok_or(record_error_t)?;
                        bundle.objects.push((kind, object));
                    }
                }
                ("new_keys", value_t::array(items)) => bundle.new_keys = digest_list(items)?,
                ("erase_after", value_t::array(items)) => bundle.erase_after = digest_list(items)?,
                _ => return Err(record_error_t),
            }
            seen.push(name);
        }
        let required = [
            "format_version",
            "operation",
            "objects",
            "new_keys",
            "erase_after",
        ];
        let recovery = bundle.operation == bundle_operation_t::recovery;
        let complete = required
            .iter()
            .all(|name| seen.iter().any(|seen| seen == name))
            && recovery == bundle.recovery_id.is_some();
        if !complete {
            return Err(record_error_t);
        }
        Ok(bundle)
    }
}

fn digest_list(items: Vec<value_t>) -> Result<Vec<digest_t>, record_error_t> {
    items
        .into_iter()
        .map(|item| match item {
            value_t::bytes(bytes) => {
                digest_t::try_from(bytes.as_slice()).map_err(|_| record_error_t)
            }
            _ => Err(record_error_t),
        })
        .collect()
}

/// Consecutive failed passphrase attempts of one UID (ADR 0005 §3). Not
/// rollback-protected: restoring an older file resets the delay (stated limit,
/// ADR 0006 §5); the Argon2id cost and the passphrase remain.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct attempts_t {
    pub failures: u64,
    /// UTC seconds of the last failure.
    pub last_failure: u64,
}

/// Failures allowed before delays apply.
pub const FreeAttempts: u64 = 5;
/// Longest delay, in seconds.
pub const MaxDelaySeconds: u64 = 3600;

impl attempts_t {
    /// Delay before the next attempt: `2^(n−5)` seconds after `n ≥ 5` consecutive
    /// failures, at most one hour.
    pub fn delay_seconds(&self) -> u64 {
        match self.failures.checked_sub(FreeAttempts) {
            None => 0,
            Some(exponent) if exponent >= 12 => MaxDelaySeconds,
            Some(exponent) => 1u64
                .checked_shl(u32::try_from(exponent).unwrap_or(u32::MAX))
                .unwrap_or(MaxDelaySeconds)
                .min(MaxDelaySeconds),
        }
    }

    /// True while the delay since the last failure has not elapsed.
    pub fn locked_at(&self, now: u64) -> bool {
        let delay = self.delay_seconds();
        delay > 0 && now < self.last_failure.saturating_add(delay)
    }

    pub fn encode(&self) -> Vec<u8> {
        encode(&value_t::map(vec![
            (text("format_version"), value_t::unsigned(RecordVersion)),
            (text("failures"), value_t::unsigned(self.failures)),
            (text("last_failure"), value_t::unsigned(self.last_failure)),
        ]))
    }

    pub fn decode(bytes: &[u8]) -> Result<attempts_t, record_error_t> {
        let value = decode(bytes, layer_t::payload_decode, false).map_err(|_| record_error_t)?;
        let value_t::map(pairs) = value else {
            return Err(record_error_t);
        };
        let mut attempts = attempts_t::default();
        let mut fields = 0u8;
        for (key, item) in pairs {
            match (key, item) {
                (value_t::text(name), value_t::unsigned(RecordVersion))
                    if name == "format_version" => {}
                (value_t::text(name), value_t::unsigned(failures)) if name == "failures" => {
                    attempts.failures = failures;
                }
                (value_t::text(name), value_t::unsigned(time)) if name == "last_failure" => {
                    attempts.last_failure = time;
                }
                _ => return Err(record_error_t),
            }
            fields = fields.saturating_add(1);
        }
        if fields != 3 {
            return Err(record_error_t);
        }
        Ok(attempts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delays_double_after_five_failures_up_to_an_hour() {
        let delay = |failures| {
            attempts_t {
                failures,
                last_failure: 0,
            }
            .delay_seconds()
        };
        assert_eq!(
            [delay(0), delay(4), delay(5), delay(6), delay(7)],
            [0, 0, 1, 2, 4]
        );
        assert_eq!(delay(16), 2048);
        assert_eq!(delay(17), 3600);
        assert_eq!(delay(u64::MAX), 3600);
        let locked = attempts_t {
            failures: 7,
            last_failure: 100,
        };
        assert!(locked.locked_at(103));
        assert!(!locked.locked_at(104));
        assert!(
            !attempts_t {
                failures: 4,
                last_failure: 100
            }
            .locked_at(100)
        );
    }

    #[test]
    fn records_round_trip_and_reject_unknown_fields() {
        let attempts = attempts_t {
            failures: 9,
            last_failure: 1_800_000_000,
        };
        assert_eq!(attempts_t::decode(&attempts.encode()), Ok(attempts));
        let bundle = bundle_t {
            operation: bundle_operation_t::recovery,
            recovery_id: Some([4; 16]),
            objects: vec![(object_kind_t::key_certificate, vec![1, 2, 3])],
            new_keys: vec![[5; 32]],
            erase_after: vec![],
        };
        assert_eq!(bundle_t::decode(&bundle.encode()), Ok(bundle.clone()));
        let unbound = bundle_t {
            recovery_id: None,
            ..bundle.clone()
        };
        assert_eq!(bundle_t::decode(&unbound.encode()), Err(record_error_t));
        let extra = encode(&value_t::map(vec![
            (text("format_version"), value_t::unsigned(1)),
            (text("failures"), value_t::unsigned(1)),
            (text("last_failure"), value_t::unsigned(1)),
            (text("other"), value_t::unsigned(1)),
        ]));
        assert_eq!(attempts_t::decode(&extra), Err(record_error_t));
    }
}
