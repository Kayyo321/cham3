# Independent security/architecture review: M2-001 and G1–G6

- **Date / reviewer:** 2026-10-12 / Codex, independent REVIEWER. No implementation authorship or owner decisions.
- **Repository / requested branch:** `Kayyo321/cham3`, `codex/m2-cryptographic-contract-core`.
- **Baseline:** `a4bc0c4371f2298787f0876d81c0c6fd21150ea6`.
- **Code/spec/ADR candidate:** `f1437f0d254af2968c3598bf14d1a9b2b8e791cd`.
- **Evidence/handoff commit:** `3c35aac0e8d93fef334fc4f208e1365857b1e84a`. Independently checked: its changes from the candidate are documentation only.
- **Authority read:** AGENTS.md; PROJECT.md §§3–5.7, 6.1, 10–15, 18 and relevant network/inspector requirements; approved ADR 0001; M2 task including A01–A15; qualification/design handoff; workflow, roles, orchestration, testing and security invariants.
- **Source checkout:** detached `/tmp/cham3_review_candidate` at the handoff commit. Execution clone: `/tmp/cham3_review_run` at the same commit, initially clean. The supplied `/workspace/cham3` was clean on `work` at `29cc9c20d55520840a06aa142e5d8ab9640d10ed`; it was not overwritten or switched. Only this review record was added there.
- **Environment:** managed Debian 13 amd64 container, task UID/GID 1000, kernel `6.18.44`; Docker 28.4.0, vfs, local daemon. Qualification runs used the candidate's digest-pinned Debian image, Debian 13.7, UID/GID 1000, no additional capabilities and no privileged container. The Dockerfile's normal image-assembly package installation is distinct from the unprivileged qualification/tests.

## Verdicts

| Package/gate | Verdict | Required changes |
|---|---|---|
| M2-001 implementation | **CHANGES_REQUESTED** | R01, R02. Native qualification otherwise reproduced successfully. |
| G1 — ADR 0003 | **CHANGES_REQUESTED** | R01–R03; commit reviewed target locks and qualify the resulting exact commit. |
| G2 — ADR 0004, wire specification/CDDL/vectors | **CHANGES_REQUESTED** | R06–R08. |
| G3 — ADR 0005 and IPC specification | **CHANGES_REQUESTED** | R03, R05; align the IPC size contract under R08. |
| G4 — ADR 0006 | **CHANGES_REQUESTED** | R04, R05, R08, R10; correct I01. |
| G5 — ADR 0007 | **CHANGES_REQUESTED** | R04, R07, R09. |
| G6 — ADR 0008 | **CHANGES_REQUESTED** | R05, R06, R11. |

These are independent-review verdicts. They neither approve architecture on the owner's behalf nor authorize dependent implementation. The owner's anchor-mode, one-shot and deletion-granularity choices remain open. M2-003 onward remain gated; full M2 is not complete.

## Findings

### R01 — Medium: missing source-identity evidence is reported as PASS

**Location:** `tests/m2/lib/m2_harness.py:268`, `:410`; related `build/scripts/qualify_m2_rust.sh:74`.

**Trigger:** run the environment collector from an exported/disposable tree without `.git`, or make an identity command fail. The collector stores exit statuses but always wraps the result as `PASS`. Its Git failure output is even interpreted as a dirty state, rather than an evidence failure. The qualification shell likewise prints command substitutions without validating the Git commands' success.

**Independent reproduction:** in `/tmp/cham3_review_defect` (a `git archive` export), `python3 -I tests/m2/lib/m2_harness.py environment --run-dir out/reviewer_missing_git` exited 0 and printed `PASS environment - tests`; `environment.json` has `git_head`, `git_tree` and `git_status` status 128 with “not a git repository,” and top-level `result: PASS`.

**Rationale:** A01/A02 and the task's dispatcher contract require exact identities and failure on missing evidence. Successful test execution cannot repair missing commit/tree evidence. This did not invalidate the clean local/CI qualification identities in this review, but it is a confirmed harness false positive.

**Required:** validate required evidence commands and formats, reject missing lock identity, and fail qualification on failed Git identity collection. Add a missing-repository/failed-identity canary. Keep deliberately dirty defect runs explicitly labeled rather than equating missing identity with permitted dirty source.

### R02 — Medium, known prerequisite: amd64 inputs are still uncommitted

**Location:** `docs/decisions/0003_m2_rust_toolchain_dependencies.md:32`; `build/scripts/qualify_m2_rust.sh:138`.

**Trigger:** native amd64 qualification at the reviewed commit. `build/m2/debian_lock_amd64.txt` and `build/m2/debian_debs_amd64.sha256` do not exist. Both CI and the independent local run fail those checks, correctly and by design.

