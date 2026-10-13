# M2-005 independent Codex review, with M2-004 revision-2 re-review

**M2-005: CHANGES_REQUESTED.** Three findings require correction: incomplete directory durability, failure to retain ended-session identity, and a logical quota allowance presented as a preallocated restrictive reserve. The supplied regression gates do not exercise these sequences. **M2-004 revision 2: APPROVE for its recorded staged scope**, with the existing integration obligations and plugin blocker retained. No owner architecture, sequencing or risk decision is made here.

Reviewer: Codex, independent REVIEWER, 2026-10-13. Publication branch `review/m2-005-security`, based on `9cf882503848f8c6cc4938a588cc9fc062a9392d`. Only this review record is published. No implementation source was edited or implementation/review branch merged. Test packages and logs live outside the candidate clones.

## Immutable targets and combined-review method

- M2-005 range: `6968bd6f8c5fa2701918686164ea50910a8449b8..b06d7cb21e1e3e42bbcd025abd9cea131574751d` (implementation `6e9d6d3`, correction `b06d7cb`). All 25 changed paths inventoried; every changed production module reviewed, with supporting identity authority, format/intrinsic rules, oracle and test paths followed.
- M2-005 docs: `24a35c1914d787b936d14b37b500226dde51722f`; **PASS**, all 37 changed paths are under `docs/`. Remote implementation branch still named this commit at the review's final target check.
- Prerequisite M2-004: `ca40c62ed7a5fc29ba8910fef514f3ce29ea4fba`, code comparison `bb3f1bad77080da34069af62c8d2cc390f2fcf7e..ca40c62`; immediate base `d2c29ad7a09b2b1d49a23e9fa92c9656aa74c711`. The immediate-base diff contains precisely nine expected source/test/inventory paths. Comparing with bb3f1ba additionally includes the intervening revision-1 documentation, not unexpected code.
- Prerequisite docs: `acefb668976359d2b123b29246f86e717f41930a`; **PASS**, all 43 paths under `docs/`.
- Combined review: first inspected the R01–R05 source corrections and I01 documentation against the prior review, then reviewed the new registry/policy composition. Separate clean UID-1000 clones ran qualification at **each** candidate. The original 17-test package was recovered from review `f2d82b7a663c1579582bc5c88fe2832a6f97e061` and rerun unchanged at ca40c62. Its source and manifest digests match that record exactly. The original review file fetched with Git has SHA-256 `eb0b011ffbab9ae72002df1e264bc9f067c5d2d5d463a4733bed822148db596a`.
- This one combined record supersedes neither implementation history nor the original review; it provides the prerequisite re-review verdict here rather than creating a separate M2-004 publication branch. No unresolved R01–R05 finding is inherited by M2-005. The three findings below concern its new code.

Authority read: AGENTS.md, PROJECT security model, contract/signing/revocation/ECR requirements and service boundaries; workflow, orchestration, roles and security invariants; M2 task A06/A07/A08 and the **pre-implementation** owner sequencing section; ADR 0006 §§1–2 plus prerequisite §§3/5/10, ADR 0007 §§1/6/8, ADR 0005 §2 and signing/recovery rules; wire v1 §§2.3/6.5–6.12/7; IPC v1 §3; both handoffs, prior reviewer record and author resolutions. Author claims were checked against source and new runs.

## Environment and execution limits

Managed cloud machine `c3235e23df6d`, Debian 13.6, Linux 6.18.44 x86_64, outer UID/GID 1000 (`agent`). Docker client/server 28.4.0, vfs storage, runc, seccomp. Inner pinned image Debian 13.7, UID/GID 1000, rustc/cargo 1.85.1; image `cham3-m2-review` = `sha256:493ecb23f8c8a0e73ecb8f88563cdefe7899f33f09a3716e5fd9948cf847f6da`. Base: `debian@sha256:913f6706df59a68922d1dd08f78c2476560a8d367897200a6005b00e5f67c2d5`; exact package, downloaded-deb and crate-source locks checked by qualification.

Clean clones: `/workspace/m2-004-clean` and `/workspace/m2-005-clean`, owned by UID 1000, detached at their respective candidates. Source-inspection worktree `/workspace/cham3-review` is detached at b06d7cb. Publication worktree `/workspace/cham3-publication` is independent. Evidence `/workspace/review-evidence`, plus each clone's ignored `out/`. All test containers run with `--user "$(id -u):$(id -g)"`, no `--privileged`, no capabilities added, no qualification `--record`. Positive protection uses only the requested memlock ulimit. External adversarial tests mount candidate `/work:ro` and use synthetic public test keys, minimum calibrated real Argon2 for vault tests, and temporary protected directories.

Network policy snapshot says unrestricted HTTP through the inherited proxy, VPN false; runtime status reports desired unrestricted mode, observed state `unknown`, not proof of enforcement. Actual Git/package/upstream reads worked. CLI GitHub calls initially worked, one interval returned `401 Bad credentials` / Git username failure, and later recovered. Both `gh run download` attempts reached the Azure artifact URL but returned **Forbidden (HTTP 403)**. Connected GitHub tools downloaded both ZIPs to File Service; `curl --fail --show-error --location` materialized them successfully with TLS verification intact. Thus artifact verification itself is PASS, not BLOCKED. No credentials or proxy settings were replaced.

## Required changes

### M2-005-R01 — Medium — directory durability is skipped on retry and at store creation

**FAIL.** Primary location `core/registryd/src/journal.rs:229–233`; also `journal_store_t::open` at lines 130–133.

`create_journal` syncs `users` only when this particular call creates `<uid>`. If creation succeeds but that parent fsync fails, the empty directory remains. The next enrollment sees an existing directory, skips the failed parent sync, writes and syncs the journal, syncs `<uid>`, and returns success. Fsync of a child directory does not establish durability of the child's name in its parent. A crash can therefore lose a supposedly durable enrollment and later journal contents. Independently, `open` creates `root/users` without syncing `root` (and may create root without syncing its parent).

The reviewer injected exactly one `sync_directory(users)` EIO. First enrollment returned `ipc/storage_unavailable`, directory existed, journal did not. Retry returned `Ok(append_receipt_t { journal_seq: 1, already_present: false })`; the only directory sync was `users/1001`. A second spy on fresh initialization observed only `users` and `users/1001`, never root. Both assertions fail identically in debug and release. These are deterministic I/O-boundary observations, not a claimed physical power-loss experiment.

Required: make every newly created directory entry durable in its parent before acknowledging enrollment; preserve or safely reconstruct failed durability obligations on retries/restart, or keep the user/store unusable. Cover a failed parent sync followed by retry and fresh root/users initialization. ADR 0006 §1 and recorded M2-005 scope require directory fsync at creation; deferring anchor/crash recovery does not defer the basic append durability guarantee. Decisions 2 and 11 are acceptable fail-closed intentions, but do not justify acknowledging this retry as durable.

### M2-005-R02 — Medium — a new grant revives eligibility for an ended session

**FAIL.** Primary location `core/registryd/src/lineage.rs:579–587`; supporting `core/policyd/src/evaluate.rs:259–277` and the registry view's absence of ended-session IDs.

`session_ended` ends only grant IDs present at that instant; the session ID itself is forgotten. Subsequent admission can add another grant bound to the same terminated session. Policy sees the new grant as unended and accepts an otherwise fresh positive attestation, even if that attestation predates the durable session end. ADR 0007 §6 says ended sessions never become live again; PROJECT §5.3 binds a session grant to an actual live session.

