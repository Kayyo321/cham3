
#[test]
fn reviewer_anomaly_episode_survives_same_boot_restart() {
    let root=temp_root("reviewer_anomaly_restart"); let time=test_time_t::at(T1Seconds);
    let alice=test_user(1001); let registry=start(&root,"boot-1",&time); enroll(&registry,&alice);
    time.step_wall_clock(T1Seconds-400);
    registry.lineage_head(Policyd,alice.uid,&NoLineage).unwrap();
    assert_eq!(anomalies(&root,alice.uid).len(),1);
    registry.shutdown().unwrap(); drop(registry);
    let registry=start(&root,"boot-1",&time);
    registry.lineage_head(Policyd,alice.uid,&NoLineage).unwrap();
    println!("after restart anomalies={:?}",anomalies(&root,alice.uid));
    assert_eq!(anomalies(&root,alice.uid).len(),1,"same episode never returned within 300s");
}
#[test]
fn reviewer_anomaly_pending_blocks_admission() {
    let root=temp_root("reviewer_anomaly_pending"); let time=test_time_t::at(T1Seconds);
    let io=reviewer_journal_fault_io_t::default(); let alice=test_user(1001);
    let registry=start_with(&root,"boot-1",&time,Box::new(io.clone())); enroll(&registry,&alice);
    io.armed.store(true,Ordering::SeqCst); time.step_wall_clock(T1Seconds-400);
    registry.lineage_head(Policyd,alice.uid,&NoLineage).unwrap();
    let genesis=draft(&alice,&app("editor"),commit_type_t::genesis,"pending admission").sign(&alice);
    assert_eq!(code(admit(&registry,&alice,&genesis)),"ipc/restriction_not_durable");
    assert_eq!(registry.rollback_status(Policyd).unwrap().pending_restrictions,vec![alice.uid]);
}
#[derive(Clone,Default)]
struct reviewer_journal_fault_io_t {armed: Arc<AtomicBool>}
impl journal_io_t for reviewer_journal_fault_io_t {
    fn write_all(&self,file:&mut fs::File,bytes:&[u8])->io::Result<()> {
        if self.armed.load(Ordering::SeqCst) {return Err(io::Error::other("reviewer journal failure"));}
        system_io_t.write_all(file,bytes)
    }
    fn sync_data(&self,file:&fs::File)->io::Result<()> {system_io_t.sync_data(file)}
    fn sync_directory(&self,path:&Path)->io::Result<()> {system_io_t.sync_directory(path)}
}
#[test]
fn reviewer_concurrent_floor_publications_keep_maximum_offsets() {
    let root=temp_root("reviewer_concurrent_floors"); let time=test_time_t::at(T1Seconds);
    let registry=start(&root,"boot-1",&time); time.advance(60);
    let barrier=Arc::new(std::sync::Barrier::new(9)); let mut writers=Vec::new();
    for i in 0..8u64 {
        let registry=registry.clone(); let barrier=barrier.clone();
        writers.push(std::thread::spawn(move || {
            barrier.wait();
            registry.time_sample(Policyd,"boot-1",StartBoottime+60,T1Seconds+60+i*100).unwrap();
            registry.set_policy_epoch(Admin,2,1).unwrap();
        }));
    }
    barrier.wait(); for writer in writers {writer.join().unwrap();}
    registry.shutdown().unwrap();
    let floor=newest_anchor(&root).time_floor.unwrap();
    assert_eq!(floor.trusted-floor.boottime,T1Seconds-StartBoottime+700);
    assert_eq!(floor.observed,60);
    drop(registry); time.step_wall_clock(T1Seconds);
    let registry=start(&root,"boot-1",&time); assert_eq!(trusted_now(&registry),T1Seconds+60+700);
}
#[test]
fn reviewer_new_boot_pending_termination_replays_from_ledger() {
    let root=temp_root("reviewer_termination_crash"); let time=test_time_t::at(T1Seconds); let alice=test_user(1001); let editor=app("editor");
    let registry=start(&root,"boot-1",&time); enroll(&registry,&alice);
    let genesis=draft(&alice,&editor,commit_type_t::genesis,"termination genesis").sign(&alice);
    let network=network_grant("temporary network",alice.uid,"direct",timed_lifetime("2026-10-20T00:00:00Z"));
    let branch=draft(&alice,&editor,commit_type_t::temporary_grant,"termination branch").on(&genesis).add(vec![network.clone()]).temporary_authority().sign(&alice);
    admit(&registry,&alice,&genesis).unwrap(); admit(&registry,&alice,&branch).unwrap(); registry.shutdown().unwrap(); drop(registry);
    let io=reviewer_journal_fault_io_t::default(); io.armed.store(true,Ordering::SeqCst);
    let registry=start_with(&root,"boot-2",&time,Box::new(io.clone()));
    assert_eq!(registry.standing(alice.uid,&network.grant_id),Ok(standing_t::inactive));
    assert_eq!(registry.rollback_status(Policyd).unwrap().pending_restrictions,vec![alice.uid]);
    drop(registry); io.armed.store(false,Ordering::SeqCst);
    let registry=start_with(&root,"boot-2",&time,Box::new(io));
    assert_eq!(registry.run_decision(),run_decision_t::ledger_replayed);
    let view=registry.lineage_head(Policyd,alice.uid,&genesis.payload.lineage_id).unwrap().unwrap();
    assert_eq!(view.trunk_head.map(|h|h.commit_id),Some(genesis.commit_id)); assert!(view.temporary_heads.is_empty());
    let count=decoded(&root,alice.uid).iter().filter(|r|matches!(r.body,record_body_t::event(registry_event_t::branch_terminated{..}))).count();
    assert_eq!(count,1); assert_eq!(registry.standing(alice.uid,&network.grant_id),Ok(standing_t::inactive));
}

