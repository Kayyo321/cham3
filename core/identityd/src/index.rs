//! Per-user key-authority index (verification layer 6; ADR 0005 §2, ADR 0006 §§5, 10).
//!
//! The index is a pure state machine over one user's journal records, in journal
//! order: the signed identity objects (enrollment, certificates, invalidations) and
//! the narrowing records registryd writes itself (containment, quarantine, admin
//! recovery, ceremony open/close, deletion) plus the ceremony's key-naming record.
//! It holds no store (R05): identityd rebuilds it from registryd's journal, so its
//! freshness is the journal's rollback protection.
//!
//! The decisions mirror the independent oracle's journal model
//! (tests/m2/oracle/ecr_oracle.py `journal_t`) and are checked against every
//! key-authority journal scenario of the published vectors. Rules the model does not
//! state, and that this index adds, are marked "(beyond the model)".

use std::collections::{BTreeMap, BTreeSet};

use cham3_contract_format::envelope::public_key_is_valid;
use cham3_contract_format::hash::digest_t;
use cham3_contract_format::schema::{
    authority_class_t, commit_payload_t, commit_type_t, identity_enrollment_t,
    invalidation_reason_t, key_certificate_t, key_invalidation_t, random_id_t,
};

use crate::outcome::{
    authority, authority_code_t, ipc, ipc_code_t, rule, rule_code_t, service_result_t,
};

/// Longest validity window of a temporary-authority certificate (ADR 0005 §2).
pub const MaxTemporaryCertificateSeconds: i64 = 7 * 24 * 60 * 60;

/// Authority class of a registered key. The identity key certifies and invalidates
/// keys and never signs commits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum key_class_t {
    identity,
    persistent,
    temporary,
}

impl key_class_t {
    fn of_certificate(class: authority_class_t) -> key_class_t {
        match class {
            authority_class_t::persistent => key_class_t::persistent,
            authority_class_t::temporary => key_class_t::temporary,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            key_class_t::identity => "identity",
            key_class_t::persistent => "persistent",
            key_class_t::temporary => "temporary",
        }
    }
}

/// One registered key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct key_record_t {
    pub class: key_class_t,
    pub public_key: [u8; 32],
    pub key_epoch: u64,
    pub not_before: String,
    pub not_after: Option<String>,
    /// The strongest invalidation reason registered (`compromised` dominates).
    pub invalidated: Option<invalidation_reason_t>,
    /// Retired by a ceremony's key-naming record.
    pub retired: bool,
}

/// User state as far as key authority is concerned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum user_state_t {
    normal,
    contained,
    quarantined,
    deleted,
}

impl user_state_t {
    pub fn as_str(self) -> &'static str {
        match self {
            user_state_t::normal => "normal",
            user_state_t::contained => "contained",
            user_state_t::quarantined => "quarantined",
            user_state_t::deleted => "deleted",
        }
    }
}

/// Who may open a recovery ceremony (owner choice ADR 0006 §9.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ceremony_mode_t {
    c1,
    c2,
}

/// Role recorded with a ceremony or admin record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum record_role_t {
    trusted_client,
    admin,
}

/// The open recovery ceremony.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ceremony_t {
    pub recovery_id: random_id_t,
    pub key_epoch: u64,
    /// (persistent, temporary) keys named by the ceremony's `recovery_keys` record.
    pub named: Option<(digest_t, digest_t)>,
}

/// Registry-written records that change user state for key authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum user_record_t {
    containment,
    quarantine,
    admin_recover {
        role: record_role_t,
    },
    recovery_opened {
        recovery_id: random_id_t,
        key_epoch: u64,
        mode: ceremony_mode_t,
        role: record_role_t,
    },
    recovery_closed {
        recovery_id: random_id_t,
    },
    deletion_pending,
}

/// A ceremony's key-naming request (`name_recovery_keys`; ADR 0006 §10 step 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct naming_t {
    pub recovery_id: random_id_t,
    pub persistent_key_id: digest_t,
    pub temporary_key_id: digest_t,
}

/// What `name_recovery_keys` would do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum naming_effect_t {
    /// A new `recovery_keys` record.
    name,
    /// The identical naming is already effective: `already_named`, no record.
    already_named,
}