Reproduction over real registry/identity/contract/policy composition: admit genesis and temporary session grant; close the services; append a correctly chained `session_ended` record at 09:00:00; replay; supply a live attestation observed at 08:59:59 (one second old); verify the original grant is denied; submit a newly signed temporary sibling with a fresh grant/request ID but the same session ID. Admission succeeds at journal sequence 7 and `evaluate` returns **eligible** for the new grant. Both debug and release fail the deny assertion. No forged signature, grant-ID reuse, clock rollback or old branch extension is involved. The test authors the lifecycle record because its writer is intentionally M2-006; replay and the record's effect are explicitly in M2-005.

Required: retain terminal session identity in the derived journal state and prevent eligibility for every grant bound to it, including newly admitted grants; choose a compliant admission refusal or ensure the resulting grant stays denied. Carry terminal sessions in the evaluation snapshot and make durable termination dominate a still-fresh pre-termination attestation. Add the sequence to both oracle and real-service tests, including replay and network grants. The oracle currently has the same projection gap (`ecr_oracle.py:1119–1124`); passing its current vectors cannot establish this ADR property. This is a specification violation beyond the vectors, not a Rust/oracle disagreement on a published scenario.

### M2-005-R03 — Medium — restrictive reserve is not preallocated

**FAIL.** Location `core/registryd/src/registry.rs:444–447`, with no allocation/release path in `journal.rs`.

`check_quota` subtracts 1 MiB from the admission ceiling, and lets invalidations use that numerical allowance. This reserves journal **quota**, not filesystem blocks. ADR 0006 §1 explicitly requires a **preallocated** 1 MiB reserve released only for restrictive records. A different writer filling the filesystem can exhaust all free space while this journal remains below its quota, leaving no promised capacity for a key invalidation.

Source search finds no preallocation, reserve file, fallocate or extent-release mechanism. After enrollment, the review's recursive `st_blocks * 512` measurement for the entire new store was 16,384 bytes, below the 1,048,576-byte reserve. The allocation assertion fails in debug and release. A separate real ENOSPC reproduction runs on a disposable 2 MiB Docker tmpfs. Valid certificate appends first align the journal to 4,090 bytes so the next 479-byte invalidation frame needs another block. An unrelated writer fills 2,093,056 bytes until ENOSPC; the invalidation returns `Err(ipc(storage_unavailable))`. The required-success assertion fails in debug and release (one test each, exit 101). An initial unaligned test succeeded because its invalidation fit in the existing block; alignment makes this test discriminate between spare bytes and reserved capacity. No host filesystem was filled. Tmpfs establishes actual allocation exhaustion, not power-loss persistence. Existing quota tests only exhaust the configured journal limit and thus miss competing filesystem allocation. Existing failed-write denial remains fail closed; the defect is the missing durability capacity guarantee, not a demonstrated immediate grant bypass.

Required: provide actual reserve allocation and release/replenishment with appropriate durability and failure handling, and test restrictive writes under actual filesystem exhaustion in a disposable filesystem. Keep admission out of that reserve. Neither the task's recorded sequencing nor the handoff lists physical reserve allocation as deferred; no owner waiver is inferred.

## Scope, behavior and decision assessment

| Check | Result | Evidence and limits |
|---|---|---|
| Exact targets, safe Rust, dependencies | PASS | registryd/policyd/contractd roots forbid unsafe. Cargo.lock contains the same 56 package identities/versions/checksums/sources; only dependency edges changed. No new external crate. |
| Shared M2-003/M2-004 behavior | PASS | `decode` still uses 61,440 bytes, same decoder/order; new `decode_container` raises only total container budget, keeps byte-string limit, refuses tag 18. Outcome additions preserve existing strings. Unchanged vault/native/provider source and independent format/layer-6 regression evidence. |
| Recorded sequencing | PASS for staged scope | `prepare_proposal`/`check_proposal`, revocation/restriction writers, anchor/ledger, recovery, trusted time/±300 s, one-shot operations and delegation move to M2-006; sockets/processes to M2-007. ADR 0007 §8 Option B remains required for M2-006. This is not full A06–A08 or deployed enforcement approval. R01–R03 concern explicitly implemented store/lifecycle/reserve behavior. |
| Journal format, budget, chain | PASS | Closed records, valid time and positive sequence, ascending 1–64 restriction IDs, 42-byte versioned/user-bound header, C3JR/u32/frame chain, 0-based chain vs 1-based record sequence; 65,536-byte container and one envelope. Full header/frame/truncation tests; own correctly chained wrong metadata/unknown terminal records deny. |
| Journal paths and append failure | PASS within protected-directory/single-registry-writer assumptions | Owner/type/group-other-mode/symlink/inode checks and bounded read; unsafe paths reject. File/directory checks prohibit extra group/other access, but do not insist on exact owner permission bits (e.g. owner execute on a regular file); no new access to other UIDs results. Existing append write/torn/sync failures poison the journal and deny reads/writes. New-directory durability is FAIL R01. No cross-process writer/host crash claim. |
| Replay | PASS for tested byte/authority/admission checks; lifecycle completeness FAIL R02 | Header UID/user enrollment binding, chain, sequential records, layers 1–7, recomputed commit/lineage/placement, invalid identities/signatures/parents/duplicates and premature expiry deny. Current configuration quotas/epoch and current-time signing window are not replayed as present-time facts; historical signed payload-window validation remains. |
| Layer 7 | PASS for frozen placement/delta/identity/restore/uniqueness/network rules; FAIL lifecycle R02 | Oracle comparison has real positive counts: 10 scenarios, 53 steps, 90 comparisons, no disagreements; 18 invalid vectors; 8 lineage commits and their replay rejected. It intentionally omits layer-6 recovery scenarios, which identityd covers separately. Seen grant/request IDs persist; no implicit merge union; branches end on any trunk admission; restore matches persistent ancestral scope and expiration; subject transitions constrained. |
| Differential sensitivity | PASS within projection | Driver asserts scenario/step counts and comparisons greater than steps; expected first errors and standings compared, unknown steps panic. 28 seeded registry/policy defects require the named Rust test to fail, not a compilation error; clean controls require exactly one passed test. It cannot pass with zero comparisons, but does not cover R02. |
| Admission snapshot/locking/identity | PASS | Per-user lock spans same identity/ECR snapshot validation, registry-time key window, current epoch, lineage/quota checks, write+sync, then derived-index update. Roles constrain append_identity to identityd and admit to contractd. First enrollment under users map lock; existing slot releases map lock before acquiring user lock; no inverse acquisition found. identity_state pages sequence-ordered identity records over actual store; this in-process Vec is not proof of future wire byte-size pagination. |
| Admission capacity/durability overall | FAIL | Logical byte ceiling and lineage ceiling pass; independent 32 rounds of conflicting Direct/Tor sibling writers yield exactly one success, other network_grant_conflict, replay one occupying slot. Failed identity append denies subsequent views. R01 directory durability and R03 physical reserve remain. |
| Current authorization rules 1–7 | PASS except ended-session reuse R02; overall FAIL | Mandatory policy/emergency denial, same-owner resource and exact runtime subject, live trunk persistent/live-branch temporary filtering, origin key compromise including identityd pending denial, epoch floor, start/expiration, session identity/5 s age/future rejection, reservations, unsupported delegation and missing-input denial inspected/tested. A later persistent restatement does not change the original grant's signing key; own compromise control passes. Pending admission status is correctly not enforcement or an eligibility gate. |
| Network mode | PASS for slot/conflict/mandatory/unknown behavior; R02 applies to session grants | Count eligible grants before selecting; conflicting/unknown state is Offline, no iteration-order authority choice. Admission counts occupying IDs across all live states and does not free by clock arithmetic. |
| contractd | PASS | Owned trusted-client submission; strict byte verifier then commit authority/scope before registry admission; unavailable identity fails closed; registry rechecks. `verify_object` reports validity and never calls admit. Role and malformed/signature/class/owner/unknown controls pass. |