**Rationale:** G1 expressly requires reviewed, committed amd64 locks before approval. A01 is not complete merely because `--record` generated proposals. This is an outstanding gate prerequisite, not a newly discovered failure in the lock checker.

**Required:** commit the reviewed locks, rerun without unrecorded inputs, and supply the new exact review/evidence commit. See the lock assessment below.

### R03 — Medium: Argon2 caller memory has no defined clearing owner

**Location:** `docs/decisions/0005_m2_identity_vault_ipc.md:71`; `docs/decisions/0003_m2_rust_toolchain_dependencies.md:77`; illustrative probe `core/probes/provider_probe/src/lib.rs:392`.

**Trigger:** use pinned Argon2 0.5.3's `hash_password_into_with_memory`, including with the `zeroize` feature. The caller-supplied `Block` workspace remains populated after return. `Block` implements `Zeroize`, not automatic clearing on drop. The probe drops an ordinary `Vec<Block>` without clearing it; the probe's inputs are public synthetic data, so this is not an operational secret leak in the current skeleton.

**Independent evidence:** a separate safe Rust executable with the exact pinned dependency and `zeroize` feature confirmed nonzero caller workspace after hashing. Dependency source `argon2-0.5.3/src/lib.rs:243` calls fill/finalize without clearing caller blocks; `src/block.rs:151` supplies explicit `Zeroize` only. The design's attribution to the libraries' “own zeroization” does not cover this workspace.

**Rationale:** PROJECT §4.5 and G1/G3 require explicit sensitive-buffer ownership/cleanup. `mlockall` prevents swapping; it does not erase a freed heap buffer. Neither residual stack-copy limitations nor the fact that RAM is involved substitutes for clearing an accessible caller-owned KDF workspace.

**Required:** name the owner and explicit clearing of the entire Argon2 workspace on success and every error path; specify its allocation/locking limits, and test actual cleanup. Also describe secret custody/protection in the passphrase-collecting development client. Preserve the documented abort/process-death limits. Do not reuse the public-data probe's allocation pattern as the vault implementation.

### R04 — High: restart can undo a mandatory restriction after failed writes

**Location:** `docs/decisions/0007_m2_lifecycle_time_consumption.md:33`; recovery `docs/decisions/0006_m2_storage_anchor_recovery.md:54`.

**Trigger:** journal and anchor writes both fail during an explicit revocation/restriction. The running service denies in memory, then exits. Storage becomes writable before restart; the old journal and old anchor still match. G4 says resume, and G5 explicitly acknowledges that the restriction is lost.

**Rationale:** this is a direct contradiction of PROJECT §5.7 and approved ADR 0001: a failed explicit-revocation commit must keep affected access denied/unknown; recovery cannot infer continuing validity from missing lifecycle evidence. Reporting the revocation as failed does not authorize resumed access. No attacker with kernel privilege is needed: transient write failure plus service restart suffices.

**Required:** freeze a conservative restart protocol that can distinguish or conservatively contain unsuccessful restriction persistence. For example, specify a durable pre-service dirty/unclean marker and its write/anchor barriers, plus compulsory quarantine after any unclean/unverifiable run. Specify what happens when even that marker cannot be persisted. Add the exact both-unwritable → restart-after-recovery fault case. No recommendation here silently selects an implementation or relaxes the approved denial rule.

### R05 — High: freshness coverage stops at registryd, leaving key/identity rollback undefined

**Location:** `docs/decisions/0005_m2_identity_vault_ipc.md:21`, `:29`; `docs/decisions/0006_m2_storage_anchor_recovery.md:21`, `:38`; `docs/decisions/0008_m2_retention_privacy_update_identity.md:40`.

**Trigger:** restore an older identityd enrollment/certificate/invalidation state while leaving registryd and its independent anchor current, or crash between identityd invalidation/replacement and registryd's related lifecycle persistence. A formerly compromised/replaced authority can again appear current to `resolve_key`. G6's assertion that restored vault keys cannot revive invalidated keys because “identityd state wins” assumes identityd freshness, which none of these proposals defines.

**Rationale:** the approved authoritative-state design requires protected key/epoch state and rollback-resistant reconciliation (A04, A08–A10, A14). Public-key signatures authenticate old identity records but cannot prove that no later invalidation/replacement was lost. The per-user registry journal anchor does not bind identityd's separate state, and the proposal does not freeze a cross-service durable invalidation barrier.