/// Answer of `recovery_status` for one ceremony.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct recovery_status_t {
    pub user_state: user_state_t,
    pub ceremony_open: bool,
    pub named: Option<(digest_t, digest_t)>,
}

/// Scope a resolved key may sign in (service IPC §3 `resolve_key`, R13).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum key_scope_t {
    normal,
    /// Contained user. For the identity key: invalidations, and the certificates of
    /// the ceremony `recovery_id` while it is open and has named no keys. For the
    /// ceremony's named persistent key: trunk `restore`/`revocation` commits whose
    /// `authorization.session_id` is `recovery_id`.
    recovery {
        recovery_id: Option<random_id_t>,
    },
    denied(authority_code_t),
}

/// The answer to `resolve_key`. A caller enforces `scope`; the other fields are what
/// layer 6 checks against an object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct key_resolution_t {
    pub key_id: digest_t,
    pub public_key: [u8; 32],
    pub owner_uid: u32,
    pub owner_user_id: digest_t,
    pub class: key_class_t,
    pub key_epoch: u64,
    pub min_key_epoch: u64,
    pub not_before: String,
    pub not_after: Option<String>,
    pub invalidated: Option<invalidation_reason_t>,
    pub retired: bool,
    pub user_state: user_state_t,
    /// Journal sequence of the snapshot answered from.
    pub journal_seq: u64,
    pub scope: key_scope_t,
}

/// Key-authority state of one enrolled user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct user_index_t {
    uid: u32,
    user_id: digest_t,
    identity_epoch: u64,
    keys: BTreeMap<digest_t, key_record_t>,
    min_key_epoch: u64,
    state: user_state_t,
    ceremony: Option<ceremony_t>,
    ceremony_epochs: BTreeSet<u64>,
    named_keys: BTreeMap<random_id_t, (digest_t, digest_t)>,
    objects: BTreeMap<digest_t, u64>,
    journal_seq: u64,
}

impl user_index_t {
    /// A new user from a verified enrollment (journal record `sequence`).
    pub fn from_enrollment(
        enrollment: &identity_enrollment_t,
        object_id: digest_t,
        sequence: u64,
    ) -> user_index_t {
        let identity = key_record_t {
            class: key_class_t::identity,
            public_key: enrollment.public_key,
            key_epoch: 0,
            not_before: enrollment.enrolled.clone(),
            not_after: None,
            invalidated: None,
            retired: false,
        };
        user_index_t {
            uid: enrollment.owner_uid,
            user_id: enrollment.user_id,
            identity_epoch: enrollment.identity_epoch,
            keys: BTreeMap::from([(enrollment.user_id, identity)]),
            min_key_epoch: 0,
            state: user_state_t::normal,
            ceremony: None,
            ceremony_epochs: BTreeSet::new(),
            named_keys: BTreeMap::new(),
            objects: BTreeMap::from([(object_id, sequence)]),
            journal_seq: sequence,
        }
    }

    pub fn uid(&self) -> u32 {
        self.uid
    }

    pub fn user_id(&self) -> digest_t {
        self.user_id
    }

    pub fn identity_epoch(&self) -> u64 {
        self.identity_epoch
    }

    pub fn state(&self) -> user_state_t {
        self.state
    }

    pub fn min_key_epoch(&self) -> u64 {
        self.min_key_epoch
    }

    pub fn ceremony(&self) -> Option<&ceremony_t> {
        self.ceremony.as_ref()
    }

    pub fn journal_seq(&self) -> u64 {
        self.journal_seq
    }

    pub fn key(&self, key_id: &digest_t) -> Option<&key_record_t> {
        self.keys.get(key_id)
    }

    pub fn keys(&self) -> impl Iterator<Item = (&digest_t, &key_record_t)> {
        self.keys.iter()
    }

    /// Journal sequence of an identity object already applied (idempotency).
    pub fn object_sequence(&self, object_id: &digest_t) -> Option<u64> {
        self.objects.get(object_id).copied()
    }

    /// Records the journal position of a record that changes nothing here (a commit
    /// or a grant lifecycle record in the same journal).
    pub fn observe_sequence(&mut self, sequence: u64) {
        self.journal_seq = self.journal_seq.max(sequence);
    }

