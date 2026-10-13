//! vaultd with real providers (Argon2id, XChaCha20-Poly1305, Ed25519, getrandom) in
//! secret regions, against the identity service over the journal fixture: key
//! creation, exact-proposal approval and signing, approval binding and replay,
//! single-use proposal ids, the signing key's window at signing time, authority
//! changes during the derivation, attempt limiting, tampered and substituted key
//! objects, user separation, roles, invalidation, rotation and restartable
//! registration (A04, A05, A12 in-process).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::as_conversions
)]

mod fixture;

use std::os::unix::fs::PermissionsExt;

use cham3_contract_format::dcbor::encode;
use cham3_contract_format::envelope::{assemble_envelope, protected_header_bytes, to_be_signed};
use cham3_contract_format::hash::{digest_t, sha256};
use cham3_contract_format::schema::{authority_class_t, invalidation_reason_t, key_invalidation_t};
use cham3_contract_format::{key_resolver_t, object_kind_t, payload_t, verify_object};
use cham3_identityd::caller::{caller_role_t, service_name_t};
use cham3_identityd::index::{
    MaxTemporaryCertificateSeconds, check_commit_authority, key_scope_t, user_record_t,
};
use cham3_identityd::outcome::service_error_t;
use cham3_vaultd::kdf::{derive_kek, kdf_params_t};
use cham3_vaultd::key_object::key_object_t;
use cham3_vaultd::service::{
    DefaultMaxRetiredProposalsPerUser, created_keys_t, proposal_request_t,
};
use ed25519_dalek::{Signer, SigningKey};
use fixture::*;

const Contractd: caller_role_t = caller_role_t::service(service_name_t::contractd);
const Alice: caller_role_t = caller_role_t::trusted_client { uid: 1001 };
const Bob: caller_role_t = caller_role_t::trusted_client { uid: 1002 };
const AlicePassphrase: &str = "synthetic alice passphrase";
const BobPassphrase: &str = "synthetic bob passphrase";
const CommitTime: &str = "2026-10-13T08:05:00Z";

fn code<value_t: std::fmt::Debug>(result: Result<value_t, service_error_t>) -> String {
    match result {
        Ok(value) => panic!("expected a refusal, got {value:?}"),
        Err(error) => error.to_string(),
    }
}

struct setup_t {
    root: std::path::PathBuf,
    identity: identity_t,
    vault: vault_t,
    alice: created_keys_t,
}

fn setup(label: &str) -> setup_t {
    let root = temp_root(label);
    let identity = identity_service();
    let mut vault = vault(&root, &identity);
    let alice = vault
        .create_user_keys(Alice, &passphrase(AlicePassphrase))
        .expect("alice enrolls");
    setup_t {
        root,
        identity,
        vault,
        alice,
    }
}

fn request(
    label: &str,
    uid: u32,
    payload: &cham3_contract_format::schema::commit_payload_t,
) -> proposal_request_t {
    proposal_request_t {
        proposal_id: proposal_id(label),
        owner_uid: uid,
        kind: object_kind_t::commit,
        key_id: payload.key_id,
        expected_head: trunk_parent(payload),
        policy_epoch: payload.policy.epoch,
        expires_in_seconds: 120,
        payload: encoded(payload),
    }
}

fn alice_genesis(setup: &setup_t) -> cham3_contract_format::schema::commit_payload_t {
    commit_payload(
        "commit_genesis",
        1001,
        &setup.alice.user_id,
        &setup.alice.persistent_key_id,
        authority_class_t::persistent,
        CommitTime,
    )
}

/// What the trusted client computes from the bytes it displays.
fn client_digest(key_id: &digest_t, payload: &[u8]) -> digest_t {
    let protected = protected_header_bytes(key_id, object_kind_t::commit);
    sha256(&to_be_signed(object_kind_t::commit, &protected, payload))
}

struct identity_resolver_t<'a>(&'a identity_t);

impl key_resolver_t for identity_resolver_t<'_> {
    fn resolve(&self, kid: &digest_t) -> Option<[u8; 32]> {
        self.0
            .borrow_mut()
            .resolve_key(Contractd, kid)
            .ok()
            .map(|resolution| resolution.public_key)
    }
}

