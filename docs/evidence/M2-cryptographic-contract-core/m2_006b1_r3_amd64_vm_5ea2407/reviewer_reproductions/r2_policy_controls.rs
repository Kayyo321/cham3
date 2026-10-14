
#[test]
fn rereview_refused_samples_after_fresh_and_successful_evaluations() {
    for earlier_success in [false, true] {
        for malformed in [false, true] {
            for network in [false, true] {
                let label = format!("r2_refusal_{earlier_success}_{malformed}_{network}");
                let armed = Arc::new(AtomicBool::new(false));
                let mut world = world_at_with(&temp_root(&label), registry_config_t::default(),
                    mandatory_policy_t::default(), Box::new(reviewer_ledger_fault_io_t { armed: Arc::clone(&armed) }));
                let alice = test_user(1001); world.enroll(&alice); let editor = app("editor");
                let mut grants = vec![
                    file_grant(&format!("{label}_file"), alice.uid, "home:R2", &["read"], persistent_lifetime()),
                    network_grant(&format!("{label}_network"), alice.uid, "direct", persistent_lifetime())
                ];
                grants.sort_by_key(|grant| grant.grant_id);
                let genesis = draft(&alice, &editor, commit_type_t::genesis, &label).add(grants).sign(&alice);
                world.submit(&alice, &genesis);
                let base = world.time.current().unwrap();
                let source = test_time_t::at(base.realtime + 5);
                source.set_available(true, base.realtime + 5, base.boottime);
                let mut policy = policy_with_clock(&world, &source, &kernel_boot_id());
                let identity = runtime(&alice, &editor);
                let request = files(alice.uid, "home:R2", "read");
                if earlier_success {
                    assert!(is_eligible(policy.evaluate(Launcher, &identity, &request).unwrap()));
                    assert!(matches!(policy.network_mode(Launcher, &identity).unwrap(), network_decision_t::mode { .. }));
                }
                let before = policy.clock().offset();
                let trusted_before = world.registry.rollback_status(Policyd).unwrap().clock.trusted_now;
                source.set_available(true, base.realtime + 500, base.boottime + if malformed {100} else {0});
                armed.store(!malformed, Ordering::SeqCst);
                let reason = if malformed { unknown_reason_t::clock_unavailable } else { unknown_reason_t::registry_unavailable };
                if network {
                    assert_eq!(policy.network_mode(Launcher, &identity).unwrap(), network_decision_t::offline_unknown(reason));
                } else {
                    assert_eq!(policy.evaluate(Launcher, &identity, &request).unwrap(), decision_t::unknown(reason));
                }
                assert_eq!(policy.clock().offset(), before, "no refused value adopted");
                assert_eq!(world.registry.rollback_status(Policyd).unwrap().clock.trusted_now, trusted_before);
                println!("PASS fresh={} malformed={} network={} offset={before:?}", !earlier_success, malformed, network);
            }
        }
    }
}
