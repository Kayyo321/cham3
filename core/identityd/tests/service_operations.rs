//! identityd operations over the labeled journal fixture: registration, roles, user
//! separation, restrictive invalidation, public-key validation, certificate windows,
//! fail-closed journal handling and the recovery scopes (A04, A05, A12 in-process).
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::as_conversions
)]

mod builders;
#[path = "../../contract_format/tests/support/mod.rs"]
mod support;

use builders::*;
use cham3_contract_format::hash::{digest_t, key_id};
use cham3_contract_format::schema::{authority_class_t, commit_payload_t, commit_type_t};
use cham3_contract_format::{object_kind_t, payload_t, verify_object};
use cham3_identityd::caller::{caller_role_t, service_name_t};
use cham3_identityd::index::{
    ceremony_mode_t, check_commit_authority, check_commit_scope, key_scope_t, naming_t,
    record_role_t, user_record_t, user_state_t,
};
use cham3_identityd::journal::{identity_journal_t, memory_journal_t};
use cham3_identityd::outcome::service_error_t;
use cham3_identityd::service::identity_service_t;
use ed25519_dalek::{Signer, SigningKey};

const Vaultd: caller_role_t = caller_role_t::service(service_name_t::vaultd);
const Contractd: caller_role_t = caller_role_t::service(service_name_t::contractd);

type service_t = identity_service_t<memory_journal_t>;

fn code(result: Result<impl std::fmt::Debug, service_error_t>) -> String {
    match result {
        Ok(value) => panic!("expected a refusal, got {value:?}"),
        Err(error) => error.to_string(),
    }
}

fn started() -> service_t {
    identity_service_t::start(memory_journal_t::new()).expect("start")
}

fn enrolled(users: &[&test_user_t]) -> service_t {
    let mut service = started();
    for user in users {
        service
            .register_enrollment(
                Vaultd,
                user.uid,
                &user.enrollment,
                &user.persistent_certificate,
                &user.temporary_certificate,
            )
            .expect("enrollment");
    }
    service
}

fn scope_of(service: &mut service_t, key: &digest_t) -> key_scope_t {
    service.resolve_key(Contractd, key).expect("resolve").scope
}

#[test]
fn enrollment_registers_three_records_and_resolves_every_key() {
    let user = test_user(1001);
    let mut service = started();
    let receipt = service
        .register_enrollment(
            Vaultd,
            1001,
            &user.enrollment,
            &user.persistent_certificate,
            &user.temporary_certificate,
        )
        .expect("enrollment");
    assert_eq!(
        (
            receipt.enrollment.journal_seq,
            receipt.persistent.journal_seq,
            receipt.temporary.journal_seq
        ),
        (1, 2, 3)
    );
    assert_eq!(service.journal().record_count(1001), 3);
    for key in [&user.identity, &user.persistent, &user.temporary] {
        let resolution = service
            .resolve_key(Contractd, &key.key_id)
            .expect("resolve");
        assert_eq!(resolution.public_key, key.public_key);
        assert_eq!(
            (resolution.owner_uid, resolution.owner_user_id),
            (1001, user.identity.key_id)
        );
        assert_eq!(resolution.scope, key_scope_t::normal);
        assert_eq!(resolution.journal_seq, 3);
    }
    let identity = service
        .user_identity(caller_role_t::trusted_client { uid: 1001 }, 1001)
        .expect("identity");
    assert_eq!(identity.user_id, user.identity.key_id);
    assert_eq!(identity.persistent_key_ids, vec![user.persistent.key_id]);
    assert_eq!(identity.temporary_key_ids, vec![user.temporary.key_id]);
    assert_eq!(identity.state, user_state_t::normal);

    // A vaultd retry of the identical enrollment appends nothing.
    let retry = service
        .register_enrollment(
            Vaultd,
            1001,
            &user.enrollment,
            &user.persistent_certificate,
            &user.temporary_certificate,
        )
        .expect("retry");
    assert!(
        retry.enrollment.already_present
            && retry.persistent.already_present
            && retry.temporary.already_present
    );
    assert_eq!(service.journal().record_count(1001), 3);
}