#[test]
fn enrollment_creates_three_encrypted_keys_registered_with_identityd() {
    let setup = setup("enrollment");
    let directory = setup.root.join("1001");
    assert_eq!(
        std::fs::metadata(&setup.root).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        std::fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let keys = setup.vault.store().list_keys(1001).unwrap();
    assert_eq!(keys.len(), 3);
    for key in [
        setup.alice.user_id,
        setup.alice.persistent_key_id,
        setup.alice.temporary_key_id,
    ] {
        assert!(keys.contains(&key));
        let path = directory.join(format!("{}.key", fixture::support::hex_encode(&key)));
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let resolution = setup
            .identity
            .borrow_mut()
            .resolve_key(Contractd, &key)
            .unwrap();
        assert_eq!(resolution.scope, key_scope_t::normal);
        assert_eq!(resolution.owner_uid, 1001);
    }
    assert!(
        !directory.join("pending.reg").exists(),
        "the bundle is removed once registered"
    );
    let mut vault = setup.vault;
    assert_eq!(
        code(vault.create_user_keys(Alice, &passphrase(AlicePassphrase))),
        "ipc/duplicate_identifier"
    );
}

#[test]
fn an_exact_proposal_is_signed_once_after_digest_approval() {
    let mut setup = setup("signing");
    let payload = alice_genesis(&setup);
    let registered = setup
        .vault
        .register_proposal(Contractd, request("genesis", 1001, &payload))
        .expect("register");
    let bytes = encoded(&payload);
    // The client decodes the exact bytes and computes the digest itself.
    assert_eq!(
        registered.approval_digest,
        client_digest(&payload.key_id, &bytes)
    );
    let envelope = setup
        .vault
        .approve_and_sign(
            Alice,
            &registered.proposal_id,
            &registered.approval_digest,
            &passphrase(AlicePassphrase),
        )
        .expect("signed");
    let verified = verify_object(
        &envelope,
        object_kind_t::commit,
        &identity_resolver_t(&setup.identity),
    )
    .expect("verifies");
    assert_eq!(
        verified.payload_bytes, bytes,
        "the vault signed exactly the registered bytes"
    );
    let payload_t::commit(signed) = &verified.payload else {
        panic!("commit");
    };
    let resolution = setup
        .identity
        .borrow_mut()
        .resolve_key(Contractd, &signed.key_id)
        .unwrap();
    check_commit_authority(&resolution, signed).expect("layer 6");
    // One use only.
    assert_eq!(
        code(setup.vault.approve_and_sign(
            Alice,
            &registered.proposal_id,
            &registered.approval_digest,
            &passphrase(AlicePassphrase)
        )),
        "ipc/proposal_used"
    );
}

#[test]
fn approval_must_match_the_registered_proposal() {
    let mut setup = setup("approval");
    let payload = alice_genesis(&setup);
    let registered = setup
        .vault
        .register_proposal(Contractd, request("approval", 1001, &payload))
        .unwrap();
    let secret = passphrase(AlicePassphrase);
    let mut forged = registered.approval_digest;
    forged[0] ^= 1;
    assert_eq!(
        code(
            setup
                .vault
                .approve_and_sign(Alice, &registered.proposal_id, &forged, &secret)
        ),
        "ipc/approval_mismatch"
    );
    // A digest of other bytes (another head) is not this proposal's either.
    let mut other = payload.clone();
    other.message = "a different message".to_owned();
    let other_digest = client_digest(&other.key_id, &encoded(&other));
    assert_eq!(
        code(
            setup
                .vault
                .approve_and_sign(Alice, &registered.proposal_id, &other_digest, &secret)
        ),
        "ipc/approval_mismatch"
    );
    assert_eq!(
        code(setup.vault.approve_and_sign(
            Bob,
            &registered.proposal_id,
            &registered.approval_digest,
            &secret
        )),
        "ipc/owner_mismatch"
    );
    assert_eq!(
        code(setup.vault.approve_and_sign(
            Alice,
            &proposal_id("unknown"),
            &registered.approval_digest,
            &secret
        )),
        "ipc/proposal_unknown"
    );
    for caller in [
        caller_role_t::admin,
        caller_role_t::fixture_consumer,
        Contractd,
        caller_role_t::service(service_name_t::vaultd),
    ] {
        assert_eq!(
            code(setup.vault.approve_and_sign(
                caller,
                &registered.proposal_id,
                &registered.approval_digest,
                &secret
            )),
            "ipc/role_denied"
        );
    }
    // Bob cannot cancel Alice's proposal; Alice can, and it is then gone.
    assert_eq!(
        code(setup.vault.cancel_proposal(Bob, &registered.proposal_id)),
        "ipc/owner_mismatch"
    );
    setup
        .vault
        .cancel_proposal(Alice, &registered.proposal_id)
        .unwrap();
    assert_eq!(
        code(setup.vault.approve_and_sign(
            Alice,
            &registered.proposal_id,
            &registered.approval_digest,
            &secret
        )),
        "ipc/proposal_unknown"
    );
}

#[test]
fn proposals_expire_by_either_clock() {
    let mut setup = setup("expiry");
    let payload = alice_genesis(&setup);
    let secret = passphrase(AlicePassphrase);
    for (label, monotonic, utc) in [("monotonic", 120, 0), ("utc", 0, 120)] {
        let registered = setup
            .vault
            .register_proposal(Contractd, request(label, 1001, &payload))
            .unwrap();
        let start = *setup.vault.clock_mut();
        setup.vault.clock_mut().monotonic += monotonic;
        setup.vault.clock_mut().utc += utc;
        assert_eq!(
            code(setup.vault.approve_and_sign(
                Alice,
                &registered.proposal_id,
                &registered.approval_digest,
                &secret
            )),
            "ipc/proposal_expired",
            "{label}"
        );
        *setup.vault.clock_mut() = start;
    }
    let mut late = request("late", 1001, &payload);
    late.expires_in_seconds = 121;
    assert_eq!(
        code(setup.vault.register_proposal(Contractd, late)),
        "ipc/malformed_request"
    );
}

#[test]
fn proposal_ids_stay_single_use_for_their_lifetime() {
    // A used or cancelled id is kept, without its bytes, until the proposal's lifetime
    // ends: neither a later registration's cleanup nor a cancellation makes it
    // registrable or signable again (M2-004 review R04).
    let mut setup = setup("single_use");
    let payload = alice_genesis(&setup);
    let secret = passphrase(AlicePassphrase);
    let register = |vault: &mut vault_t, label: &str| {
        vault.register_proposal(Contractd, request(label, 1001, &payload))
    };
    let a = register(&mut setup.vault, "a").unwrap();
    setup
        .vault
        .approve_and_sign(Alice, &a.proposal_id, &a.approval_digest, &secret)
        .expect("a signed once");
    // Interleaved: b registered (its cleanup runs), c cancelled by the owner, d by
    // contractd, then b approved.
    let b = register(&mut setup.vault, "b").unwrap();
    let c = register(&mut setup.vault, "c").unwrap();
    setup.vault.cancel_proposal(Alice, &c.proposal_id).unwrap();
    let d = register(&mut setup.vault, "d").unwrap();
    setup
        .vault
        .cancel_proposal(Contractd, &d.proposal_id)
        .unwrap();
    // A used marker cannot be cancelled away; a cancelled one cannot be cancelled again.
    assert_eq!(
        code(setup.vault.cancel_proposal(Contractd, &a.proposal_id)),
        "ipc/proposal_used"
    );
    assert_eq!(
        code(setup.vault.cancel_proposal(Alice, &c.proposal_id)),
        "ipc/proposal_unknown"
    );
    setup
        .vault
        .approve_and_sign(Alice, &b.proposal_id, &b.approval_digest, &secret)
        .expect("b signed once");
    for (label, approval) in [
        ("a", "ipc/proposal_used"),
        ("b", "ipc/proposal_used"),
        ("c", "ipc/proposal_unknown"),
        ("d", "ipc/proposal_unknown"),
    ] {
        assert_eq!(
            code(register(&mut setup.vault, label)),
            "ipc/duplicate_identifier",
            "{label}"
        );
        // The digest is the same for every id here (same bytes).
        assert_eq!(
            code(setup.vault.approve_and_sign(
                Alice,
                &proposal_id(label),
                &a.approval_digest,
                &secret
            )),
            approval,
            "{label}"
        );
    }
    // Retention is bounded: the kept ids count against their own per-user limit...
    for index in 4..DefaultMaxRetiredProposalsPerUser {
        let filler = register(&mut setup.vault, &format!("filler {index}")).unwrap();
        setup
            .vault
            .cancel_proposal(Contractd, &filler.proposal_id)
            .unwrap();
    }
    assert_eq!(
        code(register(&mut setup.vault, "over the limit")),
        "ipc/quota_exhausted"
    );
    // ... and end with the proposals' lifetime.
    setup.vault.clock_mut().monotonic += 120;
    register(&mut setup.vault, "after the lifetime").expect("a fresh id registers");
}

#[test]
fn a_live_proposal_does_not_extend_the_signing_keys_window() {
    // The key's certificate window is checked at the current time, at registration
    // and again immediately before the signature, apart from layer 6's check of the
    // payload timestamp (M2-004 review R02).
    let mut setup = setup("window");
    let secret = passphrase(AlicePassphrase);
    let not_after = StartUtc + MaxTemporaryCertificateSeconds;
    let grant = commit_payload(
        "commit_temporary_session",
        1001,
        &setup.alice.user_id,
        &setup.alice.temporary_key_id,
        authority_class_t::temporary,
        "2026-10-20T07:59:55Z",
    );
    let approve = |vault: &mut vault_t,
                   registered: &cham3_vaultd::service::registered_proposal_t| {
        vault.approve_and_sign(
            Alice,
            &registered.proposal_id,
            &registered.approval_digest,
            &secret,
        )
    };
    // Registered five seconds before not_after with a timestamp inside the window,
    // approved one second after it: refused although the proposal is still live.
    setup.vault.clock_mut().utc = not_after - 5;
    let late = setup
        .vault
        .register_proposal(Contractd, request("late approval", 1001, &grant))
        .expect("inside the window");
    setup.vault.clock_mut().utc += 6;
    setup.vault.clock_mut().monotonic += 6;
    assert_eq!(
        code(approve(&mut setup.vault, &late)),
        "key_authority/certificate_window"
    );
    // Approved at not_after exactly (inclusive, like layer 6): signs.
    setup.vault.clock_mut().utc = not_after - 5;
    let boundary = setup
        .vault
        .register_proposal(Contractd, request("at the boundary", 1001, &grant))
        .unwrap();
    setup.vault.clock_mut().utc = not_after;
    let envelope = approve(&mut setup.vault, &boundary).expect("at not_after");
    verify_object(
        &envelope,
        object_kind_t::commit,
        &identity_resolver_t(&setup.identity),
    )
    .expect("verifies");
    // Registration after not_after, or before not_before, is refused even for a
    // payload timestamp inside the window.
    setup.vault.clock_mut().utc = not_after + 1;
    assert_eq!(
        code(
            setup
                .vault
                .register_proposal(Contractd, request("after", 1001, &grant))
        ),
        "key_authority/certificate_window"
    );
    setup.vault.clock_mut().utc = StartUtc - 1;
    assert_eq!(
        code(
            setup
                .vault
                .register_proposal(Contractd, request("before", 1001, &grant))
        ),
        "key_authority/certificate_window"
    );
    // The persistent key has no not_after.
    setup.vault.clock_mut().utc = StartUtc + 400 * 24 * 60 * 60;
    let genesis = alice_genesis(&setup);
    let persistent = setup
        .vault
        .register_proposal(Contractd, request("persistent", 1001, &genesis))
        .expect("no end");
    approve(&mut setup.vault, &persistent).expect("signs");
}

#[test]
fn register_proposal_checks_role_bytes_binding_and_key_authority() {
    let mut setup = setup("register");
    let bob = setup
        .vault
        .create_user_keys(Bob, &passphrase(BobPassphrase))
        .unwrap();
    let payload = alice_genesis(&setup);
    for caller in [
        Alice,
        caller_role_t::admin,
        caller_role_t::fixture_consumer,
        caller_role_t::service(service_name_t::policyd),
    ] {
        assert_eq!(
            code(
                setup
                    .vault
                    .register_proposal(caller, request("role", 1001, &payload))
            ),
            "ipc/role_denied"
        );
    }
    let mut wrong_kind = request("kind", 1001, &payload);
    wrong_kind.kind = object_kind_t::key_certificate;
    assert_eq!(
        code(setup.vault.register_proposal(Contractd, wrong_kind)),
        "ipc/malformed_request"
    );
    assert_eq!(
        code(
            setup
                .vault
                .register_proposal(Contractd, request("owner", 1002, &payload))
        ),
        "ipc/owner_mismatch"
    );
    let mut head = request("head", 1001, &payload);
    head.expected_head = Some([1; 32]);
    assert_eq!(
        code(setup.vault.register_proposal(Contractd, head)),
        "ipc/malformed_request"
    );
    let mut epoch = request("epoch", 1001, &payload);
    epoch.policy_epoch += 1;
    assert_eq!(
        code(setup.vault.register_proposal(Contractd, epoch)),
        "ipc/malformed_request"
    );
    let mut bytes = request("bytes", 1001, &payload);
    bytes.payload.push(0);
    assert_eq!(
        code(setup.vault.register_proposal(Contractd, bytes)),
        "payload_decode/trailing_bytes"
    );
    let mut large = request("large", 1001, &payload);
    large.payload = vec![0x40; 61_440];
    assert_eq!(
        code(setup.vault.register_proposal(Contractd, large)),
        "ipc/object_too_large"
    );
    // Another user's key for Alice's commit, and Alice's commit as Bob.
    let foreign = commit_payload(
        "commit_genesis",
        1001,
        &setup.alice.user_id,
        &bob.persistent_key_id,
        authority_class_t::persistent,
        CommitTime,
    );
    assert_eq!(
        code(
            setup
                .vault
                .register_proposal(Contractd, request("foreign", 1001, &foreign))
        ),
        "key_authority/key_not_certified"
    );
    // The temporary key cannot sign persistent authority.
    let temporary = commit_payload(
        "commit_genesis",
        1001,
        &setup.alice.user_id,
        &setup.alice.temporary_key_id,
        authority_class_t::persistent,
        CommitTime,
    );
    assert_eq!(
        code(
            setup
                .vault
                .register_proposal(Contractd, request("temporary", 1001, &temporary))
        ),
        "key_authority/authority_class_denied"
    );
    // It can sign a temporary grant.
    let grant = commit_payload(
        "commit_temporary_session",
        1001,
        &setup.alice.user_id,
        &setup.alice.temporary_key_id,
        authority_class_t::temporary,
        CommitTime,
    );
    setup
        .vault
        .register_proposal(Contractd, request("grant", 1001, &grant))
        .expect("temporary grant");
    // Commits before the key's certificate window are refused.
    let early = commit_payload(
        "commit_genesis",
        1001,
        &setup.alice.user_id,
        &setup.alice.persistent_key_id,
        authority_class_t::persistent,
        "2026-10-13T07:59:59Z",
    );
    assert_eq!(
        code(
            setup
                .vault
                .register_proposal(Contractd, request("early", 1001, &early))
        ),
        "key_authority/certificate_window"
    );
    setup
        .vault
        .register_proposal(Contractd, request("positive", 1001, &payload))
        .expect("positive control");
    assert_eq!(
        code(
            setup
                .vault
                .register_proposal(Contractd, request("positive", 1001, &payload))
        ),
        "ipc/duplicate_identifier"
    );
    for index in 0..6 {
        setup
            .vault
            .register_proposal(Contractd, request(&format!("fill {index}"), 1001, &payload))
            .expect("capacity");
    }
    assert_eq!(
        code(
            setup
                .vault
                .register_proposal(Contractd, request("over", 1001, &payload))
        ),
        "ipc/quota_exhausted"
    );
}

#[test]
fn pending_proposals_count_against_the_retained_id_bounds() {
    // M2-004 revision-2 review, M2-004-R2-01: a pending proposal becomes a retained
    // used or cancelled id without a further check, so registration counts pending
    // ids against the retained-id bound. With 31 ids retired (30 cancelled, 1 used),
    // exactly one more proposal registers; after its transition the user holds 32 ids
    // and nothing registers until a lifetime ends.
    let mut setup = setup("retained_bound");
    let payload = alice_genesis(&setup);
    let secret = passphrase(AlicePassphrase);
    let register = |vault: &mut vault_t, label: &str| {
        vault.register_proposal(Contractd, request(label, 1001, &payload))
    };
    for index in 0..30 {
        let cancelled = register(&mut setup.vault, &format!("cancelled {index}")).unwrap();
        setup
            .vault
            .cancel_proposal(Alice, &cancelled.proposal_id)
            .unwrap();
    }
    let used = register(&mut setup.vault, "used").unwrap();
    setup
        .vault
        .approve_and_sign(Alice, &used.proposal_id, &used.approval_digest, &secret)
        .expect("signed once");
    let last = register(&mut setup.vault, "last pending").expect("the 32nd id");
    for label in ["one over", "two over"] {
        assert_eq!(
            code(register(&mut setup.vault, label)),
            "ipc/quota_exhausted"
        );
    }
    setup
        .vault
        .cancel_proposal(Alice, &last.proposal_id)
        .unwrap();
    assert_eq!(
        code(register(&mut setup.vault, "after the transition")),
        "ipc/quota_exhausted"
    );
    // Approval of a pending id is a transition too: refill after the lifetime, then
    // approve the one pending id and check that nothing else fits.
    setup.vault.clock_mut().monotonic += 120;
    for index in 0..31 {
        let cancelled = register(&mut setup.vault, &format!("second round {index}")).unwrap();
        setup
            .vault
            .cancel_proposal(Contractd, &cancelled.proposal_id)
            .unwrap();
    }
    let pending = register(&mut setup.vault, "second round pending").unwrap();
    assert_eq!(
        code(register(&mut setup.vault, "second round over")),
        "ipc/quota_exhausted"
    );
    setup
        .vault
        .approve_and_sign(
            Alice,
            &pending.proposal_id,
            &pending.approval_digest,
            &secret,
        )
        .expect("signed once");
    assert_eq!(
        code(register(&mut setup.vault, "second round after")),
        "ipc/quota_exhausted"
    );
    setup.vault.clock_mut().utc += 120;
    register(&mut setup.vault, "after the second lifetime").expect("a fresh id registers");
}

#[test]
fn the_global_retained_id_bound_counts_pending_proposals_of_every_user() {
    // M2-004-R2-01, global bound: nine users hold 255 retired ids together; pending
    // registrations across users fill the single remaining slot, and the transitions
    // of that slot leave 256 retained ids with nothing else registrable.
    let root = temp_root("retained_global");
    let identity = identity_service();
    let mut vault = vault(&root, &identity);
    let mut users = Vec::new();
    let mut retained = 0usize;
    for uid in 1001..=1009u32 {
        let caller = caller_role_t::trusted_client { uid };
        let keys = vault
            .create_user_keys(caller, &passphrase(AlicePassphrase))
            .expect("enrolls");
        let payload = commit_payload(
            "commit_genesis",
            uid,
            &keys.user_id,
            &keys.persistent_key_id,
            authority_class_t::persistent,
            CommitTime,
        );
        let count = if uid == 1009 { 7 } else { 31 };
        for index in 0..count {
            let registered = vault
                .register_proposal(
                    Contractd,
                    request(&format!("{uid} retired {index}"), uid, &payload),
                )
                .unwrap();
            vault
                .cancel_proposal(Contractd, &registered.proposal_id)
                .unwrap();
            retained += 1;
        }
        users.push((uid, payload));
    }
    assert_eq!(retained, 255);
    let mut pending = Vec::new();
    for (uid, payload) in users.iter().take(8) {
        for index in 0..8 {
            if let Ok(registered) = vault.register_proposal(
                Contractd,
                request(&format!("{uid} pending {index}"), *uid, payload),
            ) {
                pending.push(registered);
            }
        }
    }
    assert_eq!(pending.len(), 1, "exactly one slot was free");
    for registered in &pending {
        vault
            .cancel_proposal(Contractd, &registered.proposal_id)
            .unwrap();
    }
    let (uid, payload) = &users[8];
    assert_eq!(
        code(vault.register_proposal(Contractd, request("global over", *uid, payload))),
        "ipc/quota_exhausted"
    );
    vault.clock_mut().monotonic += 120;
    vault
        .register_proposal(Contractd, request("global after", *uid, payload))
        .expect("a fresh id registers after the lifetime");
}

#[test]
fn failed_passphrases_are_limited_and_the_limit_survives_restart() {
    let mut setup = setup("attempts");
    let payload = alice_genesis(&setup);
    let registered = setup
        .vault
        .register_proposal(Contractd, request("attempts", 1001, &payload))
        .unwrap();
    let wrong = passphrase("not the passphrase");
    for _ in 0..5 {
        assert_eq!(
            code(setup.vault.approve_and_sign(
                Alice,
                &registered.proposal_id,
                &registered.approval_digest,
                &wrong
            )),
            "ipc/authentication_failed"
        );
    }
    // Five failures: the next attempt waits one second, even with the right passphrase.
    let right = passphrase(AlicePassphrase);
    assert_eq!(
        code(setup.vault.approve_and_sign(
            Alice,
            &registered.proposal_id,
            &registered.approval_digest,
            &right
        )),
        "ipc/attempts_exhausted"
    );
    // A restarted vault reads the same counter.
    let mut restarted = vault(&setup.root, &setup.identity);
    let again = restarted
        .register_proposal(Contractd, request("restart", 1001, &payload))
        .unwrap();
    assert_eq!(
        code(restarted.approve_and_sign(Alice, &again.proposal_id, &again.approval_digest, &right)),
        "ipc/attempts_exhausted"
    );
    restarted.clock_mut().utc += 1;
    restarted
        .approve_and_sign(Alice, &again.proposal_id, &again.approval_digest, &right)
        .expect("after the delay");
    assert!(
        !setup.root.join("1001").join("attempts.rec").exists(),
        "success resets the counter"
    );
}

fn key_path(setup: &setup_t, key: &digest_t) -> std::path::PathBuf {
    setup
        .root
        .join("1001")
        .join(format!("{}.key", fixture::support::hex_encode(key)))
}

#[test]
fn tampered_or_substituted_key_objects_never_sign() {
    let mut setup = setup("tamper");
    let payload = alice_genesis(&setup);
    let path = key_path(&setup, &setup.alice.persistent_key_id);
    let original = std::fs::read(&path).unwrap();
    let object = key_object_t::decode(&original).unwrap();
    let secret = passphrase(AlicePassphrase);
    let restore = |bytes: &[u8]| std::fs::write(&path, bytes).unwrap();
    let mut cases: Vec<(&str, key_object_t, &str)> = Vec::new();
    let mut flipped = object.clone();
    flipped.ciphertext[0] ^= 1;
    cases.push(("ciphertext", flipped, "ipc/authentication_failed"));
    let mut tag = object.clone();
    tag.tag[0] ^= 1;
    cases.push(("tag", tag, "ipc/authentication_failed"));
    let mut nonce = object.clone();
    nonce.metadata.nonce[0] ^= 1;
    cases.push(("nonce", nonce, "ipc/authentication_failed"));
    let mut salt = object.clone();
    salt.metadata.salt[0] ^= 1;
    cases.push(("salt", salt, "ipc/authentication_failed"));
    let mut created = object.clone();
    created.metadata.created = "2026-10-13T08:00:01Z".to_owned();
    cases.push((
        "created (authenticated metadata)",
        created,
        "ipc/authentication_failed",
    ));
    let mut passes = object.clone();
    passes.metadata.kdf.passes = 2;
    cases.push((
        "kdf passes (authenticated metadata)",
        passes,
        "ipc/authentication_failed",
    ));
    let mut class = object.clone();
    class.metadata.authority_class = cham3_identityd::index::key_class_t::temporary;
    cases.push(("authority class", class, "ipc/storage_unavailable"));
    let mut weak = object.clone();
    weak.metadata.kdf.memory_kib = 1024;
    cases.push((
        "kdf below the bounds (checked before derivation)",
        weak,
        "ipc/storage_unavailable",
    ));
    let mut costly = object.clone();
    costly.metadata.kdf.memory_kib = 1_048_576;
    cases.push((
        "kdf above the locked budget",
        costly,
        "ipc/protection_unavailable",
    ));
    let mut owner = object.clone();
    owner.metadata.owner_uid = 1002;
    cases.push(("owner uid", owner, "ipc/storage_unavailable"));
    // Each case must end without a signature, with the expected code, at
    // registration (object checks) or at approval (derivation and decryption).
    let never_signs = |label: &str, vault: &mut vault_t, expected: &str| {
        let outcome = vault
            .register_proposal(
                Contractd,
                request(&format!("tamper {label}"), 1001, &payload),
            )
            .and_then(|registered| {
                let signed = vault.approve_and_sign(
                    Alice,
                    &registered.proposal_id,
                    &registered.approval_digest,
                    &secret,
                );
                vault
                    .cancel_proposal(Contractd, &registered.proposal_id)
                    .ok();
                signed
            });
        assert_eq!(code(outcome), expected, "{label}");
    };
    for (label, tampered, expected) in cases {
        restore(&tampered.encode());
        never_signs(label, &mut setup.vault, expected);
        // Keep the attempt limit out of the way of the next case.
        std::fs::remove_file(setup.root.join("1001").join("attempts.rec")).ok();
    }
    // Substitution: a valid object of another key, encrypted under the same
    // passphrase, renamed to the persistent key's file.
    let temporary = std::fs::read(key_path(&setup, &setup.alice.temporary_key_id)).unwrap();
    restore(&temporary);
    never_signs("renamed", &mut setup.vault, "ipc/storage_unavailable");
    // Substitution with consistent metadata: the persistent key's metadata over the
    // temporary key's seed (sealed with the right KEK): the recomputed key id differs.
    let temporary_object = key_object_t::decode(&temporary).unwrap();
    let kek = derive_kek(
        &kdf_params_t::Minimum,
        u64::MAX,
        AlicePassphrase.as_bytes(),
        &temporary_object.metadata.salt,
    )
    .unwrap();
    let temporary_seed = temporary_object.open(&kek).unwrap();
    let persistent_kek = derive_kek(
        &kdf_params_t::Minimum,
        u64::MAX,
        AlicePassphrase.as_bytes(),
        &object.metadata.salt,
    )
    .unwrap();
    let substituted =
        key_object_t::seal(object.metadata.clone(), &persistent_kek, &temporary_seed).unwrap();
    restore(&substituted.encode());
    never_signs("substituted", &mut setup.vault, "ipc/storage_unavailable");
    // Unsafe file modes and symbolic links are refused before reading.
    restore(&original);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    never_signs("mode", &mut setup.vault, "ipc/storage_unavailable");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let aside = setup.root.join("aside.key");
    std::fs::rename(&path, &aside).unwrap();
    std::os::unix::fs::symlink(&aside, &path).unwrap();
    never_signs("symlink", &mut setup.vault, "ipc/storage_unavailable");
    std::fs::remove_file(&path).unwrap();
    std::fs::rename(&aside, &path).unwrap();
    // Positive control: the original object signs.
    let registered = setup
        .vault
        .register_proposal(Contractd, request("original", 1001, &payload))
        .unwrap();
    setup
        .vault
        .approve_and_sign(
            Alice,
            &registered.proposal_id,
            &registered.approval_digest,
            &secret,
        )
        .expect("original object");
}

#[test]
fn key_files_hold_neither_the_passphrase_nor_a_seed() {
    let setup = setup("plaintext");
    let mut seeds = Vec::new();
    for key in [
        setup.alice.user_id,
        setup.alice.persistent_key_id,
        setup.alice.temporary_key_id,
    ] {
        let object = key_object_t::decode(&std::fs::read(key_path(&setup, &key)).unwrap()).unwrap();
        let kek = derive_kek(
            &kdf_params_t::Minimum,
            u64::MAX,
            AlicePassphrase.as_bytes(),
            &object.metadata.salt,
        )
        .unwrap();
        seeds.push(object.open(&kek).unwrap().as_slice().to_vec());
    }
    let mut files = 0;
    for entry in std::fs::read_dir(setup.root.join("1001")).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        files += 1;
        for secret in seeds
            .iter()
            .map(Vec::as_slice)
            .chain([AlicePassphrase.as_bytes()])
        {
            assert!(!bytes.windows(secret.len()).any(|window| window == secret));
        }
    }
    assert_eq!(files, 3);
}

