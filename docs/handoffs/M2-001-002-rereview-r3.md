# Handoff: M2-001 / M2-002 revision 3 — request for independent re-review

- **State:** M2-001 and M2-002 revision 3 **IN_REVIEW**. Every G1–G6 proposal (ADRs 0003–0008) is PROPOSED (revision 3); none is approved. M2-003..M2-008 remain DRAFT. Full M2 is not DONE.
- **Role:** IMPLEMENTER (and ARCHITECT for the proposals). The author approves nothing, including its own security-sensitive changes.
- **Model/provider:** Claude Code (Anthropic). Requested reviewer: Codex (independent).
- **Working directory / branch:** Anthropic cloud VM `/home/user/cham3`, `codex/m2-cryptographic-contract-core` (pushed fast-forward to `origin`).
- **Task specification:** [M2 task](../tasks/M2-cryptographic-contract-core.md) (M2-001, M2-002, G1–G6); [Claude prompt](../prompts/m2_claude_implementation.md).
- **Review answered:** `docs/reviews/M2-001-002-codex-security-rereview-r2.md` at 336f13be6a19bb871ea956d64f9cacb0936fe028 on `review/m2-001-002-r2-security` (parent 9cf8825, review file SHA-256 `7c38e8a7…c532`, verified; not merged, not edited): CHANGES_REQUESTED for M2-001 and G1–G6. Author response: [finding resolutions r2](../reviews/M2-001-002-finding-resolutions-r2.md).
- **Baseline commit:** 6b4ef9d87d69185ff7205b51fa417df1fed5c6f6 (= `origin/codex/m2-cryptographic-contract-core` at start; revision-2 candidate a2bbb370c867da57fdc72274fb17ca19a23426e4 plus its docs-only evidence commit), clean.
- **Revision-3 candidate (exact re-review target for code, specs, CDDL, IPC, ADRs, oracle, vectors, tests):** **84fa6ef172b3fad21bb079549776252bb65624d6**, tree 7e3bc1a4cedfff60ad437deca05428c45c26dcdc. The next commit (adding this handoff, evidence, task and status progress) changes only `docs/`.

Product authority: PROJECT.md and approved ADR 0001.

## Completed work

| Finding | Gates | Status | Where |
|---|---|---|---|
| R01-r2 failed lock hash → identity PASS | M2-001, G1 | implemented: hash status checked, 64-hex digest required, no pipeline/unchecked values; qualify/build/Python collectors audited and fixed; 7 canaries | `build/scripts/m2_source_identity.sh`, `qualify_m2_rust.sh`, `build_m2_core.sh`, `tests/m2/lib/m2_harness.py`, `tests/m2/harness/test_m2_source_identity.py`; ADR 0003 §8 |
| R07-r2 conflicting live network branches | G2, G5 | design revised + oracle/vectors: lineage network slot across trunk and all live temporary heads, durable-record release, placement/`stale_head`, branch revocation, conservative Offline evaluation, transition rules | wire format §§6.10–6.12, §7; ADR 0004 §7b–7c; ADR 0007 §§1–2; IPC; `tests/m2/oracle/ecr_oracle.py` `journal_t`; 9 scenarios + 2 vectors |
| R09-r2 trusted time not rebased/persisted | G5 | design revised + executable reference model: BOOTTIME + maximum accepted offset per boot; trusted floors; shared offset (`time_sample`); new-boot limits kept | ADR 0007 §§5, 7; IPC; `tests/m2/oracle/trusted_time_model.py`, `tests/m2/harness/test_m2_trusted_time.py` (12 tests) |
| R12 restore converts session to persistent | G2, G6 | design revised + oracle/vectors: persistent-only matching, ancestor requirement, explicit fresh-approval route | wire format §6.7; ADR 0004 §7; ADR 0008 §8; 2 vectors + 1 scenario |
| R13 containment blocks recovery signing | G3, G4, G6 | design revised + oracle/vectors: grant containment vs scoped key authority; staged ceremony (contain → open → fresh keys at new epoch → per-lineage ceremony-bound restores → close) with ordering, progress, crash and bypass rules; quarantine exits to contained; ADR 0006 §9.2 rewritten | ADR 0006 §§2–5, 9.2, 10; ADR 0005 §2; ADR 0008 §§4–5; IPC (`resolve_key` scope, `recover_keys`, `begin_recovery`, `recovery_status`, `close_recovery`, `recover_user`); oracle key-authority layer; 9 scenarios |

