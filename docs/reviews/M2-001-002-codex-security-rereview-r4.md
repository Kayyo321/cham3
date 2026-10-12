# M2-001 / M2-002 independent security re-review, revision 4

Reviewer: **Codex**, independent REVIEWER, 2026-10-12. No implementation edits, merges, owner decisions, or delegation.

Reviewed candidate **9b1aa2b9d1435fb33c303ca7fe01ce7afcb2964a**, tree **cb9f15101455f6c670a5f901f1e829544d45ef7d**, range **42397304663bff6057e26118014c18e7b90a2040..9b1aa2b9d1435fb33c303ca7fe01ce7afcb2964a**. Following evidence/handoff commit **70c431c13a1c76fd2948e6c8f0ae33b720618043** has that candidate as its parent and changes **52 paths, all under `docs/`**. Publication branch: `review/m2-001-002-r4-security`, based on `origin/main` at **9cf882503848f8c6cc4938a588cc9fc062a9392d**. Publication adds only this record.

Read AGENTS.md, PROJECT.md, approved ADR 0001, the M2 task and acceptance/gate definitions, WORKFLOW/TESTING/SECURITY_INVARIANTS, role/orchestration guidance, the r3 re-review at **f451f2e346e6981b24802a7e9070ba09303c40d3**, ADRs/specifications and their r4 diffs, the finding resolutions r3 and handoff r4 (the latter obtained with `git show 70c431c:docs/handoffs/M2-001-002-rereview-r4.md`). Author resolutions and evidence were treated as claims to verify. The prior review's SHA-256 independently matches **4881c1e9de089af544041d5140492aa2920460b09ed0f04a9a9359a745d80b0d**.

## Verdicts

| Package/gate | Verdict | Reason |
|---|---|---|
| M2-001 | **CHANGES_REQUESTED** | Native qualification and prior build/provider findings pass; required independent Debian advisory review remains BLOCKED. No newly demonstrated provider/build defect. |
| G1 / ADR 0003 | **CHANGES_REQUESTED** | Own Debian tracker retrieval again returns HTTP 403. RustSec passes independently; author advisory evidence cannot clear the missing check. |
| G2 / ADR 0004 | **APPROVE for owner consideration** | Normative wire/CDDL unchanged; independent bytes/signatures, regression probes, all vectors/scenarios and byte-for-byte regeneration pass. |
| G3 / ADR 0005 | **APPROVE for owner consideration** | R14's authenticated naming route, closed arguments and reconciliation are specified and modeled; no generic append route. |
| G4 / ADR 0006 | **APPROVE for owner consideration** | Naming is ineffective before anchoring; pending-tail ordering, roll-forward, retry and held-key reconciliation pass the reviewed design/model cases. Low-severity coverage note I04 below is nonblocking. |
| G5 / ADR 0007 | **APPROVE for owner consideration** | R15's false limits are replaced by honest limits and meaningful property evidence. Section 8 is an unselected owner choice, not a hidden change of the clock rule. |
| G6 / ADR 0008 | **APPROVE for owner consideration** | Recovery route completed; persistent-only restore, privacy, quotas and tombstone rules remain intact at the prior design scope. |

APPROVE here is an independent recommendation about the exact **design package**, not owner approval, service implementation acceptance, or permission to bypass G1. M2-002/full M2 are not DONE; dependent work still requires owner decisions and all dependency gates. Actual sockets, vault custody, registry/anchor writes, service clock and resource enforcement remain **NOT_RUN**.

## R14 and R13

**R14 PASS; R13 PASS at design/interface/model scope.** The original registry-table gap assertion now exits 1 because the gap is closed. The table has `name_recovery_keys` for **only `service:vaultd`**. Vaultd also has `recovery_status`; that read/reconciliation route is separately authorized for owner/admin. The role-table harness checks those exact sets.

The closed naming map has only `owner_uid`, `recovery_id`, `persistent_key_id`, `temporary_key_id`. Registryd constructs the record and obtains its epoch from the open ceremony. It requires a contained, non-quarantined, non-deleting user, matching anchored identity sequence, an open matching ceremony with no keys named, and two durable, non-invalidated certificates of this user under the current identity key at exactly the ceremony epoch, of persistent/temporary class respectively. `append_identity` refuses naming and ceremony records. There is no caller-controlled generic registry append operation. The identity-signature/user checks precede model key insertion; the model does not check anchor `identity_seq`, which is explicitly a future service test.

