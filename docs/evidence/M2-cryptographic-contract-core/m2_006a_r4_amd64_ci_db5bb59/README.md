# M2-006 stage A revision 4 — GitHub Actions evidence at db5bb59

This is the second amd64 run of candidate db5bb59fcf461034461d9e0d2f21f5816986134c (push event; range 908c391..db5bb59). The author retrieved it with the GitHub CLI (`gh api repos/Kayyo321/cham3/actions/runs/<id>`, `…/jobs`, `…/artifacts`; `gh run view <id> --log`) and downloaded the artifact; it is not an independent review.

| Workflow run | Jobs | Result |
|---|---|---|
| [m2_checks 38592632636](https://github.com/Kayyo321/cham3/actions/runs/38592632636) | `qualify_amd64` (115951179754): GitHub-hosted amd64 runner, pinned Debian 13.7 container built from the unchanged Dockerfile, UID 1001, `CHAM3_M2_REQUIRE_CLEAN=1`; then the vault protection positive path, the full-filesystem step and the wrapper step | success, every step (`m2 qualification: PASS`, both lock checks PASS, suite_unit and suite_harness PASS; `vault protection positive path: PASS`; `restrictive reserve under a full filesystem: PASS`; wrapper cases `Ran 5 tests in 0.490s … OK`) |
| [m1_checks 38592632234](https://github.com/Kayyo321/cham3/actions/runs/38592632234) | static_and_desktop, privileged_capture, package_build | success, every step |
| [m0_checks 38592632553](https://github.com/Kayyo321/cham3/actions/runs/38592632553) | static_checks, image_build, package_build | success, every step |

The three runs were created by the push of db5bb59 at 2026-10-14T15:48:52Z (attempt 1 each); m1 completed at 15:52:05Z, m0 at 15:52:35Z, m2 at 16:15:49Z.

Artifact `m2_qualification_amd64`: id 11910945081, 44 files, 107 219 bytes, `sha256:f9d7bb8444648e0ffe486b7e9fb3f214a0e21841b13d4b9988ef7e16d6bd6671`. The artifact listing reports this digest, and the author's download (`gh api repos/Kayyo321/cham3/actions/artifacts/11910945081/zip`, then `sha256sum`) gives the same digest. It holds the qualification directory `amd64_20261014T155006Z_1` (its `summary.txt` has the same PASS and NOT_AVAILABLE lines as the VM run; `identity.txt`: commit db5bb59…, tree `76b1e2a1…`, `dirty_tracked 0`, Debian 13.7, kernel 6.17.0-1022-azure, `euid 1001`, `cargo_lock_sha256 9801453a…`, the same tree and lock digest as the VM run), both run directories (unit `unit_20261014T155123Z_1849`: 209 cargo tests per profile, inventory-exact, format differential 5955, corpus unchanged; harness `harness_20261014T155653Z_4474`: `Ran 147 tests in 1024.837s`, `OK`, against 388.2 s on the author's machine), `protection/vault_protection_report.txt` and `storage/reserve_full_disk_{release,debug}.txt`. Both storage reports are byte-identical to the author's VM run (`../m2_006a_r4_amd64_vm_db5bb59/storage/`).

Positive-path protection report, from the artifact:

~~~text
memlock_limit Max locked memory         1073741824           1073741824           bytes
required_memlock_bytes 805306368
derivation_ms 448
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

Wrapper preservation step, from the job log:

~~~text
test_a_disk_without_the_paths_is_left_as_found ... ok
test_existing_root_and_filler_are_refused_and_preserved ... ok
test_symbolic_links_and_their_targets_are_refused_and_preserved ... ok
test_restored_pre_deletion_is_detected ... ok
test_unmodified_copy_preserves_existing_paths ... ok
Ran 5 tests in 0.490s
OK
~~~