#[test]
fn only_vaultd_registers_identity_objects() {
    let user = test_user(1001);
    let mut service = started();
    let others = [
        caller_role_t::service(service_name_t::identityd),
        caller_role_t::service(service_name_t::contractd),
        caller_role_t::service(service_name_t::policyd),
        caller_role_t::service(service_name_t::registryd),
        caller_role_t::trusted_client { uid: 1001 },
        caller_role_t::admin,
        caller_role_t::fixture_consumer,
    ];
    for caller in others {
        assert_eq!(
            code(service.register_enrollment(
                caller,
                1001,
                &user.enrollment,
                &user.persistent_certificate,
                &user.temporary_certificate
            )),
            "ipc/role_denied"
        );
        assert_eq!(
            code(service.register_certificate(caller, 1001, &user.persistent_certificate)),
            "ipc/role_denied"
        );
        assert_eq!(
            code(service.register_invalidation(caller, 1001, &user.persistent_certificate)),
            "ipc/role_denied"
        );
    }
    assert_eq!(service.journal().record_count(1001), 0);
}

#[test]
fn enrollment_is_bound_to_its_uid_and_happens_once() {
    let user = test_user(1001);
    let other = test_user(1002);
    let mut service = enrolled(&[&user]);
    // The enrollment names UID 1001; registering it for 1002 is refused.
    assert_eq!(
        code(service.register_enrollment(
            Vaultd,
            1002,
            &user.enrollment,
            &user.persistent_certificate,
            &user.temporary_certificate
        )),
        "ipc/owner_mismatch"
    );
    // A second identity for UID 1001 is refused (identity replacement is admin-only).
    let second = test_key("user 1001 second identity");
    let second_enrollment = enrollment(&second, 1001);
    let second_persistent = certificate(
        &second,
        1001,
        &test_key("second p"),
        authority_class_t::persistent,
        1,
    );
    let second_temporary = certificate(
        &second,
        1001,
        &test_key("second t"),
        authority_class_t::temporary,
        1,
    );
    assert_eq!(
        code(service.register_enrollment(
            Vaultd,
            1001,
            &second_enrollment,
            &second_persistent,
            &second_temporary
        )),
        "ipc/duplicate_identifier"
    );
    // Another user's certificates cannot complete an enrollment.
    assert_eq!(
        code(service.register_enrollment(
            Vaultd,
            1002,
            &other.enrollment,
            &user.persistent_certificate,
            &other.temporary_certificate
        )),
        "key_authority/key_not_certified"
    );
    // The slots are typed: a temporary certificate is not a persistent one.
    assert_eq!(
        code(service.register_enrollment(
            Vaultd,
            1002,
            &other.enrollment,
            &other.temporary_certificate,
            &other.persistent_certificate
        )),
        "key_authority/authority_class_denied"
    );
    assert_eq!(
        service.journal().record_count(1002),
        0,
        "nothing of a refused enrollment is persisted"
    );
    service
        .register_enrollment(
            Vaultd,
            1002,
            &other.enrollment,
            &other.persistent_certificate,
            &other.temporary_certificate,
        )
        .expect("positive control");
}

