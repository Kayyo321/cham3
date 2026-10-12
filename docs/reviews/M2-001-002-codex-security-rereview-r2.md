# Independent security/architecture re-review: M2-001 / G1–G6, revision 2

- **Date / reviewer:** 2026-10-12 / Codex, independent REVIEWER. No implementation authorship or owner decisions.
- **Repository / target branch:** `Kayyo321/cham3`, `codex/m2-cryptographic-contract-core`.
- **Prior review:** `docs/reviews/M2-001-002-codex-security-review.md`, published on main at `9cf882503848f8c6cc4938a588cc9fc062a9392d`.
- **Revision range:** `3c35aac0e8d93fef334fc4f208e1365857b1e84a..a2bbb370c867da57fdc72274fb17ca19a23426e4`.
- **Exact implementation/spec/ADR/vector/lock candidate:** `a2bbb370c867da57fdc72274fb17ca19a23426e4`, tree `50a60eac8b2c32104d2758053534dd91b473d88d`.
- **Evidence/handoff:** `6b4ef9d87d69185ff7205b51fa417df1fed5c6f6`; independently confirmed that its changes after the candidate are only under `docs/`.
- **Authority and scope read:** AGENTS.md, PROJECT.md, approved ADR 0001, M2 task and A01–A15/G1–G6, roles/workflow/orchestration/testing/security invariants/status; revision-2 handoff and finding resolutions; ADRs 0003–0008; wire specification/CDDL/IPC; changed implementation, harness, oracle, vectors and CI workflow. Author resolutions were treated as claims to verify.
- **Checkouts:** detached source `/tmp/cham3_m2_r2_source` at the handoff; independent execution clone `/tmp/cham3_m2_r2_run` at the candidate, clean before and after checks. Supplied `/workspace/cham3` began clean on `work` at `9cf8825`; review output is on `review/m2-001-002-r2-security` based there. Only this review record is added to the repository.
- **Environment:** managed Debian 13 amd64 host, UID/GID 1000, kernel `6.18.44`; local Docker, vfs. Unchanged candidate Dockerfile built native amd64 Debian 13.7 image `sha256:a8263eae3067054ba8ca27229a753d27f777d7bee9590350771a7a7dc05f2dda`. Qualification runs were UID/GID 1000, without additional capabilities or privileged containers. Image assembly uses the Dockerfile's normal package installation; qualification itself is unprivileged. No production keys or host-protection changes.

## Verdicts

| Package/gate | Independent verdict | Reason |
|---|---|---|
| M2-001 | **CHANGES_REQUESTED** | R01 remains partially open: failed lock hashing can still produce source-identity PASS. Native qualification otherwise fully PASS. |
| G1 — ADR 0003 | **CHANGES_REQUESTED** | R01; independent Debian advisory review remains BLOCKED. Locks, cleanup pattern and continuing secure-memory Valgrind coverage are verified. |
| G2 — ADR 0004 / wire / CDDL / vectors | **CHANGES_REQUESTED** | R07-r2 and R12: conflicting live network branches and an oracle restore that violates the temporary-lifetime prohibition. |
| G3 — ADR 0005 / IPC | **CHANGES_REQUESTED** | R13: normal key-resolution denial also blocks the required fresh recovery signing. |
| G4 — ADR 0006 | **CHANGES_REQUESTED** | R13: freeze an implementable containment/quarantine recovery ceremony. |
| G5 — ADR 0007 | **CHANGES_REQUESTED** | R07-r2 and R09-r2: network uniqueness across live branches and trusted-time rebasing/persistence. |
| G6 — ADR 0008 | **CHANGES_REQUESTED** | R12 and R13: restore validation and the coordinated recovery path. |

These verdicts are independent review decisions, **not owner approval**. The default anchor mode, containment-release option, one-shot option and deletion granularity remain owner choices. M2-003 onward remain gated; full M2 is not complete. A fully passing qualification does not prove the future services or cure contradictory design rules.

## Resolution verification of every prior finding/note

Here **PASS (design)** means the proposal addresses the original finding at the design gate. It does not mean runtime verification of an unimplemented mechanism. All such runtime checks remain NOT_RUN.