#[test]
fn invalidated_keys_never_sign_even_from_a_restored_object() {
    let mut setup = setup("invalidated");
    let payload = alice_genesis(&setup);
    let path = key_path(&setup, &setup.alice.persistent_key_id);
    let saved = std::fs::read(&path).unwrap();
    let pending = setup
        .vault
        .register_proposal(Contractd, request("before", 1001, &payload))
        .unwrap();
    let secret = passphrase(AlicePassphrase);
    setup
        .vault
        .invalidate_key(
            Alice,
            &setup.alice.persistent_key_id,
            invalidation_reason_t::compromised,
            &secret,
        )
        .expect("invalidate");
    assert!(!path.exists(), "the invalidated key's object is erased");
    // A proposal registered before the invalidation can no longer be signed.
    assert_eq!(
        code(setup.vault.approve_and_sign(
            Alice,
            &pending.proposal_id,
            &pending.approval_digest,
            &secret
        )),
        "key_authority/key_invalidated"
    );
    // Restoring the old encrypted object confers nothing.
    std::fs::write(&path, &saved).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        code(
            setup
                .vault
                .register_proposal(Contractd, request("after", 1001, &payload))
        ),
        "key_authority/key_invalidated"
    );
    // Bob cannot invalidate Alice's keys.
    let _bob = setup
        .vault
        .create_user_keys(Bob, &passphrase(BobPassphrase))
        .unwrap();
    assert_eq!(
        code(setup.vault.invalidate_key(
            Bob,
            &setup.alice.temporary_key_id,
            invalidation_reason_t::retired,
            &passphrase(BobPassphrase)
        )),
        "key_authority/key_not_certified"
    );
    assert_eq!(
        code(setup.vault.invalidate_key(
            Alice,
            &setup.alice.user_id,
            invalidation_reason_t::retired,
            &secret
        )),
        "key_authority/key_not_certified"
    );
}