#[test]
fn certified_keys_must_be_canonical_points_of_large_order() {
    let user = test_user(1001);
    let mut service = enrolled(&[&user]);
    for (index, point) in BadPoints.iter().enumerate() {
        let mut payload = certificate_payload(
            &user.identity,
            1001,
            &test_key(&format!("bad {index}")),
            authority_class_t::persistent,
            1,
        );
        payload.subject_public_key = *point;
        payload.subject_key_id = key_id(point);
        let object = sign_certificate(&user.identity, &payload);
        assert_eq!(
            code(service.register_certificate(Vaultd, 1001, &object)),
            "key_authority/key_not_certified",
            "bad point {index}"
        );
    }
    // An enrollment whose own key is a bad point cannot be verified at all.
    let mut bad_enrollment = enrollment_payload(&user.identity, 1003);
    bad_enrollment.public_key = BadPoints[0];
    bad_enrollment.user_id = key_id(&BadPoints[0]);
    let object = sign_object(
        object_kind_t::identity_enrollment,
        &user.identity,
        &bad_enrollment.to_value(),
    );
    assert!(
        service
            .register_enrollment(Vaultd, 1003, &object, &object, &object)
            .is_err()
    );
    // Positive control: a valid point is certified.
    let good = test_key("good rotation key");
    service
        .register_certificate(
            Vaultd,
            1001,
            &certificate(
                &user.identity,
                1001,
                &good,
                authority_class_t::persistent,
                1,
            ),
        )
        .expect("valid point");
}

#[test]
fn temporary_certificates_need_a_window_of_at_most_seven_days() {
    let user = test_user(1001);
    let mut service = enrolled(&[&user]);
    let make = |label: &str, not_after: Option<&str>| {
        let mut payload = certificate_payload(
            &user.identity,
            1001,
            &test_key(label),
            authority_class_t::temporary,
            1,
        );
        payload.not_after = not_after.map(str::to_owned);
        sign_certificate(&user.identity, &payload)
    };
    for (label, not_after) in [
        ("no end", None),
        ("eight days", Some(WeekAndASecond)),
        ("empty", Some(T0)),
    ] {
        assert_eq!(
            code(service.register_certificate(Vaultd, 1001, &make(label, not_after))),
            "key_authority/certificate_window",
            "{label}"
        );
    }
    service
        .register_certificate(Vaultd, 1001, &make("seven days", Some(Week)))
        .expect("exactly seven days");
}

#[test]
fn rotation_must_name_an_earlier_key_of_the_same_class_at_a_current_epoch() {
    let user = test_user(1001);
    let mut service = enrolled(&[&user]);
    let rotate = |label: &str, supersedes: digest_t, epoch: u64| {
        let mut payload = certificate_payload(
            &user.identity,
            1001,
            &test_key(label),
            authority_class_t::persistent,
            epoch,
        );
        payload.supersedes_key_id = Some(supersedes);
        sign_certificate(&user.identity, &payload)
    };
    assert_eq!(
        code(service.register_certificate(Vaultd, 1001, &rotate("unknown", [7; 32], 1))),
        "key_authority/key_not_certified"
    );
    assert_eq!(
        code(service.register_certificate(
            Vaultd,
            1001,
            &rotate("other class", user.temporary.key_id, 1)
        )),
        "key_authority/key_not_certified"
    );
    assert_eq!(
        code(service.register_certificate(
            Vaultd,
            1001,
            &rotate("epoch zero", user.persistent.key_id, 0)
        )),
        "key_authority/key_epoch_stale"
    );
    // A key already registered (to any user) cannot be certified again.
    let other = test_user(1002);
    service
        .register_enrollment(
            Vaultd,
            1002,
            &other.enrollment,
            &other.persistent_certificate,
            &other.temporary_certificate,
        )
        .expect("other user");
    let mut stolen = certificate_payload(
        &user.identity,
        1001,
        &other.persistent,
        authority_class_t::persistent,
        1,
    );
    stolen.supersedes_key_id = Some(user.persistent.key_id);
    assert_eq!(
        code(service.register_certificate(
            Vaultd,
            1001,
            &sign_certificate(&user.identity, &stolen)
        )),
        "key_authority/key_not_certified"
    );
    // A certificate signed by another user's identity key is not this user's.
    let foreign = certificate(
        &other.identity,
        1002,
        &test_key("foreign"),
        authority_class_t::persistent,
        1,
    );
    assert_eq!(
        code(service.register_certificate(Vaultd, 1001, &foreign)),
        "key_authority/key_not_certified"
    );
    service
        .register_certificate(Vaultd, 1001, &rotate("rotation", user.persistent.key_id, 1))
        .expect("positive control");
}

