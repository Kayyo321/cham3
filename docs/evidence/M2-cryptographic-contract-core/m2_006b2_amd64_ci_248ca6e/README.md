# M2-006 stage B2 — GitHub Actions evidence at 248ca6e

Retrieved by the author with `gh api` (runs, jobs, artifacts, the artifact zip) and `gh run view --log`.

| Workflow | Run | Result |
|---|---|---|
| m2_checks | 38657106670 | **success**: every job and step succeeded, including `m2 qualification: PASS`, the vault protection positive path, the full-filesystem step and the wrapper step |
| m1_checks | 38657106555 | **success** |
| m0_checks | 38657106631 | **success** |

Seven jobs, 88 steps, all successful or skipped.

**Artifact:** `m2_qualification_amd64`, id 11938475946, 45 files, 111 308 bytes. The API digest `sha256:5d7adfc6fbe4126ef1353b53b2488f5604d8fdc6cb60b557bdb1206f14b67dff` equals the downloaded zip's SHA-256.

**The artifact's contents agree with the [author run](../m2_006b2_amd64_vm_248ca6e/):** commit 248ca6e, `dirty_tracked 0`, cargo tests 256 + 256, harness 149, format differential 5 968, and a byte-identical full-filesystem report.