The new interface scenario rejects every listed non-vaultd role, extra epoch/record arguments, wrong user/ceremony/class, another user's uncertified key, old-epoch keys and second/different naming. The existing ceremony scenarios retain compromised/abandoned/deleted/bypass controls. Additional reviewer attacks independently reject naming with an invalidated key in either position and in normal/quarantined/deleted user states. A retired abandoned key from a preceding ceremony cannot meet the new ceremony's strictly higher epoch. Unnamed same-ceremony candidates confer no signing authority and are retired when the selected pair is named; holding such a pair during an unfinished operation is not itself evidence of abandonment.

Journal → anchor → reply is required. A durable unanchored naming record is the user's pending tail: no scope/epoch/retirement effect yet; every subsequent operation completes its anchor or fails `storage_unavailable`; startup rolls it forward. My direct state observation confirms that the pending record has not set the named persistent key or raised the minimum epoch. Repeated anchor failures, restart, successful roll-forward and same-key retry produce one durable naming record. Re-registration of the identical certificate and same-key naming are idempotent. Closed/aborted status retains the named pair so a lost reply cannot erase current keys. Unknown status keeps objects; an open unnamed ceremony resumes with the held pair rather than regenerating it. The late in-flight request and resumed request consequently name the same pair; the second returns `already_named`. An aborted/closed unnamed ceremony cannot admit a late naming, so its positive status can safely authorize cleanup of unnamed objects.

The R13 state machine is unchanged in substance except this routed naming step and the clarified bypass wording. Every suspended grant id remains terminal before/during/after recovery. Closing after key naming can end the key-scope restriction without restoring any grant; fresh commits with new ids remain necessary to regain grant authority. C1/C2, per-lineage progress, new-containment abort and deletion rules remain as reviewed. No recovery-opening option was selected.

**Sensitivity:** all shipped role/no-direct-route/retirement/lost-tail/idempotence/reconciliation/in-flight tests pass, and their named injected defects are detected. The shipped early-anchor mutant also changes the response from error to success; it is detected. A stricter response-preserving early-state mutant exposes a coverage limitation, I04 below. The actual candidate passes the independent state check; this is not evidence of a candidate authority bypass.

## R15 and the owner time choices

**R15 PASS, design/reference-model scope.** The unchanged `time_limits_probe.py` still reproduces lag of 7,200 seconds with a 5-second-old floor and 1,735 seconds remaining on an actually expired grant, and immediate eligibility of an admitted future start after an arbitrary forward error. Exit 0 means those reproductions succeeded; its printed `FAIL stated ... bound` describes the disproved r3 claims, not the repaired r4 wording.

ADR 0007 §5.7 now explicitly says: without samples ahead of actual time, lag does not grow within a boot but can accumulate across boots, bounded by the sum of unobserved intervals since the last synchronization, **not** latest-floor age; repeated set-back boots give no finite bound. Any accepted forward error has no bound, accelerates already-admitted future starts (fail-open for starts), causes early expiration, is never corrected by a lowering operation, and makes honest admission timestamps fail until actual time catches up. The timestamp admission check constrains the new timestamp only. Actual time can eventually catch up; “never corrected” is about this mechanism never lowering its trusted clock, not a claim that physical time cannot advance.

The seeded property runs **200 × 60** steps with independently tracked actual time, saved-floor actual time and accumulated unobserved-time bound. It checks `trusted <= actual` under its no-forward-error precondition, and `actual - trusted <= bound`; it requires reaching a latest-floor-age counterexample. I independently injected discarded persisted floors: the property failed with **18,218 > 14,791**, rather than passing a vacuous generator. The original 12 R09-r2 tests and maximum-offset rule remain unchanged.

Section 8 A/B/C and the forward-error recovery question are owner choices; **none selected**. For B, let admission trusted time be `T_a`, actual time `A_a`, and start `S`. The extra condition requires observed elapsed time at least `S - T_a`. The modeled observed counter cannot exceed actual elapsed time: same-boot BOOTTIME is elapsed time; cross-boot floors preserve or lose observed elapsed time and never add powered-off time. Thus actual eligibility cannot precede `S - (T_a - A_a)`; acceleration is bounded by the positive error present at admission. A later arbitrary forward sample cannot enlarge that bound. If `S <= T_a`, immediate eligibility still satisfies that bound. Reviewer-generated **300 × 80** steps with new boots, persisted counters and later forward errors confirm it, alongside the shipped B tests. B delays starts for powered-off/unobserved time and does not repair expiration lag. This is model/algebra evidence, not service-clock or authenticated-time evidence.

## G2 and prior findings

