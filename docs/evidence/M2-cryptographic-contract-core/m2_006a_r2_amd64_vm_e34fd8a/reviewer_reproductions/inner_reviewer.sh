set -u
cd /work
echo "== HEAD $(git rev-parse HEAD)"
echo "== sources"; sha256sum core/registryd/tests/reviewer_storage.rs core/policyd/tests/policy_review.rs
for profile in "" "--release"; do
  echo "== reviewer_storage review_ ${profile:-debug}"
  cargo test --frozen ${profile} -p cham3_registryd --test reviewer_storage review_ -- --nocapture 2>&1 | grep -v "^warning\|^   Compiling\|^    Finished\|^     Running" | tail -60
  echo "== policy_review ${profile:-debug}"
  cargo test --frozen ${profile} -p cham3_policyd --test policy_review -- --nocapture 2>&1 | grep -v "^warning\|^   Compiling\|^    Finished\|^     Running" | tail -40
done
echo REVIEWER_DONE
