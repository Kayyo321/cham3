# M2-006 stage A revision 2 — independent Codex review

- **Verdict: CHANGES_REQUESTED.** The original eight regression tests pass, but four additional recovery/acceptance findings below prevent stage-A approval. No owner decision is made and stage B is not approved by this record.
- Reviewer/role: Codex, independent REVIEWER; no implementation authored or modified.
- Candidate: `e34fd8a0b97f5a606656ab87914d4d1b19517d7c`; base `a9e47e47660879af86ede99986b25a18ffb537aa`; exactly one implementation commit.
- Documentation reviewed: `fc147e0f80188a82a761ffe203a6ac7704ea25f9`, direct child of e34fd8a, docs-only. Previous review: `344c37210ff15f297cf015ee047d8e147efe2fbd`.
- Implementation checkout: `/home/dev/cham3`, clean at fc147e0 after the requested fetch/checkout/reset; no local changes or stashes were moved or discarded. It remains on the implementation branch, untouched.
- Publication: only this record, branch `review/m2-006-a-r2-security`, based on `origin/main` at `9cf882503848f8c6cc4938a588cc9fc062a9392d`; worktree `/home/dev/cham3-review-r2/record`. No merge or force-push.
- Authority: PROJECT.md, approved ADR 0006 §§2–4 and §6, task scope, engineering workflow/testing/security invariants. Author resolutions, storage/IPC specifications and handoff are claims assessed against that authority.

## Findings

### M2-006-A-R2-01 — Medium — Equal-counter quarantine disappears at the next restart

Location: `core/registryd/src/anchor.rs:486` (through 507); startup publication `core/registryd/src/registry.rs:865` (through 879).

Two checksum-valid slots with equal counters are correctly rejected on the first start. However, rejection resets the replacement anchor counter to zero and leaves the old slots on disk. Startup writes only slot A with counter 1 and a fresh map. Slot B retains the previously rejected higher counter and normal/clean map. After another unclean same-boot restart, slot B wins; the quarantine and permanently ended grant state disappear without recovery authorization.

Independent debug/release reproduction uses equal counter 100: first start reports `anchor_invalid`, counter 1, grant ended/quarantined; second reports `resumed`, no quarantine, `Ok(current)`. The control uses unequal counters 100/101 and resumes normally twice. This is a protected-storage corruption/recovery scenario, not proof an ordinary application can write anchor files.

**Required:** make invalid/equal-slot rejection and the resulting quarantine durable across repeated starts, without permitting a rejected surviving slot to become authoritative. Test both slot orders and repeated unclean/clean starts. Do not solve this by accepting equal counters.

### M2-006-A-R2-02 — Medium — Startup overwrites its own unverifiable marker

Location: `core/registryd/src/registry.rs:1247` (marker path through 1266), conflicting startup publication at `:865–879`.

During same-boot ledger replay, failure to append the intent to the new ledger invokes `mark_run_unverifiable`. Even when the anchor write succeeds, `open` later unconditionally publishes a new `running` marker. If validity unlink and truncation also fail and the restrictive journal append fails, the returned process is denied, but its durable evidence is now a valid empty ledger plus `running`. A second same-boot restart resumes the old unrestricted journal.

Independent debug/release tests permit ledger creation, then inject failure at the first new-run ledger append; subsequent validity invalidation and journal writes fail, while **anchor writes remain successful**. Before the next drop the marker is `running`; next start is `ledger_replayed` and the grant is `current`. Allowing validity invalidation is the positive control: next start contains and suspends the grant. This is distinct from the documented all-anchor-writes-fail residual.

**Required:** never publish a recoverable `running` state after replay/containment has made the run unverifiable. Preserve the failure marker or refuse successful startup until durable restart safety is established. Add this failure during startup, not only during normal operation, to the tests and mutation harness.

### M2-006-A-R2-03 — Medium — Documented compound-failure residual loses a restriction

Location: `core/registryd/src/ledger.rs:301–307`, `core/registryd/src/registry.rs:1257–1266`; author disclosure `docs/specs/m2_registry_storage_v1.md:98`.