| ID | Result | Independently checked evidence / remaining scope |
|---|---|---|
| R01 | **FAIL — partially resolved** | `m2_source_identity.sh:29–43` validates Git IDs/status; harness `m2_harness.py:274` validates required identity commands; dispatcher/qualification call them. Actual no-Git export now exits 1 in both collectors; 8 identity canaries pass. But shell `:45` masks a failed `sha256sum`, demonstrated below. |
| R02 | **PASS** | Both committed amd64 lock digests match the prior reviewer's proposals; native qualification compares observed bytes with them with empty diffs. CI also passes both lock checks at the exact candidate. |
| R03 | **PASS (probe/design)** | `provider_probe/src/lib.rs:375–436` owns an exact-size workspace, clears after the returned success/error result and on drop; `:678–745` includes the library residual control, success, parameter error and wrong-size cleanup tests. All execute in debug/release. ADR 0003 §7 / ADR 0005 §§4–5,7 name workspace, KEK, seed, passphrase and client custody, locked budget and abort limits. Actual vault/client cleanup NOT_RUN. The test named hash error exercises parameter construction, not every possible library error, but the common cleanup path covers returned errors structurally. |
| R04 | **PASS (design)** | ADR 0006 §§3–4 / ADR 0007 §§2–3 replace the unsafe accepted residual with a durable pre-service running marker, process-surviving tmpfs ledger, restrictive ordering and containment for unverifiable runs. Startup marker failure serves no eligible decisions. Same-boot double-write failure replays the ledger; reboot/missing ledger contains. Suspensions are restrictive lifecycle events and must follow the restrictive path, including across repeated restarts. Both-unwritable fault tests and actual marker/ledger implementation NOT_RUN. Recovery signing still needs R13. |
| R05 | **PASS (design)** | ADR 0006 §§1,5,8 / ADR 0005 §§1–2,6 / ADR 0008 §5 put enrollment/certificates/invalidation/replacement in the anchored registry journal; identityd owns validation, not another restorable store. Consistent authority snapshots, journal-sequence answers and unknown/unavailable denial are specified. Vault files confer no authority. Implementation must prove current-snapshot freshness, including stale caches, service restarts and concurrent restrictive changes; runtime NOT_RUN. |
| R06 | **PASS (oracle/design)** | Wire §6.9, CDDL `identity_change`, oracle `:801–812`, ADR 0004 §7a / ADR 0008 §7 bind every ordinary successor to the authoritative subject; only an explicit displayed transition can change package/executable. Own re-signed substitution is rejected with `admission/subject_binding_mismatch`; published substitution vectors and positive transition pass. Rust admission NOT_RUN. |
| R07 | **FAIL — original cases fixed, new branch gap** | Network grants now carry IDs/lifetimes/restriction/invalidation/restore semantics; legacy zero-grant Direct state and network restore mismatch reject. CDDL and wire §6.10 limit each individual state to one grant. They do not limit the union of live branches: R07-r2 below. Runtime networking NOT_RUN. |
| R08 | **PASS (oracle/design)** | Separate 61,440-byte envelope / 65,536-byte wrapper limits, 4,096-byte metadata budget, one-object pagination, and separate embedded decoding are specified. Independently verified maximum and one-byte-over vectors; minimal IPC wrapper is 61,533 bytes; published journal wrapper is 61,619. Actual IPC/journal decoders NOT_RUN; the latter is a published wrapper example, not a runtime proof for every future record layout. |
| R09 | **FAIL — original idle case fixed, model incomplete** | BOOTTIME baseline fixes rollback after an idle interval/suspend; latch and ledger preserve observed expiry; temporary grants end on new boot. But the rule does not advance from a later forward sample, and floors store raw realtime: R09-r2 below. Clock implementation NOT_RUN. |
| R10 | **PASS (design)** | ADR 0006 §2 names one authoritative in-memory global anchor map/lock, user→anchor lock order, whole-map slot writes and durability before acknowledgment, including group commit. This resolves the per-user-lock lost update. Actual concurrent/torn/failed slot and tombstone tests NOT_RUN. |
| R11 | **PASS (design)** | ADR 0006 §7 / ADR 0008 §4 order denial/ledger → durable deletion_pending and pending anchor tombstone → idempotent erasure → durable deleted tombstone → success. Failed pre-erase writes destroy nothing; pending restart stays denied; UID reuse gets a new user_id; tombstones retained. Actual crash/restore/erase tests NOT_RUN. |
| I01 | **PASS (documentation)** | ADR 0006 §2 now explicitly labels backends planned, unimplemented and untested. No anchor runtime PASS granted. |
| I02 | **PASS** | Qualification builds and actually runs the release secure-memory test executable under Valgrind; 10 tests and 0 errors. The exec'd child-fault processes remain uninstrumented, explicitly documented. |
| I03 | **PASS (scope documentation)** | ADR 0005 §§3,7–8 preserves callable-by-same-UID development-client limits, terminal exposure and absence of trusted input/display/human-presence proof. Fixtures establish no attribution, sandbox or network enforcement. |

## Actionable findings

### R01-r2 — Medium: failed Cargo.lock hashing still produces source-identity PASS

**Location:** `build/scripts/m2_source_identity.sh:45`, `:66–68`; caller `build/scripts/qualify_m2_rust.sh:78`.

**Trigger:** a lock exists but `sha256sum` fails (unreadable input, I/O/tool failure). The command substitution is a pipeline without pipefail; successful `cut` masks the failed hash command. There is no 64-hex validation of `lock_digest`.

**Reproduction:** on the exact clean candidate, put a synthetic `sha256sum` returning exit 7 with no stdout first in PATH and call the identity script with `--require-clean`. Both `sh` and the qualification's actual `bash` interpreter return 0, `dirty_tracked 0`, an empty `cargo_lock_sha256`, and `identity PASS`. The native pinned-container reproduction also returns 0. An unreadable-lock disposable fixture likewise loses its digest; clean-tree enforcement can separately fail for its dirty status, which does not fix the hash handling.

**Rationale:** the original requirement rejects missing/failed source identity even when later commands pass. The no-Git fix is real, but a recorded PASS still need not establish the lock identity. This controlled tool failure does not invalidate the ordinary clean qualification digest, which was independently checked.

**Required:** collect and check the hash command's own status before extracting its field; validate the resulting digest; add failed-hash/unreadable-lock canaries. Do not rely on a later Cargo/source check to correct a false source_identity result.

### R07-r2 — Medium: per-state network uniqueness permits conflicting live sibling grants

**Location:** `docs/specs/m2_wire_format_v1.md:206`; `docs/decisions/0007_m2_lifecycle_time_consumption.md:20`, `:27`; implementation evidence `tests/m2/oracle/ecr_oracle.py:707`.