Author decisions: **1, 3–10 and 12 accepted within staged scope**. Decision 2's fail-closed intent and decision 11's absence-vs-partial-file distinction are reasonable, but their creation/retry implementation is subject to R01. Decision 7's session match and 5-second bound are necessary, insufficient without R02's terminal session identity. Decisions 5/9 do not waive the ledger or cross-service integration obligations. The reserve shortfall R03 is not an approved author decision. No owner choices are made.

## M2-004 revision-2 resolution

| Prior item | Status | Independent basis |
|---|---|---|
| R01 pending compromise downgrade | PASS | Both already-present/newly-appended branches settle only when durable state is at least as restrictive. Original reproducer passes. New test covers two keys, failed appends, read fault, refresh/rebuild and final durable restart. |
| R02 signing window | PASS | Inclusive current UTC bounds at registration and approval before/after derivation; historical timestamp check separate. Original expiry-crossing reproducer passes; new helper probes endpoints, i64 extremes and unreadable times. |
| R03 final authority/scope | PASS for requested fix | Post-unlock key/object check and identity-signature/ceremony rechecks inspected. Original clock-scheduled reproducer passes; supplied interleaving fixture explicitly executes a change and counts resolutions, with positive signature verification. It narrows the sample-to-sign interval, not cross-service atomicity. |
| R04 proposal ID reuse | PASS | Used/cancelled markers retain IDs without payload bytes until lifetime end; used marker cannot be cancelled away; bounded retention. Original interleaving reproducer and new cancelled-ID control pass. M2-005 separately rejects duplicate request IDs. |
| R05 recovery erasure | PASS | Every failure reconciles via status/pending bundle, no error-only erasure; original definitive-error reproducer passes; supplied unavailable-status/open/abort cases and controls pass. |
| I01 recovery finalization wording | PASS | Documentation now distinguishes completing existing signed/durable bundles from fresh signing and enrollment authentication. Original stricter assertion still fails: expected authentication_failed, actual accepted. That assertion was already classified as stronger than R14; it is not a new signing bypass. |
| Earlier PASS items | PASS within prior limits | Only nine intended revision-2 paths change; no dependency, format, key-object/KDF/store/protection change. Original role/owner, byte-proof, substituted-object, clock, status and restart controls pass; qualification/source locks/oracles and positive protection repeat at ca40c62. |

Original external package, unchanged: **17 tests in each profile: 16 PASS, 1 FAIL (I01 only), exit 101**. New package: three additional tests PASS (the substring filter also runs the existing unknown-status recovery control: four total PASS). The initial new-test run missed its copied vector path; fixed the external fixture layout and reran, no candidate change. M2-005 test development initially attempted a no-op revision, rejected intrinsically; corrected to a real added grant before measuring origin compromise. Neither setup error is a product finding.

Carried obligations remain acceptable for staged M2-004: durable restriction ledger across identityd restart and real registry/anchor recovery in M2-006; signature/restriction serialization across services and socket/process boundaries M2-006/007; no human-presence guarantee from passphrase/digest alone; no same-UID isolation claim before M3. M2-005 fulfills the duplicate-request admission obligation, but its independent findings must be fixed before its approval.

## Commands and verification record

All shell commands ran as outer UID 1000; Docker build from `/workspace/cham3-review`. Tests in clean clones at exact SHAs, log output outside source or under ignored `out/`. No source patch was applied. The following are the exact command forms used (the loop states both identical invocations compactly):

```sh
git fetch origin refs/heads/codex/m2-cryptographic-contract-core:refs/remotes/origin/codex/m2-cryptographic-contract-core
git worktree add --detach /workspace/cham3-review b06d7cb21e1e3e42bbcd025abd9cea131574751d
git clone --no-hardlinks /workspace/cham3-review /workspace/m2-005-clean
git -C /workspace/m2-005-clean checkout --detach b06d7cb
git clone --no-hardlinks /workspace/cham3-review /workspace/m2-004-clean
git -C /workspace/m2-004-clean checkout --detach ca40c62
docker build -f build/m2/dev_container.dockerfile -t cham3-m2-review build/m2

for n in 004 005; do
  docker run --rm --user "$(id -u):$(id -g)" \
    -e CHAM3_M2_REQUIRE_CLEAN=1 \
    -v /workspace/m2-$n-clean:/work -w /work cham3-m2-review \
    bash build/scripts/qualify_m2_rust.sh
  docker run --rm --user "$(id -u):$(id -g)" \
    --ulimit memlock=1073741824:1073741824 \
    -v /workspace/m2-$n-clean:/work -w /work cham3-m2-review \
    sh tests/m2/protection/check_vault_protection.sh out/m2/protection
  docker run --rm --user "$(id -u):$(id -g)" \
    -v /workspace/m2-$n-clean:/work -w /work cham3-m2-review \
    bash -c 'bash build/scripts/run_static_checks.sh > out/static_checks_review.log 2>&1; a=$?; sh tests/run_tests.sh static > out/static_tests_review.log 2>&1; b=$?; echo "static_checks=$a static_tests=$b"; test "$a$b" = 00'
done
```

| Check | M2-004 ca40c62 | M2-005 b06d7cb | Evidence |
|---|---|---|---|
| Pinned image build | PASS | PASS, same unchanged inputs | `docker-build.log`, image digest above; package/deb/crate locks match |
| Clean qualification | PASS / exit 0 | PASS / exit 0 | `qualify-004.log`, `qualify-005.log`; clean-source requirement enabled, no --record |
| Unit inventory debug/release | PASS 121 + 121 | PASS 157 + 157 | cargo_test_debug/release.json, exact test-name inventories match CI |
| Provider oracle | PASS 7/7 | PASS 7/7 | provider_oracle.json; Rust RFC Argon2 KAT and independent ordinary-input result; existing libargon2 RFC secret/AD API `NOT_CHECKED` remains disclosed |
| ECR oracle | PASS 19/96/18; 22 scenarios/346 steps | Same PASS | byte-identical local/CI ecr_vector_oracle.log |
| Format differential | PASS 5950 / 0 mismatches | Same PASS | fixed seed 3, corpus digest `080d0c86a6787d7a60e58c86a92717aa7e7ad71c8556c2f43de99b634c43257d`; full outcome coverage matches CI |
| Vault-object oracle | PASS 38 checks | PASS 38 checks | byte-identical local/CI vault_object_oracle.log |
| Harness | PASS 133 (20 identity/vault seeded defects) | PASS 135 (20 identity/vault + 28 registry/policy seeded defects) | python_harness.json and seeded defect source inspected; clean controls and named-test-failure requirements |
| Protection positive | PASS local and CI | PASS local and CI | `protection004.log`, `protection005.log`; dumpable 0, core 0, unlocked mappings 0, protection_verified, exit 0 |
| Static checks | PASS / exit 0 | PASS / exit 0 | shellcheck, py_compile, yamllint, secret-pattern scan |
| Static tests | PASS 82 / exit 0 | PASS 82 / exit 0 | respective `out/static_tests_review.log` |
| Original reviewer 17 tests | FAIL / 101: 16 pass, I01 fails; both profiles | NOT_RUN unchanged at this target | Exact ca40c62 prerequisite reproduction, source digests below; I01 is not reportable |
| Added prerequisite tests | PASS three new + one matched existing control | NOT_RUN separately | `extra004-final.log`, pending-denial order/read-fault/rebuild, window boundaries, cancelled ID retention |
| New M2-005 adversarial tests | NOT_RUN at pre-implementation target | FAIL / 101, both profiles: 9 tests, 5 pass, 4 assertions fail (3 findings) | `own005-debug-final.log`, `own005-release.log`; details below |
| Actual filesystem exhaustion | NOT_RUN | FAIL / 101 in debug and release: key invalidation unavailable | Dedicated 2 MiB tmpfs; `full-disk005-final.log`, `full-disk005-release.log` |
| Layer-6 differential direct | PASS 12/318/0; two tests | PASS 12/318/0; two tests | scenarios004.log, scenarios005.log |
| Layer-7 differential direct | NOT_RUN, absent | PASS 10/53/90/0; four tests including invalid vectors and 8-commit lineage | scenarios005.log |
| Miri, sanitizers, coverage-guided fuzzing | NOT_RUN | NOT_RUN | pinned toolchain reports NOT_AVAILABLE; valgrind and bounded mutation tests do not replace these |
| Real processes/IPC, enforcement, anchor/ledger/recovery, host power loss | NOT_RUN | NOT_RUN | staged or explicitly stated verification limits; no simulated test is claimed as system proof |

