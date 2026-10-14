
#[test]
fn rereview_startup_sample_closes_the_resumed_episode() {
    let mut bad = false;
    for clean in [true, false] {
        for startup_in_range in [false, true] {
            let root = temp_root(&format!("r2_startup_closure_{clean}_{startup_in_range}"));
            let time = test_time_t::at(T1Seconds); let alice = test_user(1001);
            let registry = start(&root, "boot-1", &time); enroll(&registry, &alice);
            time.step_wall_clock(T1Seconds - 400);
            registry.lineage_head(Policyd, alice.uid, &NoLineage).unwrap();
            assert_eq!(anomaly_episodes(&root, alice.uid), vec![("boot-1".into(), 1)]);
            if clean { registry.shutdown().unwrap(); }
            drop(registry);
            // Only an actual in-range sample ends the episode, never restart alone.
            if startup_in_range { time.step_wall_clock(T1Seconds); }
            let registry = start(&root, "boot-1", &time);
            let startup_mark = newest_anchor(&root).time_floor.as_ref().unwrap().anomaly;
            time.step_wall_clock(T1Seconds - 400);
            registry.lineage_head(Policyd, alice.uid, &NoLineage).unwrap();
            let episodes = anomaly_episodes(&root, alice.uid);
            let expected = if startup_in_range {
                vec![("boot-1".into(), 1), ("boot-1".into(), 2)]
            } else { vec![("boot-1".into(), 1)] };
            println!("clean={clean} startup_in_range={startup_in_range} startup_mark={startup_mark}; episodes={episodes:?}; expected={expected:?}");
            bad |= episodes != expected || startup_mark != if startup_in_range {3} else {2};
        }
    }
    assert!(!bad, "startup's actual in-range sample must close the episode; anomalous startup must resume it");
}