**Required:** define identityd's storage, anchored current enrollment/key/identity epochs and invalidations, and the atomic/restrictive ordering linking key lifecycle changes with registry authorization. Independently restore each service's state and crash between the durable boundaries; ambiguity must deny. G6 backup instructions must name the coordinated restore/recovery protocol rather than rely on unanchored identityd state.

### R06 — Medium: ordinary revisions can rebind application identity in the proposed oracle

**Location:** `docs/specs/m2_wire_format_v1.md:152`, `:173`; `tests/m2/oracle/ecr_oracle.py:722`; `docs/decisions/0008_m2_retention_privacy_update_identity.md:49`.

**Trigger:** take the published signed `commit_revision`, replace its `subject.package_identity` and `subject.executable_sha256`, and re-sign with its synthetic persistent key. The lineage hash is unchanged because it binds user/app/installation only. All published base/delta checks still pass. The oracle admits this as an ordinary revision after genesis.

**Independent reproduction:** `schema_holes.py` reports `ACCEPTED ordinary revision with substituted package/executable; no identity_transition`.

**Rationale:** G6/G5 require denial until the explicit, approved identity transition. G2 defines a positive difference check for `identity_transition`, but omits the corresponding invariant that other successor types preserve the authorized subject binding. A revision can retain old grants while silently restating them under a different executable. The oracle labels its admission logic a subset; this is a missing design/vector constraint, not a demonstrated vulnerability in a nonexistent Rust registry.

**Required:** define which subject fields may change for every commit type, compare non-transition subjects with authoritative current subject state (including merges/temporary branches), and add signed negative vectors for ordinary revision/merge/restore substitution. Bind/display identity changes in the approval delta, not just an incidental subject field.

### R07 — Medium: network authority lacks grant lifetime/revocation semantics and complete restore constraints

**Location:** `docs/specs/m2_ecr_v1.cddl:119`, `:147`; `docs/specs/m2_wire_format_v1.md:148`, `:149`, `:188`; `docs/decisions/0007_m2_lifecycle_time_consumption.md:20`, `:31`.

**Trigger:** the oracle accepts a Direct genesis with an empty grant set. Network mode is a top-level enum, without its own grant ID, effective start, expiration/session/usage bounds or delegation. `temporary_grant` forbids network changes; `revocation` requires removing at least one resource grant and forbids network changes. G5 restricts named grants, so its specified revocation path cannot target this network authority. Separately, a restore referencing the published Offline genesis can introduce Tor even though that snapshot did not contain it; the oracle accepts the signed object/chain because restore scope matching covers resource grants only.

**Independent reproductions:** `schema_holes.py` accepts both the zero-grant Direct genesis and the Offline-reference/Tor-result restore.

**Rationale:** PROJECT §§5.1–5.5 and ADR 0001 require lifetime-bound authority, explicit revocation and an exact before/after state for network changes too. The current design cannot encode bounded network authorization or its grant-specific restriction. A freshly approved general revision can intentionally change a mode, but that does not supply the missing lifetime/revocation model or justify presenting an unrelated mode as a historical restore.

**Required:** freeze the network authority representation and how it participates in lifetime, restriction, revocation, key/policy invalidation and restore checks. If v1 intentionally supports only persistent network policy, seek an explicit scoped owner/spec decision instead of claiming every required contract field/lifetime is covered. Constrain and display the entire restore target, including network state, and publish vectors.

### R08 — Medium: a wire-valid maximum object cannot fit its IPC/storage containers

**Location:** `docs/specs/m2_wire_format_v1.md:55`; `docs/specs/m2_service_ipc_v1.md:24`; `docs/decisions/0006_m2_storage_anchor_recovery.md:24`.

**Trigger:** an independently constructed signed, schema/intrinsically valid and oracle-admitted commit is exactly 65,536 bytes. Wrapping that envelope in a minimal `submit_signed` request takes 65,602 bytes; even a minimal `commit_admitted` record with kind/seq/time/envelope takes 65,603 bytes. Both specifications cap the outer container at 65,536 bytes. Required record metadata only increases the excess.

**Rationale:** G2's maximum accepted object and G3/G4's transport/storage admission contract disagree. This is a reproducible boundary failure, not just a large hypothetical payload. The current small vectors miss it (A03, A08, A12).

**Required:** define separate consistent envelope, message and journal-record limits, including worst-case wrapper metadata. Account for the embedded payload's depth/size limits and provide maximum-size success/one-byte-over rejection vectors across the interfaces. Specify bounded history/export pagination too; one datagram cannot contain the quota-sized store.

### R09 — Medium: journal-record time floor does not bound rollback after an idle period

**Location:** `docs/decisions/0007_m2_lifecycle_time_consumption.md:53`, `:54`.