**PASS unchanged G2 substance.** `git diff 84fa6ef 9b1aa2b -- docs/decisions/0004_m2_wire_schema_hash.md docs/specs/m2_wire_format_v1.md docs/specs/m2_ecr_v1.cddl` shows only scenario counts/description and review/approval-record edits; CDDL has no diff. The exact JSON spans outside `journal_scenarios` are byte-identical. All nine network scenarios and the restore scenario are identical. Each of nine earlier recovery scenarios has exactly one changed step: the same naming ids now use the vaultd interface, with interface result/note metadata. Three new naming scenarios bring the total to **22 / 346 steps**. No earlier recovery commit/signature, non-naming step or expected grant standing changed.

The original independent verifier checks all **18** named objects' canonical envelope/header/payload bytes, signatures, object/state hashes and changed preimages without importing the author oracle. Its historical final line hardcodes “17”; the loop and 18 per-object results establish 18. The preserved schema probe exits 1 at the repaired `subject_binding_mismatch`; the preserved r2 probe exits 1 at the repaired `network_grant_conflict`. Because those scripts stop on rejection, later cases are established by the separate qualified regression tests, not falsely attributed to those script tails. `ecr_oracle.py check` passes **18 valid, 90 invalid, 18 admission, 22 journal scenarios / 346 steps**. A separate generate-and-`cmp` also passes byte-for-byte, both files SHA-256 **b3654b4eb8be2ffd7798a58d67cb0e2be354b1e0bb6180e176c237e7c120677f**.

| Prior item | Status | Evidence and retained limit |
|---|---|---|
| R01-r2 | PASS | Exit-7 hash tool first in PATH under sh/bash: exit 1, `cargo_lock_sha256 FAILED`, `identity FAIL`; unreadable-lock fixtures likewise. Qualified collector canaries and valid clean identity pass. |
| R07-r2 | PASS, intact design/model | Original sibling probe rejects; network-slot normative text/scenarios unchanged; both admission-order and positive controls run. Runtime concurrency/network effects NOT_RUN. |
| R09-r2 | PASS, intact reference model | Maximum-offset implementation and original 12 tests unchanged; all run. R15 limits now honest. Service clocks NOT_RUN. |
| R12 | PASS, intact design/model | Persistent-only ancestral predicate/scenario unchanged; session/timed/one-shot and merged ancestry rejection controls run. |
| R02 | PASS, intact | Full installed/deb locks reproduce byte-for-byte (digests below). |
| R03 | PASS, intact probe/design | Rust unchanged; exact Argon2 workspace clearing/error paths and populated-library contrast execute in both profiles. Actual vault custody NOT_RUN. |
| R04 | PASS, intact design | Run marker, ledger ordering, same-boot replay and unverifiable-run containment unchanged; routed naming does not relax mandatory denial. Actual disk/restart faults NOT_RUN. |
| R05 | PASS, intact design | Identity in anchored journal, scoped/current-sequence resolution and lost-identity-tail refusal preserved; vault files are not authority. Runtime cache/anchor freshness NOT_RUN. |
| R06 | PASS, intact oracle/design | Subject binding rejects the independent re-signed substitution; no unreviewed subject-transition rule. |
| R08 | PASS, intact oracle/design | 61,440-byte envelope and 61,441 rejection, bounded wrappers/pagination unchanged and checked. Actual service decoder NOT_RUN. |
| R10 | PASS, intact design | Global map/lock, user→anchor order and durable-slot-before-reply unchanged. Runtime concurrent updates NOT_RUN. |
| R11 | PASS, intact design | Denial/ledger → pending tombstone → erase → deleted tombstone, failed pre-erase writes destroy nothing, restored deleted identities refuse load. Actual erase/restore faults NOT_RUN. |
| I01 | PASS, intact | Anchor backends still expressly planned/unimplemented; same-volume whole-restore limit remains explicit. |
| I02 | PASS, intact | Actual release secure-memory executable under Valgrind: 10 tests, zero errors; exec'd fault children not instrumented. |
| I03 | PASS, intact | Development same-UID/terminal/knowledge-factor limitations and fixture attribution/consumers still explicit; no human-presence/isolation claim. |

The unchanged-input command exits 0 for `core`, Cargo files, `build`, `.github`, PROJECT, AGENTS, ADR 0001, engineering guidance, M2 runner and harness library. This confirms integrity within the prior reviewed scope, not new runtime acceptance of skeleton services.

## Execution environment and exact command ledger

Outer executor: Debian 13, **UID/GID 1000:1000**, Linux **6.18.44**, x86_64; managed local Docker **28.4.0**, runc, vfs, default seccomp/cgroup namespace. Clean network clone `/workspace/cham3-review-r4` is owned by 1000:1000 and stays detached at the candidate. Pinned image: Debian **13.7 amd64**, image ID **sha256:4b2d7828159556f7e591b04b6f2cf8b79e72dbbb6a569b3128cef39fa5385e1d**. Native execution, no emulation, `--privileged`, added capabilities, `--record`, production keys or relaxed checks.