External test commands (first old-package run used `--offline` to generate only its own lockfile; subsequent runs `--locked --offline`):

```sh
docker run --rm --user "$(id -u):$(id -g)" \
  -v /workspace/m2-004-clean:/work:ro \
  -v /workspace/review-evidence/adversarial004:/evidence -w /evidence \
  cham3-m2-review cargo test --manifest-path core/reviewer/Cargo.toml \
  --offline -- --nocapture --test-threads=1
# Same command with --release --locked --offline for the unchanged release run.

docker run --rm --user "$(id -u):$(id -g)" \
  -e CARGO_TARGET_DIR=/evidence/core/reviewer/target \
  -v /workspace/m2-004-clean:/work:ro \
  -v /workspace/review-evidence/adversarial004:/evidence -w /evidence \
  cham3-m2-review cargo test --manifest-path core/reviewer/extra/Cargo.toml \
  --locked --offline own_ -- --nocapture --test-threads=1

docker run --rm --user "$(id -u):$(id -g)" \
  -v /workspace/m2-005-clean:/work:ro \
  -v /workspace/review-evidence/adversarial005:/evidence -w /evidence \
  cham3-m2-review cargo test --manifest-path core/reviewer/Cargo.toml \
  --locked --offline ::own_ -- --nocapture --test-threads=1
# Same command with --release for release verification.

# Separate actual ENOSPC test; repeat with --release for the release profile.
docker run --rm --user "$(id -u):$(id -g)" \
  --tmpfs /full-disk:rw,size=2m,uid=1000,gid=1000,mode=0700 \
  -v /workspace/m2-005-clean:/work:ro \
  -v /workspace/review-evidence/adversarial005:/evidence -w /evidence \
  cham3-m2-review cargo test --manifest-path core/reviewer/Cargo.toml \
  --locked --offline filesystem_full_reserve -- --nocapture --test-threads=1

docker run --rm --user "$(id -u):$(id -g)" \
  -v /workspace/m2-005-clean:/work:ro -v /workspace/review-evidence:/evidence \
  -w /work cham3-m2-review bash -c \
  'CARGO_TARGET_DIR=/evidence/differential-target cargo test --frozen -p cham3_registryd --test admission_scenarios -- --nocapture && CARGO_TARGET_DIR=/evidence/differential-target cargo test --frozen -p cham3_identityd --test journal_scenarios -- --nocapture'
# ca40c62 direct differential: m2-004-clean:/work:ro, differential004-target,
# identityd journal_scenarios only, same remaining options.
```

The five new passing M2-005 tests cover six correctly chained invalid journals plus restored positive control; 32 rounds of concurrent network siblings and replay; failed identity append denying views; 5-second session boundary/future/stale/unreadable-clock cases; and an original key compromised after another persistent key restates its grant. The four failing assertions are R01's two durability cases, R02 and R03. The separately selected filesystem-full test also fails for R03 in both profiles. Source construction and complete additions are embedded below for reproduction without workspace persistence.

## CI and artifact comparison

**PASS for all six run identities and all job/step outcomes.** Connected `github_fetch` reads of `https://api.github.com/repos/Kayyo321/cham3/actions/runs/<id>` confirmed each exact head SHA and success. `github_fetch_workflow_run_jobs` and `github_fetch_workflow_job_logs` retrieved all 14 job logs (two M2, six M1, six M0):

| Target | Runs (M2 / M1 / M0) | Job IDs |
|---|---|---|
| b06d7cb | 38404018473 / 38404018403 / 38404018497 | 115318746873; 115318761487, 115318761510, 115318761641; 115318747704, 115318747903, 115318748088 |
| ca40c62 | 38347259191 / 38347259188 / 38347259258 | 115131427974; 115131427820, 115131427985, 115131428080; 115131428041, 115131428364, 115131428412 |

M1 at both targets: package tests 8, privileged capture 77, static 82, desktop 261, staging 16, native pin identity 22, all OK. M0 at both: package 8, image 17, boot login prompt reached, boot-result 18, static 82, all OK. Privileged M0/M1 suites are CI evidence, not claimed locally rerun.

Both M2 workflows contain a successful `Vault process protection, positive path (memlock limit raised, unprivileged)` step. CI reports dumpable/core/unlocked mappings all zero, required memlock 805306368; release derivation 420 ms at b06d7cb and 471 ms at ca40c62. Local positive reports have the same protection properties; timing differs (local M2-005 606 ms). CI qualification uses --record; these reviewer runs do not.

Required CLI attempts:

```sh
gh run view 38404018473 --repo Kayyo321/cham3 --json headSha,conclusion,jobs,url
gh run download 38404018473 --repo Kayyo321/cham3 -n m2_qualification_amd64 -D /workspace/ci-m2-005
gh api 'repos/Kayyo321/cham3/actions/runs/38404018473/artifacts'
gh run download 38347259191 -n m2_qualification_amd64 --repo Kayyo321/cham3 -D /workspace/review-evidence/gh004
```

First view PASS; both downloads BLOCKED at their redirected `productionresultssa8.blob.core.windows.net` / `productionresultssa7.blob.core.windows.net` ZIP URLs with exact error suffix `Forbidden` (HTTP 403). The intervening API attempt returned `gh: Bad credentials (HTTP 401)`. A Git fetch during that interval returned `fatal: could not read Username for 'https://github.com': No such device or address`; later Git fetch/ls-remote worked. Expiring signed URL query credentials are intentionally not reproduced in the published record.

Fallback **PASS**: `github_fetch_workflow_run_artifacts`, `github_download_workflow_artifact(artifact_id=11819401906)` and `(artifact_id=11789878130)`, then `curl --fail --show-error --location '<returned download_url>' -o <zip>` and `sha256sum`:

- b06d7cb: 100,963 bytes, SHA-256 **c9cfe469bbc109faff6fe7a82a8eccbe1bb7a4f9f7073292e83b5c21b13301b5**, exactly the requested digest.
- ca40c62: 97,312 bytes, SHA-256 **6315b087535ee9bec503e2291ab3e01d76c4bd3ebbfa35fc4a7c7d07bb895a5d**, matches advertised metadata.

