# M2-006 stage B1 — independent Codex security review

**Verdict: CHANGES_REQUESTED** for candidate `c9129bca0e6c4b1e6caf72774e05a27e09a4d542`. One Medium authorization finding and one Low E6 acceptance finding are reproduced in debug and release. Qualification and the existing harness pass; they do not cover either defect.

- State: IN_REVIEW → CHANGES_REQUESTED; independent REVIEWER, Codex. No implementation was authored or modified by this review.
- Authority: `PROJECT.md`; approved ADR 0007 §5 rules 1–4 and 6, §5.7 limits, §8 Option B, E4/E6; ADR 0006/E5; AGENTS and engineering/agent workflow.
- Task: `docs/tasks/M2-cryptographic-contract-core.md`, staged B1 scope, and `docs/handoffs/M2-006-stage-b1.md` from the candidate-follow-up checkout.
- Implementation base: `e7803efaeb346ac94e12aca6ce32e2fabaa3cadd`; candidate parent is exactly that base.
- Local owner clone after requested step 0: `/home/dev/cham3`, branch `codex/m2-cryptographic-contract-core`, HEAD `b4e4cdc124a82faaed15aeeda8ca90b48fe6a43d`, parent exactly `c9129bc`. Initially and finally clean; no stashes or local changes moved aside.
- Review record branch: `review/m2-006-b1-security`, based on `origin/main` = `9cf882503848f8c6cc4938a588cc9fc062a9392d`; separate worktree `/home/dev/cham3-review-m2-006-b1-security`. Adds only this record; no merge or implementation-branch write.
- Environment: Debian 13.7 x86_64; Linux 7.1.13+deb13-amd64; UID/GID 1001. Docker accessible directly through existing group 119 (docker); `docker ps` exit 0. Retained image `cham3-m2-independent-r3-final`, `sha256:4528aed8d303c7255b9138392a4b2f7973a1b944b140cac66a9da40360ff3e56`; no new image build claimed. Containers unprivileged UID/GID 1001, `--ulimit nofile=1024:1024`, no added capabilities or privileged host changes. Host nofile was 524288.
- Qualification clone: `/tmp/cham3-m2-b1-independent-qual`, detached `c9129bc`; `CHAM3_M2_REQUIRE_CLEAN=1`, `dirty_tracked 0`. Reviewer console capture was an untracked non-source file during execution and was moved outside the clone afterwards; final porcelain output empty. All source tests run against the unmodified candidate.
- Reviewer test clone: under the plugin-returned temporary directory (below), detached `c9129bc`. Adds only two test modules composed from existing candidate fixtures plus the snippets embedded below. Every production source is unchanged.

## Findings

### M2-006-B1-R01 — Medium — deny the current evaluation when time sharing fails

**Location:** `core/policyd/src/service.rs:200–207`, shared by `evaluate` at 280–294 and `network_mode` at 309–323. CWE-636/CWE-754.

A valid request first obtains a healthy lineage snapshot. If policyd then observes a forward sample, `time_sample` can fail its ledger append, mark the entire registry run unverifiable, and return `storage_unavailable` (`core/registryd/src/registry.rs:2304–2322`). The `if let Ok(reply)` discards this failure and evaluates the previously obtained snapshot with the old offset.

**Reproduction:** admit at T1+60 a persistent file grant expiring at T1+100; registry remains at T1+60; policyd reads T1+200 at the same BOOTTIME. Fail only the ledger append. The current response is `eligible` with `ledger_failed=true`; the next response is `unknown(registry_unavailable)`. The identical no-fault forward sample correctly returns `denied(expired)`. Separate network controls return both `mode { mode: direct }` and `mode { mode: tor }` after the ledger fails.

This violates the mandatory failed-ledger denial condition for the current decision. No actual file access or packet leakage is claimed: B1 is in-process authorization, and transport/enforcement are later milestones. A forward sample between snapshot retrieval and evaluation plus a ledger I/O failure is an operational precondition; unprivileged ability to force those conditions is not established. That constrains severity to Medium.

**Required:** propagate a failed shared-time update into an unknown/denied current evaluation and Offline network outcome. Do not rely on a snapshot obtained before the failure. Preserve the no-fault expiry control, malformed/other-boot sample controls, restrictions during sample failure and registry recovery tests. Add regressions for both entrypoints, not only a direct `registry.time_sample` test.

### M2-006-B1-R02 — Low — preserve anomaly episode accounting across same-boot restart

**Locations:** `core/registryd/src/clock.rs:247` resets anomaly state; `core/registryd/src/registry.rs:672` resets the user's recorded episode; replay at 590–595 preserves the event but not episode bookkeeping. The new write occurs at 1394–1417.

E6 defines an episode as continuing until a sample returns within 300 seconds of trusted time. A same-boot service restart is not such a sample. Start at T1, step realtime to T1−400, read Alice's lineage, cleanly shut down, restart in the same boot at exactly the same clocks, and read Alice again. Two identical `(trusted=T1, behind_seconds=400)` records appear despite no end to the episode. The expected once-per-user count of one fails in both profiles.

This is an acceptance/audit-accounting defect; records change no authority and no exploit gain is demonstrated. Codex Security therefore retains the validated candidate but suppresses it as a vulnerability under its attack-path policy. The independent B1 acceptance verdict still requires correction.

**Required:** recover/persist same-boot episode identity/open state and per-user reported membership through the existing protected anchor/ledger/replay machinery. Preserve episode closure too, so a later genuine episode is distinguishable. Simply deduplicating equal event payloads would hide distinct episodes and is insufficient. Test clean and unclean restart within an episode, a persisted closure followed by a new episode, and distinct users. This needs no new clock-policy owner decision.

## Acceptance review and evidence

All rows refer to candidate c9129bc. PASS here is component evidence in the declared container, not deployed OS enforcement. Commands C1–C9 and retained logs below identify the exact execution.

