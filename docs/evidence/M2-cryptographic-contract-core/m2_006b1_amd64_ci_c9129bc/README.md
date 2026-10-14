# M2-006 stage B1 — GitHub Actions evidence at c9129bc

- **Candidate:** c9129bca0e6c4b1e6caf72774e05a27e09a4d542, triggered by its push to `codex/m2-cryptographic-contract-core`.
- **Retrieved by the author** with `gh api repos/Kayyo321/cham3/actions/runs/<id>` (and `/jobs`, `/artifacts`), `gh api repos/Kayyo321/cham3/actions/artifacts/<id>/zip` and `gh run view 38609857266 --log`.

| Workflow | Run | head_sha | Result |
|---|---|---|---|
| m2_checks | 38609857266 | c9129bc… | **success**; every job and step succeeded: image build, `qualify_amd64` (`m2 qualification: PASS`), the vault protection positive path (`protection_verified`; `vault protection positive path: PASS`), the full-filesystem step and the wrapper step |
| m1_checks | 38609857244 | c9129bc… | **success**, every job |
| m0_checks | 38609857191 | c9129bc… | **success**, every job |

**Artifact `m2_qualification_amd64`:**
- id 11918223662, 45 files, 109 619 bytes (zip).
- API digest `sha256:d8070e9b69a5ee20c1aa659009146dc019dd33e5123cb385423dbb6db652d3b5`. The downloaded zip's SHA-256 is the same.

**Its contents agree with the [author run](../m2_006b1_amd64_vm_c9129bc/):**
- Identity: commit c9129bc, `dirty_tracked 0`, `Cargo.lock` `9801453a…`.
- Cargo tests: 234 + 234 PASS.
- Harness: 149 PASS.
- ECR oracle: 19/96/18 with 27 scenarios / 365 steps, PASS.
- Trusted-time vectors: 165 cases, PASS.
- Format differential: 5 968 cases, 0 mismatches, corpus `44ea2aa4…426c`.
- The full-filesystem release report is byte-identical to the author's.