    /// Observation used by the published scenarios (I04): whether the open ceremony
    /// has named its keys, the minimum signing key epoch and the retired key ids.
    pub fn key_observation(&self) -> (bool, u64, Vec<digest_t>) {
        let named = self
            .ceremony
            .as_ref()
            .is_some_and(|ceremony| ceremony.named.is_some());
        let retired = self
            .keys
            .iter()
            .filter(|(_, key)| key.retired)
            .map(|(key_id, _)| *key_id)
            .collect();
        (named, self.min_key_epoch, retired)
    }

    // ---------------------------------------------------------------- identity objects

    /// Checks a verified certificate signed by this user's identity key.
    pub fn check_certificate(&self, certificate: &key_certificate_t) -> service_result_t<()> {
        let not_certified = authority(authority_code_t::key_not_certified);
        if certificate.owner_user_id != self.user_id {
            return Err(not_certified);
        }
        match self.state {
            user_state_t::deleted => return Err(authority(authority_code_t::user_deleted)),
            user_state_t::quarantined => return Err(authority(authority_code_t::quarantined)),
            user_state_t::normal | user_state_t::contained => {}
        }
        if certificate.owner_uid != self.uid || self.keys.contains_key(&certificate.subject_key_id)
        {
            return Err(not_certified);
        }
        // M2-004 obligation (review M2-003): a certified key must be a canonical,
        // non-small-order point (wire format §4.3) (beyond the model).
        if !public_key_is_valid(&certificate.subject_public_key) {
            return Err(not_certified);
        }
        let class = key_class_t::of_certificate(certificate.authority_class);
        // A rotation names an earlier key of this user in the same class (beyond the
        // model).
        if let Some(superseded) = &certificate.supersedes_key_id {
            match self.keys.get(superseded) {
                Some(key) if key.class == class => {}
                _ => return Err(not_certified),
            }
        }
        check_certificate_window(certificate, class)?;
        match (self.state, &self.ceremony) {
            (user_state_t::contained, None) => Err(authority(authority_code_t::contained)),
            (user_state_t::contained, Some(ceremony)) => {
                if certificate.key_epoch != ceremony.key_epoch || ceremony.named.is_some() {
                    Err(rule(rule_code_t::recovery_rule))
                } else {
                    Ok(())
                }
            }
            _ if certificate.key_epoch < self.min_key_epoch.max(1) => {
                Err(authority(authority_code_t::key_epoch_stale))
            }
            _ => Ok(()),
        }
    }

    pub fn apply_certificate(
        &mut self,
        certificate: &key_certificate_t,
        object_id: digest_t,
        sequence: u64,
    ) {
        self.keys.insert(
            certificate.subject_key_id,
            key_record_t {
                class: key_class_t::of_certificate(certificate.authority_class),
                public_key: certificate.subject_public_key,
                key_epoch: certificate.key_epoch,
                not_before: certificate.not_before.clone(),
                not_after: certificate.not_after.clone(),
                invalidated: None,
                retired: false,
            },
        );
        self.record_object(object_id, sequence);
    }

    /// Checks a verified invalidation signed by this user's identity key. Invalidation
    /// only narrows, so it is accepted in every state except deleted (service IPC §3).
    pub fn check_invalidation(&self, invalidation: &key_invalidation_t) -> service_result_t<()> {
        let not_certified = authority(authority_code_t::key_not_certified);
        if invalidation.owner_user_id != self.user_id {
            return Err(not_certified);
        }
        if self.state == user_state_t::deleted {
            return Err(authority(authority_code_t::user_deleted));
        }
        match self.keys.get(&invalidation.target_key_id) {
            Some(key) if key.class != key_class_t::identity => Ok(()),
            _ => Err(not_certified),
        }
    }

    pub fn apply_invalidation(
        &mut self,
        invalidation: &key_invalidation_t,
        object_id: digest_t,
        sequence: u64,
    ) {
        if let Some(key) = self.keys.get_mut(&invalidation.target_key_id) {
            // A later `compromised` escalates an earlier reason (beyond the model): it
            // makes every commit the key signed ineligible (ADR 0007).
            key.invalidated = match (key.invalidated, invalidation.reason) {
                (_, invalidation_reason_t::compromised) => Some(invalidation_reason_t::compromised),
                (Some(earlier), _) => Some(earlier),
                (None, reason) => Some(reason),
            };
        }
        self.record_object(object_id, sequence);
    }

