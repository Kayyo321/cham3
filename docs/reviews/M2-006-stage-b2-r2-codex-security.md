# M2-006 stage B2 revision 2 independent Codex review

**Verdict: CHANGES_REQUESTED.** B2-02, B2-03 and B2-04 are repaired. B2-01 is repaired for lock contention and an earlier pending-write flush, but the new decision observation can itself perform blocking persistence after sampling. It also introduces a path past the failed-ledger gate. Both remaining defects reproduce in admission and reservation, in debug and release.

Reviewer: Codex, independent of the implementation author; REVIEWER role, 2026-10-15 America/New_York. No implementation changes, merge or owner decisions. M2-006 B2 returns to its implementer; stage C remains gated on B2 approval. Only this record is published, based on `origin/main` at `9cf882503848f8c6cc4938a588cc9fc062a9392d`.

## Immutable scope and startup

- Baseline: `ac28d240d880c1e6b31cd6d3e772c74f2b37af01`.
- Candidate: `671a7d86262a6f77a391dd8fcbc99f9888be671a`, one commit above baseline; tree `36c18e372d2e87a97666ff91c28eb14c11fb430f`.
- Docs follow-up: `14c3e4c6332dcfb9a0013419b651d2df6b17fb42`, whose parent is exactly the candidate. All 51 changed paths are under `docs/`, including evidence source files. No implementation path changes in this commit.
- Previous independent record: `review/m2-006-b2-security` at `3ca35811776672867440cca632705b67ef920ad6`; read without modifying it.
- Review branch/worktree: `review/m2-006-b2-r2-security`, `/home/dev/cham3-m2006b2r2-codex/review`.

PASS — startup commands:

```sh
cd /home/dev/cham3
git status --porcelain
git stash list
git fetch origin codex/m2-cryptographic-contract-core review/m2-006-b2-security main
git checkout codex/m2-cryptographic-contract-core
git reset --hard origin/codex/m2-cryptographic-contract-core
git rev-parse HEAD
git log --oneline -4
```

The clone and stash list were empty before the requested reset. HEAD after it was `14c3e4c6332dcfb9a0013419b651d2df6b17fb42`; no changes were moved aside. This implementation checkout remains clean at that commit. No force-push or implementation edit occurred.

PASS — `git diff --name-only ac28d24 671a7d8` contains exactly these nine files:

```text
core/policyd/tests/current_authorization.rs
core/registryd/src/lineage.rs
core/registryd/src/record.rs
core/registryd/src/registry.rs
core/registryd/tests/lifecycle_consumption.rs
docs/specs/m2_registry_storage_v1.md
docs/specs/m2_service_ipc_v1.md
tests/m2/harness/test_m2_registry_policy.py
tests/m2/test_inventory.txt
```

PASS — `git diff --check ac28d24 671a7d8`; `git rev-list --count ac28d24..671a7d8` returns 1; `git rev-parse 14c3e4c^` returns the candidate. Explicit `git diff --exit-code ac28d24 671a7d8 -- core/policyd/src Cargo.toml Cargo.lock .cargo build/scripts build/m2 .github tests/m2/oracle tests/m2/vectors docs/decisions core/registryd/src/anchor.rs core/registryd/src/ledger.rs core/registryd/src/journal.rs core/registryd/src/clock.rs` returns 0. The complete nine-path inventory also establishes that nothing else changed.

Read from the synchronized checkout: AGENTS, relevant PROJECT requirements, WORKFLOW, ORCHESTRATION, ROLES, TESTING, SECURITY_INVARIANTS, STATUS, ADR 0007 (particularly §§5, 6, 8 and E7), storage specification §§2, 8, 9 and supporting recovery/limits, changed service IPC rows, author finding resolutions, handoff and evidence. Author placements were claims to verify. No applicable SECURITY.md was returned by the plugin resolver.

## Findings requiring revision

### M2-006-B2-R2-01 — decision time must follow its own blocking persistence

**Low security severity; B2-01 remains open.** Root control: `core/registryd/src/registry.rs:1581–1586`. Callers: admission at `2261`, reservation at `2433`; supporting `observe_time` at `1599–1658`, `record_anomaly` at `1691–1731`, lifetime check at `2470`.

