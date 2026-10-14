
// Independent review controls: no production source changed.
#[derive(Clone)]
struct reviewer_ledger_fault_io_t { armed: Arc<AtomicBool> }
impl journal_io_t for reviewer_ledger_fault_io_t {
    fn write_all(&self, file: &mut fs::File, bytes: &[u8]) -> io::Result<()> { system_io_t.write_all(file, bytes) }
    fn sync_data(&self, file: &fs::File) -> io::Result<()> { system_io_t.sync_data(file) }
    fn sync_directory(&self, path: &Path) -> io::Result<()> { system_io_t.sync_directory(path) }
    fn append_file(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        if self.armed.load(Ordering::SeqCst) && path.file_name().is_some_and(|n| n == "entries") {
            return Err(io::Error::other("reviewer injected ledger write failure"));
        }
        system_io_t.append_file(path, bytes)
    }
}
#[test]
fn reviewer_time_sample_ledger_failure_denies_current_evaluation() {
    let root = temp_root("reviewer_sample_failure");
    let armed = Arc::new(AtomicBool::new(false));
    let mut world = world_at_with(&root, registry_config_t::default(), mandatory_policy_t::default(), Box::new(reviewer_ledger_fault_io_t {armed: armed.clone()}));
    let alice = test_user(1001); world.enroll(&alice); let editor = app("editor");
    let mut lifetime = persistent_lifetime(); lifetime.expiration = Some(cham3_identityd::index::utc_text((T1Seconds + 100) as i64).unwrap());
    let genesis = draft(&alice, &editor, commit_type_t::genesis, "reviewer failure").add(vec![file_grant("reviewer timed",alice.uid,"home:Files",&["read"],lifetime)]).sign(&alice);
    world.submit(&alice,&genesis);
    let identity = runtime(&alice,&editor); let request = files(alice.uid,"home:Files","read");
    assert!(is_eligible(world.evaluate(&identity,&request)), "positive control before forward sample");
    let base=world.time.current().unwrap(); let ahead=test_time_t::at(T1Seconds+200);
    ahead.set_available(true,T1Seconds+200,base.boottime);
    let mut policy=policy_with_clock(&world,&ahead,&kernel_boot_id());
    armed.store(true,Ordering::SeqCst);
    let decision=policy.evaluate(Launcher,&identity,&request).unwrap();
    let failed=world.registry.rollback_status(Policyd).unwrap().ledger_failed;
    println!("ledger_failed={failed}; current={decision:?}");
    assert!(failed,"fault reached ledger");
    let next=policy.evaluate(Launcher,&identity,&request).unwrap();
    println!("next={next:?}");
    assert_eq!(next,decision_t::unknown(unknown_reason_t::registry_unavailable));
    assert!(!is_eligible(decision), "mandatory denial: ledger failed during this evaluation");
}
#[test]
fn reviewer_forward_sample_without_fault_expires_the_grant() {
    let mut world=world("reviewer_sample_positive"); let alice=test_user(1001); world.enroll(&alice); let editor=app("editor");
    let mut lifetime=persistent_lifetime(); lifetime.expiration=Some(cham3_identityd::index::utc_text((T1Seconds+100) as i64).unwrap());
    let genesis=draft(&alice,&editor,commit_type_t::genesis,"reviewer positive").add(vec![file_grant("reviewer positive timed",alice.uid,"home:Files",&["read"],lifetime)]).sign(&alice);
    world.submit(&alice,&genesis);
    let base=world.time.current().unwrap(); let ahead=test_time_t::at(T1Seconds+200); ahead.set_available(true,T1Seconds+200,base.boottime);
    let mut policy=policy_with_clock(&world,&ahead,&kernel_boot_id());
    assert_eq!(policy.evaluate(Launcher,&runtime(&alice,&editor),&files(alice.uid,"home:Files","read")).unwrap(),denied(deny_reason_t::expired));
}