    fn record_object(&mut self, object_id: digest_t, sequence: u64) {
        self.objects.insert(object_id, sequence);
        self.observe_sequence(sequence);
    }

    // ---------------------------------------------------------------- registry records

    /// Checks a registry-written record against the user's state.
    pub fn check_record(&self, record: &user_record_t) -> service_result_t<()> {
        let recovery_rule = rule(rule_code_t::recovery_rule);
        match record {
            user_record_t::containment => match self.state {
                user_state_t::deleted | user_state_t::quarantined => Err(recovery_rule),
                _ => Ok(()),
            },
            user_record_t::quarantine => match self.state {
                user_state_t::deleted => Err(recovery_rule),
                _ => Ok(()),
            },
            user_record_t::admin_recover { role } => {
                if self.state != user_state_t::quarantined || *role != record_role_t::admin {
                    Err(recovery_rule)
                } else {
                    Ok(())
                }
            }
            user_record_t::recovery_opened {
                key_epoch,
                mode,
                role,
                ..
            } => {
                if self.state != user_state_t::contained || self.ceremony.is_some() {
                    return Err(recovery_rule);
                }
                if *mode == ceremony_mode_t::c2 && *role != record_role_t::admin {
                    return Err(recovery_rule);
                }
                let highest = self
                    .keys
                    .values()
                    .map(|key| key.key_epoch)
                    .chain(self.ceremony_epochs.iter().copied())
                    .max()
                    .unwrap_or(0);
                if *key_epoch <= highest {
                    return Err(recovery_rule);
                }
                Ok(())
            }
            user_record_t::recovery_closed { recovery_id } => match &self.ceremony {
                Some(ceremony)
                    if self.state == user_state_t::contained
                        && ceremony.recovery_id == *recovery_id
                        && ceremony.named.is_some() =>
                {
                    Ok(())
                }
                _ => Err(recovery_rule),
            },
            user_record_t::deletion_pending => Ok(()),
        }
    }

    pub fn apply_record(&mut self, record: &user_record_t, sequence: u64) {
        match record {
            user_record_t::containment => {
                self.state = user_state_t::contained;
                self.ceremony = None;
            }
            user_record_t::quarantine => {
                self.state = user_state_t::quarantined;
                self.ceremony = None;
            }
            user_record_t::admin_recover { .. } => self.state = user_state_t::contained,
            user_record_t::recovery_opened {
                recovery_id,
                key_epoch,
                ..
            } => {
                self.ceremony = Some(ceremony_t {
                    recovery_id: *recovery_id,
                    key_epoch: *key_epoch,
                    named: None,
                });
                self.ceremony_epochs.insert(*key_epoch);
            }
            user_record_t::recovery_closed { .. } => {
                self.ceremony = None;
                self.state = user_state_t::normal;
            }
            user_record_t::deletion_pending => {
                self.state = user_state_t::deleted;
                self.ceremony = None;
            }
        }
        self.observe_sequence(sequence);
    }

    /// Validates `name_recovery_keys` (ADR 0006 §10 step 3; service IPC §3).
    pub fn check_naming(
        &self,
        owner_uid: u32,
        naming: &naming_t,
    ) -> service_result_t<naming_effect_t> {
        let recovery_rule = rule(rule_code_t::recovery_rule);
        if owner_uid != self.uid {
            return Err(recovery_rule);
        }
        let requested = (naming.persistent_key_id, naming.temporary_key_id);
        if let Some(named) = self.named_keys.get(&naming.recovery_id) {
            let open_here = self
                .ceremony
                .as_ref()
                .is_some_and(|ceremony| ceremony.recovery_id == naming.recovery_id);
            return if *named == requested && open_here {
                Ok(naming_effect_t::already_named)
            } else {
                Err(recovery_rule)
            };
        }
        let ceremony = match &self.ceremony {
            Some(ceremony)
                if self.state == user_state_t::contained
                    && ceremony.recovery_id == naming.recovery_id
                    && ceremony.named.is_none() =>
            {
                ceremony
            }
            _ => return Err(recovery_rule),
        };
        for (key_id, class) in [
            (naming.persistent_key_id, key_class_t::persistent),
            (naming.temporary_key_id, key_class_t::temporary),
        ] {
            match self.keys.get(&key_id) {
                Some(key)
                    if key.class == class
                        && key.key_epoch == ceremony.key_epoch
                        && key.invalidated.is_none() => {}
                _ => return Err(recovery_rule),
            }
        }
        Ok(naming_effect_t::name)
    }

