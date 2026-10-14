#!/bin/bash
# usage: run_rereview_r2.sh <clean clone> <log>
# Composes the R2 record's C6 files: fixture + newline + original snippet + supplemental snippet(s), as rereview_r2.rs.
set -u
clone=$1; log=$2; R=/tmp/claude-1001/-home-dev-cham3/6ec58a3f-7807-42cc-8d7a-3281cf1f347e/scratchpad/reviewer_b1
p=$clone/core/policyd/tests; r=$clone/core/registryd/tests
{ cat $p/current_authorization.rs; printf '\n'; cat $R/policy_reviewer_snippet.rs $R/r2_policy_controls.rs; } > $p/rereview_r2.rs
{ cat $r/trusted_time.rs; printf '\n'; cat $R/registry_reviewer_snippet.rs $R/r2_registry_controls.rs $R/r2_startup_control.rs; } > $r/rereview_r2.rs
{ echo "commit $(git -C $clone rev-parse HEAD)"; sha256sum $R/*_snippet.rs $R/r2_*.rs; } > $log
for profile in "" "--release"; do
  echo "=== profile ${profile:-debug}" >> $log
  sg docker -c "docker run --rm --user $(id -u):$(id -g) --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/work/out/r2-target -v $clone:/work -w /work cham3-m2-dev bash -c 'cargo test --frozen $profile -p cham3_policyd --test rereview_r2 rereview_ -- --nocapture --test-threads=1; echo policy_exit=\$?; cargo test --frozen $profile -p cham3_registryd --test rereview_r2 rereview_ -- --nocapture --test-threads=1; echo registry_exit=\$?'" >> $log 2>&1
done
echo REREVIEW_DONE >> $log