If ledger append, validity unlink/truncation, anchor update and restrictive journal append all fail before changing their bytes, the restriction survives only in process memory. An intact earlier validity/empty ledger and `running` marker cause the next same-boot start to resume without the restriction. The independent debug/release test observes `current`; allowing validity invalidation produces containment instead.

This residual predates revision 2 and is explicitly inside this re-review's requested fault scope. Its candid documentation is useful, but it is not an owner-approved exception to ADR 0006 §3: “A restriction must survive any write failure followed by a restart.” The reviewer does not accept that architecture change on the owner's behalf.

**Required:** establish a fail-closed restart design for this condition, or obtain a concrete owner-approved ADR that narrows the guarantee with explicit assumptions and consequences, then review the implementation against it. Do not silently count the documented residual as stage-A PASS.

### M2-006-A-R2-04 — Low — Queue flush leaves multiple unanchored records and bypasses the read gate on partial failure

Location: `core/registryd/src/registry.rs:1287–1338`.

`flush_pending` appends the entire pending queue before anchoring. Two restrictions queued during journal failure, followed by successful journal writes and failed anchor update, leave journal sequence 6 versus anchor 4. A same-boot restart with valid ledger quarantines as `unexplained_records`. If only the first pending record succeeds and the second journal write fails, the early return leaves `anchor_failed` false: `lineage_head` returns a view at sequence 5 while the anchor remains 4.

Both debug and release reproduce three failed expected-invariant assertions; a fully successful flush/restart control passes. The records are restrictive: no widening of grants is demonstrated, so this is Low recovery/availability and read-consistency impact. It nevertheless contradicts the revised “nothing appended beyond an unanchored record” and “nothing unanchored served” contract.

**Required:** keep the one-record roll-forward invariant throughout queue draining, including every early return. Anchor each durable record before proceeding, or provide an approved equivalent recovery protocol. A partial failure must retain enough state to retry anchoring and deny unanchored reads. Add multi-entry queue and partial-loop tests.

## Original findings and requested fault matrix

| Check | Result | Evidence and limits |
|---|---|---|
| Original R01–R08 tests unchanged | **PASS** | `reviewer_storage`: 8/8 in debug and release. Exact original source SHA-256 `cb3bbbd81e3c56f88cb85d42205c5a99d15535cff96f922d68fed7275d88c6f0`. |
| Original adverse policyd controls | **PASS** expected rejection | `policy_review`: 0/2 in both profiles, exit 101 as expected; unanchored admission gives `unknown(identity_unavailable)` and failed emergency release gives `denied(emergency_restriction)`. Source SHA-256 `41ec9718ac19b87b60705a62771c51eb6261a55bfbf312df05291b5f0d2fada0`. Matches author reviewer_reproductions logs. |
| R01/R02 direct anchor-failure path | **PASS** bounded scenarios; aggregate **FAIL** R2-04 | Denial, first successful read retry, second admission refusal, restriction and invalidation queued/ledgered, same/new-boot drop points and three failing read retries. Queue-drain boundary remains defective. |
| “Until anchored” instead of “until restart”; read-path retry | **PASS** design placement | Successful slot write naming the record supplies the missing durability proof; a read retry does not invent authority. `anchor_failed` is cleared by successful `anchor_slot`. This placement stays within ADR recovery semantics; R2-04 shows flag tracking is incomplete. |
| R03 re-ledgering/exact matching/identity count | **PASS** ordinary repeated failures; aggregate **FAIL** R2-02/R2-03 | Independent extra matrix: lifecycle and identity each survive three and four consecutive failed same-boot restarts; exactly one eventual write, no duplicate identity count. Containment survives four failed restarts. Author unknown/held/quarantined/checksum cases pass. Startup ledger failure remains unsafe. |
| R04/R05 policy | **PASS** | Pending policy published by slot update and shutdown; release durable before memory publication; failed release same boot and independent new-boot case; concurrent raises/releases/enrollments in author test. Source review finds no policy/ledger acquisition while holding anchor. `rollback_status` releases each guard before the next. Stress is not an exhaustive scheduler proof. |
| R06/R07/R08 | **PASS** bounded original fixes | Missing entries invalidates ledger; identity count checked at anchored prefix including justified identity a+1; anchored states invoke common grant/branch containment helpers. Crafted slot regressions pass. R2-01 separately fails persistence of newly detected invalid-anchor quarantine. |
| `unverifiable` format byte 3; default `remove_file` | **PASS** format/API placement; aggregate **FAIL** marker lifecycle | Compatible default method compiles original fault_io unchanged; parser rejects malformed marker/counter layouts; normal-run validity-unlink failure and unlink+anchor failure controls pass. Byte 3 records an already-required unverifiable condition, but startup overwrites it in R2-02. |
| Foreign anchor / mixed ledger entries | **PASS** stated tests | Author foreign-store, unknown UID, exact already-held lifecycle, quarantined user and checksum cases; independent containment/identity repeated-failure extensions. |
| Equal counters / compound residual / pending queue | **FAIL** | R2-01, R2-03 and R2-04; positive controls pass. |
| Actual daemon termination, power loss, kernel crash, real reboot | **NOT_RUN** | Tests inject I/O faults into actual Rust code over real files and drop/reopen registry objects with synthetic boot IDs. No physical-crash or full-system enforcement claim. These were already explicit staged limits. |