The second sample is after the lock and `ready`, but `observe_time` can write a time entry or refreshed anchor floor, and `record_anomaly` can synchronously persist an anomaly. `fresh_now` then reads the cached clock. It does not account for time spent in those prerequisite writes. Moving the old sample past `ready` therefore leaves the same class of stale decision inside the replacement helper.

FAIL, debug and release — `reviewer_r2_anomaly_write_delay_must_not_stale_reservation`: first sample is settled at T1+60. Only the second sample sees realtime stepped back 600 seconds, opening an anomaly while trusted time stays monotonic. The anomaly journal write then advances both fixture clocks 600 seconds. An existing one-shot expires at T1+300. At the lifetime decision, elapsed trusted time should be T1+660, but `reserve_use` uses cached T1+60 and returns `Ok(reservation)`. The delay belongs to prerequisite anomaly evidence, before consumption, not to persistence of the reservation itself.

FAIL, debug and release — `reviewer_r2_anomaly_delay_must_not_stale_admission`: the corresponding new commit stamped T1 is admitted `Ok(trunk)` after the same delay instead of `admission/timestamp_rule`. Admission's signing-window check also uses this same `now`; its ordinary post-flush coverage passes, but this shared stale-time path still affects that check by source tracing.

Required: obtain decision time after potentially blocking prerequisite observation/anomaly persistence, or conservatively retry/refuse when that work ages the sample, before validating and consuming/admitting. Preserve offset ledgering, floors, anomaly episodes and option B. Add coverage for persistence triggered by the second sample, including floor/ledger paths, not just writes already owed before it.

This is a component temporal-authorization defect. The request began before expiry; this is not a proof of formal non-linearizability, actual resource access, or an application's ability to manipulate the host clock. Environmental clock anomalies and write latency constrain severity. PASS control: an anomaly that remains owed refuses without consumption, then recovery permits the valid reservation.

### M2-006-B2-R2-02 — recheck failed-ledger state after the decision observation

**Low security/control-regression severity; required repair.** Root control: `core/registryd/src/registry.rs:1581–1586`. Existing `ready` gate at `1532–1545`; callers at `2261` and `2433`; persistence sinks at `2304` and `2486`.

`ready` checks that the ledger has not failed. The new second observation runs afterwards. If that observation tries to ledger an offset increase and the append fails, `observe_time` marks the ledger/run failed and refuses to adopt the offset increase, but still returns `fresh: true`. `decision_now` checks only pending records before returning trusted time. Neither caller repeats the failed-ledger gate before its new authorization decision.

FAIL, debug and release — `reviewer_r2_second_sample_ledger_failure_refuses_before_consumption`: first sample and `ready` see healthy storage; only the second sample steps realtime ahead one second and arms ledger-only append failure. `reserve_use` still returns `Ok` and the journal contains one new `use_reserved`.

FAIL, debug and release — `reviewer_r2_second_sample_ledger_failure_refuses_admission`: the same second-sample failure permits `Ok(trunk)` for a fresh revision.

Required: propagate decision-observation persistence failure or re-establish failed-ledger readiness after that observation and before eligibility/admission/consumption. Add separate reservation and admission controls that fail only the second sample's time-entry append. Preserve restrictive writes and recovery behavior.

This is distinct from the previously observed/accepted behavior where the first ledger failure occurs during persistence of an **already consumed** reservation and its journal/anchor still succeed. Here the time-entry failure occurs before new authorization or consumption, after their existing gate. Journal and anchor writes still succeed; subsequent calls deny the failed ledger. No lost restriction, repeated one-shot use or deployed resource misuse is claimed.

## Acceptance and author placements