    /// Applies an anchored `recovery_keys` record: retires every other contract key
    /// (including epoch-E keys of an abandoned attempt) and raises the minimum signing
    /// key epoch to the ceremony's.
    pub fn apply_naming(&mut self, naming: &naming_t, sequence: u64) {
        let named = (naming.persistent_key_id, naming.temporary_key_id);
        let Some(ceremony) = self.ceremony.as_mut() else {
            return;
        };
        for (key_id, key) in &mut self.keys {
            if key.class != key_class_t::identity && *key_id != named.0 && *key_id != named.1 {
                key.retired = true;
            }
        }
        self.min_key_epoch = ceremony.key_epoch;
        ceremony.named = Some(named);
        self.named_keys.insert(naming.recovery_id, named);
        self.observe_sequence(sequence);
    }

    pub fn recovery_status(&self, recovery_id: &random_id_t) -> recovery_status_t {
        recovery_status_t {
            user_state: self.state,
            ceremony_open: self
                .ceremony
                .as_ref()
                .is_some_and(|ceremony| ceremony.recovery_id == *recovery_id),
            named: self.named_keys.get(recovery_id).copied(),
        }
    }

    // ---------------------------------------------------------------- layer 6

    /// Resolves a key of this user with its signing scope.
    pub fn resolve(&self, key_id: &digest_t) -> Option<key_resolution_t> {
        let key = self.keys.get(key_id)?;
        Some(key_resolution_t {
            key_id: *key_id,
            public_key: key.public_key,
            owner_uid: self.uid,
            owner_user_id: self.user_id,
            class: key.class,
            key_epoch: key.key_epoch,
            min_key_epoch: self.min_key_epoch,
            not_before: key.not_before.clone(),
            not_after: key.not_after.clone(),
            invalidated: key.invalidated,
            retired: key.retired,
            user_state: self.state,
            journal_seq: self.journal_seq,
            scope: self.scope_of(key_id, key),
        })
    }

    fn scope_of(&self, key_id: &digest_t, key: &key_record_t) -> key_scope_t {
        // Key validity before user state (wire format §7).
        if key.class != key_class_t::identity {
            if key.invalidated.is_some() {
                return key_scope_t::denied(authority_code_t::key_invalidated);
            }
            if key.key_epoch < self.min_key_epoch {
                return key_scope_t::denied(authority_code_t::key_epoch_stale);
            }
            if key.retired {
                return key_scope_t::denied(authority_code_t::key_retired);
            }
        }
        match self.state {
            user_state_t::normal => key_scope_t::normal,
            user_state_t::deleted => key_scope_t::denied(authority_code_t::user_deleted),
            user_state_t::quarantined => key_scope_t::denied(authority_code_t::quarantined),
            user_state_t::contained => match (&self.ceremony, key.class) {
                (ceremony, key_class_t::identity) => key_scope_t::recovery {
                    recovery_id: ceremony
                        .as_ref()
                        .filter(|ceremony| ceremony.named.is_none())
                        .map(|ceremony| ceremony.recovery_id),
                },
                (None, _) => key_scope_t::denied(authority_code_t::contained),
                (Some(ceremony), _) => match ceremony.named {
                    Some((persistent, _)) if persistent == *key_id => key_scope_t::recovery {
                        recovery_id: Some(ceremony.recovery_id),
                    },
                    _ => key_scope_t::denied(authority_code_t::recovery_key_required),
                },
            },
        }
    }
}

