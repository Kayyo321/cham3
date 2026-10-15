# M2-006 stage B2 revision 3 — GitHub Actions evidence at 8b00ebc

Retrieved by the author with `gh api` (runs, jobs, artifacts, the artifact zip).

| Workflow | Run | Result |
|---|---|---|
| m2_checks | 38755817749 | **success**: every job and step succeeded, including the qualification (21 PASS, 0 FAIL), the vault protection positive path (`protection_verified`), the full-filesystem step and the wrapper step |
| m1_checks | 38755817592 | **success** |
| m0_checks | 38755817888 | **success** |

Seven jobs, 88 steps, all successful or skipped. Every run's head SHA is 8b00ebcff89276d2c0a70a238459fd246e7353b7.

**Artifact:** `m2_qualification_amd64`, id 11984943374, 45 files, 111 957 bytes. The API digest `sha256:57177faaa9ff5223bf6611316ccd8aee3296851dcb941986dae7ee401b9443b2` equals the downloaded zip's SHA-256.

**The artifact's contents agree with the [author run](../m2_006b2_r3_amd64_vm_8b00ebc/):** commit 8b00ebc, `dirty_tracked 0`, cargo tests 267 + 267, harness 149, format differential 5 968 / 0, and a byte-identical full-filesystem report.