## Scope and startup evidence

**PASS**, exact commands from `/home/dev/cham3`:

```sh
git status --porcelain
git stash list
git fetch origin codex/m2-cryptographic-contract-core review/m2-006-a-security main
git checkout codex/m2-cryptographic-contract-core
git reset --hard origin/codex/m2-cryptographic-contract-core
git rev-parse HEAD
git log --oneline -5
git show --format='%H %P' --no-patch fc147e0
git diff --name-only a9e47e4 e34fd8a
git diff --check a9e47e4 e34fd8a
git diff --check e34fd8a fc147e0
git diff --stat e34fd8a fc147e0
```

The implementation range changes exactly these ten files:

```text
core/registryd/src/anchor.rs
core/registryd/src/journal.rs
core/registryd/src/ledger.rs
core/registryd/src/registry.rs
core/registryd/tests/storage_integrity.rs
core/policyd/tests/current_authorization.rs
docs/specs/m2_registry_storage_v1.md
docs/specs/m2_service_ipc_v1.md
tests/m2/harness/test_m2_registry_policy.py
tests/m2/test_inventory.txt
```

No dependency, Cargo.lock, manifest, ADR, oracle, vector, workflow, Dockerfile or pin changed. fc147e0 changes 42 paths, all under docs/, and corrects “17 files changed” to “22 files changed, 5 of them added” in the stage-A handoff.

## Runtime environment and commands

Review host: Debian 13.7, Linux `7.1.13+deb13-amd64`, x86_64, UID/GID 1001. Docker client `26.1.5+dfsg1`, build a72d7cd. This session has `no new privileges`; the assignment's unrestricted-Docker premise is not true in the observed tool runtime.

**BLOCKED**, pinned Docker qualification, exact attempted command:

```sh
sudo -n docker run --rm --user 1001:1001 --ulimit nofile=1024:1024 \
  -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
  -v /home/dev/cham3-review-r2/clean:/work -w /work cham3-m2-independent-r3 \
  bash build/scripts/qualify_m2_rust.sh
```

Exit 1: `sudo: The "no new privileges" flag is set, which prevents sudo from running as root.` Plain `docker images --format '{{.Repository}} {{.ID}}'` independently fails: `dial unix /var/run/docker.sock: connect: permission denied`. Host protections were not disabled.

A bounded unprivileged fallback was used for component tests: authenticated APT downloads of the exact committed Rust 1.85.1 compiler/Cargo/std packages and 238 locked librust source packages, extracted with `dpkg-deb -x` under `/home/dev/cham3-review-r2/toolchain`. No host package installation. `cargo` wrapper supplies only private Debian source-directory location, RUSTC and library path; repository flags/lock/code stay unchanged. All **1,454 dependency source files match** the committed manifest. rustfmt/clippy were the same pinned release. A private Python venv with system packages and extracted python3-argon2 supplies the independent vault oracle. This fallback is component evidence, **not a complete pinned-container qualification**.