#[test]
fn reviewer_invalid_anchor_discards_floor_and_quarantines() {
    let root=temp_root("reviewer_invalid_floor"); let time=test_time_t::at(T1Seconds); let alice=test_user(1001);
    let registry=start(&root,"boot-1",&time); enroll(&registry,&alice);
    time.step_wall_clock(T1Seconds+3600); registry.shutdown().unwrap(); drop(registry);
    for name in SlotFileNames {fs::write(root.join("anchor").join(name),b"invalid").unwrap();}
    time.step_wall_clock(T1Seconds); let registry=start(&root,"boot-1",&time);
    assert_eq!(trusted_now(&registry),T1Seconds);
    assert!(!registry.rollback_status(Policyd).unwrap().quarantined.is_empty());
    assert!(registry.identity_state(Identityd,alice.uid,0).is_err());
}
#[test]
fn reviewer_foreign_run_ledger_is_not_a_time_floor() {
    let root=temp_root("reviewer_foreign_ledger"); let foreign=temp_root("reviewer_foreign_source"); let time=test_time_t::at(T1Seconds); let alice=test_user(1001);
    let registry=start(&root,"boot-1",&time); enroll(&registry,&alice); drop(registry);
    let foreign_registry=start(&foreign,"boot-1",&time); time.step_wall_clock(T1Seconds+3600); assert_eq!(trusted_now(&foreign_registry),T1Seconds+3600); drop(foreign_registry);
    fs::copy(foreign.join("ledger/validity"),root.join("ledger/validity")).unwrap(); fs::copy(foreign.join("ledger/entries"),root.join("ledger/entries")).unwrap();
    time.step_wall_clock(T1Seconds); let registry=start(&root,"boot-1",&time);
    assert_eq!(registry.run_decision(),run_decision_t::contained); assert_eq!(trusted_now(&registry),T1Seconds);
    assert_eq!(registry.rollback_status(Policyd).unwrap().contained,vec![alice.uid]);
}
#[derive(Clone,Default)]
struct reviewer_start_failure_io_t;
impl journal_io_t for reviewer_start_failure_io_t {
    fn write_all(&self,_file:&mut fs::File,_bytes:&[u8])->io::Result<()> {Err(io::Error::other("reviewer before termination durability"))}
    fn sync_data(&self,file:&fs::File)->io::Result<()> {system_io_t.sync_data(file)}
    fn sync_directory(&self,path:&Path)->io::Result<()> {system_io_t.sync_directory(path)}
    fn replace_file(&self,path:&Path,bytes:&[u8])->io::Result<()> {
        if path.file_name().is_some_and(|n|n=="slot_a"||n=="slot_b") {return Err(io::Error::other("reviewer marker failure"));}
        system_io_t.replace_file(path,bytes)
    }
}
#[test]
fn reviewer_failed_new_boot_start_detects_the_old_boot_again() {
    let root=temp_root("reviewer_old_boot_marker"); let time=test_time_t::at(T1Seconds); let alice=test_user(1001); let editor=app("editor");
    let registry=start(&root,"boot-1",&time); enroll(&registry,&alice); let genesis=draft(&alice,&editor,commit_type_t::genesis,"old boot").sign(&alice);
    let grant=file_grant("old boot branch",alice.uid,"home:Tmp",&["read"],one_shot_lifetime());
    let branch=draft(&alice,&editor,commit_type_t::temporary_grant,"old marker branch").on(&genesis).add(vec![grant.clone()]).temporary_authority().sign(&alice);
    admit(&registry,&alice,&genesis).unwrap(); admit(&registry,&alice,&branch).unwrap(); registry.shutdown().unwrap(); drop(registry);
    assert!(registry_t::open(&root,owner_of(&root),config("boot-2"),Box::new(reviewer_start_failure_io_t),Box::new(time.clone())).is_err());
    assert_eq!(newest_anchor(&root).run_marker.unwrap().boot_id,"boot-1");
    let registry=start(&root,"boot-2",&time); assert_eq!(registry.standing(alice.uid,&grant.grant_id),Ok(standing_t::inactive));
    assert_eq!(decoded(&root,alice.uid).iter().filter(|r|matches!(r.body,record_body_t::event(registry_event_t::branch_terminated{..}))).count(),1);
}