#[test]
fn reviewer_network_time_sample_failure_stays_offline() {
    let mut bad=false;
    for mode in ["direct","tor"] {
        let root=temp_root(&format!("reviewer_network_{mode}")); let armed=Arc::new(AtomicBool::new(false));
        let mut world=world_at_with(&root,registry_config_t::default(),mandatory_policy_t::default(),Box::new(reviewer_ledger_fault_io_t{armed:armed.clone()}));
        let alice=test_user(1001); world.enroll(&alice); let editor=app("editor");
        let genesis=draft(&alice,&editor,commit_type_t::genesis,&format!("reviewer network {mode}")).add(vec![network_grant(&format!("reviewer {mode}"),alice.uid,mode,persistent_lifetime())]).sign(&alice);
        world.submit(&alice,&genesis);
        let base=world.time.current().unwrap(); let ahead=test_time_t::at(T1Seconds+200); ahead.set_available(true,T1Seconds+200,base.boottime);
        let mut policy=policy_with_clock(&world,&ahead,&kernel_boot_id()); armed.store(true,Ordering::SeqCst);
        let decision=policy.network_mode(Launcher,&runtime(&alice,&editor)).unwrap();
        println!("network {mode} after failed ledger={decision:?}");
        bad |= matches!(decision,network_decision_t::mode{..});
    }
    assert!(!bad,"network must be Offline after ledger failure");
}

#[test]
fn reviewer_equal_start_and_error_at_admission() {
    let mut world=world("reviewer_equal_start"); let alice=test_user(1001); world.enroll(&alice); let editor=app("editor");
    // Forward error is present at admission; start equals admission's trusted time.
    world.time.step_wall_clock(T1Seconds+3600);
    let mut lifetime=persistent_lifetime(); lifetime.effective_start=cham3_identityd::index::utc_text((T1Seconds+3600) as i64).unwrap();
    let genesis=draft(&alice,&editor,commit_type_t::genesis,"reviewer equal").timestamp(&lifetime.effective_start).add(vec![file_grant("equal",alice.uid,"home:Equal",&["read"],lifetime)]).sign(&alice);
    world.submit(&alice,&genesis);
    assert!(is_eligible(world.evaluate(&runtime(&alice,&editor),&files(alice.uid,"home:Equal","read"))));
}
#[test]
fn reviewer_restore_and_merge_keep_future_start_observed_delay() {
    for kind in ["restore","merge"] {
        let mut world=world(&format!("reviewer_future_{kind}")); let alice=test_user(1001); world.enroll(&alice); let editor=app("editor");
        let old=file_grant("historical",alice.uid,"home:Future",&["read"],persistent_lifetime());
        let genesis=draft(&alice,&editor,commit_type_t::genesis,"future genesis").add(vec![old.clone()]).sign(&alice); world.submit(&alice,&genesis);
        let head=draft(&alice,&editor,commit_type_t::revision,"future remove").on(&genesis).remove(vec![old.grant_id]).sign(&alice); world.submit(&alice,&head);
        let mut lifetime=persistent_lifetime(); lifetime.effective_start=cham3_identityd::index::utc_text((T1Seconds+160) as i64).unwrap();
        let grant=file_grant(kind,alice.uid,"home:Future",&["read"],lifetime);
        let commit=if kind=="restore" {
            draft(&alice,&editor,commit_type_t::restore,"future restore").on(&head).restore_from(&genesis).add(vec![grant.clone()]).sign(&alice)
        } else {
            let branch=draft(&alice,&editor,commit_type_t::temporary_grant,"future branch").on(&head).add(vec![file_grant("branch",alice.uid,"home:Other",&["read"],timed_lifetime("2026-10-20T00:00:00Z"))]).temporary_authority().sign(&alice);
            world.submit(&alice,&branch);
            draft(&alice,&editor,commit_type_t::merge,"future merge").merge(&head,&[&branch]).add(vec![grant.clone()]).sign(&alice)
        };
        world.submit(&alice,&commit);
        let view=world.registry.lineage_head(Policyd,alice.uid,&commit.payload.lineage_id).unwrap().unwrap();
        assert_eq!(view.origins[&grant.grant_id].commit_id,commit.commit_id);
        assert_eq!(view.origins[&grant.grant_id].admitted.trusted,(T1Seconds+60) as i64);
        world.time.step_wall_clock(T1Seconds+200);
        let identity=runtime(&alice,&editor); let request=files(alice.uid,"home:Future","read");
        assert_eq!(world.evaluate(&identity,&request),denied(deny_reason_t::not_started));
        world.time.advance(99); assert_eq!(world.evaluate(&identity,&request),denied(deny_reason_t::not_started));
        world.time.advance(1); assert!(is_eligible(world.evaluate(&identity,&request)));
    }
}