Execution root `R=/home/dev/cham3-review-r2` (notation for commands below); clean source clone:

```sh
git clone --no-hardlinks /home/dev/cham3 /home/dev/cham3-review-r2/clean
git -C /home/dev/cham3-review-r2/clean checkout --detach e34fd8a0b97f5a606656ab87914d4d1b19517d7c
```

From that clone, dependency/static/unit commands:

```sh
export PATH=/home/dev/cham3-review-r2/bin:$PATH
export LD_LIBRARY_PATH=/home/dev/cham3-review-r2/toolchain/usr/lib/x86_64-linux-gnu
cargo metadata --frozen --format-version 1 > /home/dev/cham3-review-r2/metadata.json
python3 -I build/scripts/m2_dependency_inventory.py sources --metadata /home/dev/cham3-review-r2/metadata.json --check build/m2/crate_sources.sha256
cargo fmt --all --check
CARGO_TARGET_DIR=/home/dev/cham3-review-r2/clippy_target cargo clippy --workspace --all-targets --frozen -- -D warnings
python3 -I build/scripts/check_rust_naming.py
bash build/scripts/run_static_checks.sh
sh tests/run_tests.sh static
ulimit -n 1024
CHAM3_M2_REQUIRE_CLEAN=1 sh tests/m2/run_m2_tests.sh unit
TMPDIR=/home/dev/cham3-review-r2/tmp CHAM3_M2_REQUIRE_CLEAN=1 sh tests/m2/run_m2_tests.sh harness
```

Setup interruptions: the first manifest invocation lacked the wrapper PATH, then passed; initial clippy lost source-directory config through Cargo's external-subcommand dispatch, then passed after forwarding that argument in the wrapper. Unit script's final vault oracle initially failed `ModuleNotFoundError: No module named 'argon2'` because it started before the private venv wrapper was ready. No test or security condition was weakened. The targeted retry was:

```sh
/home/dev/cham3-review-r2/bin/python3 -I tests/m2/oracle/vault_object_oracle.py \
 --probe /home/dev/cham3-review-r2/clean/out/m2/runs/unit_20261013T233659Z_786861/target_debug/debug/cham3_vault_probe
```

It passes **38 checks**. The original unit-suite summary retains its FAIL; component results and successful retry are reported separately.

Original reviewer sources were copied unchanged into a separate detached candidate clone `R/reviewer_tests`. Commands there (both without and with `--release`):

```sh
CARGO_TARGET_DIR=/home/dev/cham3-review-r2/reviewer_target /home/dev/cham3-review-r2/bin/cargo test --frozen -p cham3_registryd --test reviewer_storage review_ -- --nocapture
CARGO_TARGET_DIR=/home/dev/cham3-review-r2/reviewer_target /home/dev/cham3-review-r2/bin/cargo test --frozen -p cham3_policyd --test policy_review -- --nocapture
```

These inherit host nofile **524288**. Independent additional tests use separate clones, unchanged implementation, and only extra test modules:

```sh
# R/storage_validation; nofile 65536; repeat with --release
CARGO_TARGET_DIR=/home/dev/cham3-review-r2/shared_validation_target /home/dev/cham3-review-r2/bin/cargo test --locked --offline -p cham3_registryd --test review_r2_storage review_r2_ -- --nocapture
# R/matrix_validation; nofile 4096; repeat with --release
CARGO_TARGET_DIR=/home/dev/cham3-review-r2/matrix_target /home/dev/cham3-review-r2/bin/cargo test --frozen -p cham3_registryd --test reviewer_matrix review_matrix -- --nocapture
# R/queue_validation; inherited nofile; repeat with --release
CARGO_TARGET_DIR=/home/dev/cham3-review-r2/queue_target /home/dev/cham3-review-r2/bin/cargo test --frozen -p cham3_registryd --test queue_validation queue_ -- --nocapture
```

