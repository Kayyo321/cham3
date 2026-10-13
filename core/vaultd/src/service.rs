//! cham3-vaultd operations (service IPC §3; ADR 0005 §§2–4, ADR 0006 §10 step 3).
//!
//! There is no unlocked state: a key is decrypted only inside one operation, from a
//! passphrase presented for that operation, and its seed lives only in secret regions
//! that are zeroized when the operation ends. There is no operation that signs
//! caller-chosen bytes or exports a private key. Commits are signed only as exact
//! proposals that contractd registered, that the vault decoded and checked itself and
//! whose digest the owner's trusted client approved together with the passphrase.
//!
//! The transport (socket activation, `SO_PEERCRED`, datagrams received into locked
//! regions) and the process protection at start ([`crate::protection`]) are applied
//! by the vaultd binary in M2-007; this module takes the caller role the transport
//! derived and the passphrase already in a secret region.

use std::collections::BTreeMap;
use std::path::PathBuf;

use cham3_contract_format::dcbor::{MaxEncodedBytes, decode};
use cham3_contract_format::envelope::{
    SignatureBytes, assemble_envelope, protected_header_bytes, public_key_is_valid, strict_verify,
    to_be_signed,
};
use cham3_contract_format::hash::{digest_t, key_id, sha256};
use cham3_contract_format::intrinsic::check_commit;
use cham3_contract_format::schema::{
    authority_class_t, commit_payload_t, identity_enrollment_t, invalidation_reason_t,
    key_certificate_t, key_invalidation_t, random_id_t,
};
use cham3_contract_format::{layer_t, object_kind_t};
use cham3_identityd::caller::{caller_role_t, service_name_t};
use cham3_identityd::index::{
    MaxTemporaryCertificateSeconds, ceremony_t, check_commit_scope, check_signing_window,
    key_class_t, key_scope_t, naming_t, user_state_t, utc_text,
};
use cham3_identityd::outcome::{
    authority_code_t, ipc_code_t, rule_code_t, service_error_t, service_result_t,
};
use cham3_os_adapter::fill_random;
use cham3_secure_memory::secret_region_t;
use ed25519_dalek::{Signer, SigningKey};
use subtle::ConstantTimeEq;

use crate::clock::vault_clock_t;
use crate::kdf::{MinMemoryKib, SaltBytes, derive_kek, kdf_error_t, kdf_params_t};
use crate::key_object::{
    NonceBytes, SeedBytes, key_metadata_t, key_object_error_t, key_object_t, public_key_of,
};
use crate::links::{identity_client_t, recovery_registry_t};
use crate::records::{attempts_t, bundle_operation_t, bundle_t};
use crate::store::{AttemptsRecord, PendingRecord, key_store_t, store_error_t};

/// Longest proposal lifetime (ADR 0005 §3).
pub const MaxProposalSeconds: u64 = 120;
/// Longest passphrase accepted, in bytes.
pub const MaxPassphraseBytes: usize = 1024;
/// Pending proposals per user and in total.
pub const DefaultMaxProposalsPerUser: usize = 8;
pub const DefaultMaxProposals: usize = 64;
/// Used or cancelled proposal ids kept until their lifetime ends, per user and in
/// total.
pub const DefaultMaxRetiredProposalsPerUser: usize = 32;
pub const DefaultMaxRetiredProposals: usize = 256;

/// vaultd configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct vault_config_t {
    pub root: PathBuf,
    /// Calibration of new key objects (fixed per release: `kdf_params_t::Default`).
    pub kdf: kdf_params_t,
    /// Largest KDF memory the vault derives with (ADR 0005 §4).
    pub locked_budget_bytes: u64,
    pub max_proposals_per_user: usize,
    pub max_proposals: usize,
    pub max_retired_proposals_per_user: usize,
    pub max_retired_proposals: usize,
}

impl vault_config_t {
    pub fn new(root: PathBuf) -> vault_config_t {
        vault_config_t {
            root,
            kdf: kdf_params_t::Default,
            locked_budget_bytes: crate::kdf::DefaultLockedBudgetBytes,
            max_proposals_per_user: DefaultMaxProposalsPerUser,
            max_proposals: DefaultMaxProposals,
            max_retired_proposals_per_user: DefaultMaxRetiredProposalsPerUser,
            max_retired_proposals: DefaultMaxRetiredProposals,
        }
    }
}

/// A proposal as contractd registers it (`register_proposal`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct proposal_request_t {
    pub proposal_id: random_id_t,
    pub owner_uid: u32,
    pub kind: object_kind_t,
    pub key_id: digest_t,
    /// Trunk parent the proposal builds on (`None` for a genesis).
    pub expected_head: Option<digest_t>,
    pub policy_epoch: u64,
    pub expires_in_seconds: u64,
    pub payload: Vec<u8>,
}

/// The digest the owner's client must approve: `SHA-256(to_be_signed)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct registered_proposal_t {
    pub proposal_id: random_id_t,
    pub approval_digest: digest_t,
}

/// Keys created by `create_user_keys`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct created_keys_t {
    pub user_id: digest_t,
    pub persistent_key_id: digest_t,
    pub temporary_key_id: digest_t,
}

/// A registered proposal id. A used or cancelled id stays (without its bytes) until
/// the proposal's lifetime ends, so it cannot be registered and approved again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum proposal_state_t {
    pending,
    used,
    cancelled,
}

struct proposal_t {
    owner_uid: u32,
    key_id: digest_t,
    protected: Vec<u8>,
    payload: Vec<u8>,
    approval_digest: digest_t,
    expires_monotonic: u64,
    expires_utc: i64,
    state: proposal_state_t,
}

impl proposal_t {
    /// Ends the proposal but keeps its id; its bytes are released.
    fn retire(&mut self, state: proposal_state_t) {
        self.state = state;
        self.protected = Vec::new();
        self.payload = Vec::new();
    }
}

