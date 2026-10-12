# M2-001 / M2-002 independent security re-review, revision 3

Reviewer: **Codex**, independent REVIEWER, 2026-10-12. No implementation changes or owner decisions.

Reviewed candidate **84fa6ef172b3fad21bb079549776252bb65624d6**, tree **7e3bc1a4cedfff60ad437deca05428c45c26dcdc**, range **6b4ef9d87d69185ff7205b51fa417df1fed5c6f6..84fa6ef172b3fad21bb079549776252bb65624d6**. Evidence/handoff **42397304663bff6057e26118014c18e7b90a2040** changes **52 paths, all under `docs/`**. Publication branch: `review/m2-001-002-r3-security`, based on `origin/main` at **9cf882503848f8c6cc4938a588cc9fc062a9392d**. The implementation branch was neither edited nor merged into.

Read AGENTS.md, PROJECT.md, approved ADR 0001, the M2 task, WORKFLOW/TESTING/SECURITY_INVARIANTS, role/orchestration guidance, ADRs 0003–0008, wire/CDDL/IPC, the prior re-review at **336f13be6a19bb871ea956d64f9cacb0936fe028**, and the author response/handoff. Author resolutions and evidence were treated as claims, not independent results.

## Verdicts

| Package/gate | Verdict | Reason |
|---|---|---|
| M2-001 | **CHANGES_REQUESTED** | Native qualification and R01-r2 pass. Required independent Debian advisory review remains BLOCKED; G1 is not cleared. This verdict does not allege a newly demonstrated provider or build defect. |
| G1 / ADR 0003 | **CHANGES_REQUESTED** | Independent tracker retrieval remains BLOCKED. RustSec-only review passes; author triage cannot substitute for the missing check. |
| G2 / ADR 0004 | **APPROVE for owner consideration** | Network-slot/placement and persistent-only ancestral restore rules withstand the reviewed model cases; deterministic vectors and independent byte/signature checks pass. |
| G3 / ADR 0005 | **CHANGES_REQUESTED** | R14: the specified IPC table cannot complete recovery key naming. |
| G4 / ADR 0006 | **CHANGES_REQUESTED** | R14: freeze the authenticated path for the step-3 `recovery_keys` record, including retry/reconciliation. |
| G5 / ADR 0007 | **CHANGES_REQUESTED** | R15: within-boot time repair passes, but two advertised clock limits are false. |
| G6 / ADR 0008 | **CHANGES_REQUESTED** | R14 affects the coordinated recovery ceremony on which backup/restore depends. Persistent-only restore and tombstone rules otherwise remain intact. |

G2 approval is an independent design recommendation, not owner approval, Rust-verifier acceptance, or authorization to bypass another gate. Every unimplemented service/enforcement mechanism remains NOT_RUN. M2-002 and full M2 are not complete.

## Actionable findings

### R14 — Medium — Recovery key naming has no authenticated registry IPC operation

Gates: **G3, G4, G6**. Locations: `docs/specs/m2_service_ipc_v1.md:52`, `:81`, `:90`; `docs/decisions/0006_m2_storage_anchor_recovery.md:126`.

Trigger: a contained user opens a ceremony and invokes vaultd `recover_keys`. The vault writes fresh encrypted objects and sends certificates through identityd. It must then ask registryd to append `recovery_keys`, which names the keys, raises the minimum signing epoch, and retires every other contract key. The registryd operation table has **no operation for this record and no operation authorized for `service:vaultd`**. Its `append_identity` belongs only to `service:identityd` and accepts enrollment/certificate/invalidation/replacement records. `begin_recovery` and `close_recovery` belong to owner/admin and do not perform naming. There is no specified forwarding route through identityd either.

Consequently the frozen IPC package cannot implement the successful step-3 path without inventing an operation/role extension. Certificates alone intentionally grant no recovery signing authority; the user remains contained. The positive oracle scenario does not detect this gap: it calls `journal_t.admit_record(recovery_keys)` directly without transport or role authorization.

Required: specify a narrow operation or an explicit authenticated forwarding route that admits exactly the current ceremony's naming record. Freeze caller role, owner/ceremony/epoch and certified-key checks, retirement semantics, journal/anchor durability, and idempotent reconciliation after a durable naming record whose reply or anchor update failed. Add positive and unauthorized-caller model/interface controls; later real IPC/crash tests remain required. Do not introduce a generic caller-controlled append API.

Evidence: `model_attacks.log` reports `GAP no registryd operation or vaultd role admits recovery_keys; oracle directly appends it`; the portable static check below independently lists the registry operations. **FAIL** at design/interface level; runtime **NOT_RUN**. R13 is substantially repaired at the state-machine level but remains **FAIL** as a complete recovery package.

### R15 — Medium — Trusted-time limitations overstate the cross-boot and future-start bounds

Gate: **G5**. Location: `docs/decisions/0007_m2_lifecycle_time_consumption.md:61` (and the purported mitigating admission rule at `:65`).