**Trigger:** an Offline current trunk receives two signed temporary siblings, one timed Direct grant and one timed Tor grant, with overlapping lifetimes and the same trunk root. Each branch's result has exactly one network grant and its base has none. Neither branch is merged/revoked/terminated, so both satisfy the stated live-branch and grant eligibility rules.

**Reproduction:** appendix probe constructs and admits both signed siblings; the oracle accepts them. The design then requires choosing the lineage's “single eligible network grant,” although two qualify. This example does not depend on the oracle omitting expected-head checks: both roots are the same still-current trunk, and sibling temporary branches are an intended DAG feature.

**Rationale:** PROJECT §§3.2,19 require default-deny and one effective mode per application. The proposal supplies no rule to reject, replace, terminate or safely arbitrate conflicting live network branches. Picking a mode by iteration order would make approved Tor/Direct behavior ambiguous. No actual enforcement exploit is claimed: the network/policy services do not exist.

**Required:** freeze uniqueness/conflict semantics over the trunk plus all live temporary heads, under the admission lock, including overlap, simultaneous proposals, session/timed expiry and revocation. If conflicts can exist, explicitly define conservative evaluation and transition behavior. Add signed positive/negative multi-branch vectors.

### R09-r2 — Medium: later forward time samples are not rebased or persisted as trusted time

**Location:** `docs/decisions/0007_m2_lifecycle_time_consumption.md:55–56`, `:58–59`.

**Trigger / design counterexample:** start at 12:00, BOOTTIME 0. At BOOTTIME 60 s observe wall time 14:00, then wall time is corrected back. An hour later, the two prescribed lower bounds allow `high_water = max(14:00, 12:00 + 3,660 s, corrected wall time) = 14:00`: the later observed time has stopped advancing. A grant expiring at 14:30 can remain eligible despite an hour elapsed after the 14:00 observation. The anomaly record does not change this bound, and expiry was never observed so its latch does not help.

There is a second same-boot restart issue: `time_floor` stores `{realtime, boot_id, boottime}`, not the trusted high-water value. After that correction, a later floor/clean shutdown can store raw 13:00 even though trusted time is at least 14:00. A service restarting from the last floor can move trusted time backward. The text specifies neither preserving the maximum rebased floor nor prohibiting regressing raw floors. This is inside one boot, outside the stated cross-boot hardware-clock limitation.

**Rationale:** BOOTTIME-at-start repairs the original idle scenario but does not define a clock that continues from each accepted forward observation and survives process restart. Non-decreasing alone is insufficient when deadlines freeze. The appendix executes the literal lower-bound counterexample; no service clock implementation has run.

**Required:** define trusted-time/BOOTTIME rebasing after every accepted sample, persist the trusted value paired with its BOOTTIME (or an equivalent maximum-offset rule), and share consistent floors between services. Include forward step→backward correction→long idle/suspend and same-boot restart/clean-shutdown cases. Retain the explicit new-boot limits.

### R12 — Medium: restore oracle converts historical session authority to persistent authority

**Location:** `tests/m2/oracle/ecr_oracle.py:791–798`; violated specification `docs/specs/m2_wire_format_v1.md:191`, ADR 0004 item 7.

**Trigger:** set a new restore's reference to the published `commit_temporary_session` (an actual ancestor of its current parent through `commit_merge`), copy that session microphone grant's resource/delegation/type with a fresh grant ID, and replace its lifetime with persistent and no session or expiration. Sign with the current persistent fixture key and correct base/result/request.

**Reproduction:** the appendix admits the published six-commit prefix plus this envelope successfully. The matching predicate compares resource/delegation/contract_type, then only checks historical expiration. It never requires the historical grant's lifetime kind to be persistent. A session grant without expiration therefore matches.

**Rationale:** wire §6.7 explicitly says temporary grants, ended sessions and consumed one-shots are never restored. Fresh persistent scope may be separately approved as an ordinary revision; labeling this as a permitted historical restore bypasses the proposal's stated restriction. This is a vector/oracle mismatch, not an exploit in a Rust registry. Existing restore vectors test network scope and expiration but miss historical lifetime kind.

**Required:** align the restore predicate with the prohibition and add session/timed/one-shot negative vectors and persistent-history controls, including actual ancestor references. Preserve distinct explicit approval for making temporary scope persistent through an allowed transaction type.

### R13 — Medium: containment recovery requires signatures whose keys cannot resolve

**Location:** `docs/decisions/0006_m2_storage_anchor_recovery.md:88–89`, `:112`; `docs/specs/m2_service_ipc_v1.md:40`; related ADR 0005 §2 and verification plan.

**Trigger:** a reboot after an unclean run contains the user. Option C1 requires freshly approved signed restores per lineage to lift suspension. But identityd must answer unknown/denied for a contained user, and vaultd must check key status before every signature; its verification plan explicitly requires refusing signing for contained keys. A restore also requires authenticated current key authority at verification/admission.

**Rationale:** the proposed user recovery path cannot perform its prerequisite. C2/admin recovery also needs to define exactly when a user becomes permitted to sign fresh reauthorization while old authority stays denied. The existing text freezes neither such a state nor a narrowly permitted operation. Inferring a generic bypass for signing or lifting all containment before approval would weaken the denial rule. This is an availability/architecture contradiction, not a demonstrated fail-open implementation.