#[test]
fn invalidation_denies_in_memory_before_it_is_durable() {
    let user = test_user(1001);
    let mut service = enrolled(&[&user]);
    let object = invalidation(
        &user.identity,
        &user.persistent.key_id,
        reason_of("compromised"),
        "p",
    );
    service.journal_mut().faults.fail_appends = true;
    assert_eq!(
        code(service.register_invalidation(Vaultd, 1001, &object)),
        "ipc/restriction_not_durable"
    );
    assert_eq!(service.journal().record_count(1001), 3, "not durable");
    let held = service
        .resolve_key(Contractd, &user.persistent.key_id)
        .expect("resolve");
    assert_eq!(
        held.scope,
        key_scope_t::denied(cham3_identityd::outcome::authority_code_t::key_invalidated)
    );
    assert!(
        service
            .user_identity(Vaultd, 1001)
            .expect("identity")
            .persistent_key_ids
            .is_empty()
    );
    // A journal that cannot even be read also keeps the denial.
    service.journal_mut().faults.unavailable = true;
    assert_eq!(
        code(service.register_invalidation(Vaultd, 1001, &object)),
        "ipc/restriction_not_durable"
    );
    service.journal_mut().faults = Default::default();
    let receipt = service
        .register_invalidation(Vaultd, 1001, &object)
        .expect("retry");
    assert!(!receipt.already_present);
    assert_eq!(service.journal().record_count(1001), 4);
    // Durable now: a restarted identityd rebuilds the denial from the journal alone.
    let journal = service.journal().clone();
    let mut restarted = identity_service_t::start(journal).expect("restart");
    assert_eq!(
        scope_of(&mut restarted, &user.persistent.key_id),
        key_scope_t::denied(cham3_identityd::outcome::authority_code_t::key_invalidated)
    );
    assert_eq!(
        scope_of(&mut restarted, &user.temporary.key_id),
        key_scope_t::normal
    );
    let again = restarted
        .register_invalidation(Vaultd, 1001, &object)
        .expect("idempotent");
    assert!(again.already_present);
}

#[test]
fn compromise_escalates_an_earlier_invalidation_reason() {
    let user = test_user(1001);
    let mut service = enrolled(&[&user]);
    service
        .register_invalidation(
            Vaultd,
            1001,
            &invalidation(
                &user.identity,
                &user.persistent.key_id,
                reason_of("rotated"),
                "1",
            ),
        )
        .expect("rotated");
    service
        .register_invalidation(
            Vaultd,
            1001,
            &invalidation(
                &user.identity,
                &user.persistent.key_id,
                reason_of("compromised"),
                "2",
            ),
        )
        .expect("compromised");
    service
        .register_invalidation(
            Vaultd,
            1001,
            &invalidation(
                &user.identity,
                &user.persistent.key_id,
                reason_of("retired"),
                "3",
            ),
        )
        .expect("retired");
    let resolution = service
        .resolve_key(Contractd, &user.persistent.key_id)
        .expect("resolve");
    assert_eq!(resolution.invalidated, Some(reason_of("compromised")));
}