Cloud runtime skill and its Docker/network references were read. Startup network snapshot is restricted, VPN false; current environment-status observations report network policy state `unknown`, which is **not** called enforced here. Network requests use the inherited proxy; no policy bypass or disabled TLS. Docker's unchanged build uses Debian-authenticated APT HTTP inputs and configured daemon proxy.

Local evidence **E** = `/workspace/r4-evidence`; external scripts **T** = `/workspace/r4-tools`; candidate source **S** = `/workspace/cham3-review-r4`. Container `/work` maps to S. Qualification **Q** = `out/m2/qualification/amd64_20261012T195536Z_1`; unit **U** = `out/m2/runs/unit_20261012T195701Z_1599`; harness **H** = `out/m2/runs/harness_20261012T195755Z_2675`. Raw local logs remain in these workspace paths; only this review record is published.

Exact setup:

```sh
git clone https://github.com/Kayyo321/cham3.git /workspace/cham3-review-r4
git -C /workspace/cham3-review-r4 checkout --detach 9b1aa2b9d1435fb33c303ca7fe01ce7afcb2964a
id
uname -a
stat -c '%u:%g %n' /workspace/cham3-review-r4
env -u DOCKER_HOST -u DOCKER_CONTEXT -u DOCKER_TLS -u DOCKER_TLS_VERIFY -u DOCKER_CERT_PATH docker --host=unix:///var/run/docker.sock info
docker build -f /workspace/cham3-review-r4/build/m2/dev_container.dockerfile -t cham3-m2-r4-review /workspace/cham3-review-r4/build/m2
```

`run` and `review_run` below are these literal settings, saved as T/run.sh and T/review_run.sh and invoked with `bash`:

```sh
run() {
  docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home \
    -e CHAM3_M2_REQUIRE_CLEAN=1 -e GIT_CONFIG_COUNT=1 \
    -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
    -v /workspace/cham3-review-r4:/work -w /work cham3-m2-r4-review "$@"
}
review_run() {
  docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home \
    -e CHAM3_M2_REQUIRE_CLEAN=1 -e GIT_CONFIG_COUNT=1 \
    -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
    -v /workspace/cham3-review-r4:/work:ro -v /workspace/r4-tools:/review:ro \
    -v /workspace/r4-evidence:/evidence -w /work cham3-m2-r4-review "$@"
}
```