/// A compromise invalidation of `target` signed by Alice's identity key, as another
/// vaultd operation would register it (test only: the identity seed is decrypted with
/// the synthetic passphrase).
fn identity_signed_invalidation(
    root: &std::path::Path,
    user_id: &digest_t,
    target: &digest_t,
) -> Vec<u8> {
    let path = root
        .join("1001")
        .join(format!("{}.key", fixture::support::hex_encode(user_id)));
    let object = key_object_t::decode(&std::fs::read(path).unwrap()).unwrap();
    let kek = derive_kek(
        &kdf_params_t::Minimum,
        u64::MAX,
        AlicePassphrase.as_bytes(),
        &object.metadata.salt,
    )
    .unwrap();
    let seed = object.open(&kek).unwrap();
    let signing = SigningKey::from_bytes(<&[u8; 32]>::try_from(seed.as_slice()).unwrap());
    let payload = encode(
        &key_invalidation_t {
            invalidation_id: proposal_id("interleaved invalidation"),
            issued: "2026-10-13T08:00:00Z".to_owned(),
            owner_user_id: *user_id,
            target_key_id: *target,
            reason: invalidation_reason_t::compromised,
        }
        .to_value(),
    );
    let protected = protected_header_bytes(user_id, object_kind_t::key_invalidation);
    let message = to_be_signed(object_kind_t::key_invalidation, &protected, &payload);
    assemble_envelope(&protected, &payload, &signing.sign(&message).to_bytes())
}