#[test]
fn invalidation_narrows_in_every_state_except_deleted_and_never_targets_the_identity_key() {
    let user = test_user(1001);
    let mut service = enrolled(&[&user]);
    assert_eq!(
        code(service.register_invalidation(
            Vaultd,
            1001,
            &invalidation(
                &user.identity,
                &user.identity.key_id,
                reason_of("retired"),
                "i"
            )
        )),
        "key_authority/key_not_certified"
    );
    service
        .journal_mut()
        .push_user_record(1001, user_record_t::quarantine)
        .expect("quarantine");
    assert_eq!(
        scope_of(&mut service, &user.persistent.key_id),
        key_scope_t::denied(cham3_identityd::outcome::authority_code_t::quarantined)
    );
    // Quarantine denies every key; an invalidation still narrows further.
    service
        .register_invalidation(
            Vaultd,
            1001,
            &invalidation(
                &user.identity,
                &user.temporary.key_id,
                reason_of("compromised"),
                "t",
            ),
        )
        .expect("invalidation while quarantined");
    // Certificates are refused while quarantined.
    let fresh = certificate(
        &user.identity,
        1001,
        &test_key("fresh"),
        authority_class_t::persistent,
        1,
    );
    assert_eq!(
        code(service.register_certificate(Vaultd, 1001, &fresh)),
        "key_authority/quarantined"
    );
    service
        .journal_mut()
        .push_user_record(1001, user_record_t::deletion_pending)
        .expect("deletion");
    assert_eq!(
        code(service.register_invalidation(
            Vaultd,
            1001,
            &invalidation(
                &user.identity,
                &user.persistent.key_id,
                reason_of("retired"),
                "p"
            )
        )),
        "key_authority/user_deleted"
    );
    assert_eq!(
        scope_of(&mut service, &user.identity.key_id),
        key_scope_t::denied(cham3_identityd::outcome::authority_code_t::user_deleted)
    );
    // A deleted user's UID cannot enroll the old identity again.
    assert_eq!(
        code(service.register_enrollment(
            Vaultd,
            1001,
            &user.enrollment,
            &user.persistent_certificate,
            &user.temporary_certificate
        )),
        "key_authority/user_deleted"
    );
}

#[test]
fn resolve_key_and_user_identity_check_caller_roles() {
    let user = test_user(1001);
    let mut service = enrolled(&[&user]);
    for caller in [
        caller_role_t::trusted_client { uid: 1001 },
        caller_role_t::admin,
    ] {
        assert_eq!(
            code(service.resolve_key(caller, &user.persistent.key_id)),
            "ipc/role_denied"
        );
    }
    for caller in [
        caller_role_t::fixture_consumer,
        caller_role_t::service(service_name_t::vaultd),
        caller_role_t::service(service_name_t::policyd),
        caller_role_t::service(service_name_t::registryd),
    ] {
        service
            .resolve_key(caller, &user.persistent.key_id)
            .expect("allowed role");
    }
    assert_eq!(
        code(service.resolve_key(Contractd, &[9; 32])),
        "key_authority/key_not_certified"
    );
    assert_eq!(
        code(service.user_identity(caller_role_t::trusted_client { uid: 1002 }, 1001)),
        "ipc/owner_mismatch"
    );
    for caller in [caller_role_t::admin, caller_role_t::fixture_consumer] {
        assert_eq!(code(service.user_identity(caller, 1001)), "ipc/role_denied");
    }
    service
        .user_identity(caller_role_t::trusted_client { uid: 1001 }, 1001)
        .expect("own identity");
}

#[test]
fn an_unreadable_or_inconsistent_journal_denies_instead_of_answering_stale_state() {
    let user = test_user(1001);
    let other = test_user(1002);
    let mut service = enrolled(&[&user, &other]);
    service.journal_mut().faults.unavailable = true;
    assert_eq!(
        code(service.resolve_key(Contractd, &user.persistent.key_id)),
        "ipc/storage_unavailable"
    );
    service.journal_mut().faults.unavailable = false;
    assert_eq!(
        scope_of(&mut service, &user.persistent.key_id),
        key_scope_t::normal
    );
    // A record that cannot apply (closing a ceremony that was never opened) makes the
    // whole user unusable; the other user is unaffected.
    service
        .journal_mut()
        .push_user_record(
            1001,
            user_record_t::recovery_closed {
                recovery_id: [1; 16],
            },
        )
        .expect("record");
    for key in [&user.identity, &user.persistent, &user.temporary] {
        assert_eq!(
            code(service.resolve_key(Contractd, &key.key_id)),
            "ipc/storage_unavailable"
        );
    }
    assert_eq!(
        scope_of(&mut service, &other.persistent.key_id),
        key_scope_t::normal
    );
    // The same holds after a restart: the keys read before the bad record are
    // answered storage_unavailable, never normal.
    let mut restarted = identity_service_t::start(service.journal().clone()).expect("restart");
    assert_eq!(
        code(restarted.resolve_key(Contractd, &user.persistent.key_id)),
        "ipc/storage_unavailable"
    );
    assert_eq!(
        scope_of(&mut restarted, &other.persistent.key_id),
        key_scope_t::normal
    );
}