| Check | Status | Exact command/environment | Evidence/result |
|---|---|---|---|
| Setup / clean candidate | PASS | Setup above; `git diff --exit-code`; `git status --porcelain`; `git rev-parse HEAD origin/main origin/codex/m2-cryptographic-contract-core` in S | Requested SHA/tree, clean tracked tree before/after; implementation ref still 70c431c. |
| Evidence commit | PASS | `git rev-list --parents -1 70c431c`; `git diff-tree --no-commit-id --name-only -r 70c431c` plus assertion all paths start `docs/` | E/evidence_commit_paths.txt; 52 docs-only paths, candidate parent. |
| Docker image | PASS | Build above | E/docker_build.log, exit 0; inspected image architecture amd64. |
| Native qualification | PASS | `run bash build/scripts/qualify_m2_rust.sh` | E/qualification_console.log, Q/summary.txt; exit 0, every required step PASS. |
| Static checks | PASS | `run bash build/scripts/run_static_checks.sh` | E/static_checks.log; exit 0, shellcheck 24 / Python compile 62 / YAML 5 / configured secret scan. |
| Static tests | PASS | `run sh tests/run_tests.sh static` | E/static_tests.log; exit 0, 82 tests, OK. |
| R14 preserved static probe | PASS (gap disproved) | `review_run sh /review/r14_static.sh`, unchanged shell block extracted from f451f2e | E/r14_static.log; exact operation/role rows; old gap assertion exits 1 as expected. |
| R14/R13 scenarios and harness controls | PASS, model | Qualification's `sh tests/m2/run_m2_tests.sh harness`, nested `python3 -I -m unittest discover -v -s /work/tests/m2/harness -p 'test_*.py'` | H/python_harness.log/json; all role/table/no-direct-route/durability/retry/retirement/reconciliation/in-flight controls run; 119 total. |
| Additional naming/time attacks | PASS for candidate; FAIL for broader early-effect sensitivity | `review_run python3 -I /review/extra_attacks.py` | E/extra_attacks.log; invalidated/user-state denials, pending-state observer, retries, B property, dropped-floor sensitivity PASS. Early-effect mutant gives no scenario failures (I04); independent snapshot observer detects it. Script exit 0 means observations/reproductions completed. |
| R15 preserved probe | PASS (limits reproduced and now disclosed) | `review_run python3 -I /review/time_limits_probe.py` | E/time_limits_probe.log; exit 0, both old bound counterexamples unchanged. |
| R15 seeded/B tests | PASS, reference model | Qualified harness invocation above | H/python_harness.log; all eight added R15 tests run, seeded bound checked and reviewer floor-discard sensitivity fails as expected. |
| G2 documentation diff | PASS | `git diff 84fa6ef 9b1aa2b -- docs/decisions/0004_m2_wire_schema_hash.md docs/specs/m2_wire_format_v1.md docs/specs/m2_ecr_v1.cddl` in S | E/g2_docs.diff; only counts/description/review-record edits. |
| Exact vector/scenario comparisons | PASS | `python3 -I /workspace/r4-tools/compare_vectors.py` in S | E/vector_comparison.log; raw bytes outside scenarios unchanged, ten non-recovery scenarios identical, exactly one naming-step change in each of nine recovery scenarios. |
| Preserved tools integrity | PASS | Archive obtained using `git show origin/main:docs/reviews/M2-001-002-codex-security-review-evidence.tar.gz`; `tar -xzf` into T; `sha256sum --check SHA256SUMS` in extracted directory; r2 appendix extracted from `git show 336f13b:docs/reviews/M2-001-002-codex-security-rereview-r2.md` | E/archive_check.log; archive SHA-256 8655477da0c83b186e9468cf874e85fc4a6536d9e5401327c44ef1c9159ab5af; r2 probe 8dcdde94144edb014a662ba0d0b69db356b095b02499272fbd68092daa16ee8f. |
| Independent vectors | PASS | `review_run python3 -I /review/cham3_m2_review_evidence/independent_vectors.py tests/m2/vectors/ecr_v1/vectors.json` | E/independent_vectors.log; exit 0; 18 named objects, canonical bytes/signatures/hashes/preimage controls. |
| Original schema holes | PASS (expected rejection) | `review_run python3 -I /review/cham3_m2_review_evidence/schema_holes.py tests/m2/oracle/ecr_oracle.py tests/m2/vectors/ecr_v1/vectors.json` | E/schema_holes.log; exit 1, admission/subject_binding_mismatch. |
| Original r2 probe | PASS (expected rejection) | `review_run python3 -I /review/r2_probes.py tests/m2/oracle/ecr_oracle.py tests/m2/vectors/ecr_v1/vectors.json` | E/r2_probes.log; exit 1, admission/network_grant_conflict. Later restore cases run in harness. |
| Oracle check | PASS, model | `review_run python3 -I tests/m2/oracle/ecr_oracle.py check tests/m2/vectors/ecr_v1/vectors.json` | E/vector_regeneration.log, U/ecr_vector_oracle.log; exit 0; 18/90/18/22/346. |
| Byte regeneration | PASS | `review_run sh -c 'python3 -I tests/m2/oracle/ecr_oracle.py generate /evidence/regenerated_vectors.json && cmp tests/m2/vectors/ecr_v1/vectors.json /evidence/regenerated_vectors.json && sha256sum tests/m2/vectors/ecr_v1/vectors.json /evidence/regenerated_vectors.json'` | E/byte_regeneration.log; exit 0, identical digest above. |
| R01-r2 own failures | PASS (expected FAIL) | `review_run python3 -I /review/identity_attacks.py`; nested sh/bash collector with exit-7 sha256sum first in PATH, and isolated clone's unreadable Cargo.lock | E/identity_attacks.log and four identity files; all exit 1 and explicit FAILED/FAIL. |
| Previously passed inputs | PASS | `git diff --exit-code 84fa6ef 9b1aa2b -- core Cargo.toml Cargo.lock build .github PROJECT.md AGENTS.md docs/decisions/0001_eternal_contract_repository.md docs/engineering tests/m2/run_m2_tests.sh tests/m2/lib` | E/unchanged_inputs.log empty, exit 0; matrix above. |
| Author/local summary comparison | PASS | `git show 70c431c:docs/evidence/M2-cryptographic-contract-core/m2_001_amd64_vm_9b1aa2b/summary.txt` into E/author_vm_summary.txt; `diff -u` with Q/summary.txt | Exit 0; reviewer observations reproduce summary bytes, not merely author labels. |
| Debian tracker retrieval | BLOCKED | `curl -fSL --max-time 45 https://security-tracker.debian.org/tracker/data/json -o /workspace/r4-evidence/debian_tracker.json` on outer executor | E/debian_tracker_retrieval.log: HTTP 403, curl exit 22; no tracker bytes/digest. |
| RustSec fetch | PASS | `git clone https://github.com/RustSec/advisory-db.git /workspace/r4-tools/advisory-db`; `git -C /workspace/r4-tools/advisory-db rev-parse HEAD` | E/rustsec_clone.log; 57dcc0f2192e025138907a495c1ac9a7e1e3e123. |
| RustSec-only judgment | PASS | `review_run python3 -I /review/rustsec_only.py out/m2/qualification/amd64_20261012T195536Z_1/dependency_inventory.json /review/advisory-db` | E/rustsec_only.log; own inventory, 37 versions, six nonwithdrawn advisories, zero affected; all displayed ranges independently inspected. |
| Combined Debian/RustSec clearance and author triage | BLOCKED | No successful combined advisory invocation; review of `git show 70c431c:docs/evidence/M2-cryptographic-contract-core/advisory_review_r4/triage.tsv` is source/scope judgment only | Missing independently retrieved tracker prevents Debian status/description/fix corroboration; no author clearance adopted. |
| CI comparison | PASS within remote CI scope | Connector calls/ids in next section | Jobs, logs, SHA and qualification summary compared independently. |
| CI ZIP bytes | NOT_RUN | No artifact ZIP download/byte comparison | Metadata digests are not reviewer-computed ZIP digests. |
| Standalone build entry point | NOT_RUN separately | `bash build/scripts/build_m2_core.sh` audited but not separately invoked | Its fresh debug/release underlying builds run in qualification; no extra standalone claim. |
| Integration/recovery/packaging product suites | NOT_RUN | `sh tests/m2/run_m2_tests.sh integration\|recovery\|packaging` not invoked as acceptance | Gated/unimplemented; dispatcher controls do not constitute product tests. |
| Miri/sanitizers/coverage-guided fuzzing | NOT_RUN | Qualification availability probes report NOT_AVAILABLE | Pinned toolchain lacks support; not a safety PASS. |
| Actual A04–A15 service/enforcement mechanisms | NOT_RUN | No actual IPC/vault/registry/anchor/time/ceremony/enforcement acceptance run | Models and fixture acknowledgments are not runtime evidence. |