Also: ADRs 0003–0008 and the decisions index marked revision 3; CDDL comments (no type change); vector README; reviewers' cases (R07-r2 siblings in both orders, R12 ancestral session restore, R09-r2 time sequence, R01-r2 failing hash) are harness regression tests with positive controls, and six controlled oracle defects are detected by the published scenarios. Unchanged: every Rust source, Cargo files, Dockerfile, pins, locks, crate sources, CI workflows, PROJECT.md, ADR 0001 and both review records.

Decisions made within approved scope (all PROPOSED, for review): lineage placement and branch revocation as the signed revocation of temporary grants; network slot release only by durable records; Offline on any network inconsistency; maximum-offset time rule with registryd owning the offset; recovery binding via `authorization.session_id = recovery_id` (no schema change); recovery keys always freshly generated at a new epoch; anchor `identity_seq` for the quarantine exit.

## Verification

All runs below are at the exact candidate 84fa6ef unless stated. "VM" = Anthropic cloud VM, Ubuntu 24.04, native x86_64, kernel 6.18.44, Docker 29.8.2; image `cham3-m2-dev` from the unchanged Dockerfile (Debian 13.7 amd64); clean clone owned by UID 1000; containers `--user 1000:1000`, no `--privileged`, no added capabilities, no `--record`.

