# M2-006 stage A revision 3 — GitHub Actions evidence at 2217e62 (and the failed m2 run at 9633ce3)

This is the second amd64 run of candidate 2217e62ddf42c42573346155727c983d937addc0 (push event; range fc147e0..2217e62). The author retrieved it with the GitHub CLI (`gh api repos/Kayyo321/cham3/actions/runs/<id>`, `…/jobs`, `…/artifacts`; `gh run view <id> --log`) and downloaded the artifact; it is not an independent review.

| Workflow run | Jobs | Result |
|---|---|---|
| [m2_checks 38584362113](https://github.com/Kayyo321/cham3/actions/runs/38584362113) | `qualify_amd64` (115922261243): GitHub-hosted amd64 runner, pinned Debian 13.7 container built from the unchanged Dockerfile, UID 1001, `CHAM3_M2_REQUIRE_CLEAN=1`; then **"Vault process protection, positive path (memlock limit raised, unprivileged)"**; then **"Restrictive reserve under a full filesystem (2 MiB tmpfs, unprivileged)"**; then **"Wrapper preservation on the tmpfs (unprivileged)"** | success, every step (`m2 qualification: PASS` with both lock checks PASS against the refreshed locks, suite_unit and suite_harness PASS; `vault protection positive path: PASS`; `restrictive reserve under a full filesystem: PASS`; wrapper cases `Ran 5 tests in 0.392s … OK`) |
| [m1_checks 38584362062](https://github.com/Kayyo321/cham3/actions/runs/38584362062) | static_and_desktop, privileged_capture, package_build | success, every step |
| [m0_checks 38584362077](https://github.com/Kayyo321/cham3/actions/runs/38584362077) | static_checks, image_build, package_build | success, every step |

The three runs were created by the push of 2217e62 at 2026-10-14T14:52:12Z (attempt 1 each); m1 completed at 14:54:51Z, m0 at 14:56:41Z, m2 at 15:13:28Z.

Artifact `m2_qualification_amd64`: id 11906127702, 44 files, 106 953 bytes, `sha256:1ed83981cfbe186c42969c96760b96f8bfc189dc2d348d5c78a777078de3937a`. The artifact listing reports this digest, and the author's download (`gh api repos/Kayyo321/cham3/actions/artifacts/11906127702/zip`, then `sha256sum`) gives the same digest. It holds the qualification directory `amd64_20261014T145342Z_1` (its `summary.txt` has the same PASS and NOT_AVAILABLE lines as the VM run; `identity.txt`: commit 2217e62…, tree `5f82bac0…`, `dirty_tracked 0`, Debian 13.7, kernel 6.17.0-1022-azure, `euid 1001`, `cargo_lock_sha256 9801453a…`, the same tree and lock digest as the VM run; `rust_naming.log`: `checked 63 Rust files, 72 paths: PASS`), both run directories (unit `unit_20261014T145441Z_1849`, inventory-exact against the same `test_inventory.txt`: 206 cargo tests per profile, format differential 5955, corpus `0ad8bc2b…d395e`; harness `harness_20261014T145917Z_4468`: `Ran 147 tests in 768.104s`, `OK`, against 384.6 s on the author's machine), `protection/vault_protection_report.txt` and `storage/reserve_full_disk_{release,debug}.txt`. Both storage reports are byte-identical to the author's VM run (`../m2_006a_r3_amd64_vm_2217e62/storage/`) and to revision 2's.

## The failed m2 run at 9633ce3 (the reason for the lock refresh)

| Workflow run | Result |
|---|---|
| [m2_checks 38577923955](https://github.com/Kayyo321/cham3/actions/runs/38577923955) | **failure** in "Qualify M2 Rust toolchain and run M2 suites (unprivileged)": `FAIL installed_packages_lock` and `FAIL downloaded_debs_lock`, `m2 qualification: FAIL`; every other check PASS, 206 + 206 cargo tests and `Ran 147 tests in 753.385s … OK`; the protection, full-filesystem and wrapper steps skipped |
| [m1_checks 38577923430](https://github.com/Kayyo321/cham3/actions/runs/38577923430), [m0_checks 38577923571](https://github.com/Kayyo321/cham3/actions/runs/38577923571) | success |

Created by the push of 9633ce3 at 2026-10-14T14:08:54Z; m2 completed at 14:28:55Z. Artifact id 11902319817, 41 files, 106 536 bytes, `sha256:1f589df6a09b43bf8d8162b77fa87fdccad3a40962e8c30509fe11634ff444dc` (download digest identical). Its two lock diffs change only libcurl:

~~~text
-libcurl3t64-gnutls:amd64=8.14.1-2+deb13u5
-libcurl4t64:amd64=8.14.1-2+deb13u5
+libcurl3t64-gnutls:amd64=8.14.1-2+deb13u7
+libcurl4t64:amd64=8.14.1-2+deb13u7
-19da93544615a4c9a3d7df473f5bf8ebca99442454622e9b0460168dadf7b8ae  libcurl3t64-gnutls_8.14.1-2+deb13u5_amd64.deb
-0dedddfe614fc4a4335c9167b73c64a50136edd76b7991aa851408d0795163eb  libcurl4t64_8.14.1-2+deb13u5_amd64.deb
+9475c591b338b81859ee6d06bf2a9fbb01b52f184c235afa7f6e7f1652136e55  libcurl3t64-gnutls_8.14.1-2+deb13u7_amd64.deb
+4f89bd3e7a33bbd8f7422817a8a8e2233c9e194fda9c331e33bf98705f525bfc  libcurl4t64_8.14.1-2+deb13u7_amd64.deb
~~~

The run's full `installed_packages.txt` and `downloaded_debs.sha256` are byte-identical to the author's no-cache image `2a0bcf342c57` and are the refreshed locks of a2d0ebd (owner-approved, ADR 0003 §2).

Positive-path protection report at 2217e62, from the artifact:

~~~text
memlock_limit Max locked memory         1073741824           1073741824           bytes
required_memlock_bytes 805306368
derivation_ms 377
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

Full-filesystem step at 2217e62 (release report from the artifact; the debug report differs only in its `profile` line):

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
~~~

Wrapper preservation step, from the job log:

~~~text
test_a_disk_without_the_paths_is_left_as_found ... ok
test_existing_root_and_filler_are_refused_and_preserved ... ok
test_symbolic_links_and_their_targets_are_refused_and_preserved ... ok
test_restored_pre_deletion_is_detected ... ok
test_unmodified_copy_preserves_existing_paths ... ok
Ran 5 tests in 0.392s
OK
~~~