/// Layer 6 for a commit that passed layers 1–5, in the check order of the oracle
/// model: actor binding, key validity, authority class, certificate window, then user
/// state (wire format §7: key validity before user state).
pub fn check_commit_authority(
    resolution: &key_resolution_t,
    payload: &commit_payload_t,
) -> service_result_t<()> {
    if payload.key_id != resolution.key_id
        || payload.actor.user_id != resolution.owner_user_id
        || payload.actor.uid != resolution.owner_uid
    {
        return Err(authority(authority_code_t::key_not_certified));
    }
    if resolution.invalidated.is_some() {
        return Err(authority(authority_code_t::key_invalidated));
    }
    if resolution.key_epoch < resolution.min_key_epoch {
        return Err(authority(authority_code_t::key_epoch_stale));
    }
    if resolution.retired {
        return Err(authority(authority_code_t::key_retired));
    }
    let class_denied = authority(authority_code_t::authority_class_denied);
    if resolution.class == key_class_t::identity
        || key_class_t::of_certificate(payload.authority_class) != resolution.class
    {
        return Err(class_denied);
    }
    if resolution.class == key_class_t::temporary
        && payload.commit_type != commit_type_t::temporary_grant
    {
        return Err(class_denied);
    }
    // utc_time is fixed-width (wire format §3.4), so text order is time order.
    let before_window = payload.timestamp.as_str() < resolution.not_before.as_str();
    let after_window = resolution
        .not_after
        .as_ref()
        .is_some_and(|not_after| payload.timestamp.as_str() > not_after.as_str());
    if before_window || after_window {
        return Err(authority(authority_code_t::certificate_window));
    }
    match resolution.scope {
        key_scope_t::denied(code) => Err(authority(code)),
        key_scope_t::normal | key_scope_t::recovery { .. } => Ok(()),
    }
}

/// The scope restriction a signer applies on top of layer 6: while contained, the
/// named recovery key signs only `restore`/`revocation` commits bound to the open
/// ceremony (ADR 0006 §10; trunk placement is checked at admission).
pub fn check_commit_scope(
    resolution: &key_resolution_t,
    payload: &commit_payload_t,
) -> service_result_t<()> {
    check_commit_authority(resolution, payload)?;
    match resolution.scope {
        key_scope_t::recovery { recovery_id } => {
            let bound = recovery_id == Some(payload.authorization.session_id);
            let recovery_type = matches!(
                payload.commit_type,
                commit_type_t::restore | commit_type_t::revocation
            );
            if bound && recovery_type {
                Ok(())
            } else {
                Err(rule(rule_code_t::recovery_rule))
            }
        }
        _ => Ok(()),
    }
}

/// Validity window rules of a certificate (beyond the model): `not_after`, when
/// present, follows `not_before`; temporary authority requires `not_after` at most
/// seven days after `not_before` (ADR 0005 §2).
fn check_certificate_window(
    certificate: &key_certificate_t,
    class: key_class_t,
) -> service_result_t<()> {
    let window = authority(authority_code_t::certificate_window);
    let start = utc_seconds(&certificate.not_before).ok_or(window)?;
    match (&certificate.not_after, class) {
        (None, key_class_t::temporary) => Err(window),
        (None, _) => Ok(()),
        (Some(not_after), _) => {
            let end = utc_seconds(not_after).ok_or(window)?;
            let length = end.checked_sub(start).ok_or(window)?;
            if length <= 0
                || (class == key_class_t::temporary && length > MaxTemporaryCertificateSeconds)
            {
                Err(window)
            } else {
                Ok(())
            }
        }
    }
}

/// Seconds since 1970-01-01T00:00:00Z of a validated `utc_time`
/// (`YYYY-MM-DDTHH:MM:SSZ`), or `None` if the text is not one.
pub fn utc_seconds(text: &str) -> Option<i64> {
    if !cham3_contract_format::text::is_valid_time(text) {
        return None;
    }
    let field = |range: std::ops::Range<usize>| -> Option<i64> { text.get(range)?.parse().ok() };
    let (year, month, day) = (field(0..4)?, field(5..7)?, field(8..10)?);
    let (hour, minute, second) = (field(11..13)?, field(14..16)?, field(17..19)?);
    // Days from civil (proleptic Gregorian), H. Hinnant's algorithm.
    let shifted_year = if month <= 2 {
        year.checked_sub(1)?
    } else {
        year
    };
    let era = shifted_year.checked_div(400)?;
    let year_of_era = shifted_year.checked_sub(era.checked_mul(400)?)?;
    let shifted_month = if month > 2 {
        month.checked_sub(3)?
    } else {
        month.checked_add(9)?
    };
    let day_of_year = shifted_month
        .checked_mul(153)?
        .checked_add(2)?
        .checked_div(5)?
        .checked_add(day)?
        .checked_sub(1)?;
    let day_of_era = year_of_era
        .checked_mul(365)?
        .checked_add(year_of_era.checked_div(4)?)?
        .checked_sub(year_of_era.checked_div(100)?)?
        .checked_add(day_of_year)?;
    let days = era
        .checked_mul(146_097)?
        .checked_add(day_of_era)?
        .checked_sub(719_468)?;
    days.checked_mul(86_400)?
        .checked_add(hour.checked_mul(3600)?)?
        .checked_add(minute.checked_mul(60)?)?
        .checked_add(second)
}

