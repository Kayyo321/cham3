#!/bin/bash
# usage: run_reviewer_b1.sh <clean clone at the commit> <log>
# Composes the reviewer's two test files exactly as the record's C6 does and runs them in debug and release.
set -u
clone=$1; log=$2; R=/tmp/claude-1001/-home-dev-cham3/6ec58a3f-7807-42cc-8d7a-3281cf1f347e/scratchpad/reviewer_b1
cat $clone/core/policyd/tests/current_authorization.rs > $clone/core/policyd/tests/reviewer_b1.rs; printf '\n' >> $clone/core/policyd/tests/reviewer_b1.rs; cat $R/policy_reviewer_snippet.rs >> $clone/core/policyd/tests/reviewer_b1.rs
cat $clone/core/registryd/tests/trusted_time.rs > $clone/core/registryd/tests/reviewer_b1.rs; printf '\n' >> $clone/core/registryd/tests/reviewer_b1.rs; cat $R/registry_reviewer_snippet.rs >> $clone/core/registryd/tests/reviewer_b1.rs
{ echo "commit $(git -C $clone rev-parse HEAD)"; sha256sum $R/*_snippet.rs; } > $log
for profile in "" "--release"; do
  echo "=== profile ${profile:-debug}" >> $log
  sg docker -c "docker run --rm --user $(id -u):$(id -g) --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/work/out/reviewer-target -v $clone:/work -w /work cham3-m2-dev bash -c 'cargo test --frozen $profile -p cham3_policyd --test reviewer_b1 reviewer_ -- --nocapture --test-threads=1; echo policy_exit=\$?; cargo test --frozen $profile -p cham3_registryd --test reviewer_b1 reviewer_ -- --nocapture --test-threads=1; echo registry_exit=\$?'" >> $log 2>&1
done
echo REVIEWER_DONE >> $log