| Requested check | Result | Evidence and practical limit |
|---|---|---|
| Exact commit/scope/docs-only history | PASS | Step-0 and Git commands below; 34-file candidate, one commit; four intervening docs commits and b4e4cdc docs-only. |
| Offset rule, forward rebasing, never lowering, BOOTTIME pace, old sample's own offset | PASS | clock.rs inspection; 7 clock unit tests; trusted_time 12; 165/4813 differential. `assess` uses the sample's own boottime; `observe` keeps max last boottime. |
| Floor/ledger arithmetic, each adopted operational raise ledgered first, floor in every anchor image, max merge, refresh at 60 seconds | PASS | Anchor/ledger additions and all anchor update sites inspected. Failed raise is not adopted. Floor is published before startup serves; intermediate startup does not serve on failed final marker. C1 and concurrent 8-writer reviewer test pass. |
| Same-boot restart from anchor + ledger; crash after ledger raise before anchor; setback after restart | PASS | Existing trusted_time same-boot test first raises at unchanged boottime so no refresh publishes it; dropped process resumes ledger. Clean shutdown then anchor resume passes. |
| Invalid anchor/no floor; foreign-run ledger; contained start | PASS | Reviewer invalid-anchor control discards invalid floor and quarantines; foreign valid ledger run is rejected, prior anchored floor retained, user contained. Private/run/boot checks inspected. Invalid anchor intentionally has no recoverable time floor; no eligible user state survives. |
| Sharing with policyd: max own/snapshot, peer report before reliance; other boot/future BOOTTIME | **FAIL** | Direct registry and ordinary controls pass, but current policy request returns eligibility after the failed report: R01, two reviewer fault tests. |
| Anomaly 300/301 boundary, record content/replay, once per user per episode | **FAIL** | Ordinary boundary/content and two-user tests pass; replayed journal remains valid, but same-boot continuous episode duplicates: R02. |
| Anomaly operation coverage/quarantine/pending | PASS, scoped | Audited existing-user `lineage_head/admit/restrict/append_identity/identity_state`; role check precedes sampling. Quarantined journal unchanged in reviewer test; pending anomaly blocks admission with `restriction_not_durable`. Existing harness does not establish restart episode persistence. Policyd-only backward samples are not separately reported; see owner note below. |
| Observed counter; same/new boot; no wall/powered-off advancement | PASS | Clock model and integration restart tests; counter offset independent of realtime raises, prior observed value restored in new boot. Physical reboot/suspend NOT_RUN. |
| Option B post-admission error; equal start; error already at admission; restored/merged origins | PASS | Existing forward-error test; reviewer equality/error-at-admission and restore/merge tests. Restored/merged new grant ids carry their new origin's admitted pair and wait at 99 seconds, pass at 100. Error already present at admission can accelerate equal start exactly as the approved bound permits. |
| E4 both clauses/order/admission-only/replay | PASS | Inclusive ±300 vs ±301, every parent lower bound; after stale_head/placement before state; replay `None`. Four E4 scenarios and existing signing-window test pass. |
| E4 no durable change on refusal | PASS for settled clock; owner wording question | Reviewer snapshots journal, both anchor slots and both ledger files before/after refusal and finds byte identity. Independent mandatory clock refresh/anomaly housekeeping can precede a refusal; whole-request wording ambiguity is kept separately, not resolved by reviewer. |
| Signing window at trusted now; unavailable admission/restrict clock | PASS | Existing registry_operations certificate-window case (before/inclusive end/+1/replay) and trusted_time unavailable source case. Last trusted value is a lower bound of **future trusted time**, because the service clock never decreases; it is not a claim that it equals accurate UTC after a forward error. It proves an expiration already at/below that value, while later expirations are refused; general restriction still succeeds. |
| New boot ends temporary branches in memory/durably; same-boot no repeat; network slot freed, trunk unchanged | PASS | Clean timed branch baseline; reviewer failed-write network branch + ledger restart; failed initial new-boot marker + old boot detected again; one-shot branch. Every branch clearing site inspected. No actual power-loss test. |
| Oracles/vector identity/counts | PASS with limits | C1/C5: trusted vectors 165/4813, 0 disagreement; ECR 19/96/18, 27/365; pure admission differential 15/72/121, 0 disagreement. Counts and model coverage below. |
| Existing unit/harness and seeded defects | PASS with gaps | C1: 234+234 Rust, 149 Python, 119/119 seeded registry/policy defects; 22 new/12 repointed. All 22 discriminating assertions individually inspected below. Passing suite does not cover R01/R02. |
| Reviewer failure/positive controls | **FAIL acceptance** | C6: 14 cases in each profile; 11 PASS, 3 FAIL. Policy:3 PASS/2 FAIL; registry:8 PASS/1 FAIL. Three failed assertions represent the two findings; original candidate source unchanged. |
| amd64 qualification | PASS | C1 exit0 `m2 qualification: PASS`; locks, sources, toolchain, rustfmt/clippy/naming, fresh builds, profile/backends, Memcheck probe and secure_memory (10 tests/0 errors), unit/harness all pass. |
| Static checks and static tests | PASS | C2 exit0 `static checks: PASS`, 82 tests OK. |
| Full-filesystem wrapper; preservation cases | PASS | C3/C4 exit0, actual 2MiB container tmpfs, debug/release reserve+ledger probe; 5/5 wrapper cases, existing paths and links preserved. Anchor ENOSPC behavior is not claimed by this wrapper. |
| Provider/format/vault oracles | PASS with established bounds | C1 provider PASS, format 5968/0/56 outcomes, corpus `44ea2aa43135df93b3639c1c64810e4c16b14506297b314b89ba3e4cd8cf426c`; vault38. Argon2 secret/AD reference case remains NOT_CHECKED by provider binding, not a full cryptographic audit. |
| CI comparison/download/digest | PASS | C7–C8: m2/m1/m0 requested runs all success at exact candidate, every job/step success. Independently downloaded raw ZIP artifact11918223662 digest matches. Local results match CI counts. CI vault protection positive report `protection_verified`, exit0, dumpable0, unlocked_mappings0; local positive protection NOT_RUN. |
| Codex Security | PASS execution, 1 Medium finding | Explicit `security-diff-scan`, plugin `codex-security`0.1.32; scan690a6945-d328-40c8-9a25-bdab39061ac6, completed/sealed. Two candidates validated: R01 promoted, R02 ignore as exploit but retained acceptance defect. All 34 changed paths accounted. |
| Hardware clocks/suspend/sync/RTC setback, real crash, actual IPC/enforcement | NOT_RUN | No real host clock changes or physical crash performed; settable-source and I/O fault tests are simulations. Production process/authentication/tmpfs placement M2-007 and runtime/network enforcement later milestones. |
| Miri/sanitizers/coverage-guided fuzzing | NOT_RUN | Qualification records NOT_AVAILABLE for pinned stable Debian toolchain; never PASS. |
| Architecture/risk decisions and implementation changes by reviewer | NOT_RUN intentionally | Reviewer made no owner decisions and applied no fixes. Next owner is implementer for R01/R02; independent re-review required before B2. |

## Scope and authoritative checkout

Exact step 0 (PASS; no stash necessary):

~~~sh
cd /home/dev/cham3
git status --porcelain
git stash list
git fetch origin codex/m2-cryptographic-contract-core main
git checkout codex/m2-cryptographic-contract-core
git reset --hard origin/codex/m2-cryptographic-contract-core
git rev-parse HEAD
git log --oneline -7
~~~

Reviewed docs-only commits:

| Commit | Parent | Changed files | Result |
|---|---|---:|---|
| 84464cc6bf750d8a2128c662b2fcc10cb7f8ed4f | db5bb59fcf461034461d9e0d2f21f5816986134c | 43 | PASS docs-only |
| 9d13864ae82e443abcc3988711703fe0d3d09d4b | 84464cc | 3 | PASS docs-only |
| 68ca252d0a73799b2d1514ed9bd69f4ed6eda604 | 9d13864 | 5 | PASS docs-only |
| e7803efaeb346ac94e12aca6ce32e2fabaa3cadd | 68ca252 | 4 | PASS docs-only |
| b4e4cdc124a82faaed15aeeda8ca90b48fe6a43d | c9129bc | 39 | PASS docs-only |

