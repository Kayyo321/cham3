# M2-004 revision 2 — GitHub Actions evidence at ca40c62

This is the second amd64 run of candidate ca40c62ed7a5fc29ba8910fef514f3ce29ea4fba (push event). The author retrieved it through the GitHub MCP connector; it is not an independent review.

| Workflow run | Jobs | Result |
|---|---|---|
| [m2_checks 38347259191](https://github.com/Kayyo321/cham3/actions/runs/38347259191) | `qualify_amd64` (115131427974): GitHub-hosted amd64 runner, pinned Debian 13.7 container, `CHAM3_M2_REQUIRE_CLEAN=1`; then **"Vault process protection, positive path (memlock limit raised, unprivileged)"** with `--ulimit memlock=1073741824:1073741824` | success, every step |
| [m1_checks 38347259188](https://github.com/Kayyo321/cham3/actions/runs/38347259188) | static_and_desktop (115131428080), privileged_capture (115131427985), package_build (115131427820) | success, every step |
| [m0_checks 38347259258](https://github.com/Kayyo321/cham3/actions/runs/38347259258) | static_checks (115131428364), image_build (115131428041), package_build (115131428412) | success, every step |

Positive-path report from the job log (also `out/m2/protection/vault_protection_report.txt` in the artifact):

~~~text
memlock_limit Max locked memory         1073741824           1073741824           bytes
required_memlock_bytes 805306368
derivation_ms 471
dumpable 0
core_limit 0
vm_locked_kib 3156
unlocked_mappings 0
special_mappings [vdso]
dontdump_mappings 4
protection_verified
exit_status 0
vault protection positive path: PASS
~~~

- m2 artifact metadata: `m2_qualification_amd64`, id 11789878130, 97 312 bytes, `sha256:6315b087535ee9bec503e2291ab3e01d76c4bd3ebbfa35fc4a7c7d07bb895a5d` (the upload step logs the same digest), head ca40c62, 42 files.
- **NOT_RUN:** download of the artifact ZIP bytes (author `gh` unauthenticated). Download with `gh run download 38347259191 -n m2_qualification_amd64`.
