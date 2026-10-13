# M2-006 stage A — GitHub Actions evidence at f4b0628

This is the second amd64 run of candidate f4b062826ae54ba4e79c5abd08eeb9fe21d48510 (push event; range 22b6b5f..f4b0628). The author retrieved it through the GitHub MCP connector; it is not an independent review.

| Workflow run | Jobs | Result |
|---|---|---|
| [m2_checks 38459079087](https://github.com/Kayyo321/cham3/actions/runs/38459079087) | `qualify_amd64` (115509254958): GitHub-hosted amd64 runner, pinned Debian 13.7 container, UID 1001, `CHAM3_M2_REQUIRE_CLEAN=1`; then **"Vault process protection, positive path (memlock limit raised, unprivileged)"** with `--ulimit memlock=1073741824:1073741824`; then **"Restrictive reserve under a full filesystem (2 MiB tmpfs, unprivileged)"** with `--tmpfs /full-disk:rw,size=2m,…` (the probe now also starts a ledger on the full tmpfs); then **"Wrapper preservation on the tmpfs (unprivileged)"** | success, every step (qualification 19:59–20:17 UTC, `m2 qualification: PASS`, suite_unit and suite_harness PASS; protection 18 s; full filesystem 31 s; wrapper cases `Ran 5 tests in 0.364s … OK`) |
| [m1_checks 38459079018](https://github.com/Kayyo321/cham3/actions/runs/38459079018) | static_and_desktop, privileged_capture, package_build | success, every step |
| [m0_checks 38459079003](https://github.com/Kayyo321/cham3/actions/runs/38459079003) | static_checks, image_build, package_build | success, every step |

Artifact `m2_qualification_amd64`: id 11846490342, 44 files, 105 358 bytes, `sha256:6c27239b2149b3a0d39f342f83adc81f0500c1060d5a4437a35dfbd080319a04` (the digest reported by the upload step and the artifact listing; the author did not download the ZIP). It holds the qualification summaries, both run directories (unit and harness, inventory-exact against the same `test_inventory.txt`: 188 cargo tests per profile, 147 harness tests), `out/m2/protection/vault_protection_report.txt` and `out/m2/storage/reserve_full_disk_{release,debug}.txt`.

Positive-path protection report from the job log:

~~~text
memlock_limit Max locked memory         1073741824           1073741824           bytes
required_memlock_bytes 805306368
derivation_ms 533
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

Full-filesystem step, from the job log (identical for the debug profile). The ledger lines are new in stage A: the ledger started on the same tmpfs before the fill cannot take an entry once the filesystem is full, is marked failed, loses its validity file (the next start would find no ledger: an unverifiable run), and stays failed after space returns; the journal's reserve path is unchanged from M2-005.

~~~text
profile release
mount tmpfs /full-disk tmpfs rw,nosuid,nodev,noexec,relatime,size=2048k,mode=700,uid=1001,gid=1001,inode64 0 0
reserve_bytes 1048576
reserve_allocated_bytes 1048576
reserve_promised_bytes 1048576
reserve_preallocated
tmpfs_mount /full-disk
ledger_started
filler_bytes 1036288
restrictive_append_on_full_filesystem ok seq=35
reserve_bytes_after_release 0
admission_refused reserve_depleted
second_restrictive_append ok seq=36
ledger_append_on_full_filesystem refused
ledger_failed
ledger_validity_removed
previous_ledger missing
append_after_space_returned ok seq=37
reserve_bytes 1048576
reserve_allocated_bytes 1048576
reserve_promised_bytes 1048576
reserve_preallocated
reserve_replenished
journal_records 37
journal_chained
ledger_stays_failed
reserve_verified
exit_status 0
restrictive reserve under a full filesystem: PASS
~~~

The filler is two blocks smaller than at 5860f27 (1 036 288 against 1 044 480 bytes): the ledger's validity file and its block-aligned entries file now occupy the tmpfs before the fill. The sequence numbers and reports match the author's VM run (`../m2_006a_amd64_vm_f4b0628/storage/`) except for the mount line's UID/GID and the `inode64` option.
