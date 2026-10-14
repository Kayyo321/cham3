# M2-006 stage B1 revision 2 — independent Codex re-review

**Verdict: CHANGES_REQUESTED** at `77a3d0e65072b60f4ce1780a709dd342fdda67f7`. R01 is resolved. R02 preserves uninterrupted episodes across restarts, but does not close an episode when registryd's startup itself samples an in-range clock. A later genuine episode is consequently omitted from the journal. This is one Low acceptance/audit defect, with no demonstrated permission bypass.

- State: IN_REVIEW → CHANGES_REQUESTED. Role/provider: independent REVIEWER / Codex. No implementation authored, edited, merged or approved by this review.
- Authority: PROJECT.md; AGENTS.md; engineering workflow/testing/security invariants; approved ADR 0006/E5 and ADR 0007 §5, §8 Option B, E4/E6 and their owner clarifications. Task: docs/tasks/M2-cryptographic-contract-core.md, bounded B1 revision-2 handoff and finding resolutions.
- Exact base: `b4e4cdc124a82faaed15aeeda8ca90b48fe6a43d`; candidate is its one-commit child. Exact docs-only follow-up: `0ec854de46af32f0e6625d0c59deace6e3d4ba9a`, parent exactly candidate, 46 paths all under docs/.
- Owner clone `/home/dev/cham3`: implementation branch at `0ec854d`, initially/finally clean, no stashes or changes moved aside.
- Review publication: `review/m2-006-b1-r2-security`, separate worktree `/home/dev/cham3-review-m2-006-b1-r2-security`, base `origin/main` = `9cf882503848f8c6cc4938a588cc9fc062a9392d`. Adds only this record. No implementation-branch commit or push.
- Prior review: `792f5f0f8836f63f5823e9e98c514bb7b2df93ff`, record SHA-256 `534a20807969d586f7c80b59cae0b934060aae1922eed2e7deed9f1584f3e5f9`, independently verified with `git show` and `sha256sum`.
- Environment: Debian 13.7 x86_64, Linux 7.1.13+deb13-amd64, UID/GID 1001. Docker access directly through existing group 119; `docker ps` exit 0. Image `cham3-m2-independent-r3-final` = `sha256:4528aed8d303c7255b9138392a4b2f7973a1b944b140cac66a9da40360ff3e56`. Unprivileged containers as UID/GID 1001, `--ulimit nofile=1024:1024`; host nofile 524288. No privileged host changes, real clock changes or production secrets used.

## Finding M2-006-B1-R2-01 — Low — process the startup sample's episode transition

**Root control:** `core/registryd/src/clock.rs:260–266` (changed code), with startup at `core/registryd/src/registry.rs:846–850` and duplicate suppression at `:1422–1434`.

`registry_t::open` takes a real sample from its own `time_source` and uses it to initialize the clock. `trusted_clock_t::start` restores the maximum same-boot anomaly mark but never assesses that sampled value against the restored episode. It initializes realtime/BOOTTIME state and publishes the unchanged mark in the startup anchor. Thus an actual in-range startup sample fails to close a resumed open episode.

**Reproduction against unmodified production source, debug and release:** at trusted T1, sample T1−400 and record episode 1 for Alice. Shut down cleanly, or drop the registry uncleanly. Set realtime to T1 and restart in the same boot. Startup samples T1, within the required 300 seconds, but publishes mark 2 (episode 1 open), rather than mark 3 (episode 1 closed). Before the next IPC operation set realtime back to T1−400. `lineage_head` retains episode 1, and `record_anomaly` finds Alice's prior boot/episode-1 event and writes nothing. Actual episodes remain `[("boot-1", 1)]`; expected `[("boot-1", 1), ("boot-1", 2)]`.

The same test includes positive controls: clean and unclean restart while startup's sample remains anomalous both preserve episode 1 without duplication. Those pass. Both in-range-startup cases fail in both profiles. See C6 and the complete test below.

**Requirement and counterevidence:** approved E6 says an episode ends when a sample is within 300 s of trusted time again (ADR 0007:77), and its clarification identifies registryd's own sample as the observation that counts (:84). Neither exempts startup. Restart alone must not close an episode; that does not exempt an actual in-range clock sample consumed during restart. Per-user attribution concerns anomaly journal reporting, whereas this closure is global state and needs no user attribution. An independent source/spec validation corroborated this interpretation; that worker did not execute the test or modify source.

**Impact:** missing anomaly audit evidence and incorrect episode continuity/status. The event itself does not alter grants, key state or policy. No lower-privileged clock/restart manipulation or authorization gain is established, so this remains Low and is suppressed as an exploit finding in Codex Security. The B1 acceptance defect still requires correction.