ZIPs were extracted with Python zipfile into `artifact005`/`artifact004`. `python3 /workspace/review-evidence/compare_artifacts.py` compares unit test names/counts, result codes, differential coverage/corpus, provider outputs, harness test inventory, oracle logs, installed/deb package inventories and qualification summaries, excluding timing/build-path fields where necessary. **PASS for both complete comparisons**; every compared result, inventory, coverage field, oracle output and package lock matches. Debug/release tests are actual inventories, not merely a reported total. Digest verification is over downloaded ZIP bytes, not only metadata.

Upstream reachability retried successfully, without proxy/TLS bypass:

```sh
curl --fail --show-error --location https://security-tracker.debian.org/tracker/source-package/rust-rustix -o /workspace/review-evidence/debian-rustix.html
curl --fail --show-error --location https://rustsec.org/advisories/ -o /workspace/review-evidence/rustsec.html
```

Both PASS (HTTP retrieval), not a claim of a new exhaustive dependency-advisory audit. Dependency identities and qualified crate sources are unchanged; the task's new vulnerability review concerns the stated diff.

## Required Codex Security plugin attempt

**BLOCKED.** Exact skill `codex-security:security-diff-scan`; package `skill://Plugin_1e648473be9c8191a91ac3947151af55/security-diff-scan`. Version **unavailable**: no version field in skill front matter or exposed installed manifest. The manual review and its findings above are not plugin findings.

Executed tool calls and recovery:

1. `skills.read({"package":"c0/security-diff-scan"})`: PASS, read the full workflow. Intended immutable ranges were 6968bd6..b06d7cb and prerequisite bb3f1ba..ca40c62.
2. `skills.read({"package":"c0/security-diff-scan","resource":"skill://Plugin_1e648473be9c8191a91ac3947151af55/references/config-preflight.md"})`: exact error **`failed to read skill resource`**, initial attempt and retry.
3. Same read for `.../references/desktop-config-preflight.md`: same exact error on initial attempt and retry.
4. Callable tool inventory searched for `codex_security`, `start_codex_security_prompt_only_scan`, scan/preflight methods: **zero matching backend methods**. No nonexistent tool invocation or scan ID is claimed.
5. `plugin_management_search_plugins({"query":"Codex Security","limit":5})`: confirms installed/enabled `Plugin_1e648473be9c8191a91ac3947151af55`; provides no version or callable security backend.
6. `skills.list({"authority":{"kind":"executor"}})`: `skills: []`, no warnings.
7. Terminal fallback search `rg --files --hidden /opt/codex /workspace /home/agent -g config-preflight.md -g desktop-config-preflight.md -g launch_codex_security_mcp -g generate_in_scope_files.py -g finalize_scan_contract.py 2>/dev/null; command -v launch_codex_security_mcp`: exit 1, no output.

The skill requires preflight before a scan goal; neither required preflight resources nor backend/fallback scripts are available. No goal was created, no security scan was completed, no plugin findings were emitted. This is a capability/resource blocker despite successful unrestricted-network fetches. The plugin remains required for final M2-008 review; this record does not waive it.


## Combined prerequisite verdict

The publication identity and SHA-256 of this combined record are supplied in the final handoff rather than embedded recursively in its own bytes.

```text
=== CHAM3 M2-004 R2 VERDICT ===
reviewer: Codex
environment: c3235e23df6d / Debian 13.6 / x86_64; UID 1000; Docker 28.4.0; network and fallback results above
review_record: review/m2-005-security : docs/reviews/M2-005-codex-security-review.md (this combined record; commit/digest in final handoff)
candidate_reviewed: ca40c62ed7a5fc29ba8910fef514f3ce29ea4fba
docs_commits_reviewed: acefb668976359d2b123b29246f86e717f41930a (docs-only: yes)
verdict: M2-004: APPROVE
prior_findings: R01=PASS R02=PASS R03=PASS R04=PASS R05=PASS I01=docs corrected: yes earlier PASS items intact=yes
carried_obligations: recorded and acceptable for staged scope: yes; restriction ledger and serialization remain M2-006/007; duplicate request IDs enforced in M2-005
qualification_amd64: PASS (121+121; provider 7/7; ECR 19/96/18 and 22/346; format 5950/0; vault 38; harness 133)
protection_positive_path: PASS (own unprivileged run and CI positive step)
static_checks: PASS   static_tests: PASS (82)
own_tests: original unchanged 17 in debug/release: 16 PASS, I01 stronger-than-contract assertion FAIL; three added tests PASS, plus existing control PASS
security_plugin: BLOCKED (codex-security:security-diff-scan; version unavailable; required preflight reads fail and backend absent; exact calls/errors above)
ci_comparison: PASS (all three exact-head workflows, downloaded ZIP digest and complete local/artifact comparison)
new_findings: none in M2-004 revision 2
notes_for_owner: staged-scope approval; independent M2-005 findings R01-R03 require correction; no owner decision or M2-008 plugin waiver
=== END VERDICT ===
```

## Evidence integrity

Both exact-candidate clones remained clean after all runs (`git status --porcelain`: empty). Only this record is staged for publication. Logs remain in the review workspace; the appendix contains portable reproductions and the immutable CI ZIP digests above identify upstream evidence. SHA-256 of final local evidence:

| Evidence under /workspace/review-evidence | SHA-256 |
|---|---|
| `qualify-004.log` | `a5a42b85bcf0d792cc10f23a366b3ae84548be464a76eb6c04e82bb110714813` |
| `qualify-005.log` | `6cb5587c9356248230aba323e352f1c462381bc34049d001a5d239e59610fea8` |
| `ci-comparison.log` | `719735ff6c9c20dc5c6b90e01e8d8267c3d23fdccae688ee9c507c88086102d5` |
| `old-tests-debug.log` | `9441f99b869eeb1e7f96333e928122789b027ebf50029836fcbdfb59cffe70b0` |
| `old-tests-release.log` | `2d65fa4ff8d42a6b1210ad8fa4d4cb8338bb41312bd90f9a3ff919ce8a7d8132` |
| `extra004-final.log` | `d6ca82343f4b727e2d89e3e4c37292242406153dc3524239f6fffeb32bba0534` |
| `own005-debug-final.log` | `c99682a609d30657bfd11c2083572b2647b142f5df6d596a2087b067a8e03c57` |
| `own005-release.log` | `274b7ff44eb342d8bb35b339f6d28076a4bb891b3f4bf08262ed235dff182723` |
| `full-disk005-final.log` | `26273d6bf2ebf1d59be006de65cfd4fe8eed0f2ba61cf76ecbce0d2815dddd94` |
| `full-disk005-release.log` | `e0ccc8fb497c3b3dea64a052d65bd2df76d6917e05e1b014e0289d5e6959dc8d` |
| `protection004.log` | `3169b1270fb19a4382a11c8146822eff66ea034de56993b15026cf9638023e49` |
| `protection005.log` | `7597cbc7246945da1b16ce3b16cbeafe996b00ae8da938b0ed892998feffedf7` |
| `static004.log` | `09f2f9a94389506eeabaa2b4a2554dfb269d8cb4e353aeabd596a7424e5092a7` |
| `static005.log` | `09f2f9a94389506eeabaa2b4a2554dfb269d8cb4e353aeabd596a7424e5092a7` |
| `scenarios004.log` | `23df932635e6def685be9021c2d0d5854eb63111fc8edb7c85b22bc135a4f613` |
| `scenarios005.log` | `c3ce7a506241f69e98418f25220710b28565910af6032dc7b3356dfe1c232ff5` |


## Portable adversarial reproduction appendix

