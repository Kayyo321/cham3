# Handoff: M2-001 / M2-002 revision 4 — request for independent re-review

- **State:** M2-001 and M2-002 revision 4 are **IN_REVIEW**. Every G1–G6 proposal (ADRs 0003–0008) is PROPOSED; none is owner-approved. G2 carries the reviewer's f451f2e recommendation "APPROVE for owner consideration". M2-003..M2-008 remain DRAFT. Full M2 is not DONE.
- **Role:** IMPLEMENTER (and ARCHITECT for the proposals). The author approves nothing, including its own security-sensitive changes.
- **Model/provider:** Claude Code (Anthropic). Requested reviewer: Codex (independent).
- **Working directory / branch:** Anthropic cloud VM `/home/user/cham3`, branch `codex/m2-cryptographic-contract-core`, pushed fast-forward to `origin`.
- **Task specification:** [M2 task](../tasks/M2-cryptographic-contract-core.md) (M2-001, M2-002, G1–G6); [Claude prompt](../prompts/m2_claude_implementation.md).
- **Review answered:** `docs/reviews/M2-001-002-codex-security-rereview-r3.md` at f451f2e346e6981b24802a7e9070ba09303c40d3 on `review/m2-001-002-r3-security` (parent 9cf8825; review file SHA-256 `4881c1e9…0d`, verified; not merged, not edited).
  - CHANGES_REQUESTED: M2-001, G1, G3, G4, G5, G6.
  - APPROVE for owner consideration: G2.
  - New findings: R14, R15.
  - Author response: [finding resolutions r3](../reviews/M2-001-002-finding-resolutions-r3.md).
- **Baseline commit:** 42397304663bff6057e26118014c18e7b90a2040 (= `origin/codex/m2-cryptographic-contract-core` at start: revision-3 candidate 84fa6ef plus its docs-only evidence commit), clean.
- **Revision-4 candidate:** **9b1aa2b9d1435fb33c303ca7fe01ce7afcb2964a**, tree cb9f15101455f6c670a5f901f1e829544d45ef7d. This is the exact re-review target for specs, IPC, ADRs, oracle, vectors and tests. The next commit, which adds this handoff, the evidence, task progress and status, changes only `docs/`.

Product authority: PROJECT.md and approved ADR 0001.

## Completed work

| Finding | Gates | Status | Where |
|---|---|---|---|
| R14: no authenticated route for the step-3 key-naming record | G3, G4, G6 (R13 residual) | design revised + oracle/vectors:<br>• registryd `name_recovery_keys`, callable only by `service:vaultd`, with closed arguments; registryd builds the record itself;<br>• exact checks (contained user, open ceremony, two keys certified at the ceremony epoch by the current identity key);<br>• journal → anchor → reply, effective only once anchored; the pending-tail rule; idempotent retries;<br>• `recovery_status` serves vaultd; vaultd reconciliation (never erases on unknown status or named keys; resumes with held keys);<br>• `append_identity` refuses naming records. | IPC spec (`name_recovery_keys`, `recovery_status`, `recover_keys`, `register_certificate`, `append_identity`); ADR 0006 §10; ADR 0005 §2; ADR 0008; oracle `registry_service_t`, `check_naming`/`admit_naming`, `vault_reconcile`; 3 new scenarios; 9 harness tests |
| R13 bypass wording | G4 | Adopted the re-review's reading: no role regains *grant* authority without fresh commits | ADR 0006 §10 |
| R15: overstated time limits | G5 | design revised + reference model:<br>• honest limits — accumulated cross-boot lag; accepted forward errors accelerate admitted future starts and are never corrected; honest timestamps refused afterwards;<br>• new owner choice ADR 0007 §8 (A accept; B observed-time start eligibility, recommended; C authenticated time source);<br>• time rule unchanged. | ADR 0007 §§5.6–5.7, §8; `trusted_time_model.py`; 8 harness tests |
| Identity-audit wording | G1 | ADR 0003 §9 now names the two remaining checked pipelines; no script change | ADR 0003 §9 |

Also:
- Review outcomes are recorded in each ADR, and the decisions index is updated.
- ADR 0004 and wire format §9 received count and description edits only. G2 normative text is unchanged.
- `vectors.json`:
  - Byte-identical except `journal_scenarios`.
  - The network and restore scenarios are identical.
  - Each recovery scenario differs only in its naming step.
  - 3 scenarios are new.
- Unchanged: every Rust source, the Cargo files, `build/` (Dockerfile, pins, locks, scripts), crate sources, CI workflows, `tests/m2/run_m2_tests.sh`, `tests/m2/lib/`, PROJECT.md, ADR 0001 and every review record.