#[test]
fn rebuild_pages_through_long_journals() {
    let user = test_user(1001);
    let mut service = enrolled(&[&user]);
    let mut rotated = Vec::new();
    for index in 0..40 {
        let key = test_key(&format!("rotation {index}"));
        service
            .register_certificate(
                Vaultd,
                1001,
                &certificate(&user.identity, 1001, &key, authority_class_t::persistent, 1),
            )
            .expect("certificate");
        rotated.push(key.key_id);
    }
    assert_eq!(service.journal().record_count(1001), 43);
    let page = service.journal().identity_state(1001, 0).expect("page");
    assert_eq!(page.entries.len(), 16);
    assert_eq!(page.next, Some(16));
    let mut restarted = identity_service_t::start(service.journal().clone()).expect("restart");
    for key_id in &rotated {
        assert_eq!(scope_of(&mut restarted, key_id), key_scope_t::normal);
    }
    assert_eq!(
        restarted
            .resolve_key(Contractd, &rotated[39])
            .expect("resolve")
            .journal_seq,
        43
    );
}

fn vector_commit(name: &str) -> commit_payload_t {
    let document = support::vectors();
    let keys = support::synthetic_keys(&document);
    let resolver = support::resolver_t::new(&keys);
    let item = document
        .get("valid")
        .map(support::json_t::items)
        .unwrap_or_default()
        .iter()
        .find(|item| item.field("name") == name)
        .expect("vector")
        .clone();
    let verified = verify_object(
        &support::hex_decode(item.field("envelope")),
        object_kind_t::commit,
        &resolver,
    )
    .expect("valid vector");
    match verified.payload {
        payload_t::commit(payload) => *payload,
        _ => panic!("commit"),
    }
}

/// Re-binds a vector commit to `user` and `key` (layer 6 reads only the payload).
fn commit_for(
    name: &str,
    user: &test_user_t,
    key: &test_key_t,
    class: authority_class_t,
) -> commit_payload_t {
    let mut payload = vector_commit(name);
    payload.actor.uid = user.uid;
    payload.actor.user_id = user.identity.key_id;
    payload.key_id = key.key_id;
    payload.authority_class = class;
    payload.timestamp = T1.to_owned();
    payload
}