The following additions are reviewer code, not a patch to the candidate. Reconstruct the external M2-005 package using this Python setup (run outside both candidate clones), then append the two additions below to the generated module files. `/work` in Rust paths is the read-only Docker mount, not the outer pathname. This deliberately reuses candidate synthetic builders and setup helpers; all new assertions and failure sequences are reviewer-authored. The `::own_` filter selects only the nine new tests; 23 copied tests and the separately selected filesystem-full test remain filtered (24 total); normal qualification independently runs the candidate tests. The additional `filesystem_full_reserve` test must be selected separately with the dedicated tmpfs command above.

```python
from pathlib import Path
import shutil
r = Path('/workspace/review-evidence/adversarial005')
p = r / 'core/reviewer'
p.mkdir(parents=True, exist_ok=True)
(r / '.cargo').mkdir(exist_ok=True)
shutil.copy('/workspace/m2-005-clean/.cargo/config.toml', r / '.cargo/config.toml')
v = r / 'tests/m2/vectors/ecr_v1'
v.mkdir(parents=True, exist_ok=True)
shutil.copy('/workspace/m2-005-clean/tests/m2/vectors/ecr_v1/vectors.json', v / 'vectors.json')
for name, source in [('registry', 'registryd/tests/registry_operations.rs'),
                     ('policy', 'policyd/tests/current_authorization.rs')]:
    text = Path('/workspace/m2-005-clean/core', source).read_text()
    text = text.replace('mod builders;', '#[path = "/work/core/registryd/tests/builders/mod.rs"]\nmod builders;')
    text = text.replace('#[path = "../../registryd/tests/builders/mod.rs"]\n', '')
    (p / (name + '.rs')).write_text(text)
```

### M2-005 external Cargo.toml

```toml
[package]
name = "m2_005_independent_review_tests"
version = "0.1.0"
edition = "2024"
[dependencies]
cham3_registryd = { path = "/work/core/registryd" }
cham3_policyd = { path = "/work/core/policyd" }
cham3_contractd = { path = "/work/core/contractd" }
cham3_identityd = { path = "/work/core/identityd" }
cham3_contract_format = { path = "/work/core/contract_format" }
ed25519-dalek = { version = "=2.1.1", default-features = false, features = ["std", "zeroize"] }
[lib]
path = "review.rs"
[profile.release]
overflow-checks = true
```

### M2-005 external review.rs

```rust
#![forbid(unsafe_code)]
#![allow(dead_code, non_camel_case_types, non_upper_case_globals)]
mod registry;
mod policy;
```

### Append to external registry.rs

