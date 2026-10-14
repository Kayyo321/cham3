set -u
cd /work
echo "== HEAD $(git rev-parse HEAD)"
echo "== sources"; sha256sum core/registryd/tests/review_r2_storage.rs core/registryd/tests/queue_validation.rs core/registryd/tests/reviewer_matrix.rs
for profile in "" "--release"; do
  for target in "review_r2_storage review_r2_" "queue_validation queue_" "reviewer_matrix review_matrix"; do
    set -- $target
    echo "== $1 $2 ${profile:-debug}"
    cargo test --frozen ${profile} -p cham3_registryd --test $1 $2 -- --nocapture 2>&1 | grep -v "^warning\|^   Compiling\|^    Finished\|^     Running" | grep -E "^test |test result|panicked|assert|left:|right:|equal=|replay=|BOUND|RESTART|PARTIAL|POSITIVE" | head -60
  done
done
echo REVIEWER_R2_DONE