#[test]
fn layer_six_binds_signer_class_window_and_owner() {
    let user = test_user(1001);
    let other = test_user(1002);
    let mut service = enrolled(&[&user, &other]);
    let check = |service: &mut service_t, payload: &commit_payload_t| {
        let resolution = service.resolve_key(Contractd, &payload.key_id)?;
        check_commit_authority(&resolution, payload)
    };
    let revision = commit_for(
        "commit_revision",
        &user,
        &user.persistent,
        authority_class_t::persistent,
    );
    check(&mut service, &revision).expect("positive control");
    let temporary = commit_for(
        "commit_temporary_session",
        &user,
        &user.temporary,
        authority_class_t::temporary,
    );
    assert_eq!(temporary.commit_type, commit_type_t::temporary_grant);
    check(&mut service, &temporary).expect("temporary key signs a temporary grant");

    let cases: Vec<(&str, commit_payload_t)> = vec![
        (
            "key_authority/authority_class_denied",
            commit_for(
                "commit_revision",
                &user,
                &user.temporary,
                authority_class_t::temporary,
            ),
        ),
        (
            "key_authority/authority_class_denied",
            commit_for(
                "commit_revision",
                &user,
                &user.persistent,
                authority_class_t::temporary,
            ),
        ),
        (
            "key_authority/authority_class_denied",
            commit_for(
                "commit_revision",
                &user,
                &user.identity,
                authority_class_t::persistent,
            ),
        ),
        (
            "key_authority/key_not_certified",
            commit_for(
                "commit_revision",
                &other,
                &user.persistent,
                authority_class_t::persistent,
            ),
        ),
        (
            "key_authority/key_not_certified",
            commit_for(
                "commit_revision",
                &user,
                &other.persistent,
                authority_class_t::persistent,
            ),
        ),
        ("key_authority/certificate_window", {
            let mut payload = commit_for(
                "commit_temporary_session",
                &user,
                &user.temporary,
                authority_class_t::temporary,
            );
            payload.timestamp = "2026-10-15T08:00:00Z".to_owned();
            payload
        }),
        ("key_authority/certificate_window", {
            let mut payload = revision.clone();
            payload.timestamp = "2026-10-13T07:59:59Z".to_owned();
            payload
        }),
        ("key_authority/key_not_certified", {
            let mut payload = revision.clone();
            payload.actor.uid = 1002;
            payload
        }),
    ];
    for (expected, payload) in cases {
        assert_eq!(
            code(check(&mut service, &payload)),
            expected,
            "{:?}",
            payload.commit_type
        );
    }
}