**Required:** separate grant containment from the key authority needed for a tightly bound recovery transaction, or define an explicit staged ceremony. Specify permitted kinds/roles, fresh approval, current uncompromised key checks, journal/anchor ordering, per-lineage progress and crashes; old grants must stay denied throughout. Add positive recovery plus stale/compromised/deleted-key and unauthorized-bypass negatives. Owner options should refer to this complete ceremony.

## Executed qualification and check ledger

Every result uses PASS / FAIL / NOT_RUN / BLOCKED. `NOT_AVAILABLE` in raw qualification output maps to **NOT_RUN** here; it is never a memory-safety PASS. Command variables below designate the exact paths used, not unspecified future inputs.

### Checkout / environment / immutable scope

| Status | Exact command / location | Evidence |
|---|---|---|
| PASS | `/workspace/cham3`: `git status --short`; `git branch --show-current`; `git log -5 --oneline`; `git remote -v` | Initially clean at 9cf8825 on work; intended origin confirmed. |
| PASS | `git fetch origin codex/m2-cryptographic-contract-core` | Requested immutable objects fetched. No push. |
| PASS | `git worktree add --detach /tmp/cham3_m2_r2_source 6b4ef9d87d69185ff7205b51fa417df1fed5c6f6` | Source/handoff review checkout. |
| PASS | `git clone --no-hardlinks /workspace/cham3 /tmp/cham3_m2_r2_run`; `git -C /tmp/cham3_m2_r2_run checkout --detach a2bbb370c867da57fdc72274fb17ca19a23426e4` | Self-contained Git clone for container mount; initially clean. |
| PASS | In source: `git diff --stat 3c35aac a2bbb37`; `git diff --name-only a2bbb37 6b4ef9d` | 32 candidate files; follow-on only docs/evidence and handoff. |
| PASS | `id`; `uname -a`; `cat /etc/os-release`; `cat /etc/codex/network-policy.json` | UID 1000, native x86_64; restricted proxy policy, no VPN. |
| PASS | `env -u DOCKER_HOST -u DOCKER_CONTEXT -u DOCKER_TLS -u DOCKER_TLS_VERIFY -u DOCKER_CERT_PATH docker --host=unix:///var/run/docker.sock info --format '{{.Architecture}} {{.Driver}}'` | Local x86_64 vfs daemon. |
| PASS | In run clone: `docker build -f build/m2/dev_container.dockerfile -t cham3-m2-r2-review build/m2` | Unchanged pinned Dockerfile/context; build log `/tmp/cham3_m2_r2_evidence/docker_build.log`. |
| PASS | `sha256sum build/m2/debian_lock_amd64.txt build/m2/debian_debs_amd64.sha256` | `6cd45590bdebd08f53c95a56112a97030b844959983284db44232b59baa11d3c` / `6ce06e450a8b91cd2c91c1d3aaffc360dfed7276f6622ac03dbd05fd35f0f43a`; 612 / 535 lines. Match prior reviewer proposals. |

### Native amd64 qualification — PASS, exit 0

Executed from `/tmp/cham3_m2_r2_run` with the handoff's command, changing only the image tag and using an absolute mount:

```sh
docker run --rm --user "$(id -u):$(id -g)" \
  -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
  -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory \
  -e GIT_CONFIG_VALUE_0=/work \
  -v /tmp/cham3_m2_r2_run:/work -w /work cham3-m2-r2-review \
  bash build/scripts/qualify_m2_rust.sh
```

Evidence: `/tmp/cham3_m2_r2_evidence/qualification.log`; `run_dir=/work/out/m2/qualification/amd64_20261012T172917Z_1` in the container (host prefix `/tmp/cham3_m2_r2_run`). Clean candidate/tree as above, Debian 13.7, EUID 1000, Cargo.lock SHA-256 `fc87901c5e58acdba21a33fe516871e45a81072585bfa2ae82aa8e60c40310f6`. No `--record` or proposed/uncommitted inputs used.

