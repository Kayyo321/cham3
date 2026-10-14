# Reviewer reproductions (stage-A re-review 7d570c1): the reviewer's three test sources, unchanged

## Sources (SHA-256 as the review record states them)
bd6e59153e9fa89dd1439b5addb31800d4a4f6bf1c127ea3240a5f582d84236f  artifacts/02_discovery/validation_artifacts/storage_r2_validation.rs   -> core/registryd/tests/review_r2_storage.rs
845d502b5ebb59857e6dabbf2fe381f1970d14fc45c8718a56db4848e739e0c4  artifacts/02_discovery/validation_artifacts/candidate-7591ccc60092f495/queue_additions.rs
e10c6e40a87431dcfb621ddafe63c16191165a070ebef6c97279a3b9c4352fce  artifacts/reviewer_matrix.rs   -> core/registryd/tests/reviewer_matrix.rs

queue_additions.rs holds only the four queue_ tests; as the reviewer ran them, they are appended to the
e34fd8a storage_integrity.rs (479ac3a86c0e9a881bf8ebdb6ab08653ae52e18b95b70be82fdfc972ed7dc85a), whose
fault-injection helpers they use. The concatenation, the same file at both candidates:
2e1d442c68ab17b17438e28388d453a6d420a1ba8c8bc0b8112818dc52987953  core/registryd/tests/queue_validation.rs

## Clones
git clone --no-hardlinks /home/dev/cham3 /home/dev/cham3-m2006a-r3/e34fd8a && git -C /home/dev/cham3-m2006a-r3/e34fd8a checkout --detach e34fd8a0b97f5a606656ab87914d4d1b19517d7c
git clone --no-hardlinks /home/dev/cham3 /home/dev/cham3-m2006a-r3/9633ce3 && git -C /home/dev/cham3-m2006a-r3/9633ce3 checkout --detach 9633ce30c212cd2cd3515b9a13d7227d1f9f9796
git clone --no-hardlinks /home/dev/cham3 /home/dev/cham3-m2006a-r3/2217e62 && git -C /home/dev/cham3-m2006a-r3/2217e62 checkout --detach 2217e62ddf42c42573346155727c983d937addc0
cp review_r2_storage.rs queue_validation.rs reviewer_matrix.rs <clone>/core/registryd/tests/   # no other change; the first lines of each log print the three digests

## Container (cham3-m2-dev from the repository Dockerfile: image id 4f4f684d76f7, the same as the reviewer's, at e34fd8a and 9633ce3; the no-cache rebuild 2a0bcf342c57 with the refreshed locks at 2217e62)
docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/work/out/m2/reviewer_target -v <clone>:/work -v <scratch>:/scratch -w /work cham3-m2-dev bash /scratch/inner_reviewer_r2.sh   # debug and release runs of the three test binaries, filtered to the reviewer's tests

## Results (debug and release identical)
e34fd8a: review_r2_storage 3 passed / 3 failed (review_r2_equal_counter_second_restart_must_remain_quarantined, review_r2_replay_marker_must_survive_second_restart, review_r2_compound_failure_must_preserve_restriction: R2-01, R2-02, R2-03 present); queue_validation 1 passed / 3 failed (R2-04 present); reviewer_matrix 4 passed. As in the review record.
9633ce3 and 2217e62 (identical; the two later commits change no code): review_r2_storage 5 passed / 1 failed (only review_r2_compound_failure_must_preserve_restriction, in its replay=false fail_invalidation=true case: the case ADR 0006 §3 erratum E5 excludes, a process death before any retry succeeds, then a same-boot restart; the replay=true cases are now contained, the equal-counter quarantine holds at the second restart as `anchored`); queue_validation 3 passed / 1 failed (only queue_expected_partial_flush_read_gate, at its premise assertion that a record is appended but not yet anchored: left 5, right 4, the anchor equals the journal); reviewer_matrix 4 passed.

## Log digests
ae9a5d3eaa230ab2fc7c66ec842989d138de2c55ca109ce5ddef11f07e54efa5  reviewer_tests_at_e34fd8a.log
558dee350d5176b31b4dc6b77840c2cafb68711108c3b1283e6cf526b7b6db56  reviewer_tests_at_9633ce3.log
f7242587cef2e375826872c7fc5c3de64fab1aac15f6c10a9b437d2e98121b15  reviewer_tests_at_2217e62.log
