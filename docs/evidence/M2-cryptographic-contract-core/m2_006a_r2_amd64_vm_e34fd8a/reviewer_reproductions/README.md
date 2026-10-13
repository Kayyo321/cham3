# Reviewer reproductions (stage-A review 344c372): the reviewer's two test sources, unchanged

## Sources (SHA-256 as the review record states them)
cb3bbbd81e3c56f88cb85d42205c5a99d15535cff96f922d68fed7275d88c6f0  core/registryd/tests/reviewer_storage.rs
41ec9718ac19b87b60705a62771c51eb6261a55bfbf312df05291b5f0d2fada0  core/policyd/tests/policy_review.rs

## Clones
git clone --no-hardlinks /home/dev/cham3 /home/dev/cham3-m2006a-r2/f4b0628 && git -C /home/dev/cham3-m2006a-r2/f4b0628 checkout --detach f4b062826ae54ba4e79c5abd08eeb9fe21d48510
git clone --no-hardlinks /home/dev/cham3 /home/dev/cham3-m2006a-r2/candidate && git -C /home/dev/cham3-m2006a-r2/candidate checkout --detach e34fd8a0b97f5a606656ab87914d4d1b19517d7c
cp <retained>/reviewer_storage.rs <clone>/core/registryd/tests/reviewer_storage.rs; cp <retained>/policy_review.rs <clone>/core/policyd/tests/policy_review.rs   # no other change

## Container (the reviewer's image cham3-m2-independent-r3 at f4b0628, cham3-m2-dev at the candidate; both image id 4f4f684d76f7 from the repository Dockerfile)
docker run --rm --user 1001:1001 -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/work/out/m2/reviewer_target -v <clone>:/work -v <scratch>:/scratch -w /work <image> bash /scratch/inner_reviewer.sh   # inner_reviewer.sh: debug and release runs of both test binaries

## Results
f4b0628: reviewer_storage review_ 0 passed / 8 failed (debug and release); policy_review 2 passed (the adverse-behaviour assertions hold: the defects are present)
e34fd8a: reviewer_storage review_ 8 passed / 0 failed (debug and release); policy_review 0 passed / 2 failed (review_unanchored_admission_is_eligible gets unknown(identity_unavailable), review_failed_emergency_release_is_eligible gets denied(emergency_restriction): the adverse behaviour is gone)

## Log digests
9549b3c893fb7f09c6a44c694fe1bcb3068b948550f9ff99dfe26b0c5bd712da  reviewer_tests_at_f4b0628.log
02071a70c39ec3d71fef2d88d018e98dd87051ea450746184683763af2160999  reviewer_tests_at_e34fd8a.log