| Status | Check / exact nested command | Observed result |
|---|---|---|
| PASS | `bash build/scripts/m2_source_identity.sh "$run_dir/identity.txt" --require-clean` | Real clean candidate identity and valid lock digest; failure-path defect R01 is separately reproduced. |
| PASS | `cat /etc/debian_version`; `dpkg --print-architecture`; `id -u` | Debian 13.7 / amd64 / 1000. |
| PASS | `rustc -Vv`; `cargo -V`; `cc --version`; `ld --version`; `ldd --version` | rustc/Cargo 1.85.1, LLVM 19.1.7, GCC 14.2.0-19, ld 2.44, glibc 2.41-12+deb13u4. |
| PASS | `dpkg-query -W -f '${Version}' "$package"` for each package in `build/m2/debian_packages.txt` | All explicit pins match. |
| PASS | `dpkg-query -W -f '${Package}:${Architecture}=${Version}\n' \| LC_ALL=C sort`; `diff -u build/m2/debian_lock_amd64.txt "$run_dir/installed_packages.txt"` | Full installed lock identical; empty diff. |
| PASS | `cp /usr/local/share/cham3/m2/downloaded_debs.sha256 "$run_dir/downloaded_debs.sha256"`; `diff -u build/m2/debian_debs_amd64.sha256 "$run_dir/downloaded_debs.sha256"` | Downloaded .deb lock identical; empty diff; nonempty recorded list. |
| PASS | `cargo metadata --locked --offline --format-version 1` | Offline locked metadata. |
| PASS | `python3 -I build/scripts/m2_dependency_inventory.py sources --metadata "$run_dir/metadata.json" --check build/m2/crate_sources.sha256` | All 1,454 dependency files match. |
| PASS | `python3 -I build/scripts/m2_dependency_inventory.py inventory --metadata "$run_dir/metadata.json" --out-dir "$run_dir"` | 33 compiled dependency crates; provenance/features/build/unsafe locators. |
| PASS | `cargo fmt --all --check` | Formatting. |
| PASS | `env CARGO_TARGET_DIR="$run_dir/clippy_target" cargo clippy --workspace --all-targets --frozen -- -D warnings` | Lints. |
| PASS | `python3 -I build/scripts/check_rust_naming.py` | Naming. |
| PASS | `python3 -I -` with unchanged profile-check heredoc in `build/scripts/qualify_m2_rust.sh:180–195` | Overflow checks in all profiles; release abort/dev unwind. Exact heredoc is pinned by candidate. |
| PASS | `env CARGO_TARGET_DIR="$run_dir/build_target" cargo build --workspace --frozen` | Fresh debug build. |
| PASS | `env CARGO_TARGET_DIR="$run_dir/build_target" cargo build --workspace --release --frozen --verbose` | Fresh release build. |
| PASS | Qualification's unchanged `grep -Eq -- "$expected" "$run_dir/build_release.log"` loop and `grep -l 'curve25519_dalek_backend="fiat"' "$run_dir/build_target"/*/build/curve25519-dalek-*/output` | Actual invocation flags: chacha20/poly1305 force_soft, sha2 force-soft, curve25519 fiat; 2 fiat build-script outputs. Exact patterns pinned in `qualify_m2_rust.sh:202–217`. |
| PASS | `valgrind --error-exitcode=99 --track-origins=yes "$run_dir/build_target/release/cham3_provider_probe" report` | Provider report, 0 errors. |
| PASS | `env CARGO_TARGET_DIR="$run_dir/build_target" cargo test --release --frozen -p cham3_secure_memory --no-run --message-format=json`; `valgrind --error-exitcode=99 --track-origins=yes "$sm_exe" --test-threads=1` | Compiler-artifact-selected actual test executable, 10 passed, 0 failed/ignored/filtered, 0 errors. Exec'd children not instrumented; Cargo test harness unwinds. Deployed abort is separately tested by harness. |
| PASS | `sh tests/m2/run_m2_tests.sh unit` | Run `unit_20261012T173101Z_1601`: 36 debug + 36 release tests, inventory exact, 7 provider checks/native-oracle agreement; ECR 18 valid / 90 invalid / 14 admission-negative. Nested actual commands include `cargo test --workspace --frozen`, same with `--release`, and `python3 -I tests/m2/oracle/ecr_oracle.py check tests/m2/vectors/ecr_v1/vectors.json`. |
| PASS | `sh tests/m2/run_m2_tests.sh harness` | Run `harness_20261012T173147Z_2683`: 70 tests, actual assertion/compile/link/panic/zero/ignore/filter/compile-only, unsafe/naming, identity and deployed panic/overflow controls. Actual runner: `python3 -I -m unittest discover -v -s /work/tests/m2/harness -p 'test_*.py'`. |
| NOT_RUN | `cargo miri --version` availability probe failed; no Miri analysis command | Missing pinned toolchain support. |
| NOT_RUN | Sanitizer execution / `cargo fuzz run <target>` | Nightly/targets unavailable; never PASS. |

### Independent probes, regression and advisory checks

For the commands below define the actual invocation prefixes:

```sh
review_run() {
  docker run --rm --user 1000:1000 \
    -v /tmp/cham3_m2_r2_run:/work:ro \
    -v /tmp/cham3_m2_r2_tools:/review:ro \
    -w /work cham3-m2-r2-review "$@"
}
export_run() {
  docker run --rm --user 1000:1000 \
    -v /tmp/cham3_m2_r2_export:/work -w /work cham3-m2-r2-review "$@"
}
```

These abbreviate the identical literal Docker prefixes used; no test privileges were added. The original two reviewer scripts were extracted, unchanged, from the prior review archive into `/tmp/cham3_m2_r2_tools/cham3_m2_review_evidence/`.

