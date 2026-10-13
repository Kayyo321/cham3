# Handoff: M2-005 revision 4 — request for bounded independent re-review

- **State:** M2-005 is **IN_REVIEW** (revision 4). M2-004 is DONE. The author approves nothing.
- **Role:** IMPLEMENTER. **Model/provider:** Claude Code (Anthropic), cloud VM. Requested reviewer: Codex (independent, on the owner's home computer).
- **Review answered:** `docs/reviews/M2-005-codex-security-rereview-r3.md` at 4865d7b7a4c885fddf71b3e0f0b14a361c3c609e on `review/m2-005-r3-security` (parent 9cf8825, adds only that file, SHA-256 `6ce7449c8784cbf67ccd5b1ee89c5da9f97c088e9001f04201cb6c4165288c50`; verified, not merged or edited). Author response: [M2-005 finding resolutions r4](../reviews/M2-005-finding-resolutions-r4.md).
- **Owner decisions used:** none were needed. The earlier open question (the ±300 s error code) was answered by the owner on 2026-10-13 (`timestamp_rule`, ADR 0007 erratum E4, in the docs-only commit ad18d8b).
- **Base:** ad18d8be68d2e5a7fd50d4577ed90edd37ce5235 (docs only: erratum E4 and the M2-006 scope record, after the revision-3 docs commit 30f77b2 on candidate be24152).
- **Revision-4 candidate:** **5860f276ab593da34771f704691ea35649324d6e** (one commit on the base: range ad18d8b..5860f27, four files).
- **Docs-only follow-up:** the commit adding this handoff, the resolutions, the evidence, the task progress and STATUS.

## Completed work

| Finding | Change | Where |
|---|---|---|
| R3-01 | The wrapper owns nothing on the disk: no pre-deletion; an existing root or filler path (file, directory, symbolic link, dangling or not) refuses the whole check untouched before anything is built; leftover checks cover symbolic links; repository root and cargo target directory overridable for the harness | `tests/m2/storage/check_reserve_full_disk.sh` |

**Complete change list of ad18d8b..5860f27 (`git diff --stat`, 4 files):** `tests/m2/storage/check_reserve_full_disk.sh`, `tests/m2/harness/test_m2_reserve_probe.py`, `tests/m2/test_inventory.txt`, `.github/workflows/m2_checks.yml`. No Rust source, dependency, `Cargo.lock`, manifest, specification, ADR, oracle or vector changed.

New controls:

- **Harness** (`test_m2_reserve_probe.py`, 12 tests, 5 new): `full_disk_wrapper_t` — `test_existing_root_and_filler_are_refused_and_preserved`, `test_symbolic_links_and_their_targets_are_refused_and_preserved`, `test_a_disk_without_the_paths_is_left_as_found`; `seeded_wrapper_defects_t` — `test_restored_pre_deletion_is_detected` (the baseline `rm -rf` restored in a copy of the script is caught by the preservation case), `test_unmodified_copy_preserves_existing_paths`. The cases run the real script against the dispatcher's probe build; with `CHAM3_M2_WRAPPER_DISK` set they run against fresh subdirectories of that disk (the CI tmpfs). 147 harness tests.
- **CI:** the m2 job runs the five wrapper cases on the 2 MiB tmpfs right after the full-filesystem positive path (step "Wrapper preservation on the tmpfs (unprivileged)").

## Verification (at 5860f276ab593da34771f704691ea35649324d6e)

| Check | Environment / command | Result | Evidence / limits |
|---|---|---|---|
| Reviewer's wrapper scenario (`wrapper_run.py`, as a shell script of the same steps) | pinned container, UID 1000, clean clones at be24152 and 5860f27, the candidate's probe build, synthetic disk on the overlay | **be24152: both paths deleted** (wrapper exit 1 after `refusing_to_fill`); **5860f27: `refusing: … exists`, wrapper exit 1, both paths preserved** | [m2_005_amd64_vm_5860f27](../evidence/M2-cryptographic-contract-core/m2_005_amd64_vm_5860f27/) `reviewer_reproductions/` |
| Wrapper cases on an ordinary filesystem | the harness module, pinned container | PASS (12/12, including the two seeded probe defects and the seeded wrapper defect) | in the qualification's harness run |
| Wrapper cases on the tmpfs | `--tmpfs /full-disk:rw,size=2m,…`, the positive path first, then the five wrapper cases with `CHAM3_M2_WRAPPER_DISK=/full-disk` | **PASS** in the author's VM (positive path PASS, 5/5, no leftovers) and in CI (`Ran 5 tests in 0.394s … OK`, step success) | `storage/` |
| Full M2 qualification | Clean UID-1000 clone `/tmp/cham3_m2005r4`, pinned container, `qualify_m2_rust.sh`, `CHAM3_M2_REQUIRE_CLEAN=1` | **PASS** (`m2 qualification: PASS`; unit 169+169 inventory-exact, harness 147 in 859.8 s; provider 7/7, ECR 19/96/18 with 23 scenarios / 352 steps, format differential 5 955 / 0, vault oracle 38; layer-7 differential 11 scenarios / 59 steps / 105 comparisons / 0 disagreements; `dirty_tracked 0`, Cargo.lock unchanged) | same directory |
| Static checks; static tests | same clone and container | **PASS** (`static checks: PASS`; `Ran 82 tests … OK`; the clone clean afterwards) | `qualification_console.txt` |
| Unchanged inputs | `git diff --stat ad18d8b 5860f27` | the 4 files above; no Rust, `Cargo.lock`, specification, ADR, oracle or vector change | `unchanged_inputs.txt` |
| GitHub Actions at 5860f27 | m2 38444270032, m1 38444270097, m0 38444270083 | **success**, every step of every job (m2 qualification `m2 qualification: PASS`, protection positive path PASS, full-filesystem step PASS, wrapper step 5/5) | [CI evidence](../evidence/M2-cryptographic-contract-core/m2_005_amd64_ci_5860f27/) |
| Vault protection, positive path | CI step | **PASS** in CI (`protection_verified`, `vault protection positive path: PASS`); BLOCKED in this VM (memlock cannot be raised) | CI evidence |
| Miri, sanitizers, coverage-guided fuzzing | — | NOT_RUN (NOT_AVAILABLE on the pinned toolchain) | — |

## Remaining

- **Known risks and limits:** unchanged from revision 3 (in-process composition; no anchor or ledger until M2-006 stage A; lazy reserve replenishment; compressing filesystems; system clock; Miri, sanitizers and fuzzing NOT_RUN).
- **Uncommitted state:** none after the evidence commit. M2-006 stage A is in progress locally on this branch and is not in the candidate; its first commit follows this re-review.
- **Next:**
  1. Codex re-reviews M2-005 revision 4 (bounded, prompt below). An APPROVE closes M2-005 (completion handoff).
  2. M2-006 stage A continues (scope record in the task file).
- **Reviewer environment (owner note):** Codex runs on the owner's home computer, unrestricted; `~/cham3` is refreshed by step 0 of the prompt. The container-only `--ulimit nofile=1024:1024` for Valgrind is acceptable if recorded.

## Review prompt for Codex

~~~text
You are the independent REVIEWER (Codex) for cham3 M2-005 revision 4 in Kayyo321/cham3: a bounded re-review of one
finding. You did not author this work. You run on the owner's home computer with unrestricted access; record BLOCKED
only with the exact command and error.

0. Bring the local clone to the candidate FIRST:
     cd ~/cham3 && git status --porcelain && git stash list
     git fetch origin codex/m2-cryptographic-contract-core review/m2-005-r3-security main
     git checkout codex/m2-cryptographic-contract-core && git reset --hard origin/codex/m2-cryptographic-contract-core
     git rev-parse HEAD        # the docs-only commit after 5860f276ab593da34771f704691ea35649324d6e (its parent must be 5860f27)
     git log --oneline -4      # 30f77b2 -> ad18d8b (docs: erratum E4, M2-006 scope) -> 5860f27 (candidate) -> <docs-only commit>
   Move unexpected local changes aside (git stash); never force-push or edit the implementation branch. Read every
   document from this checkout; make the clean clone for testing from it
   (git clone --no-hardlinks ~/cham3 <dir>; git -C <dir> checkout --detach 5860f276ab593da34771f704691ea35649324d6e).

Read AGENTS.md; your record review/m2-005-r3-security at 4865d7b7a4c885fddf71b3e0f0b14a361c3c609e;
docs/reviews/M2-005-finding-resolutions-r4.md; docs/handoffs/M2-005-rereview-r4.md. Treat author claims as claims to
verify.

Targets: candidate 5860f276ab593da34771f704691ea35649324d6e (range ad18d8b..5860f27, one commit) and the docs-only
commit after it; confirm the candidate changes exactly tests/m2/storage/check_reserve_full_disk.sh,
tests/m2/harness/test_m2_reserve_probe.py, tests/m2/test_inventory.txt and .github/workflows/m2_checks.yml and
nothing else (no Rust source, dependency, Cargo.lock, specification, ADR, oracle or vector), and that ad18d8b and the
docs-only commit change only docs/. Do not merge main or review branches; do not edit implementation files.

1. R3-01 (check_reserve_full_disk.sh): no deletion before the probe; an existing root or filler path (file,
   directory, symbolic link, dangling or not) refuses the whole check untouched before anything is built; the
   leftover checks cover symbolic links; CHAM3_REPO_ROOT and CHAM3_M2_RESERVE_TARGET_DIR only redirect the build.
   Re-run your wrapper scenario (wrapper_run.py) at the candidate and extend it: existing filler only, existing root
   only, symbolic links with existing and missing targets, a root path that is a regular file, on the overlay and on
   a bounded 2 MiB tmpfs (where the untouched case must still run the positive path and leave nothing behind). Judge
   the refusal (rather than a private per-invocation namespace, which cannot cover the filler at the mount root).
2. Harness: run the twelve tests of tests/m2/harness/test_m2_reserve_probe.py through the dispatcher's qualification
   and, separately, the five wrapper cases against the tmpfs: in the pinned container with --tmpfs
   /full-disk:rw,size=2m,uid=$(id -u),gid=$(id -g),mode=0700, after sh tests/m2/storage/check_reserve_full_disk.sh
   out/m2/storage, run CHAM3_M2_RESERVE_PROBE=/work/out/m2/storage/target/release/cham3_reserve_probe
   CHAM3_M2_WRAPPER_DISK=/full-disk python3 -I -m unittest discover -s tests/m2/harness -p test_m2_reserve_probe.py
   -k wrapper -v. Judge whether the seeded wrapper defect discriminates.
3. Confirm every PASS item of your revision-3 record is intact: the candidate touches no Rust source, so your
   journal, reserve and probe tests need no re-run; say so explicitly, or re-run any you choose.
4. Evidence: qualification in a clean clone at 5860f27 (CHAM3_M2_REQUIRE_CLEAN=1; the nofile limit as before):
   unit 169+169, provider 7/7, ECR 19/96/18 with 23 scenarios / 352 steps, format differential 5955 / 0 (corpus
   unchanged), vault oracle 38, harness 147 (12 in test_m2_reserve_probe, 41 and 21 seeded defects); static checks
   and sh tests/run_tests.sh static. Compare GitHub Actions m2_checks 38444270032, m1_checks 38444270097 and
   m0_checks 38444270083, including the new step "Wrapper preservation on the tmpfs (unprivileged)"; download the
   artifact and check its digest (id 11839175077, 44 files, 103 622 bytes, sha256:3b9cb3b99a7dcdf04f4998c4303be3195d732a1646a0dc065ba1d340a9720ba1).
5. Codex Security plugin on ad18d8b..5860f27 (shell and Python only); record name/version and findings.

Record every check as PASS/FAIL/NOT_RUN/BLOCKED with exact commands and evidence. Return APPROVE or
CHANGES_REQUESTED for M2-005; make no owner decisions. Publish only your record on a new review branch
(review/m2-005-r4-security) and end with:

=== CHAM3 M2-005 R4 VERDICT ===
reviewer: Codex
environment: <OS/arch; UID; Docker version; nofile limit used>
local_clone: <~/cham3 HEAD after step 0; local changes or stashes: none / moved aside (what)>
review_record: <branch> @ <commit> : <path> (sha256 <digest>)
candidate_reviewed: 5860f276ab593da34771f704691ea35649324d6e
docs_commits_reviewed: <shas> (docs-only: yes/no)
scope_exact: <the four files and nothing else: yes/no>
verdict: M2-005: <APPROVE/CHANGES_REQUESTED>
r3_01: <PASS/FAIL> (<your wrapper scenario and extensions, overlay and tmpfs>)
wrapper_cases_on_tmpfs: <PASS/FAIL (5 cases; leftovers)>
seeded_wrapper_defect: <discriminates yes/no>
earlier_items_intact: <yes/no; re-run or not>
qualification_amd64: <PASS/FAIL/BLOCKED> (<counts>)
static_checks: <..>   static_tests: <..>
ci_comparison: <..>
security_plugin: <..>
new_findings:
- id: <..> | severity: <..> | location: <file:line> | summary: <..> | required: <..>
notes_for_owner: <..>
=== END VERDICT ===
~~~