**Trigger:** last journal time/floor is 12:00, a timed grant expires at 13:00, and the service clock advances to 13:10 without a later durable record. The clock then moves back to 12:30. This is a 40-minute rollback, yet `now < floor - 300s` is false and `max(now,floor)` is before expiration. An already expired grant can again evaluate eligible unless an independently durable/latching restriction exists; none is required by the specified floor rule. Lazy or failed expiry evidence is explicitly allowed.

**Rationale:** the floor is the most recent recorded time, not the greatest observed trusted time. The design's promised clock-rollback/expiry controls (A06/A09) therefore do not follow from the comparison, especially when storage writes are unavailable. The ±300-second admission tolerance does not close this larger rollback case.

**Required:** define an in-process monotonic deadline/high-water mechanism and conservative restart behavior for uncertain elapsed time, including suspend/resume and failed floor persistence. An observed expiry must not be undone by later clock correction. Test rollback after a long record-free interval and after failed expiry evidence, not only a clock behind the last commit.

### R10 — Medium: per-user writer locks do not serialize the shared anchor

**Location:** `docs/decisions/0006_m2_storage_anchor_recovery.md:22`, `:38`.

**Trigger:** users A and B update their journals concurrently under separate user locks, read the same global anchor map/counter, and each prepares the next alternating slot with its own user update. A's durable update/reply can be overwritten by B's snapshot of A's older anchor. A later restoration of A's journal to that older sequence can match the surviving anchor despite an acknowledged consumption/restriction being lost.

**Rationale:** the anchor contains all users plus tombstones, so atomic per-user admission does not suffice for atomic anchor-map updates. No global serialization/CAS rule or slot-level durable write protocol is specified. This is an architectural concurrency gap (A08/A10), not a claim about implemented anchor code.

**Required:** specify global anchor update serialization and a durable counter/slot protocol, lock ordering with user locks, and failure handling. Race independent users and deletion tombstones as well as two writers in one lineage.

### R11 — Medium: account deletion lacks a durable tombstone-first crash protocol

**Location:** `docs/decisions/0008_m2_retention_privacy_update_identity.md:34`; `docs/decisions/0006_m2_storage_anchor_recovery.md:63`.

**Trigger:** deletion erases some/all vault, registry and identityd files and is interrupted before the global tombstone becomes durable, or tombstone persistence fails. Restoring the old complete account state can match the old anchor and resume because no tombstone exists. The prose lists deletion and a tombstone but specifies no barrier, pending-delete recovery state or cross-service ordering.

**Rationale:** G6 claims deleted identities cannot be resurrected from backup. A tombstone is effective only if it is durably established before destructive steps and before reporting successful deletion; the current proposal cannot be verified at its crash boundaries (A08/A10/A14). This does not assert that deletion must complete when storage is failing; it must fail conservatively and preserve the recorded intent/restriction.

**Required:** specify and test a durable deny/tombstone-first ceremony, per-service idempotent cleanup, UID reuse, tombstone retention, and recovery when any erase/anchor write fails. Define its coordination with R05/R10. Never acknowledge complete deletion before the tombstone is durable.

## Additional review notes

- **I01 — Low, unverifiable status wording:** `docs/decisions/0006_m2_storage_anchor_recovery.md:44` and `:46` describe anchor backends as “Implemented and tested”/“Implemented.” At this candidate registryd contains only its qualification name function/test. No anchor backend or A10 test exists. Change these to planned M2 implementation/verification obligations. No anchor PASS is granted by this review.
- **I02 — Low, ongoing tooling coverage:** `build/scripts/qualify_m2_rust.sh:217` runs Valgrind on provider `report`; `core/probes/provider_probe/src/lib.rs:521`'s `run_all` does not allocate a `secret_region_t`. That check is provider/OS-randomness evidence, not Valgrind evidence for the local unsafe allocation adapter. The reviewer separately ran the actual secure-memory test executable successfully under Valgrind. Add that actual path to continuing qualification, with explicit subprocess/instrumentation limits.
- **I03 — Scope:** the setgid development client is executable by ordinary users and is explicitly described as callable by same-UID processes. Its role/DAC checks do not establish same-user application isolation or trusted input/display/human presence. The proposals acknowledge this; preserve the development-only distinction and M3/M5 obligations. Passphrase knowledge and exact digest binding are not independently verified human presence. Fixture identities/consumers also establish no actual resource/network enforcement.

## Independent verification

All following commands ran as task UID 1000. Qualification containers ran as UID/GID 1000 with no `--privileged`, `CAP_IPC_LOCK`, host-protection changes or production secrets. `cham3-m2-review` is the image built from the unchanged candidate Dockerfile/context, only with a reviewer-specific tag. Raw results/tooling are in the evidence bundle described below.

