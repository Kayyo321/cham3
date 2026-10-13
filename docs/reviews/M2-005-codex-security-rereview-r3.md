# M2-005 independent review, revision 3

Verdict: **CHANGES_REQUESTED**. Reviewer: Codex, independent of implementation. Date: 2026-10-13. No implementation edits, merge, architecture change or owner decision made. Publish only this record on `review/m2-005-r3-security`, based on main `9cf882503848f8c6cc4938a588cc9fc062a9392d`.

The three original R2 findings are resolved at their reported locations. A separately reproduced, pre-existing data-loss defect in the explicitly requested qualification wrapper prevents approval of that path. It is not a regression introduced by revision 3 and is not claimed as an application-reachable privilege escalation.

## Revisions and scope — PASS

Base: `959524cf52212d9db260457b1c458fe6f95b59f7`. Candidate: `be2415291bf5bcf9dad8df323bf971bd193d5120`. Documentation HEAD: `30f77b270f04952fb9211b59ac80d8c12a32b40c`. Initial local status and stash list were empty; checkout was brought to the fetched implementation branch as requested. Main checkout remains clean at documentation HEAD. Test clone `/tmp/cham3_review_r3/candidate` was cloned with `git clone --no-hardlinks /home/dev/cham3 /tmp/cham3_review_r3/candidate` and detached at the candidate, owned by UID 1001. Qualification required a clean clone.

Read AGENTS.md, PROJECT.md, workflow/roles/testing/security guidance, ADR 0006 section 1, ADR 0008 section 1, the R3 finding resolutions and handoff, and M2-004 completion handoff. Previous independent record: `review/m2-005-r2-security` at `d300b609ccde3eaf1fe8e909f99e78707c4b6c5a`, `docs/reviews/M2-005-codex-security-rereview-r2.md`, SHA-256 `28b4623121c65093d1fdc98963262b30e68da251417a868d6cea6c0f82b17435`.

`git diff --stat 959524cf52212d9db260457b1c458fe6f95b59f7 be2415291bf5bcf9dad8df323bf971bd193d5120`: exactly seven files, 745 insertions / 109 deletions:

- core/registryd/src/bin/cham3_reserve_probe.rs
- core/registryd/src/journal.rs
- core/registryd/tests/registry_operations.rs
- tests/m2/harness/test_m2_registry_policy.py
- tests/m2/harness/test_m2_reserve_probe.py
- tests/m2/storage/check_reserve_full_disk.sh
- tests/m2/test_inventory.txt

`git diff --name-only be2415291bf5bcf9dad8df323bf971bd193d5120 30f77b270f04952fb9211b59ac80d8c12a32b40c`: all changes under docs/. Dependencies, Cargo.lock, manifests, workflows, specification, ADRs, oracle/vector inputs, registry.rs, lineage.rs, policyd, vaultd, identityd, contractd and contract_format unchanged. M2-004 completion handoff accurately records the earlier APPROVE and its staged, in-process scope.

## Required change — M2-005-R3-01, Medium

Location: `tests/m2/storage/check_reserve_full_disk.sh:40`.

The wrapper unconditionally executes `rm -rf "$disk/cham3_reserve_probe_root" "$disk/cham3_reserve_probe_filler"` before invoking the binary, which performs filesystem validation. Its second argument selects disk; it does not establish ownership of these paths. A non-root invocation on a normal overlay directory containing an existing root/sentinel and filler deletes both, then refuses the filesystem. The new binary preservation behavior and wrapper's new leftover assertions cannot recover or detect the pre-deleted data.

Reproduced with the actual unmodified wrapper and Cargo invocation in a disposable non-root container. The probe reports `refusing_to_fill / is overlay, not tmpfs`, exit 3; wrapper exits 1; both existing paths are absent. Parent independently repeated the worker reproduction. Only synthetic container data was used. Exact reproduction scripts and output are embedded below. The driver returns 0 because it asserts reproduction of the defect; the preservation acceptance check is **FAIL**.

This line is unchanged from baseline 959524c. R2 did not identify the wrapper's pre-cleaning; its binary finding is nevertheless fixed. This new finding blocks the broader R3 acceptance path because the user explicitly requested review of the wrapper and preservation of existing paths. No remote exploit or lower-privilege attacker control is established.

Required: remove unconditional deletion of pre-existing paths; refuse them untouched or exclusively create a private per-invocation namespace and clean only artifacts owned by that invocation. Add wrapper-level preservation tests for existing root, filler and symlink targets on overlay and bounded tmpfs; keep the successful fresh 2 MiB run and leftover checks passing.

## Prior findings and design judgments

- R2-01 **PASS**: startup repeats parent/root directory synchronization; reopening synchronizes user directories; each creation attempt synchronizes users. Original four failing directory tests and the two-user control pass; the control still observes exactly two users-directory syncs after restart. New tests cover repeated startup failure, isolation of one unusable user from another, and recreated reserve with repeated directory-sync failure across restarts.
- UID-entry argument **accepted** under the private owned-store/trusted-filesystem assumptions: the journal is created only after users synchronization succeeds, an empty user directory does not yield a journal, and every subsequent creation attempt repeats synchronization. Directory-sync cost is **2 + N** at startup for N replayed users, plus reserve-file synchronization where applicable. “Three per start” describes the one-user case; it must not also be added to another per-user directory count.
- R2-02 **PASS**: durable reserve size is separated from observed length. Creation retries allocation/sync; reopening full-length reserves syncs before counting them; failed sync remains pending. The two original reserve-sync tests and partial-reserve control pass. New cases cover failure at startup and next append then recovery, oversized reserve, hostile no-op allocation, and reserve release/failed replenishment across restart.
- Pending-at-start choice **accepted** against ADR 0008 section 1: ordinary append cannot proceed until the full reserve is synchronized; restrictive writes remain possible. Failure of the user-directory sync itself makes the user unusable.
- Oversized reserve characterization: a 1 MiB + 4096-byte reserve starts pending, then a successful sync allows counting the required 1 MiB without growing or truncating the actual file. This is safe minimum-capacity accounting, not an exact-size format guarantee.
- Hostile journal_io_t characterization: an adapter returning Ok without writing can make creation trust a nonexistent reserve; reopening detects pending state and ordinary replenishment refuses. The test explicitly asserts this boundary rather than proving safety against a malicious adapter. The embedding process supplies trusted I/O and can already fake writes/fsync; real allocation writes all bytes and syncs. No untrusted application path to adapter replacement was established.
- R2-03 **PASS at the direct binary**: mode/filesystem refusal precedes creation, root/filler creation is exclusive, existing objects are preserved, cleanup owns only newly created root. The separate wrapper defect above remains.
- R01, R03, R02/error vocabulary, ordinary/full-disk reserves, layer-7/format differential behavior and earlier passing M2-004 items: **PASS, intact**. M2-004 source is unchanged and qualification integrations pass. Its original 17/12 independent tests were **NOT_RUN this revision**; their immutable R2 evidence remains the basis for those specific prior results.

## Environment and commands

Home computer: Debian 13.7, Linux 7.1.13+deb13-amd64, x86_64; UID/GID 1001:1001 (dev). Docker 26.1.5+dfsg1; gh 2.102.0. Network unrestricted, fetch and gh succeeded. Docker invoked with `sudo -n docker`, containers run as 1001:1001, no --privileged or added capabilities, no --record. Pinned image `cham3-m2-independent-r3`, SHA-256 `4f4f684d76f7c3c299932846ebe66765204c7e15c9414d77922d15ff561bb327`, linux/amd64, Rust 1.85.1. Qualification uses container-only nofile 1024:1024 for Valgrind. Protection separately uses memlock 1073741824:1073741824. Full filesystem uses a fresh 2 MiB tmpfs with UID/GID 1001, mode 0700. No ordinary host protection was disabled.

Commands below use the test clone as cwd; output paths under `/tmp/cham3_review_r3` are retained locally. External reviewer sources never enter the candidate checkout.

