# Handoff: M2-001 / M2-002 revision 4.1 — request for independent re-review (I04 delta and G1 Debian check)

- **State:** M2-001 and M2-002 are **IN_REVIEW**. The re-review at 3a3b5d1 recommends **APPROVE for owner consideration** for G2–G6. M2-001 and G1 remain CHANGES_REQUESTED only because the reviewer's own Debian tracker check was BLOCKED. Nothing is owner-approved; M2-003..M2-008 remain DRAFT.
- **Role:** IMPLEMENTER. The author approves nothing.
- **Model/provider:** Claude Code (Anthropic), working in its cloud VM. Requested reviewer: Codex (independent), now able to run on the owner's desktop.
- **Branch:** `codex/m2-cryptographic-contract-core` (pushed fast-forward).
- **Review answered:** `docs/reviews/M2-001-002-codex-security-rereview-r4.md` at 3a3b5d17ddc44b96c3fcc8265c5b337066aff0fb on `review/m2-001-002-r4-security`. Its parent is 9cf8825, the SHA-256 is `4d1d22b8…433d` (verified), and it was neither merged nor edited. Author response: [finding resolutions r4](../reviews/M2-001-002-finding-resolutions-r4.md).
- **Base:** 70c431c13a1c76fd2948e6c8f0ae33b720618043.
- **Revision-4.1 candidate:** **28a8595e627bcd092acaff4b8161522cf5d38f7f** (tree ad08a16302ea51d8b9f9322ca6599b50c987f2ea), which changes tests and the oracle only.
- **Docs-only follow-ups:**
  - ed2ba0d79e60b9a476af5b93685a1b4b214573f2: task progress and the owner's final-review instruction.
  - The commit adding this handoff, the evidence and STATUS.

## Completed work

| Item | Status | Where |
|---|---|---|
| I04: naming scenarios did not observe early key-state effects | oracle/vectors + test. The `keys` observations (`named`, `min_key_epoch`, `retired`) are asserted at 9 steps of the 3 `recovery_naming_*` scenarios. The reviewer's response-preserving early-application mutant is a harness defect test. | `tests/m2/oracle/ecr_oracle.py`, `tests/m2/vectors/ecr_v1/vectors.json` (`keys` fields only), `tests/m2/harness/test_m2_review_regressions.py`, inventory, vectors readme |
| Owner instruction for the final M2 review | recorded | [M2 task, "Final M2 review requirement"](../tasks/M2-cryptographic-contract-core.md): the final M2 review uses the **Codex Security plugin skill**, and the final handoff must remind the owner and include it in the prompt |

Nothing else changed: no ADR, specification, CDDL, IPC text, script, Rust source, build input or workflow (`git diff --exit-code 9b1aa2b 28a8595 -- core Cargo.toml Cargo.lock build .github PROJECT.md docs/decisions docs/specs tests/m2/run_m2_tests.sh tests/m2/lib` exits 0). The G2–G6 texts are therefore exactly those the re-review recommended.

## Verification (at 28a8595; same VM and container conditions as revision 4)