**Required:** reconcile the actual startup sample with the recovered episode state, and make any resulting transition durable before serving. Preserve uninterrupted anomalous-start continuity, same-boot maximum merge, new-boot reset and all current fail-closed clock/storage semantics. Add clean and unclean startup-in-range → renewed-lag regressions paired with anomalous-start positive controls. No new owner clock-policy decision is needed.

## Acceptance evidence

PASS below means component/container evidence only, not deployed OS enforcement. Every execution used the image, UID and nofile limit above. Command identifiers refer to the exact commands below.

| Check | Result | Evidence / limits |
|---|---|---|
| History, exact scope, forbidden inputs | PASS | C0; exactly fifteen candidate paths listed below. No dependency, lock, workflow, Dockerfile, oracle or vector changes; docs follow-up only docs/. |
| Original reviewer controls, unchanged | PASS | C5; 14/14 debug and 14/14 release, policyd 5/5 and registryd 9/9. Matches author's reviewer_reproductions/at_77a3d0e.log. Prior 11/14 at c9129bc is verified from the prior independent record and author's baseline log, not rerun in this bounded assignment. |
| R01 every failure exit in evaluation_time | PASS | Error propagation occurs before `self.clock.offset = Some(offset)` and before snapshot evaluation. Storage refusal → registry_unavailable; malformed → clock_unavailable; boot disagreement exits before sampling/report. evaluate/network_mode both return unknown/Offline. |
| R01 fresh and earlier-success policies | PASS | C6 policy matrix: eight combinations per profile (fresh/prior-success × storage/malformed × evaluate/network). All deny, own offset stays None or its previous accepted value, registry trusted value unchanged. Original network control includes Direct and Tor. |
| No-fault expiry, registry time_sample checks, failed-sample restrictions | PASS | Original controls, full current_authorization/trusted_time suites and source trace; no-fault forward sample expires immediately, foreign-boot/future BOOTTIME rejected, restriction remains possible when sample unavailable, pending anomaly blocks admission. The changed malformed assertion correctly expects unknown. |
| R02 mark/record formats and merge | PASS | Anchor/ledger exact appended u64, checksum/closed decoding preserved; nonzero episode record roundtrip, marks 2e/2e+1 over 99 episodes and reversed max merge checked in C6. Every anchor publication site carries publish_floor/max merge. No deployed data migration claimed. |
| R02 ordinary transitions/replay/restarts | PASS, scoped | C6 registry controls and candidate trusted_time suite: ledger-only closure retained, equal-payload distinct episodes for two users, clean/unclean continuity, new-boot reset, replayed boot/episode membership, contained and invalid-anchor starts. Quarantined journal remains byte-identical. |
| R02 startup-only closure | FAIL | C6, finding R2-01; actual startup mark 2 and only episode 1 in both clean/unclean in-range cases, debug and release. |
| Mark's authority effect | PASS, source scope | `clock_anomaly` replay pushes only an event; no grant/key/policy mutation. Mark is absent from `time_view_t` and temporal authorization arithmetic. A failed added ledger write invokes existing unverifiable/deny behavior; storage failure is not a permission grant. No claim of identical availability through failed I/O. |
| E4/E6 owner clarifications | PASS | Wording recorded as decided: refusal itself writes nothing; independent required clock persistence may precede it; registryd's own sample counts. No new decision or risk acceptance made. Settled timestamp-refusal storage remains byte-identical in original reviewer control. |
| Changed seeded defects | PASS | C7: six changed/replaced definitions independently tested; every mutation compiles and fails the intended assertion, unmodified controls pass. Matrix below. |
| Unit inventory | PASS | 239 debug + 239 release; five additions agree with inventory. C1 JSON receipts. |
| Full qualification/harness | PASS | C1 exit 0, `m2 qualification: PASS`, clean tracked source; unit 239+239, harness 149 with all 123 seeded registry/policy defects detected. Both Debian locks, offline metadata, sources, pins, rustfmt/clippy/naming, debug/release builds and soft backends pass. |
| ECR and trusted-time vectors/differentials | PASS | C1/C4: 19 valid/96 invalid/18 admission vectors; 27 journal scenarios/365 steps; trusted vectors 165, differential 4813/0; layer-7 15 scenarios/72 steps/121 comparisons/0 disagreements; format 5968/0. |
| Static checks/tests | PASS | C2 exit 0, static checks: PASS; 82 tests OK. |
| Full-filesystem wrapper and cases | PASS | C3 exit 0 on actual disposable 2 MiB tmpfs; debug/release reserve+ledger reports byte-identical to author's; wrapper 5/5. No anchor-ENOSPC guarantee inferred from the reserve probe. |
| CI comparison and artifact digest | PASS | C8; all requested runs at exact candidate completed/success; seven jobs and 88 steps all success. ZIP11928017840 actually downloaded; 109953 bytes; SHA-256 equals API and supplied digest. CI counts agree with local. |
| Vault protection/provider evidence | PASS, scoped | CI positive report protection_verified, exit0, dumpable0, unlocked_mappings0. Local qualification Memcheck probe and secure_memory (10 tests/0 errors) pass. Local positive protection path NOT_RUN; Argon2 secret/AD provider limitation carried forward. |
| Codex Security | PASS execution | C9, plugin 0.1.32 completed/sealed; six source inventory files plus nine supporting changed paths accounted. One validated audit candidate, ignored as vulnerability; zero promoted exploit findings. |
| Real hardware clocks/suspend/sync/reboot/crash, deployed IPC/enforcement | NOT_RUN | Settable clocks and I/O faults are simulations. Physical tests and later milestones not executed in this B1 component review. |
| Miri/sanitizers/coverage-guided fuzzing | NOT_RUN | Qualification reports NOT_AVAILABLE for pinned Debian stable toolchain; not labelled PASS. |
| Implementation fixes/merge/architecture decisions by reviewer | NOT_RUN intentionally | Publication adds only this review record. Next owner: implementer for R2-01; bounded independent re-review before B2. |