The maximum-offset rule correctly prevents within-boot regression/freezing. However, the text claims a persistent expiration can be extended by at most the interval since the last persisted floor. That bound omits drift already accumulated during earlier boots. Reproduction using the candidate time model, with clean shutdown floors throughout:

1. Boot A persists trusted/actual 12:00.
2. Two actual hours pass powered off. The hardware clock is reset to 12:00 for boot B. Boot B runs 60 seconds and cleanly persists trusted 12:01, actual 14:01.
3. Boot C resumes from that new floor. Five real seconds later, trusted time is 12:01:05 and actual time 14:01:05. A persistent 12:30 expiration remains usable for **1,735 seconds**, although the latest persisted floor is only **five seconds** old and the expiration is already past in actual time.

Fresh floors preserve the accumulated lag rather than repairing it. Repeated boots/downtime with a corrected-back hardware clock can accumulate arbitrarily large lag absent a newly accepted accurate time source. This is a limitation of the stated software clock, not a newly claimed defense against root or kernel compromise.

The same paragraph says early `effective_start` caused by a forward wall-clock step is bounded by the commit-time rule. A previously admitted noon-timestamp grant with a future 15:00 start passes admission while the clock is noon; a later arbitrarily large accepted forward sample makes it eligible immediately. The ±300-second **admission timestamp** check never rechecks or bounds the actual-time acceleration of that already-admitted grant.

Required: state the limits honestly, including accumulated cross-boot lag and arbitrary acceleration of previously admitted future starts under trusted-clock manipulation. Add these cases to the model evidence. If a stronger bound is desired, propose its actual time source or fail-closed mechanism for owner review; do not infer it from timestamp skew. The reviewer selects no new time source or owner tradeoff.

Evidence: `time_limits_probe.log` and `model_attacks.log`, with portable reproduction below. **FAIL** for the stated bounds; service clock **NOT_RUN**. The original R09-r2 forward-sample/restart defects themselves are fixed and **PASS**.

## Prior finding verification

| Item | Result | Evidence and practical limit |
|---|---|---|
| R01-r2 | **PASS** | Original exit-7 `sha256sum` injected first in PATH under **sh and bash**: both collectors exit 1, `cargo_lock_sha256 FAILED`, `identity FAIL`. Unreadable-lock fixtures under both shells also exit 1; qualification executes the Python unreadable-lock canary. Valid clean source positive controls pass. |
| R07-r2 | **PASS, design/model** | Original sibling construction rejects `admission/network_grant_conflict`; harness checks both admission orders and each single-branch positive. Wire §§6.11–6.12 defines placement and a single lineage slot over all live heads. No runtime network/admission concurrency proof is claimed. |
| R09-r2 | **PASS, original regression/design model** | Noon → 14:00 forward sample → 13:00 correction → one elapsed hour reaches **15:00**; 14:30 expiry is past. Trusted floors, newer ledger offset, clean restart, long idle/suspend, maximum over out-of-order floors, and two-service adoption pass. New limitation finding R15 prevents G5 approval. |
| R12 | **PASS, design/model** | Original ancestral session restore rejects `admission/restore_rule`. Timed/session/one-shot ancestors through merges reject; persistent scope through the same ancestor passes. Temporary-to-persistent scope requires a separate new-id, freshly approved revision/merge. Recovery restores use the same persistent-only predicate and reject temporary-only historical scope. |
| R13 | **FAIL, residual interface gap R14** | Complete C1/C2 model paths, scoped key denial, retirement and progress controls pass. The required key-naming IPC route is missing, so the specified positive service path is incomplete. Actual vault/IPC/anchor/crash evidence remains NOT_RUN. |
| R02 | **PASS, intact** | Unchanged amd64 locks reproduced byte-for-byte: installed set `6cd45590bdebd08f53c95a56112a97030b844959983284db44232b59baa11d3c`; downloaded debs `6ce06e450a8b91cd2c91c1d3aaffc360dfed7276f6622ac03dbd05fd35f0f43a`. |
| R03 | **PASS, intact, probe/design** | Every Rust source unchanged. Exact-size Argon2 workspace owner/clearing paths and residual-library positive contrast execute in both profiles. Secret-region ownership, guarded/locked pages, clearing and abort limits inspected again. Actual vault/client custody NOT_RUN. |
| R04 | **PASS, intact, design** | Pre-service running marker; same-boot preserved ledger; in-memory → ledger → journal → anchor restriction order; missing/unverifiable ledger/reboot containment. New containment is durable before opening a ceremony. Actual disk/ledger/restart faults NOT_RUN. |
| R05 | **PASS, intact, design** | Identity/key authority remains in the same anchored journal, with sequence-scoped consistent answers, denial on unavailable/stale state, and no authority from restored vault files. `identity_seq` adds refusal of quarantine recovery when a lost tail included identity records. Actual cache/anchor freshness NOT_RUN. |
| R06 | **PASS, intact, oracle/design** | Own re-signed ordinary subject substitution rejects; revision/branch/merge/restore substitution vectors still reject. Only a displayed `identity_transition` changes the authoritative package/executable. |
| R08 | **PASS, intact, oracle/design** | Maximum envelope **61,440**, one-byte-over **61,441** rejection; own minimal IPC wrapper **61,533**; published journal wrapper **61,619**, within 65,536/4,096 budgets. Pagination unchanged; actual decoders NOT_RUN. |
| R10 | **PASS, intact, design** | One global in-memory anchor map, global lock, user→anchor lock order, whole-map slots, durability-before-reply remain. No concurrent/lost-update runtime evidence claimed. |
| R11 | **PASS, intact, design** | Denial/ledger → durable deletion_pending plus pending tombstone → erasure → durable deleted tombstone. Failed pre-erasure writes destroy nothing; restored tombstoned identity never loads; UID reuse enrolls new identity. Deletion now aborts a ceremony. Actual erase/crash/restore NOT_RUN. |
| I01 | **PASS, intact** | Anchor backends remain explicitly planned/unimplemented/untested, with same-volume degraded rollback limits. |
| I02 | **PASS, intact** | Actual secure-memory release test executable ran under Valgrind: **10 tests, zero errors**. Exec'd fault children are not instrumented; their parent/in-process tests are. |
| I03 | **PASS, intact** | Same-UID callable development client, terminal exposure, knowledge factor without trusted human presence, fixture identity/consumers and absence of actual isolation/enforcement remain explicit. |

