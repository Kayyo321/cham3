# M2-006 stage B2 revision 2 — GitHub Actions evidence at 671a7d8

Retrieved by the author with `gh api` (runs, jobs, artifacts, the artifact zip).

| Workflow | Run | Result |
|---|---|---|
| m2_checks | 38665833821 | **success**: every job and step succeeded, including the qualification (21 PASS, 0 FAIL), the vault protection positive path (`protection_verified`), the full-filesystem step and the wrapper step |
| m1_checks | 38665833839 | **success** |
| m0_checks | 38665833783 | **success** |

Seven jobs, 88 steps, all successful or skipped. Every run's head SHA is 671a7d86262a6f77a391dd8fcbc99f9888be671a.

**Artifact:** `m2_qualification_amd64`, id 11942412776, 45 files, 111 813 bytes. The API digest `sha256:d3679f8e2e2630a1e0c097b9c358bf7ab5c1bcf4ee592758a78953516ecf8965` equals the downloaded zip's SHA-256.

**The artifact's contents agree with the [author run](../m2_006b2_r2_amd64_vm_671a7d8/):** commit 671a7d8, `dirty_tracked 0`, cargo tests 264 + 264, harness 149, format differential 5 968 / 0, and a byte-identical full-filesystem report.