## Changed-defect discrimination

C7 reruns the unchanged sensitivity harness with only the six changed definitions selected and logs every cargo output, including the exact panic. Overall exit 0, two Python tests OK; six mutations fail after compiling and all six unmodified controls pass. It does not cover the new startup finding.

| Definition | Independent intended failure | Assessment |
|---|---|---|
| failed_time_sample_ignored | current eligible versus unknown(registry_unavailable), current_authorization:1914 | PASS, R01 current-decision failure |
| anomaly_episode_not_resumed | after real closure/new episode, (1,true) versus (2,true), trusted_time:719 | PASS for wrong resume; duplicate journal check suppresses the earlier collision, so later distinct-episode assertion discriminates |
| anomaly_transition_not_ledgered | ledger-only closure lost, (1,true) versus (2,true), trusted_time:719 | PASS, transition durability |
| replayed_anomaly_records_ignored | [(boot-1,1),(boot-1,1)] versus one record, trusted_time:700 | PASS, replayed membership |
| anomaly_recorded_at_every_operation (re-pointed) | count 2 versus 1, trusted_time:460 | PASS, once-per-user-per-episode |
| policyd_never_reports_a_forward_sample (replacement) | offset old+0 versus old+5, current_authorization:1777 | PASS, accepted forward sample actually shared |

The author describes +4 and two re-pointed definitions. Precisely, the second is a renamed/replaced definition: the old unreported-offset assignment mutation is no longer reached on refusal, and the replacement detects failure to report. Total 123 is correct. This is not a promise that every possible episode defect is detected.

The trusted-time oracle/vectors are unchanged. The changed Rust differential explicitly zeros the new mark when comparing with the model, which has no episodes. Agreement still tests the original trusted/observed arithmetic, not mark persistence or startup episode semantics. Actual registry integration controls provide that coverage and reveal R2-01.

## Exact scope

```text
core/policyd/src/service.rs
core/policyd/tests/current_authorization.rs
core/registryd/src/anchor.rs
core/registryd/src/clock.rs
core/registryd/src/ledger.rs
core/registryd/src/record.rs
core/registryd/src/registry.rs
core/registryd/tests/trusted_time.rs
core/registryd/tests/trusted_time_model.rs
docs/decisions/0007_m2_lifecycle_time_consumption.md
docs/decisions/README.md
docs/specs/m2_registry_storage_v1.md
docs/specs/m2_service_ipc_v1.md
tests/m2/harness/test_m2_registry_policy.py
tests/m2/test_inventory.txt
```

## Commands and evidence

Independent logs were generated under `/tmp/cham3-b1-r2-evidence` and retained with CI ZIP/JSON, original snippets and qualification receipts under `/home/dev/cham3-m2-b1-r2-review-evidence`; clean qualification clone `/tmp/cham3-m2-b1-r2-independent-qual`; original-control clone `/tmp/cham3-m2-b1-r2-reviewer`. Plugin temporary workspace `T` is `/tmp/codex-security-artifacts-25c402c4b2bbce7100971aad13a60dc496eb48fed92bd2eae49e8ceb4f4a7581`; its `repro` is another clean clone detached at candidate with test-only additions. All tracked production files in both reviewer clones remain unchanged (`git diff --exit-code`); qualification clone and owner clone remain porcelain-clean.

C0 (PASS; no stash needed):