#[test]
fn authority_changes_during_the_derivation_prevent_the_signature() {
    // Key status, scope and window are resolved again after the KEK derivation,
    // immediately before the signature. Each change lands right after the approval's
    // first `resolve_key` answer, i.e. after the check before the derivation
    // (M2-004 review R03).
    let secret = passphrase(AlicePassphrase);
    for (case, expected) in [
        ("invalidation", Some("key_authority/key_invalidated")),
        ("deletion", Some("key_authority/user_deleted")),
        ("containment", Some("key_authority/contained")),
        ("no change", None),
    ] {
        let root = temp_root(&format!("interleaved {case}"));
        let identity = identity_service();
        let mut vault = interleaved_vault(&root, &identity);
        let alice = vault.create_user_keys(Alice, &secret).unwrap();
        let payload = commit_payload(
            "commit_genesis",
            1001,
            &alice.user_id,
            &alice.persistent_key_id,
            authority_class_t::persistent,
            CommitTime,
        );
        let registered = vault
            .register_proposal(Contractd, request(case, 1001, &payload))
            .unwrap();
        let shared = identity.clone();
        match case {
            "invalidation" => {
                let object =
                    identity_signed_invalidation(&root, &alice.user_id, &alice.persistent_key_id);
                vault.identity_mut().arm(1, move || {
                    shared
                        .borrow_mut()
                        .register_invalidation(
                            caller_role_t::service(service_name_t::vaultd),
                            1001,
                            &object,
                        )
                        .expect("invalidation");
                });
            }
            "deletion" => vault.identity_mut().arm(1, move || {
                push_record(&shared, 1001, user_record_t::deletion_pending);
            }),
            "containment" => vault.identity_mut().arm(1, move || {
                push_record(&shared, 1001, user_record_t::containment);
            }),
            _ => {}
        }
        let resolves = vault.identity_mut().resolves;
        let outcome = vault.approve_and_sign(
            Alice,
            &registered.proposal_id,
            &registered.approval_digest,
            &secret,
        );
        match expected {
            Some(expected) => {
                assert_eq!(code(outcome), expected, "{case}");
                assert!(
                    vault.identity_mut().change.is_none(),
                    "{case}: the change ran"
                );
            }
            None => {
                verify_object(
                    &outcome.expect("signed"),
                    object_kind_t::commit,
                    &identity_resolver_t(&identity),
                )
                .expect("verifies");
            }
        }
        assert_eq!(
            vault.identity_mut().resolves - resolves,
            2,
            "{case}: resolved before and after the derivation"
        );
    }
}