| Check | Result | Evidence / limits |
|---|---|---|
| Full M2 qualification, clean UID-1000 clone | **PASS**: unit 36+36; ECR 18/90/18 with 22 scenarios / 346 steps; harness 120 | [m2_001_amd64_vm_28a8595](../evidence/M2-cryptographic-contract-core/m2_001_amd64_vm_28a8595/) |
| Static checks; static tests | PASS; PASS (82) | same directory |
| Reviewer's `early_effect_probe.py`, unchanged | 9b1aa2b: zero failures (I04 reproduced). 28a8595: key-state failures; the probe's assertion fails (exit 1) | `reviewer_r4_reproductions/` |
| New I04 defect test | PASS. It requires a key-state failure in both anchor-failure scenarios and no response or record-count difference | harness suite |
| GitHub Actions | **PASS**: m2 [38312981989](https://github.com/Kayyo321/cham3/actions/runs/38312981989), m1 [38312982028](https://github.com/Kayyo321/cham3/actions/runs/38312982028), m0 [38312982051](https://github.com/Kayyo321/cham3/actions/runs/38312982051) | [m2_001_amd64_ci_28a8595](../evidence/M2-cryptographic-contract-core/m2_001_amd64_ci_28a8595/) |
| CI artifact ZIP bytes | NOT_RUN | Author `gh` is not authenticated; the owner's desktop can download them |
| Independent Debian advisory check | BLOCKED until the reviewer runs it | Needs the reviewer's own retrieval; the author's advisory_review_r4 is not clearance |
| Miri / sanitizers / fuzzing; real IPC, vault, registry, anchors, clock, enforcement | NOT_RUN | Unchanged limits |

## Owner decisions still open (none taken)

1. G1–G6. G2–G6 carry the reviewer's APPROVE recommendation; G1 waits for the Debian check.
2. ADR 0006 §9.1: default anchor mode.
3. ADR 0006 §9.2: who opens the recovery ceremony.
4. ADR 0007 §4: one-shot semantics.
5. ADR 0007 §8: strength of the time bounds, including recovery from a large forward error.
6. ADR 0008 §3: deletion granularity.

## Remaining

- **Known risks:** unchanged from [revision 4](M2-001-002-rereview-r4.md#remaining).
- **Uncommitted state:** none after the evidence commit.
- **Next:**
  1. Codex, on the owner's desktop, performs the Debian check and reviews the I04 delta.
  2. The implementer repairs any findings.
  3. The owner decides G1–G6 and the five choices.
  4. Only then may M2-003 become READY.
- **Reminder for the end of M2:** run the final independent review with the Codex Security plugin skill (task file).

## Re-review prompt for Codex (owner's desktop)

~~~text
You are the independent REVIEWER (Codex) for cham3 M2-001/M2-002 revision 4.1 in Kayyo321/cham3, running on the
owner's desktop. You did not author this work. Read AGENTS.md, your re-review (review/m2-001-002-r4-security at
3a3b5d17ddc44b96c3fcc8265c5b337066aff0fb), docs/reviews/M2-001-002-finding-resolutions-r4.md and
docs/handoffs/M2-001-002-rereview-r5.md. Treat author claims and evidence as claims to verify.

Targets: candidate 28a8595e627bcd092acaff4b8161522cf5d38f7f (range 70c431c13a1c76fd2948e6c8f0ae33b720618043..28a8595)
and the docs-only commits after it on origin/codex/m2-cryptographic-contract-core; confirm those change only docs/.
Do not merge main or review branches into the implementation branch; do not edit implementation files.

1. G1 / M2-001 Debian advisory check (the reason those gates are still CHANGES_REQUESTED): from this machine,
   retrieve https://security-tracker.debian.org/tracker/data/json yourself (record time, size, SHA-256) and the
   RustSec advisory-db (record commit). In a clean clone at 28a8595 owned by an unprivileged UID, build the pinned
   image and run the qualification (docker run --user "$(id -u):$(id -g)" ... bash build/scripts/qualify_m2_rust.sh,
   no --privileged, no added capabilities, no --record) to get your own dependency_inventory.json, then run
     python3 -I build/scripts/m2_advisory_review.py --tracker <tracker.json> --advisory-db <advisory-db> \
       --inventory <qualification dir>/dependency_inventory.json --out-dir <out>
   inside the same container settings. Judge debian_behind.tsv, debian_open.tsv and rustsec.tsv yourself, and
   independently judge the author's 23-item triage (docs/evidence/M2-cryptographic-contract-core/
   advisory_review_r4/triage.tsv) against the tracker data you retrieved. If the retrieval still fails, record
   BLOCKED with the exact error and do not treat author evidence as clearance.
2. I04: confirm the change is test/oracle-only (git diff --exit-code 9b1aa2b 28a8595 -- core Cargo.toml Cargo.lock
   build .github PROJECT.md docs/decisions docs/specs tests/m2/run_m2_tests.sh tests/m2/lib), that vectors.json
   differs from 9b1aa2b only by the "keys" observations in the recovery_naming_* scenarios, that your
   early_effect_probe.py now fails at 28a8595, and that the new harness test would fail for the defect it names.
   Try one more early-effect variant of your own.
3. Confirm the qualification above PASSes (unit 36+36, ECR 18/90/18 with 22 scenarios / 346 steps, harness 120),
   plus build/scripts/run_static_checks.sh and sh tests/run_tests.sh static, and compare GitHub Actions m2_checks
   38312981989, m1_checks 38312982028 and m0_checks 38312982051 at 28a8595 (you may download the m2 artifact with
   gh run download 38312981989 -n m2_qualification_amd64 and compare its summary and digest).
4. Confirm that your G2-G6 recommendations still apply to unchanged texts and that every earlier passed item is
   intact.

Record every check as PASS/FAIL/NOT_RUN/BLOCKED with exact commands, environment and evidence. Return APPROVE or
CHANGES_REQUESTED for M2-001 and each of G1-G6; make no owner decisions. Publish only your review record on a new
review branch (review/m2-001-002-r5-security), and end your reply with this block filled in:

=== CHAM3 M2 R5 VERDICT ===
reviewer: Codex
environment: <machine/OS/arch; UID; Docker version; network notes>
review_record: <branch> @ <commit> : <path> (sha256 <digest>)
candidate_reviewed: 28a8595e627bcd092acaff4b8161522cf5d38f7f
docs_commits_reviewed: <shas> (docs-only: yes/no)
verdicts: M2-001: <..> G1: <..> G2: <..> G3: <..> G4: <..> G5: <..> G6: <..>
prior_findings: I04=<PASS/FAIL> R13/R14/R15 and earlier passed items intact=<yes/no>
qualification_amd64: <PASS/FAIL/BLOCKED> (<counts>)
static_checks: <..>   static_tests: <..>
advisory_review: <PASS/FAIL/BLOCKED> (tracker <time, bytes, sha256> / advisory-db <commit> / behind: <n> / open not-unimportant: <n> / RustSec affected: <n>)
triage_judgment: <agree/disagree per item group; items needing action before M2-007>
ci_comparison: <..>
new_findings:
- id: <..> | severity: <..> | gates: <..> | location: <file:line> | summary: <..> | required: <..>
notes_for_owner: <..>
=== END VERDICT ===
~~~