```sh
sudo -n docker build --network host -f build/m2/dev_container.dockerfile -t cham3-m2-independent-r3 build/m2
sudo -n docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 -v /tmp/cham3_review_r3/candidate:/work -w /work cham3-m2-independent-r3 bash build/scripts/qualify_m2_rust.sh
sudo -n docker run --rm --user 1001:1001 -e HOME=/tmp/cham3_home -v /tmp/cham3_review_r3/candidate:/work -w /work cham3-m2-independent-r3 bash -c 'bash build/scripts/run_static_checks.sh > /work/out/static_checks_review.log 2>&1; echo static_checks_exit=$?; sh tests/run_tests.sh static > /work/out/out_static_tests_review.log 2>&1; echo static_tests_exit=$?'
sudo -n docker run --rm --user 1001:1001 --ulimit memlock=1073741824:1073741824 -e HOME=/tmp/cham3_home -v /tmp/cham3_review_r3/candidate:/work -w /work cham3-m2-independent-r3 sh tests/m2/protection/check_vault_protection.sh out/m2/protection
sudo -n docker run --rm --user 1001:1001 --tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700 -e HOME=/tmp/cham3_home -v /tmp/cham3_review_r3/candidate:/work -w /work cham3-m2-independent-r3 sh tests/m2/storage/check_reserve_full_disk.sh out/m2/storage
```

### Verification ledger

| Check | Result | Evidence/result |
| --- | --- | --- |
| Pinned build | PASS | docker_build.log; image digest above |
| Clean amd64 qualification | PASS | qualification.log; 169 debug + 169 release unit tests |
| Provider oracle | PASS | 7/7 |
| ECR oracle / scenarios | PASS | 19/96/18; 23 scenarios, 352 steps |
| Format differential | PASS | 5955 cases, zero mismatches; corpus 0ad8bc2b1e8d39224de699db64aa45ca7adcf70117c555f35b57c537471d395e |
| Vault object oracle | PASS | 38 checks |
| Harness | PASS | 142 tests; 21 identity/vault, 41 registry/policy seeded defects; probe seven tests/two seeded defects |
| Static checks | PASS | shellcheck 26, Python 69, YAML 5, secret check; out/static_checks_review.log |
| Static tests | PASS | 82 tests; out/out_static_tests_review.log |
| Protection positive path | PASS | own run, locked-memory limit 1 GiB; required 805306368 bytes; dumpable/core 0; unlocked mappings 0; four dontdump mappings |
| Full-filesystem wrapper positive | PASS | own debug/release; 37 records, restrictive seq 35/36, depleted admission refused, refill seq 37, 1 MiB allocated; leftovers absent |
| Original edge tests | PASS | 10 debug + 10 release |
| Original own tests | PASS | 9 debug + 9 release |
| Original filesystem_full_reserve | PASS | one debug and one release; fresh 2 MiB tmpfs per profile |
| New journal tests | PASS | seven debug + seven release, source/logs below |
| Extended direct-probe matrix | PASS | 32 cases plus two real filler-write EFBIG failures, both profiles |
| Seed discrimination | PASS | seven repository tests; pristine preserves, adopt-existing and cleanup-on-refusal mutants delete |
| Wrapper existing-path preservation | FAIL | M2-005-R3-01; independently repeated |
| Actual power loss / deployed IPC / anchors | NOT_RUN | staged work, no system-enforcement claim |
| Miri / sanitizers / fuzzing | NOT_RUN | not part of executed qualification; unavailable/not executed |
| Local M0/M1 full suites | NOT_RUN | exact-head successful CI checked instead |

Successful run directories: `candidate/out/m2/runs/unit_20261013T173421Z_1844`, `candidate/out/m2/runs/harness_20261013T173557Z_4338`, `candidate/out/m2/qualification/amd64_20261013T173356Z_1`.

Original external tests use the R2 package at `adversarial005/core/reviewer`, exact original registry.rs SHA-256 `157eb1c69056e928794193edc5eda4d670b28871a16e01bcb605fa947ea0e4a2`. Mounted candidate read-only at /work, external package writable at /evidence, cwd /evidence, HOME=/tmp/cham3_home, CARGO_TARGET_DIR=/evidence/target, same non-root image. Command: `cargo test --manifest-path core/reviewer/Cargo.toml --locked --offline FILTER -- --nocapture --test-threads=1`, repeated with `--release`, FILTER `::edge_`, `::own_`, or `filesystem_full_reserve`. Filesystem filter additionally uses a fresh /full-disk tmpfs as above for each profile. New tests append the embedded source to the original registry test module; runner cwd /evidence/runner under the temporary artifact directory, CARGO_TARGET_DIR=/target (external target mounted), filter `::r3_`, both profiles.

Attempt accounting: initial external-package copy lacked .cargo configuration and failed offline dependency resolution (`sha2` not found); copying the unchanged configuration fixed the fixture. Initial full-release test reused debug's tmpfs and failed AlreadyExists; a fresh container passed. Initial wrapper reproduction used the wrong cwd and failed Cargo discovery before deletion; corrected cwd /work reproduced the actual defect. Failed full-release and first-wrapper logs are retained. These are fixture errors, not product failures.

Direct-probe matrix covers existing roots on overlay in all three modes, fresh allocate-only success, wrong filesystem/mode refusal, missing parents, symlink roots and unchanged targets, oversized 32 MiB tmpfs, existing filler on 2 MiB tmpfs, positive full-disk cleanup, post-creation allocation failure on 512 KiB tmpfs, and filler EFBIG on 8 MiB tmpfs with child-only RLIMIT_FSIZE 1 MiB. Existing content/mode/inode/mtime preservation is checked. Missing-parent full-disk exit 2 rather than 3 reflects canonicalization failure; it creates nothing. Concurrent same-UID ancestor replacement, kill/panic cleanup and unmount failure are NOT_RUN. Static pre-existing filler.sync_all error propagation can bypass filler unlink; it was not runtime-injected and is not claimed as a confirmed additional finding. Cleanup is best-effort.

## CI comparison — PASS

