# M2-005 revision 3 — GitHub Actions evidence at be24152

This is the second amd64 run of candidate be2415291bf5bcf9dad8df323bf971bd193d5120 (push event; range 959524c..be24152). The author retrieved it through the GitHub MCP connector; it is not an independent review.

| Workflow run | Jobs | Result |
|---|---|---|
| [m2_checks 38435225455](https://github.com/Kayyo321/cham3/actions/runs/38435225455) | `qualify_amd64` (115427328268): GitHub-hosted amd64 runner (kernel 6.17.0-1022-azure), pinned Debian 13.7 container, UID 1001, `CHAM3_M2_REQUIRE_CLEAN=1`, `dirty_tracked 0`, Cargo.lock `364e9c58…256a`; then **"Vault process protection, positive path (memlock limit raised, unprivileged)"** with `--ulimit memlock=1073741824:1073741824`; then **"Restrictive reserve under a full filesystem (2 MiB tmpfs, unprivileged)"** with `--tmpfs /full-disk:rw,size=2m,…` | success, every step (qualification 17:02–17:20 UTC, `m2 qualification: PASS`, suite_unit and suite_harness PASS; protection 20 s; full filesystem 31 s) |
| [m1_checks 38435225444](https://github.com/Kayyo321/cham3/actions/runs/38435225444) | static_and_desktop, privileged_capture, package_build | success, every step |
| [m0_checks 38435225490](https://github.com/Kayyo321/cham3/actions/runs/38435225490) | static_checks, image_build, package_build | success, every step |

Artifact `m2_qualification_amd64`: id 11834225727, 44 files, 103 412 bytes, `sha256:4168e9190cdef6e6fff0187e7a76099beded24aac32902d34d032f6dc984eef6` (the digest reported by the upload step and the artifact listing; the author did not download the ZIP). It holds the qualification summaries, both run directories (unit and harness, inventory-exact against the same `test_inventory.txt`: 169 cargo tests per profile, 142 harness tests), `out/m2/protection/vault_protection_report.txt` and `out/m2/storage/reserve_full_disk_{release,debug}.txt`.

Positive-path protection report from the job log:

~~~text
memlock_limit Max locked memory         1073741824           1073741824           bytes
required_memlock_bytes 805306368
derivation_ms 421
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

Full-filesystem step, from the job log (identical for the debug profile; the revision-3 script also asserts that neither the probe's root nor its filler is left on the tmpfs):

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

The sequence numbers, filler size and reports match the author's VM run (`../m2_005_amd64_vm_be24152/storage/`) except for the mount line's UID/GID.
