# The reviewer's B1 controls, reproduced unchanged

**Sources:** `policy_reviewer_snippet.rs` (SHA-256 `c33ade32…b25b`) and `registry_reviewer_snippet.rs` (`014db1de…a47`).
- Extracted byte-identically from `docs/reviews/M2-006-stage-b1-codex-security.md` at 792f5f0 on `review/m2-006-b1-security`; the digests equal the record's table.
- Each snippet is appended to the candidate's `core/policyd/tests/current_authorization.rs` or `core/registryd/tests/trusted_time.rs`, as `reviewer_b1.rs`, exactly as the record's C6 step does.

**Script:** `run_reviewer_b1.sh <clone> <log>` runs the reviewer tests (filter `reviewer_`, one thread) in debug and release, in `cham3-m2-dev` as UID 1001. Each run uses a fresh clean clone.

| Log | Commit | policyd | registryd |
|---|---|---|---|
| `at_c9129bc.log` | c9129bc (the reviewed candidate) | 3 passed, **2 failed** (`…ledger_failure_denies_current_evaluation`: `current=eligible`; `…network_time_sample_failure_stays_offline`: `mode { mode: direct }`) | 8 passed, **1 failed** (`…anomaly_episode_survives_same_boot_restart`: two `(1791882000, 400)`) |
| `at_77a3d0e.log` | 77a3d0e (revision 2) | **5 passed** | **9 passed** |

The c9129bc counts are the record's 11 PASS / 3 FAIL, and both profiles agree.
