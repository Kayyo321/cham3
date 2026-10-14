# Reviewer reproductions (stage-A re-review f0d6eb0): the reviewer's test sources, unchanged

## Sources (SHA-256 as the review records state them)
5331ddd1356d4bc9d64ff4c0a64f26462051a24c631774efd775e4189bc9ec9b  artifacts/review_r3_validation.rs (re-review f0d6eb0)   -> core/registryd/tests/review_r3_validation.rs
bd6e59153e9fa89dd1439b5addb31800d4a4f6bf1c127ea3240a5f582d84236f  storage_r2_validation.rs (re-review 7d570c1)             -> core/registryd/tests/review_r2_storage.rs
2e1d442c68ab17b17438e28388d453a6d420a1ba8c8bc0b8112818dc52987953  e34fd8a storage_integrity.rs + queue_additions.rs (845d502b…) -> core/registryd/tests/queue_validation.rs
e10c6e40a87431dcfb621ddafe63c16191165a070ebef6c97279a3b9c4352fce  reviewer_matrix.rs (re-review 7d570c1)                   -> core/registryd/tests/reviewer_matrix.rs

review_r3_validation.rs is the 2217e62 storage_integrity.rs (byte-identical prefix) plus its own fault injector and twelve review_r3_ tests.

## Clones
git clone --no-hardlinks /home/dev/cham3 /home/dev/cham3-m2006a-r4/2217e62 && git -C /home/dev/cham3-m2006a-r4/2217e62 checkout --detach 2217e62ddf42c42573346155727c983d937addc0   # review_r3_validation.rs only
git clone --no-hardlinks /home/dev/cham3 /home/dev/cham3-m2006a-r4/db5bb59 && git -C /home/dev/cham3-m2006a-r4/db5bb59 checkout --detach db5bb59fcf461034461d9e0d2f21f5816986134c   # all four sources
cp <retained sources> <clone>/core/registryd/tests/   # no other change; each log prints the digests first

## Container (cham3-m2-dev, image id 2a0bcf342c57: the no-cache rebuild with the refreshed locks)
docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/work/out/m2/reviewer_target -v <clone>:/work -v <scratch>:/scratch -w /work cham3-m2-dev bash /scratch/inner_reviewer_r3.sh   # then inner_reviewer_r2.sh; debug and release

## Results (debug and release identical)
2217e62: review_r3_validation 7 passed / 5 failed (review_r3_retry_journal_users, review_r3_retry_policy_state, review_r3_retry_idempotent_identity, review_r3_retry_new_enrollment, review_r3_retry_emergency_release: invalid=false marker=running restart=ledger_replayed standing=current; R3-01 present). As in the review record.
db5bb59: review_r3_validation 12 passed / 0 failed (every operation now invalid=true marker=unverifiable restart=contained). review_r2_storage 5 passed / 1 failed (only review_r2_compound_failure_must_preserve_restriction, the case ADR 0006 §3 erratum E5 excludes); queue_validation 3 passed / 1 failed (only queue_expected_partial_flush_read_gate, its premise assertion anchored == before); reviewer_matrix 4 passed. Unchanged from 2217e62.

## Log digests
9c4dffc9a55dc1f1e9bb8f45b3b2dc10aca7e891763ddc597ff2f1a593e94096  reviewer_r3_tests_at_2217e62.log
7de725bfd3d79d383a42744d84feef2cfc92de26403180f6675981f578624332  reviewer_r3_tests_at_db5bb59.log
8bb4a51701c649fa9ae01dca58adf42a32561f24c272969a576bd9e912ee34cf  reviewer_r2_tests_at_db5bb59.log