### Qualification subchecks (all required checks PASS)

The single unmodified qualification script runs `bash build/scripts/m2_source_identity.sh <Q>/identity.txt --require-clean`, platform/UID checks, `rustc -Vv`, `cargo -V`, `cc --version`, `ld --version`, `ldd --version`, pinned-package version queries and complete installed/downloaded lock diffs. Rust/Cargo **1.85.1**, x86_64-unknown-linux-gnu, GCC **14.2.0-19**, binutils **2.44-3**, glibc **2.41-12+deb13u4**. Q/identity.txt: dirty 0, UID 1000, Cargo.lock **fc87901c5e58acdba21a33fe516871e45a81072585bfa2ae82aa8e60c40310f6**.

| Subcheck | Status | Actual nested command/check and evidence |
|---|---|---|
| Installed/downloaded input identities | PASS | Complete dpkg/deb lock comparisons, empty diffs; installed SHA-256 **6cd45590bdebd08f53c95a56112a97030b844959983284db44232b59baa11d3c**, deb lock **6ce06e450a8b91cd2c91c1d3aaffc360dfed7276f6622ac03dbd05fd35f0f43a**. |
| Metadata / crate sources | PASS | `cargo metadata --locked --offline --format-version 1`; `python3 -I build/scripts/m2_dependency_inventory.py sources --metadata <Q>/metadata.json --check build/m2/crate_sources.sha256`; 1,454 source files. |
| Inventory | PASS | `python3 -I build/scripts/m2_dependency_inventory.py inventory --metadata <Q>/metadata.json --out-dir <Q>`; 33 compiled dependencies; inventory JSON **7ff6c05870994425b829aed7b26c15ca70af27d35c42c98a0004c66425a17b67**. |
| Format / clippy / naming | PASS | `cargo fmt --all --check`; `env CARGO_TARGET_DIR=<Q>/clippy_target cargo clippy --workspace --all-targets --frozen -- -D warnings`; `python3 -I build/scripts/check_rust_naming.py`; corresponding Q logs. |
| Profiles / fresh builds / soft backends | PASS | Script's exact profile TOML heredoc; `env CARGO_TARGET_DIR=<Q>/build_target cargo build --workspace --frozen`, then `--release --verbose`; checked force_soft/force-soft/fiat output patterns. Overflow checks all profiles; dev unwind/release abort; two fiat outputs. |
| Valgrind probe | PASS | `valgrind --error-exitcode=99 --track-origins=yes <Q>/build_target/release/cham3_provider_probe report`; zero errors. |
| Valgrind secure memory | PASS | `env CARGO_TARGET_DIR=<Q>/build_target cargo test --release --frozen -p cham3_secure_memory --no-run --message-format=json`, then compiler-artifact-selected `valgrind --error-exitcode=99 --track-origins=yes <test-executable> --test-threads=1`; actual 10 tests, zero errors; exec'd fault children not instrumented. |
| Unit / providers / ECR | PASS | `sh tests/m2/run_m2_tests.sh unit`: `cargo test --workspace --frozen`, then `--release`; **36+36** inventory-exact tests; seven provider checks and independent OpenSSL/libsodium/libargon2 comparison; ECR check above. Independent Argon2 binding cannot reproduce RFC secret/AD inputs: explicitly NOT_CHECKED for that particular oracle KAT, separate shared-input case matches. |
| Harness | PASS | `sh tests/m2/run_m2_tests.sh harness`, exact unittest command above; **119** inventory-exact tests, including assertion/compile/link/panic/zero/ignore/filter/compile-only, source-identity, unsafe/naming and deployed-profile sensitivity controls. |

