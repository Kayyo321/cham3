# M2-006 stage B1 revision 3 — GitHub Actions evidence at 5ea2407

Retrieved by the author with `gh api` (runs, jobs, artifacts, the artifact zip) and `gh run view --log`.

| Workflow | Run | Result |
|---|---|---|
| m2_checks | 38641025183 | **success**: every job and step succeeded, including `m2 qualification: PASS`, the vault protection positive path, the full-filesystem step and the wrapper step |
| m1_checks | 38641025244 | **success** |
| m0_checks | 38641025091 | **success** |

**Artifact:** `m2_qualification_amd64`, id 11931692301, 45 files, 110 092 bytes. The API digest `sha256:c0caa359a6c5e6a7daba6931f1c5fa8589c18ec0694ed8ff7dc0429c947ae2b0` equals the downloaded zip's SHA-256.

**The artifact's contents agree with the [author run](../m2_006b1_r3_amd64_vm_5ea2407/):** commit 5ea2407, `dirty_tracked 0`, cargo tests 241 + 241, harness 149, and a byte-identical full-filesystem report.