/// One freshly generated key: its encrypted object and its seed (secret region).
struct new_key_t {
    object: key_object_t,
    seed: secret_region_t,
    public_key: [u8; 32],
}

fn ipc(code: ipc_code_t) -> service_error_t {
    service_error_t::ipc(code)
}

fn authority(code: authority_code_t) -> service_error_t {
    service_error_t::key_authority(code)
}

fn storage(_: store_error_t) -> service_error_t {
    ipc(ipc_code_t::storage_unavailable)
}

fn transient(error: &service_error_t) -> bool {
    matches!(
        error,
        service_error_t::ipc(
            ipc_code_t::storage_unavailable
                | ipc_code_t::restriction_not_durable
                | ipc_code_t::deadline_exceeded
        )
    )
}

fn random_bytes<const Length: usize>() -> service_result_t<[u8; Length]> {
    let mut bytes = [0u8; Length];
    fill_random(&mut bytes).map_err(|_| ipc(ipc_code_t::protection_unavailable))?;
    Ok(bytes)
}

fn kdf_failure(error: kdf_error_t) -> service_error_t {
    match error {
        // A stored object outside the bounds is corrupt or a downgrade attempt.
        kdf_error_t::out_of_bounds => ipc(ipc_code_t::storage_unavailable),
        kdf_error_t::over_budget | kdf_error_t::secret_memory => {
            ipc(ipc_code_t::protection_unavailable)
        }
        kdf_error_t::workspace_size | kdf_error_t::derivation => {
            ipc(ipc_code_t::protection_unavailable)
        }
    }
}

fn class_of_certificate(class: authority_class_t) -> key_class_t {
    match class {
        authority_class_t::persistent => key_class_t::persistent,
        authority_class_t::temporary => key_class_t::temporary,
    }
}

/// The vault.
pub struct vault_service_t<identity_impl_t, registry_impl_t, clock_impl_t>
where
    identity_impl_t: identity_client_t,
    registry_impl_t: recovery_registry_t,
    clock_impl_t: vault_clock_t,
{
    config: vault_config_t,
    store: key_store_t,
    identity: identity_impl_t,
    registry: registry_impl_t,
    clock: clock_impl_t,
    proposals: BTreeMap<random_id_t, proposal_t>,
}

impl<identity_impl_t, registry_impl_t, clock_impl_t>
    vault_service_t<identity_impl_t, registry_impl_t, clock_impl_t>
