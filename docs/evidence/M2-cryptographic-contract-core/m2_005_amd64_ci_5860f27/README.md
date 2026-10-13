# M2-005 revision 4 — GitHub Actions evidence at 5860f27

This is the second amd64 run of candidate 5860f276ab593da34771f704691ea35649324d6e (push event; range ad18d8b..5860f27). The author retrieved it through the GitHub MCP connector; it is not an independent review.

| Workflow run | Jobs | Result |
|---|---|---|
| [m2_checks 38444270032](https://github.com/Kayyo321/cham3/actions/runs/38444270032) | `qualify_amd64` (115458446772): GitHub-hosted amd64 runner, pinned Debian 13.7 container, UID 1001, `CHAM3_M2_REQUIRE_CLEAN=1`; then **"Vault process protection, positive path (memlock limit raised, unprivileged)"** with `--ulimit memlock=1073741824:1073741824`; then **"Restrictive reserve under a full filesystem (2 MiB tmpfs, unprivileged)"** with `--tmpfs /full-disk:rw,size=2m,…`; then the new step **"Wrapper preservation on the tmpfs (unprivileged)"** (`python3 -I -m unittest discover -s tests/m2/harness -p test_m2_reserve_probe.py -k wrapper -v` with `CHAM3_M2_RESERVE_PROBE=/work/out/m2/storage/target/release/cham3_reserve_probe CHAM3_M2_WRAPPER_DISK=/full-disk` in a fresh container with the same tmpfs) | success, every step (qualification 18:09–18:24 UTC, `m2 qualification: PASS`, suite_unit and suite_harness PASS; protection 17 s; full filesystem 25 s; wrapper cases `Ran 5 tests in 0.394s … OK`) |
| [m1_checks 38444270097](https://github.com/Kayyo321/cham3/actions/runs/38444270097) | static_and_desktop, privileged_capture, package_build | success, every step |
| [m0_checks 38444270083](https://github.com/Kayyo321/cham3/actions/runs/38444270083) | static_checks, image_build, package_build | success, every step |

Artifact `m2_qualification_amd64`: id 11839175077, 44 files, 103 622 bytes, `sha256:3b9cb3b99a7dcdf04f4998c4303be3195d732a1646a0dc065ba1d340a9720ba1` (the digest reported by the upload step and the artifact listing; the author did not download the ZIP). It holds the qualification summaries, both run directories (unit and harness, inventory-exact against the same `test_inventory.txt`: 169 cargo tests per profile, 147 harness tests), `out/m2/protection/vault_protection_report.txt` and `out/m2/storage/reserve_full_disk_{release,debug}.txt`.

Positive-path protection report from the job log:

~~~text
memlock_limit Max locked memory         1073741824           1073741824           bytes
required_memlock_bytes 805306368
derivation_ms 361
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

Full-filesystem step, from the job log (identical for the debug profile; the script asserts that neither the probe's root nor its filler, nor a symbolic link at either path, is left on the tmpfs):

~~~text
profile release
mount tmpfs /full-disk tmpfs rw,nosuid,nodev,noexec,relatime,size=2048k,mode=700,uid=1001,gid=1001,inode64 0 0
reserve_bytes 1048576
reserve_allocated_bytes 1048576
reserve_promised_bytes 1048576
reserve_preallocated
tmpfs_mount /full-disk
filler_bytes 1044480
restrictive_append_on_full_filesystem ok seq=35
reserve_bytes_after_release 0
admission_refused reserve_depleted
second_restrictive_append ok seq=36
append_after_space_returned ok seq=37
reserve_bytes 1048576
reserve_allocated_bytes 1048576
reserve_promised_bytes 1048576
reserve_preallocated
reserve_replenished
journal_records 37
journal_chained
reserve_verified
exit_status 0
restrictive reserve under a full filesystem: PASS
~~~

Wrapper preservation step, from the job log:

~~~text
test_a_disk_without_the_paths_is_left_as_found (test_m2_reserve_probe.full_disk_wrapper_t.test_a_disk_without_the_paths_is_left_as_found) ... ok
test_existing_root_and_filler_are_refused_and_preserved (test_m2_reserve_probe.full_disk_wrapper_t.test_existing_root_and_filler_are_refused_and_preserved) ... ok
test_symbolic_links_and_their_targets_are_refused_and_preserved (test_m2_reserve_probe.full_disk_wrapper_t.test_symbolic_links_and_their_targets_are_refused_and_preserved) ... ok
test_restored_pre_deletion_is_detected (test_m2_reserve_probe.seeded_wrapper_defects_t.test_restored_pre_deletion_is_detected) ... ok
test_unmodified_copy_preserves_existing_paths (test_m2_reserve_probe.seeded_wrapper_defects_t.test_unmodified_copy_preserves_existing_paths) ... ok

----------------------------------------------------------------------
Ran 5 tests in 0.394s

OK
~~~

The sequence numbers, filler size and reports match the author's VM run (`../m2_005_amd64_vm_5860f27/storage/`) except for the mount line's UID/GID and the `inode64` option.