| Status | Exact command | Evidence / limit |
|---|---|---|
| PASS | `tar -xzf docs/reviews/M2-001-002-codex-security-review-evidence.tar.gz -C /tmp/cham3_m2_r2_tools cham3_m2_review_evidence/independent_vectors.py cham3_m2_review_evidence/schema_holes.py` | Portable scripts recovered from preserved prior review. |
| PASS | `review_run python3 -I /review/cham3_m2_review_evidence/independent_vectors.py tests/m2/vectors/ecr_v1/vectors.json` | All **18** objects' canonical envelope/header/payload, signatures, envelope/lineage hashes and altered-preimage rejections. Script's final text hardcodes 17 from revision 1; its actual loop and 18 per-object lines establish 18. cbor2 5.6.5, cryptography 43.0.0, OpenSSL 3.5.7. Own preimage assembly; no author-oracle import, no independent full policy/strict-Ed25519 proof. |
| PASS (expected rejection) | `review_run python3 -I /review/cham3_m2_review_evidence/schema_holes.py tests/m2/oracle/ecr_oracle.py tests/m2/vectors/ecr_v1/vectors.json` | Exit 1 at first original case, `admission/subject_binding_mismatch`; remaining original cases do not execute in this old script. |
| PASS (original regressions); FAIL (new semantic expectations) | `review_run python3 -I /review/r2_probes.py tests/m2/oracle/ecr_oracle.py tests/m2/vectors/ecr_v1/vectors.json` | Exit 0: all four original holes checked separately and rejected / boundary fixed; two additional signed cases are ACCEPTED (R07-r2/R12); time counterexample (R09-r2). Script in appendix. Deliberately uses author oracle to expose its behavior, not independent policy implementation. |
| PASS (negative control) | In run clone: `mkdir -p /tmp/cham3_m2_r2_export`; `git archive a2bbb370c867da57fdc72274fb17ca19a23426e4 \| tar -x -C /tmp/cham3_m2_r2_export`; `export_run python3 -I tests/m2/lib/m2_harness.py environment --run-dir out/reviewer_missing_git`; `export_run sh build/scripts/m2_source_identity.sh out/no_git_identity.txt` | Each collector exits 1; harness records FAIL with Git status 128 and unknown dirty state. No exported implementation was changed. |
| FAIL (identity requirement) | `PATH="/tmp/cham3_m2_r2_tools/failing_sha:$PATH" bash build/scripts/m2_source_identity.sh /tmp/cham3_m2_r2_evidence/failed_hash_candidate_identity.txt --require-clean` in clean run clone | Exit 0, empty digest, identity PASS. Failing tool is a synthetic script printing its injected-failure label and exiting 7. Native-container exact variant below. |
| PASS | `docker run --rm --user 1000:1000 -v /tmp/cham3_m2_r2_run:/work -w /work cham3-m2-r2-review bash build/scripts/run_static_checks.sh` | shellcheck 24 files, Python compile 60, yamllint 5, configured secret scan. |
| PASS | `docker run --rm --user 1000:1000 -v /tmp/cham3_m2_r2_run:/work -w /work cham3-m2-r2-review sh tests/run_tests.sh static` | 82 tests; candidate regression. |
| BLOCKED | `curl --fail --silent --show-error https://security-tracker.debian.org/tracker/data/json -o /tmp/cham3_m2_r2_evidence/debian_tracker.json` | HTTP 403 under executor policy; no snapshot bytes obtained, no policy bypass. Cannot independently clear current Debian advisories or adopt author's triage. |
| PASS | `git clone https://github.com/RustSec/advisory-db.git /tmp/cham3_m2_r2_tools/advisory-db`; `git -C /tmp/cham3_m2_r2_tools/advisory-db rev-parse HEAD` | Independently fetched commit `25a288f68f2d5ebe658b243a334110c7217a2569`, same identity as author snapshot. |
| PASS (RustSec only) | `docker run --rm --user 1000:1000 -v /tmp/cham3_m2_r2_run:/work -v /tmp/cham3_m2_r2_tools:/review:ro -w /work cham3-m2-r2-review python3 -I /review/rustsec_only.py out/m2/qualification/amd64_20261012T172917Z_1/dependency_inventory.json /review/advisory-db` | Own live database + own native inventory, 37 crate versions, 6 nonwithdrawn advisories, 0 affected. Uses candidate script's range predicate; independently inspected all six displayed patched ranges. No Debian result inferred. |
| BLOCKED | `docker run --rm --user 1000:1000 -v /tmp/cham3_m2_r2_run:/work -v /tmp/cham3_m2_r2_tools:/review:ro -v /tmp/cham3_m2_r2_evidence:/evidence -w /work cham3-m2-r2-review python3 -I build/scripts/m2_advisory_review.py --tracker /evidence/debian_tracker.json --advisory-db /review/advisory-db --inventory out/m2/qualification/amd64_20261012T172917Z_1/dependency_inventory.json --out-dir /evidence/advisory_full` | Attempted exit 1: missing tracker after blocked fetch. Full combined advisory review did not execute successfully. Author's Debian 0-behind/23-untriaged result remains author evidence. |
| NOT_RUN | `sh tests/m2/run_m2_tests.sh integration`; `sh tests/m2/run_m2_tests.sh recovery`; `sh tests/m2/run_m2_tests.sh packaging` as product acceptance | Domain/backend/package implementation gated. Harness exercised dispatcher NOT_RUN behavior only. |
| NOT_RUN | `sh build/scripts/build_m2_core.sh` as separate entry point | Its underlying fresh Cargo builds ran through qualification; no separate entry-point claim. |
| NOT_RUN | Actual A04–A15 identity/vault/IPC/policy/journal/anchor/recovery/packaging/enforcement and M1 boot/desktop suites | Skeletons and design review, no current runtime product proof; static regression only. |
| PASS | `git diff --exit-code` in run clone; `git -C /tmp/cham3_m2_r2_source diff --exit-code` | Candidate and handoff source unchanged after checks. |
| PASS | In publication worktree: `git diff --cached --check`; `git diff --cached --name-only` | Review record has no whitespace errors; the staged change is this one path under docs/reviews only. |

Native-container failed-hash command (FAIL requirement, actual exit 0):

