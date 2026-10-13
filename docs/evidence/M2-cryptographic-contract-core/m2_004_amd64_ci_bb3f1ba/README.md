# M2-004 — GitHub Actions evidence at bb3f1ba

This is the second amd64 run of candidate bb3f1bad77080da34069af62c8d2cc390f2fcf7e (push event). The author retrieved it through the GitHub MCP connector; it is not an independent review.

| Workflow run | Jobs | Result |
|---|---|---|
| [m2_checks 38341512299](https://github.com/Kayyo321/cham3/actions/runs/38341512299) | `qualify_amd64` (115113321971): GitHub-hosted amd64 runner, pinned Debian 13.7 container, `CHAM3_M2_REQUIRE_CLEAN=1`; then **"Vault process protection, positive path (memlock limit raised, unprivileged)"** with `--ulimit memlock=1073741824:1073741824` | success, every step |
| [m1_checks 38341511647](https://github.com/Kayyo321/cham3/actions/runs/38341511647) | static_and_desktop (115113319766), privileged_capture (115113319993), package_build (115113320022) | success, every step |
| [m0_checks 38341511661](https://github.com/Kayyo321/cham3/actions/runs/38341511661) | static_checks (115113319928), image_build (115113320140), package_build (115113320184) | success, every step |

Positive-path report from the job log (also `out/m2/protection/vault_protection_report.txt` in the artifact):

~~~text
memlock_limit Max locked memory         1073741824           1073741824           bytes
required_memlock_bytes 805306368
derivation_ms 431
dumpable 0
core_limit 0
vm_locked_kib 3152
unlocked_mappings 0
special_mappings [vdso]
dontdump_mappings 4
protection_verified
exit_status 0
vault protection positive path: PASS
~~~

- m2 artifact metadata: `m2_qualification_amd64`, id 11787038783, 96 660 bytes, `sha256:ca8945506ae171139f8b994d8fcd7373fe05382f314b6d3c7d32f00ccc7d361e`, head bb3f1ba.
- **NOT_RUN:** download of the artifact ZIP bytes (author `gh` unauthenticated). Download with `gh run download 38341512299 -n m2_qualification_amd64`.
- Earlier runs of the superseded commits 0e4f993 and e5be206 are in [attempts.md](../m2_004_amd64_vm_bb3f1ba/attempts.md).