Q/summary.txt SHA-256 **c9c3f1c2cd07bc464863213c4e8272419c3cc41974a8ca90dc8615c7d21cb40f**. The decoded CI summary, after removing GitHub log timestamps and extracting status lines, is byte-identical to Q/summary.txt; saved E/ci_summary.txt. Setup-only mistakes in reviewer scripts (JSON indentation assumption, overly strict naming-step metadata comparison, an extra timestamp-only summary newline, and two initial historical evidence filenames) were corrected before final checks; these are not candidate failures.

## Advisory judgment

Own RustSec fetch is **57dcc0f2192e025138907a495c1ac9a7e1e3e123**. Reviewed ranges: blake2 0.10.6 ≥0.8.1; chacha20 0.9.1 ≥0.2.3; curve25519-dalek 4.1.3 ≥4.1.3; ed25519-dalek 2.1.1 ≥2; generic-array 0.14.7 ≥0.13.3; sha2 0.10.8 ≥0.9.8 (with unaffected <0.9.7 also specified). All six are outside affected ranges; no new advisory match among 37 compiled/test versions. This does not prove all unsafe code or side channels sound.

The author's 23 advisory triage rows are plausibly separated by current Rust probe, test-oracle and future host/setgid-service reachability. The unchanged sources do not use the described libc locale/formatting/tree/glob/iconv/resolver operations or a C++ application layer; ppc64le-only code is outside amd64. Ed25519 verification uses no X.509. Dynamic-loader/setgid and systemd risks must be reconsidered before M2-007, and test-oracle scope is not acceptance for future untrusted input. These are **conditional source/scope judgments**, not corroboration of live Debian advisory descriptions/status/fixed versions, and not owner risk acceptance. HTTP 403 leaves independent Debian review **BLOCKED**. No author tracker digest is passed off as a reviewer-fetched digest. G1/M2-001 remain CHANGES_REQUESTED for that prerequisite.

## GitHub Actions comparison

Independently called `github_fetch_workflow_run_jobs(repo_full_name="Kayyo321/cham3", run_id=<id>)` and `github_fetch_workflow_run_artifacts` for **38299143273 / 38299143245 / 38299143276**, then `github_fetch_workflow_job_logs(repo_full_name="Kayyo321/cham3", job_id=<id>)` for all seven jobs below. Raw metadata E/ci_metadata.json, decoded logs E/ci_job_<id>.log. Every reported job/step succeeds; artifact metadata and checkout logs bind the runs to **9b1aa2b9d1435fb33c303ca7fe01ce7afcb2964a**, branch `codex/m2-cryptographic-contract-core`.

| Run | Status | Independently inspected evidence and scope |
|---|---|---|
| m2_checks 38299143273 | PASS | qualify_amd64 **114977492780**: clean candidate, Debian 13.7, UID 1001, kernel 6.17.0-1022-azure; required qualification summary identical to local. Artifact **11770401811**, 86,294 bytes, metadata digest **b2b3501d65e8c9a004419dbfaaecdd91a85c6f1cac2265b3a689bcf61783977f**. |
| m1_checks 38299143245 | PASS, remote CI scope | static_and_desktop **114977492842**, privileged_capture **114977493295**, package_build **114977493453**. Logs: 82 static, 261 desktop static, 16 staging, 22 apt-pin, 77 privileged capture/maintainer-script tests; successful package build/tests. No local privileged suites or desktop acceptance claimed. |
| m0_checks 38299143276 | PASS, remote CI scope | image_build **114977492654**, package_build **114977493305**, static_checks **114977493327**. Logs: 82 static, successful package/image build, 17 image tests, serial login prompt in throwaway VM, 18 boot-result tests. No local ISO/VM rerun claimed. |
| Artifact ZIP byte verification | NOT_RUN | Metadata/log comparison only; no reviewer-computed ZIP digest. |