```rust
// Reviewer additions: external tests only; production source mounted read-only.
#[derive(Clone)]
struct sync_spy_t {
    directories: Arc<Mutex<Vec<PathBuf>>>,
    fail_users_once: Arc<std::sync::atomic::AtomicBool>,
}
impl journal_io_t for sync_spy_t {
    fn write_all(&self, file: &mut fs::File, bytes: &[u8]) -> io::Result<()> { system_io_t.write_all(file, bytes) }
    fn sync_data(&self, file: &fs::File) -> io::Result<()> { system_io_t.sync_data(file) }
    fn sync_directory(&self, path: &Path) -> io::Result<()> {
        self.directories.lock().unwrap().push(path.to_owned());
        if path.file_name().unwrap() == "users" && self.fail_users_once.swap(false, Ordering::SeqCst) {
            return Err(io::Error::other("review injected parent directory fsync EIO"));
        }
        system_io_t.sync_directory(path)
    }
}
#[test]
fn own_retry_must_not_acknowledge_an_unsynced_user_directory() {
    let root=temp_root("own_retry_sync");
    let calls=Arc::new(Mutex::new(Vec::new()));
    let spy=sync_spy_t { directories:calls.clone(), fail_users_once:Arc::new(std::sync::atomic::AtomicBool::new(true)) };
    let registry=open_with(&root,registry_config_t::default(),Box::new(spy));
    let user=test_user(1001);
    let first=registry.append_identity(Identityd,user.uid,object_kind_t::identity_enrollment,&user.enrollment);
    assert_eq!(code(first),"ipc/storage_unavailable");
    assert!(root.join("users/1001").is_dir());
    assert!(!journal_path(&root,user.uid).exists());
    calls.lock().unwrap().clear();
    let retry=registry.append_identity(Identityd,user.uid,object_kind_t::identity_enrollment,&user.enrollment);
    println!("retry={retry:?}, directory syncs={:?}",calls.lock().unwrap());
    assert!(retry.is_err() || calls.lock().unwrap().contains(&root.join("users")),
        "retry acknowledged enrollment without retrying the failed parent fsync");
}
#[test]
fn own_new_store_syncs_users_entry_in_root_before_ack() {
    let root=temp_root("own_root_sync");
    let calls=Arc::new(Mutex::new(Vec::new()));
    let spy=sync_spy_t { directories:calls.clone(), fail_users_once:Arc::new(std::sync::atomic::AtomicBool::new(false)) };
    let registry=open_with(&root,registry_config_t::default(),Box::new(spy));
    let user=test_user(1001);enroll(&registry,&user);
    println!("directory syncs={:?}",calls.lock().unwrap());
    assert!(calls.lock().unwrap().contains(&root),"new root/users entry never synced in root");
}
#[test]
fn own_rechained_wrong_metadata_and_terminal_records_fail_closed() {
    let root=temp_root("own_rechained");let u=test_user(1001);let a=app("own");
    let reg=open(&root);enroll(&reg,&u);
    let c=draft(&u,&a,commit_type_t::genesis,"own good").sign(&u);
    admit(&reg,&u,&c).unwrap();drop(reg);
    let original=records_of(&root,u.uid);
    let mut cases=Vec::new();
    for field in ["commit","lineage","placement"] {
        let mut rs=original.clone();
        rs[3]=record(4,record_body_t::commit_admitted {
            commit_id:if field=="commit" {[9;32]}else{c.commit_id},
            lineage_id:if field=="lineage" {[9;32]}else{c.payload.lineage_id},
            placement:if field=="placement" {placement_t::temporary}else{placement_t::trunk},
            envelope:c.envelope.clone() });cases.push((field,rs));
    }
    for (name,lifecycle) in [
        ("unknown restriction",lifecycle_record_t::restriction{grant_ids:vec![[7;16]]}),
        ("unknown session",lifecycle_record_t::session_ended{session_id:[8;16]}),
        ("unknown expiration",lifecycle_record_t::expired{grant_id:[9;16]})] {
        let mut rs=original.clone();rs.push(record(5,record_body_t::lifecycle(lifecycle)));cases.push((name,rs));
    }
    for (label,rs) in cases {
        write_chained(&root,u.uid,&u.identity.key_id,&rs);
        let reg=open(&root);
        assert_eq!(reg.unusable_users().unwrap().len(),1,"{label}");
        assert_eq!(code(reg.lineage_head(Policyd,u.uid,&c.payload.lineage_id)),"ipc/storage_unavailable","{label}");
    }
    write_chained(&root,u.uid,&u.identity.key_id,&original);
    assert!(open(&root).unusable_users().unwrap().is_empty());
}
#[test]
fn own_network_sibling_writers_have_exactly_one_winner() {
    let u=test_user(1001);let a=app("ownrace");
    for round in 0..32 {
        let root=temp_root(&format!("own_race_{round}"));let reg=open(&root);enroll(&reg,&u);
        let g=draft(&u,&a,commit_type_t::genesis,"own race genesis").sign(&u);admit(&reg,&u,&g).unwrap();
        let barrier=Arc::new(Barrier::new(3));let mut workers=Vec::new();
        for mode in ["direct","tor"] {
            let grant=network_grant(mode,u.uid,mode,session_lifetime(mode));
            let c=draft(&u,&a,commit_type_t::temporary_grant,mode).on(&g).temporary_authority().add(vec![grant]).sign(&u);
            let r=reg.clone();let b=barrier.clone();workers.push(std::thread::spawn(move||{b.wait();r.admit(Contractd,1001,&c.envelope)}));
        }
        barrier.wait();let outcomes:Vec<_>=workers.into_iter().map(|w|w.join().unwrap()).collect();
        assert_eq!(outcomes.iter().filter(|x|x.is_ok()).count(),1);
        assert_eq!(outcomes.iter().find_map(|x|x.as_ref().err()).unwrap().code(),"network_grant_conflict");
        drop(reg);let r=open(&root);assert!(r.unusable_users().unwrap().is_empty());
        assert_eq!(r.lineage_head(Policyd,u.uid,&g.payload.lineage_id).unwrap().unwrap().occupying_network.len(),1);
    }
}
#[test]
fn own_failed_identity_append_denies_existing_grants() {
    let root=temp_root("own_failed_identity");let u=test_user(1001);let a=app("ownfailed");
    let fault=Arc::new(Mutex::new(fault_t::none));
    let reg=open_with(&root,registry_config_t::default(),Box::new(faulty_io_t{fault:fault.clone(),appends:Arc::new(AtomicUsize::new(0))}));
    enroll(&reg,&u);let c=draft(&u,&a,commit_type_t::genesis,"own failed").sign(&u);admit(&reg,&u,&c).unwrap();
    *fault.lock().unwrap()=fault_t::fail_write;
    let inv=invalidation(&u.identity,&u.persistent.key_id,invalidation_reason_t::compromised,"own compromise");
    assert_eq!(code(reg.append_identity(Identityd,u.uid,object_kind_t::key_invalidation,&inv)),"ipc/storage_unavailable");
    assert_eq!(code(reg.lineage_head(Policyd,u.uid,&c.payload.lineage_id)),"ipc/storage_unavailable");
}
#[test]
fn own_restrictive_reserve_has_preallocated_storage() {
    fn blocks(path:&Path)->u64 {
        let meta=fs::symlink_metadata(path).unwrap();
        meta.blocks()*512 + if meta.is_dir() {fs::read_dir(path).unwrap().map(|e|blocks(&e.unwrap().path())).sum()}else{0}
    }
    let root=temp_root("own_reserve");let reg=open(&root);let u=test_user(1001);enroll(&reg,&u);
    let allocated=blocks(&root);
    println!("allocated bytes for entire new store={allocated}; promised reserve={}",cham3_registryd::registry::ReserveBytes);
    assert!(allocated>=cham3_registryd::registry::ReserveBytes,"logical quota slack is not preallocated disk space for restrictive records");
}
#[test]
fn filesystem_full_reserve() {
    use std::io::Write;
    // This test runs only with a dedicated 2 MiB Docker tmpfs, never the host disk.
    let disk=PathBuf::from("/full-disk");
    let mounts=fs::read_to_string("/proc/mounts").unwrap();
    assert!(mounts.lines().any(|l|l.split_whitespace().nth(1)==Some("/full-disk") && l.split_whitespace().nth(2)==Some("tmpfs")));
    let root=disk.join("registry");fs::create_dir(&root).unwrap();fs::set_permissions(&root,fs::Permissions::from_mode(0o700)).unwrap();
    let reg=open(&root);let u=test_user(1001);enroll(&reg,&u);
    let inv=invalidation(&u.identity,&u.persistent.key_id,invalidation_reason_t::compromised,"full disk");
    let required=record(100,record_body_t::identity{kind:object_kind_t::key_invalidation,object:inv.clone()}).len()+FrameOverheadBytes;
    let mut aligned=false;
    for n in 0..64 {
        let len=fs::metadata(journal_path(&root,u.uid)).unwrap().len();
        if 4096-(len%4096)<required as u64 {aligned=true;break;}
        let key=test_key(&format!("reserve alignment {n}"));
        let cert=certificate(&u.identity,u.uid,&key,authority_class_t::persistent,1);
        reg.append_identity(Identityd,u.uid,object_kind_t::key_certificate,&cert).unwrap();
    }
    assert!(aligned,"positive setup: next restrictive frame must require a new block");
    println!("journal bytes before disk fill={}; required restrictive frame={required}",fs::metadata(journal_path(&root,u.uid)).unwrap().len());
    let mut filler=fs::File::create(disk.join("unrelated-writer")).unwrap();let mut filled=0usize;
    loop {
        match filler.write_all(&[0u8;4096]) {
            Ok(())=>{filled+=4096;assert!(filled<=3*1024*1024,"tmpfs bound absent");},
            Err(e)=>{assert_eq!(e.raw_os_error(),Some(28));break;}
        }
    }
    let inv=invalidation(&u.identity,&u.persistent.key_id,invalidation_reason_t::compromised,"full disk");
    let result=reg.append_identity(Identityd,u.uid,object_kind_t::key_invalidation,&inv);
    println!("unrelated writer filled {filled} bytes; restrictive append={result:?}");
    assert!(result.is_ok(),"the promised preallocated reserve must let this small restrictive record become durable");
}
```

### Append to external policy.rs