The independently passed items R02–R06, R08, R10, R11 and I01–I03 are **intact**. The unchanged implementation/input check was `git diff --exit-code 6b4ef9d..84fa6ef -- core Cargo.toml Cargo.lock build/m2 .github PROJECT.md docs/decisions/0001_eternal_contract_repository.md` (exit 0).

### Identity audit

Inspected `m2_source_identity.sh`, `qualify_m2_rust.sh`, `build_m2_core.sh`, `run_m2_tests.sh`, and `m2_harness.py` beyond the injected-hash test. Hash output is checked before extracting/validating the digest; Git status is checked before counting; required environment commands and Python file reads report failures. Standalone build toolchain substitutions run under `set -e`; collector failure exits the build. Dispatcher/qualification aggregate failures and cannot convert later passing tests to overall PASS.

The literal claim that there are **no identity pipelines anywhere** is too broad: qualification still streams `dpkg-query | sort` for the installed-package identity, and the standalone build streams executable enumeration/sorting/hashing. These are explicitly checked under Bash `pipefail`/`set -e`; they do not mask failed producers as the prior lock-hash pipeline did. Architecture naming fallbacks are followed by the mandatory fail-closed identity collector. Optional Miri INFO/version output is not qualifying identity evidence. No remaining unchecked pipeline yielding a qualifying identity PASS was found. This is an audit conclusion within the actual script paths, not proof against arbitrary replacement of every host tool.

### Network and restore attacks

The 9 network scenarios cover chained branch heads, non-network siblings, a second network grant on a branch, non-overlapping lifetimes before a terminal record, premature expiry/wrong session, durable expiry/session/restriction, signed branch revocation, simultaneous proposals serialized in both orders, and trunk commits ending all live branches. Merge placement requires current trunk base and live other heads; restore requires an ancestor and persistent-only matching. Containment ends branches/old IDs and limits new signing to the recovery scope.

The exclusion argument is also structural: after each admission the **set of nonterminal network IDs in every resulting live state** has size ≤1. Current eligible network grants are a subset of that set. Same-ID inherited trunk copies do not create a second authority; new IDs must pass the union check. Expiry arithmetic alone cannot remove an occupant. A trunk replacement durably terminates prior branches as part of its own admission. Evaluation explicitly returns **Offline** for conflict, inconsistent snapshot, containment/quarantine or forbidden mode, never recency/iteration-order selection. These statements are specification/model evidence; real admission locks and resource transitions are NOT_RUN.

### Recovery and crash review

The oracle positive C1 path re-authorizes Kate while the editor remains suspended, then closes; later ordinary fresh approval can re-authorize the editor. Old IDs are terminal throughout. C2 requires admin to open; the user still authorizes each lineage. Named epoch-E keys alone can sign ceremony-bound trunk restore/revocation; old, abandoned, compromised, deleted-user and wrong-scope cases reject. Closing with some or all lineages unapproved does not revive their old grants: the suspended IDs remain terminal and later additions require ordinary fresh approval. Thus the prose sentence “No role can lift containment without step 4 commits” should be read as **no role can regain grant authority without fresh commits**; step 5 itself permits closing after step 3, even if the user declines all lineage restores.

Repeated model replay also inserted deletion and fresh containment at **11 contained prefix boundaries each**: ceremonies abort, old IDs remain ended, new containment suspends recovered IDs, and a new ceremony requires a higher epoch. This is neither a process crash nor a journal/anchor write-fault test. The ADR's journal→anchor→reply, durable-unacknowledged one-record roll-forward, unnamed-certificate retry, per-lineage atomic progress, deletion-first denial, and new-unverifiable-run abort rules are appropriately fail-closed on paper. R14 leaves the service positive path incomplete. All actual failure points are NOT_RUN. ADR 0006 §9.2 now correctly refers both C1/C2 choices to the entire §10 ceremony; these remain **owner choices**, neither selected here.