| Area | Result | Evidence and limits |
|---|---|---|
| Original reviewer controls, unchanged | PASS | SHA-256 exact; candidate registryd 12/12 and policyd 1/1 in debug and release. Baseline rerun reproduces 10/12 and 0/1. |
| B2-01 original lock/ready-flush cases | PASS | Supplied contention, slow flush, second-sample unavailability and original reviewer delayed-write test pass. |
| B2-01 complete decision-time requirement | FAIL | R2-01; observation/anomaly persistence can itself age the sample. |
| New observation fail-closed behavior | FAIL | R2-02; second-sample time-entry failure occurs after the ledger gate. |
| Option B, floors and anomalies | PASS for unchanged rules | Clock source unchanged; observed counter read after second sample; model/vectors/differentials pass. Owed second anomaly refuses before consumption and recovers. No hardware-clock claim. |
| B2-02 public restrict and restrictive writer | PASS | `well_formed` before any public effect and again before queue/ledger; malformed bodies never denied/ledgered/persisted. |
| Four malformed packages, 255-byte positive, restart | PASS | Registry and policy component tests; original reviewer target; decoder remains strict. Policyd source unchanged; refused mismatch report leaves `unknown_identity` denial. |
| Unsorted/repeated/65 restriction IDs | PASS | Supplied three cases and 64-ID positive/restart; author restriction probe passes debug/release. No other current writer path producing an undecodable persisted body was identified. |
| B2-03 all known sessions at a new boot | PASS | Started/unbound and historical grant-bound sessions, no-live-branch and live-branch users; clean and contained startup. One record per usable user with live known sessions, after branch termination/containment and before uncertain reservations. |
| Sessions pending/retry/re-journal/finality | PASS | Immediate in-memory end; operation retry and same-boot ledger replay journal exactly once; repeated starts/new bound admissions refuse; end returns boot record sequence. Same-boot restart does not end new sessions; ordinary trunk supersession still ends branches rather than all sessions. |
| Forged record/quarantined journal | PASS | Forged no-live-session `sessions_ended` quarantines. Supplemental new-boot test with live session plus replay defect preserves journal bytes exactly. |
| B2-04 stale-head evidence | PASS | Two focused tests; own mutants stop at named superseded trunk/ended branch assertions; live-head positives and unknown-ID error contract retained last. |
| Harness and inventory | PASS | 264 Cargo/149 Python inventory; full harness detects all 172 defects. Independent reason rerun for 13 new and 3 re-pointed defects. |
| Qualification, static, wrapper, protection, CI | PASS | Commands and counts below. These supplied gates do not cover R2-01/R2-02. |

Five author placements: (1) second-sample anomaly recording is appropriate but incomplete ordering/readiness is R2-01/R2-02; (2) full strict round trip is acceptable for the bounded body check; (3) `malformed_request` has the documented before-effects behavior; (4) one `sessions_ended` per user has the stated bounded reserve use and terminal/replay behavior; (5) unchanged policyd retains denial when reporting a malformed identity fails. No owner decision is needed for the two demonstrated repairs.

The round trip checks the body with a synthetic valid record header. Actual writers supply a valid UTC time/sequence; journal framing bounds actual record bytes. Ordinary identity/admission writers verify their structured inputs; ordinary evidence uses closed enums/fixed-width IDs and a boot ID bounded by ledger startup. This inspection found no remaining same-class body-poisoning path; it is not a claim of exhaustive arbitrary-input fuzzing.

## Environment, exact commands and evidence

Host: Debian GNU/Linux 13.7 amd64, kernel `7.1.13+deb13-amd64`, UID/GID 1001. Docker **yes**, direct `docker version` succeeded as dev in docker group, no sudo. Server `26.1.5+dfsg1`. Image `cham3-m2-dev`, `sha256:2a0bcf342c57a9352326376771229a7e78e291095c561728df38636e577630a9`. All test containers use `--user 1001:1001 --ulimit nofile=1024:1024`; host nofile 524288 unchanged. No privileged container or host-protection change.

Evidence root `E=/home/dev/cham3-m2006b2r2-codex`; clean qualification clone `Q=$E/qual`; reproduction clone `R=$E/repro`. Both were created with `git clone --no-hardlinks /home/dev/cham3 <path>` and `git checkout --detach 671a7d86262a6f77a391dd8fcbc99f9888be671a`. Q remains clean before/after. R only adds untracked integration targets; implementation bytes are unchanged. The old reproduction clone `B=/home/dev/cham3-m2006b2-codex/repro` is still at `248ca6ef90d22e270db0e9a3f5bf789502d18720` with its two untracked original reviewer targets, reused without edits.

PASS — qualification, exit 0 (`E/qualification.log`):

```sh
docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 \
  -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
  -v "$Q:/work" -w /work cham3-m2-dev \
  bash build/scripts/qualify_m2_rust.sh --record
```

Evidence: `Q/out/m2/qualification/amd64_20261015T135931Z_1/`; unit `Q/out/m2/runs/unit_20261015T140002Z_1865/`; harness `Q/out/m2/runs/harness_20261015T140148Z_4713/`.

