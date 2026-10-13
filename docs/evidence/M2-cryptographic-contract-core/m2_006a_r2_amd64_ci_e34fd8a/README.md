# M2-006 stage A revision 2 — GitHub Actions evidence at e34fd8a

This is the second amd64 run of candidate e34fd8a0b97f5a606656ab87914d4d1b19517d7c (push event; range a9e47e4..e34fd8a). The author retrieved it with the GitHub CLI (`gh api repos/Kayyo321/cham3/actions/runs/<id>`, `…/jobs`, `…/artifacts`; `gh run view <id> --log`) and downloaded the artifact; it is not an independent review.

| Workflow run | Jobs | Result |
|---|---|---|
| [m2_checks 38481185159](https://github.com/Kayyo321/cham3/actions/runs/38481185159) | `qualify_amd64` (115583891147): GitHub-hosted amd64 runner, pinned Debian 13.7 container, UID 1001, `CHAM3_M2_REQUIRE_CLEAN=1`; then **"Vault process protection, positive path (memlock limit raised, unprivileged)"** with `--ulimit memlock=1073741824:1073741824`; then **"Restrictive reserve under a full filesystem (2 MiB tmpfs, unprivileged)"** with `--tmpfs /full-disk:rw,size=2m,…` (the journal reserve and the ledger on the full tmpfs); then **"Wrapper preservation on the tmpfs (unprivileged)"** | success, every step (qualification 22:58–23:21 UTC, `m2 qualification: PASS`, suite_unit and suite_harness PASS; protection 19 s; full filesystem 35 s; wrapper cases `Ran 5 tests in 0.465s … OK`) |
| [m1_checks 38481185180](https://github.com/Kayyo321/cham3/actions/runs/38481185180) | static_and_desktop, privileged_capture, package_build | success, every step |
| [m0_checks 38481185314](https://github.com/Kayyo321/cham3/actions/runs/38481185314) | static_checks, image_build, package_build | success, every step |

The three runs were created by the push of e34fd8a at 2026-10-13T22:57:27Z (attempt 1 each); m2 completed at 23:21:52Z, m1 at 23:00:08Z, m0 at 23:01:30Z.

Artifact `m2_qualification_amd64`: id 11857892996, 44 files, 106 575 bytes, `sha256:d20cc30e5ef855f61e841e7fe8cfcaf93495bbd00e2e847736292150d3409173`. The upload step and the artifact listing report this digest, and the author's download (`gh api repos/Kayyo321/cham3/actions/artifacts/11857892996/zip`, then `sha256sum`) gives the same digest. It holds the qualification directory `amd64_20261013T225834Z_1` (its `summary.txt` has the same 21 PASS and 3 NOT_AVAILABLE lines as the VM run; `identity.txt`: commit e34fd8a…, tree `ae42f87d…`, `dirty_tracked 0`, Debian 13.7, kernel 6.17.0-1022-azure, `euid 1001`, `cargo_lock_sha256 9801453a…`, the same tree and lock digest as the VM run; `rust_naming.log`: `checked 63 Rust files, 72 paths: PASS`), both run directories (unit `unit_20261013T225950Z_1849`, inventory-exact against the same `test_inventory.txt`: 202 cargo tests per profile, format differential 5955; harness `harness_20261013T230519Z_4460`: `Ran 147 tests in 891.028s`, `OK`, against 368.6 s on the author's machine), `out/m2/protection/vault_protection_report.txt` and `out/m2/storage/reserve_full_disk_{release,debug}.txt`.

Positive-path protection report from the job log:

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

Full-filesystem step, from the job log (identical for the debug profile apart from the `profile` line). Every line, `filler_bytes 1036288`, the sequence numbers and `journal_records 37` included, is unchanged from the f4b0628 run: revision 2 changed how a ledger failure is recorded (the run marker `unverifiable`; the validity file emptied when it cannot be unlinked) but not the probe's path, whose unlink succeeds (`ledger_validity_removed`, `previous_ledger missing`). Both report files are byte-identical to the author's VM run (`../m2_006a_r2_amd64_vm_e34fd8a/storage/`), the mount line included (both environments run as UID/GID 1001).

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

Wrapper preservation step, from the job log (the five cases of `test_m2_reserve_probe.py -k wrapper`, run on the same 2 MiB tmpfs with the release reserve probe):

~~~text
test_a_disk_without_the_paths_is_left_as_found ... ok
test_existing_root_and_filler_are_refused_and_preserved ... ok
test_symbolic_links_and_their_targets_are_refused_and_preserved ... ok
test_restored_pre_deletion_is_detected ... ok
test_unmodified_copy_preserves_existing_paths ... ok
Ran 5 tests in 0.465s
OK
~~~