Commands: `git log --format='%H %P' db5bb59..b4e4cdc`, `git diff-tree --no-commit-id --name-only -r <each_sha>`, `git diff --stat e7803ef c9129bc`, `git diff --name-only e7803ef c9129bc`, `git diff --name-only c9129bc b4e4cdc`, `git diff --check e7803ef c9129bc`, `git diff --check c9129bc b4e4cdc`. All exit0; every intervening/follow-up path starts with docs/.

Forbidden input check (PASS, exit0):

~~~sh
git diff --exit-code e7803ef c9129bc -- Cargo.toml Cargo.lock .cargo build .github PROJECT.md docs/decisions core/contract_format/src core/contractd/src core/vaultd core/native core/registryd/src/journal.rs core/registryd/src/bin tests/m2/lib tests/m2/storage tests/m2/protection
~~~

No manifests/dependencies, locks, pins, workflows, Dockerfile or ADR changes in the implementation range. `trusted_time_model.py` diff is documentation only. Programmatic ECR JSON comparison finds every field unchanged except four appended journal scenarios; the original 23 scenarios remain exactly equal. Candidate includes exactly these 34 files (all reviewed; generated vectors structurally compared and regenerated):

~~~text
core/contract_format/tests/ecr_vectors.rs
core/contractd/tests/submission.rs
core/identityd/src/outcome.rs
core/policyd/src/evaluate.rs
core/policyd/src/service.rs
core/policyd/tests/current_authorization.rs
core/policyd/tests/end_to_end.rs
core/registryd/src/anchor.rs
core/registryd/src/clock.rs
core/registryd/src/ledger.rs
core/registryd/src/lib.rs
core/registryd/src/lineage.rs
core/registryd/src/record.rs
core/registryd/src/registry.rs
core/registryd/tests/admission_scenarios.rs
core/registryd/tests/builders/registry_support.rs
core/registryd/tests/registry_operations.rs
core/registryd/tests/storage_integrity.rs
core/registryd/tests/trusted_time.rs
core/registryd/tests/trusted_time_model.rs
docs/specs/m2_registry_storage_v1.md
docs/specs/m2_service_ipc_v1.md
docs/specs/m2_wire_format_v1.md
tests/m2/harness/test_m2_ecr_oracle.py
tests/m2/harness/test_m2_registry_policy.py
tests/m2/harness/test_m2_trusted_time.py
tests/m2/oracle/ecr_oracle.py
tests/m2/oracle/trusted_time_model.py
tests/m2/oracle/trusted_time_vectors.py
tests/m2/run_m2_tests.sh
tests/m2/test_inventory.txt
tests/m2/vectors/ecr_v1/vectors.json
tests/m2/vectors/ecr_v1/vectors_readme.md
tests/m2/vectors/trusted_time_v1/vectors.json
~~~

## Exact independent execution commands

C1 — clean-clone qualification (PASS exit0; source identity tracked clean, final Git porcelain clean):

~~~sh
git clone --no-hardlinks /home/dev/cham3 /tmp/cham3-m2-b1-independent-qual
git -C /tmp/cham3-m2-b1-independent-qual checkout --detach c9129bca0e6c4b1e6caf72774e05a27e09a4d542
docker run --rm --name cham3-m2-b1-independent-qual --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 -v /tmp/cham3-m2-b1-independent-qual:/work -w /work cham3-m2-independent-r3-final bash build/scripts/qualify_m2_rust.sh > /tmp/cham3-m2-b1-independent-qual/qualification_console.txt 2>&1
mv /tmp/cham3-m2-b1-independent-qual/qualification_console.txt /tmp/cham3-m2-b1-qualification.log
git -C /tmp/cham3-m2-b1-independent-qual status --porcelain
~~~

C2 — static checks/tests (PASS exit0, 82 OK):

~~~sh
docker run --rm --name cham3-m2-b1-independent-static --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -v /tmp/cham3-m2-b1-independent-qual:/work -w /work cham3-m2-independent-r3-final bash -c 'bash build/scripts/run_static_checks.sh && sh tests/run_tests.sh static' > /tmp/cham3-m2-b1-static.log 2>&1
~~~

C3/C4 — full tmpfs and five wrapper cases (both PASS exit0):

~~~sh
docker run --rm --name cham3-m2-b1-independent-storage --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home --tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700 -v /tmp/cham3-m2-b1-independent-qual:/work -w /work cham3-m2-independent-r3-final sh tests/m2/storage/check_reserve_full_disk.sh out/m2/storage > /tmp/cham3-m2-b1-storage.log 2>&1
docker run --rm --name cham3-m2-b1-independent-wrapper --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home --tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700 -e CHAM3_M2_RESERVE_PROBE=/work/out/m2/storage/target/release/cham3_reserve_probe -e CHAM3_M2_WRAPPER_DISK=/full-disk -v /tmp/cham3-m2-b1-independent-qual:/work -w /work cham3-m2-independent-r3-final python3 -I -m unittest discover -s tests/m2/harness -p test_m2_reserve_probe.py -k wrapper -v > /tmp/cham3-m2-b1-wrapper.log 2>&1
~~~

C5 — explicit differential output (PASS exit0):

~~~sh
docker run --rm --name cham3-m2-b1-independent-differentials --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/work/out/m2/differential_target -v /tmp/cham3-m2-b1-independent-qual:/work -w /work cham3-m2-independent-r3-final bash -c 'cargo test --frozen -p cham3_registryd --test trusted_time_model -- --nocapture && cargo test --frozen -p cham3_registryd --test admission_scenarios -- --nocapture' > /tmp/cham3-m2-b1-differentials.log 2>&1
~~~

C6 — reviewer controls. Plugin returned temporary directory:
`/tmp/codex-security-artifacts-d4500ced5481e8c5199f8f2af27d7e3604d12a4e45d3741ce4d7d070ac34572e`.
Clone was created there with `git clone --no-hardlinks /home/dev/cham3 <directory>/repro`, then `git -C <directory>/repro checkout --detach c9129bc`. The two complete snippet blocks below were saved as `<directory>/artifacts/policy_reviewer_snippet.rs` and `registry_reviewer_snippet.rs`. Test generation:

~~~python
from pathlib import Path
root = Path("/tmp/codex-security-artifacts-d4500ced5481e8c5199f8f2af27d7e3604d12a4e45d3741ce4d7d070ac34572e")
for crate, source, snippet in [
    ("policyd", "current_authorization", "policy"),
    ("registryd", "trusted_time", "registry"),
]:
    p = root / "repro" / "core" / crate / "tests"
    (p / "reviewer_b1.rs").write_text(
        (p / (source + ".rs")).read_text()
        + "\n"
        + (root / "artifacts" / (snippet + "_reviewer_snippet.rs")).read_text()
    )
~~~

Final debug command (FAIL exit1, 11 PASS/3 FAIL assertions):