Exact candidate head and every job/step successful for [m2_checks 38435225455](https://github.com/Kayyo321/cham3/actions/runs/38435225455), [m1_checks 38435225444](https://github.com/Kayyo321/cham3/actions/runs/38435225444), [m0_checks 38435225490](https://github.com/Kayyo321/cham3/actions/runs/38435225490). Inspected metadata and logs via `gh run view RUN --repo Kayyo321/cham3 --json headSha,conclusion,jobs` and `gh run view RUN --repo Kayyo321/cham3 --log`. Protection and full-filesystem steps pass; own independent runs also pass.

`gh run download 38435225455 --repo Kayyo321/cham3 -n m2_qualification_amd64 --dir /tmp/cham3_review_r3/ci_artifact`; archive fetched with `gh api repos/Kayyo321/cham3/actions/artifacts/11834225727/zip > /tmp/cham3_review_r3/ci_artifact.zip`. SHA-256 matches `4168e9190cdef6e6fff0187e7a76099beded24aac32902d34d032f6dc984eef6`. Test inventories/counts, provider output, format coverage/corpus and harness inventory match; unit/harness/qualification summaries and ECR/vault logs are byte-identical. Reserve reports match excluding mount UID/GID. Comparison output embedded below.

## Codex Security — PASS (scan completed)

Invoked `codex-security:security-diff-scan`, Codex Security plugin **0.1.32**, scan `599fb67c-f6bf-4524-852b-b0fc46c44bbf`, exact base/head above. Four surfaces complete, all seven changed files accounted for, no deferred candidates, **zero introduced diff-security findings**. The baseline wrapper defect is retained explicitly as rejected for introduced-vulnerability reporting, while remaining an ordinary acceptance-blocking review finding. These are different judgments, not an assertion that the wrapper is safe.

Preflight command: `python3 /home/dev/.codex/plugins/cache/openai-curated-remote/codex-security/0.1.32/scripts/config_preflight.py --profile security_diff_scan --cwd /home/dev/cham3 --runtime-check delegation_available=true --runtime-check goal_tools_available=true`; three capability checks pass. Fresh independent preflight, architecture and bounded probe workers were used. Previously authorized chmod g-w /home/dev/.codex tightened artifact-parent permissions to 0755. No further configuration changes. Applicable SECURITY.md inventory was empty.

Plugin calls: start_codex_security_prompt_only_scan (once); get_codex_security_scan_context; update_codex_security_scan_progress; prepare_codex_security_review_items/list_codex_security_review_items; record_codex_security_discovery_candidates (empty list); record_codex_security_scan_draft (partial retained threat model, then final complete coverage); save artifact calls; complete_codex_security_scan; get_codex_security_completed_scan. Inventory selected two production Rust files; manual review covered all seven files and bounded supporting call paths. No candidate required vulnerability validation/attack-path escalation. Tests and shell commands are documented in this record and embedded runners.

An initial final draft used an invalid explicitExclusions field and was not saved. Completion then returned: “The latest saved scan draft is incomplete; continue the scan before completing it.” The response stopped as the tool required. On user continuation, corrected final draft succeeded, then completion validated/indexed the scan at 2026-10-13T17:54:58.557149Z; completed documents were retrieved. This was a reviewer reporting error, now resolved, not a remaining BLOCKED gate.

Measured plugin usage (codex_rollout, four threads): input 10203546, cached input 9912320, output 39226, reasoning output 4419, total 10242772 tokens. These are the plugin's aggregate counters, including cached context. Daybreak advisory access was not granted; separately reported as non-gating, not a failed local scan. Retained architecture model's runtime NOT_RUN refers to that worker's static assignment; parent runtime verification above did run.

Next owner: implementer, to repair M2-005-R3-01 and return the bounded wrapper fix/tests for independent review. M2-008's required security gate remains a future task; this scoped completion does not certify staged enforcement or whole-system security.

## Evidence digests

Paths are local evidence retained on the review host. Embedded sources and outputs below make the new reproductions portable; original tests remain in the immutable prior review.

| Evidence | SHA-256 |
| --- | --- |
| /tmp/cham3_review_r3/ci_comparison.txt | db8ba45ccf2318a078ffe9ffc768a97931b2e3b7e2bfd7d28c52ba5812022085 |
| /tmp/cham3_review_r3/wrapper_parent.log | e05741083aafc7e4c62862cad37a3d722baa5d34786f9b6c196b99eaa11dc98d |
| /tmp/cham3_review_r3/probe_review/matrix.log | c3f5ef88127a2faf21aa125145ceea9e548c47e246f0b797959916140fe4fd44 |
| /tmp/cham3_review_r3/probe_review/filler_failure.log | 452d52851b73dcb72092232ae0d109a993bb75cbda07ab9b41d90d3a0a9a8d2c |
| /tmp/cham3_review_r3/probe_review/seeded.log | 67464e520ea0bce6e2160f2fecc7e70bc0b7092fb4f1b92184294248e8fc4cd2 |
| /tmp/cham3_review_r3/probe_review/run_matrix.sh | 9f07ac6d1788056d8b4c2df18cf65065193b7dd02ad73ee6acda8335d55e4588 |
| /tmp/cham3_review_r3/probe_review/matrix.py | 6bbec951e50a991f7313a90cadc471b7774306f28c1e54e5f44c02e1dcea4faf |
| /tmp/cham3_review_r3/probe_review/run_filler_failure.sh | fde57cc5995b8c62286be1d813cb619d5a7ab52dc0ea67cdcf0ba4fdd624a9d7 |
| /tmp/cham3_review_r3/probe_review/filler_failure.py | 3bde2cb286da58d49e60f36e66a670ccdaa948441ba4b4834b93c971a6d64628 |
| /tmp/cham3_review_r3/probe_review/run_seeded.sh | 32081b52b8d89d3ba9290b48e2d022b8657fb3186ab36d483244382d2edc16f7 |
| /tmp/cham3_review_r3/probe_review/seeded_run.py | 148f39c9b135c04ae9879e5bbcbfe53dbc3e0cd3186ca4f4d8fd12f9aff79ca3 |
| /tmp/cham3_review_r3/probe_review/run_wrapper.sh | cbb9e4023483389a34b419a9764682662931f48373e120e3c6ba0f2e4ed7dd69 |
| /tmp/cham3_review_r3/probe_review/wrapper_run.py | b498f76e3ba0c2f7edf46ba8bb9f1eb30fa442e1e48bc39e516552c856046c12 |
| /tmp/codex-security-artifacts-7482a338c37121a2898e96fe43fbbe528bc25f67ca514a536cda0d5b5a87da3d/artifacts/r3_journal_tests.rs | 2b21f9118f9d24dabc2cd2d844ccf6e7679ac76cb7edc0f8ee09e0eff92cfe49 |
| /tmp/codex-security-artifacts-7482a338c37121a2898e96fe43fbbe528bc25f67ca514a536cda0d5b5a87da3d/artifacts/r3_debug.log | a7f77b2b489c854eb6c38342bf42da6d381ce7ecd22dd93822b41676d8be2d46 |
| /tmp/codex-security-artifacts-7482a338c37121a2898e96fe43fbbe528bc25f67ca514a536cda0d5b5a87da3d/artifacts/r3_release.log | a2c8b13f464646c0030748ade5ad774099786dc30b6b2e1ca1e9f97d8d40ed94 |
| /tmp/cham3_review_r3/qualification.log | 57615decf5e7a23b63f99ea3e61ce2fac53812de006ba93bcc0bd23682b623c1 |
| /tmp/cham3_review_r3/docker_build.log | 555cde69f74ab95ee8d2e15cf1d8d7c109bdbbb54652b5f0f673a21b1aeb0583 |
| /tmp/cham3_review_r3/protection.log | b53d8389993e82631d4421bca2448f7ffbd646ea30ef5f63ee4c0eea53bd486f |
| /tmp/cham3_review_r3/reserve_full.log | 4c3a4418bca2e2e6ab602d8789799f4dc9cc102f43fe0ab2fad6f3a517467402 |
| /tmp/cham3_review_r3/ci_artifact.zip | 4168e9190cdef6e6fff0187e7a76099beded24aac32902d34d032f6dc984eef6 |
| /tmp/cham3_review_r3/candidate/out/static_checks_review.log | 3b060ae8c6cbd18697388da94cc0ad5bef0416d593b2b86f18ab503fc87b7cd0 |
| /tmp/cham3_review_r3/candidate/out/out_static_tests_review.log | e3f334fd28fe0cc2dd8de17f99ea2adfdb2414e25a4c42a02e6835bd520722ef |
| /tmp/cham3_review_r3/adversarial005/edge_debug.log | 66297a8d6b906ea3bef3be8e77fa43c6d41815cd431617996572d8815e7b152e |
| /tmp/cham3_review_r3/adversarial005/edge_release.log | 19a4bbe845d64f2b027dd621116d6beb9095754721fa23eb890e83dbc99e0bd2 |
| /tmp/cham3_review_r3/adversarial005/full_debug.log | 6c6948a35bf2aec02eb05bc45b39ec20adb07bf43b24547e7911cd59751bc61e |
| /tmp/cham3_review_r3/adversarial005/full_release.log | 7439ce81f947ccfad19ee3a8947924cf67096c518b4a03fbc61aa345d10c773d |
| /tmp/cham3_review_r3/adversarial005/full_release_fresh.log | 41f3d5c9d6906e95fad40c2df61a50b315bd6d2c10119fc054c93a9a1e60aee6 |
| /tmp/cham3_review_r3/adversarial005/own_debug.log | 9827ad6e526c7610f8baba9c6643ba16808a8e8e95be0e27ecc926e9db126639 |
| /tmp/cham3_review_r3/adversarial005/own_release.log | b6ffe18181a882894b865bd353707582f203ad17d5726994d5a563e6a1e98782 |
| /home/dev/.codex/state/plugins/codex-security/scans/cham3/be2415291bf5bcf9dad8df323bf971bd193d5120_20261013T173327Z_t9si_h_n/scan-manifest.json | 7dd0dbf4f23a573b92c0a55c6cbfd1eb92903cc6b4f0a38a46eb827a4bf5c3ab |
| /home/dev/.codex/state/plugins/codex-security/scans/cham3/be2415291bf5bcf9dad8df323bf971bd193d5120_20261013T173327Z_t9si_h_n/coverage.json | 26c0ac3f2dfed66a0927ab99c73c7c1369643971502c0fb595a2e17a6676f621 |
| /home/dev/.codex/state/plugins/codex-security/scans/cham3/be2415291bf5bcf9dad8df323bf971bd193d5120_20261013T173327Z_t9si_h_n/findings.json | 8ebf31fc1209d45196534d2f8679fa1bef78ad70857eac103426e1880b89587e |
| /home/dev/.codex/state/plugins/codex-security/scans/cham3/be2415291bf5bcf9dad8df323bf971bd193d5120_20261013T173327Z_t9si_h_n/report.md | e7d8ade0752ba352d59c85bc8dfae64005950063c38e840a4c1a9c02ba791ee7 |
| /home/dev/.codex/state/plugins/codex-security/scans/cham3/be2415291bf5bcf9dad8df323bf971bd193d5120_20261013T173327Z_t9si_h_n/threatmodel.md | 5b66b751a6884ff8c7c7ccaf6572e0ae1cdd741b4a3742affce145a8ed4db93c |
| /home/dev/.codex/state/plugins/codex-security/scans/cham3/be2415291bf5bcf9dad8df323bf971bd193d5120_20261013T173327Z_t9si_h_n/exports/results.sarif | 21c30fdde4867b6793d691d870c212b96328bac11f8ea62508307dcb12f8341c |

## Reproduction sources and captured output

### ci_comparison.txt

```text
cargo_test_debug.json: PASS exact ['tests', 'total_tests', 'result', 'exit_status']; total_tests=169
cargo_test_release.json: PASS exact ['tests', 'total_tests', 'result', 'exit_status']; total_tests=169
format_differential.json: PASS exact ['cases', 'coverage', 'distinct_outcomes', 'corpus_sha256', 'mismatch_count', 'seed', 'result']; total_tests=5955
provider_oracle.json: PASS exact ['oracle_output', 'probe_output', 'result']; total_tests=n/a
python_harness.json: PASS exact ['tests', 'total_tests', 'result', 'exit_status']; total_tests=142
ecr_vector_oracle.log: PASS byte-identical
vault_object_oracle.log: PASS byte-identical
unit summary: PASS byte-identical
harness summary: PASS byte-identical
qualification summary: PASS byte-identical
reserve_full_disk_debug.txt: PASS identical excluding mount UID/GID
reserve_full_disk_release.txt: PASS identical excluding mount UID/GID
```

### wrapper_parent.log

```text
COMMAND ['sh', '/work/tests/m2/storage/check_reserve_full_disk.sh', '/tmp/wrapper_out', '/tmp/wrapper_synthetic_disk']
profile release
refusing_to_fill / is overlay, not tmpfs
exit_status 3

    Finished `release` profile [optimized] target(s) in 0.03s

wrapper_exit_status 1
existing_root_exists False
existing_filler_exists False
CONFIRMED: wrapper deletes both pre-existing synthetic paths before overlay refusal
```

### matrix.log

```text
[
  {
    "profile": "debug",
    "case": "existing_overlay_full-disk",
    "command": [
      "/work/out/m2/storage/target/debug/cham3_reserve_probe",
      "/tmp/independent_probe_kli_8lu_/existing_full-disk",
      "full-disk"
    ],
    "status": 3,
    "stdout": "refusing_to_fill / is overlay, not tmpfs\n",
    "stderr": "",
    "preserved": true
  },
  {
    "profile": "debug",
    "case": "existing_overlay_allocate-only",
    "command": [
      "/work/out/m2/storage/target/debug/cham3_reserve_probe",
      "/tmp/independent_probe_kli_8lu_/existing_allocate-only",
      "allocate-only"
    ],
    "status": 3,
    "stdout": "refusing_to_fill /tmp/independent_probe_kli_8lu_/existing_allocate-only exists; the probe uses only a root it created and leaves other paths alone\n",
    "stderr": "",
    "preserved": true
  },
  {
    "profile": "debug",
    "case": "existing_overlay_unknown",
    "command": [
      "/work/out/m2/storage/target/debug/cham3_reserve_probe",
      "/tmp/independent_probe_kli_8lu_/existing_unknown",
      "unknown"
    ],
    "status": 3,
    "stdout": "refusing_to_fill unknown mode unknown\n",
    "stderr": "",
    "preserved": true
  },
  {
    "profile": "debug",
    "case": "fresh_overlay_allocate-only",
    "command": [
      "/work/out/m2/storage/target/debug/cham3_reserve_probe",
      "/tmp/independent_probe_kli_8lu_/fresh",
      "allocate-only"
    ],
    "status": 0,
    "stdout": "reserve_bytes 1048576\nreserve_allocated_bytes 1048576\nreserve_promised_bytes 1048576\nreserve_preallocated\nreserve_verified\n",
    "stderr": "",
    "cleaned_or_never_created": true
  },
  {
    "profile": "debug",
    "case": "fresh_overlay_full-disk",
    "command": [
      "/work/out/m2/storage/target/debug/cham3_reserve_probe",
      "/tmp/independent_probe_kli_8lu_/fresh",
      "full-disk"
    ],
    "status": 3,
    "stdout": "refusing_to_fill / is overlay, not tmpfs\n",
    "stderr": "",
    "cleaned_or_never_created": true
  },
  {
    "profile": "debug",
    "case": "fresh_overlay_unknown",
    "command": [
      "/work/out/m2/storage/target/debug/cham3_reserve_probe",
      "/tmp/independent_probe_kli_8lu_/fresh",
      "unknown"
    ],
    "status": 3,
    "stdout": "refusing_to_fill unknown mode unknown\n",
    "stderr": "",
    "cleaned_or_never_created": true
  },
  {
    "profile": "debug",
    "case": "missing_parent_allocate-only",
    "command": [
      "/work/out/m2/storage/target/debug/cham3_reserve_probe",
      "/tmp/independent_probe_kli_8lu_/missing/root",
      "allocate-only"
    ],
    "status": 3,
    "stdout": "refusing_to_fill cannot create /tmp/independent_probe_kli_8lu_/missing/root: No such file or directory (os error 2)\n",
    "stderr": ""
  },
  {
    "profile": "debug",
    "case": "missing_parent_full-disk",
    "command": [
      "/work/out/m2/storage/target/debug/cham3_reserve_probe",
      "/tmp/independent_probe_kli_8lu_/missing/root",
      "full-disk"
    ],
    "status": 2,
    "stdout": "check_failed io error No such file or directory (os error 2)\n",
    "stderr": ""
  },
  {
    "profile": "debug",
    "case": "missing_parent_unknown",
    "command": [
      "/work/out/m2/storage/target/debug/cham3_reserve_probe",
      "/tmp/independent_probe_kli_8lu_/missing/root",
      "unknown"
    ],
    "status": 3,
    "stdout": "refusing_to_fill unknown mode unknown\n",
    "stderr": ""
  },
  {
    "profile": "debug",
    "case": "root_symlink_allocate-only",
    "command": [
      "/work/out/m2/storage/target/debug/cham3_reserve_probe",
      "/tmp/independent_probe_kli_8lu_/symlink",
      "allocate-only"
    ],
    "status": 3,
    "stdout": "refusing_to_fill /tmp/independent_probe_kli_8lu_/symlink exists; the probe uses only a root it created and leaves other paths alone\n",
    "stderr": "",
    "preserved": true
  },
  {
    "profile": "debug",
    "case": "root_symlink_full-disk",
    "command": [
      "/work/out/m2/storage/target/debug/cham3_reserve_probe",
      "/tmp/independent_probe_kli_8lu_/symlink",
      "full-disk"
    ],
    "status": 3,
    "stdout": "refusing_to_fill / is overlay, not tmpfs\n",
    "stderr": "",
    "preserved": true
  },
  {
    "profile": "debug",
    "case": "root_symlink_unknown",
    "command": [
      "/work/out/m2/storage/target/debug/cham3_reserve_probe",
      "/tmp/independent_probe_kli_8lu_/symlink",
      "unknown"
    ],
    "status": 3,
    "stdout": "refusing_to_fill unknown mode unknown\n",
    "stderr": "",
    "preserved": true
  },
  {
    "profile": "debug",
    "case": "32m_tmpfs_refused",
    "command": [
      "/work/out/m2/storage/target/debug/cham3_reserve_probe",
      "/oversize/root",
      "full-disk"
    ],
    "status": 3,
    "stdout": "refusing_to_fill tmpfs of 33554432 bytes exceeds the 16777216-byte limit\n",
    "stderr": ""
  },
  {
    "profile": "debug",
    "case": "existing_filler_2m_tmpfs",
    "command": [
      "/work/out/m2/storage/target/debug/cham3_reserve_probe",
      "/full-disk/root",
      "full-disk"
    ],
    "status": 3,
    "stdout": "reserve_bytes 1048576\nreserve_allocated_bytes 1048576\nreserve_promised_bytes 1048576\nreserve_preallocated\ntmpfs_mount /full-disk\nrefusing_to_fill /full-disk/cham3_reserve_probe_filler exists; the probe fills only through a file it created\n",
    "stderr": "",
    "preserved": true
  },
  {
    "profile": "debug",
    "case": "2m_tmpfs_full_disk_positive",
    "command": [
      "/work/out/m2/storage/target/debug/cham3_reserve_probe",
      "/full-disk/root",
      "full-disk"
    ],
    "status": 0,
    "stdout": "reserve_bytes 1048576\nreserve_allocated_bytes 1048576\nreserve_promised_bytes 1048576\nreserve_preallocated\ntmpfs_mount /full-disk\nfiller_bytes 1044480\nrestrictive_append_on_full_filesystem ok seq=35\nreserve_bytes_after_release 0\nadmission_refused reserve_depleted\nsecond_restrictive_append ok seq=36\nappend_after_space_returned ok seq=37\nreserve_bytes 1048576\nreserve_allocated_bytes 1048576\nreserve_promised_bytes 1048576\nreserve_preallocated\nreserve_replenished\njournal_records 37\njournal_chained\nreserve_verified\n",
    "stderr": ""
  },
  {
    "profile": "debug",
    "case": "failure_after_exclusive_creation_512k",
    "command": [
      "/work/out/m2/storage/target/debug/cham3_reserve_probe",
      "/too-small/root",
      "allocate-only"
    ],
    "status": 2,
    "stdout": "check_failed store error io\n",
    "stderr": "",
    "created_root_cleaned_after_store_failure": true
  },
  {
    "profile": "release",
    "case": "existing_overlay_full-disk",
    "command": [
      "/work/out/m2/storage/target/release/cham3_reserve_probe",
      "/tmp/independent_probe_72arnvkb/existing_full-disk",
      "full-disk"
    ],
    "status": 3,
    "stdout": "refusing_to_fill / is overlay, not tmpfs\n",
    "stderr": "",
    "preserved": true
  },
  {
    "profile": "release",
    "case": "existing_overlay_allocate-only",
    "command": [
      "/work/out/m2/storage/target/release/cham3_reserve_probe",
      "/tmp/independent_probe_72arnvkb/existing_allocate-only",
      "allocate-only"
    ],
    "status": 3,
    "stdout": "refusing_to_fill /tmp/independent_probe_72arnvkb/existing_allocate-only exists; the probe uses only a root it created and leaves other paths alone\n",
    "stderr": "",
    "preserved": true
  },
  {
    "profile": "release",
    "case": "existing_overlay_unknown",
    "command": [
      "/work/out/m2/storage/target/release/cham3_reserve_probe",
      "/tmp/independent_probe_72arnvkb/existing_unknown",
      "unknown"
    ],
    "status": 3,
    "stdout": "refusing_to_fill unknown mode unknown\n",
    "stderr": "",
    "preserved": true
  },
  {
    "profile": "release",
    "case": "fresh_overlay_allocate-only",
    "command": [
      "/work/out/m2/storage/target/release/cham3_reserve_probe",
      "/tmp/independent_probe_72arnvkb/fresh",
      "allocate-only"
    ],
    "status": 0,
    "stdout": "reserve_bytes 1048576\nreserve_allocated_bytes 1048576\nreserve_promised_bytes 1048576\nreserve_preallocated\nreserve_verified\n",
    "stderr": "",
    "cleaned_or_never_created": true
  },
  {
    "profile": "release",
    "case": "fresh_overlay_full-disk",
    "command": [
      "/work/out/m2/storage/target/release/cham3_reserve_probe",
      "/tmp/independent_probe_72arnvkb/fresh",
      "full-disk"
    ],
    "status": 3,
    "stdout": "refusing_to_fill / is overlay, not tmpfs\n",
    "stderr": "",
    "cleaned_or_never_created": true
  },
  {
    "profile": "release",
    "case": "fresh_overlay_unknown",
    "command": [
      "/work/out/m2/storage/target/release/cham3_reserve_probe",
      "/tmp/independent_probe_72arnvkb/fresh",
      "unknown"
    ],
    "status": 3,
    "stdout": "refusing_to_fill unknown mode unknown\n",
    "stderr": "",
    "cleaned_or_never_created": true
  },
  {
    "profile": "release",
    "case": "missing_parent_allocate-only",
    "command": [
      "/work/out/m2/storage/target/release/cham3_reserve_probe",
      "/tmp/independent_probe_72arnvkb/missing/root",
      "allocate-only"
    ],
    "status": 3,
    "stdout": "refusing_to_fill cannot create /tmp/independent_probe_72arnvkb/missing/root: No such file or directory (os error 2)\n",
    "stderr": ""
  },
  {
    "profile": "release",
    "case": "missing_parent_full-disk",
    "command": [
      "/work/out/m2/storage/target/release/cham3_reserve_probe",
      "/tmp/independent_probe_72arnvkb/missing/root",
      "full-disk"
    ],
    "status": 2,
    "stdout": "check_failed io error No such file or directory (os error 2)\n",
    "stderr": ""
  },
  {
    "profile": "release",
    "case": "missing_parent_unknown",
    "command": [
      "/work/out/m2/storage/target/release/cham3_reserve_probe",
      "/tmp/independent_probe_72arnvkb/missing/root",
      "unknown"
    ],
    "status": 3,
    "stdout": "refusing_to_fill unknown mode unknown\n",
    "stderr": ""
  },
  {
    "profile": "release",
    "case": "root_symlink_allocate-only",
    "command": [
      "/work/out/m2/storage/target/release/cham3_reserve_probe",
      "/tmp/independent_probe_72arnvkb/symlink",
      "allocate-only"
    ],
    "status": 3,
    "stdout": "refusing_to_fill /tmp/independent_probe_72arnvkb/symlink exists; the probe uses only a root it created and leaves other paths alone\n",
    "stderr": "",
    "preserved": true
  },
  {
    "profile": "release",
    "case": "root_symlink_full-disk",
    "command": [
      "/work/out/m2/storage/target/release/cham3_reserve_probe",
      "/tmp/independent_probe_72arnvkb/symlink",
      "full-disk"
    ],
    "status": 3,
    "stdout": "refusing_to_fill / is overlay, not tmpfs\n",
    "stderr": "",
    "preserved": true
  },
  {
    "profile": "release",
    "case": "root_symlink_unknown",
    "command": [
      "/work/out/m2/storage/target/release/cham3_reserve_probe",
      "/tmp/independent_probe_72arnvkb/symlink",
      "unknown"
    ],
    "status": 3,
    "stdout": "refusing_to_fill unknown mode unknown\n",
    "stderr": "",
    "preserved": true
  },
  {
    "profile": "release",
    "case": "32m_tmpfs_refused",
    "command": [
      "/work/out/m2/storage/target/release/cham3_reserve_probe",
      "/oversize/root",
      "full-disk"
    ],
    "status": 3,
    "stdout": "refusing_to_fill tmpfs of 33554432 bytes exceeds the 16777216-byte limit\n",
    "stderr": ""
  },
  {
    "profile": "release",
    "case": "existing_filler_2m_tmpfs",
    "command": [
      "/work/out/m2/storage/target/release/cham3_reserve_probe",
      "/full-disk/root",
      "full-disk"
    ],
    "status": 3,
    "stdout": "reserve_bytes 1048576\nreserve_allocated_bytes 1048576\nreserve_promised_bytes 1048576\nreserve_preallocated\ntmpfs_mount /full-disk\nrefusing_to_fill /full-disk/cham3_reserve_probe_filler exists; the probe fills only through a file it created\n",
    "stderr": "",
    "preserved": true
  },
  {
    "profile": "release",
    "case": "2m_tmpfs_full_disk_positive",
    "command": [
      "/work/out/m2/storage/target/release/cham3_reserve_probe",
      "/full-disk/root",
      "full-disk"
    ],
    "status": 0,
    "stdout": "reserve_bytes 1048576\nreserve_allocated_bytes 1048576\nreserve_promised_bytes 1048576\nreserve_preallocated\ntmpfs_mount /full-disk\nfiller_bytes 1044480\nrestrictive_append_on_full_filesystem ok seq=35\nreserve_bytes_after_release 0\nadmission_refused reserve_depleted\nsecond_restrictive_append ok seq=36\nappend_after_space_returned ok seq=37\nreserve_bytes 1048576\nreserve_allocated_bytes 1048576\nreserve_promised_bytes 1048576\nreserve_preallocated\nreserve_replenished\njournal_records 37\njournal_chained\nreserve_verified\n",
    "stderr": ""
  },
  {
    "profile": "release",
    "case": "failure_after_exclusive_creation_512k",
    "command": [
      "/work/out/m2/storage/target/release/cham3_reserve_probe",
      "/too-small/root",
      "allocate-only"
    ],
    "status": 2,
    "stdout": "check_failed store error io\n",
    "stderr": "",
    "created_root_cleaned_after_store_failure": true
  }
]
PASS: 32 independent probe cases
```

### filler_failure.log

```text
{
  "profile": "debug",
  "command": [
    "/work/out/m2/storage/target/debug/cham3_reserve_probe",
    "/full-disk/root",
    "full-disk"
  ],
  "status": 2,
  "stdout": "reserve_bytes 1048576\nreserve_allocated_bytes 1048576\nreserve_promised_bytes 1048576\nreserve_preallocated\ntmpfs_mount /full-disk\ncheck_failed io error File too large (os error 27)\n",
  "stderr": "",
  "remaining": []
}
{
  "profile": "release",
  "command": [
    "/work/out/m2/storage/target/release/cham3_reserve_probe",
    "/full-disk/root",
    "full-disk"
  ],
  "status": 2,
  "stdout": "reserve_bytes 1048576\nreserve_allocated_bytes 1048576\nreserve_promised_bytes 1048576\nreserve_preallocated\ntmpfs_mount /full-disk\ncheck_failed io error File too large (os error 27)\n",
  "stderr": "",
  "remaining": []
}
PASS: synthetic RLIMIT_FSIZE failure during filler write cleans created root and filler in both profiles
```

### seeded.log

```text
test_a_missing_parent_is_refused (test_m2_reserve_probe.reserve_probe_t.test_a_missing_parent_is_refused) ... ok
test_allocate_only_preallocates_the_reserve (test_m2_reserve_probe.reserve_probe_t.test_allocate_only_preallocates_the_reserve) ... ok
test_an_existing_root_is_refused_and_preserved_in_every_mode (test_m2_reserve_probe.reserve_probe_t.test_an_existing_root_is_refused_and_preserved_in_every_mode) ... ok
test_an_unknown_mode_creates_nothing (test_m2_reserve_probe.reserve_probe_t.test_an_unknown_mode_creates_nothing) ... ok
test_full_disk_mode_refuses_an_ordinary_filesystem_before_creating_anything (test_m2_reserve_probe.reserve_probe_t.test_full_disk_mode_refuses_an_ordinary_filesystem_before_creating_anything) ... ok
test_each_seeded_defect_is_detected (test_m2_reserve_probe.seeded_probe_defects_t.test_each_seeded_defect_is_detected) ... OBSERVED: preservation=False status=0 stdout='reserve_bytes 1048576\nreserve_allocated_bytes 1048576\nreserve_promised_bytes 1048576\nreserve_preallocated\nreserve_verified\n'
OBSERVED: preservation=False status=3 stdout='refusing_to_fill /tmp/cham3_reserve_probe_ibplquym/existing exists; the probe uses only a root it created and leaves other paths alone\n'
ok
test_unmodified_copy_preserves_an_existing_root (test_m2_reserve_probe.seeded_probe_defects_t.test_unmodified_copy_preserves_an_existing_root) ... OBSERVED: preservation=True status=3 stdout='refusing_to_fill /tmp/cham3_reserve_probe_c289ojl5/existing exists; the probe uses only a root it created and leaves other paths alone\n'
ok

----------------------------------------------------------------------
Ran 7 tests in 4.468s

OK
```

### run_matrix.sh

```sh
#!/bin/sh
set -eu
sudo -n docker run --rm --network none --user 1001:1001 \
  --mount type=bind,src=/tmp/cham3_review_r3/candidate,dst=/work,readonly \
  --mount type=bind,src=/tmp/cham3_review_r3/probe_review,dst=/evidence,readonly \
  --tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700 \
  --tmpfs /oversize:rw,size=32m,uid=1001,gid=1001,mode=0700 \
  --tmpfs /too-small:rw,size=512k,uid=1001,gid=1001,mode=0700 \
  cham3-m2-independent-r3 python3 /evidence/matrix.py
```

### matrix.py

```python
import json
import os
from pathlib import Path
import stat
import subprocess
import tempfile

results = []
def snapshot(path):
    meta = path.lstat()
    return [meta.st_mode, meta.st_ino, meta.st_size, meta.st_mtime_ns, path.read_bytes().hex()]
def invoke(profile, name, root, mode):
    command = ['/work/out/m2/storage/target/' + profile + '/cham3_reserve_probe', str(root), mode]
    completed = subprocess.run(command, capture_output=True, text=True, timeout=60)
    result = dict(profile=profile, case=name, command=command, status=completed.returncode, stdout=completed.stdout, stderr=completed.stderr)
    results.append(result)
    return result
for profile in ('debug', 'release'):
    with tempfile.TemporaryDirectory(prefix='independent_probe_') as directory:
        base = Path(directory)
        for mode in ('full-disk', 'allocate-only', 'unknown'):
            root = base / ('existing_' + mode)
            root.mkdir(mode=0o750)
            sentinel = root / 'sentinel'
            sentinel.write_bytes(b'synthetic sentinel\x00unchanged\n')
            sentinel.chmod(0o640)
            before = snapshot(sentinel)
            root_before = root.stat()
            result = invoke(profile, 'existing_overlay_' + mode, root, mode)
            assert result['status'] == 3
            assert snapshot(sentinel) == before
            assert sorted(p.name for p in root.iterdir()) == ['sentinel']
            assert root.stat().st_mode == root_before.st_mode
            assert root.stat().st_mtime_ns == root_before.st_mtime_ns
            result['preserved'] = True
        for mode, code in [('allocate-only', 0), ('full-disk', 3), ('unknown', 3)]:
            root = base / 'fresh'
            result = invoke(profile, 'fresh_overlay_' + mode, root, mode)
            assert result['status'] == code and not root.exists()
            result['cleaned_or_never_created'] = True
        for mode, code in [('allocate-only', 3), ('full-disk', 2), ('unknown', 3)]:
            root = base / 'missing' / 'root'
            result = invoke(profile, 'missing_parent_' + mode, root, mode)
            assert result['status'] == code and not root.parent.exists()
        target = base / 'target'
        target.mkdir()
        sentinel = target / 'sentinel'
        sentinel.write_bytes(b'symlink target unchanged\n')
        before = snapshot(sentinel)
        root = base / 'symlink'
        root.symlink_to(target, target_is_directory=True)
        for mode in ('allocate-only', 'full-disk', 'unknown'):
            result = invoke(profile, 'root_symlink_' + mode, root, mode)
            assert result['status'] == 3 and root.is_symlink() and snapshot(sentinel) == before
            result['preserved'] = True
    root = Path('/oversize/root')
    result = invoke(profile, '32m_tmpfs_refused', root, 'full-disk')
    assert result['status'] == 3 and not root.exists() and list(root.parent.iterdir()) == []
    filler = Path('/full-disk/cham3_reserve_probe_filler')
    filler.write_bytes(b'existing filler sentinel\n')
    before = snapshot(filler)
    root = Path('/full-disk/root')
    result = invoke(profile, 'existing_filler_2m_tmpfs', root, 'full-disk')
    assert result['status'] == 3 and not root.exists() and snapshot(filler) == before
    result['preserved'] = True
    filler.unlink()
    root = Path('/full-disk/root')
    result = invoke(profile, '2m_tmpfs_full_disk_positive', root, 'full-disk')
    assert result['status'] == 0 and not root.exists() and list(root.parent.iterdir()) == []
    assert 'reserve_replenished' in result['stdout'] and 'journal_chained' in result['stdout']
    root = Path('/too-small/root')
    result = invoke(profile, 'failure_after_exclusive_creation_512k', root, 'allocate-only')
    assert result['status'] == 2 and not root.exists() and list(root.parent.iterdir()) == []
    result['created_root_cleaned_after_store_failure'] = True
print(json.dumps(results, indent=2))
print('PASS:', len(results), 'independent probe cases')
```

### run_filler_failure.sh

```sh
#!/bin/sh
set -eu
sudo -n docker run --rm --network none --user 1001:1001 \
  --mount type=bind,src=/tmp/cham3_review_r3/candidate,dst=/work,readonly \
  --mount type=bind,src=/tmp/cham3_review_r3/probe_review,dst=/evidence,readonly \
  --tmpfs /full-disk:rw,size=8m,uid=1001,gid=1001,mode=0700 \
  cham3-m2-independent-r3 python3 /evidence/filler_failure.py
```

### filler_failure.py

```python
import json
from pathlib import Path
import resource
import signal
import subprocess

def limit_filler():
    signal.signal(signal.SIGXFSZ, signal.SIG_IGN)
    resource.setrlimit(resource.RLIMIT_FSIZE, (1024 * 1024, 1024 * 1024))
for profile in ('debug', 'release'):
    command = ['/work/out/m2/storage/target/' + profile + '/cham3_reserve_probe', '/full-disk/root', 'full-disk']
    result = subprocess.run(command, capture_output=True, text=True, timeout=60, preexec_fn=limit_filler)
    print(json.dumps(dict(profile=profile, command=command, status=result.returncode, stdout=result.stdout, stderr=result.stderr, remaining=[str(path) for path in Path('/full-disk').iterdir()]), indent=2))
    assert result.returncode == 2 and 'File too large' in result.stdout
    assert 'tmpfs_mount /full-disk' in result.stdout and 'reserve_preallocated' in result.stdout
    assert list(Path('/full-disk').iterdir()) == []
print('PASS: synthetic RLIMIT_FSIZE failure during filler write cleans created root and filler in both profiles')
```

### run_seeded.sh

```sh
#!/bin/sh
set -eu
sudo -n docker run --rm --network none --user 1001:1001 \
  --mount type=bind,src=/tmp/cham3_review_r3/candidate,dst=/work,readonly \
  --mount type=bind,src=/tmp/cham3_review_r3/probe_review,dst=/evidence,readonly \
  --env CARGO_HOME=/tmp/cargo_home --env CHAM3_M2_RESERVE_PROBE=/work/out/m2/storage/target/release/cham3_reserve_probe \
  cham3-m2-independent-r3 python3 /evidence/seeded_run.py
```

### seeded_run.py

```python
import sys
import unittest
sys.path.insert(0, '/work/tests/m2/harness')
import test_m2_reserve_probe as probe_tests
original = probe_tests.seeded_probe_defects_t.existing_root_survives
def observed_check(self, probe):
    survived, completed = original(self, probe)
    print('OBSERVED: preservation=%s status=%s stdout=%r' % (survived, completed.returncode, completed.stdout), flush=True)
    return survived, completed
probe_tests.seeded_probe_defects_t.existing_root_survives = observed_check
suite = unittest.defaultTestLoader.loadTestsFromModule(probe_tests)
result = unittest.TextTestRunner(verbosity=2).run(suite)
raise SystemExit(0 if result.wasSuccessful() else 1)
```

### run_wrapper.sh

```sh
#!/bin/sh
set -eu
sudo -n docker run --rm --network none --user 1001:1001 \
  --mount type=bind,src=/tmp/cham3_review_r3/candidate,dst=/work,readonly \
  --mount type=bind,src=/tmp/cham3_review_r3/probe_review,dst=/evidence,readonly \
  cham3-m2-independent-r3 python3 /evidence/wrapper_run.py
```

### wrapper_run.py

```python
from pathlib import Path
import os
import shutil
import subprocess
root = Path('/tmp/wrapper_synthetic_disk')
root.mkdir()
existing_root = root / 'cham3_reserve_probe_root'
existing_root.mkdir()
(existing_root / 'sentinel').write_text('synthetic existing root must survive refusal\n')
filler = root / 'cham3_reserve_probe_filler'
filler.write_text('synthetic existing filler must survive refusal\n')
shutil.copytree('/work/out/m2/storage/target', '/tmp/wrapper_out/target')
environment = dict(os.environ, CARGO_HOME='/tmp/cargo_home')
command = ['sh', '/work/tests/m2/storage/check_reserve_full_disk.sh', '/tmp/wrapper_out', str(root)]
print('COMMAND', repr(command), flush=True)
completed = subprocess.run(command, cwd='/work', env=environment, capture_output=True, text=True, timeout=600)
print(completed.stdout, completed.stderr, sep='\n')
print('wrapper_exit_status', completed.returncode)
print('existing_root_exists', existing_root.exists())
print('existing_filler_exists', filler.exists())
assert completed.returncode != 0 and not existing_root.exists() and not filler.exists()
print('CONFIRMED: wrapper deletes both pre-existing synthetic paths before overlay refusal')
```

### r3_journal_tests.rs

```rust
// Independent R3 failure-ordering tests; fixtures and public APIs only.
#[derive(Clone)]
struct r3_io_t {
    allocation_failures: Arc<AtomicUsize>,
    allocations: Arc<AtomicUsize>,
    fail_write: Arc<std::sync::atomic::AtomicBool>,
    releases: Arc<AtomicUsize>,
    no_allocation: bool,
}
impl r3_io_t {
    fn new() -> Self {
        Self { allocation_failures: Arc::new(AtomicUsize::new(0)), allocations: Arc::new(AtomicUsize::new(0)),
            fail_write: Arc::new(std::sync::atomic::AtomicBool::new(false)), releases: Arc::new(AtomicUsize::new(0)), no_allocation: false }
    }
}
impl journal_io_t for r3_io_t {
    fn write_all(&self, file: &mut fs::File, bytes: &[u8]) -> io::Result<()> {
        if self.fail_write.swap(false, Ordering::SeqCst) { return Err(io::Error::from_raw_os_error(28)); }
        system_io_t.write_all(file, bytes)
    }
    fn sync_data(&self, file: &fs::File) -> io::Result<()> { system_io_t.sync_data(file) }
    fn sync_directory(&self, path: &Path) -> io::Result<()> { system_io_t.sync_directory(path) }
    fn allocate(&self, file: &mut fs::File, from: u64, to: u64) -> io::Result<()> {
        self.allocations.fetch_add(1, Ordering::SeqCst);
        if self.no_allocation { return Ok(()); }
        let count = to.saturating_sub(from);
        assert!(count <= cham3_registryd::journal::ReserveBytes);
        io::Write::write_all(file, &vec![0; count as usize])?;
        if self.allocation_failures.load(Ordering::SeqCst) > 0 {
            self.allocation_failures.fetch_sub(1, Ordering::SeqCst);
            return Err(io::Error::other("R3 reserve sync failure"));
        }
        file.sync_all()
    }
    fn release(&self, file: &fs::File) -> io::Result<()> {
        self.releases.fetch_add(1, Ordering::SeqCst);
        system_io_t.release(file)
    }
}
fn r3_certificate(user: &test_user_t, label: &str) -> Vec<u8> {
    certificate(&user.identity, user.uid, &test_key(label), authority_class_t::persistent, 1)
}

#[test]
fn r3_startup_sync_fails_twice_then_succeeds() {
    for new_root in [true, false] {
        let parent = temp_root(if new_root { "r3_root_twice" } else { "r3_users_twice" });
        let root = if new_root { parent.join("registry") } else { parent.clone() };
        let required = if new_root { parent.clone() } else { root.clone() };
        let spy = edge_sync_t { calls: Arc::new(Mutex::new(Vec::new())), fail_path: required,
            fail_once: Arc::new(std::sync::atomic::AtomicBool::new(true)) };
        for _ in 0..2 {
            spy.fail_once.store(true, Ordering::SeqCst);
            assert!(registry_t::open(&root, owner_of(&parent), registry_config_t::default(), Box::new(spy.clone()), Box::new(fixed_clock_t)).is_err());
        }
        spy.calls.lock().unwrap().clear();
        let registry = open_with(&root, registry_config_t::default(), Box::new(spy.clone()));
        assert_eq!(spy.calls.lock().unwrap().len(), 2, "two startup directory syncs before any user");
        enroll(&registry, &test_user(1001));
        println!("new_root={new_root}: two failures refused, third startup and enrollment acknowledged");
    }
}
#[test]
fn r3_failed_user_directory_isolated_from_other_users() {
    let root = temp_root("r3_user_isolation");
    let alice = test_user(1001); let bob = test_user(1002);
    let registry = open(&root); enroll(&registry, &alice); enroll(&registry, &bob); drop(registry);
    let spy = edge_sync_t { calls: Arc::new(Mutex::new(Vec::new())), fail_path: root.join("users/1001"),
        fail_once: Arc::new(std::sync::atomic::AtomicBool::new(true)) };
    for attempt in 0..2 {
        spy.fail_once.store(true, Ordering::SeqCst);
        let registry = open_with(&root, registry_config_t::default(), Box::new(spy.clone()));
        let unusable = registry.unusable_users().unwrap();
        assert_eq!(unusable.len(), 1); assert_eq!(unusable[0].0, alice.uid);
        assert_eq!(code(registry.append_identity(Identityd, alice.uid, object_kind_t::key_certificate, &r3_certificate(&alice, "blocked"))), "ipc/storage_unavailable");
        registry.append_identity(Identityd, bob.uid, object_kind_t::key_certificate, &r3_certificate(&bob, &format!("bob {attempt}"))).unwrap();
    }
    assert!(open(&root).unusable_users().unwrap().is_empty());
}
#[test]
fn r3_recreated_reserve_directory_fails_across_two_restarts() {
    let root = temp_root("r3_recreated_twice"); let user = test_user(1001);
    let registry = open(&root); enroll(&registry, &user); drop(registry);
    fs::remove_file(root.join("users/1001/reserve")).unwrap();
    let spy = edge_sync_t { calls: Arc::new(Mutex::new(Vec::new())), fail_path: root.join("users/1001"),
        fail_once: Arc::new(std::sync::atomic::AtomicBool::new(true)) };
    for _ in 0..2 {
        spy.fail_once.store(true, Ordering::SeqCst);
        let registry = open_with(&root, registry_config_t::default(), Box::new(spy.clone()));
        assert_eq!(registry.unusable_users().unwrap().len(), 1);
        assert!(root.join("users/1001/reserve").is_file());
    }
    let registry = open_with(&root, registry_config_t::default(), Box::new(spy));
    assert!(registry.unusable_users().unwrap().is_empty());
    assert_eq!(registry.reserve_bytes(user.uid), Ok(0));
    registry.append_identity(Identityd, user.uid, object_kind_t::key_certificate, &r3_certificate(&user, "recreated refill")).unwrap();
    assert_eq!(registry.reserve_bytes(user.uid), Ok(cham3_registryd::journal::ReserveBytes));
}
#[test]
fn r3_reserve_sync_fails_at_start_and_append_then_recovers() {
    let root = temp_root("r3_reserve_twice"); let user = test_user(1001);
    let registry = open(&root); enroll(&registry, &user); drop(registry);
    let fault = r3_io_t::new(); fault.allocation_failures.store(2, Ordering::SeqCst);
    let registry = open_with(&root, registry_config_t::default(), Box::new(fault.clone()));
    assert!(registry.unusable_users().unwrap().is_empty());
    assert_eq!(registry.reserve_bytes(user.uid), Ok(0));
    let restrictive = invalidation(&user.identity, &user.temporary.key_id, invalidation_reason_t::compromised, "pending reserve");
    registry.append_identity(Identityd, user.uid, object_kind_t::key_invalidation, &restrictive).unwrap();
    assert_eq!(fault.allocations.load(Ordering::SeqCst), 1);
    let cert = r3_certificate(&user, "retry same certificate");
    assert_eq!(code(registry.append_identity(Identityd, user.uid, object_kind_t::key_certificate, &cert)), "ipc/storage_unavailable");
    assert_eq!(registry.reserve_bytes(user.uid), Ok(0));
    registry.append_identity(Identityd, user.uid, object_kind_t::key_certificate, &cert).unwrap();
    assert_eq!(fault.allocations.load(Ordering::SeqCst), 3);
    assert_eq!(registry.reserve_bytes(user.uid), Ok(cham3_registryd::journal::ReserveBytes));
}
#[test]
fn r3_oversized_reserve_is_pending_then_synced_without_growth() {
    let root = temp_root("r3_oversized"); let user = test_user(1001);
    let registry = open(&root); enroll(&registry, &user); drop(registry);
    let reserve = root.join("users/1001/reserve");
    let mut file = fs::OpenOptions::new().append(true).open(&reserve).unwrap();
    io::Write::write_all(&mut file, &[0;4096]).unwrap(); file.sync_all().unwrap(); drop(file);
    let length = fs::metadata(&reserve).unwrap().len();
    let fault = r3_io_t::new();
    let registry = open_with(&root, registry_config_t::default(), Box::new(fault.clone()));
    assert_eq!(registry.reserve_bytes(user.uid), Ok(0)); assert_eq!(fault.allocations.load(Ordering::SeqCst), 0);
    registry.append_identity(Identityd, user.uid, object_kind_t::key_certificate, &r3_certificate(&user, "oversized refill")).unwrap();
    assert_eq!(fault.allocations.load(Ordering::SeqCst), 1);
    assert_eq!(registry.reserve_bytes(user.uid), Ok(cham3_registryd::journal::ReserveBytes));
    assert_eq!(fs::metadata(&reserve).unwrap().len(), length);
}
#[test]
fn r3_hostile_noop_allocator_boundary_is_explicit() {
    // A malicious implementation violates allocate's trusted contract. Characterize
    // creation trust separately from the extra replenishment length guard.
    let root = temp_root("r3_noop_create"); let user = test_user(1001);
    let mut hostile = r3_io_t::new(); hostile.no_allocation = true;
    let registry = open_with(&root, registry_config_t::default(), Box::new(hostile.clone()));
    enroll(&registry, &user);
    assert_eq!(fs::metadata(root.join("users/1001/reserve")).unwrap().len(), 0);
    assert_eq!(registry.reserve_bytes(user.uid), Ok(cham3_registryd::journal::ReserveBytes));
    println!("trusted no-op allocator: creation accepts its false success, length=0; not system_io behavior");
    drop(registry);
    let registry = open_with(&root, registry_config_t::default(), Box::new(hostile));
    assert_eq!(registry.reserve_bytes(user.uid), Ok(0));
    assert_eq!(code(registry.append_identity(Identityd, user.uid, object_kind_t::key_certificate, &r3_certificate(&user, "noop refill"))), "ipc/storage_unavailable");
}
#[test]
fn r3_release_failed_refill_restart_retries_sync() {
    let root = temp_root("r3_release_restart"); let user = test_user(1001);
    let fault = r3_io_t::new();
    let registry = open_with(&root, registry_config_t::default(), Box::new(fault.clone()));
    enroll(&registry, &user);
    fault.fail_write.store(true, Ordering::SeqCst);
    let restrictive = invalidation(&user.identity, &user.temporary.key_id, invalidation_reason_t::compromised, "release");
    registry.append_identity(Identityd, user.uid, object_kind_t::key_invalidation, &restrictive).unwrap();
    assert_eq!(fault.releases.load(Ordering::SeqCst), 1);
    assert_eq!(registry.reserve_bytes(user.uid), Ok(0));
    fault.allocation_failures.store(3, Ordering::SeqCst);
    let cert = r3_certificate(&user, "after released reserve");
    assert_eq!(code(registry.append_identity(Identityd, user.uid, object_kind_t::key_certificate, &cert)), "ipc/storage_unavailable");
    assert_eq!(fs::metadata(root.join("users/1001/reserve")).unwrap().len(), cham3_registryd::journal::ReserveBytes);
    drop(registry);
    let registry = open_with(&root, registry_config_t::default(), Box::new(fault.clone()));
    assert!(registry.unusable_users().unwrap().is_empty());
    assert_eq!(registry.reserve_bytes(user.uid), Ok(0));
    assert_eq!(code(registry.append_identity(Identityd, user.uid, object_kind_t::key_certificate, &cert)), "ipc/storage_unavailable");
    registry.append_identity(Identityd, user.uid, object_kind_t::key_certificate, &cert).unwrap();
    assert_eq!(registry.reserve_bytes(user.uid), Ok(cham3_registryd::journal::ReserveBytes));
    drop(registry);
    assert!(open(&root).unusable_users().unwrap().is_empty());
}
```

### r3_debug.log

```text
   Compiling m2_005_independent_review_tests v0.1.0 (/evidence/runner/core/reviewer)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.27s
     Running unittests review.rs (/target/debug/deps/m2_005_independent_review_tests-6f99949d5b2a283a)

running 7 tests
test registry::r3_failed_user_directory_isolated_from_other_users ... ok
test registry::r3_hostile_noop_allocator_boundary_is_explicit ... trusted no-op allocator: creation accepts its false success, length=0; not system_io behavior
ok
test registry::r3_oversized_reserve_is_pending_then_synced_without_growth ... ok
test registry::r3_recreated_reserve_directory_fails_across_two_restarts ... ok
test registry::r3_release_failed_refill_restart_retries_sync ... ok
test registry::r3_reserve_sync_fails_at_start_and_append_then_recovers ... ok
test registry::r3_startup_sync_fails_twice_then_succeeds ... new_root=true: two failures refused, third startup and enrollment acknowledged
new_root=false: two failures refused, third startup and enrollment acknowledged
ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 43 filtered out; finished in 0.13s
```

### r3_release.log

```text
   Compiling m2_005_independent_review_tests v0.1.0 (/evidence/runner/core/reviewer)
    Finished `release` profile [optimized] target(s) in 2.26s
     Running unittests review.rs (/target/release/deps/m2_005_independent_review_tests-a50213b7c6057427)

running 7 tests
test registry::r3_failed_user_directory_isolated_from_other_users ... ok
test registry::r3_hostile_noop_allocator_boundary_is_explicit ... trusted no-op allocator: creation accepts its false success, length=0; not system_io behavior
ok
test registry::r3_oversized_reserve_is_pending_then_synced_without_growth ... ok
test registry::r3_recreated_reserve_directory_fails_across_two_restarts ... ok
test registry::r3_release_failed_refill_restart_retries_sync ... ok
test registry::r3_reserve_sync_fails_at_start_and_append_then_recovers ... ok
test registry::r3_startup_sync_fails_twice_then_succeeds ... new_root=true: two failures refused, third startup and enrollment acknowledged
new_root=false: two failures refused, third startup and enrollment acknowledged
ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 43 filtered out; finished in 0.05s
```