/// The `utc_time` text of `seconds` since 1970-01-01T00:00:00Z, or `None` outside
/// the years 2000–9999.
pub fn utc_text(seconds: i64) -> Option<String> {
    let days = seconds.checked_div_euclid(86_400)?;
    let second_of_day = seconds.checked_rem_euclid(86_400)?;
    // Civil from days (proleptic Gregorian), H. Hinnant's algorithm.
    let shifted = days.checked_add(719_468)?;
    let era = shifted.checked_div_euclid(146_097)?;
    let day_of_era = shifted.checked_sub(era.checked_mul(146_097)?)?;
    let year_of_era = day_of_era
        .checked_sub(day_of_era.checked_div(1460)?)?
        .checked_add(day_of_era.checked_div(36_524)?)?
        .checked_sub(day_of_era.checked_div(146_096)?)?
        .checked_div(365)?;
    let day_of_year = day_of_era.checked_sub(
        year_of_era
            .checked_mul(365)?
            .checked_add(year_of_era.checked_div(4)?)?
            .checked_sub(year_of_era.checked_div(100)?)?,
    )?;
    let shifted_month = day_of_year
        .checked_mul(5)?
        .checked_add(2)?
        .checked_div(153)?;
    let day = day_of_year
        .checked_sub(
            shifted_month
                .checked_mul(153)?
                .checked_add(2)?
                .checked_div(5)?,
        )?
        .checked_add(1)?;
    let month = if shifted_month < 10 {
        shifted_month.checked_add(3)?
    } else {
        shifted_month.checked_sub(9)?
    };
    let year = year_of_era
        .checked_add(era.checked_mul(400)?)?
        .checked_add(if month <= 2 { 1 } else { 0 })?;
    let text = format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        second_of_day.checked_div(3600)?,
        second_of_day.checked_rem(3600)?.checked_div(60)?,
        second_of_day.checked_rem(60)?
    );
    utc_seconds(&text)
        .filter(|parsed| *parsed == seconds)
        .map(|_| text)
}

/// Refusal used when a user's journal cannot be read or applied (fail closed).
pub fn journal_unavailable() -> crate::outcome::service_error_t {
    ipc(ipc_code_t::storage_unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_seconds_matches_known_instants() {
        assert_eq!(
            utc_seconds("1970-01-01T00:00:00Z"),
            None,
            "years before 2000 are invalid"
        );
        assert_eq!(utc_seconds("2000-01-01T00:00:00Z"), Some(946_684_800));
        assert_eq!(utc_seconds("2026-10-12T12:00:00Z"), Some(1_791_806_400));
        assert_eq!(utc_seconds("2024-02-29T23:59:59Z"), Some(1_709_251_199));
        assert_eq!(utc_seconds("2024-03-01T00:00:00Z"), Some(1_709_251_200));
        assert_eq!(utc_seconds("2026-02-29T00:00:00Z"), None);
        assert_eq!(utc_seconds("2026-10-12T12:00:00"), None);
        for text in [
            "2000-01-01T00:00:00Z",
            "2024-02-29T23:59:59Z",
            "2026-10-12T12:00:00Z",
            "2100-03-01T00:00:00Z",
            "9999-12-31T23:59:59Z",
        ] {
            assert_eq!(utc_text(utc_seconds(text).unwrap()).as_deref(), Some(text));
        }
        assert_eq!(utc_text(946_684_799), None, "1999 is outside utc_time");
        let a = utc_seconds("2026-10-12T12:00:00Z").unwrap();
        let b = utc_seconds("2026-10-19T12:00:00Z").unwrap();
        assert_eq!(b - a, MaxTemporaryCertificateSeconds);
    }
}
