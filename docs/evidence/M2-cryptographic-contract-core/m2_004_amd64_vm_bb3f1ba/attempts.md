# M2-004 candidate attempts before bb3f1ba

The handoff gate requires a clean-clone qualification and green m2/m1/m0 CI at the final candidate. Two earlier commits did not meet it. Both are kept in the branch history, and the fixes are separate commits.

| Commit | What failed | Where | Fix |
|---|---|---|---|
| 0e4f993172019de8a55badbdd8a0a56943b89eb1 (implementation) | `run_static_checks.sh`: yamllint `.github/workflows/m2_checks.yml` line 10, 126 > 120 characters. `tests/run_tests.sh static`: 2 failures, `test_scripts_and_shebang_files_are_executable` (`tests/m2/protection/check_vault_protection.sh` was 100644) and `test_clean_clone_of_head_passes` (the same yamllint error) | Author VM clean clone. In CI the "Static checks" step failed in m0_checks 38339466012 (`static_checks` job) and m1_checks 38339465979 (`static_and_desktop` job), and their later steps were skipped; every other m0/m1 job succeeded | e5be2065d10c1f41bac27020540117175ff46d5b: comment wrapped, executable bit set |
| 0e4f993 | m2_checks 38339465992: the qualification step passed, but the "Vault process protection, positive path" step reported `unlocked_mappings 7`, `protection_check_failed`, exit 4. Protection itself applied: memlock 1 GiB, `dumpable 0`, `core_limit 0`, derivation 434 ms | GitHub Actions | bb3f1bad77080da34069af62c8d2cc390f2fcf7e: the probe judges mappings by the VM_LOCKED flag (smaps `Locked` is PSS for shared pages); `protection.txt` §4 |
| e5be2065d10c1f41bac27020540117175ff46d5b | m0_checks 38341057573 and m1_checks 38341057585 success. m2_checks 38341057571: qualification step success; the positive-path step failed with the old probe check, as at 0e4f993 | GitHub Actions | bb3f1ba (above) |

The vault's protection code (`core/vaultd/src/protection.rs`) is the same in all three commits. The qualification at 0e4f993 in this VM otherwise passed: every required check, unit 114+114, harness 133.