```sh
cd /home/dev/cham3
git status --porcelain
git stash list
git fetch origin codex/m2-cryptographic-contract-core review/m2-006-b1-security main
git checkout codex/m2-cryptographic-contract-core
git reset --hard origin/codex/m2-cryptographic-contract-core
git rev-parse HEAD
git log --oneline -4
git rev-parse 77a3d0e^ 0ec854d^
git diff --name-only b4e4cdc 77a3d0e
git diff --name-only 77a3d0e 0ec854d
git diff --check b4e4cdc 77a3d0e
git diff --check 77a3d0e 0ec854d
git diff --exit-code b4e4cdc 77a3d0e -- Cargo.toml Cargo.lock .cargo build .github PROJECT.md tests/m2/oracle tests/m2/vectors tests/m2/lib core/registryd/src/journal.rs
git show 792f5f0f8836f63f5823e9e98c514bb7b2df93ff:docs/reviews/M2-006-stage-b1-codex-security.md > /tmp/cham3_b1_prior_review.md
sha256sum /tmp/cham3_b1_prior_review.md
docker ps
docker image inspect cham3-m2-independent-r3-final --format '{{.Id}}'
id
uname -a
cat /etc/os-release
ulimit -n
```

C1 (clean clone and qualification; completed result recorded above):

```sh
git clone --no-hardlinks /home/dev/cham3 /tmp/cham3-m2-b1-r2-independent-qual
git -C /tmp/cham3-m2-b1-r2-independent-qual checkout --detach 77a3d0e65072b60f4ce1780a709dd342fdda67f7
docker run --rm --name cham3-m2-b1-r2-independent-qual --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 -v /tmp/cham3-m2-b1-r2-independent-qual:/work -w /work cham3-m2-independent-r3-final bash build/scripts/qualify_m2_rust.sh > /tmp/cham3-b1-r2-evidence/qualification.log 2>&1
git -C /tmp/cham3-m2-b1-r2-independent-qual status --porcelain
```

C2 (PASS exit 0; static.log):

```sh
docker run --rm --name cham3-m2-b1-r2-static --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -v /tmp/cham3-m2-b1-r2-independent-qual:/work -w /work cham3-m2-independent-r3-final bash -c 'bash build/scripts/run_static_checks.sh && sh tests/run_tests.sh static' > /tmp/cham3-b1-r2-evidence/static.log 2>&1
```

C3 (PASS exit 0 for each; storage.log, wrapper.log):

```sh
docker run --rm --name cham3-m2-b1-r2-storage --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home --tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700 -v /tmp/cham3-m2-b1-r2-independent-qual:/work -w /work cham3-m2-independent-r3-final sh tests/m2/storage/check_reserve_full_disk.sh out/m2/storage > /tmp/cham3-b1-r2-evidence/storage.log 2>&1
docker run --rm --name cham3-m2-b1-r2-wrapper --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home --tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700 -e CHAM3_M2_RESERVE_PROBE=/work/out/m2/storage/target/release/cham3_reserve_probe -e CHAM3_M2_WRAPPER_DISK=/full-disk -v /tmp/cham3-m2-b1-r2-independent-qual:/work -w /work cham3-m2-independent-r3-final python3 -I -m unittest discover -s tests/m2/harness -p test_m2_reserve_probe.py -k wrapper -v > /tmp/cham3-b1-r2-evidence/wrapper.log 2>&1
```

C4 (PASS exit 0; differentials.log):

```sh
docker run --rm --name cham3-m2-b1-r2-differentials --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/work/out/m2/differential_target -v /tmp/cham3-m2-b1-r2-independent-qual:/work -w /work cham3-m2-independent-r3-final bash -c 'cargo test --frozen -p cham3_registryd --test trusted_time_model -- --nocapture && cargo test --frozen -p cham3_registryd --test admission_scenarios -- --nocapture' > /tmp/cham3-b1-r2-evidence/differentials.log 2>&1
```

C5 (PASS exit 0; reviewer_controls.log): snippets extracted from the prior review's two Rust blocks, retaining exactly one trailing newline, and composed as its C6 (candidate current_authorization.rs/trusted_time.rs + newline + respective snippet, saved as reviewer_b1.rs). Digests match both the prior record and author's copied sources:

- policy: `c33ade322b722bee4494f532fd2e7ae6b7cff00d7741760f32412f6af983b25b`.
- registry: `014db1de90301e29a0c03e19133929705598164f4a71a9496cdaf5109b1cba47`.

```sh
docker run --rm --name cham3-m2-b1-r2-controls --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/work/out/reviewer-target -v /tmp/cham3-m2-b1-r2-reviewer:/work -w /work cham3-m2-independent-r3-final bash -c 'cargo test --frozen -p cham3_policyd --test reviewer_b1 reviewer_ -- --nocapture --test-threads=1 && cargo test --frozen -p cham3_registryd --test reviewer_b1 reviewer_ -- --nocapture --test-threads=1 && cargo test --frozen --release -p cham3_policyd --test reviewer_b1 reviewer_ -- --nocapture --test-threads=1 && cargo test --frozen --release -p cham3_registryd --test reviewer_b1 reviewer_ -- --nocapture --test-threads=1' > /tmp/cham3-b1-r2-evidence/reviewer_controls.log 2>&1
```