```sh
docker run --rm --user 1000:1000 \
  -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory \
  -e GIT_CONFIG_VALUE_0=/work \
  -v /tmp/cham3_m2_r2_run:/work:ro \
  -v /tmp/cham3_m2_r2_tools:/review:ro \
  -v /tmp/cham3_m2_r2_evidence:/evidence -w /work cham3-m2-r2-review \
  bash -c 'PATH="/review/failing_sha:$PATH" bash build/scripts/m2_source_identity.sh /evidence/failed_hash_container_identity.txt --require-clean'
```

Synthetic `failing_sha/sha256sum` (created outside the repo; executable):

```sh
#!/bin/sh
printf '%s\n' 'reviewer injected sha256sum failure' >&2
exit 7
```

### CI run 38283264875 comparison

| Status | Exact connector call / command | Independently observed evidence |
|---|---|---|
| PASS | `github_fetch_workflow_run_jobs(repo_full_name="Kayyo321/cham3", run_id=38283264875)` | `qualify_amd64` job `114925416377`: completed, success, all steps success. |
| PASS | `github_fetch_workflow_job_logs(repo_full_name="Kayyo321/cham3", job_id=114925416377)` | Independently obtained decoded logs establish candidate a2bbb37, clean source identity, native amd64, Debian 13.7, UID 1001, kernel 6.17.0-1022-azure. Every required qualification check PASS, including both committed locks and actual secure-memory Valgrind. Raw decoded log retained `/tmp/cham3_m2_r2_evidence/ci_job.log`. |
| PASS | `github_fetch_workflow_run_artifacts(repo_full_name="Kayyo321/cham3", run_id=38283264875, name="m2_qualification_amd64")` | Artifact `11762796075`, 84,281 bytes, SHA-256 `6e10bd1fd312af8ba4742460e432b33a097300998355741c4d3d0943d226bb56`, workflow head a2bbb37. Metadata inspected, not ZIP bytes. |
| PASS | `diff -u /tmp/cham3_m2_r2_source/docs/evidence/M2-cryptographic-contract-core/m2_001_amd64_ci_a2bbb37/summary.txt /tmp/cham3_m2_r2_run/out/m2/qualification/amd64_20261012T172917Z_1/summary.txt` | Empty diff. Independent remote log's summary agrees with this checked-in CI summary and local results. 36+36 / 70 / 18-90-14 inventories agree. |
| NOT_RUN | CI artifact ZIP download/byte comparison | No claim to have inspected this ZIP, nor the revision-1 CI ZIP. Local observed lock bytes and decoded CI lock PASS are the comparison evidence. |

## Scope, evidence and next action

R01, R07 and R09 remain partially open; R02/R03/R04/R05/R06/R08/R10/R11 and I01–I03 address their original concerns within the explicit probe/design scope. R12 and R13 are additional gaps. None of the design closures is an A04–A15 runtime PASS. The unsafe adapter remains unchanged; inspected ownership/layout/checked bounds/guard protections/RAII/zeroization plus executed tests and Valgrind found no new confirmed memory-unsoundness defect. Dependency inventory and advisory absence do not prove every transitive unsafe path or side-channel property sound.

The native qualification is a full PASS and the reviewed amd64 lock bytes are now committed. The blocked Debian retrieval is an evidence limitation, not a reported vulnerability in a package; complete its independent review before G1 approval. RustSec-only evidence does not replace that review. Current untriaged system/tooling advisories need applicability judgment before packaging; no independent Debian clearance is granted here.

Raw local logs/tools are under `/tmp/cham3_m2_r2_evidence`, `/tmp/cham3_m2_r2_tools`, and the execution clone's ignored `out/`. They were not added to implementation or evidence paths. The portable new semantic probe is included below; the old independent scripts remain in the prior preserved review archive. Reviewer writes are this record only under `docs/reviews/`; no candidate implementation/spec/ADR/vector changes, no push, no merge, no owner options selected.

Implementer next: fix the five actionable items, reconcile specs/ADRs/oracle/vectors, add discriminating controls, repeat affected checks and provide exact new candidate/evidence commits. Reviewer next: re-review those changes and complete the blocked advisory evidence when access is available. Owner decisions follow independent review; neither qualification PASS nor publication of this record authorizes dependent implementation.

## Appendix: portable new probe

Save the following as `r2_probes.py` outside the repository and run with the exact command above. It asserts the original four repairs and prints accepted new cases. Its exit 0 means the reproductions completed, **not** that the reproduced behavior satisfies the design.