~~~sh
docker run --rm --name cham3-m2-b1-reviewer-all --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/work/out/reviewer-target -v /tmp/codex-security-artifacts-d4500ced5481e8c5199f8f2af27d7e3604d12a4e45d3741ce4d7d070ac34572e/repro:/work -w /work cham3-m2-independent-r3-final bash -c 'cargo test --frozen -p cham3_policyd --test reviewer_b1 reviewer_ -- --nocapture; policy_exit=$?; cargo test --frozen -p cham3_registryd --test reviewer_b1 reviewer_ -- --nocapture; registry_exit=$?; test "$policy_exit" -eq 0 && test "$registry_exit" -eq 0' > /tmp/codex-security-artifacts-d4500ced5481e8c5199f8f2af27d7e3604d12a4e45d3741ce4d7d070ac34572e/artifacts/reviewer_debug.log 2>&1
~~~

Final release command (same FAIL exit1, same 11 PASS/3 FAIL):

~~~sh
docker run --rm --name cham3-m2-b1-reviewer-all-release --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/work/out/reviewer-target -v /tmp/codex-security-artifacts-d4500ced5481e8c5199f8f2af27d7e3604d12a4e45d3741ce4d7d070ac34572e/repro:/work -w /work cham3-m2-independent-r3-final bash -c 'cargo test --frozen --release -p cham3_policyd --test reviewer_b1 reviewer_ -- --nocapture; policy_exit=$?; cargo test --frozen --release -p cham3_registryd --test reviewer_b1 reviewer_ -- --nocapture; registry_exit=$?; test "$policy_exit" -eq 0 && test "$registry_exit" -eq 0' > /tmp/codex-security-artifacts-d4500ced5481e8c5199f8f2af27d7e3604d12a4e45d3741ce4d7d070ac34572e/artifacts/reviewer_release.log 2>&1
~~~

Two initial reviewer-only source mistakes (head_view_t comparison and identity_state argument count) caused compilation errors during test construction and were corrected; those attempts are not candidate failures. Only the final compiled runtime assertions support findings. The implementation was never patched.

C7/C8 — CI (PASS):

~~~sh
gh api repos/Kayyo321/cham3/actions/runs/38609857266 --jq '{head_sha,conclusion,status,workflow_id}'
gh api repos/Kayyo321/cham3/actions/runs/38609857244 --jq '{head_sha,status,conclusion}'
gh api repos/Kayyo321/cham3/actions/runs/38609857191 --jq '{head_sha,status,conclusion}'
gh api repos/Kayyo321/cham3/actions/runs/38609857266/jobs --paginate --jq '.jobs[] | {name,conclusion,steps:[.steps[] | {name,conclusion}]}'
gh api repos/Kayyo321/cham3/actions/runs/38609857244/jobs --paginate --jq '.jobs[] | {name,conclusion,steps:[.steps[] | {name,conclusion}]}'
gh api repos/Kayyo321/cham3/actions/runs/38609857191/jobs --paginate --jq '.jobs[] | {name,conclusion,steps:[.steps[] | {name,conclusion}]}'
gh api repos/Kayyo321/cham3/actions/artifacts/11918223662 --jq '{name,id,size_in_bytes,digest,expired,workflow_run}'
gh api repos/Kayyo321/cham3/actions/artifacts/11918223662/zip > /tmp/cham3-m2-b1-ci-artifact.zip
sha256sum /tmp/cham3-m2-b1-ci-artifact.zip
unzip -q /tmp/cham3-m2-b1-ci-artifact.zip -d /tmp/cham3-m2-b1-ci-artifact
~~~