C6 (actual-source extended controls): compose candidate fixture source + original snippet + the respective supplemental snippet below as rereview_r2.rs, adding startup snippet to registry's file. `T` is the exact temporary workspace stated above; the actual calls used its literal absolute path.

```sh
test_root=/tmp/codex-security-artifacts-25c402c4b2bbce7100971aad13a60dc496eb48fed92bd2eae49e8ceb4f4a7581
docker run --rm --name cham3-m2-b1-r2-policy-matrix --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/work/out/r2-target -v "$test_root/repro:/work" -w /work cham3-m2-independent-r3-final bash -c 'cargo test --frozen -p cham3_policyd --test rereview_r2 rereview_ -- --nocapture --test-threads=1 && cargo test --frozen --release -p cham3_policyd --test rereview_r2 rereview_ -- --nocapture --test-threads=1' > "$test_root/artifacts/policy_matrix_final.log" 2>&1
docker run --rm --name cham3-m2-b1-r2-startup-matrix --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -e CARGO_TARGET_DIR=/work/out/r2-target -v "$test_root/repro:/work" -w /work cham3-m2-independent-r3-final bash -c 'cargo test --frozen -p cham3_registryd --test rereview_r2 rereview_ -- --nocapture --test-threads=1; d=$?; cargo test --frozen --release -p cham3_registryd --test rereview_r2 rereview_ -- --nocapture --test-threads=1; r=$?; test "$d$r" = "0000"' > "$test_root/artifacts/startup_matrix.log" 2>&1
```

Policy command PASS exit 0, eight combinations per profile. Registry command FAIL exit 1: each cargo run exits 101, three tests pass and startup matrix fails; that matrix contains two positive cases and two failing cases per profile. Earlier reviewer-only fixture construction used duplicate synthetic grant ids and correctly failed schema admission before reaching the test; corrected with distinct file/network labels. Those fixture failures are not candidate findings. Initial C7 staging omitted stdin and then the Python module search path; neither attempt counts as verification. The final exact C7 below passed.

C7 (PASS exit 0; changed_defect_reasons.log):

```sh
docker run -i --rm --name cham3-m2-b1-r2-defect-reasons --user 1001:1001 --ulimit nofile=1024:1024 -e HOME=/tmp/cham3_home -v /tmp/cham3-m2-b1-r2-independent-qual:/work -w /work cham3-m2-independent-r3-final python3 -I - <<'PY' > /tmp/cham3-b1-r2-evidence/changed_defect_reasons.log 2>&1
import importlib.util, unittest,sys
sys.path.insert(0,'/work/tests/m2/harness')
import test_m2_registry_policy as m
names = {'failed_time_sample_ignored', 'anomaly_episode_not_resumed', 'anomaly_transition_not_ledgered', 'replayed_anomaly_records_ignored', 'anomaly_recorded_at_every_operation', 'policyd_never_reports_a_forward_sample'}
m.SeededDefects = [d for d in m.SeededDefects if d[0] in names]
original_run = m.run
def logged_run(*args, **kwargs):
    status, output = original_run(*args, **kwargs)
    print('COMMAND', args[0], 'EXIT', status, flush=True)
    print(output, flush=True)
    return status, output
m.run = logged_run
print('CHANGED_DEFECTS', len(m.SeededDefects), flush=True)
result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromModule(m))
raise SystemExit(not result.wasSuccessful())
PY
```

`SeededDefects` retains its externally required upstream spelling at that Python module boundary; new review code otherwise follows repository naming.

C8 (PASS exit 0 for requests/download/unzip/digest comparison): for each run 38632964069 (m2), 38632964091 (m1), 38632964078 (m0), execute `gh api repos/Kayyo321/cham3/actions/runs/<id>` and `gh api repos/Kayyo321/cham3/actions/runs/<id>/jobs --paginate`, retaining JSON. Every run's head_sha equals candidate; every returned job and step is success. Artifact metadata/download:

```sh
gh api repos/Kayyo321/cham3/actions/artifacts/11928017840 > /tmp/cham3-b1-r2-evidence/ci_artifact_meta.json
gh api repos/Kayyo321/cham3/actions/artifacts/11928017840/zip > /tmp/cham3-b1-r2-evidence/ci_artifact.zip
sha256sum /tmp/cham3-b1-r2-evidence/ci_artifact.zip
unzip -q /tmp/cham3-b1-r2-evidence/ci_artifact.zip -d /tmp/cham3-b1-r2-evidence/ci_artifact
```