### Environment and source

| Status | Exact command / environment | Result/limit |
|---|---|---|
| PASS | `git status --short; git rev-parse HEAD; git branch --show-current` in `/workspace/cham3` | Initial clean source at `29cc9c2`, branch `work`; identified mismatch with requested branch. |
| PASS | `git fetch origin codex/m2-cryptographic-contract-core` in `/workspace/cham3` | All requested immutable objects available. No push. |
| PASS | `git worktree add --detach /tmp/cham3_review_candidate 3c35aac0e8d93fef334fc4f208e1365857b1e84a` | Read-only review source; initial clean status. |
| PASS | `git diff --stat a4bc0c4371f2298787f0876d81c0c6fd21150ea6 f1437f0d254af2968c3598bf14d1a9b2b8e791cd`; `git diff --name-only f1437f0d254af2968c3598bf14d1a9b2b8e791cd 3c35aac0e8d93fef334fc4f208e1365857b1e84a` | Candidate scope confirmed; handoff/evidence change documentation only. |
| PASS | `id`; `uname -a`; `cat /etc/os-release`; `cat /etc/codex/network-policy.json` | UID 1000, amd64, Debian 13. Runtime status rechecked ready/enforced; restricted HTTP policy, no VPN. |
| PASS | `env -u DOCKER_HOST -u DOCKER_CONTEXT -u DOCKER_TLS -u DOCKER_TLS_VERIFY -u DOCKER_CERT_PATH docker --host=unix:///var/run/docker.sock info` | Local daemon healthy, native amd64. |
| PASS | `docker build -f build/m2/dev_container.dockerfile -t cham3-m2-review build/m2` in `/tmp/cham3_review_candidate` | Unchanged digest-pinned Debian image and APT-authenticated pinned direct inputs. Build log retained. |
| PASS | `git clone --no-hardlinks /workspace/cham3 /tmp/cham3_review_run`; `git -C /tmp/cham3_review_run checkout --detach 3c35aac0e8d93fef334fc4f208e1365857b1e84a` | Self-contained Git clone for Docker mount; no linked-worktree `.git` path problem. |

### Exact qualification command

From `/tmp/cham3_review_run`, equivalently using its absolute bind mount (as executed):

```sh
docker run --rm --user "$(id -u):$(id -g)" \
  -e HOME=/tmp/cham3_home \
  -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory \
  -e GIT_CONFIG_VALUE_0=/work \
  -v /tmp/cham3_review_run:/work -w /work cham3-m2-review \
  bash build/scripts/qualify_m2_rust.sh --record
```

**FAIL overall, exit 1**: only the two expected uncommitted-lock checks fail. Run `/tmp/cham3_review_run/out/m2/qualification/amd64_20261012T162828Z_1`; identity is clean commit `3c35aac`, tree `b675d9cc242ba38ad9698245a226c39ed0c18b27`, Debian 13.7, UID 1000. Cargo.lock SHA-256 `fc87901c5e58acdba21a33fe516871e45a81072585bfa2ae82aa8e60c40310f6`.

