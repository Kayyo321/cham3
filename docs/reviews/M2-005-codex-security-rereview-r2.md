# M2-005 revision 2 and M2-004 revision 3 — independent Codex review

**M2-005: CHANGES_REQUESTED. M2-004: APPROVE for the recorded staged scope.** M2-005 R02 and M2-004-R2-01 are closed at the candidate. R01 and R03 are improved but remain open in failure/restart paths. The new reserve probe also deletes a pre-existing caller-supplied directory even when refusing full-disk mode. Passing qualification and CI do not cover these cases.

- Role/provider: independent REVIEWER / Codex, 2026-10-13. No implementation authorship or implementation edits in this assignment.
- Task/specification: M2-005 revision 2 and M2-004 revision 3, approved PROJECT requirements, ADR 0006 §1, ADR 0007 §6, ADR 0008 §1 and service IPC v1 §3; recorded M2 task staging remains in force.
- Implementation review base: `24a35c1914d787b936d14b37b500226dde51722f`.
- Candidate: `051b4c8aacdeecfabb23625f93dd16615aec39a0`, consisting of implementation `4f7080b091adef38d3c8b1a36fb975db53ba52b7` and harness-only `051b4c8`.
- Docs reviewed: `f575ac3e62fdc28ccbf458b34faaa6a0174eee17`, `4f0fe9392dedfeaa4943370f6a5a0919e4f4669a`, `959524cf52212d9db260457b1c458fe6f95b59f7`. **PASS: every changed path is under docs/**.
- Publication: only this record, branch `review/m2-005-r2-security`, based on `9cf882503848f8c6cc4938a588cc9fc062a9392d` (origin/main). No merge or force-push. Exact publication commit and file digest are in the final handoff, avoiding a self-referential digest.
- Working directories: source `/home/dev/cham3`; clean detached test clone `/tmp/cham3_review_r2/candidate`; independent publication worktree `/tmp/cham3_review_r2/publication`; external tests and logs `/tmp/cham3_review_r2/`.

## Startup, authority and environment

The initial implementation checkout was clean at `87c9cbc9bdeed40fd90b93213c22e5b854502b29`, with no stashes. Fetched the five requested branches, checked out the implementation branch and reset it to its fetched remote head, `959524cf52212d9db260457b1c458fe6f95b59f7`. No unknown work was discarded. The clean candidate clone was made **from that fetched local clone**, then detached at `051b4c8`. Source, candidate and publication were kept separate. The initial generic PROJECT/workflow/role guidance read was verified byte-identical against the updated head with `git diff --exit-code 87c9cbc 959524c -- PROJECT.md docs/engineering/WORKFLOW.md docs/agents/ORCHESTRATION.md docs/agents/ROLES.md`.

Read current AGENTS, PROJECT, workflow, orchestration, roles, testing, security invariants, STATUS, task/handoffs, resolutions, cited ADRs and IPC specification. Read the exact historical reviews with `git show`, rather than relying on the author response. Author evidence was treated as claims and compared with own runs. No owner architecture, risk, sequencing or integration decision is made.

Home computer: Debian GNU/Linux 13.7, Linux `7.1.13+deb13-amd64`, x86_64, hostname `a03-hating-machine`, UID/GID **1001:1001 (dev)**. Docker client/server **26.1.5+dfsg1**, containerd 1.7.24, runc 1.1.15; gh 2.102.0. Network is owner-declared unrestricted; Git fetch, authenticated gh metadata/log/artifact downloads succeeded. Ordinary Docker build networking had DNS failures; host-network build succeeded using the same pinned Dockerfile. This is not proof every network protocol is unrestricted.

`docker version` initially reported socket permission denied; `sudo -n docker version` worked. Every test container used `--user 1001:1001`; none used `--privileged` or added capabilities. Pinned image `cham3-m2-independent-r2` is `sha256:4f4f684d76f7c3c299932846ebe66765204c7e15c9414d77922d15ff561bb327`, linux/amd64; Debian digest `913f6706df59a68922d1dd08f78c2476560a8d367897200a6005b00e5f67c2d5`, Rust/Cargo 1.85.1. Qualification checked exact package, downloaded-deb, crate-source and dependency locks. Candidate clone is UID 1001 owned and clean before/after qualification. No `--record` was used.

## Required changes in M2-005

### M2-005-R2-01 — Medium — failed directory syncs are forgotten across startup/replay

**FAIL; R01 remains open.** Main changed control: `core/registryd/src/journal.rs:213–219`. Related replay control: `:331–334`; journal creation's final directory sync `:388`.

`open` syncs the root's parent or root only if that particular call created root/users. A failed sync leaves the newly created directory visible. Restart sees it as existing and skips the failed parent sync. Enrollment then acknowledges success after syncing only lower directories. Neither syncing the journal nor its child directories explicitly establishes durability of the missing parent entry.

Independent debug and release tests demonstrate both cases:

- New root: fail the parent sync, restart, enroll. The second run syncs new_root, new_root/users and new_root/users/1001, but never retries the root's parent.
- Existing root/new users: fail root sync, restart, enroll. The second run syncs users and users/1001, never root.

Two further same-family cases also fail in both profiles:

- Fail the final user-directory sync when creating the journal. Restart replays the visible journal; identical enrollment returns `already_present: true, journal_seq: 1`, with **no directory sync at all**.
- Remove a synthetic reserve, fail the user-directory sync when `open_journal` recreates it, restart and append a certificate. The append succeeds with **no directory sync** for the new reserve entry.

The original two directory tests pass, as does the new failed-users-sync → restart → enroll UID 1002 then UID 1001 control. The unconditional users sync fixes that particular path; it does not recover every failed creation-directory obligation.

**Required:** reconstruct or repeat the relevant parent/file-entry durability checks before acknowledging success, including retry/restart after root, users, journal and reserve creation-sync failures; alternatively keep affected state unusable. Add these failure sequences to regression and seeded-defect coverage. Do not infer that existence establishes a previous successful sync. ADR 0006 §1's basic creation durability is not deferred with anchors.

Impact is potential loss/unreachability of acknowledged authority history and audit integrity after a subsequent crash. These are deterministic observations of the actual I/O boundary, not physical power-loss proof or demonstrated permission resurrection. Independent source review confirmed the missing compensating syncs.

### M2-005-R2-02 — Low — full reserve length bypasses retry of a failed reserve sync

**FAIL; R03 remains open for failure handling.** `core/registryd/src/journal.rs:365–368` and `:447–459`, with default allocation at `:78–91`.

`allocate` writes all zero bytes, then syncs the reserve. If the sync fails after the writes, the file can have length 1 MiB without completed durability. Creation retry checks only length and skips allocation/sync. Replenishment similarly caches the full metadata length even when `allocate` returned an error; the next call immediately returns success at `:448–449`.

Two independent tests use the real store and journal code, with an allocator that writes every requested byte then returns a reserve-sync EIO. In both profiles:

- Initial creation fails with no journal, but the next `create_journal` succeeds, with exactly **one** allocation call overall.
- Initial non-restrictive append refuses replenishment, but the next append returns **Ok(2)**, again with exactly one allocation call although the reserve-sync fault remains.

The synthetic low-level first record tests the storage API, not enrollment validation. Actual enrollment and admission reach these same storage functions. A normal successful allocation control and a partial allocation/ENOSPC/restart/refill control pass. Accepted writes may already occupy blocks during that boot, so this does **not** establish immediate ENOSPC failure, permission expansion or a lost restriction; Low reflects that narrower proven impact.

**Required:** track successful reserve synchronization separately from observed file length and reconstruct it safely after reopen. An incomplete/error allocation must remain pending until a successful sync or safely completed allocation occurs; do not admit based solely on length. Cover full-write-then-sync failure at creation, refill and reopen, and keep restrictive operations available as designed.

### M2-005-R2-03 — Medium — reserve probe recursively deletes an existing root on refusal

**FAIL.** `core/registryd/src/bin/cham3_reserve_probe.rs:300–302`; supporting `open` at `:133–140` and `run` at `:283–289`.

The probe accepts an already-existing root (`create_dir_all`), then `main` unconditionally calls `remove_dir_all(root)`, whether setup succeeded, setup failed, the mode was invalid, or full-disk mode refused the filesystem. No ownership-of-creation flag limits cleanup. This can delete unrelated caller-accessible data or an existing test store. The CI wrapper's fresh private path avoids the defect in CI but does not make the binary safe.

In a disposable container, create `/tmp/existing_probe_data/sentinel` containing only synthetic text, then invoke full-disk mode at that root on overlay. **Both debug and release** report `refusing_to_fill / is overlay, not tmpfs`, exit 3, and leave `sentinel=deleted`. No host data was used or deleted. Both profiles also correctly refuse a disposable 32 MiB tmpfs before filling it.

**Required:** refuse pre-existing roots before mutation, or create an exclusive private child and clean up only paths actually created/owned by this invocation. Validate mode and eligible backing mount before setup where practicable. Add preservation tests for pre-existing roots, setup failures and refusal paths. The fill limit itself passes; cleanup safety does not. This is a qualification-tool data-loss defect, not a demonstrated lower-privileged exploit.

## Findings closed and retained decisions

| Item | Status | Evidence and assessment |
|---|---|---|
| M2-005 R01 | FAIL overall | Original two tests PASS; users sync is unconditional on each creation attempt. Four new startup/replay cases above FAIL; first enrollments of two UIDs and failed users sync followed by restart PASS. |
| M2-005 R02 | PASS | Ended-session set retained through replay, copied for live grants into view, and checked before liveness. Original ended-session reproduction PASS; own network/repeated-end/merge/restore/replay matrix PASS. |
| R02 error vocabulary | PASS | `duplicate_identifier` is accepted: ADR 0007 §6 makes sessions single-use and terminal; IPC/wire vocabulary already includes it. The check follows grant/request uniqueness and creates no new wire error. A repeated narrowing session_ended record remains idempotent in effect. |
| M2-005 R03 normal/full-disk paths | PASS | Physical 1 MiB allocation; ENOSPC/EDQUOT-only release on restrictive append after truncation; torn-write test; two restrictive appends; ordinary append refuses depleted reserve; partial allocation across restart refuses until refill; path/mode controls; real 2 MiB tmpfs debug/release PASS. Overall FAIL for R2-02; probe safety additionally R2-03. |
| Reserve lazy replenishment/limits | PASS for staged choice | Refill before the next non-restrictive append is acceptable; depletion cannot grant authority. Compression/deduplication of zeros is a stated unsupported capacity assumption, not a universal guarantee. Missing pre-M2-006 status fields is explicitly staged. Neither limit waives failed-sync handling. Owner/group bits behave like journal checks: 0644/0660/0777 rejected; 0600/0700 accepted (owner execute creates no cross-user access). |
| M2-004-R2-01 | PASS | All 12 prior re-review tests pass in each profile; quotas now observe 32 and 256. Every pending entry reserves its retained slot. Unit approval-transition and cancellation controls pass; no marker eviction, ID revival, or stranded admitted workflow. |

The session matrix inserts a repeated session end, then rejects a fresh Tor grant bound to it as `admission/duplicate_identifier`. Merge and restore carrying session additions are refused earlier as `intrinsic/commit_type_rule` because those types allow persistent additions only; an empty legitimate merge succeeds without importing the ended network grant. A correctly chained journal containing the rejected later grant makes the user unusable on replay. Published 11 layer-7 scenarios/59 steps/105 comparisons and 18 admission-invalid vectors are exercised by qualification; ECR totals also include layer-6 scenarios.

## M2-004 revision-2 records and approval transition

The records describe the **same ca40c62 candidate**, not successive fixes:

1. `review/m2-004-r2-security` @ `fd6de588bbee575d936fde448e823bff08f44e7a`, `M2-004-codex-security-rereview-r2.md`: CHANGES_REQUESTED, with reproducible M2-004-R2-01.
2. `review/m2-005-security` @ `f9f3b828fcd117ebcaa4179fa6ae05e782381db1`, `M2-005-codex-security-review.md`: later APPROVE for staged M2-004, but it neither mentions nor disposes of that finding and did not execute those two batch quota tests.

**The earlier CHANGES_REQUESTED stands for ca40c62.** A later incomplete review cannot close an unaddressed reproducible defect merely by date. This is reconciliation of review evidence, not an owner architecture decision. The implementer's refusal to infer approval was correct. This review now independently closes M2-004-R2-01 **at 051b4c8** and changes the recommendation to **APPROVE for revision 3's recorded staged scope**. It does not retroactively approve revision 2 or mark the milestone DONE.

| Earlier assessment | Status at candidate | Basis and preserved limitations |
|---|---|---|
| M2-004 R01 pending compromise | PASS | Prior reproductions, multi-key weaker/durable/restart test, qualification; identity source unchanged in this candidate range. |
| M2-004 R02 signing window | PASS | Original expiry and extra boundary/clock tests PASS; before/after-derivation checks unchanged. |
| M2-004 R03 final authority/scope | PASS | Original scheduled deletion and extra missing-object/unavailable-authority/recovery controls PASS; cross-service atomicity remains future work. |
| M2-004 R04 proposal retention | PASS after this fix | Original reuse controls plus two quota batches and unit approval transition PASS. |
| M2-004 R05 recovery erasure | PASS | Original definitive-error and extra status/abort/retry cases PASS. |
| I01 | PASS documentation interpretation | Original stronger assertion still FAILs: completing an existing signed/durable recovery bundle need not perform fresh passphrase authentication. No fresh key/signature is made; enrollment retry separately requires correct passphrase and passes. |
| M2-004 boundaries, roles, point validation, layer 6, key objects/AAD, KDF, store, custody | PASS within previous limits | Source unchanged except proposal count; 38-check vault oracle, exact pinned source/feature checks, original 17 and extra 12 evidence; no new crypto/dependency. Real IPC and full-system enforcement remain NOT_RUN. |
| M2-005 format/chain/budget/path validation and fail-closed replay | PASS within previous limits | Full unit corpus plus own rechained-invalid-metadata/unknown-lifecycle tests; creation durability remains R01 rather than a passing claim. |
| M2-005 layer7 placement/delta/restore/subject/uniqueness/network slot | PASS | Frozen inputs plus new session scenario, seeded defect detection and own 32-round sibling race: exactly one winner; correct replay. |
| M2-005 admission snapshots, roles and locking | PASS | Per-user check/write/sync/index order retained; identity append role and key-window/epoch checks unchanged; source and qualification. |
| M2-005 evaluation/contractd | PASS apart from previously isolated durability concerns | Session fix, mandatory denial, runtime binding, origin compromise across persistent restatement, attestation boundaries/unknown clock, policy epoch and unsupported delegation remain covered; contractd source unchanged. |
| Author decisions and carried obligations | PASS within staged scope | Earlier accepted choices remain; R01/R03 never waived. M2-006 ledger/anchor/recovery/lifecycle writers/trusted time/status and M2-007 transport/serialization remain required. Admission duplicate request IDs is already enforced by M2-005. |

Earlier **passing** items remain intact; this does not upgrade R01/R03 or approve the new unsafe probe behavior. In-process fixtures are not OS isolation evidence. Miri, sanitizers, coverage-guided fuzzing, actual power-loss testing, deployed sessions/IPC and full system enforcement are **NOT_RUN**, with the first three unavailable on the pinned toolchain and the remainder outside this staged implementation review.

## Scope and immutable-input checks

**PASS:** no dependency, Cargo.lock, architecture/ADR, specification, key-object format or cryptographic-provider change. `registryd/Cargo.toml` adds only the qualification binary. `051b4c8` changes only the seeded-defect harness. The three later commits change only docs/.

The exact 23 changed paths were reviewed (eight production/CI items from the plugin inventory, plus every changed oracle/vector/test/harness/documentation path from Git). Removing exactly the **80 added lines** of vectors.json reproduces the entire previous file **byte for byte**. The only added scenario is `session_ended_refuses_later_grants_of_that_session`; it contributes five signed objects to the differential corpus.

**FAIL, documentation precision only:** the handoff's enumerated scope is not literally exhaustive: it omits `core/contract_format/tests/ecr_vectors.rs` (129→134 signed-object count) and does not explicitly name `tests/m2/vectors/ecr_v1/vectors_readme.md`. Both are necessary matching test/documentation updates, inspected and accepted; no hidden implementation or architecture expansion. Correct the list in the next handoff. Other 21 paths match the combined handoffs/resolution lists.

## Own execution evidence

Logs below are relative to `/tmp/cham3_review_r2`; scripts/test sources are outside the candidate. Exact commands follow. All final required infrastructure gates are executable; no final BLOCKED gate is claimed.

| Check | Result | Exact evidence / limitations |
|---|---|---|
| Pinned build | PASS | `docker_build_host.log`; same pinned inputs, host build network. Initial ordinary network build FAIL/123 after DNS errors, retained in `docker_build.log`. |
| Qualification initial | FAIL environment | `qualification.log`: Valgrind fatal private-file creation at descriptor limit 1073741804; same cause in two Massif harness tests, 135/137. This result was not relabeled PASS. |
| Qualification rerun | PASS/0 | `qualification_nofile.log`; container-only `--ulimit nofile=1024:1024`. All required checks PASS, including Valgrind probe and secure-memory 10 tests/0 errors. No source/configuration weakening. |
| Rust unit | PASS | **166 debug +166 release**, same named inventory as CI, zero ignored required tests. `candidate/out/m2/runs/unit_20261013T161805Z_1844/`. |
| Provider | PASS | 7 probe checks/7 independent published non-Argon2 checks; ordinary-input Argon2 output agrees. libargon2 secret/AD RFC interface remains explicitly NOT_CHECKED, not a new claim. |
| ECR oracle | PASS | **19 valid/96 invalid/18 admission;23 scenarios/352 steps**. |
| Format differential | PASS | **5955 cases/0 mismatches**, corpus SHA-256 `0ad8bc2b1e8d39224de699db64aa45ca7adcf70117c555f35b57c537471d395e`. |
| Vault-object oracle | PASS | **38 checks**. |
| Harness | PASS | **137**, all named tests equal CI; includes **21 identity/vault** and **36 registry/policy** seeded defects, and two reserve probe tests. Successful run 277.719 s. |
| Static checks | PASS/0 | `candidate/out/static_checks_review.log`: shellcheck 26, Python 69, YAML 5, secret scan PASS. |
| Static tests | PASS/0 | `candidate/out/static_tests_review.log`: **82**, 24.907s. |
| Protection positive | PASS/0 own run | `protection.log`: memlock 1073741824, required 805306368, derivation 269 ms, dumpable 0, core 0, unlocked mappings 0, dontdump 4, `protection_verified`. CI also PASS. |
| Full-filesystem script | PASS/0 own run | `reserve_full.log`, debug/release **37 chained records**, restrictive success seq 35; depleted ordinary refusal; second restriction seq 36; refill append seq 37. Reserve 1048576 bytes, filler 1040384 bytes. |
| Original nine M2-005 | PASS | `adversarial005/own_debug.log`, `own_release.log`: **9/0** each, original added source unchanged, baseline fixture modules recovered at b06d7cb to avoid new-name collisions. |
| Original filesystem_full_reserve | PASS | `own_full_debug.log`, `own_full_release.log`: **1/0** each on fresh 2MiB tmpfs. |
| New first edge batch | FAIL/101 | `adversarial005/edge_debug.log`, `edge_release.log`: **1 PASS/4 FAIL** each; two startup-directory and two reserve-sync failures, UID-restart/two-user control PASS. |
| New session/partial/mode controls | PASS | `adversarial005/controls_debug.log`, `controls_release.log`: **3/0** each. |
| New creation-directory restart cases | FAIL/101 | `adversarial005/directory_debug.log`, `directory_release.log`: **0/2** each, journal and recreated-reserve entries. |
| Probe preservation / refusal matrix | FAIL preservation; PASS refusal | `probe_refusal_matrix.log`: existing sentinel deleted in debug/release; ordinary overlay and 32MiB tmpfs refused with exit 3. |
| Original seventeen M2-004 | FAIL expected assertion only | `adversarial004/original_debug.log`, `original_release.log`: **16 PASS/1 I01 FAIL** each, no other failure. |
| Prior twelve M2-004 re-review tests | PASS | `adversarial004/r2_debug_verified.log`, `r2_release_verified.log`: **12/0** each, two quota counts 32/256 plus ten other tests. |
| Review-harness setup attempts | NOT evidence of product defects | First new-edge compilation failed due reviewer helper expecting Arc; corrected only external test. First r2 run selected zero tests due same-name Cargo packages sharing a target; detected, rejected as evidence, rerun with separate `r2_target`, all 12 executed. |
| Source cleanliness | PASS | Final `git status --porcelain` empty in both source and detached candidate; only this record in publication. |

Original M2-004 reproduction digests match both prior records: review.rs `922dd57b9ba92ce7b79517c6a3f890a1129626dc61766af6b814ee13df15bea4`; Cargo.toml `82224a3c0e00b6cc3ac9c6b4ceaebc810c7d1912ff32366ecfdc8941ba961004`; concatenated original+12 `d619b29f72eea7f71600258a5e41c3f1f0609809c963f5cd690b912b51ea0b20`.

## Commands

Commands ran from `/home/dev/cham3` unless a cwd is stated. Shell loop exit codes are not used to infer individual test success; each named log's executed-test result is checked. All fixtures contain published synthetic keys/passphrases only.

```sh
git status --porcelain
git stash list
git fetch origin codex/m2-cryptographic-contract-core review/m2-005-security review/m2-004-r2-security review/m2-004-security main
git checkout codex/m2-cryptographic-contract-core
git reset --hard origin/codex/m2-cryptographic-contract-core
git rev-parse HEAD
git log --oneline -6
git clone --no-hardlinks /home/dev/cham3 /tmp/cham3_review_r2/candidate
git -C /tmp/cham3_review_r2/candidate checkout --detach 051b4c8aacdeecfabb23625f93dd16615aec39a0
git show f9f3b828fcd117ebcaa4179fa6ae05e782381db1:docs/reviews/M2-005-codex-security-review.md
git show fd6de588bbee575d936fde448e823bff08f44e7a:docs/reviews/M2-004-codex-security-rereview-r2.md
git show f2d82b7a663c1579582bc5c88fe2832a6f97e061:docs/reviews/M2-004-codex-security-review.md
git diff --stat 24a35c1 051b4c8
git diff --check 24a35c1 051b4c8
git log --format='%H' --name-only 051b4c8..959524c
git diff --exit-code 24a35c1 051b4c8 -- Cargo.lock Cargo.toml .cargo docs/decisions docs/specs core/vaultd/src/key_object.rs core/vaultd/src/kdf.rs core/vaultd/src/store.rs core/native core/identityd core/contractd core/contract_format/src

# cwd /tmp/cham3_review_r2/candidate
sudo -n docker build -f build/m2/dev_container.dockerfile -t cham3-m2-independent-r2 build/m2
sudo -n docker build --network host -f build/m2/dev_container.dockerfile -t cham3-m2-independent-r2 build/m2
sudo -n docker image inspect cham3-m2-independent-r2 --format '{{.Id}} {{.Os}}/{{.Architecture}}'
# Initial qualification omitted only --ulimit nofile from this command.
sudo -n docker run --rm --user "$(id -u):$(id -g)" --ulimit nofile=1024:1024 \
  -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
  -v /tmp/cham3_review_r2/candidate:/work -w /work cham3-m2-independent-r2 \
  bash build/scripts/qualify_m2_rust.sh
sudo -n docker run --rm --user "$(id -u):$(id -g)" --ulimit memlock=1073741824:1073741824 \
  -e HOME=/tmp/cham3_home -v /tmp/cham3_review_r2/candidate:/work -w /work \
  cham3-m2-independent-r2 sh tests/m2/protection/check_vault_protection.sh out/m2/protection
sudo -n docker run --rm --user "$(id -u):$(id -g)" \
  --tmpfs "/full-disk:rw,size=2m,uid=$(id -u),gid=$(id -g),mode=0700" \
  -e HOME=/tmp/cham3_home -v /tmp/cham3_review_r2/candidate:/work -w /work \
  cham3-m2-independent-r2 sh tests/m2/storage/check_reserve_full_disk.sh out/m2/storage
sudo -n docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp/cham3_home \
  -v /tmp/cham3_review_r2/candidate:/work -w /work cham3-m2-independent-r2 \
  bash -c 'bash build/scripts/run_static_checks.sh > /work/out/static_checks_review.log 2>&1; sh tests/run_tests.sh static > /work/out/static_tests_review.log 2>&1'
```

External test runner: for M2-005, use each filter `::own_`, `::edge_` (first batch at that stage), `::edge_control_`, `::edge_directory_`; both default and `--release`. The filesystem test uses its exact filter and the added 2MiB tmpfs. The initial original-nine debug invocation used `--offline` without `--locked` to generate its external lock. Subsequent invocations were locked. Candidate locks never changed.

```sh
sudo -n docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp/cham3_home \
  -e CARGO_TARGET_DIR=/evidence/target -v /tmp/cham3_review_r2/candidate:/work:ro \
  -v /tmp/cham3_review_r2/adversarial005:/evidence -w /evidence cham3-m2-independent-r2 \
  cargo test --manifest-path core/reviewer/Cargo.toml --locked --offline ::own_ -- --nocapture --test-threads=1
# Repeat with --release before --locked. Other filters use precisely the same command.
# filesystem_full_reserve uses --tmpfs /full-disk:rw,size=2m,uid=1001,gid=1001,mode=0700.

# M2-004 original17: same Docker user/home and candidate:ro; evidence=adversarial004,
# CARGO_TARGET_DIR=/evidence/target, manifest core/reviewer/Cargo.toml, no filter.
# Initial lock generation --offline; release has same original immutable source.
# M2-004 additional12 (separate target to avoid artifact collision):
sudo -n docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp/cham3_home \
  -e CARGO_TARGET_DIR=/evidence/r2_target -v /tmp/cham3_review_r2/candidate:/work:ro \
  -v /tmp/cham3_review_r2/adversarial004:/evidence -w /evidence cham3-m2-independent-r2 \
  cargo test --manifest-path core/reviewer_r2/Cargo.toml --locked --offline r2_ -- --nocapture --test-threads=1
# Repeat with --release.
```

Probe preservation/refusal command (the only data is disposable synthetic data):

```sh
sudo -n docker run --rm --user "$(id -u):$(id -g)" \
  --tmpfs "/large:rw,size=32m,uid=$(id -u),gid=$(id -g),mode=0700" \
  -v /tmp/cham3_review_r2/candidate:/work:ro -w /work cham3-m2-independent-r2 bash -c '
for profile in debug release; do
  mkdir -m 700 /tmp/existing_probe_data
  echo synthetic_keep > /tmp/existing_probe_data/sentinel
  /work/out/m2/storage/target/$profile/cham3_reserve_probe /tmp/existing_probe_data full-disk
  result=$?
  echo profile=$profile probe_exit=$result
  if test -f /tmp/existing_probe_data/sentinel; then echo sentinel=preserved; else echo sentinel=deleted; fi
  /work/out/m2/storage/target/$profile/cham3_reserve_probe /large/probe_root full-disk
  echo large_tmpfs_exit=$?
done'
```

## CI comparison

**PASS**: m2 **38422383429**, m1 **38422383455**, m0 **38422383430** all completed successfully at exact head `051b4c8aacdeecfabb23625f93dd16615aec39a0`; every job and reported step succeeded. Retrieved metadata and logs with `gh run view`, and downloaded artifact with the requested command. The m2 protection and full-filesystem steps both PASS, independently matched by own runs. M0/M1 full-image/desktop/privileged suites were **NOT_RUN locally**; comparison uses their exact-head CI job/step/log evidence.

```sh
gh run view 38422383429 --repo Kayyo321/cham3 --json headSha,status,conclusion,jobs,url
gh run view 38422383455 --repo Kayyo321/cham3 --json headSha,status,conclusion,jobs,url
gh run view 38422383430 --repo Kayyo321/cham3 --json headSha,status,conclusion,jobs,url
gh run download 38422383429 -n m2_qualification_amd64 --repo Kayyo321/cham3 --dir /tmp/cham3_review_r2/ci_artifact
gh api repos/Kayyo321/cham3/actions/runs/38422383429/artifacts
gh api repos/Kayyo321/cham3/actions/artifacts/11827908447/zip > /tmp/cham3_review_r2/ci_artifact.zip
sha256sum /tmp/cham3_review_r2/ci_artifact.zip
gh run view 38422383429 --repo Kayyo321/cham3 --log
gh run view 38422383455 --repo Kayyo321/cham3 --log
gh run view 38422383430 --repo Kayyo321/cham3 --log
```

Computed ZIP digest equals advertised/requested **`f05dedc6e999261dddc44ffcf780a97ae7b0fa1ea61b26ddd57ab41fe383a926`**. Comparison script `ci_comparison.txt` confirms both 166-test inventories, provider outputs, all differential counts/outcomes/corpus digest, ECR and vault oracle logs, qualification/unit/harness summary files. Both reserve reports are identical after removing only the mount line (UID/GID differ). Timing, build/probe digests and runtime paths are environment-dependent and are not assumed identical. `vectors_comparison.txt` confirms raw byte preservation independently.

## Codex Security plugin

**PASS: ran and completed**, exact skill **codex-security:security-diff-scan**, installed version **0.1.32**, path `/home/dev/.codex/plugins/cache/openai-curated-remote/codex-security/0.1.32/skills/security-diff-scan/SKILL.md`.

- Candidate scan **10df7f5a-5562-4e52-9e26-010c00204d63**, exact `24a35c1..051b4c8`. Report `/home/dev/.codex/state/plugins/codex-security/scans/cham3/051b4c8aacdeecfabb23625f93dd16615aec39a0_20261013T161052Z_0qpa0f5v/report.md`; canonical coverage/findings/manifest and SARIF alongside it. Full 23-path manual inventory supplements the plugin's eight production/CI inventory entries.
- Prerequisite scan **885efc96-936d-48b1-9209-20611608d36c**, exact `bb3f1bad77080da34069af62c8d2cc390f2fcf7e..ca40c62ed7a5fc29ba8910fef514f3ce29ea4fba`. Report `/home/dev/.codex/state/plugins/codex-security/scans/cham3/ca40c62ed7a5fc29ba8910fef514f3ce29ea4fba_20261013T161606Z_q4x4zvq_/report.md`. Static review of all three changed production and six test/harness files; original ca40c62 dynamic qualification NOT_RUN in this assignment. Parent independently runs all tests at the new candidate.

Preflight command (exit 0, ready; no unmet capability):

```sh
python3 /home/dev/.codex/plugins/cache/openai-curated-remote/codex-security/0.1.32/scripts/config_preflight.py --profile security_diff_scan --cwd /home/dev/cham3 --runtime-check delegation_available=true --runtime-check goal_tools_available=true
# Prerequisite worker uses delegation_available=false (no further workers), same ready result with documented warning.
```

Skill-authorized workers performed preflight, source-backed architecture and prerequisite scan; the architecture worker separately challenged the two failure findings. The main reviewer wrote/tested the new reproductions. These activities do not constitute independent approval of implementation fixes.

Executed workbench tool sequence for each exact-range scan: `start_codex_security_prompt_only_scan(mode=diff,scope=.,diffTarget=range)`; preflight/progress; per-scan threat model `record_codex_security_scan_draft(complete=false)`; `prepare_codex_security_review_items` and full `list_codex_security_review_items`; source/diff inspection and candidate recording; `list_codex_security_candidates`; `record_codex_security_candidate_validations`; `record_candidate_attack_paths`; final `record_codex_security_scan_draft(complete=true)`; `complete_codex_security_scan` once successfully; `get_codex_security_completed_scan`. Candidate dynamic commands are the recorded external Rust/Docker invocations above. Prerequisite validation was static. Supplemental evidence used the plugin artifact tool.

One artifact-save attempt returned **`Scan output parent must not be group- or world-writable without the sticky bit.`** `/home/dev/.codex` was 0775. The owner explicitly authorized `chmod g-w /home/dev/.codex`; it became 0755 and the save succeeded. No persistent model/config setting was changed. This is a resolved attempt, not a final plugin blocker. Daybreak advisory was `not_granted`; disclosed to the owner, non-gating. Both completed reports were readable.

**Every plugin candidate/disposition:**

- `candidate-d86698c2c9c5afb5`: confirmed failed-directory-sync retry defect; validation reportable/high confidence, final attack-path policy **ignore** because no realistic lower-privileged control of the sync failure or demonstrated exploitation is established. Remains ordinary review R2-01, Medium.
- `candidate-1ec74d4823de566b`: confirmed reserve failed-sync retry defect; same validation and final exploit-policy **ignore**, without erasing the correctness/ADR obligation. Remains ordinary review R2-02, Low.
- Prerequisite `candidate-c6d3e75fd4e28dff`: statically confirmed historical retained-ID quota defect at ca40c62; final exploit-policy **ignore** because metadata overshoot is finite, payloads released, lifetime bounded, and harmful exhaustion/unauthorized signing unproven. Ordinary historical CHANGES_REQUESTED remains valid and is closed only by this candidate's tested fix.
- Probe pre-existing-root deletion is explicitly retained in candidate scan coverage as a confirmed tool-correctness defect, not a lower-privileged exploit candidate. It remains ordinary review R2-03, Medium.

Both scans therefore have **zero final exploit-oriented security findings**, not zero bugs and not an approval of M2-005. No plugin requirement is waived for final M2-008. Candidate completion reported token telemetry: total 13447446, input 13404103 (cached 12916224), output 43343, reasoning 4874, four threads; this is the workbench's rollout-accounting measurement, not a billing claim. Prerequisite worker's separately measured usage was unavailable.

## Handoff

M2-005 returns to **CHANGES_REQUESTED**: implementer should fix R2-01/R2-02/R2-03 and scope-list precision, add discriminating regression/seeded coverage, provide exact commits and repeat affected qualification for independent re-review. M2-004 revision 3 is **APPROVE for staged scope**; coordinator/integrator retains normal gates and the M2-006/007 obligations. No merge, release, owner decision, M2 completion or deployed security claim is made. Main implementation checkout remains clean at 959524c; clean test clone remains detached at 051b4c8. Publication contains only this record.

## Reproduction appendix

Reconstruct the original M2-005 external package from f9f3b82's portable appendix, using b06d7cb fixture modules (as in that review), the current candidate's `/work` imports and vectors, and its unchanged manifest. Append the following three Rust blocks to that external `registry.rs`. Run `::edge_` to execute all 10 new tests together (expected 4 PASS / 6 FAIL), or the three recorded batch filters at their corresponding stage. The source is reviewer-only; never append it to the implementation.

### review_extra.rs

```rust
// Independent candidate-only edge tests. Synthetic fixtures; no implementation edits.
#[derive(Clone)]
struct edge_sync_t {
    calls: Arc<Mutex<Vec<PathBuf>>>,
    fail_path: PathBuf,
    fail_once: Arc<std::sync::atomic::AtomicBool>,
}
impl journal_io_t for edge_sync_t {
    fn write_all(&self, file: &mut fs::File, bytes: &[u8]) -> io::Result<()> { system_io_t.write_all(file, bytes) }
    fn sync_data(&self, file: &fs::File) -> io::Result<()> { system_io_t.sync_data(file) }
    fn sync_directory(&self, path: &Path) -> io::Result<()> {
        self.calls.lock().unwrap().push(path.to_owned());
        if path == self.fail_path && self.fail_once.swap(false, Ordering::SeqCst) {
            return Err(io::Error::other("independent directory sync EIO"));
        }
        system_io_t.sync_directory(path)
    }
}
fn startup_retry_case(new_root: bool) {
    let parent = temp_root(if new_root {"edge_root_parent"} else {"edge_users_parent"});
    let root = if new_root { parent.join("new_root") } else { parent.clone() };
    let owner = owner_of(&parent);
    let required = if new_root { parent.clone() } else { root.clone() };
    let calls = Arc::new(Mutex::new(Vec::new()));
    let spy = edge_sync_t { calls: calls.clone(), fail_path: required.clone(), fail_once: Arc::new(std::sync::atomic::AtomicBool::new(true)) };
    let first = registry_t::open(&root, owner, registry_config_t::default(), Box::new(spy.clone()), Box::new(fixed_clock_t));
    assert!(first.is_err());
    calls.lock().unwrap().clear();
    let registry = Arc::new(registry_t::open(&root, owner, registry_config_t::default(), Box::new(spy), Box::new(fixed_clock_t)).unwrap());
    let user = test_user(1001);
    enroll(&registry, &user);
    println!("required retry sync={required:?}; observed={:?}", calls.lock().unwrap());
    assert!(calls.lock().unwrap().contains(&required), "successful restart/enrollment skipped the earlier failed parent fsync");
}
#[test] fn edge_failed_root_parent_sync_survives_restart() { startup_retry_case(true); }
#[test] fn edge_failed_users_parent_sync_survives_restart() { startup_retry_case(false); }

#[test]
fn edge_failed_uid_sync_restart_and_two_users() {
    let root = temp_root("edge_uid_retry");
    let calls = Arc::new(Mutex::new(Vec::new()));
    let spy = edge_sync_t { calls:calls.clone(), fail_path:root.join("users"), fail_once:Arc::new(std::sync::atomic::AtomicBool::new(true)) };
    let registry = open_with(&root,registry_config_t::default(),Box::new(spy.clone()));
    let user = test_user(1001);
    assert_eq!(code(registry.append_identity(Identityd,user.uid,object_kind_t::identity_enrollment,&user.enrollment)), "ipc/storage_unavailable");
    drop(registry);
    calls.lock().unwrap().clear();
    let registry = open_with(&root,registry_config_t::default(),Box::new(spy));
    enroll(&registry,&test_user(1002));
    enroll(&registry,&user);
    assert_eq!(calls.lock().unwrap().iter().filter(|p| *p == &root.join("users")).count(),2);
    drop(registry);
    assert!(open(&root).unusable_users().unwrap().is_empty());
}

#[derive(Clone)]
struct allocation_sync_error_t { calls:Arc<AtomicUsize> }
impl journal_io_t for allocation_sync_error_t {
    fn write_all(&self,file:&mut fs::File,bytes:&[u8])->io::Result<()> {system_io_t.write_all(file,bytes)}
    fn sync_data(&self,file:&fs::File)->io::Result<()> {system_io_t.sync_data(file)}
    fn sync_directory(&self,path:&Path)->io::Result<()> {system_io_t.sync_directory(path)}
    fn allocate(&self,file:&mut fs::File,from:u64,to:u64)->io::Result<()> {
        use std::io::Write;
        self.calls.fetch_add(1,Ordering::SeqCst);
        file.write_all(&vec![0; (to-from) as usize])?;
        // All writes accepted, but their durability operation fails.
        Err(io::Error::other("independent reserve fsync EIO"))
    }
}
#[test]
fn edge_reserve_creation_sync_error_is_not_forgotten() {
    use cham3_registryd::journal::journal_store_t;
    let root=temp_root("edge_create_reserve");
    let store=journal_store_t::open(&root,owner_of(&root),&system_io_t).unwrap();
    let calls=Arc::new(AtomicUsize::new(0));
    let bad=allocation_sync_error_t{calls:calls.clone()};
    let first=store.create_journal(1001,&[4;32],&record(1,record_body_t::lifecycle(lifecycle_record_t::session_ended{session_id:[5;16]})),&bad);
    assert!(first.is_err());
    assert!(!journal_path(&root,1001).exists());
    let retry=store.create_journal(1001,&[4;32],&record(1,record_body_t::lifecycle(lifecycle_record_t::session_ended{session_id:[5;16]})),&bad);
    println!("creation retry={retry:?}; allocate calls={}", calls.load(Ordering::SeqCst));
    assert!(retry.is_err(),"acknowledged a journal after failed reserve sync, without retrying that sync");
}
#[test]
fn edge_reserve_replenish_sync_error_is_not_forgotten() {
    use cham3_registryd::journal::{journal_store_t,ReserveFileName};
    let root=temp_root("edge_replenish_reserve");
    let store=journal_store_t::open(&root,owner_of(&root),&system_io_t).unwrap();
    let bytes=record(1,record_body_t::lifecycle(lifecycle_record_t::session_ended{session_id:[5;16]}));
    drop(store.create_journal(1001,&[4;32],&bytes,&system_io_t).unwrap());
    fs::OpenOptions::new().write(true).open(root.join("users/1001").join(ReserveFileName)).unwrap().set_len(0).unwrap();
    let data=fs::read(journal_path(&root,1001)).unwrap();
    let parsed=parse_journal(&data,1001).unwrap();
    let mut journal=store.open_journal(1001,&parsed,data.len() as u64,&system_io_t).unwrap();
    let calls=Arc::new(AtomicUsize::new(0));
    let bad=allocation_sync_error_t{calls:calls.clone()};
    let next=record(2,record_body_t::lifecycle(lifecycle_record_t::session_ended{session_id:[5;16]}));
    assert!(journal.append(&next,false,&bad).is_err());
    let retry=journal.append(&next,false,&bad);
    println!("replenish retry={retry:?}; allocate calls={}",calls.load(Ordering::SeqCst));
    assert!(retry.is_err(),"admission accepted although reserve sync keeps failing");
}
```

### review_controls.rs

```rust
// Additional independent controls.
#[test]
fn edge_control_ended_network_merge_restore_and_repeated_end() {
    let root=temp_root("edge_session_matrix");let user=test_user(1001);let application=app("edge_session");
    let registry=open(&root);enroll(&registry,&user);
    let genesis=draft(&user,&application,commit_type_t::genesis,"edge genesis").sign(&user);admit(&registry,&user,&genesis).unwrap();
    let old=network_grant("edge direct",user.uid,"direct",session_lifetime("edge ended"));
    let session=old.lifetime.session_id.unwrap();
    let branch=draft(&user,&application,commit_type_t::temporary_grant,"edge branch").on(&genesis).temporary_authority().add(vec![old]).sign(&user);
    admit(&registry,&user,&branch).unwrap();drop(registry);
    let mut records=records_of(&root,user.uid);
    for _ in 0..2 {records.push(record(records.len() as u64+1,record_body_t::lifecycle(lifecycle_record_t::session_ended{session_id:session})));}
    write_chained(&root,user.uid,&user.identity.key_id,&records);
    let registry=open(&root);assert!(registry.unusable_users().unwrap().is_empty());
    let fresh=network_grant("edge tor",user.uid,"tor",session_lifetime("edge ended"));
    let again=draft(&user,&application,commit_type_t::temporary_grant,"edge again").on(&genesis).temporary_authority().add(vec![fresh.clone()]).sign(&user);
    assert_eq!(code(admit(&registry,&user,&again)),"admission/duplicate_identifier");
    let merge=draft(&user,&application,commit_type_t::merge,"edge merge").merge(&genesis,&[&branch]).add(vec![fresh.clone()]).sign(&user);
    let restore=draft(&user,&application,commit_type_t::restore,"edge restore").on(&genesis).restore_from(&genesis).add(vec![fresh]).sign(&user);
    for candidate in [&merge,&restore] {assert_eq!(code(admit(&registry,&user,candidate)),"intrinsic/commit_type_rule");}
    let clean_merge=draft(&user,&application,commit_type_t::merge,"edge empty merge").merge(&genesis,&[&branch]).sign(&user);
    admit(&registry,&user,&clean_merge).unwrap();
    assert!(registry.lineage_head(Policyd,user.uid,&genesis.payload.lineage_id).unwrap().unwrap().occupying_network.is_empty());
    drop(registry);
    // Restore the pre-merge journal, then insert a correctly chained ended-session grant.
    records.push(record(records.len() as u64+1,record_body_t::commit_admitted{commit_id:again.commit_id,lineage_id:again.payload.lineage_id,placement:placement_t::temporary,envelope:again.envelope}));
    write_chained(&root,user.uid,&user.identity.key_id,&records);
    assert_eq!(open(&root).unusable_users().unwrap().len(),1);
}
struct partial_reserve_t;
impl journal_io_t for partial_reserve_t {
    fn write_all(&self,file:&mut fs::File,bytes:&[u8])->io::Result<()> {system_io_t.write_all(file,bytes)}
    fn sync_data(&self,file:&fs::File)->io::Result<()> {system_io_t.sync_data(file)}
    fn sync_directory(&self,path:&Path)->io::Result<()> {system_io_t.sync_directory(path)}
    fn allocate(&self,file:&mut fs::File,from:u64,to:u64)->io::Result<()> {
        use std::io::Write;
        file.write_all(&vec![0;usize::try_from((to-from).min(4096)).unwrap()])?;
        Err(io::Error::from_raw_os_error(28))
    }
}
#[test]
fn edge_control_partial_reserve_refused_across_restart_then_refilled() {
    use cham3_registryd::journal::{journal_store_t,ReserveBytes,ReserveFileName};
    let root=temp_root("edge_partial");let user=test_user(1001);
    let registry=open(&root);enroll(&registry,&user);drop(registry);
    let reserve=root.join("users/1001").join(ReserveFileName);
    fs::OpenOptions::new().write(true).open(&reserve).unwrap().set_len(0).unwrap();
    let data=fs::read(journal_path(&root,1001)).unwrap();let parsed=parse_journal(&data,1001).unwrap();
    let next=record(parsed.records.len() as u64+1,record_body_t::lifecycle(lifecycle_record_t::session_ended{session_id:[7;16]}));
    for expected in [4096,8192] {
        let store=journal_store_t::open(&root,owner_of(&root),&system_io_t).unwrap();
        let mut journal=store.open_journal(1001,&parsed,data.len() as u64,&system_io_t).unwrap();
        assert!(journal.append(&next,false,&partial_reserve_t).is_err());
        assert_eq!(fs::metadata(&reserve).unwrap().len(),expected);
        assert_eq!(fs::read(journal_path(&root,1001)).unwrap(),data);
        assert!(!journal.failed());
    }
    let store=journal_store_t::open(&root,owner_of(&root),&system_io_t).unwrap();
    let mut journal=store.open_journal(1001,&parsed,data.len() as u64,&system_io_t).unwrap();
    journal.append(&next,false,&system_io_t).unwrap();
    assert_eq!(fs::metadata(&reserve).unwrap().len(),ReserveBytes);
    assert!(fs::metadata(&reserve).unwrap().blocks()*512>=ReserveBytes);
}
#[test]
fn edge_control_reserve_modes_and_symlink() {
    let root=temp_root("edge_modes");let user=test_user(1001);let registry=open(&root);enroll(&registry,&user);drop(registry);
    let reserve=root.join("users/1001/reserve");
    for (mode,accepted) in [(0o644,false),(0o660,false),(0o777,false),(0o600,true),(0o700,true)] {
        fs::set_permissions(&reserve,fs::Permissions::from_mode(mode)).unwrap();
        assert_eq!(open(&root).unusable_users().unwrap().is_empty(),accepted,"mode {mode:o}");
    }
    fs::remove_file(&reserve).unwrap();std::os::unix::fs::symlink(root.join("elsewhere"),&reserve).unwrap();
    assert_eq!(open(&root).unusable_users().unwrap().len(),1);
}
```

### review_directory_restarts.rs

```rust
// Existing-file retry must also reconstruct failed creation-directory syncs.
#[test]
fn edge_directory_journal_creation_sync_failure_then_restart() {
    let root=temp_root("edge_journal_parent");let user=test_user(1001);
    let calls=Arc::new(Mutex::new(Vec::new()));
    let required=root.join("users/1001");
    let spy=edge_sync_t{calls:calls.clone(),fail_path:required.clone(),fail_once:Arc::new(std::sync::atomic::AtomicBool::new(true))};
    let registry=open_with(&root,registry_config_t::default(),Box::new(spy.clone()));
    assert_eq!(code(registry.append_identity(Identityd,user.uid,object_kind_t::identity_enrollment,&user.enrollment)),"ipc/storage_unavailable");
    drop(registry);calls.lock().unwrap().clear();
    let registry=open_with(&root,registry_config_t::default(),Box::new(spy));
    let receipt=registry.append_identity(Identityd,user.uid,object_kind_t::identity_enrollment,&user.enrollment).unwrap();
    println!("already-present receipt={receipt:?}; directory syncs={:?}",calls.lock().unwrap());
    assert!(calls.lock().unwrap().contains(&required),"journal entry acknowledged after restart without repeating failed creation-directory sync");
}
#[test]
fn edge_directory_recreated_reserve_sync_failure_then_restart() {
    let root=temp_root("edge_reserve_parent");let user=test_user(1001);
    let registry=open(&root);enroll(&registry,&user);drop(registry);
    fs::remove_file(root.join("users/1001/reserve")).unwrap();
    let calls=Arc::new(Mutex::new(Vec::new()));let required=root.join("users/1001");
    let spy=edge_sync_t{calls:calls.clone(),fail_path:required.clone(),fail_once:Arc::new(std::sync::atomic::AtomicBool::new(true))};
    let registry=open_with(&root,registry_config_t::default(),Box::new(spy.clone()));
    assert_eq!(registry.unusable_users().unwrap().len(),1);
    drop(registry);calls.lock().unwrap().clear();
    let registry=open_with(&root,registry_config_t::default(),Box::new(spy));
    let key=test_key("edge reserve cert");
    let cert=certificate(&user.identity,user.uid,&key,authority_class_t::persistent,1);
    registry.append_identity(Identityd,user.uid,object_kind_t::key_certificate,&cert).unwrap();
    println!("directory syncs after reserve recreation retry={:?}",calls.lock().unwrap());
    assert!(calls.lock().unwrap().contains(&required),"new reserve entry never resynced before ordinary append");
}
```

## Evidence digests

SHA-256 values below identify local evidence retained for this review; logs are not added to the repository. Relative paths are under `/tmp/cham3_review_r2/`. Reproduction source is embedded above and retained under the plugin artifact staging directory.

| Evidence | SHA-256 |
|---|---|
| `qualification_nofile.log` | `815ab5e00211028deabced6dfb08773b3ae375882b6242870a0c7c162a4a445e` |
| `protection.log` | `a71c8dbe61baa6655c8a2f22a43f58495c663506bb8d6be496deef3fcae7a2f3` |
| `reserve_full.log` | `c2e23ed263c7565ddd28f473aae0c193fa9dd65a07abe281855d5d482972e83d` |
| `candidate/out/static_checks_review.log` | `3b060ae8c6cbd18697388da94cc0ad5bef0416d593b2b86f18ab503fc87b7cd0` |
| `candidate/out/static_tests_review.log` | `ec0d0a9d90040ec41ef29345ece417b23ebb9be81c64981fad64551f5a42bd50` |
| `adversarial005/own_debug.log` | `e2eb78d3695798d63acd2236012a244458b1855c9ad4f27efdfc6900a4a5f8fe` |
| `adversarial005/own_release.log` | `8aefeae22b62f4c207941153aa01007f3ec3abfb9b6b55fe10eab56d1d863025` |
| `own_full_debug.log` | `d08a339c5d2f448d7023236a3c7aad4f37f908c04f1562c5710350783807dddb` |
| `own_full_release.log` | `6c6ce704bb5d455a4ec72a5f38f0f1bc908199b57c9a6c02497fd41e119ddb75` |
| `adversarial005/edge_debug.log` | `e2f4cfe91039b05f57d2e31fffa90d8a38cd53bd1bd758d35d8b0b6b651a21d1` |
| `adversarial005/edge_release.log` | `f8f029e1914fbaa7716079a4d0364d0d12d1bbd415a482fa291ab065869afed7` |
| `adversarial005/controls_debug.log` | `1a2f58faaa7e77448a3021095a025cb69ef655bd255f679368cafcc75584686b` |
| `adversarial005/controls_release.log` | `5dd0c2920a9ec1dd382dea289cb547d3f42c44d8c8c577997571414295d556fe` |
| `adversarial005/directory_debug.log` | `cfe5e8810de599769583184dd0ae33a3871c445629d5abc79f7cf1f164cca08e` |
| `adversarial005/directory_release.log` | `978efdd9b94837097031fa445f127645f001839fb2168231fe54095fe3b2532a` |
| `probe_refusal_matrix.log` | `bff20ebdb8adb14e2fbb25dff7b0f1bef76561a0741a866c45ab9e386ab65306` |
| `adversarial004/original_debug.log` | `d395fb381d33a7b8d14a55958dd5ec97cd30cb5cc498f6367de7a4c47af301db` |
| `adversarial004/original_release.log` | `9cb11770d9295b3b4cfc06981cf0d38aa4d7acd13ab691815c1160e4bbf1bc46` |
| `adversarial004/r2_debug_verified.log` | `599aae1f49983fcd686a36f4f9643c62ed08de4e1b5ab0c73cd36946e54b72ed` |
| `adversarial004/r2_release_verified.log` | `1d9b0e2e6f0504b1106aaeb9ea303ff59605ffd1bf9d08f5fcc9da875bbe35a0` |
| `ci_comparison.txt` | `8b998fe1a90dee3857e7b57664d8155360e87af3d4c92db38a986085327fef2d` |
| `vectors_comparison.txt` | `a321cf14916f28c7908d0957d6138730760a3510eb12af6993cda28bf303a85d` |
| `ci_m2.json` | `a1bf55f609a71804527de924c9c25a7263878ca467d3ac12c5dac96a517b3741` |
| `ci_m1.json` | `f66a87fb7c06118b4d4379523ac0ff7e94fcbf234e63b2869c60f9d797b93fe1` |
| `ci_m0.json` | `62ae05818b296c20437797aa5e129f7ced61d15101d0a028d22327f9c1e2e8a7` |
| `ci_m2.log` | `90e93716ef015a35538860093107157098e4cc5de080dc98441996ba991169c4` |
| `ci_m1.log` | `6825fdad92acf6756a5885c2d50aa1723543f63b0cb419e7d9478a5b3131f820` |
| `ci_m0.log` | `b31b9ede62bac8f879cae6a584853a79e483446071be3d25638eeead3232998d` |
| `ci_artifact.zip` | `f05dedc6e999261dddc44ffcf780a97ae7b0fa1ea61b26ddd57ab41fe383a926` |
| reproduction `review_extra.rs` | `d437b4aa16d8f89d2d24214940594bf2015688c89817c7b2a8ff935f2c382cd1` |
| reproduction `review_controls.rs` | `efd6142c2d17b88bcbf4fc8d1cec30401fa7476370b623e3cc37ae93429e7b94` |
| reproduction `review_directory_restarts.rs` | `4a972f877654d344c15fefe522bbf4d438521531fbfd83488a0f5b5e8d72da74` |
| plugin `051b4c8aacdeecfabb23625f93dd16615aec39a0_20261013T161052Z_0qpa0f5v/report.md` | `4233e954b78a417d426fa3ade6357150507202f1adf204db6ca4249d57fa07be` |
| plugin `051b4c8aacdeecfabb23625f93dd16615aec39a0_20261013T161052Z_0qpa0f5v/coverage.json` | `684485d499036989fab4ca5c8160b7401d162a01ffce95936d8f9e108b5f608d` |
| plugin `051b4c8aacdeecfabb23625f93dd16615aec39a0_20261013T161052Z_0qpa0f5v/findings.json` | `665f6c3e768ca0a9f140bbde71cdec47645b2ae740e070ac75f40f7cecb38bd5` |
| plugin `051b4c8aacdeecfabb23625f93dd16615aec39a0_20261013T161052Z_0qpa0f5v/scan-manifest.json` | `67ad6b52d866b2d720499923246294a29c1bbaf8d73968c5755acfab913331ab` |
| plugin `051b4c8aacdeecfabb23625f93dd16615aec39a0_20261013T161052Z_0qpa0f5v/exports/results.sarif` | `31706364128fd0eb7b8618639196a9cd3df27a8e2ad5903c7332285373818b72` |
| plugin `ca40c62ed7a5fc29ba8910fef514f3ce29ea4fba_20261013T161606Z_q4x4zvq_/report.md` | `a7a6b0b77e666905a5cba1800077efa8a1a634367b4c0f8ab18a833118a1d41c` |
| plugin `ca40c62ed7a5fc29ba8910fef514f3ce29ea4fba_20261013T161606Z_q4x4zvq_/coverage.json` | `404e0f7f90d1bcb91c369a8d3388beb962b9f9f54130846f2f964cbb3b0eeabc` |
| plugin `ca40c62ed7a5fc29ba8910fef514f3ce29ea4fba_20261013T161606Z_q4x4zvq_/findings.json` | `a1827ba318ac44bb8ff0db49829feef0c6515337c9a31abcfa04060fd8a2d2cf` |
| plugin `ca40c62ed7a5fc29ba8910fef514f3ce29ea4fba_20261013T161606Z_q4x4zvq_/scan-manifest.json` | `0ec13fccdcd6c5f29ed4d957881b1c8dbbe7334fc54e5c2d3f1f3774ecb57e3b` |
| plugin `ca40c62ed7a5fc29ba8910fef514f3ce29ea4fba_20261013T161606Z_q4x4zvq_/exports/results.sarif` | `181a248ac5a2ad4f5f4b8a499e30ec5e6e81d91a9dce55d08db0e5178f825dda` |