## Execution environment and exact command ledger

All local tests ran **natively on amd64, UID/GID 1000:1000**, inside the pinned Debian 13.7 image; outer executor also UID 1000, Debian 13, Linux **6.18.44**, x86_64. Docker **28.4.0**, local managed daemon, runc, default seccomp/cgroup namespace, vfs storage. No `--privileged`, added capabilities, emulation, `--record`, production keys, or weakened checks. Image ID: **sha256:09d23ddfc68b2093c48db406dd40e4e06c45f16855346eec87dbbcbd2aa799cd**. Build uses the unchanged pinned Dockerfile and managed proxy; no TLS checks were disabled. Network policy is enforced/restricted; no identity bindings were configured. Cloud-environment runtime guidance was read before Docker/network work.

Actual setup:

```sh
id
uname -m
stat -c '%u:%g %n' /tmp/cham3_r3_run

git -C /workspace/cham3 fetch origin codex/m2-cryptographic-contract-core:refs/remotes/origin/codex/m2-cryptographic-contract-core review/m2-001-002-r2-security:refs/remotes/origin/review/m2-001-002-r2-security
git clone --no-hardlinks /workspace/cham3 /tmp/cham3_r3_run && git -C /tmp/cham3_r3_run checkout --detach 84fa6ef172b3fad21bb079549776252bb65624d6
docker build -f /tmp/cham3_r3_run/build/m2/dev_container.dockerfile -t cham3-m2-r3-review /tmp/cham3_r3_run/build/m2
```

For the tables below `run` and `review_run` expand to these exact settings (the actual calls used the literal Docker prefixes):

```sh
run() {
  docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home \
    -e CHAM3_M2_REQUIRE_CLEAN=1 -e GIT_CONFIG_COUNT=1 \
    -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
    -v /tmp/cham3_r3_run:/work -w /work cham3-m2-r3-review "$@"
}
review_run() {
  docker run --rm --user 1000:1000 -e HOME=/tmp/cham3_home \
    -e CHAM3_M2_REQUIRE_CLEAN=1 -e GIT_CONFIG_COUNT=1 \
    -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
    -v /tmp/cham3_r3_run:/work:ro -v /tmp/cham3_r3_tools:/review:ro \
    -v /tmp/cham3_r3_evidence:/evidence -w /work cham3-m2-r3-review "$@"
}
```

Local evidence directory **E** = `/tmp/cham3_r3_evidence`. Container `/work` is `/tmp/cham3_r3_run`. Qualification directory **Q** = `out/m2/qualification/amd64_20261012T190150Z_1`; unit directory **U** = `out/m2/runs/unit_20261012T190237Z_1599`; harness directory **H** = `out/m2/runs/harness_20261012T190259Z_2675`. These are reviewer observations, separate from the author's checked-in evidence.