- PASS: debug/release **264+264**, exact inventory; Python **149**, 544.110 s, all **172** seeded defects detected and unmodified controls pass.
- PASS: ECR **19 valid / 96 invalid / 18 admission vectors**, **27 scenarios / 365 steps**; trusted-time **165 cases**; format **5968 / 0 mismatches**, corpus `44ea2aa43135df93b3639c1c64810e4c16b14506297b314b89ba3e4cd8cf426c`; provider oracle and vault oracle (38).
- PASS: Debian/toolchain/pins, installed/downloaded package locks, offline locked Cargo metadata, source/dependency inventory, rustfmt, clippy, naming, debug/release builds, profile/backend checks, Valgrind probe and secure-memory tests (10, zero errors).
- PASS: identity candidate/tree exact, `dirty_tracked 0`, actual `git status --porcelain` empty. Cargo.lock SHA-256 `9801453aa7023cffc1e3f5b1ec327c284d365c2ce4b9cdd4dc41efddee384c57` unchanged.
- NOT_RUN: Miri, sanitizers and coverage-guided fuzzing, unavailable on the pinned stable/package environment as qualification reports.

PASS — static checks and 82 static tests, `E/static.log`, using the same Docker user/nofile/HOME/mount options:

```sh
bash build/scripts/run_static_checks.sh
sh tests/run_tests.sh static
```

PASS — full-filesystem wrapper, both profiles, `E/storage.log`, actual disposable 2 MiB tmpfs:

```sh
docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 \
  --tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700 \
  -e HOME=/tmp/cham3_home -v "$Q:/work" -w /work cham3-m2-dev \
  sh tests/m2/storage/check_reserve_full_disk.sh out/m2/storage
```

PASS — 5/5 wrapper preservation/sensitivity cases, `E/wrapper.log`, same container/tmpfs options plus:

```sh
-e CHAM3_M2_RESERVE_PROBE=/work/out/m2/storage/target/release/cham3_reserve_probe
-e CHAM3_M2_WRAPPER_DISK=/full-disk
python3 -I -m unittest discover -s tests/m2/harness -p test_m2_reserve_probe.py -k wrapper -v
```

PASS — vault protection positive path, `E/protection.log`, same standard container options plus `--ulimit memlock=1073741824:1073741824`, command `sh tests/m2/protection/check_vault_protection.sh out/m2/protection`: `protection_verified`, exit 0.

PASS — explicit differentials, `E/differentials.log`, standard container options and `CARGO_TARGET_DIR=/work/out/m2/runs/unit_20261015T140002Z_1865/target_debug`:

```sh
cargo test --frozen -p cham3_registryd --test admission_scenarios -- --nocapture
cargo test --frozen -p cham3_registryd --test trusted_time_model -- --nocapture
```

Results: admission **15 scenarios / 72 steps / 121 comparisons / 0 disagreements**; trusted-time **165 cases / 4813 steps / 0 disagreements**.

PASS — original reviewer targets copied unchanged from B to R. Digests match the previous record and author's SHA256SUMS:

```text
reviewer_b2.rs        24d4b2adfaa77fe1755259616746c81dd46412a2caa6526fc701fe34e47448f6
reviewer_policy_b2.rs c1dbabbe6167b010835159f7d50f8b2b6229aae401c432dc0e78ce5f56b53d72
```

Standard container options with R mounted as `/work`, `CARGO_TARGET_DIR=/work/out/reviewer_target`:

```sh
cargo test --frozen -p cham3_registryd --test reviewer_b2 reviewer_ -- --nocapture
cargo test --frozen -p cham3_policyd --test reviewer_policy_b2 reviewer_ -- --nocapture
cargo test --frozen -p cham3_registryd --test author_restriction_probe -- --nocapture
# Each repeated with --release before --frozen.
```

`E/reviewer_controls.log`: registryd **12/12**, policyd **1/1**, author restriction probe **1/1**, debug and release. `E/reviewer_baseline.log`: original reviewer commands with B mounted instead reproduce registryd **10/12** (B2-01/B2-03) and policyd **0/1**, both profiles, matching author logs. The baseline negative results are expected sensitivity PASS, not a candidate failure. Author probe cases are descending and duplicate; the supplied lifecycle test additionally covers 65 IDs.