#[test]
fn rotation_certifies_a_new_key_and_retires_the_old_one() {
    let mut setup = setup("rotation");
    let secret = passphrase(AlicePassphrase);
    let old = setup.alice.persistent_key_id;
    let new = setup
        .vault
        .rotate_key(Alice, authority_class_t::persistent, &secret)
        .expect("rotate");
    assert_ne!(new, old);
    assert!(!key_path(&setup, &old).exists());
    let old_resolution = setup
        .identity
        .borrow_mut()
        .resolve_key(Contractd, &old)
        .unwrap();
    assert_eq!(
        old_resolution.invalidated,
        Some(invalidation_reason_t::rotated)
    );
    let identity = setup
        .identity
        .borrow_mut()
        .user_identity(Contractd, 1001)
        .unwrap();
    assert_eq!(identity.persistent_key_ids, vec![new]);
    let payload = commit_payload(
        "commit_genesis",
        1001,
        &setup.alice.user_id,
        &new,
        authority_class_t::persistent,
        CommitTime,
    );
    let registered = setup
        .vault
        .register_proposal(Contractd, request("rotated", 1001, &payload))
        .unwrap();
    setup
        .vault
        .approve_and_sign(
            Alice,
            &registered.proposal_id,
            &registered.approval_digest,
            &secret,
        )
        .expect("the new key signs");
    let old_payload = alice_genesis(&setup);
    assert_eq!(
        code(
            setup
                .vault
                .register_proposal(Contractd, request("old", 1001, &old_payload))
        ),
        "key_authority/key_invalidated"
    );
    // A wrong passphrase rotates nothing.
    assert_eq!(
        code(
            setup
                .vault
                .rotate_key(Alice, authority_class_t::temporary, &passphrase("wrong"))
        ),
        "ipc/authentication_failed"
    );
    assert_eq!(setup.vault.store().list_keys(1001).unwrap().len(), 3);
}