| Status | Check and exact nested command | Evidence |
|---|---|---|
| PASS | `cat /etc/debian_version`; `rustc -Vv`; `cargo -V`; `cc --version`; `ld --version`; `ldd --version` | rustc/Cargo 1.85.1, LLVM 19.1.7, gcc 14.2.0-19, ld 2.44, glibc 2.41-12+deb13u4. |
| PASS | `dpkg-query -W -f '${Version}' <each pin in build/m2/debian_packages.txt>` | All 33 explicit pins match. |
| FAIL | `dpkg-query -W -f '${Package}:${Architecture}=${Version}\n' \| LC_ALL=C sort`, compared with `build/m2/debian_lock_amd64.txt` | No committed amd64 lock; proposal generated. |
| FAIL | Recorded `/usr/local/share/cham3/m2/downloaded_debs.sha256` compared with `build/m2/debian_debs_amd64.sha256` | No committed amd64 digest list; proposal generated. |
| PASS | `cargo metadata --locked --offline --format-version 1` | Offline resolution. |
| PASS | `python3 -I build/scripts/m2_dependency_inventory.py sources --metadata "$run_dir/metadata.json" --check build/m2/crate_sources.sha256` | All 1,454 dependency files match. |
| PASS | `python3 -I build/scripts/m2_dependency_inventory.py inventory --metadata "$run_dir/metadata.json" --out-dir "$run_dir"` | 33 compiled external crates; Debian source provenance/features/build scripts/unsafe locators recorded. |
| PASS | `cargo fmt --all --check` | Formatting. |
| PASS | `env CARGO_TARGET_DIR="$run_dir/clippy_target" cargo clippy --workspace --all-targets --frozen -- -D warnings` | Lints. |
| PASS | `python3 -I build/scripts/check_rust_naming.py` | Naming. |
| PASS | `python3 -I -` with the profile-check heredoc at `build/scripts/qualify_m2_rust.sh:175` | Every defined profile overflow checks; release abort/dev unwind declarations. |
| PASS | `env CARGO_TARGET_DIR="$run_dir/build_target" cargo build --workspace --frozen` | Fresh debug build. |
| PASS | `env CARGO_TARGET_DIR="$run_dir/build_target" cargo build --workspace --release --frozen --verbose` | Fresh release build; soft chacha20/poly1305, sha2 force-soft and curve25519 fiat flags reach actual invocations; fiat build-script outputs in both profiles. |
| PASS | `valgrind --error-exitcode=99 --track-origins=yes "$run_dir/build_target/release/cham3_provider_probe" report` | Provider probe; scope under I02. |
| PASS | `sh tests/m2/run_m2_tests.sh unit` | 32 debug + 32 release tests, exact inventories, 7 provider checks/oracle agreement, ECR oracle 17 valid/80 invalid/7 admission-invalid. |
| PASS | `sh tests/m2/run_m2_tests.sh harness` | 57 tests, real assertion/compile/link/panic/zero/ignored/filtered/compile-only canaries, unsafe insertion controls per layer, deployed release panic/overflow SIGABRT and dev 101. |
| NOT_RUN | `cargo miri --version` was probed and unavailable; no Miri analysis command ran | Debian toolchain has no Miri. |
| NOT_RUN | Sanitizer execution / `cargo fuzz run <target>` | No supported sanitizer or coverage-guided fuzz target/toolchain; never a memory-safety PASS. |

Here `$run_dir` is `/work/out/m2/qualification/amd64_20261012T162828Z_1` inside the container. Unit run: `unit_20261012T162902Z_1525`; harness run: `harness_20261012T162925Z_2577`.

### Additional checks and controlled defects

| Status | Exact command / environment | Result/limit |
|---|---|---|
| PASS | `docker run --rm --user 1000:1000 -v /tmp/cham3_review_candidate:/work:ro -v /tmp/cham3_review_tools:/review -w /work cham3-m2-review python3 -I /review/independent_vectors.py tests/m2/vectors/ecr_v1/vectors.json` | All 17 canonical envelopes/protected maps/payloads, signatures and envelope hashes; commit lineage hashes; 17 altered-preimage rejections. cbor2 5.6.5 and cryptography 43.0.0/OpenSSL 3.5.7. No import of the author's oracle. Own COSE Sig_structure assembly; not an off-the-shelf COSE policy implementation or independent full semantic verifier. |
| FAIL (design expectations) | `docker run --rm --user 1000:1000 -v /tmp/cham3_review_candidate:/work:ro -v /tmp/cham3_review_tools:/review -w /work cham3-m2-review python3 -I /review/schema_holes.py tests/m2/oracle/ecr_oracle.py tests/m2/vectors/ecr_v1/vectors.json` | Script exits 0 proving four accepted examples: identity substitution, unbounded network state, network restore mismatch and valid 65,536-byte object exceeding wrapper caps. Uses the author's oracle deliberately to demonstrate coverage/design gaps; does not claim independent policy verification. First boundary-builder attempt failed to construct the target length; reviewer tool was corrected and the final reproduction passed. |
| PASS (sensitivity) | `git archive 3c35aac0e8d93fef334fc4f208e1365857b1e84a \| tar -x -C /tmp/cham3_review_defect`; insert only `#[ignore]` before `memlock_exhaustion_fails_closed` in that disposable export; `docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home -v /tmp/cham3_review_defect:/work -w /work cham3-m2-review sh tests/m2/run_m2_tests.sh unit` | Expected exit 1: debug and release each reject 1 ignored test, 9/10 passed. Clean actual clone's 32/32 tests are the positive control. Review source unchanged. |
| FAIL (harness requirement) | `docker run --rm --user 1000:1000 -v /tmp/cham3_review_defect:/work -w /work cham3-m2-review python3 -I tests/m2/lib/m2_harness.py environment --run-dir out/reviewer_missing_git` | Unexpected PASS/exit 0 despite required Git status 128, R01. |
| PASS | `docker run --rm --user 1000:1000 -v /tmp/cham3_review_run:/work -w /work cham3-m2-review bash -c 'set -e; for p in out/m2/runs/unit_*/target_release/release/deps/cham3_secure_memory-*; do case "$p" in *.d\|*.rlib\|*.rmeta) continue ;; esac; [ -x "$p" ] \|\| continue; valgrind --error-exitcode=99 --track-origins=yes "$p" --test-threads=1; done'` | Actual release-profile secure-memory test executable: 10/10, 0 errors, no leaks. Child fault probes execute separately; Valgrind does not instrument exec'd child binaries here. Cargo tests use unwinding; deployed abort is separately checked above. |
| FAIL (design cleanup assumption) | `docker run --rm --user 1000:1000 -v /tmp/cham3_review_tools:/review -w /review/argon_probe cham3-m2-review bash -c 'cargo generate-lockfile --offline && cargo run --frozen'` | Separate safe Rust probe exits 0 confirming uncleared caller workspace; R03. Source, manifest/lock/config included in bundle; public synthetic password/salt only. |
| PASS | `bash build/scripts/run_static_checks.sh` inside `cham3-m2-review`, UID 1000, `/tmp/cham3_review_run` mounted at `/work` | shellcheck, Python compile, YAML and configured secret scan. |
| PASS | `sh tests/run_tests.sh static` in the same container/mount | 82 tests. |
| NOT_RUN | `sh tests/m2/run_m2_tests.sh integration`; `... recovery`; `... packaging` | Actual backend/package acceptance not implemented/gated. Harness canaries did invoke these dispatchers and verified NOT_RUN/exit 1; that is harness behavior only. |
| NOT_RUN | `sh build/scripts/build_m2_core.sh` as a separate entry point | Source reviewed; its underlying fresh debug/release Cargo builds ran through qualification. No additional standalone-build-script claim. |
| NOT_RUN | M1 desktop/packaging/boot suites | No affected desktop/package/image implementation; static regression ran. |
| NOT_RUN | Real M2 identity/vault/IPC/admission/anchor/recovery/runtime enforcement; full transitive dependency soundness/side-channel proof | Domain skeletons only. No simulations counted as implementation proof. |