All requested runs completed/success at c9129bc. Every returned job and step succeeded. Artifact `m2_qualification_amd64`, 109619 bytes, not expired; actual ZIP SHA-256
`d8070e9b69a5ee20c1aa659009146dc019dd33e5123cb385423dbb6db652d3b5`,
equal to both supplied digest and GitHub metadata.
[CI m2](https://github.com/Kayyo321/cham3/actions/runs/38609857266),
[CI m1](https://github.com/Kayyo321/cham3/actions/runs/38609857244),
[CI m0](https://github.com/Kayyo321/cham3/actions/runs/38609857191).
CI package/image/boot/capture results are CI evidence, not independent local reruns of those unchanged M0/M1 suites.

C9 — plugin (PASS execution, completed, one finding):

~~~sh
python3 /home/dev/.codex/plugins/cache/openai-curated-remote/codex-security/0.1.32/scripts/config_preflight.py --profile security_diff_scan --cwd /home/dev/cham3 --runtime-check delegation_available=true --runtime-check goal_tools_available=true --multi-agent-runtime-owner native --multi-agent-runtime-version v2 --multi-agent-session-cap 4 --multi-agent-runtime-provenance thread-context
~~~

Exit0, ready, no unmet capabilities and no persistent config change. The skill explicitly authorizes bounded delegated source review: dedicated preflight/independent architecture worker, policy/lineage reviewer, and oracle/harness reviewer; parent reviewed storage/clock/recovery and validated candidates. Workers did not author the implementation or publish changes.

Tools actually used: `start_codex_security_prompt_only_scan(mode=diff,targetPath=/home/dev/cham3,scope=.,diffTarget=range e7803ef..c9129bc)`; preflight progress; advisory `get_codex_security_daybreak_access`; generated model/early draft; `prepare/list_codex_security_review_items` (10 source items), supplemented by all 34 changed paths; `record/list_codex_security_discovery_candidates`; `record_codex_security_candidate_validations`; `record_candidate_attack_paths`; managed supplemental artifacts; final complete draft; `complete_codex_security_scan` once; `get_codex_security_completed_scan`. No second scan or failed/cancelled replacement.

- Plugin: Codex Security, `codex-security` **0.1.32**, explicitly invoked `security-diff-scan` workflow.
- Scan: `690a6945-d328-40c8-9a25-bdab39061ac6`; completed 2026-10-14T19:08:41.316598Z.
- R01 canonical candidate `candidate-988b588e36acbbf0`: confirmed/promoted Medium.
- R02 candidate `candidate-7e1a5cad6472ecbd`: confirmed E6 bug, attack-path policy ignore (audit duplication without demonstrated lower-privilege security gain); retained in this acceptance review.
- Daybreak advisory: not_granted; scan still ran. No access policy changed.
- Measured completion metadata (coverage complete, four threads, source codex_rollout): total_tokens **13500748**, input_tokens **13454079**, cached_input_tokens **12900224**. These are returned accounting values, not an estimate of uncached work.

## Oracles, mutant sensitivity and evidence limits

Trusted vectors are exactly regenerated by `trusted_time_vectors.py --check` (also mandatory in C1). Cases:165; steps4813, comprising samples2889, floors766, adoptions457, restarts701 (same-boot452/new-boot249). The Python model and Rust implementation independently encode offset arithmetic, but both operate on provided clock/floor fixtures. The differential always calls `observe(..., true)`, not a failing ledger; compares each produced floor immediately, then places the **model** floor into later restart fixtures. It can catch an immediate wrong output, yet does not prove actual persistence of the Rust floor, E6 episode persistence, clock sharing failures, concurrent writers or OS clock behavior. Agreement alone cannot certify those service rules; R01/R02 demonstrate that gap.

ECR vectors:19 valid/96 invalid/18 admission-invalid;27 scenarios/365 steps. Pure layer-7 Rust driver filters15 scenarios/72 steps/121 comparisons and has0 disagreements. Four E4 additions cover: ±300 accept/±301 reject; parent−300 accept/−301 reject; stale_head precedes timestamp_rule; timestamp_rule precedes base_state_mismatch. No-now published steps retain historical semantics. Admission driver defaults admitted_at metadata to zero, so it is not Option-B service evidence.

The entire `test_m2_registry_policy` ran independently within C1: both tests passed, all119 mutations compiled and their named tests failed, unmodified controls passed. Source/AST diff identified22 new mutations and12 repointed stage-A strings, each original anchor occurs exactly once. The author’s22 detailed assertion logs were cross-checked against current test source. This discriminating-assertion matrix describes those logs plus the independently successful sensitivity harness; it does not label author logs as independent executions.

| New seeded defect | Discriminating assertion / reason | Assessment |
|---|---|---|
| offset_follows_backward_samples | trusted_time_model: first corrected output regresses;2080 disagreements | PASS, intended offset defect |
| same_boot_floors_ignored | restarted trusted T1 instead of T1+3600 | PASS |
| new_boot_starts_from_the_wall_clock | trusted T1−3600 instead of persisted T1+7200 | PASS |
| observed_counter_follows_the_wall_clock | eligible instead of not_started after post-admission forward step | PASS |
| anomaly_at_exactly_300_seconds | record at lag300 instead of none | PASS |
| timestamp_window_unchecked | refusal helper sees admitted stale genesis | PASS |
| timestamp_parent_clause_unchecked | refusal helper sees admitted too-early child | PASS |
| concurrent_floor_write_regresses | merge_floor returns lower earlier floor | PASS for helper; original test is sequential |
| timestamp_rule_skipped_at_admission | first E4 refusal is admitted; later stale_head cascade follows | PASS, first mismatch is intended |
| branches_survive_a_new_boot | temporary standing current instead of inactive | PASS |
| new_boot_not_detected | same temporary-standing assertion | PASS |
| raise_adopted_without_the_ledger | failed raise nevertheless advances trusted T1→T1+3600 | PASS |
| time_sample_adopted_without_the_ledger | failed reported raise advances T1+10→T1+20 | PASS |
| time_sample_of_another_boot_accepted | foreign boot returns time_view instead of malformed_request | PASS |
| floor_never_refreshed | anchor BOOTTIME1000 instead of1060 at60s | PASS |
| anomaly_recorded_at_every_operation | two records in ordinary uninterrupted episode instead of one | PASS |
| anomaly_not_recorded_by_reads | missing lag301 record | PASS |
| admission_on_a_stale_clock | unsampleable source nevertheless admits trunk | PASS |
| admission_observed_time_not_recorded | admitted observed0 instead of100 | PASS |
| observed_start_not_checked | eligible instead of not_started | PASS |
| policyd_relies_on_an_unreported_sample | policy offset includes refused sample instead of acknowledged shared offset | PASS |
| policyd_evaluates_another_boot | eligible instead of unknown(clock_unavailable) | PASS |

The anomaly test’s restart portion only checks that the journal remains usable. It never rereads that user and verifies event count, so cannot discriminate R02. Policy forward-report tests use successful reporting and malformed-peer fallback; none exercises a failed ledger after snapshot acquisition. The new-boot baseline covers clean shutdown and a timed branch; reviewer controls add pending termination/network slot and old-marker retry. Twelve stage-A repointed mutation anchors alter match strings only and remain uniquely applied to the intended source path.

## Retained independent evidence

Canonical scan directory (persistent, owner-local):
`/home/dev/.codex/state/plugins/codex-security/scans/cham3/c9129bca0e6c4b1e6caf72774e05a27e09a4d542_20261014T185048Z_02bqv7ly`.

[Generated security report](/home/dev/.codex/state/plugins/codex-security/scans/cham3/c9129bca0e6c4b1e6caf72774e05a27e09a4d542_20261014T185048Z_02bqv7ly/report.md)
SHA-256 `bfae63f89e506757d2236986f7d70246fbc48fbe9b5d05506ba7bf31aa05e1e6`.
`scan-manifest.json/findings.json/coverage.json` are sealed; the report is the generated readable projection. Full raw clone/build trees were not imported.

Under that directory:

| Evidence path | SHA-256 |
|---|---|
| artifacts/02_discovery/validation_artifacts/b1/policy_reviewer_snippet.rs | c33ade322b722bee4494f532fd2e7ae6b7cff00d7741760f32412f6af983b25b |
| artifacts/02_discovery/validation_artifacts/b1/registry_reviewer_snippet.rs | 014db1de90301e29a0c03e19133929705598164f4a71a9496cdaf5109b1cba47 |
| artifacts/02_discovery/validation_artifacts/b1/reviewer_debug.log | 37d1dabff9c4d0abb1dd8be2f187d3ea3b5031dd802b5df066877f1dd38cc151 |
| artifacts/02_discovery/validation_artifacts/b1/reviewer_release.log | 3ca5db35399cfc50a622d930b9f114ea1b2522d5a6984e5dbfcbe9e77c969a38 |
| artifacts/03_coverage/qualification.log | ad6242252cfb011d15df5af7c616394efbd2a724c54486bccca1de87073a3f16 |
| artifacts/03_coverage/static.log | d0973f3d0e5963887c4ad0bcf59ffd9692cfca8bfa2cf3ce6f686702319aa138 |
| artifacts/03_coverage/storage.log | 90ef61c8f93366abc20569fc86ff431de9bf307f2d0a4cb8cc89d80ac4e9740d |
| artifacts/03_coverage/wrapper.log | bb98269f84a5d8620e6555d996a02b2a26d99ee5c178ffa94b3f7bb2e6ec3643 |
| artifacts/03_coverage/differentials.log | 581171e391822e1bdce9439c2d82b17e4b2efb5af91d5c9ca92bb95faf53e2bf |

Also retained there: qualification summary/identity/toolchain; cargo_test_debug.json, cargo_test_release.json, python_harness.json, provider_oracle.json, format_differential.json, ecr_vector_oracle.log, trusted_time_vectors.log, vault_object_oracle.log; independently queried ci-run.json/ci-artifact-metadata.json. The original CI ZIP remains /tmp/cham3-m2-b1-ci-artifact.zip; published digest above is independently recomputable from GitHub. Local absolute evidence links are not GitHub-hosted artifacts; the self-contained reviewer sources below permit reproduction elsewhere.

## Owner questions and handoff

1. Clarify whether E4's “no other durable change” excludes independently required time-floor/ledger/anomaly housekeeping performed before timestamp refusal. The settled-clock refusal writes nothing. A literal no-write whole-call reading conflicts with §5/E6 sampling/persistence. No policy interpretation is approved here.
2. E6 refers to a user's operation **or evaluation** observing an anomaly. Current policyd reports only forward offsets; a backward sample between registry snapshot sampling and policy sampling is not separately reported. Should B1 evidence cover that per-user policy observation, or is the registry operation's sample the designated episode observation? This is a scope/wording question; no authorization bypass is claimed from backward samples.

Nonblocking documentation correction: storage specification line35's anchor byte diagram still says `version (1)`, while its text and `AnchorVersion` correctly say2. Correct that diagram during the revision; no architecture change is needed.

Next: implementer corrects R01/R02 on the implementation branch, updates evidence/status and handoff, and presents exact new candidate for independent re-review before B2. Reviewer did not merge, make owner decisions, alter implementation or run privileged host/time changes. Required hardware/actual crash/process/enforcement work stays explicitly NOT_RUN at its assigned later stage.

## Self-contained reviewer test sources

Save the following two blocks using the filenames in C6. They extend unchanged candidate fixtures and directly call the original libraries; all grants/users/keys are synthetic.

### policy_reviewer_snippet.rs

~~~rust

// Independent review controls: no production source changed.
#[derive(Clone)]
struct reviewer_ledger_fault_io_t { armed: Arc<AtomicBool> }
impl journal_io_t for reviewer_ledger_fault_io_t {
    fn write_all(&self, file: &mut fs::File, bytes: &[u8]) -> io::Result<()> { system_io_t.write_all(file, bytes) }
    fn sync_data(&self, file: &fs::File) -> io::Result<()> { system_io_t.sync_data(file) }
    fn sync_directory(&self, path: &Path) -> io::Result<()> { system_io_t.sync_directory(path) }
    fn append_file(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        if self.armed.load(Ordering::SeqCst) && path.file_name().is_some_and(|n| n == "entries") {
            return Err(io::Error::other("reviewer injected ledger write failure"));
        }
        system_io_t.append_file(path, bytes)
    }
}
#[test]
fn reviewer_time_sample_ledger_failure_denies_current_evaluation() {
    let root = temp_root("reviewer_sample_failure");
    let armed = Arc::new(AtomicBool::new(false));
    let mut world = world_at_with(&root, registry_config_t::default(), mandatory_policy_t::default(), Box::new(reviewer_ledger_fault_io_t {armed: armed.clone()}));
    let alice = test_user(1001); world.enroll(&alice); let editor = app("editor");
    let mut lifetime = persistent_lifetime(); lifetime.expiration = Some(cham3_identityd::index::utc_text((T1Seconds + 100) as i64).unwrap());
    let genesis = draft(&alice, &editor, commit_type_t::genesis, "reviewer failure").add(vec![file_grant("reviewer timed",alice.uid,"home:Files",&["read"],lifetime)]).sign(&alice);
    world.submit(&alice,&genesis);
    let identity = runtime(&alice,&editor); let request = files(alice.uid,"home:Files","read");
    assert!(is_eligible(world.evaluate(&identity,&request)), "positive control before forward sample");
    let base=world.time.current().unwrap(); let ahead=test_time_t::at(T1Seconds+200);
    ahead.set_available(true,T1Seconds+200,base.boottime);
    let mut policy=policy_with_clock(&world,&ahead,&kernel_boot_id());
    armed.store(true,Ordering::SeqCst);
    let decision=policy.evaluate(Launcher,&identity,&request).unwrap();
    let failed=world.registry.rollback_status(Policyd).unwrap().ledger_failed;
    println!("ledger_failed={failed}; current={decision:?}");
    assert!(failed,"fault reached ledger");
    let next=policy.evaluate(Launcher,&identity,&request).unwrap();
    println!("next={next:?}");
    assert_eq!(next,decision_t::unknown(unknown_reason_t::registry_unavailable));
    assert!(!is_eligible(decision), "mandatory denial: ledger failed during this evaluation");
}
#[test]
fn reviewer_forward_sample_without_fault_expires_the_grant() {
    let mut world=world("reviewer_sample_positive"); let alice=test_user(1001); world.enroll(&alice); let editor=app("editor");
    let mut lifetime=persistent_lifetime(); lifetime.expiration=Some(cham3_identityd::index::utc_text((T1Seconds+100) as i64).unwrap());
    let genesis=draft(&alice,&editor,commit_type_t::genesis,"reviewer positive").add(vec![file_grant("reviewer positive timed",alice.uid,"home:Files",&["read"],lifetime)]).sign(&alice);
    world.submit(&alice,&genesis);
    let base=world.time.current().unwrap(); let ahead=test_time_t::at(T1Seconds+200); ahead.set_available(true,T1Seconds+200,base.boottime);
    let mut policy=policy_with_clock(&world,&ahead,&kernel_boot_id());
    assert_eq!(policy.evaluate(Launcher,&runtime(&alice,&editor),&files(alice.uid,"home:Files","read")).unwrap(),denied(deny_reason_t::expired));
}

#[test]
fn reviewer_network_time_sample_failure_stays_offline() {
    let mut bad=false;
    for mode in ["direct","tor"] {
        let root=temp_root(&format!("reviewer_network_{mode}")); let armed=Arc::new(AtomicBool::new(false));
        let mut world=world_at_with(&root,registry_config_t::default(),mandatory_policy_t::default(),Box::new(reviewer_ledger_fault_io_t{armed:armed.clone()}));
        let alice=test_user(1001); world.enroll(&alice); let editor=app("editor");
        let genesis=draft(&alice,&editor,commit_type_t::genesis,&format!("reviewer network {mode}")).add(vec![network_grant(&format!("reviewer {mode}"),alice.uid,mode,persistent_lifetime())]).sign(&alice);
        world.submit(&alice,&genesis);
        let base=world.time.current().unwrap(); let ahead=test_time_t::at(T1Seconds+200); ahead.set_available(true,T1Seconds+200,base.boottime);
        let mut policy=policy_with_clock(&world,&ahead,&kernel_boot_id()); armed.store(true,Ordering::SeqCst);
        let decision=policy.network_mode(Launcher,&runtime(&alice,&editor)).unwrap();
        println!("network {mode} after failed ledger={decision:?}");
        bad |= matches!(decision,network_decision_t::mode{..});
    }
    assert!(!bad,"network must be Offline after ledger failure");
}

#[test]
fn reviewer_equal_start_and_error_at_admission() {
    let mut world=world("reviewer_equal_start"); let alice=test_user(1001); world.enroll(&alice); let editor=app("editor");
    // Forward error is present at admission; start equals admission's trusted time.
    world.time.step_wall_clock(T1Seconds+3600);
    let mut lifetime=persistent_lifetime(); lifetime.effective_start=cham3_identityd::index::utc_text((T1Seconds+3600) as i64).unwrap();
    let genesis=draft(&alice,&editor,commit_type_t::genesis,"reviewer equal").timestamp(&lifetime.effective_start).add(vec![file_grant("equal",alice.uid,"home:Equal",&["read"],lifetime)]).sign(&alice);
    world.submit(&alice,&genesis);
    assert!(is_eligible(world.evaluate(&runtime(&alice,&editor),&files(alice.uid,"home:Equal","read"))));
}
#[test]
fn reviewer_restore_and_merge_keep_future_start_observed_delay() {
    for kind in ["restore","merge"] {
        let mut world=world(&format!("reviewer_future_{kind}")); let alice=test_user(1001); world.enroll(&alice); let editor=app("editor");
        let old=file_grant("historical",alice.uid,"home:Future",&["read"],persistent_lifetime());
        let genesis=draft(&alice,&editor,commit_type_t::genesis,"future genesis").add(vec![old.clone()]).sign(&alice); world.submit(&alice,&genesis);
        let head=draft(&alice,&editor,commit_type_t::revision,"future remove").on(&genesis).remove(vec![old.grant_id]).sign(&alice); world.submit(&alice,&head);
        let mut lifetime=persistent_lifetime(); lifetime.effective_start=cham3_identityd::index::utc_text((T1Seconds+160) as i64).unwrap();
        let grant=file_grant(kind,alice.uid,"home:Future",&["read"],lifetime);
        let commit=if kind=="restore" {
            draft(&alice,&editor,commit_type_t::restore,"future restore").on(&head).restore_from(&genesis).add(vec![grant.clone()]).sign(&alice)
        } else {
            let branch=draft(&alice,&editor,commit_type_t::temporary_grant,"future branch").on(&head).add(vec![file_grant("branch",alice.uid,"home:Other",&["read"],timed_lifetime("2026-10-20T00:00:00Z"))]).temporary_authority().sign(&alice);
            world.submit(&alice,&branch);
            draft(&alice,&editor,commit_type_t::merge,"future merge").merge(&head,&[&branch]).add(vec![grant.clone()]).sign(&alice)
        };
        world.submit(&alice,&commit);
        let view=world.registry.lineage_head(Policyd,alice.uid,&commit.payload.lineage_id).unwrap().unwrap();
        assert_eq!(view.origins[&grant.grant_id].commit_id,commit.commit_id);
        assert_eq!(view.origins[&grant.grant_id].admitted.trusted,(T1Seconds+60) as i64);
        world.time.step_wall_clock(T1Seconds+200);
        let identity=runtime(&alice,&editor); let request=files(alice.uid,"home:Future","read");
        assert_eq!(world.evaluate(&identity,&request),denied(deny_reason_t::not_started));
        world.time.advance(99); assert_eq!(world.evaluate(&identity,&request),denied(deny_reason_t::not_started));
        world.time.advance(1); assert!(is_eligible(world.evaluate(&identity,&request)));
    }
}

~~~

### registry_reviewer_snippet.rs

~~~rust

#[test]
fn reviewer_anomaly_episode_survives_same_boot_restart() {
    let root=temp_root("reviewer_anomaly_restart"); let time=test_time_t::at(T1Seconds);
    let alice=test_user(1001); let registry=start(&root,"boot-1",&time); enroll(&registry,&alice);
    time.step_wall_clock(T1Seconds-400);
    registry.lineage_head(Policyd,alice.uid,&NoLineage).unwrap();
    assert_eq!(anomalies(&root,alice.uid).len(),1);
    registry.shutdown().unwrap(); drop(registry);
    let registry=start(&root,"boot-1",&time);
    registry.lineage_head(Policyd,alice.uid,&NoLineage).unwrap();
    println!("after restart anomalies={:?}",anomalies(&root,alice.uid));
    assert_eq!(anomalies(&root,alice.uid).len(),1,"same episode never returned within 300s");
}
#[test]
fn reviewer_anomaly_pending_blocks_admission() {
    let root=temp_root("reviewer_anomaly_pending"); let time=test_time_t::at(T1Seconds);
    let io=reviewer_journal_fault_io_t::default(); let alice=test_user(1001);
    let registry=start_with(&root,"boot-1",&time,Box::new(io.clone())); enroll(&registry,&alice);
    io.armed.store(true,Ordering::SeqCst); time.step_wall_clock(T1Seconds-400);
    registry.lineage_head(Policyd,alice.uid,&NoLineage).unwrap();
    let genesis=draft(&alice,&app("editor"),commit_type_t::genesis,"pending admission").sign(&alice);
    assert_eq!(code(admit(&registry,&alice,&genesis)),"ipc/restriction_not_durable");
    assert_eq!(registry.rollback_status(Policyd).unwrap().pending_restrictions,vec![alice.uid]);
}
#[derive(Clone,Default)]
struct reviewer_journal_fault_io_t {armed: Arc<AtomicBool>}
impl journal_io_t for reviewer_journal_fault_io_t {
    fn write_all(&self,file:&mut fs::File,bytes:&[u8])->io::Result<()> {
        if self.armed.load(Ordering::SeqCst) {return Err(io::Error::other("reviewer journal failure"));}
        system_io_t.write_all(file,bytes)
    }
    fn sync_data(&self,file:&fs::File)->io::Result<()> {system_io_t.sync_data(file)}
    fn sync_directory(&self,path:&Path)->io::Result<()> {system_io_t.sync_directory(path)}
}
#[test]
fn reviewer_concurrent_floor_publications_keep_maximum_offsets() {
    let root=temp_root("reviewer_concurrent_floors"); let time=test_time_t::at(T1Seconds);
    let registry=start(&root,"boot-1",&time); time.advance(60);
    let barrier=Arc::new(std::sync::Barrier::new(9)); let mut writers=Vec::new();
    for i in 0..8u64 {
        let registry=registry.clone(); let barrier=barrier.clone();
        writers.push(std::thread::spawn(move || {
            barrier.wait();
            registry.time_sample(Policyd,"boot-1",StartBoottime+60,T1Seconds+60+i*100).unwrap();
            registry.set_policy_epoch(Admin,2,1).unwrap();
        }));
    }
    barrier.wait(); for writer in writers {writer.join().unwrap();}
    registry.shutdown().unwrap();
    let floor=newest_anchor(&root).time_floor.unwrap();
    assert_eq!(floor.trusted-floor.boottime,T1Seconds-StartBoottime+700);
    assert_eq!(floor.observed,60);
    drop(registry); time.step_wall_clock(T1Seconds);
    let registry=start(&root,"boot-1",&time); assert_eq!(trusted_now(&registry),T1Seconds+60+700);
}
#[test]
fn reviewer_new_boot_pending_termination_replays_from_ledger() {
    let root=temp_root("reviewer_termination_crash"); let time=test_time_t::at(T1Seconds); let alice=test_user(1001); let editor=app("editor");
    let registry=start(&root,"boot-1",&time); enroll(&registry,&alice);
    let genesis=draft(&alice,&editor,commit_type_t::genesis,"termination genesis").sign(&alice);
    let network=network_grant("temporary network",alice.uid,"direct",timed_lifetime("2026-10-20T00:00:00Z"));
    let branch=draft(&alice,&editor,commit_type_t::temporary_grant,"termination branch").on(&genesis).add(vec![network.clone()]).temporary_authority().sign(&alice);
    admit(&registry,&alice,&genesis).unwrap(); admit(&registry,&alice,&branch).unwrap(); registry.shutdown().unwrap(); drop(registry);
    let io=reviewer_journal_fault_io_t::default(); io.armed.store(true,Ordering::SeqCst);
    let registry=start_with(&root,"boot-2",&time,Box::new(io.clone()));
    assert_eq!(registry.standing(alice.uid,&network.grant_id),Ok(standing_t::inactive));
    assert_eq!(registry.rollback_status(Policyd).unwrap().pending_restrictions,vec![alice.uid]);
    drop(registry); io.armed.store(false,Ordering::SeqCst);
    let registry=start_with(&root,"boot-2",&time,Box::new(io));
    assert_eq!(registry.run_decision(),run_decision_t::ledger_replayed);
    let view=registry.lineage_head(Policyd,alice.uid,&genesis.payload.lineage_id).unwrap().unwrap();
    assert_eq!(view.trunk_head.map(|h|h.commit_id),Some(genesis.commit_id)); assert!(view.temporary_heads.is_empty());
    let count=decoded(&root,alice.uid).iter().filter(|r|matches!(r.body,record_body_t::event(registry_event_t::branch_terminated{..}))).count();
    assert_eq!(count,1); assert_eq!(registry.standing(alice.uid,&network.grant_id),Ok(standing_t::inactive));
}

#[test]
fn reviewer_invalid_anchor_discards_floor_and_quarantines() {
    let root=temp_root("reviewer_invalid_floor"); let time=test_time_t::at(T1Seconds); let alice=test_user(1001);
    let registry=start(&root,"boot-1",&time); enroll(&registry,&alice);
    time.step_wall_clock(T1Seconds+3600); registry.shutdown().unwrap(); drop(registry);
    for name in SlotFileNames {fs::write(root.join("anchor").join(name),b"invalid").unwrap();}
    time.step_wall_clock(T1Seconds); let registry=start(&root,"boot-1",&time);
    assert_eq!(trusted_now(&registry),T1Seconds);
    assert!(!registry.rollback_status(Policyd).unwrap().quarantined.is_empty());
    assert!(registry.identity_state(Identityd,alice.uid,0).is_err());
}
#[test]
fn reviewer_foreign_run_ledger_is_not_a_time_floor() {
    let root=temp_root("reviewer_foreign_ledger"); let foreign=temp_root("reviewer_foreign_source"); let time=test_time_t::at(T1Seconds); let alice=test_user(1001);
    let registry=start(&root,"boot-1",&time); enroll(&registry,&alice); drop(registry);
    let foreign_registry=start(&foreign,"boot-1",&time); time.step_wall_clock(T1Seconds+3600); assert_eq!(trusted_now(&foreign_registry),T1Seconds+3600); drop(foreign_registry);
    fs::copy(foreign.join("ledger/validity"),root.join("ledger/validity")).unwrap(); fs::copy(foreign.join("ledger/entries"),root.join("ledger/entries")).unwrap();
    time.step_wall_clock(T1Seconds); let registry=start(&root,"boot-1",&time);
    assert_eq!(registry.run_decision(),run_decision_t::contained); assert_eq!(trusted_now(&registry),T1Seconds);
    assert_eq!(registry.rollback_status(Policyd).unwrap().contained,vec![alice.uid]);
}
#[derive(Clone,Default)]
struct reviewer_start_failure_io_t;
impl journal_io_t for reviewer_start_failure_io_t {
    fn write_all(&self,_file:&mut fs::File,_bytes:&[u8])->io::Result<()> {Err(io::Error::other("reviewer before termination durability"))}
    fn sync_data(&self,file:&fs::File)->io::Result<()> {system_io_t.sync_data(file)}
    fn sync_directory(&self,path:&Path)->io::Result<()> {system_io_t.sync_directory(path)}
    fn replace_file(&self,path:&Path,bytes:&[u8])->io::Result<()> {
        if path.file_name().is_some_and(|n|n=="slot_a"||n=="slot_b") {return Err(io::Error::other("reviewer marker failure"));}
        system_io_t.replace_file(path,bytes)
    }
}
#[test]
fn reviewer_failed_new_boot_start_detects_the_old_boot_again() {
    let root=temp_root("reviewer_old_boot_marker"); let time=test_time_t::at(T1Seconds); let alice=test_user(1001); let editor=app("editor");
    let registry=start(&root,"boot-1",&time); enroll(&registry,&alice); let genesis=draft(&alice,&editor,commit_type_t::genesis,"old boot").sign(&alice);
    let grant=file_grant("old boot branch",alice.uid,"home:Tmp",&["read"],one_shot_lifetime());
    let branch=draft(&alice,&editor,commit_type_t::temporary_grant,"old marker branch").on(&genesis).add(vec![grant.clone()]).temporary_authority().sign(&alice);
    admit(&registry,&alice,&genesis).unwrap(); admit(&registry,&alice,&branch).unwrap(); registry.shutdown().unwrap(); drop(registry);
    assert!(registry_t::open(&root,owner_of(&root),config("boot-2"),Box::new(reviewer_start_failure_io_t),Box::new(time.clone())).is_err());
    assert_eq!(newest_anchor(&root).run_marker.unwrap().boot_id,"boot-1");
    let registry=start(&root,"boot-2",&time); assert_eq!(registry.standing(alice.uid,&grant.grant_id),Ok(standing_t::inactive));
    assert_eq!(decoded(&root,alice.uid).iter().filter(|r|matches!(r.body,record_body_t::event(registry_event_t::branch_terminated{..}))).count(),1);
}

fn reviewer_storage_image(root:&Path,uid:u32)->Vec<Vec<u8>> {
    let mut out=vec![fs::read(journal_path(root,uid)).unwrap()];
    for name in ["anchor/slot_a","anchor/slot_b","ledger/validity","ledger/entries"] {
        out.push(fs::read(root.join(name)).unwrap_or_default());
    }
    out
}
#[test]
fn reviewer_timestamp_refusal_leaves_settled_storage_byte_identical() {
    let root=temp_root("reviewer_timestamp_bytes"); let time=test_time_t::at(T1Seconds+301);
    let registry=start(&root,"boot-1",&time); let alice=test_user(1001); enroll(&registry,&alice);
    let before=reviewer_storage_image(&root,alice.uid);
    let genesis=draft(&alice,&app("editor"),commit_type_t::genesis,"refusal bytes").sign(&alice);
    assert_eq!(code(admit(&registry,&alice,&genesis)),"admission/timestamp_rule");
    assert_eq!(reviewer_storage_image(&root,alice.uid),before);
}
#[test]
fn reviewer_quarantined_user_anomaly_does_not_write_its_journal() {
    let root=temp_root("reviewer_quarantined_anomaly"); let time=test_time_t::at(T1Seconds); let alice=test_user(1001);
    let registry=start(&root,"boot-1",&time); enroll(&registry,&alice); registry.shutdown().unwrap(); drop(registry);
    for name in SlotFileNames {fs::write(root.join("anchor").join(name),b"invalid").unwrap();}
    let registry=start(&root,"boot-1",&time); let before=records_of(&root,alice.uid);
    time.step_wall_clock(T1Seconds-400);
    registry.lineage_head(Policyd,alice.uid,&NoLineage).unwrap();
    assert_eq!(records_of(&root,alice.uid),before);
    assert!(registry.rollback_status(Policyd).unwrap().clock.anomaly.open);
}

~~~
