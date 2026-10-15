//! Author probe (M2-006 stage B2 revision 2): does `restrict` persist a restriction
//! the strict decoder refuses? Pre-B2 API only.
#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unwrap_used)]

mod builders;

use builders::*;
use cham3_contract_format::schema::{commit_type_t, random_id_t};
use cham3_registryd::record::{lifecycle_record_t, registry_record_t};
use cham3_registryd::registry::{registry_config_t, registry_t};

#[test]
fn author_unsorted_restriction_poisons_replay() {
    for (label, order) in [("descending", [1usize, 0]), ("duplicate", [0, 0])] {
        let root = temp_root(&format!("probe_{label}"));
        let time = test_time_t::at(T1Seconds + 60);
        let config = registry_config_t { boot_id: Some("boot-1".to_owned()), ..registry_config_t::default() };
        let registry = std::sync::Arc::new(registry_t::open(&root, owner_of(&root), config.clone(), Box::new(cham3_registryd::journal::system_io_t), Box::new(time.clone())).expect("start"));
        let alice = test_user(1001);
        enroll(&registry, &alice);
        let editor = app("probe");
        let genesis = draft(&alice, &editor, commit_type_t::genesis, "probe")
            .add(vec![
                file_grant("probe a", 1001, "home:A", &["read"], persistent_lifetime()),
                file_grant("probe b", 1001, "home:B", &["read"], persistent_lifetime()),
            ])
            .sign(&alice);
        admit(&registry, &alice, &genesis).expect("genesis");
        let mut ids: Vec<random_id_t> = vec![synthetic_id("grant probe a"), synthetic_id("grant probe b")];
        ids.sort();
        let record = lifecycle_record_t::restriction { grant_ids: vec![ids[order[0]], ids[order[1]]] };
        let result = registry.restrict(Policyd, 1001, &record);
        let undecodable = records_of(&root, 1001).iter().filter(|bytes| registry_record_t::decode(bytes).is_err()).count();
        registry.shutdown().expect("shutdown");
        drop(registry);
        let registry = registry_t::open(&root, owner_of(&root), config, Box::new(cham3_registryd::journal::system_io_t), Box::new(time)).expect("restart");
        let unusable = registry.unusable_users().expect("users");
        eprintln!("{label}: restrict={result:?} undecodable={undecodable} unusable_after_restart={unusable:?}");
        assert!(unusable.is_empty() && undecodable == 0, "{label}");
    }
}