| Check | Environment / command | Result | Evidence / limits |
|---|---|---|---|
| Baseline and review provenance | `git rev-parse origin/codex/m2-cryptographic-contract-core` = 6b4ef9d…; `git show 336f13be` parent 9cf8825, adds only the re-review, SHA-256 `7c38e8a7…c532`; archive at 9cf8825 SHA-256 `8655477d…af`; extracted script digests match its `SHA256SUMS` | PASS | Scripts kept in `/tmp/cham3_r3_tools`, outside the repository |
| Reproduction before change | VM, baseline clone: failing `sha256sum` first in PATH + `bash build/scripts/m2_source_identity.sh … --require-clean`; `r2_probes.py`; `schema_holes.py` | reproduced (identity exit 0, empty digest, PASS; both new cases ACCEPTED) | [baseline_6b4ef9d](../evidence/M2-cryptographic-contract-core/m2_001_amd64_vm_84fa6ef/baseline_6b4ef9d/) |
| Full M2 qualification | VM: `docker run … -e CHAM3_M2_REQUIRE_CLEAN=1 … cham3-m2-dev bash build/scripts/qualify_m2_rust.sh` | **PASS** (exit 0; every required check) | [m2_001_amd64_vm_84fa6ef](../evidence/M2-cryptographic-contract-core/m2_001_amd64_vm_84fa6ef/): clean identity, both amd64 locks (empty diffs), crate sources, inventory, rustfmt, clippy, naming, profiles, fresh debug/release builds, soft backends, valgrind probe and secure-memory (10 tests, 0 errors), unit (36+36; ECR oracle 18/90/18 + 19 scenarios/256 steps), harness (102). amd64 author development evidence |
| Static gates | VM, same container settings: `bash build/scripts/run_static_checks.sh`; `sh tests/run_tests.sh static` | PASS / PASS (82 tests) | `static_checks.log`, `static_tests.log`; clone still clean afterwards |
| GitHub Actions on the candidate | `m2_checks` [38292641158](https://github.com/Kayyo321/cham3/actions/runs/38292641158), `m1_checks` [38292641153](https://github.com/Kayyo321/cham3/actions/runs/38292641153), `m0_checks` [38292641431](https://github.com/Kayyo321/cham3/actions/runs/38292641431) | **PASS** (all runs, jobs and steps success) | [m2_001_amd64_ci_84fa6ef](../evidence/M2-cryptographic-contract-core/m2_001_amd64_ci_84fa6ef/): CI qualification summary identical to the VM's; kernel 6.17.0-1022-azure, UID 1001 |
| CI artifact ZIP / raw log download | `gh run download 38292641158 -n m2_qualification_amd64`; `gh api …/jobs/114956138804/logs` | BLOCKED | Redirect to `productionresultssa14.blob.core.windows.net` denied by the session egress policy (403 on CONNECT); decoded log obtained through the GitHub connector instead; artifact metadata only |
| Reviewer regression tools | VM, candidate read-only: `independent_vectors.py`, `schema_holes.py`, `r2_probes.py` against `tests/m2/oracle/ecr_oracle.py` and `vectors.json` | PASS / expected rejection (exit 1 at `subject_binding_mismatch`) / expected rejection (exit 1 at `network_grant_conflict`, after its four original rejections) | `review_tools/`; R12 case replayed separately in the harness (`restore_rule`) |
| R01-r2 at candidate | VM: the re-review's failing-`sha256sum` command | PASS (exit 1, `cargo_lock_sha256 FAILED`, identity FAIL) | `r01_r2/failed_hash_identity_84fa6ef.log` |
| R01-r2 canary sensitivity | VM: new test module against the 6b4ef9d script/collector | PASS (6 negatives fail; positive passes) | `r01_r2/canaries_against_revision2_scripts.log` |
| Oracle defect sensitivity | harness `test_m2_review_regressions.py`: per-state-only slot, clock-based release, revision-2 restore predicate, unchecked recovery structure, containment not applied to keys, old keys not retired | PASS (each defect detected) | harness suite |
| Advisory review | VM container: `m2_advisory_review.py` on tracker SHA-256 `52d2faea…5dc6` (fresh fetch, identical to rev 2) and advisory-db `57dcc0f2192e025138907a495c1ac9a7e1e3e123` | PASS (0 behind, 0 affected, 0 open in rustc/rust-*); 23 open items triaged | [advisory_review_r3](../evidence/M2-cryptographic-contract-core/advisory_review_r3/advisory_readme.md); **author evidence**, reviewer's own Debian check still required |
| Miri / sanitizers / coverage-guided fuzzing | — | NOT_RUN (NOT_AVAILABLE on the pinned toolchain) | never PASS |
| Network slot, recovery ceremony, trusted clock, vault, IPC, registry, anchors, enforcement (A04–A15) | — | NOT_RUN | design + oracle/reference-model evidence only; gated M2-004..M2-007 |
| `tests/m2/run_m2_tests.sh integration|recovery|packaging` | — | NOT_RUN | gated suites report NOT_RUN and fail by design |

## Review

- **Reviewer and exact commit:** pending — Codex, candidate 84fa6ef172b3fad21bb079549776252bb65624d6 (range `6b4ef9d..84fa6ef`) and the following docs-only commit.
- **Findings and disposition:** R01-r2 implemented; R07-r2, R09-r2, R12, R13 design revised with oracle/vectors or reference model; R02–R06, R08, R10, R11, I01–I03 kept intact ([matrix](../reviews/M2-001-002-finding-resolutions-r2.md)).

## Owner decisions still open (none taken)

1. Approve, amend or reject each of G1–G6 (ADRs 0003–0008, revision 3).
2. ADR 0006 §9.1 default anchor mode (recommended A: degraded `same_volume` with visible status).
3. ADR 0006 §9.2 **rewritten**: who opens the complete recovery ceremony of §10 (recommended C1: the user after containment, admin `recover_user` first after quarantine; C2: admin opens every ceremony).
4. ADR 0007 §4 one-shot semantics (recommended A: reservation consumes).
5. ADR 0008 §3 deletion granularity (recommended A: whole-account only).

## Remaining

- **Known issues/risks:** all design mechanisms unimplemented; the oracle journal model does not evaluate time, sessions, mandatory policy or policy epochs; a user switching a temporary branch from Direct to Tor must end the first grant before the second is admitted (usability cost of the slot rule); reboots after unclean runs require the full ceremony including key rotation; cross-boot clock rollback by root remains a stated limit; the advisory triage is author judgment; glibc `ld.so` and systemd items must be rechecked before M2-007 packaging; the CI artifact bytes were not inspected (BLOCKED).
- **Uncommitted paths or dirty state:** none after the evidence/handoff commit (`out/` is ignored).
- **Required next action and owner:** Codex re-reviews 84fa6ef (and the docs-only commit) → implementer repairs any findings → owner decides G1–G6 and the four choices → only then may M2-003 onward become READY.

## Re-review prompt for Codex

~~~text
You are the independent REVIEWER (Codex) for cham3 M2-001/M2-002 revision 3 in Kayyo321/cham3. You did not author
this work. Read AGENTS.md, PROJECT.md, approved ADR 0001, docs/tasks/M2-cryptographic-contract-core.md,
docs/engineering/{WORKFLOW,TESTING,SECURITY_INVARIANTS}.md, your re-review (review/m2-001-002-r2-security at
336f13be6a19bb871ea956d64f9cacb0936fe028), docs/reviews/M2-001-002-finding-resolutions-r2.md and
docs/handoffs/M2-001-002-rereview-r3.md. Treat author resolutions and evidence as claims to verify.

Targets: candidate 84fa6ef172b3fad21bb079549776252bb65624d6 (range 6b4ef9d87d69185ff7205b51fa417df1fed5c6f6..84fa6ef)
and the following docs-only evidence/handoff commit on origin/codex/m2-cryptographic-contract-core. Confirm the
second commit changes only docs/. Do not merge main or review branches into the implementation branch.

1. Qualify natively on amd64 from a clean clone owned by an unprivileged UID, without --privileged or added
   capabilities and without --record:
     docker build -f build/m2/dev_container.dockerfile -t cham3-m2-r3-review build/m2
     docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
       -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
       -v "$PWD:/work" -w /work cham3-m2-r3-review bash build/scripts/qualify_m2_rust.sh
   plus build/scripts/run_static_checks.sh and sh tests/run_tests.sh static in the same container settings.
2. R01-r2: repeat your failing-sha256sum reproduction (sh and bash) and an unreadable-lock fixture; confirm FAIL
   and that no other M2 entry point (qualify_m2_rust.sh, build_m2_core.sh, run_m2_tests.sh, m2_harness.py) still
   takes an identity value from a pipeline or an unchecked command.
3. R07-r2: re-run your r2_probes.py sibling case (expect admission/network_grant_conflict); attack wire format
   §§6.11-6.12 and ADR 0007 §1: any way to make two network grants eligible in one lineage (overlap, chained
   branches, merges, restores, containment, lifecycle-record timing, simultaneous proposals), any slot freed
   without a durable terminal record, any evaluation that picks a mode instead of Offline.
4. R09-r2: try to make trusted time regress or freeze under ADR 0007 §5 (forward step, correction, idle,
   suspend, same-boot restart, clean shutdown, two services, ledger vs journal floors, new boot) using
   tests/m2/oracle/trusted_time_model.py or your own model; check that the stated new-boot limits are honest.
5. R12: re-run your ancestral session restore (expect restore_rule) and try timed/one-shot/session scope through
   any ancestor, including merges and recovery restores; confirm the fresh-approval route stays separate.
6. R13: review ADR 0006 §10 against ADR 0005 §2 and the IPC spec: can recovery be completed (positive path), can
   any pre-ceremony, compromised, abandoned, deleted-user or out-of-scope key sign, can any role lift containment
   without per-lineage fresh approval, are journal/anchor ordering, per-lineage progress and every crash case
   (including a new unclean run mid-ceremony and deletion mid-ceremony) fail-closed, and do old grants stay denied
   throughout? Check that ADR 0006 §9.2 options refer to the complete ceremony and remain owner choices.
7. Re-run independent_vectors.py, schema_holes.py and r2_probes.py; check tests/m2/vectors/ecr_v1/vectors.json
   (18 valid, 90 invalid, 18 admission, 19 journal scenarios) regenerates byte for byte and that the harness
   defect-sensitivity tests would fail on the controlled defects they name.
8. Confirm the items you passed (R02-R06, R08, R10, R11, I01-I03) are intact.
9. Repeat the Debian security tracker and RustSec advisory review with your own access
   (build/scripts/m2_advisory_review.py) and judge the 23 open items' triage.
10. Compare GitHub Actions m2_checks 38292641158, m1_checks 38292641153 and m0_checks 38292641431 at 84fa6ef.

Record every check as PASS/FAIL/NOT_RUN/BLOCKED with exact commands, environment and evidence; mocks and the
oracle model are not runtime evidence. Return APPROVE or CHANGES_REQUESTED for M2-001 and for each of G1-G6.
Do not make owner decisions, do not edit implementation files, and publish only your review record.
~~~
