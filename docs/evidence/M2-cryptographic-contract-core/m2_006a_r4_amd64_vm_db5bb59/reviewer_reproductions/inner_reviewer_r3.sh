set -u
cd /work
echo "== HEAD $(git rev-parse HEAD)"
echo "== source"; sha256sum core/registryd/tests/review_r3_validation.rs
for profile in "" "--release"; do
  echo "== review_r3_validation review_r3_ ${profile:-debug}"
  cargo test --frozen ${profile} -p cham3_registryd --test review_r3_validation review_r3_ -- --nocapture --test-threads=1 2>&1 | grep -E "^test |test result|panicked|assert|operation=" | head -80
done
echo REVIEWER_R3_DONE