| Check | Status | Exact command | Evidence |
|---|---|---|---|
| 1: clean clone / target tree | PASS | Clone/detach above; `git rev-parse 84fa6ef^{tree}`; `git diff --exit-code`; `git status --porcelain --untracked-files=no` | Requested tree; clean before/after; owned by 1000:1000. |
| Evidence docs-only | PASS | `git diff-tree --no-commit-id --name-only -r 4239730` followed by asserting every emitted path starts `docs/` | 52 docs paths; no implementation changes. |
| 1: image build | PASS | Docker build above | E/docker_build.log, exit 0. |
| 1: qualification | PASS | `run bash build/scripts/qualify_m2_rust.sh` | E/qualification_console.log; Q/summary.txt, every required step PASS, exit 0. |
| 1: static checks | PASS | `run bash build/scripts/run_static_checks.sh` | E/static_checks.log; shellcheck, Python compilation, yamllint, configured secret scan all PASS. |
| 1: static tests | PASS | `run sh tests/run_tests.sh static` | E/static_tests.log; **82** tests, OK, exit 0. |
| 2: original hash failures | PASS (expected FAIL) | `review_run python3 -I /review/reviewer_checks.py`; nested `PATH=<synthetic failing_sha>:$PATH sh build/scripts/m2_source_identity.sh /evidence/failed_hash_sh_identity.txt --require-clean`, same with `bash` | E/failed_hash_sh.log, failed_hash_bash.log and identity files; each exit 1. Exact injected tool below. |
| 2: unreadable lock | PASS (expected FAIL) | Same reviewer script: synthetic Git fixture with copied script/lock, `chmod 000 Cargo.lock`, then each `sh`/`bash <fixture>/build/scripts/m2_source_identity.sh /evidence/unreadable_<shell>_identity.txt --require-clean` | E/unreadable_lock_sh.log, unreadable_lock_bash.log; exit 1; fixture owned/read-tested as nonroot. Python fixture executes in qualified harness. |
| 2: entry-point audit | PASS, with checked-pipeline qualification above | Identity-audit `rg` command below, plus full source inspection | E/identity_audit.txt; required statuses checked/propagated. |
| 3: old sibling probe | PASS (expected rejection) | `review_run python3 -I /review/r2_probes.py tests/m2/oracle/ecr_oracle.py tests/m2/vectors/ecr_v1/vectors.json` | E/r2_probes.log; four original checks pass, then exit 1 specifically `admission/network_grant_conflict`. Original script intentionally has no expected-rejection wrapper for that new r2 case. |
| 3: network attacks | PASS, model/design | `review_run python3 -I /review/model_attacks.py`; qualifying harness `test_m2_review_regressions.py`; wire/ADR source audit | E/model_attacks.log; all network/placement scenarios and both original sibling orders pass; runtime concurrency/enforcement NOT_RUN. |
| 4: within-boot time | PASS, model | Same model-attacks command; qualification's 12 `test_m2_trusted_time.py` tests | E/model_attacks.log; H/python_harness.log; no regression/freezing in exercised sequences. |
| 4: stated new-boot / start limits | FAIL | `docker run --rm --user 1000:1000 -v /tmp/cham3_r3_run:/work:ro -v /tmp/cham3_r3_tools:/review:ro -w /work cham3-m2-r3-review python3 -I /review/time_limits_probe.py` | E/time_limits_probe.log; counterexamples in R15; script completes exit 0 because it asserts the reproduction. |
| 5: ancestral temporary restore | PASS (expected rejection), model | `review_run python3 -I /review/model_attacks.py` separately executes the original r2 appendix restore section; qualification's R12 tests/scenario | E/model_attacks.log; `restore_rule`; temporary kinds through actual ancestors/merges rejected, persistent/fresh-revision positives pass. |
| 6: recovery state machine | PASS, model/design scope | Same model-attacks command; 9 recovery scenarios and qualified sensitivity tests | C1/C2 positive paths, old/invalid/abandoned/deleted/wrong-scope denial, per-lineage progress; containment/deletion at 11 prefixes each. |
| 6: recovery IPC completeness | FAIL | Same command's operation-table check; Recovery-route `rg` command below plus role-table inspection | Missing authorized route, R14. No service execution exists. |
| 7: preserved review tools | PASS | `tar -xzf docs/reviews/M2-001-002-codex-security-review-evidence.tar.gz -C /tmp/cham3_r3_tools`; `sha256sum --check SHA256SUMS` in extracted directory; original appendix extracted from `git show 336f13b:docs/reviews/M2-001-002-codex-security-rereview-r2.md` | Archive digest **8655477da0c83b186e9468cf874e85fc4a6536d9e5401327c44ef1c9159ab5af**; every archive member checks OK; r2 review digest **7c38e8a779a37caa87fd85187d0498264987020358ca23580fee7a9df2e1c532**. |
| 7: independent bytes/signatures | PASS | `review_run python3 -I /review/cham3_m2_review_evidence/independent_vectors.py tests/m2/vectors/ecr_v1/vectors.json` | E/independent_vectors.log; all **18** canonical objects, signatures, envelope/lineage hashes and changed-preimage rejections. Old final line still hardcodes 17; actual loop/18 named results establish 18. No author-oracle import. |
| 7: original schema holes | PASS (expected rejection) | `review_run python3 -I /review/cham3_m2_review_evidence/schema_holes.py tests/m2/oracle/ecr_oracle.py tests/m2/vectors/ecr_v1/vectors.json` | E/schema_holes.log; exit 1 at first case `subject_binding_mismatch`. Other original cases exercised separately by r2 probes/harness. |
| 7: byte-for-byte regeneration | PASS | `review_run python3 -I tests/m2/oracle/ecr_oracle.py check tests/m2/vectors/ecr_v1/vectors.json` | E/vector_regeneration.log and U/ecr_vector_oracle.log: **18 valid / 90 invalid / 18 admission / 19 scenarios (256 steps)**. Vector digest **2bbfc40e35339db75f6914f703d2c4202ca11c958f5bcef6e869e4ffd2f6dfee**. |
| 7: defect sensitivity | PASS, model/harness | Qualification invokes `python3 -I -m unittest discover -v -s /work/tests/m2/harness -p 'test_*.py'`; inspected `test_m2_review_regressions.py` mutations and assertions | H/python_harness.log; six controlled defects detected: no union slot, clock-only release, r2 restore predicate, missing recovery structure, ignoring contained/deleted state, no retirement/epoch floor. Real model methods are replaced, unexpected outcomes asserted, methods restored in finally; not tests of canned output. |
| 8: previously passed items | PASS, intact at original scope | Unchanged-core/input diff above; prior finding matrix inspected against current source; current qualification/vector/harness commands | Matrix above; NOT_RUN runtime limits preserved. |
| 9: Debian fetch | BLOCKED | `curl --fail --silent --show-error --max-time 45 https://security-tracker.debian.org/tracker/data/json -o /tmp/cham3_r3_evidence/debian_tracker.json` | E/tracker_fetch.log: **`curl: (22) The requested URL returned error: 403`**. No bytes/digest; policy not bypassed. |
| 9: own RustSec fetch | PASS | `git clone https://github.com/RustSec/advisory-db.git /tmp/cham3_r3_tools/advisory-db`; `git -C /tmp/cham3_r3_tools/advisory-db rev-parse HEAD` | E/rustsec_fetch.log; **57dcc0f2192e025138907a495c1ac9a7e1e3e123**. |
| 9: combined advisory tool | BLOCKED | `review_run python3 -I build/scripts/m2_advisory_review.py --tracker /evidence/debian_tracker.json --advisory-db /review/advisory-db --inventory out/m2/qualification/amd64_20261012T190150Z_1/dependency_inventory.json --out-dir /evidence/advisory_full` | E/advisory_full.log: exit 1, `FileNotFoundError: [Errno 2] No such file or directory: '/evidence/debian_tracker.json'`, following blocked retrieval. |
| 9: RustSec-only | PASS | `review_run python3 -I /review/rustsec_only.py out/m2/qualification/amd64_20261012T190150Z_1/dependency_inventory.json /review/advisory-db` | E/rustsec_only.log: own inventory, **37 crate versions, 6 nonwithdrawn advisories, zero affected**. Helper from prior review uses inspected candidate range predicate; all six ranges independently inspected. |
| 9: author 23-item triage | BLOCKED for independent clearance; bounded applicability judgment below | `git show 4239730:docs/evidence/M2-cryptographic-contract-core/advisory_review_r3/triage.tsv`; dependency/call-path inspection | No independent tracker facts to corroborate IDs/status/descriptions. Does not grant risk acceptance. |
| Standalone build entry point | NOT_RUN separately | `bash build/scripts/build_m2_core.sh` not invoked separately | Audited; underlying fresh debug/release builds executed by qualification. |
| Product integration/recovery/packaging | NOT_RUN | `sh tests/m2/run_m2_tests.sh integration`, `recovery`, `packaging` not invoked as product acceptance | Gated/unimplemented; dispatcher tests verify NOT_RUN/failure behavior only. |
| Miri/sanitizers/fuzzing | NOT_RUN | Qualification availability probe `cargo miri --version`; no Miri/sanitizer/cargo-fuzz analysis run | Pinned toolchain lacks support; script's NOT_AVAILABLE is not a security PASS. |
| Actual A04–A15 services/IPC/journal/anchor/time/recovery/enforcement | NOT_RUN | No runtime acceptance command exists/executed | Skeletons, oracle journal and time reference model are design evidence only. |