#[test]
fn interrupted_enrollment_resumes_with_the_held_keys() {
    let root = temp_root("resume");
    let identity = identity_service();
    let mut vault = vault(&root, &identity);
    identity.borrow_mut().journal_mut().faults.fail_appends = true;
    assert_eq!(
        code(vault.create_user_keys(Alice, &passphrase(AlicePassphrase))),
        "ipc/storage_unavailable"
    );
    let held = vault.store().list_keys(1001).unwrap();
    assert_eq!(held.len(), 3, "objects are durable before registration");
    assert!(root.join("1001").join("pending.reg").exists());
    identity.borrow_mut().journal_mut().faults.fail_appends = false;
    // A restarted vault resumes; the held keys need their own passphrase.
    let mut restarted = vault_with(&root, &identity, registry(&identity));
    assert_eq!(
        code(restarted.create_user_keys(Alice, &passphrase("another passphrase"))),
        "ipc/authentication_failed"
    );
    let created = restarted
        .create_user_keys(Alice, &passphrase(AlicePassphrase))
        .expect("resume");
    assert_eq!(
        restarted.store().list_keys(1001).unwrap(),
        held,
        "no new keys were generated"
    );
    assert!(held.contains(&created.persistent_key_id));
    assert!(!root.join("1001").join("pending.reg").exists());
    let resolution = identity
        .borrow_mut()
        .resolve_key(Contractd, &created.persistent_key_id)
        .unwrap();
    assert_eq!(resolution.scope, key_scope_t::normal);
}

