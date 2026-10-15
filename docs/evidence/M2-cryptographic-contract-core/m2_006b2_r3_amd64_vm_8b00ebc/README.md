# M2-006 stage B2 revision 3 — native amd64 evidence at 8b00ebc (author, owner's home computer)

- **Candidate:** 8b00ebcff89276d2c0a70a238459fd246e7353b7 (range 14c3e4c..8b00ebc, one commit, 6 files): the answers to the stage-B2 re-review findings M2-006-B2-R2-01 and R2-02 ([finding resolutions r3](../../../reviews/M2-006-b2-finding-resolutions-r3.md)).
- **Environment:**
  - The same computer, image `cham3-m2-dev` (`2a0bcf342c57`), UID/GID 1001, `--ulimit nofile=1024:1024`.
  - Clean clone `/home/dev/cham3-m2006b2r3/qual` with `CHAM3_M2_REQUIRE_CLEAN=1`: 0 dirty paths before and after.
- **Commands:** as in the [B1 evidence](../m2_006b1_amd64_vm_c9129bc/).
- **Classification:** author development evidence; CI is the second run ([CI evidence](../m2_006b2_r3_amd64_ci_8b00ebc/)).

| File | Content |
|---|---|
| `summary.txt`, `qualification_console.txt` | `m2 qualification: PASS`, exit 0. Static checks PASS, 82 static tests OK. Miri, sanitizers and fuzzing NOT_RUN. |
| `identity.txt` | commit 8b00ebc, `dirty_tracked 0`, `Cargo.lock` unchanged, EUID 1001. |
| `unit/` | **267 + 267** cargo tests, inventory-exact (+3). ECR oracle 19/96/18 with 27 scenarios / 365 steps. Trusted-time vectors PASS (165 cases). Format differential 5 968 / 0 (corpus `44ea2aa4…`, unchanged). Vault oracle 38. |
| `admission_scenarios_nocapture.log`, `trusted_time_model_nocapture.log` | 0 disagreements each (15 / 72 / 121; 165 cases, 4 813 steps). |
| `harness/` | **149 tests** (533.6 s), with **175** seeded registry/policy defects (+3), all detected; the unmodified copy passes. |
| `harness/defect_reasons_b2r3.log` | The 3 new defects, the 2 re-pointed ones and 4 other decision-time defects, each applied alone to a copy of this clone with `RUST_BACKTRACE=1`: all exit 101 at their intended assertion. |
| `reviewer_reproductions/` | The re-review record's `reviewer_r2.rs` (kept as `reviewer_r2.rs.txt`), unchanged (digests in `SHA256SUMS`, equal to the records'), run with the record's command in clean clones, debug and release: 671a7d8 2/6; 8b00ebc **6/6**, with the earlier `reviewer_b2` **12/12**, `reviewer_policy_b2` **1/1** and the author's restriction probe **1/1**. `decision_cases_at_671a7d8.log`: the two new decision-sample tests, each case printed, accept all ten cases with 671a7d8's `registry.rs`. |
| `storage/` | Full-filesystem probe PASS, reports byte-identical to 671a7d8's; wrapper cases 5/5. |
| `unchanged_inputs.txt` | The diffstat 14c3e4c..8b00ebc; `git diff --exit-code` returns 0 for every other path listed (dependencies, lock, workflows, build scripts, oracles, vectors, ADRs, the other crates and policyd, registryd's anchor, ledger, journal, clock, record, lineage and binaries, the test builders and the other test targets); 671a7d8..14c3e4c changes only `docs/`. |

Limits: the session source, the enforcement services and the consumer are fixtures; real crashes, hardware clocks and deployed IPC are NOT_RUN. An unpushed draft of this revision (8777fe4) failed its qualification on one undetected seeded defect; see the resolutions.