The source-audit commands referenced above were run in `/tmp/cham3_r3_run`:

```sh
rg -n 'unsafe|sha256sum|\$\(|\||subprocess|command_output' build/scripts/qualify_m2_rust.sh build/scripts/build_m2_core.sh tests/m2/run_m2_tests.sh tests/m2/lib/m2_harness.py
rg -n 'recovery_keys|append_identity|unknown_operation' docs/specs/m2_service_ipc_v1.md docs/decisions/0006_m2_storage_anchor_recovery.md
```

### Qualification subchecks

All required entries below are **PASS**, under the single exact `run bash build/scripts/qualify_m2_rust.sh` invocation. The candidate pins the nested script and its heredocs; no command was modified. Evidence names are under Q unless U/H specified.

| Subcheck | Actual nested command / check | Evidence |
|---|---|---|
| Source identity/platform | `bash build/scripts/m2_source_identity.sh <Q>/identity.txt --require-clean`; Debian 13/amd64/unprivileged checks | identity.txt: candidate/tree, dirty 0, UID 1000; Cargo.lock **fc87901c5e58acdba21a33fe516871e45a81072585bfa2ae82aa8e60c40310f6**. |
| Toolchain / package identities | `rustc -Vv`, `cargo -V`, `cc --version`, `ld --version`, `ldd --version`; `dpkg-query -W -f '${Version}' <each pin>`; full installed-set and downloaded-deb lock comparisons | toolchain.txt: Rust/Cargo **1.85.1**, target x86_64-unknown-linux-gnu, GCC 14.2.0-19, binutils 2.44-3, glibc 2.41-12+deb13u4; both diff logs empty. |
| Metadata/source content | `cargo metadata --locked --offline --format-version 1`; `python3 -I build/scripts/m2_dependency_inventory.py sources --metadata <Q>/metadata.json --check build/m2/crate_sources.sha256` | metadata.log/json and crate_sources.log; 1,454 files checked. |
| Dependency inventory | `python3 -I build/scripts/m2_dependency_inventory.py inventory --metadata <Q>/metadata.json --out-dir <Q>` | dependency_inventory.json/md; 33 compiled dependencies; JSON digest **7ff6c05870994425b829aed7b26c15ca70af27d35c42c98a0004c66425a17b67**. |
| Lints | `cargo fmt --all --check`; `env CARGO_TARGET_DIR=<Q>/clippy_target cargo clippy --workspace --all-targets --frozen -- -D warnings`; `python3 -I build/scripts/check_rust_naming.py` | rustfmt.log, clippy.log, rust_naming.log. |
| Profiles | Exact Python TOML-check heredoc in `qualify_m2_rust.sh` | profile_settings.log; overflow checks all profiles, release abort/dev unwind. |
| Fresh builds | `env CARGO_TARGET_DIR=<Q>/build_target cargo build --workspace --frozen`; same with `--release --verbose` | build_debug.log, build_release.log, no stale target reuse. |
| Backend flags | Exact verbose-build grep patterns and checked build-script-output grep/count in `qualify_m2_rust.sh` | chacha/poly force_soft, sha force-soft, curve fiat; 2 fiat outputs. |
| Provider memory check | `valgrind --error-exitcode=99 --track-origins=yes <Q>/build_target/release/cham3_provider_probe report` | valgrind_memcheck_probe.log, zero errors. |
| Secure-memory execution | `env CARGO_TARGET_DIR=<Q>/build_target cargo test --release --frozen -p cham3_secure_memory --no-run --message-format=json`, then compiler-artifact-selected `valgrind --error-exitcode=99 --track-origins=yes <actual-test-executable> --test-threads=1` | secure_memory_test_build.json and valgrind_memcheck_secure_memory.log; 10 executed tests, zero errors, documented child limit. |
| Unit suite | `sh tests/m2/run_m2_tests.sh unit`: actual `cargo test --workspace --frozen` and same `--release`, provider/oracle comparison, ECR check | suite_unit.log and U; **36+36** exact-inventory tests, 7 provider checks, 18/90/18/19/256 vector counts. |
| Harness suite | `sh tests/m2/run_m2_tests.sh harness`, actual Python unittest command above, debug/release provider builds | suite_harness.log and H; **102** exact-inventory tests; assertion/compile/link/panic/zero/ignore/filter/compile-only and unsafe/naming/identity/deployed-profile controls executed. |