```rust
#[test]
fn own_ended_session_cannot_authorize_a_fresh_grant() {
    use cham3_registryd::record::{registry_record_t,record_body_t,lifecycle_record_t,parse_journal,frame,next_chain};
    use std::io::Write;
    let mut w=world("own_session");let u=test_user(1001);let a=app("ownsession");w.enroll(&u);
    let g=draft(&u,&a,commit_type_t::genesis,"own session genesis").sign(&u);w.submit(&u,&g);
    let old=file_grant("old session",u.uid,"home:Downloads",&["read"],session_lifetime("ended"));
    let sid=old.lifetime.session_id.unwrap();
    let b=draft(&u,&a,commit_type_t::temporary_grant,"own old branch").on(&g).temporary_authority().add(vec![old.clone()]).sign(&u);w.submit(&u,&b);
    let root=w.root.clone();drop(w);
    let path=root.join("users/1001/journal.c3j");let bytes=fs::read(&path).unwrap();let parsed=parse_journal(&bytes,1001).unwrap();
    let rec=registry_record_t{sequence:parsed.records.len() as u64+1,time:"2026-10-13T09:00:00Z".into(),body:record_body_t::lifecycle(lifecycle_record_t::session_ended{session_id:sid})}.encode();
    let chain=next_chain(&parsed.chain,parsed.records.len() as u64,&rec);
    let mut f=fs::OpenOptions::new().append(true).open(&path).unwrap();f.write_all(&frame(&rec,&chain).unwrap()).unwrap();f.sync_all().unwrap();drop(f);
    let mut w=world_at(&root,registry_config_t::default(),mandatory_policy_t::default());
    w.now.set(Some(T1Utc));
    let mut identity=runtime(&u,&a);identity.session_id=Some(sid);
    // A still-fresh attestation issued immediately before the durable end.
    w.sessions.attested.borrow_mut().insert(sid,session_attestation_t{live:true,observed_at:T1Utc-1});
    assert!(!is_eligible(w.evaluate(&identity,&files(u.uid,"home:Downloads","read"))));
    let fresh=file_grant("fresh same session",u.uid,"home:Downloads",&["read"],session_lifetime("ended"));
    let next=draft(&u,&a,commit_type_t::temporary_grant,"own fresh branch").on(&g).temporary_authority().add(vec![fresh]).sign(&u);
    let result=w.contract.submit_signed(caller_role_t::trusted_client{uid:u.uid},&next.envelope);
    let decision=w.evaluate(&identity,&files(u.uid,"home:Downloads","read"));
    println!("fresh ended-session admission={result:?}; authorization={decision:?}");
    assert!(!is_eligible(decision),"durably ended session regained authority through a new grant id");
}
#[test]
fn own_session_attestation_boundaries_and_unreadable_clock() {
    let mut w=world("own_attest");let u=test_user(1001);let a=app("ownattest");w.enroll(&u);
    let g=draft(&u,&a,commit_type_t::genesis,"own attest genesis").sign(&u);w.submit(&u,&g);
    let grant=file_grant("own attest",u.uid,"home:X",&["read"],session_lifetime("attest"));let sid=grant.lifetime.session_id.unwrap();
    let b=draft(&u,&a,commit_type_t::temporary_grant,"own attest branch").on(&g).temporary_authority().add(vec![grant]).sign(&u);w.submit(&u,&b);
    let mut rt=runtime(&u,&a);rt.session_id=Some(sid);let req=files(u.uid,"home:X","read");
    for (offset,expected) in [(55,true),(54,false),(61,false),(60,true)] {
        w.sessions.attested.borrow_mut().insert(sid,session_attestation_t{live:true,observed_at:T1Utc+offset});
        assert_eq!(is_eligible(w.evaluate(&rt,&req)),expected,"offset {offset}");
    }
    w.now.set(None);assert_eq!(w.evaluate(&rt,&req),decision_t::unknown(unknown_reason_t::clock_unavailable));
}
#[test]
fn own_origin_compromise_survives_persistent_restatement() {
    let mut w=world("own_origin");let u=test_user(1001);let a=app("ownorigin");w.enroll(&u);
    let grant=file_grant("origin grant",u.uid,"home:X",&["read"],persistent_lifetime());
    let g=draft(&u,&a,commit_type_t::genesis,"origin genesis").add(vec![grant]).sign(&u);w.submit(&u,&g);
    let next_key=test_key("own second persistent");let cert=certificate_payload(&u.identity,u.uid,&next_key,cham3_contract_format::schema::authority_class_t::persistent,2);
    w.identity.borrow_mut().register_certificate(Vaultd,u.uid,&sign_certificate(&u.identity,&cert)).unwrap();
    let rev=draft(&u,&a,commit_type_t::revision,"origin restated").on(&g).add(vec![file_grant("extra origin",u.uid,"home:Other",&["read"],persistent_lifetime())]).sign_with(&next_key);w.submit(&u,&rev);
    let rt=runtime(&u,&a);let req=files(u.uid,"home:X","read");assert!(is_eligible(w.evaluate(&rt,&req)));
    let inv=invalidation(&u.identity,&u.persistent.key_id,invalidation_reason_t::compromised,"origin compromised");
    w.identity.borrow_mut().register_invalidation(Vaultd,u.uid,&inv).unwrap();
    assert_eq!(w.evaluate(&rt,&req),denied(deny_reason_t::key_invalidated));
}
```

### Prerequisite test reconstruction

For the original M2-004 package, take the `External Cargo.toml` and `External review.rs` fenced blocks verbatim from [the immutable prior review](https://github.com/Kayyo321/cham3/blob/f2d82b7a663c1579582bc5c88fe2832a6f97e061/docs/reviews/M2-004-codex-security-review.md), place them in `adversarial004/core/reviewer/`, copy ca40c62's `.cargo/config.toml` into `adversarial004/.cargo/` and its vectors into `adversarial004/tests/m2/vectors/ecr_v1/`. Original source SHA-256 `922dd57b9ba92ce7b79517c6a3f890a1129626dc61766af6b814ee13df15bea4`; manifest `82224a3c0e00b6cc3ac9c6b4ceaebc810c7d1912ff32366ecfdc8941ba961004`. The unchanged reruns use these exact bytes.

For the added prerequisite tests, copy review.rs to extra.rs in that same directory and append the code below. Put a copy of the original manifest in `core/reviewer/extra/Cargo.toml`, change only package name to `m2_004_extra_review_tests` and library path to `../extra.rs`; copy the same vector file also to `adversarial004/core/tests/m2/vectors/ecr_v1/vectors.json` for this manifest's relative lookup. The command above uses a shared external target directory; `own_` matches the three added tests plus the old `controls_recovery_unknown_status_keeps_objects_and_aborted_ceremony_cleans` control. No candidate source changes.

```rust
#[test] fn own_pending_compromises_survive_other_keys_refresh_and_read_faults() {
    let (mut i,u)=enrolled_identity();
    let p=builders::invalidation(&u.identity,&u.persistent.key_id,invalidation_reason_t::compromised,"p");
    let t=builders::invalidation(&u.identity,&u.temporary.key_id,invalidation_reason_t::compromised,"t");
    i.journal_mut().faults.fail_appends=true;
    assert_eq!(code(i.register_invalidation(Vaultd,1001,&p)),"ipc/restriction_not_durable");
    assert_eq!(code(i.register_invalidation(Vaultd,1001,&t)),"ipc/restriction_not_durable");
    i.journal_mut().faults.unavailable=true;
    assert_eq!(code(i.resolve_key(Contractd,&u.persistent.key_id)),"ipc/storage_unavailable");
    i.journal_mut().faults.unavailable=false;i.journal_mut().faults.fail_appends=false;
    i.register_invalidation(Vaultd,1001,&t).unwrap();i.refresh(1001).unwrap();
    assert_eq!(i.resolve_key(Contractd,&u.persistent.key_id).unwrap().invalidated,Some(invalidation_reason_t::compromised));
    i.register_invalidation(Vaultd,1001,&p).unwrap();
    let mut restarted=identity_service_t::start(i.journal().clone()).unwrap();
    for key in [&u.persistent.key_id,&u.temporary.key_id] {
        assert_eq!(restarted.resolve_key(Contractd,key).unwrap().invalidated,Some(invalidation_reason_t::compromised));
    }
}
#[test] fn own_signing_window_inclusive_and_unreadable_values() {
    use cham3_identityd::index::{check_signing_window,utc_seconds};
    let (mut i,u)=enrolled_identity();let mut r=i.resolve_key(Contractd,&u.temporary.key_id).unwrap();
    let first=utc_seconds(&r.not_before).unwrap();let last=utc_seconds(r.not_after.as_ref().unwrap()).unwrap();
    for (at,ok) in [(first-1,false),(first,true),(last,true),(last+1,false),(i64::MIN,false),(i64::MAX,false)] {
        assert_eq!(check_signing_window(&r,at).is_ok(),ok,"at {at}");
    }
    r.not_after=Some("unreadable".into());assert!(check_signing_window(&r,first).is_err());
    r.not_after=None;r.not_before="unreadable".into();assert!(check_signing_window(&r,first).is_err());
}
#[test] fn own_cancelled_id_retained_until_original_lifetime_ends() {
    let (_,_,mut v,k)=setup("own_cancel");let p=payload(&k);
    let r=v.register_proposal(Contractd,request("cancelled",&p)).unwrap();
    v.cancel_proposal(Alice,&r.proposal_id).unwrap();
    v.register_proposal(Contractd,request("interleave",&p)).unwrap();
    assert_eq!(code(v.register_proposal(Contractd,request("cancelled",&p))),"ipc/duplicate_identifier");
    assert_eq!(code(v.approve_and_sign(Alice,&r.proposal_id,&r.approval_digest,&passphrase(Pass))),"ipc/proposal_unknown");
}

```