The storage and queue fixtures are the candidate's unchanged `storage_integrity.rs` followed by retained reviewer additions. Storage: 3 failed invariants and 3 passing controls per profile. Queue: 3 failed invariants and 1 passing control per profile. Matrix: 4/4 passing tests, 12 parameter cases per profile. Initial two delegated package-name typos (`cham3-registryd`) failed immediately, then were corrected to the repository package name before these runtime results.

## Full-filesystem and wrapper evidence

**PASS**, both probe profiles and 5/5 wrapper tests on a fresh **2 MiB tmpfs** in an unprivileged user/mount namespace. UID remains 1001; capabilities are dropped after mounting and before tests. The mount is private to the disposable namespace, with no host mount or protection changes. `unshare --user --map-root-user --mount true` first established namespace availability; the actual run uses `--map-current-user` so tests remain non-root.

From the clean clone:

```sh
export PATH=/home/dev/cham3-review-r2/bin:$PATH
export TMPDIR=/home/dev/cham3-review-r2/tmp
ulimit -n 1024
mkdir -p /home/dev/cham3-review-r2/full-disk
unshare --user --map-current-user --mount --keep-caps sh -c '
mount -t tmpfs -o size=2m,mode=0700 tmpfs /home/dev/cham3-review-r2/full-disk || exit
exec setpriv --bounding-set=-all --inh-caps=-all --ambient-caps=-all sh -c '\''
sh tests/m2/storage/check_reserve_full_disk.sh /home/dev/cham3-review-r2/storage /home/dev/cham3-review-r2/full-disk > /home/dev/cham3-review-r2/full_disk.log 2>&1
probe_result=$?
CHAM3_M2_RESERVE_PROBE=/home/dev/cham3-review-r2/storage/target/release/cham3_reserve_probe CHAM3_M2_WRAPPER_DISK=/home/dev/cham3-review-r2/full-disk python3 -I -m unittest discover -s tests/m2/harness -p test_m2_reserve_probe.py -k wrapper -v > /home/dev/cham3-review-r2/wrapper.log 2>&1
wrapper_result=$?
printf "probe=%s wrapper=%s\n" "$probe_result" "$wrapper_result"
ls -A /home/dev/cham3-review-r2/full-disk
'\''
'
```

Output `probe=0 wrapper=0`, no leftovers. `stat -f` during namespace setup reports tmpfs, block size 4096, 512 blocks. Logs show restrictive sequences 35/36, refused admission with depleted reserve, refill sequence 37, chained journal, ledger append refused on full disk, failed ledger, removed validity, previous ledger missing, and ledger stays failed. This probes journal and ledger; it does not establish an anchor reserve that does not exist.

## Harness sensitivity review

**PASS** static discrimination with one important coverage qualification. AST comparison gives **62 → 81** registry/policy mutations, **19 added**; all new/changed literal mutation anchors occur exactly once. Inventory is **202** cargo tests. The harness requires a passing exactly-one-test baseline and the named Rust test's `FAILED` result under each mutation, not a compilation failure.

| Added mutation | Why the named test discriminates |
|---|---|
| unanchored_state_served | Read must return storage_unavailable while anchor retry fails. |
| unanchored_admission_evaluated | Qualified: direct lineage_head assertion catches it; identityd already makes policyd unknown. It does not independently show permissive policyd evaluation after only this mutation. |
| restriction_dropped_after_anchor_failure | Restrictive error, ledger payload, pending state and replay. |
| invalidation_dropped_after_anchor_failure | Restrictive error, identity payload and replay invalidation. |
| second_unanchored_record_appended | Journal count unchanged behind existing unanchored tail. Does not cover multi-entry queue draining, R2-04. |
| replayed_entries_not_reledgered | New run ledger contains intent. |
| pending_policy_lost_at_shutdown | Clean marker contains current policy. |
| pending_policy_not_published_by_slot_writes | Another user's enrollment publishes pending global policy. |
| pending_policy_not_reported | policy_pending true during failed publication. |
| emergency_release_in_force_before_durable | Live and snapshot policy remain restrictive. |
| failed_release_evaluated_as_released | Actual policyd emergency denial. |
| missing_ledger_entries_accepted | Missing entries triggers containment. |
| identity_count_not_reconciled | Both count directions quarantine; identity tail roll-forward remains valid. |
| anchored_quarantine_leaves_grants_live | Grant terminal/quarantined status. |
| anchored_containment_leaves_grants_live | Grant terminal/suspended status and snapshot. |
| ledger_failure_not_marked_in_anchor | Durable marker is unverifiable. Normal-run test misses R2-02 startup overwrite. |
| unverifiable_marker_resumed | Following start contains. |
| equal_slot_counters_accepted | First start quarantines. Does not cover second restart, R2-01. |
| quarantined_users_entries_rejournaled | Quarantined journal length stays unchanged. |

