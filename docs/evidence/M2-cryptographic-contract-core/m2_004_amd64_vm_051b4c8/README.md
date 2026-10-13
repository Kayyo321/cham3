# M2-004 revision 3 — the reviewers' own tests at ca40c62 and 051b4c8 (author VM)

- **Candidate:** 051b4c8aacdeecfabb23625f93dd16615aec39a0 (the M2-004 change, M2-004-R2-01, is in 4f7080b; 051b4c8 adds harness seeded defects for M2-005 R03 only). Full qualification, static gates and CI: [m2_005_amd64_vm_051b4c8](../m2_005_amd64_vm_051b4c8/) and [m2_005_amd64_ci_051b4c8](../m2_005_amd64_ci_051b4c8/) (shared candidate).
- **Environment:** Anthropic cloud VM, native x86_64; the pinned `cham3-m2-dev` image; UID/GID 1000; clean clones `/tmp/cham3_old_004r2` (ca40c62) and `/tmp/cham3_m2005r2final` (051b4c8) mounted read-only at `/work`; the reviewer packages live outside both clones and were reconstructed byte for byte from the records (`reviewer_reproductions/source_digests.txt`: the original `review.rs` `922dd57b…bea4` and `Cargo.toml` `82224a3c…1004` of f2d82b7; `new-tests.rs` `e8053731…0eb8` and the concatenation `d619b29f…1b20` of fd6de58).
- **Classification:** author development evidence; not an independent review.

| File | Content |
|---|---|
| `reviewer_reproductions/m2_004_r2_tests_at_ca40c62.log` | The twelve tests of the revision-2 record at ca40c62, debug and release: **10 passed, 2 failed** in each profile (`per_user_retired_ids 39, limit 32` and `global_retired_ids 319, limit 256`), as the record reports |
| `reviewer_reproductions/reviewer_tests_at_051b4c8.log` | Every reviewer package at the candidate, debug and release: the twelve revision-2 tests **12 passed**; the original seventeen of f2d82b7 **16 passed, 1 failed** (`defect_recovery_retry_authenticates_passphrase_before_naming`, the I01 assertion both records classify as stronger than R14); and the M2-005 packages (see the M2-005 directory) |
| `reviewer_reproductions/source_digests.txt` | SHA-256 of every reconstructed reviewer source and manifest |

Commands (the external packages are reviewer code; `/evidence` holds them, `/work` the read-only clone):

~~~sh
docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/evidence/adversarial004/target \
  -v /tmp/cham3_old_004r2:/work:ro -v <packages>:/evidence -w /evidence/adversarial004 cham3-m2-dev \
  cargo test --manifest-path core/reviewer-r2/Cargo.toml --offline r2_ -- --nocapture --test-threads=1
# the same with --release --locked; the same two against /tmp/cham3_m2005r2final:/work:ro;
# the original seventeen: --manifest-path core/reviewer/Cargo.toml (no filter), debug and release.
~~~
