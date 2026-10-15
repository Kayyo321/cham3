# M2-006 stage B2 — native amd64 evidence at 248ca6e (author, owner's home computer)

- **Candidate:** 248ca6ef90d22e270db0e9a3f5bf789502d18720 (range f6f6883..248ca6e, one commit, 13 files): reservations, sessions, confirmations, audit gaps, `identity_mismatch`, the expiry latch and `history` (ADR 0007 §§3, 4, 6, 7; ADR 0008 §2; erratum E7).
- **Environment:**
  - The same computer, image `cham3-m2-dev` (`2a0bcf342c57`), UID/GID 1001, `--ulimit nofile=1024:1024`.
  - Clean clone `/home/dev/cham3-m2006b2/qual` with `CHAM3_M2_REQUIRE_CLEAN=1`: 0 dirty paths before and after.
- **Commands:** as in the [B1 evidence](../m2_006b1_amd64_vm_c9129bc/).
- **Classification:** author development evidence; CI is the second run ([CI evidence](../m2_006b2_amd64_ci_248ca6e/)).

| File | Content |
|---|---|
| `summary.txt`, `qualification_console.txt` | `m2 qualification: PASS`, exit 0. Static checks PASS, 82 static tests OK. Miri, sanitizers and fuzzing NOT_RUN. |
| `identity.txt` | commit 248ca6e, `dirty_tracked 0`, `Cargo.lock` unchanged, EUID 1001. |
| `unit/` | **256 + 256** cargo tests, inventory-exact (+15). ECR oracle 19/96/18 with 27 scenarios / 365 steps. Trusted-time vectors PASS (165 cases). Format differential 5 968 / 0 (corpus `44ea2aa4…`, unchanged). Vault oracle 38. |
| `admission_scenarios_nocapture.log`, `trusted_time_model_nocapture.log` | 0 disagreements each (15 / 72 / 121; 165 cases, 4 813 steps). |
| `harness/` | **149 tests** (497.7 s), with **159** seeded registry/policy defects (+35). |
| `harness/defect_reasons_b2.log` | The 35 stage-B2 defects, each applied alone to a copy of this clone: all exit 101, each at its intended assertion. |
| `storage/` | Full-filesystem probe PASS, reports byte-identical to 5ea2407's; wrapper cases 5/5. |
| `unchanged_inputs.txt` | The diffstat f6f6883..248ca6e; `git diff --exit-code` returns 0 for every other path listed (dependencies, lock, workflows, build scripts, oracles, vectors, ADRs, the other crates, registryd's anchor, ledger, journal and clock, policyd's mandatory policy); 5ea2407..f6f6883 changes only `docs/`. |

Limits: the session source, the enforcement services and the consumer are fixtures; real crashes, hardware clocks and deployed IPC are NOT_RUN.