```python
import copy, hashlib, importlib.util, json, sys
spec = importlib.util.spec_from_file_location('oracle', sys.argv[1])
o = importlib.util.module_from_spec(spec); spec.loader.exec_module(o)
x = json.load(open(sys.argv[2]))
keys = {k['label']: o.key_t(k['label']) for k in x['keys']}
resolver = o.resolver_t(list(keys.values()))
valid = {v['name']: bytes.fromhex(v['envelope']) for v in x['valid']}
signer = keys['user_1001_persistent']
def payload(name): return o.verify(valid[name], 'commit', resolver)
def reject(name, callback, layer, code):
    try: callback()
    except o.oracle_error_t as error:
        assert (error.layer, error.code) == (layer, code), str(error)
        print('PASS rejection', name, error.layer, error.code)
    else: raise AssertionError(name + ' accepted')
gen = payload('commit_genesis')
rev = copy.deepcopy(payload('commit_revision'))
rev['subject']['package_identity'] = 'deb:attacker:amd64'; rev['subject']['executable_sha256'] = b'\x42'*32
env, _ = o.sign_commit(signer, rev)
reject('R06', lambda: o.admit_chain([valid['commit_genesis'], env], resolver), 'admission', 'subject_binding_mismatch')
net = copy.deepcopy(gen)
net['delta'] = {'add_grants': [], 'remove_grant_ids': [], 'network_mode': {'from': 'offline', 'to': 'direct'}}
net['result_state'] = {'grants': [], 'network_mode': 'direct'}
env, _ = o.sign_commit(signer, net)
reject('R07 legacy unbounded network', lambda: o.verify(env, 'commit', resolver), 'schema', 'unknown_field')
case = next(v for v in x['admission_invalid'] if v['name'] == 'restore_network_not_in_snapshot')
chain = [valid[n] for n in x['lineage']['commits']]
reject('R07 restore network', lambda: o.admit_chain(chain[:case['prefix']] + [bytes.fromhex(case['envelope'])], resolver), 'admission', 'restore_rule')
boundary = valid['commit_boundary_max_size']; o.admit_chain([boundary], resolver)
wrapped = o.encode({'op': 'submit_signed', 'request_id': b'\x00'*16, 'args': {'proposal_id': b'\x00'*16, 'envelope': boundary}})
assert len(boundary) == 61440 and len(wrapped) == 61533
over = next(v for v in x['invalid'] if v['name'] == 'boundary_one_byte_over')
assert len(bytes.fromhex(over['envelope'])) == 61441
reject('R08 size', lambda: o.verify(bytes.fromhex(over['envelope']), 'commit', resolver), 'envelope_decode', 'input_too_large')
print('PASS maximum envelope', len(boundary), 'IPC', len(wrapped))
# Two network grants on two live sibling branches, each rooted at the current Offline trunk.
sibling_envelopes = []
for mode in ('direct', 'tor'):
    p = copy.deepcopy(gen)
    p['commit_type'] = 'temporary_grant'; p['parents'] = [hashlib.sha256(valid['commit_genesis']).digest()]
    p['base_state_hash'] = o.state_hash(gen['result_state']); p['timestamp'] = o.Times[1]
    p['authorization']['request_id'] = o.synthetic_id('review sibling ' + mode)
    grant = o.make_grant('review network '+mode, 'network', 'network_mode', [mode], kind='timed', start=o.Times[1], expiration=o.Times[11])
    p['delta'] = {'add_grants': [grant], 'remove_grant_ids': []}; p['result_state'] = o.apply_delta(gen['result_state'], p['delta'])
    env, _ = o.sign_commit(signer, p); sibling_envelopes.append(env)
o.admit_chain([valid['commit_genesis']] + sibling_envelopes, resolver)
print('ACCEPTED overlapping sibling Direct/Tor temporary network grants, common current Offline root')
# Restore a session-only grant as persistent. The reference is an actual ancestor via merge.
p = copy.deepcopy(payload('commit_restore'))
old = payload('commit_temporary_session')['delta']['add_grants'][0]
restored = copy.deepcopy(old); restored['grant_id'] = o.synthetic_id('review restored session as persistent')
restored['lifetime'] = {'kind': 'persistent', 'effective_start': o.Times[6]}
p['restore_reference'] = hashlib.sha256(valid['commit_temporary_session']).digest()
p['authorization']['request_id'] = o.synthetic_id('review temporary restore')
p['delta'] = {'add_grants': [restored], 'remove_grant_ids': []}
p['result_state'] = o.apply_delta(payload('commit_revocation')['result_state'], p['delta'])
env, _ = o.sign_commit(signer, p)
o.admit_chain(chain[:6] + [env], resolver)
print('ACCEPTED restore of ancestral session grant as persistent without session/expiration')
# Literal trusted-time lower bounds: a forward observation is not rebased to boottime.
start_rt, start_bt = 12*3600, 0
high = max(start_rt + 60, 14*3600)
later_bt, later_rt = 3660, 13*3600 + 60
high = max(high, start_rt + later_bt, later_rt)
assert high == 14*3600 and high < 14*3600 + 30*60
print('DESIGN COUNTEREXAMPLE time freezes at 14:00 after forward sample then correction; 14:30 expiry remains eligible an hour later under stated inequalities')
print('PROBE COMPLETE; accepted cases are oracle/design gaps, not runtime exploits')
```

RustSec-only helper (`rustsec_only.py`, outside the repository):

```python
import importlib.util, json, os, re, sys, tomllib
spec = importlib.util.spec_from_file_location('review', '/work/build/scripts/m2_advisory_review.py')
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
inventory = json.load(open(sys.argv[1]))
crates = {(r['name'], r['version']) for r in inventory} | m.TestOnlyCrates
rows = []
for name, version in sorted(crates):
    directory = os.path.join(sys.argv[2], 'crates', name)
    if not os.path.isdir(directory): continue
    for entry in sorted(os.listdir(directory)):
        data = tomllib.loads(re.search(r'```toml\n(.*?)\n```', open(os.path.join(directory, entry)).read(), re.S).group(1))
        if data['advisory'].get('withdrawn'): continue
        versions = data.get('versions', {})
        cleared = m.any_range(version, versions.get('patched', [])) or m.any_range(version, versions.get('unaffected', []))
        rows.append((name, version, data['advisory']['id'], cleared, versions))
        print(*rows[-1])
assert all(r[3] for r in rows)
print('PASS RustSec-only:', len(crates), 'crate versions,', len(rows), 'advisories, 0 affected; Debian review BLOCKED, not inferred')
```