fn reviewer_storage_image(root:&Path,uid:u32)->Vec<Vec<u8>> {
    let mut out=vec![fs::read(journal_path(root,uid)).unwrap()];
    for name in ["anchor/slot_a","anchor/slot_b","ledger/validity","ledger/entries"] {
        out.push(fs::read(root.join(name)).unwrap_or_default());
    }
    out
}
#[test]
fn reviewer_timestamp_refusal_leaves_settled_storage_byte_identical() {
    let root=temp_root("reviewer_timestamp_bytes"); let time=test_time_t::at(T1Seconds+301);
    let registry=start(&root,"boot-1",&time); let alice=test_user(1001); enroll(&registry,&alice);
    let before=reviewer_storage_image(&root,alice.uid);
    let genesis=draft(&alice,&app("editor"),commit_type_t::genesis,"refusal bytes").sign(&alice);
    assert_eq!(code(admit(&registry,&alice,&genesis)),"admission/timestamp_rule");
    assert_eq!(reviewer_storage_image(&root,alice.uid),before);
}
#[test]
fn reviewer_quarantined_user_anomaly_does_not_write_its_journal() {
    let root=temp_root("reviewer_quarantined_anomaly"); let time=test_time_t::at(T1Seconds); let alice=test_user(1001);
    let registry=start(&root,"boot-1",&time); enroll(&registry,&alice); registry.shutdown().unwrap(); drop(registry);
    for name in SlotFileNames {fs::write(root.join("anchor").join(name),b"invalid").unwrap();}
    let registry=start(&root,"boot-1",&time); let before=records_of(&root,alice.uid);
    time.step_wall_clock(T1Seconds-400);
    registry.lineage_head(Policyd,alice.uid,&NoLineage).unwrap();
    assert_eq!(records_of(&root,alice.uid),before);
    assert!(registry.rollback_status(Policyd).unwrap().clock.anomaly.open);
}
