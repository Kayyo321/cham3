# M2-006 stage B1 revision 2 — GitHub Actions evidence at 77a3d0e

Retrieved by the author with `gh api` (runs, jobs, artifacts, the artifact zip) and `gh run view --log`.

| Workflow | Run | Result |
|---|---|---|
| m2_checks | 38632964069 | **success**: every job and step succeeded, including `m2 qualification: PASS`, the vault protection positive path, the full-filesystem step and the wrapper step |
| m1_checks | 38632964091 | **success** |
| m0_checks | 38632964078 | **success** |

**Artifact:** `m2_qualification_amd64`, id 11928017840, 45 files, 109 953 bytes. The API digest `sha256:18b673470e4e73478a2fe314ed627c7324bb14d092d44bd5bff9f24a08751f40` equals the downloaded zip's SHA-256.

**The artifact's contents agree with the [author run](../m2_006b1_r2_amd64_vm_77a3d0e/):**
- commit 77a3d0e, `dirty_tracked 0`;
- cargo tests 239 + 239 PASS;
- harness 149 PASS;
- ECR oracle 27 scenarios / 365 steps PASS;
- trusted-time vectors 165 cases PASS;
- the full-filesystem release report is byte-identical.
