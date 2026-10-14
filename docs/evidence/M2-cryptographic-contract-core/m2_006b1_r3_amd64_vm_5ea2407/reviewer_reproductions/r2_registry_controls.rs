
#[test]
fn rereview_ledger_only_closure_and_equal_payload_episodes() {
    let root = temp_root("r2_ledger_closure"); let time = test_time_t::at(T1Seconds);
    let alice = test_user(1001); let bob = test_user(1002);
    let registry = start(&root, "boot-1", &time); enroll(&registry, &alice); enroll(&registry, &bob);
    let look = |r: &registry_t, u: &test_user_t| { r.lineage_head(Policyd, u.uid, &NoLineage).unwrap(); };
    time.step_wall_clock(T1Seconds - 400); look(&registry, &alice); look(&registry, &bob);
    let anchor_before = newest_anchor(&root).time_floor.unwrap();
    assert_eq!(anchor_before.anomaly, 2);
    time.step_wall_clock(T1Seconds); look(&registry, &alice);
    assert_eq!(newest_anchor(&root).time_floor.unwrap(), anchor_before, "closure only ledger");
    assert_eq!(ledger_floors(&root).last().unwrap().anomaly, 3);
    drop(registry);
    let registry = start(&root, "boot-1", &time);
    assert_eq!(registry.run_decision(), run_decision_t::ledger_replayed);
    assert_eq!(registry.rollback_status(Policyd).unwrap().clock.anomaly.mark(), 3);
    time.step_wall_clock(T1Seconds - 400); look(&registry, &alice); look(&registry, &bob);
    for user in [&alice, &bob] {
        assert_eq!(anomaly_episodes(&root, user.uid), vec![("boot-1".into(), 1), ("boot-1".into(), 2)]);
        assert_eq!(anomalies(&root, user.uid), vec![(T1Seconds, 400), (T1Seconds, 400)]);
    }
    registry.shutdown().unwrap(); drop(registry);
    let registry = start(&root, "boot-1", &time); look(&registry, &alice); look(&registry, &bob);
    assert_eq!(anomaly_episodes(&root, bob.uid).len(), 2);
}
#[test]
fn rereview_episode_contained_and_invalid_anchor_start() {
    for invalid_anchor in [false, true] {
        let root = temp_root(&format!("r2_contained_{invalid_anchor}"));
        let time = test_time_t::at(T1Seconds); let alice = test_user(1001);
        let registry = start(&root, "boot-1", &time); enroll(&registry, &alice);
        time.step_wall_clock(T1Seconds - 400);
        registry.lineage_head(Policyd, alice.uid, &NoLineage).unwrap();
        let before = fs::read(journal_path(&root, alice.uid)).unwrap();
        drop(registry);
        if invalid_anchor {
            for slot in SlotFileNames { fs::write(root.join("anchor").join(slot), b"invalid").unwrap(); }
        } else { fs::remove_file(root.join("ledger/validity")).unwrap(); }
        let registry = start(&root, "boot-1", &time);
        let status = registry.rollback_status(Policyd).unwrap();
        if invalid_anchor {
            assert!(!status.quarantined.is_empty());
            assert_eq!(fs::read(journal_path(&root, alice.uid)).unwrap(), before);
            assert!(registry.lineage_head(Policyd, alice.uid, &NoLineage).unwrap().unwrap().origins.is_empty());
        } else {
            assert_eq!(registry.run_decision(), run_decision_t::contained);
            assert_eq!(status.clock.anomaly.mark(), 2);
            assert_eq!(status.contained, vec![alice.uid]);
            registry.lineage_head(Policyd, alice.uid, &NoLineage).unwrap();
            assert_eq!(anomaly_episodes(&root, alice.uid), vec![("boot-1".into(), 1)]);
        }
    }
}
#[test]
fn rereview_mark_encoding_merge_and_record_domain() {
    use cham3_registryd::clock::{anomaly_state_t, merge_floor};
    for episode in 1..100 {
        let open = anomaly_state_t { episode, open: true, behind: 400 };
        let closed = anomaly_state_t { open: false, ..open };
        assert_eq!(open.mark(), episode * 2);
        assert_eq!(closed.mark(), episode * 2 + 1);
        assert_eq!(anomaly_state_t::from_mark(open.mark()).episode, episode);
        assert!(anomaly_state_t::from_mark(open.mark()).open);
        assert!(!anomaly_state_t::from_mark(closed.mark()).open);
    }
    let old = time_floor_t { boot_id: "boot-1".into(), boottime: 10, trusted: T1Seconds, observed: 20, anomaly: 7 };
    let next = time_floor_t { boottime: 11, trusted: T1Seconds + 1, observed: 21, anomaly: 2, ..old.clone() };
    assert_eq!(merge_floor(Some(old.clone()), next.clone()).anomaly, 7);
    assert_eq!(merge_floor(Some(next), old).anomaly, 7);
    for episode in [0, 1, 99] {
        let record = registry_record_t { sequence: 1, time: utc_text(T1Seconds as i64).unwrap(),
            body: record_body_t::event(registry_event_t::clock_anomaly { boot_id: "boot-1".into(), episode, trusted: T1Seconds, behind_seconds: 400 }) };
        let decoded = registry_record_t::decode(&record.encode());
        if episode == 0 { assert!(decoded.is_err()); } else { assert_eq!(decoded.unwrap(), record); }
    }
}
