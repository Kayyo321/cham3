# M2-005 revision 2 and M2-004 revision 3 — GitHub Actions evidence at 051b4c8

This is the second amd64 run of candidate 051b4c8aacdeecfabb23625f93dd16615aec39a0 (push event; range 24a35c1..051b4c8). The author retrieved it through the GitHub MCP connector; it is not an independent review.

| Workflow run | Jobs | Result |
|---|---|---|
| [m2_checks 38422383429](https://github.com/Kayyo321/cham3/actions/runs/38422383429) | `qualify_amd64` (115382831231): GitHub-hosted amd64 runner, pinned Debian 13.7 container, `CHAM3_M2_REQUIRE_CLEAN=1`; then **"Vault process protection, positive path (memlock limit raised, unprivileged)"** with `--ulimit memlock=1073741824:1073741824`; then the new **"Restrictive reserve under a full filesystem (2 MiB tmpfs, unprivileged)"** with `--tmpfs /full-disk:rw,size=2m,…` | success, every step (qualification 15:29–15:47 UTC; protection 20 s; full filesystem 31 s) |
| [m1_checks 38422383455](https://github.com/Kayyo321/cham3/actions/runs/38422383455) | static_and_desktop, privileged_capture, package_build | success, every step |
| [m0_checks 38422383430](https://github.com/Kayyo321/cham3/actions/runs/38422383430) | static_checks, image_build, package_build | success, every step |

Positive-path protection report from the job log (`out/m2/protection/vault_protection_report.txt` in the artifact):

~~~text
memlock_limit Max locked memory         1073741824           1073741824           bytes
required_memlock_bytes 805306368
derivation_ms 418
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

Full-filesystem step, from the job log (also `out/m2/storage/reserve_full_disk_{release,debug}.txt` in the artifact):

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
(the debug profile reports the same lines)
restrictive reserve under a full filesystem: PASS
~~~

- m2 artifact metadata: `m2_qualification_amd64`, id 11827908447, 103 014 bytes, `sha256:f05dedc6e999261dddc44ffcf780a97ae7b0fa1ea61b26ddd57ab41fe383a926`, head 051b4c8.
- **NOT_RUN:** download of the artifact ZIP bytes by the author (the author's `gh` is unauthenticated). Download with `gh run download 38422383429 -n m2_qualification_amd64`.
- The implementation commit 4f7080b alone also ran m2 38421605190, m1 38421605215 and m0 38421605053 (all three success: the implementation alone passes the same gates, including both extra steps); the candidate adds only harness seeded defects on top of it.
