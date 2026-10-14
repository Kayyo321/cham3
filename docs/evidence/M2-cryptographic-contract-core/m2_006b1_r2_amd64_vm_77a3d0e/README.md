# M2-006 stage B1 revision 2 — native amd64 evidence at 77a3d0e (author, owner's home computer)

- **Candidate:** 77a3d0e65072b60f4ce1780a709dd342fdda67f7 (range b4e4cdc..77a3d0e, one commit). It answers review 792f5f0:
  - R01: a refused time report denies.
  - R02: anomaly episodes survive same-boot restarts.
  - It also records the owner's E4/E6 clarifications in ADR 0007.
  - 15 files, none new.
- **Environment:**
  - The same computer, image `cham3-m2-dev` (`2a0bcf342c57`), UID/GID 1001, `--ulimit nofile=1024:1024`.
  - Clean clone `/home/dev/cham3-m2006b1r2/qual` with `CHAM3_M2_REQUIRE_CLEAN=1`: 0 dirty paths before and after.
- **Commands:** as in the [B1 evidence](../m2_006b1_amd64_vm_c9129bc/), with this clone and commit.
- **Classification:** author development evidence; CI is the second run ([CI evidence](../m2_006b1_r2_amd64_ci_77a3d0e/)).

| File | Content |
|---|---|
| `summary.txt`, `qualification_console.txt` | `m2 qualification: PASS`, exit 0. Static checks PASS, 82 static tests OK. Miri, sanitizers and fuzzing are NOT_AVAILABLE, so NOT_RUN. |
| `identity.txt` | commit 77a3d0e, `dirty_tracked 0`, `Cargo.lock` unchanged, EUID 1001. |
| `unit/` | **239 + 239** cargo tests, inventory-exact (+5). ECR oracle 19/96/18, 27 scenarios / 365 steps. Trusted-time vectors PASS (165 cases; unchanged). Format differential 5 968 / 0, corpus `44ea2aa4…` (unchanged). Vault oracle 38. |
| `admission_scenarios_nocapture.log`, `trusted_time_model_nocapture.log` | 15 / 72 / 121 / 0 disagreements; 165 cases, 4 813 steps, 0 disagreements. |
| `harness/` | **149 tests** (427.8 s), with **123** seeded registry/policy defects (+4, 2 re-pointed). |
| `harness/defect_reasons_b1.log` | The 26 stage-B1 defects, each applied alone: all exit 101, each at its intended assertion. |
| `reviewer_reproductions/` | The reviewer's controls, unchanged: 11/14 at c9129bc, 14/14 at 77a3d0e, debug and release. See its README. |
| `storage/` | Full-filesystem probe PASS, reports byte-identical to c9129bc's; wrapper cases 5/5. |
| `unchanged_inputs.txt` | The diffstat b4e4cdc..77a3d0e. `git diff --exit-code` returns 0 for the manifests, the lock, `build`, the workflows, every other crate, `evaluate.rs`, `lineage.rs`, `journal.rs`, the oracles and the vectors. |

Limits: as in the B1 evidence. NOT_RUN: hardware clocks, suspend, time synchronization and real crashes.
