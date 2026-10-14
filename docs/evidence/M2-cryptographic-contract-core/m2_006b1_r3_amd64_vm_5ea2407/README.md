# M2-006 stage B1 revision 3 — native amd64 evidence at 5ea2407 (author, owner's home computer)

- **Candidate:** 5ea2407ff2c639361ec2976bc20bc95f5acbce95 (range 0ec854d..5ea2407, one commit, 5 files). It answers re-review 9b5afa2 (R2-01: the startup sample ends or continues a resumed anomaly episode).
- **Environment:**
  - The same computer, image `cham3-m2-dev` (`2a0bcf342c57`), UID/GID 1001, `--ulimit nofile=1024:1024`.
  - Clean clone `/home/dev/cham3-m2006b1r3/qual` with `CHAM3_M2_REQUIRE_CLEAN=1`: 0 dirty paths before and after.
- **Commands:** as in the [B1 evidence](../m2_006b1_amd64_vm_c9129bc/).
- **Classification:** author development evidence; CI is the second run ([CI evidence](../m2_006b1_r3_amd64_ci_5ea2407/)).

| File | Content |
|---|---|
| `summary.txt`, `qualification_console.txt` | `m2 qualification: PASS`, exit 0. Static checks PASS, 82 static tests OK. Miri, sanitizers and fuzzing NOT_RUN. |
| `identity.txt` | commit 5ea2407, `dirty_tracked 0`, `Cargo.lock` unchanged, EUID 1001. |
| `unit/` | **241 + 241** cargo tests, inventory-exact (+2). ECR oracle 27 scenarios / 365 steps. Trusted-time vectors PASS. Format differential 5 968 / 0 (unchanged corpus). Vault oracle 38. |
| `admission_scenarios_nocapture.log`, `trusted_time_model_nocapture.log` | 0 disagreements each (15 / 72 / 121; 165 cases, 4 813 steps). |
| `harness/` | **149 tests** (423.3 s), with **124** seeded registry/policy defects (+1). |
| `harness/defect_reasons_b1.log` | The 27 stage-B1 defects, each applied alone: all exit 101, each at its intended assertion. |
| `reviewer_reproductions/` | The reviewer's controls, unchanged; see its README. |
| `storage/` | Full-filesystem probe PASS, reports byte-identical; wrapper cases 5/5. |
| `unchanged_inputs.txt` | The diffstat 0ec854d..5ea2407. `git diff --exit-code` returns 0 for every other path (registry, anchor, ledger and record included). |

Limits: as in the B1 evidence.