Q/summary.txt SHA-256 **4db6741eaec742c0d0d2b3069ea78b8933b51e035dab1737e7adf295104d9d16**. `diff -u /tmp/cham3_r3_evidence/author_vm_summary.txt /tmp/cham3_r3_run/out/m2/qualification/amd64_20261012T190150Z_1/summary.txt` (author file obtained from evidence commit with `git show`) exits 0. This compares observed summary bytes, not just author labels.

### Advisory triage judgment

RustSec has no affected version in the independently fetched database for these inputs. Inspected ranges: blake2 ≥0.8.1; chacha20 ≥0.2.3; curve25519-dalek ≥4.1.3; ed25519-dalek ≥2; generic-array patched ranges including ≥0.13.3; sha2 ≥0.9.8 (or unaffected <0.9.7). This is not a complete unsafe/side-channel proof.

The author's 23 rows are reasonably separated by **current probe binary**, **test oracle**, and **future host/setgid service deployment** reachability. Rust probe sources do not call the listed glibc locale/formatting/tree/glob/iconv/resolver functions or link a C++ application layer; ppc64le-specific code is outside amd64. Cryptography X.509 functionality is not used by the Ed25519 checks. cbor2 remains a test tool, not the future Rust parser; acceptance of its risk for future untrusted inputs is not granted. Dynamic-loader/setgid and systemd host risks must be reconsidered before M2-007, rather than dismissed because the current probes do not install services.

Those are bounded source/scope judgments **conditional on accurate advisory descriptions**. The author's “accept for test use” is author triage, not owner risk acceptance. With the live tracker blocked, I cannot corroborate each description/status/fixed version or independently clear those 23 entries. The author's tracker SHA is not reported as a reviewer-fetched SHA. Completing that independent review remains a G1/M2-001 prerequisite.

## CI comparison — check 10

Independently called `github_fetch_workflow_run_jobs(repo_full_name="Kayyo321/cham3", run_id=<id>)` and `github_fetch_workflow_run_artifacts` for each of **38292641158 / 38292641153 / 38292641431**. Artifact metadata binds all three runs to **84fa6ef172b3fad21bb079549776252bb65624d6**, branch `codex/m2-cryptographic-contract-core`. Every reported job and step completed successfully. Raw metadata: E/ci_metadata.json.

