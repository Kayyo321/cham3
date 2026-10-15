# M2-006 stage B2 revision 2 — native amd64 evidence at 671a7d8 (author, owner's home computer)

- **Candidate:** 671a7d86262a6f77a391dd8fcbc99f9888be671a (range ac28d24..671a7d8, one commit, 9 files): the answers to the stage-B2 review findings M2-006-B2-01–B2-04 ([finding resolutions](../../../reviews/M2-006-b2-finding-resolutions.md)).
- **Environment:**
  - The same computer, image `cham3-m2-dev` (`2a0bcf342c57`), UID/GID 1001, `--ulimit nofile=1024:1024`.
  - Clean clone `/home/dev/cham3-m2006b2r2/qual` with `CHAM3_M2_REQUIRE_CLEAN=1`: 0 dirty paths before and after.
- **Commands:** as in the [B1 evidence](../m2_006b1_amd64_vm_c9129bc/).
- **Classification:** author development evidence; CI is the second run ([CI evidence](../m2_006b2_r2_amd64_ci_671a7d8/)).

| File | Content |
|---|---|
| `summary.txt`, `qualification_console.txt` | `m2 qualification: PASS`, exit 0. Static checks PASS, 82 static tests OK. Miri, sanitizers and fuzzing NOT_RUN. |
| `identity.txt` | commit 671a7d8, `dirty_tracked 0`, `Cargo.lock` unchanged, EUID 1001. |
| `unit/` | **264 + 264** cargo tests, inventory-exact (+8). ECR oracle 19/96/18 with 27 scenarios / 365 steps. Trusted-time vectors PASS (165 cases). Format differential 5 968 / 0 (corpus `44ea2aa4…`, unchanged). Vault oracle 38. |
| `admission_scenarios_nocapture.log`, `trusted_time_model_nocapture.log` | 0 disagreements each (15 / 72 / 121; 165 cases, 4 813 steps). |
| `harness/` | **149 tests** (545.5 s), with **172** seeded registry/policy defects (+13), all detected; the unmodified copy passes. |
| `harness/defect_reasons_b2r2.log` | The 13 new and 3 re-pointed defects, each applied alone to a copy of this clone with `RUST_BACKTRACE=1`: all exit 101 at their intended assertion (the calling test line is listed for each). |
| `reviewer_reproductions/` | The review record's `reviewer_b2.rs` and `reviewer_policy_b2.rs`, unchanged (digests in `SHA256SUMS`, equal to the record's), run with the record's commands in clean clones, debug and release: 248ca6e registryd 10/12 and policyd 0/1; 671a7d8 **12/12** and **1/1**. The author's `author_restriction_probe.rs`: fails at 248ca6e (debug, release) and 5ea2407 (debug), passes at 671a7d8 (debug, release). |
| `storage/` | Full-filesystem probe PASS, reports byte-identical to 248ca6e's; wrapper cases 5/5. |
| `unchanged_inputs.txt` | The diffstat ac28d24..671a7d8; `git diff --exit-code` returns 0 for every other path listed (dependencies, lock, workflows, build scripts, oracles, vectors, ADRs, the other crates, registryd's anchor, ledger, journal, clock and binaries, the test builders, policyd's whole source); 248ca6e..ac28d24 changes only `docs/`. |

Limits: the session source, the enforcement services and the consumer are fixtures; real crashes, hardware clocks and deployed IPC are NOT_RUN.