Four existing source-edit tuples changed (`quarantine_leaves_grants_live`, `restriction_not_ledgered`, `failed_ledger_keeps_its_validity`, `shutdown_clean_with_pending_writes`), plus the test target of `anchor_failure_not_remembered` was renamed. “Three anchors re-pointed” is therefore an imprecise informational count, not a behavior finding.

Passing seeded defects establish sensitivity to those defects, not completeness of crash/recovery coverage. The added independent matrix closes the author's limited repeated-restart, containment/identity and new-boot release cases; the four confirmed findings remain outside the supplied mutation coverage.

## CI comparison

**PASS**, retrieved independently through GitHub CLI:

```sh
gh api repos/Kayyo321/cham3/actions/runs/38481185159
gh api repos/Kayyo321/cham3/actions/runs/38481185159/jobs
gh api repos/Kayyo321/cham3/actions/runs/38481185180
gh api repos/Kayyo321/cham3/actions/runs/38481185180/jobs
gh api repos/Kayyo321/cham3/actions/runs/38481185314
gh api repos/Kayyo321/cham3/actions/runs/38481185314/jobs
gh run view 38481185159 --repo Kayyo321/cham3 --log
gh api repos/Kayyo321/cham3/actions/artifacts/11857892996
gh api repos/Kayyo321/cham3/actions/artifacts/11857892996/zip > /home/dev/cham3-review-r2/artifact.zip
sha256sum /home/dev/cham3-review-r2/artifact.zip
```

All three are completed/success with exact candidate head_sha. Every job succeeds; no failed step. M2 log includes qualification PASS, vault positive protection PASS, full-filesystem PASS and wrapper 5/5. Downloaded artifact: id **11857892996**, `m2_qualification_amd64`, **106575 bytes**, **44 entries**, SHA-256 **d20cc30e5ef855f61e841e7fe8cfcaf93495bbd00e2e847736292150d3409173**, matches API and assignment. The local and CI 202-test inventories match exactly; differential corpus hash matches **0ad8bc2b1e8d39224de699db64aa45ca7adcf70117c555f35b57c537471d395e**. CI pinned qualification is evidence from CI, not a local pinned-container run.

## Retained review evidence

Local execution root: `/home/dev/cham3-review-r2`. Unit run: `clean/out/m2/runs/unit_20261013T233659Z_786861`; harness run: `clean/out/m2/runs/harness_20261013T233722Z_789479`; CI ZIP/extraction and API/log responses are under that root. Test clones are independently detached at e34fd8a and have no tracked implementation changes; only reviewer test modules/build artifacts were added. The clean source identity reports dirty_tracked 0 and unchanged Cargo.lock. A default cargo invocation created an untracked `target/` build tree; it is not source modification.

Codex Security scan directory:

`/home/dev/.codex/state/plugins/codex-security/scans/cham3/e34fd8a0b97f5a606656ab87914d4d1b19517d7c_20261013T233221Z_ejl0lii2`