| Check | Status | Exact connector call | Evidence / limit |
|---|---|---|---|
| m2_checks | PASS | `github_fetch_workflow_job_logs(repo_full_name="Kayyo321/cham3", job_id=114956138804)` | E/m2_ci_job.log; exact candidate, clean tree, Debian 13.7 amd64, UID 1001, Linux 6.17.0-1022-azure; every required qualification step PASS, same summary as local. Artifact 11766997724, 85,633 bytes, metadata digest **20d8c277f175c1309c57275917c06f8b26e796a5002083fe494560a4a6554374**. |
| m1_checks | PASS, CI scope | Jobs call for run 38292641153; logs call for job **114956138720** | static_and_desktop, package_build **114956139034**, privileged_capture **114956139059** all success. E/m1_static_ci_job.log: exact candidate; 82 static, 261 desktop static, 16 staging and 22 apt-pin tests. This reviewer ran no root/privileged suites; remote CI status is not a local rerun. |
| m0_checks | PASS, CI scope | Jobs call for run 38292641431; logs calls for jobs **114956139435** and **114956139067** | static_checks, package_build **114956139375**, image_build all success. E/m0_static_ci_job.log: 82 static; E/m0_image_ci_job.log: image build, 17 image tests, boot reaches serial login prompt, 18 boot-result tests. No local ISO/desktop boot claim. |
| CI ZIP bytes | NOT_RUN | No artifact ZIP download/byte comparison performed | Metadata and decoded job logs inspected; the published ZIP hashes are metadata, not reviewer-computed downloaded hashes. |

## Portable reproductions and publication boundaries

The preserved `independent_vectors.py` and `schema_holes.py` are unchanged archive members. `r2_probes.py` is extracted unchanged from the prior review appendix (digest **8dcdde94144edb014a662ba0d0b69db356b095b02499272fbd68092daa16ee8f**). Because that script now stops on the fixed sibling case, its restore section was run separately, then the entire journal scenario set and additional prefix/time/interface probes were run by `/tmp/cham3_r3_tools/model_attacks.py` (digest **491a0e96c7076e8b687641f37e6c3a9f65981776b4552e9a5f500671da4279de**). `/tmp/cham3_r3_tools/reviewer_checks.py` records each subprocess status and identity fixture result. These local scripts/logs are outside the implementation and are not published as extra repository files.

Original synthetic failed-hash tool (mode 0755, first in PATH):

```sh
#!/bin/sh
printf '%s\n' 'reviewer injected sha256sum failure' >&2
exit 7
```

R14 static reproduction, from the candidate root (no service execution):

```sh
python3 -I - <<'PY'
import pathlib, re
text = pathlib.Path('docs/specs/m2_service_ipc_v1.md').read_text()
table = text.split('### cham3-registryd')[1].split('## 4.')[0]
rows = re.findall(r'^\| (.*?) \| (.*?) \|', table, re.M)
print(*rows, sep='\n')
assert not any('recovery_keys' in operation for operation, role in rows)
assert not any('service:vaultd' in role for operation, role in rows)
print('GAP: no registryd key-naming operation or authorized vaultd caller')
PY
```

R15 portable counterexample: save as `time_limits_probe.py` outside the repository, then run the exact Docker command in the ledger. It deliberately asserts/prints the reproduced **FAIL** of the claims; its exit 0 means reproduction succeeded.

```python
import importlib.util
spec = importlib.util.spec_from_file_location('time_model', '/work/tests/m2/oracle/trusted_time_model.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
hour = 3600
# Boot A shuts down cleanly at trusted/actual noon.
a = m.trusted_clock_t('a', 12 * hour, 0)
f0 = a.floor(0)
# Two actual hours pass while powered off; realtime is reset to noon for boot B.
b = m.trusted_clock_t('b', 12 * hour, 0, [f0])
f1 = b.floor(60)  # clean shutdown, trusted 12:01, actual 14:01
# Boot C starts with the same corrected realtime. The newest floor is five seconds old.
c = m.trusted_clock_t('c', 12 * hour + 60, 0, [f0, f1])
trusted = c.now(5)
actual = 14 * hour + 65
expiration = 12 * hour + 30 * 60
assert trusted == 12 * hour + 65
assert actual > expiration and trusted < expiration
assert expiration - trusted == 1735
print('FAIL stated latest-floor bound: floor age 5 s, remaining authorization 1735 s')
# A grant admitted at noon has a valid noon timestamp and a future 15:00 start.
d = m.trusted_clock_t('d', 12 * hour, 0)
assert d.now(0) < 15 * hour
assert d.sample(30 * 24 * hour, 1) >= 15 * hour
print('FAIL stated commit-time bound: an already-admitted future start can advance arbitrarily')
```

Two reviewer-script setup errors (extracting a comment suffix and a regex containing backticks, followed by an incorrect inventory path) were corrected before the final probe/advisory-only runs; these were not candidate failures or passing checks. Final results above come from the corrected complete runs.

Publication is **this review record only**. Publication checks: clean candidate unchanged, staged whitespace check, single-path staged/committed diff against main, commit and push only to the review branch, then remote-head verification. The remote review ref was initially at main (9cf8825); publication is a fast-forward from that base. No merge, implementation edits, review-record edits on earlier branches, PR review/comment, or owner choice. Remaining owner choices: G1–G6 decisions; default anchor backend; C1/C2 opening role; one-shot consumption; deletion granularity. Fix R14/R15 and obtain independent Debian evidence before reconsidering the affected gates; actual runtime work remains gated regardless of model PASS.