### CI evidence and blocked retrieval

- **PASS, evidence read:** GitHub connector `github_fetch_workflow_run_jobs(repo_full_name="Kayyo321/cham3", run_id=38276640049)` and `github_fetch_workflow_job_logs(repo_full_name="Kayyo321/cham3", job_id=114903617892)` independently establish native amd64 CI at clean `3c35aac`, Debian 13.7, kernel `6.17.0-1022-azure`, UID 1001. CI's qualification summary agrees with the local run: every check passes except the two missing locks. Its overall job conclusion is **FAIL**, not PASS.
- **PASS, artifact metadata read:** `github_fetch_workflow_run_artifacts(repo_full_name="Kayyo321/cham3", run_id=38276640049, name="m2_qualification_amd64")`: artifact ID `11759242609`, 167,020,134 bytes, ZIP SHA-256 `a8e76da115325dd639c15d2930508dc1f2654f5a0255fccb1a987aa4dfc9df1b`, matching workflow head SHA.
- **BLOCKED, ZIP bytes:** `gh run download 38276640049 -R Kayyo321/cham3 -n m2_qualification_amd64 -D /tmp/cham3_review_ci` fails `Forbidden` for `api.github.com`. Connector `github_download_workflow_artifact(repo_full_name="Kayyo321/cham3", artifact_id=11759242609, file_name="m2_qualification_amd64.zip")` returns a File Service reference/download URL, but `curl --fail --location --silent --show-error '<returned signed URL>' -o /tmp/cham3_review_ci.zip` returns HTTP 403. The configured executor policy does not permit the returned file host. No policy bypass was attempted. Consequently the ZIP's proposed lock bytes were **not inspected or compared**; remote metadata/logs do not establish those bytes.
- **BLOCKED, live advisory snapshot:** `curl --fail --silent --show-error https://security-tracker.debian.org/tracker/data/json -o /tmp/cham3_review_debian_tracker.json` returns HTTP 403 under the configured policy. No current Debian advisory-status clearance is claimed. Pinned provenance/source hashes, selected dependency internals and provider behavior were inspected; this is not a full independent audit of every transitive unsafe path. Complete the advisory review before G1 approval.

## Proposed amd64 lock assessment

**The locally reproduced proposals are acceptable to commit as qualification input locks**, subject to exact bytes below. This recommendation does not approve G1, fix R01/R03, assert absence of advisories or constitute a clean qualification at the future commit.

