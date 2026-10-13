# M2-005 attempts before the candidate

| Commit | Gate | Result | Cause and fix |
|---|---|---|---|
| 6e9d6d3a1b4c9b280ffef1cc8e154cecf980ea26 | Author qualification (clean UID-1000 clone) | FAIL: `rustfmt` only. Every other check PASS, including the unit suite; the run was stopped during the harness suite once the fix was ready | A test (`delegated_grants_are_refused_until_their_admission_rules_exist`) was appended to `registry_operations.rs` after the last pinned-rustfmt pass. Fixed in b06d7cb |
| 6e9d6d3 | GitHub Actions m2_checks [38403053054](https://github.com/Kayyo321/cham3/actions/runs/38403053054) | FAIL: `rustfmt` only (`m2 qualification: FAIL`); suite_unit and suite_harness PASS | Same cause |
| 6e9d6d3 | m1_checks 38403053043, m0_checks 38403053110 | success | — |

b06d7cb also adds the signing-key window at admission time (handoff decision 12), its test and its seeded defect.