#[test]
fn containment_scopes_keys_to_the_recovery_ceremony() {
    let user = test_user(1001);
    let mut service = enrolled(&[&user]);
    service
        .journal_mut()
        .push_user_record(1001, user_record_t::containment)
        .expect("contain");
    let contained_code = cham3_identityd::outcome::authority_code_t::contained;
    assert_eq!(
        scope_of(&mut service, &user.persistent.key_id),
        key_scope_t::denied(contained_code)
    );
    assert_eq!(
        scope_of(&mut service, &user.identity.key_id),
        key_scope_t::recovery { recovery_id: None }
    );
    let recovery_id = [0x5a; 16];
    let fresh_persistent = test_key("recovery persistent");
    let fresh_temporary = test_key("recovery temporary");
    let ceremony_certificate = |key: &test_key_t, class: authority_class_t, epoch: u64| {
        let mut payload = certificate_payload(&user.identity, 1001, key, class, epoch);
        payload.supersedes_key_id = Some(match class {
            authority_class_t::persistent => user.persistent.key_id,
            authority_class_t::temporary => user.temporary.key_id,
        });
        sign_certificate(&user.identity, &payload)
    };
    // No ceremony is open: certificates are refused.
    assert_eq!(
        code(service.register_certificate(
            Vaultd,
            1001,
            &ceremony_certificate(&fresh_persistent, authority_class_t::persistent, 2)
        )),
        "key_authority/contained"
    );
    service
        .journal_mut()
        .push_user_record(
            1001,
            user_record_t::recovery_opened {
                recovery_id,
                key_epoch: 2,
                mode: ceremony_mode_t::c1,
                role: record_role_t::trusted_client,
            },
        )
        .expect("open");
    assert_eq!(
        scope_of(&mut service, &user.identity.key_id),
        key_scope_t::recovery {
            recovery_id: Some(recovery_id)
        }
    );
    assert_eq!(
        code(service.register_certificate(
            Vaultd,
            1001,
            &ceremony_certificate(&fresh_persistent, authority_class_t::persistent, 3)
        )),
        "admission/recovery_rule"
    );
    for (key, class) in [
        (&fresh_persistent, authority_class_t::persistent),
        (&fresh_temporary, authority_class_t::temporary),
    ] {
        service
            .register_certificate(Vaultd, 1001, &ceremony_certificate(key, class, 2))
            .expect("ceremony certificate");
    }
    let required_code = cham3_identityd::outcome::authority_code_t::recovery_key_required;
    assert_eq!(
        scope_of(&mut service, &fresh_persistent.key_id),
        key_scope_t::denied(required_code)
    );
    service
        .journal_mut()
        .push_naming(
            1001,
            naming_t {
                recovery_id,
                persistent_key_id: fresh_persistent.key_id,
                temporary_key_id: fresh_temporary.key_id,
            },
        )
        .expect("naming");
    assert_eq!(
        scope_of(&mut service, &fresh_persistent.key_id),
        key_scope_t::recovery {
            recovery_id: Some(recovery_id)
        }
    );
    assert_eq!(
        scope_of(&mut service, &fresh_temporary.key_id),
        key_scope_t::denied(required_code)
    );
    let stale_code = cham3_identityd::outcome::authority_code_t::key_epoch_stale;
    assert_eq!(
        scope_of(&mut service, &user.persistent.key_id),
        key_scope_t::denied(stale_code)
    );
    assert_eq!(
        scope_of(&mut service, &user.identity.key_id),
        key_scope_t::recovery { recovery_id: None }
    );

    // The named key signs only ceremony-bound restore/revocation commits.
    let resolution = service
        .resolve_key(Vaultd, &fresh_persistent.key_id)
        .expect("resolve");
    let mut restore = commit_for(
        "commit_restore",
        &user,
        &fresh_persistent,
        authority_class_t::persistent,
    );
    restore.authorization.session_id = recovery_id;
    check_commit_scope(&resolution, &restore).expect("ceremony restore");
    let mut unbound = restore.clone();
    unbound.authorization.session_id = [0; 16];
    assert_eq!(
        code(check_commit_scope(&resolution, &unbound)),
        "admission/recovery_rule"
    );
    let mut revision = commit_for(
        "commit_revision",
        &user,
        &fresh_persistent,
        authority_class_t::persistent,
    );
    revision.authorization.session_id = recovery_id;
    assert_eq!(
        code(check_commit_scope(&resolution, &revision)),
        "admission/recovery_rule"
    );
    check_commit_authority(&resolution, &revision)
        .expect("layer 6 alone leaves the ceremony rules to admission");

    service
        .journal_mut()
        .push_user_record(1001, user_record_t::recovery_closed { recovery_id })
        .expect("close");
    assert_eq!(
        scope_of(&mut service, &fresh_persistent.key_id),
        key_scope_t::normal
    );
    assert_eq!(
        scope_of(&mut service, &fresh_temporary.key_id),
        key_scope_t::normal
    );
    assert_eq!(
        scope_of(&mut service, &user.persistent.key_id),
        key_scope_t::denied(stale_code),
        "pre-ceremony keys never sign again"
    );
}

#[test]
fn an_enrollment_signed_by_another_key_is_rejected() {
    // Proof of possession: the enrollment must be signed by the key it enrolls.
    let user = test_user(1001);
    let intruder = SigningKey::from_bytes(&[3; 32]);
    let payload = enrollment_payload(&user.identity, 1001);
    let payload_bytes = cham3_contract_format::dcbor::encode(&payload.to_value());
    let protected = cham3_contract_format::envelope::protected_header_bytes(
        &user.identity.key_id,
        object_kind_t::identity_enrollment,
    );
    let message = cham3_contract_format::envelope::to_be_signed(
        object_kind_t::identity_enrollment,
        &protected,
        &payload_bytes,
    );
    let signature = intruder.sign(&message).to_bytes();
    let forged =
        cham3_contract_format::envelope::assemble_envelope(&protected, &payload_bytes, &signature);
    let mut service = started();
    assert_eq!(
        code(service.register_enrollment(
            Vaultd,
            1001,
            &forged,
            &user.persistent_certificate,
            &user.temporary_certificate
        )),
        "signature/signature_invalid"
    );
    assert_eq!(service.journal().record_count(1001), 0);
}