FAIL candidate supplemental regressions; PASS two controls — same R container/target directory:

```sh
cargo test --frozen -p cham3_registryd --test reviewer_r2 reviewer_r2_ -- --nocapture
cargo test --release --frozen -p cham3_registryd --test reviewer_r2 reviewer_r2_ -- --nocapture
```

`E/reviewer_r2_verified.log`: each profile **2 passed / 4 failed**. The four failures are the two findings' admission/reservation cases. The controls are owed second anomaly refusal/recovery and new-boot quarantined-journal byte preservation. Source `R/core/registryd/tests/reviewer_r2.rs` SHA-256 `d83688ac714c8932a0492338b0b4698574fa0f077bed3691307a30de74594724`; log SHA-256 `23b43562d006ca2a69c3444b36fd568d96a389b71e3bac2c2841efc5317882dd`. Both actual files are also retained in the plugin scan below. This target copies the candidate lifecycle helpers/tests and appends six `reviewer_r2_` cases; only those six are selected. Initial supplemental compilation in `E/reviewer_r2.log` failed because the review test supplied `registry_t` instead of its helper's required `Arc<registry_t>`; that test-only setup error was corrected before the reported executions. Intermediate logs retain earlier smaller matrices; the final target/log above is authoritative.

## Independent mutation-reason audit

PASS — executed `docker run --rm -i` with standard Q options and `python3 -` reading a bounded audit program. It imported `SeededDefects`, copied Cargo.toml/Cargo.lock/.cargo/core/vectors into a disposable directory, applied each selected defect alone after asserting its anchor occurs once, ran `cargo test --frozen -p <package> --test <target> <filter> -- --exact --nocapture` with `RUST_BACKTRACE=1` and `CARGO_TARGET_DIR=/work/out/mutant_reason_target`, restored original source after each, and required exit 101 plus the named test's FAILED line. Selection: the 13 new defects plus `admission_on_a_stale_clock`, `reservation_on_a_stale_clock`, `enforcement_of_a_superseded_commit`. Raw logs: `Q/out/mutant_reasons/<name>.log`; compact execution output `E/mutant_reasons.log` ends `PASS independently audited 16 mutants`.

| Defect | Independently observed reason / judgment |
|---|---|
| admission_on_a_stale_clock | PASS: missing-sample admission refusal, shared helper called at trusted_time.rs:590. |
| reservation_on_a_stale_clock | PASS: missing-sample reservation refusal, lifecycle_consumption.rs:433. |
| enforcement_of_a_superseded_commit | PASS: own superseded-trunk assertion at lifecycle_consumption.rs:1978. |
| enforcement_of_an_ended_branch | PASS: own trunk-ended branch assertion at :2035. |
| reservation_decides_on_its_first_sample | PASS: pending-flush expiry assertion at :1432. |
| admission_decides_on_its_first_sample | PASS: pending-flush timestamp assertion at :1544. |
| decision_sample_failure_ignored | PASS: unavailable second sample assertion at :1482. |
| records_written_unchecked | PASS: malformed mismatch refusal assertion at :1616. |
| unnameable_package_identity_journaled | PASS: first malformed evaluation leaves undecodable journal, current_authorization.rs:1995, called from :2241. |
| malformed_restriction_denied_before_the_check | PASS: denied before validation, caught by `nothing denied` at lifecycle_consumption.rs:1632. |
| sessions_survive_a_new_boot | PASS: no-branch user boot-end record at :1736. |
| grant_bound_sessions_survive_a_new_boot | PASS: grant-bound session restart refused at :1755. |
| contained_users_keep_their_sessions | PASS: contained record order/session end at :1828. |
| forged_sessions_ended_replayed | PASS: replay quarantine assertion at :1859. |
| boot_ended_session_end_not_durable | PASS: terminal sequence lookup at :1790. |
| sessions_end_only_once_durable | PASS: immediate in-memory terminal state at :1904. |

These reasons agree with the author's `harness/defect_reasons_b2r2.log`. B2-04's previous unknown-ID masking is repaired. The branch mutant stops at the trunk-ended case; the new-boot branch negative also runs normally. This does not claim the single mutant separately reached both branch-negative assertions.

## CI comparison

