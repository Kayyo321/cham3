# M2-005 — GitHub Actions evidence at b06d7cb

This is the second amd64 run of candidate b06d7cb21e1e3e42bbcd025abd9cea131574751d (push event). The author retrieved it through the GitHub MCP connector; it is not an independent review.

| Workflow run | Jobs | Result |
|---|---|---|
| [m2_checks 38404018473](https://github.com/Kayyo321/cham3/actions/runs/38404018473) | `qualify_amd64` (115318746873): GitHub-hosted amd64 runner, pinned Debian 13.7 container, `CHAM3_M2_REQUIRE_CLEAN=1`; then **"Vault process protection, positive path (memlock limit raised, unprivileged)"** with `--ulimit memlock=1073741824:1073741824` | success, every step |
| [m1_checks 38404018403](https://github.com/Kayyo321/cham3/actions/runs/38404018403) | static_and_desktop (115318761641), privileged_capture (115318761510), package_build (115318761487) | success, every step |
| [m0_checks 38404018497](https://github.com/Kayyo321/cham3/actions/runs/38404018497) | static_checks (115318748088), image_build (115318747903), package_build (115318747704) | success, every step |

Positive-path report from the job log (also `out/m2/protection/vault_protection_report.txt` in the artifact):

~~~text
memlock_limit Max locked memory         1073741824           1073741824           bytes
required_memlock_bytes 805306368
derivation_ms 420
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

- m2 artifact metadata: `m2_qualification_amd64`, id 11819401906, 100 963 bytes, `sha256:c9cfe469bbc109faff6fe7a82a8eccbe1bb7a4f9f7073292e83b5c21b13301b5` (the upload step logs the same digest), head b06d7cb, 42 files.
- **NOT_RUN:** download of the artifact ZIP bytes by the author (the author's `gh` is unauthenticated). Download with `gh run download 38404018473 -n m2_qualification_amd64`.
- The earlier candidate 6e9d6d3 failed m2_checks [38403053054](https://github.com/Kayyo321/cham3/actions/runs/38403053054) on rustfmt only (see `../m2_005_amd64_vm_b06d7cb/attempts.md`).