#[test]
fn an_interrupted_invalidation_already_denies_and_completes_later() {
    let mut setup = setup("interrupted_invalidation");
    let secret = passphrase(AlicePassphrase);
    let payload = alice_genesis(&setup);
    setup
        .identity
        .borrow_mut()
        .journal_mut()
        .faults
        .fail_appends = true;
    assert_eq!(
        code(setup.vault.invalidate_key(
            Alice,
            &setup.alice.persistent_key_id,
            invalidation_reason_t::compromised,
            &secret
        )),
        "ipc/restriction_not_durable"
    );
    // Denied before it is durable.
    assert_eq!(
        code(
            setup
                .vault
                .register_proposal(Contractd, request("denied", 1001, &payload))
        ),
        "ipc/restriction_not_durable"
    );
    setup
        .identity
        .borrow_mut()
        .journal_mut()
        .faults
        .fail_appends = false;
    assert_eq!(
        code(
            setup
                .vault
                .register_proposal(Contractd, request("durable", 1001, &payload))
        ),
        "key_authority/key_invalidated"
    );
    assert!(!key_path(&setup, &setup.alice.persistent_key_id).exists());
    assert!(!setup.root.join("1001").join("pending.reg").exists());
}

#[test]
fn deleted_or_quarantined_users_cannot_sign() {
    let mut setup = setup("states");
    let secret = passphrase(AlicePassphrase);
    let payload = alice_genesis(&setup);
    let registered = setup
        .vault
        .register_proposal(Contractd, request("quarantine", 1001, &payload))
        .unwrap();
    push_record(&setup.identity, 1001, user_record_t::quarantine);
    assert_eq!(
        code(setup.vault.approve_and_sign(
            Alice,
            &registered.proposal_id,
            &registered.approval_digest,
            &secret
        )),
        "key_authority/quarantined"
    );
    assert_eq!(
        code(
            setup
                .vault
                .rotate_key(Alice, authority_class_t::persistent, &secret)
        ),
        "key_authority/quarantined"
    );
    push_record(&setup.identity, 1001, user_record_t::deletion_pending);
    assert_eq!(
        code(
            setup
                .vault
                .register_proposal(Contractd, request("deleted", 1001, &payload))
        ),
        "key_authority/user_deleted"
    );
    assert_eq!(
        code(setup.vault.invalidate_key(
            Alice,
            &setup.alice.temporary_key_id,
            invalidation_reason_t::retired,
            &secret
        )),
        "key_authority/user_deleted"
    );
}

#[test]
fn only_the_owners_trusted_client_runs_key_operations() {
    let mut setup = setup("roles");
    let secret = passphrase(AlicePassphrase);
    for caller in [
        caller_role_t::admin,
        caller_role_t::fixture_consumer,
        Contractd,
        caller_role_t::service(service_name_t::identityd),
    ] {
        assert_eq!(
            code(setup.vault.create_user_keys(caller, &secret)),
            "ipc/role_denied"
        );
        assert_eq!(
            code(
                setup
                    .vault
                    .rotate_key(caller, authority_class_t::persistent, &secret)
            ),
            "ipc/role_denied"
        );
        assert_eq!(
            code(setup.vault.invalidate_key(
                caller,
                &setup.alice.persistent_key_id,
                invalidation_reason_t::retired,
                &secret
            )),
            "ipc/role_denied"
        );
        assert_eq!(
            code(setup.vault.recover_keys(caller, &[1; 16], &secret)),
            "ipc/role_denied"
        );
    }
}