PASS — `gh api repos/Kayyo321/cham3/actions/runs/<id>` and `/jobs?per_page=100` for `38665833821` (m2), `38665833839` (m1), `38665833783` (m0): all head SHAs equal candidate, all success; job counts 1/3/3 and step counts 10/39/39; zero steps outside success/skipped. Responses retained as `E/ci_run_<id>.json` and `E/ci_jobs_<id>.json`.

```sh
gh api repos/Kayyo321/cham3/actions/artifacts/11942412776
gh api repos/Kayyo321/cham3/actions/artifacts/11942412776/zip > "$E/ci_artifact.zip"
sha256sum "$E/ci_artifact.zip"
```

PASS — digest exactly `d3679f8e2e2630a1e0c097b9c358bf7ab5c1bcf4ee592758a78953516ecf8965`, matches request and API; not expired, 111813 bytes, 45 extracted files at `E/ci_artifact`. Parsed local/CI/author debug, release and Python JSON agree on result, exact tests/suites where present and totals. Format count/corpus/coverage/mismatches and provider result agree. Both full-filesystem reports are byte-identical across all three sources. Identity evidence names exact candidate with dirty_tracked 0. Timings/paths differ; no byte-identical-build claim.

## Codex Security plugin

PASS — **Codex Security 0.1.32**, exact `ac28d24..671a7d8`, same scan `9811dbb5-89a1-4363-9831-77f71f31043e`, completed/sealed once and read back with `get_codex_security_completed_scan`. Full nine-file review; native production-source inventory has three entries, supplemented explicitly by six test/doc paths. Two Low findings:

- `csf_6a69faff57c88244a238cbd8` — R2-01, CWE-367.
- `csf_1e157eef9c602ccda29c49d7` — R2-02, CWE-754.

Preflight PASS, exit 0 READY; all three capability checks pass. No configuration edits. Exact helper command:

```sh
python3 /home/dev/.codex/plugins/cache/openai-curated-remote/codex-security/0.1.32/scripts/config_preflight.py \
  --profile security_diff_scan --cwd /home/dev/cham3 \
  --runtime-check delegation_available=true --runtime-check goal_tools_available=true \
  --multi-agent-runtime-owner native --multi-agent-runtime-provenance thread-context
```

The plugin-required fresh architecture worker completed read-only. An intermediate delegated scan coordinator stopped with `This content was flagged for possible cybersecurity risk` and Daybreak-access guidance; the parent completed the same scan with its normal exposed tools, without retrying that worker or changing configuration/access controls. Daybreak advisory was not granted and did not gate the exposed scan tools. Two draft submissions initially contained host-owned fields and were rejected by schema validation; corrected typed submissions were accepted before the single successful finalization. No scan replacement or skipped candidate disposition.

Report/evidence root:

```text
/home/dev/.codex/state/plugins/codex-security/scans/cham3/671a7d86262a6f77a391dd8fcbc99f9888be671a_20261015T135915Z__sdi8q78/
```

Generated `report.md`; supplemental actual source/log under `artifacts/02_discovery/validation_artifacts/b2r2/`. Workbench usage at completion: coverage complete, three threads, **9346474 total tokens**, input 9309885, cached input 8959232, output 36589 (reasoning output 5056). These are tool-returned measurements, not cost estimates.

## Handoff and limits

State: **CHANGES_REQUESTED** for M2-006 stage B2 revision 2. Next owner: implementer repairs R2-01/R2-02 without weakening clock, ledger, restrictions or replay; adds discriminating controls; reruns affected checks and qualification; supplies a new immutable candidate for independent review. Preserve the now-passing B2-02/B2-03/B2-04 behavior. Task/STATUS changes belong to the implementation/status owner; this review publishes only its record.

Owner questions: **none**. Existing approved fail-closed and decision-time requirements suffice; no architecture/risk choice is made here.

NOT_RUN: actual sessiond, attributed runtime IDs, launcherd/netd/broker enforcement, deployed authenticated sockets, physical clock manipulation/suspend/reboot, real power/kernel/process crashes, hardware compatibility. Tests use disposable component fixtures; they do not prove full-system enforcement. Existing ADR 0006 E5 and ADR 0007 §5.7/§8 limitations remain unchanged. All requested local gates were executable; none is reported Docker BLOCKED.