Decisions made within approved scope (all PROPOSED, for review):
- Naming is a dedicated vaultd-only registryd operation rather than a forwarding route through identityd.
- registryd composes the naming record.
- A durable unanchored record blocks the user's later operations until it is anchored.
- vaultd resumes with the keys it holds instead of regenerating them.
- Option B is recommended for §8.

## Verification

All runs below are at the exact candidate 9b1aa2b unless stated.
- **VM:** Anthropic cloud VM, Ubuntu 24.04, native x86_64, kernel 6.18.44, Docker 29.8.2.
- **Image:** `cham3-m2-dev` from the unchanged Dockerfile (all steps cached; Debian 13.7 amd64).
- **Run conditions:** clean clone owned by UID 1000; containers `--user 1000:1000`, no `--privileged`, no added capabilities, no `--record`.

| Check | Environment / command | Result | Evidence / limits |
|---|---|---|---|
| Baseline and review provenance | `git rev-parse origin/codex/m2-cryptographic-contract-core` = 4239730…; `git show f451f2e`: parent 9cf8825, adds only the r3 re-review, SHA-256 `4881c1e9…0d` | PASS | Reviewer scripts extracted to `/tmp/cham3_r4_tools`, outside the repository |
| Re-review reproductions | VM: the re-review's R14 static script and R15 `time_limits_probe.py`, at 84fa6ef and at 9b1aa2b | R14:<br>• 84fa6ef prints `GAP` (exit 0);<br>• 9b1aa2b fails its gap assertion (exit 1): route present.<br>R15: both commits reproduce the behaviors, which are now stated limits (exit 0). | [reviewer_r3_reproductions](../evidence/M2-cryptographic-contract-core/m2_001_amd64_vm_9b1aa2b/reviewer_r3_reproductions/) |
| Full M2 qualification | VM: `docker run … -e CHAM3_M2_REQUIRE_CLEAN=1 … cham3-m2-dev bash build/scripts/qualify_m2_rust.sh` | **PASS** (exit 0; every required check) | [m2_001_amd64_vm_9b1aa2b](../evidence/M2-cryptographic-contract-core/m2_001_amd64_vm_9b1aa2b/):<br>• clean identity; both amd64 locks (empty diffs);<br>• crate sources, inventory, rustfmt, clippy, naming, profiles;<br>• fresh builds, soft backends;<br>• valgrind probe and secure-memory (10 tests, 0 errors);<br>• unit: 36+36 tests; ECR 18/90/18 + 22 scenarios / 346 steps;<br>• harness: 119 tests. |
| Static gates | VM, same container settings: `bash build/scripts/run_static_checks.sh`; `sh tests/run_tests.sh static` | PASS / PASS (82 tests) | `static_checks.log`, `static_tests.log`; clone still clean |
| Unchanged inputs | `git diff --exit-code 84fa6ef 9b1aa2b -- core Cargo.toml Cargo.lock build .github PROJECT.md ADR-0001 tests/m2/run_m2_tests.sh tests/m2/lib`; per-section `vectors.json` comparison | PASS (exit 0; only `journal_scenarios` changed, as stated) | `unchanged_inputs.txt` |
| GitHub Actions on the candidate | `m2_checks` [38299143273](https://github.com/Kayyo321/cham3/actions/runs/38299143273), `m1_checks` [38299143245](https://github.com/Kayyo321/cham3/actions/runs/38299143245), `m0_checks` [38299143276](https://github.com/Kayyo321/cham3/actions/runs/38299143276) | **PASS** (all runs, jobs and steps success) | [m2_001_amd64_ci_9b1aa2b](../evidence/M2-cryptographic-contract-core/m2_001_amd64_ci_9b1aa2b/): CI summary identical to the VM's; kernel 6.17.0-1022-azure, UID 1001 |
| CI artifact ZIP bytes | — | NOT_RUN | `gh` is unauthenticated in this session; metadata only (artifact 11770401811, `sha256:b2b3501d…977f`) |
| Reviewer regression tools | VM, candidate read-only:<br>• `independent_vectors.py`;<br>• `schema_holes.py`;<br>• `r2_probes.py` (digest `8dcdde94…8ee8f`);<br>• `ecr_oracle.py check`. | • PASS;<br>• expected rejection (`subject_binding_mismatch`);<br>• expected rejection (`network_grant_conflict`);<br>• PASS.<br>Same results as revision 3. | `review_tools/` |
| R14 regression and sensitivity | harness `test_m2_review_regressions.py`. Covered:<br>• spec table vs model;<br>• reviewer gap closed;<br>• no direct naming path;<br>• defects detected: all roles allowed, effective before anchor, lost tail, non-idempotent retry, old keys kept;<br>• reconciliation table;<br>• in-flight request across a vaultd restart. | PASS (each defect detected) | harness suite. The spec tests failed before the IPC spec edit (sensitivity observed during development) |
| R15 regression | harness `test_m2_trusted_time.py`. Covered:<br>• both reviewer reproductions;<br>• multi-boot lag;<br>• never-corrected forward error;<br>• seeded lag-bound property (also reaches the counterexample);<br>• option-B model (4 tests). | PASS | harness suite. Dropping cross-boot floors fails the property (sensitivity checked with a scratch defect, not committed) |
| Advisory review | VM container: `m2_advisory_review.py` on a fresh tracker (SHA-256 `005cdba6…b64b`) and advisory-db `57dcc0f2…` | PASS: 0 behind, 0 affected, 0 open in `rustc`/`rust-*`; output byte-identical to revision 3; triage unchanged | [advisory_review_r4](../evidence/M2-cryptographic-contract-core/advisory_review_r4/advisory_readme.md). **Author evidence only**; the independent Debian check is still required |
| Miri / sanitizers / coverage-guided fuzzing | — | NOT_RUN (NOT_AVAILABLE on the pinned toolchain) | Never PASS |
| `name_recovery_keys` over real IPC; vault, registry, anchors, ceremony, trusted clock, enforcement (A04–A15) | — | NOT_RUN | Design plus oracle/interface/reference-model evidence only; gated M2-004..M2-007 |
| `tests/m2/run_m2_tests.sh integration\|recovery\|packaging` | — | NOT_RUN | Gated suites report NOT_RUN and fail by design |

## Review

- **Reviewer and exact commit:** pending. Codex, candidate 9b1aa2b9d1435fb33c303ca7fe01ce7afcb2964a (range `4239730..9b1aa2b`) and the following docs-only commit.
- **Findings and disposition:**
  - R14: design revised with oracle/interface model and vectors.
  - R15: design revised with reference model.
  - G1 Debian check: owner/environment action (below).
  - R01-r2, R07-r2, R09-r2, R12 and every earlier passed item kept intact ([matrix](../reviews/M2-001-002-finding-resolutions-r3.md)).

## Owner decisions still open (none taken)

1. Approve, amend or reject each of G1–G6. G2 has the reviewer's recommendation; the rest need re-review first.
2. **How G1's independent Debian advisory check is obtained.** Options:
   - (a) allow `security-tracker.debian.org` in the reviewer's environment;
   - (b) accept another independently retrieved, verifiable Debian source;
   - (c) decide G1 without it.
3. ADR 0006 §9.1: default anchor mode (recommended A).
4. ADR 0006 §9.2: who opens the recovery ceremony (recommended C1).
5. ADR 0007 §4: one-shot semantics (recommended A).
6. **ADR 0007 §8 (new): strength of the time bounds.**
   - A: accept the §5.7 limits.
   - B (recommended): observed-time start eligibility.
   - C: authenticated time source, as a later ADR.
   - Open question under every option: correcting a large forward error.
7. ADR 0008 §3: deletion granularity (recommended A).

## Remaining

- **Known issues/risks:**
  - All design mechanisms are unimplemented. The interface model checks roles, arguments and journal/anchor ordering, but not real sockets, peer credentials or crash points, and it does not model the anchor `identity_seq` comparison.
  - Trusted-time lag can accumulate without bound across reboots with a set-back clock, and accepted forward errors are permanent, unless §8 B or C is chosen.
  - The advisory triage is author judgment. The glibc `ld.so` and systemd items must be rechecked before M2-007.
  - The CI artifact bytes were not inspected.
- **Uncommitted paths or dirty state:** none after the evidence/handoff commit (`out/` is ignored).
- **Required next action and owner:**
  1. Codex re-reviews 9b1aa2b and the docs-only commit.
  2. The implementer repairs any findings.
  3. The owner resolves the G1 Debian-check path and decides G1–G6 and the five choices.
  4. Only then may M2-003 onward become READY.

## Re-review prompt for Codex

~~~text
You are the independent REVIEWER (Codex) for cham3 M2-001/M2-002 revision 4 in Kayyo321/cham3. You did not author
this work. Read AGENTS.md, PROJECT.md, approved ADR 0001, docs/tasks/M2-cryptographic-contract-core.md,
docs/engineering/{WORKFLOW,TESTING,SECURITY_INVARIANTS}.md, your re-review (review/m2-001-002-r3-security at
f451f2e346e6981b24802a7e9070ba09303c40d3), docs/reviews/M2-001-002-finding-resolutions-r3.md and
docs/handoffs/M2-001-002-rereview-r4.md. Treat author resolutions and evidence as claims to verify.

Targets: candidate 9b1aa2b9d1435fb33c303ca7fe01ce7afcb2964a (range 42397304663bff6057e26118014c18e7b90a2040..9b1aa2b)
and the following docs-only evidence/handoff commit on origin/codex/m2-cryptographic-contract-core. Confirm the
second commit changes only docs/. Do not merge main or review branches into the implementation branch.

1. Qualify natively on amd64 from a clean clone owned by an unprivileged UID, without --privileged, added
   capabilities or --record (same docker build/run commands as your r3 review), plus
   build/scripts/run_static_checks.sh and sh tests/run_tests.sh static.
2. R14: rerun your registry-table static check (it should now find name_recovery_keys for service:vaultd only).
   Attack docs/specs/m2_service_ipc_v1.md (name_recovery_keys, recovery_status, recover_keys, register_certificate,
   append_identity) and ADR 0006 section 10 step 3 and its crash bullet: can any non-vaultd role, any caller-chosen
   record/epoch, a wrong user/ceremony/class/epoch, an uncertified/invalidated/abandoned key, or a second naming get
   a recovery_keys record admitted? Is the record effective before its anchor update is durable? Does the pending
   tail rule, startup roll-forward, idempotent retry and vaultd reconciliation (never erase on unknown status or a
   named key; resume with held keys) close the lost-reply, anchor-failure and vaultd-restart cases, including a
   naming request still in flight? Check that no generic caller-controlled append exists. Run the scenarios
   recovery_naming_* and the R14 harness tests; check they would fail on the defects they name.
3. R15: rerun your time_limits_probe.py; check that ADR 0007 section 5.7 states both limits honestly (accumulated
   cross-boot lag bounded only by unobserved time since the last sync; accepted forward errors accelerating
   admitted future starts without bound and never corrected; honest timestamps refused) and that the seeded
   property in test_m2_trusted_time.py checks that bound. Review the section 8 options as owner choices (none
   selected); in particular whether option B's model actually bounds acceleration to the error present at
   admission. Do not select an option.
4. Confirm G2 is unchanged in substance: diff ADR 0004, docs/specs/m2_wire_format_v1.md and m2_ecr_v1.cddl against
   84fa6ef (only counts/description/approval-record edits) and confirm vectors.json differs only in
   journal_scenarios (network/restore scenarios identical; each recovery scenario differs only in its naming
   step). Re-run independent_vectors.py, schema_holes.py, r2_probes.py and ecr_oracle.py check
   (18 valid, 90 invalid, 18 admission, 22 journal scenarios / 346 steps, byte-for-byte regeneration).
5. Confirm the items you passed (R01-r2, R07-r2, R09-r2, R12, R02-R06, R08, R10, R11, I01-I03) are intact, and that
   the R13 ceremony state machine is unchanged apart from the routed naming step and the bypass wording.
6. Try your own Debian security tracker retrieval again. If still BLOCKED, record it as such; do not treat the
   author's advisory_review_r4 as clearance. Judge RustSec independently.
7. Compare GitHub Actions m2_checks 38299143273, m1_checks 38299143245 and m0_checks 38299143276 at 9b1aa2b.

Record every check as PASS/FAIL/NOT_RUN/BLOCKED with exact commands, environment and evidence; mocks and the
oracle/interface/time models are not runtime evidence. Return APPROVE or CHANGES_REQUESTED for M2-001 and each of
G1-G6. Do not make owner decisions, do not edit implementation files, publish only your review record on a new
review branch, and end your reply with this block filled in:

=== CHAM3 M2 R4 VERDICT ===
reviewer: Codex
review_record: <branch> @ <commit> : <path> (sha256 <digest>)
candidate_reviewed: 9b1aa2b9d1435fb33c303ca7fe01ce7afcb2964a
evidence_commit_reviewed: <sha> (docs-only: yes/no)
verdicts: M2-001: <..> G1: <..> G2: <..> G3: <..> G4: <..> G5: <..> G6: <..>
prior_findings: R14=<PASS/FAIL> R15=<PASS/FAIL> R13=<PASS/FAIL> earlier passed items intact=<yes/no>
qualification_amd64: <PASS/FAIL/BLOCKED> (<environment; counts>)
static_checks: <..>   static_tests: <..>
advisory_review: <PASS/FAIL/BLOCKED> (<tracker sha256 or reason> / <advisory-db commit> / RustSec: <..>)
new_findings:
- id: <..> | severity: <..> | gates: <..> | location: <file:line> | summary: <..> | required: <..>
notes_for_owner: <..>
=== END VERDICT ===
~~~