where
    identity_impl_t: identity_client_t,
    registry_impl_t: recovery_registry_t,
    clock_impl_t: vault_clock_t,
{
    /// Opens the vault. The configuration must allow deriving its own calibration
    /// within the locked budget.
    pub fn start(
        config: vault_config_t,
        identity: identity_impl_t,
        registry: registry_impl_t,
        clock: clock_impl_t,
    ) -> service_result_t<vault_service_t<identity_impl_t, registry_impl_t, clock_impl_t>> {
        let minimum = u64::from(MinMemoryKib).saturating_mul(1024);
        if config.locked_budget_bytes < minimum {
            return Err(ipc(ipc_code_t::protection_unavailable));
        }
        config
            .kdf
            .check(config.locked_budget_bytes)
            .map_err(kdf_failure)?;
        let store = key_store_t::open(&config.root).map_err(storage)?;
        Ok(vault_service_t {
            config,
            store,
            identity,
            registry,
            clock,
            proposals: BTreeMap::new(),
        })
    }

    pub fn identity_mut(&mut self) -> &mut identity_impl_t {
        &mut self.identity
    }

    pub fn registry_mut(&mut self) -> &mut registry_impl_t {
        &mut self.registry
    }

    pub fn clock_mut(&mut self) -> &mut clock_impl_t {
        &mut self.clock
    }

    pub fn store(&self) -> &key_store_t {
        &self.store
    }

    fn now_text(&self, offset: i64) -> service_result_t<String> {
        self.clock
            .utc_seconds()
            .checked_add(offset)
            .and_then(utc_text)
            .ok_or(ipc(ipc_code_t::storage_unavailable))
    }

    // ---------------------------------------------------------------- passphrases

    fn check_passphrase(passphrase: &secret_region_t) -> service_result_t<()> {
        if passphrase.len() > MaxPassphraseBytes {
            return Err(ipc(ipc_code_t::malformed_request));
        }
        Ok(())
    }

    fn attempts(&self, uid: u32) -> service_result_t<attempts_t> {
        match self
            .store
            .read_record(uid, AttemptsRecord)
            .map_err(storage)?
        {
            None => Ok(attempts_t::default()),
            Some(bytes) => {
                attempts_t::decode(&bytes).map_err(|_| ipc(ipc_code_t::storage_unavailable))
            }
        }
    }

    fn utc_now_unsigned(&self) -> u64 {
        u64::try_from(self.clock.utc_seconds()).unwrap_or(0)
    }

    fn record_failure(&self, uid: u32) -> service_result_t<()> {
        let mut attempts = self.attempts(uid)?;
        attempts.failures = attempts.failures.saturating_add(1);
        attempts.last_failure = self.utc_now_unsigned();
        self.store
            .write_record(uid, AttemptsRecord, &attempts.encode())
            .map_err(storage)
    }

    fn record_success(&self, uid: u32) -> service_result_t<()> {
        if self.attempts(uid)?.failures == 0 {
            return Ok(());
        }
        self.store
            .remove_record(uid, AttemptsRecord)
            .map_err(storage)
    }

    /// Derives the KEK of `key_id`'s object from `passphrase` and decrypts the seed.
    /// Applies the attempt limit; a wrong passphrase counts as a failure.
    fn unlock(
        &self,
        uid: u32,
        key_id: &digest_t,
        expected_class: key_class_t,
        passphrase: &secret_region_t,
    ) -> service_result_t<secret_region_t> {
        Self::check_passphrase(passphrase)?;
        if self.attempts(uid)?.locked_at(self.utc_now_unsigned()) {
            return Err(ipc(ipc_code_t::attempts_exhausted));
        }
        let object = self
            .store
            .read_key(uid, key_id)
            .map_err(storage)?
            .ok_or(authority(authority_code_t::key_not_certified))?;
        if object.metadata.authority_class != expected_class {
            return Err(ipc(ipc_code_t::storage_unavailable));
        }
        let kek = derive_kek(
            &object.metadata.kdf,
            self.config.locked_budget_bytes,
            passphrase.as_slice(),
            &object.metadata.salt,
        )
        .map_err(kdf_failure)?;
        match object.open(&kek) {
            Ok(seed) => {
                self.record_success(uid)?;
                Ok(seed)
            }
            Err(key_object_error_t::authentication_failed) => {
                self.record_failure(uid)?;
                Err(ipc(ipc_code_t::authentication_failed))
            }
            Err(key_object_error_t::secret_memory) => Err(ipc(ipc_code_t::protection_unavailable)),
            Err(_) => Err(ipc(ipc_code_t::storage_unavailable)),
        }
    }

    /// Generates a key in locked memory and encrypts it under `passphrase` with a
    /// fresh salt and nonce.
    fn generate_key(
        &self,
        class: key_class_t,
        uid: u32,
        user_id: Option<digest_t>,
        passphrase: &secret_region_t,
    ) -> service_result_t<new_key_t> {
        let mut seed =
            secret_region_t::new(SeedBytes).map_err(|_| ipc(ipc_code_t::protection_unavailable))?;
        fill_random(seed.as_mut_slice()).map_err(|_| ipc(ipc_code_t::protection_unavailable))?;
        let public_key = public_key_of(&seed).ok_or(ipc(ipc_code_t::protection_unavailable))?;
        if !public_key_is_valid(&public_key) {
            return Err(ipc(ipc_code_t::protection_unavailable));
        }
        let key = key_id(&public_key);
        let salt: [u8; SaltBytes] = random_bytes()?;
        let nonce: [u8; NonceBytes] = random_bytes()?;
        let metadata = key_metadata_t {
            authority_class: class,
            owner_uid: uid,
            // An identity key is its own user id.
            owner_user_id: user_id.unwrap_or(key),
            key_id: key,
            created: self.now_text(0)?,
            kdf: self.config.kdf,
            salt,
            nonce,
        };
        let kek = derive_kek(
            &self.config.kdf,
            self.config.locked_budget_bytes,
            passphrase.as_slice(),
            &salt,
        )
        .map_err(kdf_failure)?;
        let object = key_object_t::seal(metadata, &kek, &seed).map_err(|error| match error {
            key_object_error_t::secret_memory => ipc(ipc_code_t::protection_unavailable),
            _ => ipc(ipc_code_t::storage_unavailable),
        })?;
        Ok(new_key_t {
            object,
            seed,
            public_key,
        })
    }

    /// Signs the Sig_structure the vault builds itself and checks the signature.
    fn sign(
        seed: &secret_region_t,
        kind: object_kind_t,
        protected: &[u8],
        payload: &[u8],
    ) -> service_result_t<Vec<u8>> {
        let bytes = <&[u8; SeedBytes]>::try_from(seed.as_slice())
            .map_err(|_| ipc(ipc_code_t::protection_unavailable))?;
        // ed25519-dalek zeroizes this copy when it is dropped (`zeroize` feature).
        let signing = SigningKey::from_bytes(bytes);
        let message = to_be_signed(kind, protected, payload);
        let signature: [u8; SignatureBytes] = signing.sign(&message).to_bytes();
        let public_key = signing.verifying_key().to_bytes();
        drop(signing);
        if !strict_verify(&public_key, &message, &signature) {
            return Err(ipc(ipc_code_t::protection_unavailable));
        }
        Ok(assemble_envelope(protected, payload, &signature))
    }

    fn sign_identity_object(
        seed: &secret_region_t,
        signer: &digest_t,
        kind: object_kind_t,
        payload: &cham3_contract_format::dcbor::value_t,
    ) -> service_result_t<Vec<u8>> {
        let payload = cham3_contract_format::dcbor::encode(payload);
        Self::sign(seed, kind, &protected_header_bytes(signer, kind), &payload)
    }

    // ---------------------------------------------------------------- pending bundles

    fn read_bundle(&self, uid: u32) -> service_result_t<Option<bundle_t>> {
        match self
            .store
            .read_record(uid, PendingRecord)
            .map_err(storage)?
        {
            None => Ok(None),
            Some(bytes) => bundle_t::decode(&bytes)
                .map(Some)
                .map_err(|_| ipc(ipc_code_t::storage_unavailable)),
        }
    }

    fn write_bundle(&self, uid: u32, bundle: &bundle_t) -> service_result_t<()> {
        self.store
            .write_record(uid, PendingRecord, &bundle.encode())
            .map_err(storage)
    }

    /// Erases key objects (they can never sign again) and removes the bundle.
    fn finish_bundle(&self, uid: u32, erase: &[digest_t]) -> service_result_t<()> {
        for key in erase {
            self.store.erase_key(uid, key).map_err(storage)?;
        }
        self.store
            .remove_record(uid, PendingRecord)
            .map_err(storage)
    }

    /// Completes or reconciles `uid`'s pending bundle. Transient failures keep it
    /// (nothing is erased on an unknown outcome); a definitive refusal erases the
    /// keys the bundle introduced, which can then never be registered.
    fn complete_pending(&mut self, uid: u32) -> service_result_t<()> {
        let Some(bundle) = self.read_bundle(uid)? else {
            return Ok(());
        };
        if bundle.operation == bundle_operation_t::recovery {
            return self.reconcile_recovery(uid, &bundle);
        }
        let outcome = self.register_bundle(uid, &bundle);
        match outcome {
            Ok(()) => self.finish_bundle(uid, &bundle.erase_after),
            Err(error) if transient(&error) => Err(error),
            Err(error) => {
                self.finish_bundle(uid, &bundle.new_keys)?;
                Err(error)
            }
        }
    }

    fn register_bundle(&mut self, uid: u32, bundle: &bundle_t) -> service_result_t<()> {
        if bundle.operation == bundle_operation_t::enrollment {
            let [(_, enrollment), (_, persistent), (_, temporary)] = bundle.objects.as_slice()
            else {
                return Err(ipc(ipc_code_t::storage_unavailable));
            };
            self.identity
                .register_enrollment(uid, enrollment, persistent, temporary)?;
            return Ok(());
        }
        for (kind, object) in &bundle.objects {
            match kind {
                object_kind_t::key_invalidation => {
                    self.identity.register_invalidation(uid, object)?;
                }
                object_kind_t::key_certificate => {
                    self.identity.register_certificate(uid, object)?;
                }
                _ => return Err(ipc(ipc_code_t::storage_unavailable)),
            }
        }
        Ok(())
    }

    /// `recover_keys` reconciliation (service IPC §3, revision 4 R14): decided from
    /// registryd `recovery_status` only; objects are erased only on a positive answer
    /// that they can never be named.
    fn reconcile_recovery(&mut self, uid: u32, bundle: &bundle_t) -> service_result_t<()> {
        let (recovery_id, [persistent, temporary]) =
            match (bundle.recovery_id, bundle.new_keys.as_slice()) {
                (Some(recovery_id), [persistent, temporary]) => {
                    (recovery_id, [*persistent, *temporary])
                }
                _ => return Err(ipc(ipc_code_t::storage_unavailable)),
            };
        let held = (persistent, temporary);
        // Status unavailable: keep every object and retry later.
        let status = self.registry.recovery_status(uid, &recovery_id)?;
        if let Some(named) = status.named {
            return if named == held {
                // Step 3 is complete; the superseded keys are retired.
                self.finish_bundle(uid, &bundle.erase_after)
            } else {
                // Other keys were named: the held keys are retired.
                self.finish_bundle(uid, &bundle.new_keys)
            };
        }
        if !status.ceremony_open {
            // The ceremony ended without naming these keys: they can never be named.
            return self.finish_bundle(uid, &bundle.new_keys);
        }
        // Nothing named and the ceremony still open: resume with the held keys.
        self.register_bundle(uid, bundle)?;
        self.registry.name_recovery_keys(
            uid,
            &naming_t {
                recovery_id,
                persistent_key_id: persistent,
                temporary_key_id: temporary,
            },
        )?;
        self.finish_bundle(uid, &bundle.erase_after)
    }

    // ---------------------------------------------------------------- enrollment

    /// `create_user_keys` (`trusted_client`): the caller's identity, persistent and
    /// temporary keys, encrypted under a new passphrase and registered with identityd.
    /// A retry after an interrupted enrollment resumes with the held keys and requires
    /// the passphrase that encrypted them.
    pub fn create_user_keys(
        &mut self,
        caller: caller_role_t,
        passphrase: &secret_region_t,
    ) -> service_result_t<created_keys_t> {
        let uid = caller.trusted_uid().ok_or(ipc(ipc_code_t::role_denied))?;
        Self::check_passphrase(passphrase)?;
        if let Some(bundle) = self.read_bundle(uid)? {
            return match (bundle.operation, bundle.new_keys.as_slice()) {
                (bundle_operation_t::enrollment, [identity, persistent, temporary]) => {
                    let (identity, persistent, temporary) = (*identity, *persistent, *temporary);
                    self.unlock(uid, &identity, key_class_t::identity, passphrase)?;
                    self.complete_pending(uid)?;
                    Ok(created_keys_t {
                        user_id: identity,
                        persistent_key_id: persistent,
                        temporary_key_id: temporary,
                    })
                }
                _ => {
                    self.complete_pending(uid)?;
                    Err(ipc(ipc_code_t::duplicate_identifier))
                }
            };
        }
        match self.identity.user_identity(uid) {
            Ok(_) => return Err(ipc(ipc_code_t::duplicate_identifier)),
            Err(service_error_t::key_authority(authority_code_t::key_not_certified)) => {}
            Err(error) => return Err(error),
        }
        let identity = self.generate_key(key_class_t::identity, uid, None, passphrase)?;
        let user_id = identity.object.metadata.key_id;
        let persistent =
            self.generate_key(key_class_t::persistent, uid, Some(user_id), passphrase)?;
        let temporary =
            self.generate_key(key_class_t::temporary, uid, Some(user_id), passphrase)?;
        let now = self.now_text(0)?;
        let enrollment = identity_enrollment_t {
            user_id,
            public_key: identity.public_key,
            owner_uid: uid,
            enrolled: now.clone(),
            identity_epoch: 1,
        };
        let enrollment = Self::sign_identity_object(
            &identity.seed,
            &user_id,
            object_kind_t::identity_enrollment,
            &enrollment.to_value(),
        )?;
        let mut objects = vec![(object_kind_t::identity_enrollment, enrollment)];
        for key in [&persistent, &temporary] {
            let certificate = self.certificate_for(user_id, uid, key, 1, None)?;
            objects.push((
                object_kind_t::key_certificate,
                Self::sign_identity_object(
                    &identity.seed,
                    &user_id,
                    object_kind_t::key_certificate,
                    &certificate.to_value(),
                )?,
            ));
        }
        let created = created_keys_t {
            user_id,
            persistent_key_id: persistent.object.metadata.key_id,
            temporary_key_id: temporary.object.metadata.key_id,
        };
        let bundle = bundle_t {
            operation: bundle_operation_t::enrollment,
            recovery_id: None,
            objects,
            new_keys: vec![user_id, created.persistent_key_id, created.temporary_key_id],
            erase_after: Vec::new(),
        };
        // Durable before registration: the objects first, then the bundle (a bundle
        // never names an object that is not on disk; an interrupted write leaves at
        // most unregistered encrypted objects).
        for key in [&identity, &persistent, &temporary] {
            self.store.write_key(&key.object).map_err(storage)?;
        }
        self.write_bundle(uid, &bundle)?;
        drop((identity.seed, persistent.seed, temporary.seed));
        self.complete_pending(uid)?;
        Ok(created)
    }

    fn certificate_for(
        &self,
        user_id: digest_t,
        uid: u32,
        key: &new_key_t,
        key_epoch: u64,
        supersedes: Option<digest_t>,
    ) -> service_result_t<key_certificate_t> {
        let class = key.object.metadata.authority_class;
        let authority_class = match class {
            key_class_t::persistent => authority_class_t::persistent,
            key_class_t::temporary => authority_class_t::temporary,
            key_class_t::identity => return Err(ipc(ipc_code_t::malformed_request)),
        };
        let now = self.now_text(0)?;
        let not_after = match class {
            key_class_t::temporary => Some(self.now_text(MaxTemporaryCertificateSeconds)?),
            _ => None,
        };
        Ok(key_certificate_t {
            certificate_id: random_bytes()?,
            issued: now.clone(),
            owner_uid: uid,
            owner_user_id: user_id,
            subject_key_id: key.object.metadata.key_id,
            subject_public_key: key.public_key,
            authority_class,
            key_epoch,
            not_before: now,
            not_after,
            supersedes_key_id: supersedes,
        })
    }

    // ---------------------------------------------------------------- proposals

    /// `register_proposal` (`service:contractd`): exact payload bytes for one signing
    /// operation. The vault decodes and checks the payload itself, binds it to the
    /// owner, key, head and epoch, checks the key's authority and scope with
    /// identityd and its certificate window at the current time, and returns the
    /// approval digest. A proposal id is single use: a used or cancelled id is refused
    /// (`duplicate_identifier`) until its proposal's lifetime ends.
    pub fn register_proposal(
        &mut self,
        caller: caller_role_t,
        request: proposal_request_t,
    ) -> service_result_t<registered_proposal_t> {
        if caller != caller_role_t::service(service_name_t::contractd) {
            return Err(ipc(ipc_code_t::role_denied));
        }
        if request.kind != object_kind_t::commit
            || request.expires_in_seconds == 0
            || request.expires_in_seconds > MaxProposalSeconds
        {
            return Err(ipc(ipc_code_t::malformed_request));
        }
        self.expire_proposals();
        // A used or cancelled id is refused for the rest of its lifetime too.
        if self.proposals.contains_key(&request.proposal_id) {
            return Err(ipc(ipc_code_t::duplicate_identifier));
        }
        let count = |pending_only: bool, user: Option<u32>| {
            self.proposals
                .values()
                .filter(|proposal| !pending_only || proposal.state == proposal_state_t::pending)
                .filter(|proposal| user.is_none_or(|uid| proposal.owner_uid == uid))
                .count()
        };
        // The retained-id bounds count every kept id, pending ones included: a pending
        // proposal becomes a retained used or cancelled id without any further check,
        // so its slot is reserved at registration (M2-004 revision-2 review,
        // M2-004-R2-01). The pending bounds are the smaller limits on live proposals.
        if count(true, Some(request.owner_uid)) >= self.config.max_proposals_per_user
            || count(true, None) >= self.config.max_proposals
            || count(false, Some(request.owner_uid)) >= self.config.max_retired_proposals_per_user
            || count(false, None) >= self.config.max_retired_proposals
        {
            return Err(ipc(ipc_code_t::quota_exhausted));
        }
        let protected = protected_header_bytes(&request.key_id, object_kind_t::commit);
        let envelope_length =
            assemble_envelope(&protected, &request.payload, &[0; SignatureBytes]).len();
        if envelope_length > MaxEncodedBytes {
            return Err(ipc(ipc_code_t::object_too_large));
        }
        let payload = parse_commit(&request.payload, &request.key_id)?;
        if payload.actor.uid != request.owner_uid {
            return Err(ipc(ipc_code_t::owner_mismatch));
        }
        let head = payload
            .merge_base
            .or_else(|| payload.parents.first().copied());
        if head != request.expected_head || payload.policy.epoch != request.policy_epoch {
            return Err(ipc(ipc_code_t::malformed_request));
        }
        self.complete_pending(request.owner_uid)?;
        self.check_signing_key(request.owner_uid, &payload)?;
        let message = to_be_signed(object_kind_t::commit, &protected, &request.payload);
        let approval_digest = sha256(&message);
        let monotonic = self.clock.monotonic_seconds();
        let expires_utc = i64::try_from(request.expires_in_seconds)
            .ok()
            .and_then(|seconds| self.clock.utc_seconds().checked_add(seconds))
            .ok_or(ipc(ipc_code_t::malformed_request))?;
        self.proposals.insert(
            request.proposal_id,
            proposal_t {
                owner_uid: request.owner_uid,
                key_id: request.key_id,
                protected,
                payload: request.payload,
                approval_digest,
                expires_monotonic: monotonic.saturating_add(request.expires_in_seconds),
                expires_utc,
                state: proposal_state_t::pending,
            },
        );
        Ok(registered_proposal_t {
            proposal_id: request.proposal_id,
            approval_digest,
        })
    }

    /// Layer 6 plus scope for the proposal's signing key, the key's certificate
    /// window now, and the vault must hold the key's object.
    fn check_signing_key(&mut self, uid: u32, payload: &commit_payload_t) -> service_result_t<()> {
        let resolution = self.identity.resolve_key(&payload.key_id)?;
        check_commit_scope(&resolution, payload)?;
        check_signing_window(&resolution, self.clock.utc_seconds())?;
        let object = self
            .store
            .read_key(uid, &payload.key_id)
            .map_err(storage)?
            .ok_or(authority(authority_code_t::key_not_certified))?;
        // The stored object must describe the key identityd certified.
        if object.metadata.owner_user_id != resolution.owner_user_id
            || object.metadata.authority_class != resolution.class
        {
            return Err(ipc(ipc_code_t::storage_unavailable));
        }
        Ok(())
    }

    fn expired(&self, proposal: &proposal_t) -> bool {
        self.clock.monotonic_seconds() >= proposal.expires_monotonic
            || self.clock.utc_seconds() >= proposal.expires_utc
    }

    /// Forgets every proposal id, pending or retired, whose lifetime has ended.
    fn expire_proposals(&mut self) {
        let expired: Vec<random_id_t> = self
            .proposals
            .iter()
            .filter(|(_, proposal)| self.expired(proposal))
            .map(|(id, _)| *id)
            .collect();
        for id in expired {
            self.proposals.remove(&id);
        }
    }

    /// `approve_and_sign` (`trusted_client`, owner): signs one registered proposal
    /// whose digest the client computed from the bytes it displayed, after passphrase
    /// authentication; the proposal is then used. Key status, scope and the current
    /// certificate window are checked before the KEK derivation and again after it,
    /// immediately before the signature. (The second check narrows, but does not
    /// close, the interval in which another service's restriction can cross a
    /// signature; serializing restrictions with signatures across services is part
    /// of the M2-006/M2-007 integration.)
    pub fn approve_and_sign(
        &mut self,
        caller: caller_role_t,
        proposal_id: &random_id_t,
        approval_digest: &digest_t,
        passphrase: &secret_region_t,
    ) -> service_result_t<Vec<u8>> {
        let uid = caller.trusted_uid().ok_or(ipc(ipc_code_t::role_denied))?;
        Self::check_passphrase(passphrase)?;
        let (key_id, payload, protected) = {
            let proposal = self
                .proposals
                .get(proposal_id)
                .ok_or(ipc(ipc_code_t::proposal_unknown))?;
            if proposal.owner_uid != uid {
                return Err(ipc(ipc_code_t::owner_mismatch));
            }
            match proposal.state {
                proposal_state_t::pending => {}
                proposal_state_t::used => return Err(ipc(ipc_code_t::proposal_used)),
                proposal_state_t::cancelled => return Err(ipc(ipc_code_t::proposal_unknown)),
            }
            if self.expired(proposal) {
                self.proposals.remove(proposal_id);
                return Err(ipc(ipc_code_t::proposal_expired));
            }
            if !bool::from(proposal.approval_digest.ct_eq(approval_digest)) {
                return Err(ipc(ipc_code_t::approval_mismatch));
            }
            (
                proposal.key_id,
                proposal.payload.clone(),
                proposal.protected.clone(),
            )
        };
        // Key status and scope are checked again immediately before signing.
        self.complete_pending(uid)?;
        let parsed = parse_commit(&payload, &key_id)?;
        self.check_signing_key(uid, &parsed)?;
        let class = class_of_certificate(parsed.authority_class);
        let seed = self.unlock(uid, &key_id, class, passphrase)?;
        let live = self.proposals.get(proposal_id).is_some_and(|proposal| {
            proposal.state == proposal_state_t::pending && !self.expired(proposal)
        });
        if !live {
            // Only an ended lifetime removes an id; a used or cancelled marker stays.
            self.expire_proposals();
            return Err(ipc(ipc_code_t::proposal_expired));
        }
        // Final check after the derivation, immediately before the signature: key
        // status, scope and window may have changed meanwhile (a refusal drops `seed`,
        // which clears it).
        self.check_signing_key(uid, &parsed)?;
        let envelope = Self::sign(&seed, object_kind_t::commit, &protected, &payload)?;
        drop(seed);
        if let Some(proposal) = self.proposals.get_mut(proposal_id) {
            proposal.retire(proposal_state_t::used);
        }
        Ok(envelope)
    }

    /// `cancel_proposal` (`trusted_client` owner or `service:contractd`): ends a
    /// pending proposal; its id stays refused for the rest of its lifetime. A used
    /// proposal's marker cannot be cancelled away.
    pub fn cancel_proposal(
        &mut self,
        caller: caller_role_t,
        proposal_id: &random_id_t,
    ) -> service_result_t<()> {
        self.expire_proposals();
        let proposal = self
            .proposals
            .get_mut(proposal_id)
            .ok_or(ipc(ipc_code_t::proposal_unknown))?;
        match caller {
            caller_role_t::service(service_name_t::contractd) => {}
            caller_role_t::trusted_client { uid } if uid == proposal.owner_uid => {}
            caller_role_t::trusted_client { .. } => return Err(ipc(ipc_code_t::owner_mismatch)),
            _ => return Err(ipc(ipc_code_t::role_denied)),
        }
        match proposal.state {
            proposal_state_t::pending => {
                proposal.retire(proposal_state_t::cancelled);
                Ok(())
            }
            proposal_state_t::used => Err(ipc(ipc_code_t::proposal_used)),
            proposal_state_t::cancelled => Err(ipc(ipc_code_t::proposal_unknown)),
        }
    }

    // ---------------------------------------------------------------- rotation

    /// The user's identity key, checked for a scope that permits `permitted`.
    fn identity_key(
        &mut self,
        uid: u32,
        permitted: impl Fn(key_scope_t) -> bool,
    ) -> service_result_t<digest_t> {
        let identity = self.identity.user_identity(uid)?;
        let resolution = self.identity.resolve_key(&identity.user_id)?;
        if permitted(resolution.scope) {
            return Ok(identity.user_id);
        }
        Err(match resolution.scope {
            key_scope_t::denied(code) => authority(code),
            _ => match identity.state {
                user_state_t::contained => authority(authority_code_t::contained),
                _ => ipc(ipc_code_t::role_denied),
            },
        })
    }

    /// Held contract key objects of `uid` and class `class` whose key is still usable.
    fn usable_held_keys(
        &mut self,
        uid: u32,
        class: key_class_t,
    ) -> service_result_t<Vec<(digest_t, u64)>> {
        let mut usable = Vec::new();
        for key in self.store.list_keys(uid).map_err(storage)? {
            let Ok(resolution) = self.identity.resolve_key(&key) else {
                continue;
            };
            if resolution.owner_uid == uid
                && resolution.class == class
                && !matches!(resolution.scope, key_scope_t::denied(_))
            {
                usable.push((key, resolution.key_epoch));
            }
        }
        Ok(usable)
    }

    /// `rotate_key` (`trusted_client`): a new key of `class`, certified by the
    /// identity key; the user's usable keys of that class are invalidated (`rotated`)
    /// first and their objects erased. Not available while contained (the ceremony's
    /// `recover_keys` is the only key creation then).
    pub fn rotate_key(
        &mut self,
        caller: caller_role_t,
        class: authority_class_t,
        passphrase: &secret_region_t,
    ) -> service_result_t<digest_t> {
        let uid = caller.trusted_uid().ok_or(ipc(ipc_code_t::role_denied))?;
        Self::check_passphrase(passphrase)?;
        self.complete_pending(uid)?;
        let normal = |scope: key_scope_t| scope == key_scope_t::normal;
        let user_id = self.identity_key(uid, normal)?;
        let identity = self.identity.user_identity(uid)?;
        let key_class = class_of_certificate(class);
        let current = self.usable_held_keys(uid, key_class)?;
        let seed = self.unlock(uid, &user_id, key_class_t::identity, passphrase)?;
        let new_key = self.generate_key(key_class, uid, Some(user_id), passphrase)?;
        // The identity key's scope again, after the derivations and immediately
        // before it signs.
        self.identity_key(uid, normal)?;
        let epoch = current
            .iter()
            .map(|(_, epoch)| *epoch)
            .chain([identity.min_key_epoch, 1])
            .max()
            .unwrap_or(1);
        let newest = current
            .iter()
            .max_by_key(|(_, epoch)| *epoch)
            .map(|(key, _)| *key);
        let mut objects = Vec::new();
        for (key, _) in &current {
            objects.push((
                object_kind_t::key_invalidation,
                self.invalidation_object(&seed, user_id, *key, invalidation_reason_t::rotated)?,
            ));
        }
        let certificate = self.certificate_for(user_id, uid, &new_key, epoch, newest)?;
        objects.push((
            object_kind_t::key_certificate,
            Self::sign_identity_object(
                &seed,
                &user_id,
                object_kind_t::key_certificate,
                &certificate.to_value(),
            )?,
        ));
        drop(seed);
        let bundle = bundle_t {
            operation: bundle_operation_t::rotation,
            recovery_id: None,
            objects,
            new_keys: vec![new_key.object.metadata.key_id],
            erase_after: current.iter().map(|(key, _)| *key).collect(),
        };
        self.store.write_key(&new_key.object).map_err(storage)?;
        self.write_bundle(uid, &bundle)?;
        drop(new_key.seed);
        self.complete_pending(uid)?;
        Ok(new_key.object.metadata.key_id)
    }

    fn invalidation_object(
        &self,
        seed: &secret_region_t,
        user_id: digest_t,
        target: digest_t,
        reason: invalidation_reason_t,
    ) -> service_result_t<Vec<u8>> {
        let invalidation = key_invalidation_t {
            invalidation_id: random_bytes()?,
            issued: self.now_text(0)?,
            owner_user_id: user_id,
            target_key_id: target,
            reason,
        };
        Self::sign_identity_object(
            seed,
            &user_id,
            object_kind_t::key_invalidation,
            &invalidation.to_value(),
        )
    }

    /// `invalidate_key` (`trusted_client`): narrowing, so available in every state in
    /// which identityd still lets the identity key sign (not quarantined or deleted).
    pub fn invalidate_key(
        &mut self,
        caller: caller_role_t,
        target: &digest_t,
        reason: invalidation_reason_t,
        passphrase: &secret_region_t,
    ) -> service_result_t<()> {
        let uid = caller.trusted_uid().ok_or(ipc(ipc_code_t::role_denied))?;
        Self::check_passphrase(passphrase)?;
        self.complete_pending(uid)?;
        let not_denied = |scope: key_scope_t| !matches!(scope, key_scope_t::denied(_));
        let user_id = self.identity_key(uid, not_denied)?;
        let resolution = self.identity.resolve_key(target)?;
        if resolution.owner_uid != uid || resolution.class == key_class_t::identity {
            return Err(authority(authority_code_t::key_not_certified));
        }
        let seed = self.unlock(uid, &user_id, key_class_t::identity, passphrase)?;
        // The identity key's scope again, after the derivation and immediately before
        // it signs.
        self.identity_key(uid, not_denied)?;
        let object = self.invalidation_object(&seed, user_id, *target, reason)?;
        drop(seed);
        let held = self.store.read_key(uid, target).map_err(storage)?.is_some();
        let bundle = bundle_t {
            operation: bundle_operation_t::invalidation,
            recovery_id: None,
            objects: vec![(object_kind_t::key_invalidation, object)],
            new_keys: Vec::new(),
            erase_after: if held { vec![*target] } else { Vec::new() },
        };
        self.write_bundle(uid, &bundle)?;
        self.complete_pending(uid)
    }

    // ---------------------------------------------------------------- recovery

    /// The caller's open ceremony `recovery_id`, which must not have named keys yet.
    fn open_ceremony(
        &mut self,
        uid: u32,
        recovery_id: &random_id_t,
    ) -> service_result_t<ceremony_t> {
        self.identity
            .user_identity(uid)?
            .ceremony
            .filter(|ceremony| ceremony.recovery_id == *recovery_id && ceremony.named.is_none())
            .ok_or(service_error_t::admission(rule_code_t::recovery_rule))
    }

    /// `recover_keys` (`trusted_client`, owner): ceremony step 3 (ADR 0006 §10). New
    /// persistent and temporary keys, written durably with their certificates at the
    /// ceremony's key epoch, registered with identityd and named through registryd
    /// `name_recovery_keys`. Success is reported only after the naming answered `ok`;
    /// any failure is reconciled from `recovery_status`. A call that finds this
    /// ceremony's held bundle completes it without checking `passphrase`, as every
    /// other vault operation of the user does first: reconciliation creates and signs
    /// nothing (the held objects and certificates come from the authenticated call
    /// that wrote them), unlike `create_user_keys`, whose retry authenticates.
    pub fn recover_keys(
        &mut self,
        caller: caller_role_t,
        recovery_id: &random_id_t,
        passphrase: &secret_region_t,
    ) -> service_result_t<(digest_t, digest_t)> {
        let uid = caller.trusted_uid().ok_or(ipc(ipc_code_t::role_denied))?;
        Self::check_passphrase(passphrase)?;
        // A held recovery bundle is reconciled first; held keys are reused, never
        // replaced by new ones while they can still be named.
        if let Some(bundle) = self.read_bundle(uid)? {
            let held = bundle.new_keys.clone();
            let ours = bundle.recovery_id == Some(*recovery_id);
            self.complete_pending(uid)?;
            if ours {
                let status = self.registry.recovery_status(uid, recovery_id)?;
                if let (Some(named), [persistent, temporary]) = (status.named, held.as_slice()) {
                    if named == (*persistent, *temporary) {
                        return Ok(named);
                    }
                }
            }
        }
        let ceremony = self.open_ceremony(uid, recovery_id)?;
        let in_ceremony = |scope: key_scope_t| {
            scope
                == key_scope_t::recovery {
                    recovery_id: Some(*recovery_id),
                }
        };
        let user_id = self.identity_key(uid, in_ceremony)?;
        let mut superseded = Vec::new();
        let mut held_contract_keys = Vec::new();
        for key in self.store.list_keys(uid).map_err(storage)? {
            if key == user_id {
                continue;
            }
            held_contract_keys.push(key);
            if let Ok(resolution) = self.identity.resolve_key(&key) {
                superseded.push((key, resolution.class, resolution.key_epoch));
            }
        }
        let newest = |class: key_class_t| {
            superseded
                .iter()
                .filter(|(_, key_class, _)| *key_class == class)
                .max_by_key(|(_, _, epoch)| *epoch)
                .map(|(key, _, _)| *key)
        };
        let seed = self.unlock(uid, &user_id, key_class_t::identity, passphrase)?;
        let persistent =
            self.generate_key(key_class_t::persistent, uid, Some(user_id), passphrase)?;
        let temporary =
            self.generate_key(key_class_t::temporary, uid, Some(user_id), passphrase)?;
        // The ceremony and the identity key's ceremony scope again, after the
        // derivations and immediately before it signs the certificates.
        self.open_ceremony(uid, recovery_id)?;
        self.identity_key(uid, in_ceremony)?;
        let mut objects = Vec::new();
        for (key, class) in [
            (&persistent, key_class_t::persistent),
            (&temporary, key_class_t::temporary),
        ] {
            let certificate =
                self.certificate_for(user_id, uid, key, ceremony.key_epoch, newest(class))?;
            objects.push((
                object_kind_t::key_certificate,
                Self::sign_identity_object(
                    &seed,
                    &user_id,
                    object_kind_t::key_certificate,
                    &certificate.to_value(),
                )?,
            ));
        }
        drop(seed);
        let named = (
            persistent.object.metadata.key_id,
            temporary.object.metadata.key_id,
        );
        let bundle = bundle_t {
            operation: bundle_operation_t::recovery,
            recovery_id: Some(*recovery_id),
            objects,
            new_keys: vec![named.0, named.1],
            erase_after: held_contract_keys,
        };
        for key in [&persistent, &temporary] {
            self.store.write_key(&key.object).map_err(storage)?;
        }
        self.write_bundle(uid, &bundle)?;
        drop((persistent.seed, temporary.seed));
        // Register both certificates, then name the keys; on any failure reconcile.
        let first = self.register_bundle(uid, &bundle).and_then(|()| {
            self.registry
                .name_recovery_keys(
                    uid,
                    &naming_t {
                        recovery_id: *recovery_id,
                        persistent_key_id: named.0,
                        temporary_key_id: named.1,
                    },
                )
                .map(|_| ())
        });
        match first {
            Ok(()) => {
                self.finish_bundle(uid, &bundle.erase_after)?;
                Ok(named)
            }
            Err(error) => {
                // Every failure, whatever its code, is reconciled once now from
                // `recovery_status` (R14); the error alone never erases a held object,
                // and the bundle stays while the outcome is unknown.
                let reconciled = self
                    .complete_pending(uid)
                    .and_then(|()| self.registry.recovery_status(uid, recovery_id));
                match reconciled {
                    Ok(status) if status.named == Some(named) => Ok(named),
                    _ => Err(error),
                }
            }
        }
    }
}

/// Decodes and checks commit payload bytes the way layers 4–5 do, with `key_id` as
/// the protected-header `kid` they will be signed under.
fn parse_commit(payload: &[u8], key_id: &digest_t) -> service_result_t<commit_payload_t> {
    let value = decode(payload, layer_t::payload_decode, false)?;
    let commit = commit_payload_t::parse(&value, key_id)?;
    check_commit(&commit)?;
    Ok(commit)
}