## New coverage finding (nonblocking)

### I04 — Low — Early-state sensitivity conflates no effect with the response code

Gate: **G4**. Locations at candidate: `tests/m2/harness/test_m2_review_regressions.py:296`; `tests/m2/oracle/ecr_oracle.py:2426` (scenario observations), `:1292` (correct pending-tail implementation).

The shipped `apply_before_anchor` mutation ignores the anchor fault entirely and returns success, so scenario outcome comparisons detect it. But those scenarios never directly observe the naming scope/epoch/retirement state while the tail is pending. A reviewer mutant can apply the naming state before anchoring, still return the required `storage_unavailable`, retain the pending-tail block, and settle the already-applied state on retry. **All three `recovery_naming_*` scenarios report zero failures**. A separate reviewer state assertion detects the mutant. The current candidate itself passes that state assertion.

Required for a stronger no-effect sensitivity claim: observe named-key scope, minimum key epoch and retirement flags before successful anchoring, and mutate early application while preserving error/retry semantics. Keep the existing response/served-state controls. This is a low-severity model-coverage gap; the specified boundary and candidate implementation are correct in the tested state, and the mutant still blocks every external operation. It is **not** a reproduced grant admission or runtime bypass, so it does not block the design recommendation or reopen R14/R13. Do not use these model tests alone as service durability proof.

Portable reproduction (save outside the repository; run with `review_run python3 -I /review/early_effect_probe.py`):

```python
import importlib.util, json
spec = importlib.util.spec_from_file_location('o', '/work/tests/m2/oracle/ecr_oracle.py')
o = importlib.util.module_from_spec(spec)
spec.loader.exec_module(o)
v = json.load(open('/work/tests/m2/vectors/ecr_v1/vectors.json'))
resolver = o.resolver_t([o.key_t(k['label']) for k in v['keys']])
valid = {i['name']: (i['kind'], bytes.fromhex(i['envelope'])) for i in v['valid']}
original = o.registry_service_t.op_name_recovery_keys
def early(service, args, anchor_fault):
    if not anchor_fault:
        return original(service, args, False)
    original(service, args, False)  # bug: applies scope/epoch/retirement before anchor
    service.journal.sequence -= 1  # applied state; durable naming remains the pending tail
    service.pending = {'record': 'recovery_keys', 'recovery_id': args['recovery_id'],
                       'persistent_key_id': args['persistent_key_id'],
                       'temporary_key_id': args['temporary_key_id']}
    assert service.journal.recovery['persistent_key_id'] == args['persistent_key_id']
    o.ipc_error('storage_unavailable')
def settle(service, anchor_fault=False):
    if service.pending is None:
        return
    if anchor_fault:
        o.ipc_error('storage_unavailable')
    service.pending = None  # anchors the state already applied by the bug
    service.journal.sequence += 1
o.registry_service_t.op_name_recovery_keys = early
o.registry_service_t.settle = settle
for scenario in v['journal_scenarios']:
    if scenario['name'].startswith('recovery_naming_'):
        failures = o.run_scenario(scenario, valid, resolver)
        print(scenario['name'], failures)
        assert not failures  # demonstrates the missing early-state observation
```

Observed independent script SHA-256: T/extra_attacks.py **508e802b938636e4a6c93d05934b0e00ec3fc324ab8c512f155dce6a3eb9140e**; comparisons **bd6cc40a9748a87d92a01dbecea38fc26fb1ccbaf30393f38b915d34be9ad145**; identity attacks **223e9e5817e2387a4acc36faea07271bc953fe3b42277b2f8ac09113398e803d**; preserved RustSec helper **22f07f885c336c4481694756838b4816eb5bfca77459b85170f63d5735e47968**. These scripts/logs are reviewer workspace evidence, not extra published implementation files.

## Publication and owner notes

Only this record is committed/pushed on the new review branch from main. Publication checks cover staged whitespace, single changed path against main, commit/push and remote-head verification; the detached candidate remains clean at 9b1aa2b and the implementation ref is not updated. Neither main nor any review branch was merged into the implementation. No PR review/comment or message to another person was sent.

Owner decisions remain: G1–G6; how independent Debian advisory evidence is obtained; default anchor mode; C1/C2 ceremony-opening role; one-shot consumption; ADR 0007 §8 time-bound strength and future forward-error recovery; deletion granularity. No option was selected. Address the Debian evidence prerequisite before G1/M2-001 clearance; strengthen I04's observation before claiming comprehensive no-effect sensitivity; perform the explicitly gated real IPC/vault/anchor/time/enforcement tests before implementation acceptance.