Digest `18b673470e4e73478a2fe314ed627c7324bb14d092d44bd5bff9f24a08751f40`. [m2 run](https://github.com/Kayyo321/cham3/actions/runs/38632964069), [m1 run](https://github.com/Kayyo321/cham3/actions/runs/38632964091), [m0 run](https://github.com/Kayyo321/cham3/actions/runs/38632964078). M0/M1 results are CI evidence, not local reruns of unchanged distribution suites.

C9 (PASS execution): Codex Security `security-diff-scan`, `codex-security` 0.1.32, scan `95919205-23ac-4c74-b670-bfed7c7c7614`, completed/sealed 2026-10-14T21:19:06.304505Z. Dedicated preflight worker ran:

```sh
python3 /home/dev/.codex/plugins/cache/openai-curated-remote/codex-security/0.1.32/scripts/config_preflight.py --profile security_diff_scan --cwd /home/dev/cham3 --runtime-check delegation_available=true --runtime-check goal_tools_available=true > /tmp/cham3_b1_r2_preflight.json
```

Exit 0, ready, three checks PASS, no config edit. Skill-required fresh-context architecture worker and subsequent independent source validation used immutable Git source, no tests or implementation edits. Parent reviewed all changed code and supporting tests/specifications, ran dynamic controls, recorded candidate `candidate-3ac4a17fa969463a`, validated it, then applied attack-path policy `ignore` for an audit correctness defect without demonstrated lower-privileged security gain. All fifteen changed files accounted; zero promoted exploit findings is not B1 approval. Completion called once and canonical completed scan read back. Measured token usage was not returned in the completed-scan payload; no estimate claimed. Daybreak advisory not_granted; scan still completed, no access-policy change.

Managed persistent scan directory:
`/home/dev/.codex/state/plugins/codex-security/scans/cham3/77a3d0e65072b60f4ce1780a709dd342fdda67f7_20261014T210933Z_1lw3vnj0`.
It contains canonical model/coverage/report and retained actual-source tests/logs under `artifacts/02_discovery/validation_artifacts/r2/`.

## Owner questions and handoff

Retained independent log digests (SHA-256; local evidence directory above):

| File | SHA-256 |
|---|---|
| qualification.log | 845e24362d6d971021049e5c986ceaed8b86b827606eedfc0d4394b229005782 |
| reviewer_controls.log | 06d3f8a28371d1c16ab0e96b0fc1edcf55a2a51da2b9d8f2a1b27f4b038cf994 |
| changed_defect_reasons.log | f64238262f240fd471296cd1feef7e6b69e0c00964c4b5fb020076ffb07c0210 |
| static.log | 91fc60aa442f390e184c428cfc24b0aadf2a2fa2c7694f75acd1a469f66eafdf |
| storage.log | ebe5a831ff8e80e1b91382a228fd11a71f392689442e74a9083645e92b1dcaaa |
| wrapper.log | 64309e3c2406520ce955e36d562d25ae9b46b478ac248498dce26dd0bfa05063 |
| differentials.log | 988018ac045a72109a39a3fc89d564f52fd0a8fcc580ae3967a9f474174aac46 |

Sealed plugin evidence under `artifacts/02_discovery/validation_artifacts/r2/`:

| File | SHA-256 |
|---|---|
| r2_startup_control.rs | bbb3b9ad90bcd2252a85d8b74a28a06a22bcfab2ad325424bb46c33be9441287 |
| startup_matrix.log | 45d88298fcc15a931a4490b1e716fb1eaba2f2ec69b5c0f373fcf693aed55230 |
| r2_registry_controls.rs | de0eb57e9905cef44105d95f938c74ec3f91ecb6333cd59b59b36d24e711c043 |
| r2_policy_controls.rs | 8fe2672661c33aabb8aed713dd59e31b91f2e03dbcfa0034aebac6227df05e1d |
| policy_matrix_final.log | fab00920bf78eaab4837232c401a5465a451ff094c36db2dba8a5e8dc1674dd2 |

These are reviewer-executed evidence, compared with the author logs from the requested docs-only checkout; the author's PASS claims do not close the newly reproduced startup gap.

Owner questions: **none**. E4/E6 clarifications are recorded as decided; this reviewer changes neither them nor architecture. Accepted ADR §5.7 time limits and E5 compound-failure residual remain unchanged.

Next owner/action: implementer resolves R2-01 within R02 episode semantics, adds the paired startup controls and repeats affected gates. Independent bounded re-review is required before B1 approval/B2 work. Passing existing qualification/harness cannot substitute for the missing startup closure. Review publication adds only this record; candidate and owner clone remain clean and unchanged.

## Supplemental reviewer test sources

The following sources are test-only additions to isolated candidate clones, never implementation edits. Original reviewer snippets remain as C5 specifies.

### r2_policy_controls.rs

~~~rust

#[test]
fn rereview_refused_samples_after_fresh_and_successful_evaluations() {
    for earlier_success in [false, true] {
        for malformed in [false, true] {
            for network in [false, true] {
                let label = format!("r2_refusal_{earlier_success}_{malformed}_{network}");
                let armed = Arc::new(AtomicBool::new(false));
                let mut world = world_at_with(&temp_root(&label), registry_config_t::default(),
                    mandatory_policy_t::default(), Box::new(reviewer_ledger_fault_io_t { armed: Arc::clone(&armed) }));
                let alice = test_user(1001); world.enroll(&alice); let editor = app("editor");
                let mut grants = vec![
                    file_grant(&format!("{label}_file"), alice.uid, "home:R2", &["read"], persistent_lifetime()),
                    network_grant(&format!("{label}_network"), alice.uid, "direct", persistent_lifetime())
                ];
                grants.sort_by_key(|grant| grant.grant_id);
                let genesis = draft(&alice, &editor, commit_type_t::genesis, &label).add(grants).sign(&alice);
                world.submit(&alice, &genesis);
                let base = world.time.current().unwrap();
                let source = test_time_t::at(base.realtime + 5);
                source.set_available(true, base.realtime + 5, base.boottime);
                let mut policy = policy_with_clock(&world, &source, &kernel_boot_id());
                let identity = runtime(&alice, &editor);
                let request = files(alice.uid, "home:R2", "read");
                if earlier_success {
                    assert!(is_eligible(policy.evaluate(Launcher, &identity, &request).unwrap()));
                    assert!(matches!(policy.network_mode(Launcher, &identity).unwrap(), network_decision_t::mode { .. }));
                }
                let before = policy.clock().offset();
                let trusted_before = world.registry.rollback_status(Policyd).unwrap().clock.trusted_now;
                source.set_available(true, base.realtime + 500, base.boottime + if malformed {100} else {0});
                armed.store(!malformed, Ordering::SeqCst);
                let reason = if malformed { unknown_reason_t::clock_unavailable } else { unknown_reason_t::registry_unavailable };
                if network {
                    assert_eq!(policy.network_mode(Launcher, &identity).unwrap(), network_decision_t::offline_unknown(reason));
                } else {
                    assert_eq!(policy.evaluate(Launcher, &identity, &request).unwrap(), decision_t::unknown(reason));
                }
                assert_eq!(policy.clock().offset(), before, "no refused value adopted");
                assert_eq!(world.registry.rollback_status(Policyd).unwrap().clock.trusted_now, trusted_before);
                println!("PASS fresh={} malformed={} network={} offset={before:?}", !earlier_success, malformed, network);
            }
        }
    }
}
~~~


### r2_registry_controls.rs

~~~rust

#[test]
fn rereview_ledger_only_closure_and_equal_payload_episodes() {
    let root = temp_root("r2_ledger_closure"); let time = test_time_t::at(T1Seconds);
    let alice = test_user(1001); let bob = test_user(1002);
    let registry = start(&root, "boot-1", &time); enroll(&registry, &alice); enroll(&registry, &bob);
    let look = |r: &registry_t, u: &test_user_t| { r.lineage_head(Policyd, u.uid, &NoLineage).unwrap(); };
    time.step_wall_clock(T1Seconds - 400); look(&registry, &alice); look(&registry, &bob);
    let anchor_before = newest_anchor(&root).time_floor.unwrap();
    assert_eq!(anchor_before.anomaly, 2);
    time.step_wall_clock(T1Seconds); look(&registry, &alice);
    assert_eq!(newest_anchor(&root).time_floor.unwrap(), anchor_before, "closure only ledger");
    assert_eq!(ledger_floors(&root).last().unwrap().anomaly, 3);
    drop(registry);
    let registry = start(&root, "boot-1", &time);
    assert_eq!(registry.run_decision(), run_decision_t::ledger_replayed);
    assert_eq!(registry.rollback_status(Policyd).unwrap().clock.anomaly.mark(), 3);
    time.step_wall_clock(T1Seconds - 400); look(&registry, &alice); look(&registry, &bob);
    for user in [&alice, &bob] {
        assert_eq!(anomaly_episodes(&root, user.uid), vec![("boot-1".into(), 1), ("boot-1".into(), 2)]);
        assert_eq!(anomalies(&root, user.uid), vec![(T1Seconds, 400), (T1Seconds, 400)]);
    }
    registry.shutdown().unwrap(); drop(registry);
    let registry = start(&root, "boot-1", &time); look(&registry, &alice); look(&registry, &bob);
    assert_eq!(anomaly_episodes(&root, bob.uid).len(), 2);
}
#[test]
fn rereview_episode_contained_and_invalid_anchor_start() {
    for invalid_anchor in [false, true] {
        let root = temp_root(&format!("r2_contained_{invalid_anchor}"));
        let time = test_time_t::at(T1Seconds); let alice = test_user(1001);
        let registry = start(&root, "boot-1", &time); enroll(&registry, &alice);
        time.step_wall_clock(T1Seconds - 400);
        registry.lineage_head(Policyd, alice.uid, &NoLineage).unwrap();
        let before = fs::read(journal_path(&root, alice.uid)).unwrap();
        drop(registry);
        if invalid_anchor {
            for slot in SlotFileNames { fs::write(root.join("anchor").join(slot), b"invalid").unwrap(); }
        } else { fs::remove_file(root.join("ledger/validity")).unwrap(); }
        let registry = start(&root, "boot-1", &time);
        let status = registry.rollback_status(Policyd).unwrap();
        if invalid_anchor {
            assert!(!status.quarantined.is_empty());
            assert_eq!(fs::read(journal_path(&root, alice.uid)).unwrap(), before);
            assert!(registry.lineage_head(Policyd, alice.uid, &NoLineage).unwrap().unwrap().origins.is_empty());
        } else {
            assert_eq!(registry.run_decision(), run_decision_t::contained);
            assert_eq!(status.clock.anomaly.mark(), 2);
            assert_eq!(status.contained, vec![alice.uid]);
            registry.lineage_head(Policyd, alice.uid, &NoLineage).unwrap();
            assert_eq!(anomaly_episodes(&root, alice.uid), vec![("boot-1".into(), 1)]);
        }
    }
}
#[test]
fn rereview_mark_encoding_merge_and_record_domain() {
    use cham3_registryd::clock::{anomaly_state_t, merge_floor};
    for episode in 1..100 {
        let open = anomaly_state_t { episode, open: true, behind: 400 };
        let closed = anomaly_state_t { open: false, ..open };
        assert_eq!(open.mark(), episode * 2);
        assert_eq!(closed.mark(), episode * 2 + 1);
        assert_eq!(anomaly_state_t::from_mark(open.mark()).episode, episode);
        assert!(anomaly_state_t::from_mark(open.mark()).open);
        assert!(!anomaly_state_t::from_mark(closed.mark()).open);
    }
    let old = time_floor_t { boot_id: "boot-1".into(), boottime: 10, trusted: T1Seconds, observed: 20, anomaly: 7 };
    let next = time_floor_t { boottime: 11, trusted: T1Seconds + 1, observed: 21, anomaly: 2, ..old.clone() };
    assert_eq!(merge_floor(Some(old.clone()), next.clone()).anomaly, 7);
    assert_eq!(merge_floor(Some(next), old).anomaly, 7);
    for episode in [0, 1, 99] {
        let record = registry_record_t { sequence: 1, time: utc_text(T1Seconds as i64).unwrap(),
            body: record_body_t::event(registry_event_t::clock_anomaly { boot_id: "boot-1".into(), episode, trusted: T1Seconds, behind_seconds: 400 }) };
        let decoded = registry_record_t::decode(&record.encode());
        if episode == 0 { assert!(decoded.is_err()); } else { assert_eq!(decoded.unwrap(), record); }
    }
}
~~~


### r2_startup_control.rs

~~~rust

#[test]
fn rereview_startup_sample_closes_the_resumed_episode() {
    let mut bad = false;
    for clean in [true, false] {
        for startup_in_range in [false, true] {
            let root = temp_root(&format!("r2_startup_closure_{clean}_{startup_in_range}"));
            let time = test_time_t::at(T1Seconds); let alice = test_user(1001);
            let registry = start(&root, "boot-1", &time); enroll(&registry, &alice);
            time.step_wall_clock(T1Seconds - 400);
            registry.lineage_head(Policyd, alice.uid, &NoLineage).unwrap();
            assert_eq!(anomaly_episodes(&root, alice.uid), vec![("boot-1".into(), 1)]);
            if clean { registry.shutdown().unwrap(); }
            drop(registry);
            // Only an actual in-range sample ends the episode, never restart alone.
            if startup_in_range { time.step_wall_clock(T1Seconds); }
            let registry = start(&root, "boot-1", &time);
            let startup_mark = newest_anchor(&root).time_floor.as_ref().unwrap().anomaly;
            time.step_wall_clock(T1Seconds - 400);
            registry.lineage_head(Policyd, alice.uid, &NoLineage).unwrap();
            let episodes = anomaly_episodes(&root, alice.uid);
            let expected = if startup_in_range {
                vec![("boot-1".into(), 1), ("boot-1".into(), 2)]
            } else { vec![("boot-1".into(), 1)] };
            println!("clean={clean} startup_in_range={startup_in_range} startup_mark={startup_mark}; episodes={episodes:?}; expected={expected:?}");
            bad |= episodes != expected || startup_mark != if startup_in_range {3} else {2};
        }
    }
    assert!(!bad, "startup's actual in-range sample must close the episode; anomalous startup must resume it");
}
~~~