| Reviewer-generated file | Lines | SHA-256 |
|---|---:|---|
| `proposed_debian_lock_amd64.txt` | 612 | `6cd45590bdebd08f53c95a56112a97030b844959983284db44232b59baa11d3c` |
| `proposed_debian_debs_amd64.sha256` | 535 | `6ce06e450a8b91cd2c91c1d3aaffc360dfed7276f6622ac03dbd05fd35f0f43a` |

Both are in `/tmp/cham3_review_run/out/m2/qualification/amd64_20261012T162828Z_1/` and the evidence bundle. All explicit package versions match the candidate; dependency sources match the 1,454-file manifest; 535 downloaded filenames are unique. Comparing normalized package names with arm64 shows the expected architecture-specific binutils/cpp/gcc packages plus amd64 libquadmath0. Five architecture-specific binNMU version differences are grep, lzop, patchutils, rpcsvc-proto and t1utils. No unexpected target provider/toolchain change was found. Native oracle providers include OpenSSL `3.5.7-1~deb13u3`, libsodium `1.0.18-1+deb13u1`, libargon2 `0~20190702+dfsg-4+b2`.

**CI artifact proposals:** approval of their exact bytes is **withheld/BLOCKED** because retrieval was blocked. If they match these independently reviewed digests, they are the same proposed inputs; otherwise review their differences. Do not claim a comparison already occurred. Commit the appropriate lock files under the intended names and obtain a fully passing rerun plus exact-commit rereview.

## Acceptance coverage and limits

| Criteria | This review's coverage |
|---|---|
| A01 | Target pins/source/build/backend/profile evidence reproduced; substantive domain logic remains future scope; target locks missing; identity-evidence harness hole R01. |
| A02 | Exact 32/32/57 inventories, real canaries, independently injected ignored test, independent vector signatures, targeted Valgrind; Miri/sanitizers/coverage-guided fuzzing unavailable. No general dependency soundness claim. |
| A03 | Published byte/signature/hash sample independently checked (all 17 valid); author's 80 invalid/7 admission-invalid checks executed. New negative/boundary coverage needed under R06–R08. |
| A04–A05 | Design review only; key freshness/cleanup gaps R03/R05. No implemented identity/vault or human-presence evidence. |
| A06–A07 | Design/oracle review; current authorization and registry still absent. Network/identity/time semantics need repairs. |
| A08–A10 | Design review only; mandatory denial, coordinated identity freshness, shared anchor concurrency and crash protocols are unresolved. No A10 anchor PASS. |
| A11–A14 | Interface/design review only; restore/network/identity, size, clock and deletion gaps. Fixture APIs are not product enforcement or sandbox evidence. |
| A15 | Static regression PASS; actual packages/install/upgrade/remove/restart/security integration and final independent completion remain NOT_RUN. |

The local unsafe adapter's checked layout, guarded allocation, borrow-bound slices, fail-closed protections, error-path RAII and clearing/unmapping were inspected. No confirmed memory-unsoundness defect was found in that adapter. `Drop` remains bypassed by abort/death; its stated process-hardening assumptions are essential. Provider KATs and successful tools do not prove future domain authorization, protected IPC, replay resistance or secure secret erasure.

## Evidence delivery and next action

- Published evidence: [M2-001-002-codex-security-review-evidence.tar.gz](M2-001-002-codex-security-review-evidence.tar.gz), SHA-256 `8655477da0c83b186e9468cf874e85fc4a6536d9e5401327c44ef1c9159ab5af`. This is the unchanged reviewer bundle below, added alongside this review record for the owner's requested PR/merge. Extract it in a disposable directory and verify its internal `SHA256SUMS`. Local `/tmp` paths in the report describe the original run; the archive makes the evidence portable.
- Local bundle: `/tmp/cham3_m2_review_evidence.tar.gz`, SHA-256 `8655477da0c83b186e9468cf874e85fc4a6536d9e5401327c44ef1c9159ab5af` (qualification identity/toolchain/summary/logs, both lock proposals, dependency inventories, unit/harness JSON/logs/summaries, CI relevant decoded lines, independent scripts/reproductions, static and Valgrind evidence). It excludes compiled targets and production credentials; all cryptographic test keys/data are public synthetic fixtures.
- Reviewer writes: this record and its evidence attachment only under `docs/reviews/`. Candidate implementation/spec/ADR files were not edited. Controlled mutation was isolated in a disposable export outside the repository. Publication of this review does not merge the M2 candidate implementation or approve any gate.
- Implementer: repair the findings, add the target locks, update the proposals/vectors and repeat affected checks. Supply exact new implementation/evidence commits for independent rereview. Owner: decide the repaired gates and explicit options after review; no gate was implicitly adopted here.