| Retained artifact under scan directory | SHA-256 |
|---|---|
| artifacts/02_discovery/validation_artifacts/storage_r2_validation.rs | bd6e59153e9fa89dd1439b5addb31800d4a4f6bf1c127ea3240a5f582d84236f |
| artifacts/02_discovery/validation_artifacts/storage_debug.log | 31e4d8ec53859f87110e247d4063e02255d9cc6141ede02a53934bf9a1251397 |
| artifacts/02_discovery/validation_artifacts/storage_release.log | 4a21c1564150b42099ecd847d1372061799af3c25ab1f994cdc76cfef6cd1106 |
| artifacts/02_discovery/validation_artifacts/candidate-7591ccc60092f495/queue_additions.rs | 845d502b5ebb59857e6dabbf2fe381f1970d14fc45c8718a56db4848e739e0c4 |
| artifacts/02_discovery/validation_artifacts/candidate-7591ccc60092f495/debug.log | 0afa7050c1088971750d92d6f28681e0c8f633c7ed01667854b3738895ad9257 |
| artifacts/02_discovery/validation_artifacts/candidate-7591ccc60092f495/release.log | 60e59fa1ee03ae285abc223a7005ea44503803986cc137107ac311a24ba52c3c |
| artifacts/reviewer_matrix.rs | e10c6e40a87431dcfb621ddafe63c16191165a070ebef6c97279a3b9c4352fce |
| artifacts/reviewer_matrix_debug.log | 72b9818a883510bd26a5ba424e972a05c5ca0c1a5db11b5e6ee684027fca8978 |
| artifacts/reviewer_matrix_release.log | 3d1858def75b2c04b7b7bfd1d7d8cb0ab3a8e51ace79dc34ccc1135196f0a0b8 |

A delegated queue-validation turn subsequently returned: `Agent errored: This content was flagged for possible cybersecurity risk.` No retry or model change was used to evade it. Both debug and release execution logs had already completed; the parent read and retained their actual bytes and reviewed the source. The rejection prevented that worker's final narrative, not those already-completed test runs. Daybreak advisory returned not_granted; no protected-output availability is promised.


## Final verification results

| Check | Result | Evidence |
|---|---|---|
| Rust unit inventory | **PASS** | 202/202 debug and 202/202 release, exact committed inventories, `unit_suite.log` and unit run artifacts. |
| Provider known-answer checks | **PASS** | 7/7 in unit run. |
| ECR oracle | **PASS** | 19/96/18, 23 scenarios and 352 steps. |
| Format differential | **PASS** | 5,955 cases, zero disagreements, unchanged corpus digest above. |
| Vault oracle | **PASS** retry | 38 checks after supplying missing argon2; original unit orchestration run remains FAIL for setup. |
| Python harness | **PASS** aggregate after retries | Original run: 147 tests, 145 pass and two errors because Valgrind was absent. Both errored tests then pass individually. All 81 registry/policy and 21 identity/vault seeded defects and 12 reserve-probe tests pass. Original harness orchestration run remains FAIL; no single pristine 147-test PASS is claimed. |
| Valgrind provider / secure memory | **PASS** retry | Provider report exit 0, zero errors; secure memory 10/10, zero errors, no leaked blocks. Private Valgrind 3.24.0 and matching loader debug symbols. Initial runs could not install the loader `strcmp` redirection; supplying the matching debug-symbol path fixed setup without code changes. |
| Static checks / static tests | **PASS** | `static.log`; 82 tests in `static_tests.log`. |
| rustfmt / naming / clippy | **PASS** | Commands above; `rustfmt.log`, `clippy_retry.log`; naming command exits 0. |
| Full filesystem / wrapper | **PASS** | Real isolated 2 MiB tmpfs; both probe profiles and 5/5 wrapper cases, as detailed above. |
| Full pinned local amd64 qualification | **BLOCKED** | Exact Docker/sudo commands and errors above. Component fallback and CI do not substitute for this environmental gate. |
| Miri, sanitizers, fuzz campaign, deployed system | **NOT_RUN** | Outside this bounded re-review; no claim of full-system or power-loss verification. |

Exact targeted harness retries from the clean clone, with private tool PATH and TMPDIR above and `ulimit -n 1024`:

```sh
python3 -I -m unittest discover -s tests/m2/harness -p test_m2_format_verifier.py -k reservation -v
python3 -I -m unittest discover -s tests/m2/harness -p test_m2_format_verifier.py -k short_container -v
```

Logs `harness_reservation_retry.log` and `harness_short_container_retry.log` each report one passing test. The original 147-test run took 381.751 seconds and retains both missing-tool errors.

Exact memory checks, same nofile limit and private PATH:

```sh
valgrind --error-exitcode=99 --track-origins=yes /home/dev/cham3-review-r2/clean/out/m2/runs/harness_20261013T233722Z_789479/profile_target/release/cham3_provider_probe report
valgrind --error-exitcode=99 --track-origins=yes /home/dev/cham3-review-r2/clean/out/m2/runs/unit_20261013T233659Z_786861/target_release/release/deps/cham3_secure_memory-bb1bb9e1ec6736af --test-threads=1
```

The Valgrind wrapper supplies `VALGRIND_LIB=/home/dev/cham3-review-r2/toolchain/usr/libexec/valgrind` and `--extra-debuginfo-path=/home/dev/cham3-review-r2/debug_symbols`. Matching host loader build ID is `86f7eb4ff495ed3f2831c6d3d662e4321762a2a3`. Logs: `valgrind_provider_retry.log`, `valgrind_secure_memory_retry.log`. This is host component evidence, not pinned-container evidence.

## Codex Security plugin result

**PASS** workflow completion; **partial** qualification/runtime coverage. Producer **codex-security-plugin 0.1.32**, skill `security-diff-scan`, scan **3d57eb8e-3980-4588-87ac-6e167d2b15ec**, exact range a9e47e4..e34fd8a. Three capability preflight checks passed without remediation. Four changed production source files were inventoried and read; all ten changed files were covered by this review. Threat model, discovery, validation, attack-path analysis and final semantic draft were completed. Finalization succeeded once at **2026-10-13T23:47:29.844509Z** and the sealed scan was read back.

The exploit-oriented plugin report promotes **zero findings**: each of the four technically reproduced candidates lacks a demonstrated lower-trust attacker path to its protected-file corruption or compound I/O-fault prerequisites; the queue case shows restrictive recovery/read-consistency harm, not widened grants. These dispositions do **not** negate the reproductions or waive the project's stricter failure guarantees. The independent project review retains **three Medium and one Low** acceptance findings and requests changes. There is no claim that the application can directly modify private storage.

Canonical artifacts in the scan directory above: `scan-manifest.json`, `findings.json`, `coverage.json`, `threatmodel.md`, `report.md`, and `exports/results.sarif`. Coverage explicitly retains the blocked local pinned qualification and excludes later deployment/power-loss work. Supplemental `artifacts/qualification_evidence.tar.gz` contains 59 selected logs, original reviewer sources, wrappers, CI metadata and run metadata; SHA-256 **cfbb00dfd1d189207d0dc94dabc4a4f8eb240f4d60f08223dcf136f1af530f66**. No build trees or secrets are published. Source and detailed finding evidence remain in the local scan artifacts listed above.

Tool-reported usage (rollout accounting across four threads): **17,170,515 total tokens**, comprising 17,110,427 input (16,465,024 cached) and 60,088 output; 8,765 reasoning-output tokens are reported within output accounting. These are the plugin's measured cumulative counts, not unique prose size or a cost estimate.

## Structured handoff and owner questions

- Task/state: **M2-006 stage A R2 — CHANGES_REQUESTED**. Independent Codex REVIEWER. Review base a9e47e4, implementation e34fd8a, docs fc147e0; full immutable IDs above.
- Changed behavior: none. Only this review record is authorized for publication. Implementation checkout remains clean, without stashes; test fixtures and logs are local, separate artifacts.
- Next owner/action: coordinator returns the four findings to the implementer, preserves the evidence and requests focused regression coverage; independent re-review must examine the resulting immutable commit. Integrator must not treat passing CI or zero promoted plugin exploit findings as stage-A approval.
- Owner question: if the compound all-storage-failure residual is to be accepted, what exact narrower guarantee, assumptions and recovery consequences does the owner approve in a new ADR? In the absence of that decision, the current ADR remains authoritative and R2-03 requires a fail-closed solution. This reviewer makes no architecture or risk-acceptance decision.
- Remaining gates: local pinned qualification in an environment with authorized Docker access, fixes/re-review for R2-01/02/04 and resolution of R2-03, then coordinator/status-owner updates through the established workflow. No merge, stage-B approval or production-enforcement claim is made.
