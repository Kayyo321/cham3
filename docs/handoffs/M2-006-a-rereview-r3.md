# Handoff: M2-006 stage A revision 3 — request for independent re-review

- **State:** M2-006 is **IN_PROGRESS**; stage A is **IMPLEMENTED → IN_REVIEW** (revision 3). Stages B–D are not started (stage-B reading began in a separate local clone and is not on the branch). M2-003, M2-004 and M2-005 are DONE. The author approves nothing.
- **Role:** IMPLEMENTER. **Model/provider:** Claude Code (Anthropic), the owner's home computer. Requested reviewer: Codex (independent, on the owner's home computer).
- **Review answered:** `docs/reviews/M2-006-stage-a-r2-codex-security.md` at 7d570c1ce5b14ce3b37836775881c2013aa051a4 on `review/m2-006-a-r2-security` (one commit on `main` 9cf8825, the branch adds only that file, SHA-256 `d3ecee78270a58c82bdc0c1d137f1cd2a6eb8417066cfb37cc85b12f9ccea7e3`; verified, not merged or edited). Author response: [M2-006 stage A finding resolutions r3](../reviews/M2-006-a-finding-resolutions-r3.md).
- **Owner decisions used (2026-10-14):** for R2-03 the owner chose **Option A**, an erratum of ADR 0006 §3 recording the limit (the author drafted it as **E5**; the owner is asked to confirm its wording); and the owner approved **refreshing the amd64 Debian locks** after a Debian libcurl security update failed CI. Both were explicit answers to questions with a recommended option.
- **Base:** fc147e0f80188a82a761ffe203a6ac7704ea25f9 (the revision-2 docs-only commit on e34fd8a).
- **Revision-3 candidate:** **2217e62ddf42c42573346155727c983d937addc0**, three commits on the base (range fc147e0..2217e62, 13 files):
  - 9633ce30c212cd2cd3515b9a13d7227d1f9f9796 — the code, tests, harness, inventory and specifications (8 files);
  - a2d0ebd — the amd64 lock refresh (two lock files and the ADR 0003 §2 record);
  - 2217e62 — ADR 0006 §3 erratum E5, its errata line, the decisions index and storage formats §7.
- **Docs-only follow-up:** the commit adding this handoff, the resolutions, the evidence, the task progress and STATUS.

## Owner decisions (2026-10-14)

- **R2-03, Option A.** ADR 0006 §3 erratum E5: the guarantee holds whenever one of the restriction's four durable marks — its ledger entry, the removal or emptying of the failed ledger's validity file, the `unverifiable` run marker, its journal record — is made before the process dies. Excluded is only the simultaneous failure of all four, followed by a process death before any retry succeeds and a same-boot restart. Stated consequences: only those restrictions are lost; the window lasts from the failure to the first successful retry, during which registryd denies in memory, retries on every operation, serves nothing eligible, refuses an orderly shutdown and reports `rollback_status.unverifiable_owed`; a reboot contains, and an operator who sees `unverifiable_owed` reboots before restarting the service. The alternatives (an external supervisor witness, containment on every unclean same-boot restart) were not chosen. No mechanism, record or format change.
- **amd64 lock refresh.** GitHub Actions m2_checks [38577923955](https://github.com/Kayyo321/cham3/actions/runs/38577923955) at 9633ce3 failed only the `installed_packages_lock` and `downloaded_debs_lock` checks: Debian published libcurl `8.14.1-2+deb13u7`, so a fresh image installs `libcurl3t64-gnutls` and `libcurl4t64` at deb13u7 instead of the locked deb13u5 (both unpinned dependencies of the pinned set; every test of that run passed, the protection, full-filesystem and wrapper steps were skipped). The author's no-cache build here (image `2a0bcf342c57`, now tagged `cham3-m2-dev`) gave an installed set and `.deb` digests byte-identical to CI's image; the refresh changes exactly those four lines (`debian_lock_amd64.txt` `536db59f…4530`, `debian_debs_amd64.sha256` `30e909a6…7573`). `debian_packages.txt`, `Cargo.lock`, the crate sources and the Dockerfile are unchanged; the arm64 development locks are not refreshed. The m1 and m0 runs at 9633ce3 passed.

## Completed work

| Finding / decision | Change | Where |
|---|---|---|
| R2-01 | The fresh anchor's counter continues above the highest counter the rejected slots (equal counters, or two invalid) carried, so this run's writes outrank them; the quarantine they leave is anchored at the same start and holds at every later start | `anchor.rs`, `registry.rs` |
| R2-02 | A run whose ledger failed during the start publishes the marker `unverifiable`, never `running`; if that write fails too the start is refused (a marker of another run than the ledger); the next start contains either way | `registry.rs` |
| R2-03 | Narrowed: the failed ledger's invalidation and the `unverifiable` marker are owed until they succeed and retried by every operation; `rollback_status.unverifiable_owed`; the remaining window accepted by the owner as ADR 0006 §3 erratum E5 | `ledger.rs`, `registry.rs`, ADR 0006 §3, storage formats §7 |
| R2-04 | The pending queue is drained one record at a time, each anchored before the next is appended: at most one unanchored record ever; a partial drain leaves the appended records anchored and served | `registry.rs` |
| Lock drift | amd64 locks refreshed for libcurl deb13u7 (owner-approved) | `build/m2/debian_lock_amd64.txt`, `build/m2/debian_debs_amd64.sha256`, ADR 0003 §2 |

**Complete change list of fc147e0..2217e62 (`git diff --stat`, 13 files):** `build/m2/debian_debs_amd64.sha256`, `build/m2/debian_lock_amd64.txt`, `core/registryd/src/anchor.rs`, `core/registryd/src/ledger.rs`, `core/registryd/src/registry.rs`, `core/registryd/tests/storage_integrity.rs`, `docs/decisions/0003_m2_rust_toolchain_dependencies.md`, `docs/decisions/0006_m2_storage_anchor_recovery.md`, `docs/decisions/README.md`, `docs/specs/m2_registry_storage_v1.md`, `docs/specs/m2_service_ipc_v1.md`, `tests/m2/harness/test_m2_registry_policy.py`, `tests/m2/test_inventory.txt`. No dependency, `Cargo.lock`, manifest, pinned version, oracle, vector, workflow or Dockerfile changed.

New controls: `storage_integrity` 27 tests (+4; the fault injector gains an all-writes fault, a first-ledger-append trigger and a journal write budget); 87 seeded registry/policy defects (+6, two anchors re-pointed); inventory 206 cargo tests (+4).

## Verification (at 2217e62ddf42c42573346155727c983d937addc0)

| Check | Environment / command | Result | Evidence / limits |
|---|---|---|---|
| Reviewer's tests, unchanged, at e34fd8a and at the candidate | pinned container, UID 1001, clean clones; `cargo test --frozen -p cham3_registryd --test review_r2_storage review_r2_`, `--test queue_validation queue_`, `--test reviewer_matrix review_matrix` (debug and release) | **e34fd8a: storage 3/6 (the three findings fail), queue 1/4, matrix 4/4; 9633ce3 and 2217e62: storage 5/6 (only the case erratum E5 excludes), queue 3/4 (the partial-flush test's premise assertion, see the resolutions), matrix 4/4** | `reviewer_reproductions/` |
| Stage-A revision-3 tests | `storage_integrity` (27): the four new tests cover R2-01–R2-04 with their restart exits; `registry_operations` (25), `current_authorization` (14), `anchor::tests` (3) unchanged | PASS (debug and release) | in the qualification |
| Seeded defects | `test_m2_registry_policy.py`: 87 defects (6 new), each caught by the named test, the unmodified copy passes | **PASS** (`test_m2_registry_policy` 2 tests over 87 defects, in the 147-test harness run, 384.6 s) | in the qualification's harness run |
| Full M2 qualification | Clean clone owned by this host's unprivileged UID 1001, pinned container `cham3-m2-dev` rebuilt without cache from the unchanged Dockerfile (image id 2a0bcf342c57, with the refreshed locks), `--ulimit nofile=1024:1024`, `qualify_m2_rust.sh`, `CHAM3_M2_REQUIRE_CLEAN=1` | **PASS** (`m2 qualification: PASS`; both lock checks PASS against the refreshed locks; unit 206 + 206; provider PASS; ECR 19/96/18 with 23 scenarios / 352 steps; format differential 5955 / 0, corpus unchanged; vault oracle 38; harness 147; admission differential 11 scenarios / 59 steps / 105 comparisons / 0 disagreements; Valgrind memcheck 0 errors; Miri, sanitizers, fuzzing NOT_AVAILABLE) | [VM evidence](../evidence/M2-cryptographic-contract-core/m2_006a_r3_amd64_vm_2217e62/) |
| Static checks; static tests | same clone and container | **PASS** (`static checks: PASS`; `Ran 82 tests … OK`; the clone clean afterwards) | `qualification_console.txt` |
| Full-filesystem probe with the ledger, then the five wrapper cases | `--tmpfs /full-disk:rw,size=2m,…` | **PASS** (`restrictive reserve under a full filesystem: PASS`, both reports byte-identical to revision 2's and to CI's; wrapper cases 5/5 OK) | `storage/` |
| Unchanged inputs | `git diff --stat fc147e0 2217e62` | the 13 files; no dependency, `Cargo.lock`, manifest, pinned version, oracle, vector, workflow, probe, wrapper or Dockerfile change; `journal.rs`, `record.rs`, `lineage.rs`, policyd, identityd and the builders untouched | `unchanged_inputs.txt` |
| GitHub Actions at 9633ce3 | m2_checks 38577923955, m1_checks 38577923430, m0_checks 38577923571 | m1, m0 success; **m2 FAIL**: only the two lock checks (libcurl deb13u7), every test passed | [CI evidence](../evidence/M2-cryptographic-contract-core/m2_006a_r3_amd64_ci_2217e62/) |
| GitHub Actions at 2217e62 | m2_checks 38584362113, m1_checks 38584362062, m0_checks 38584362077 | **success**, every job and step (both lock checks PASS; harness 147 in 768.1 s; protection, full-filesystem and wrapper steps PASS); artifact 11906127702, `sha256:1ed83981cfbe186c42969c96760b96f8bfc189dc2d348d5c78a777078de3937a`, download digest identical | [CI evidence](../evidence/M2-cryptographic-contract-core/m2_006a_r3_amd64_ci_2217e62/) |
| Vault protection, positive path | CI step | **PASS** in CI (`protection_verified`, `vault protection positive path: PASS`); NOT_RUN on this host (memlock limit) | CI evidence |
| Real power loss, kernel crash, process death at each boundary (A08/A10) | — | NOT_RUN: faults are injected at the io boundaries and restarts/reboots simulated | storage spec §7 |
| Miri, sanitizers, coverage-guided fuzzing | — | NOT_RUN (NOT_AVAILABLE on the pinned toolchain) | — |

## Review

- Reviewer and exact commit: Codex, `review/m2-006-a-r2-security` @ 7d570c1 (CHANGES_REQUESTED, R2-01–R2-04); the earlier `review/m2-006-a-security` @ 344c372 (R01–R08, resolved in revision 2 and confirmed PASS by the re-review).
- Findings and disposition: R2-01, R2-02, R2-04 resolved as above; R2-03 narrowed, the remaining window accepted by the owner (erratum E5). Details: [resolutions r3](../reviews/M2-006-a-finding-resolutions-r3.md).

## Remaining

- **Known risks and limits:** as in the stage-A handoff, with the R2-03 window now an owner-accepted limit (ADR 0006 §3 erratum E5; storage formats §7). The arm64 development locks still record libcurl deb13u5.
- **Uncommitted state:** none after the docs commit. The author's stage-B reading clone (`/home/dev/cham3-m2006b/work`, a local branch on fc147e0 with no changes) is not on the branch.
- **Next:** the owner confirms the wording of erratum E5; Codex re-reviews stage A revision 3 (bounded, prompt below); an APPROVE closes stage A (completion handoff) and opens stage B.
- **Reviewer environment (owner note):** Codex runs on the owner's home computer; its tool runtime had no Docker access in the last review, so the local pinned qualification was BLOCKED there. The author's Docker access on this machine works through the `docker` group (`sg docker`); the same may work for the reviewer. The local `cham3-m2-dev` tag now names the refreshed image `2a0bcf342c57`; the reviewer's old `cham3-m2-independent-r3` (`4f4f684d76f7`) carries the old locks and fails the two lock checks at the candidate by design. The container-only `--ulimit nofile=1024:1024` for Valgrind is acceptable if recorded.

## Review prompt for Codex

~~~text
You are the independent REVIEWER (Codex) for cham3 M2-006 stage A revision 3 in Kayyo321/cham3: a bounded
re-review of your four findings (M2-006-A-R2-01–R2-04), of the owner-approved amd64 lock refresh, and of the
implementation against ADR 0006 §3 erratum E5. You did not author this work. You run on the owner's home
computer; record BLOCKED only with the exact command and error (the `docker` group may give your runtime
access: try `sg docker -c 'docker ps'` before concluding BLOCKED).

0. Bring the local clone to the candidate FIRST:
     cd ~/cham3 && git status --porcelain && git stash list
     git fetch origin codex/m2-cryptographic-contract-core review/m2-006-a-r2-security main
     git checkout codex/m2-cryptographic-contract-core && git reset --hard origin/codex/m2-cryptographic-contract-core
     git rev-parse HEAD        # expect the docs-only commit after 2217e62 (its parent must be 2217e62)
     git log --oneline -6      # fc147e0 (docs) -> 9633ce3 (code) -> a2d0ebd (locks) -> 2217e62 (erratum) -> the docs-only commit
   Move unexpected local changes aside (git stash); never force-push or edit the implementation branch. Read every
   document from this checkout; make the clean clone for testing from it
   (git clone --no-hardlinks ~/cham3 <dir>; git -C <dir> checkout --detach 2217e62ddf42c42573346155727c983d937addc0).
   Build the image from the unchanged Dockerfile without cache (docker build --no-cache --network host
   -f build/m2/dev_container.dockerfile -t <tag> build/m2): an image with the old locks fails the lock checks.

Read AGENTS.md; your record review/m2-006-a-r2-security at 7d570c1ce5b14ce3b37836775881c2013aa051a4;
docs/reviews/M2-006-a-finding-resolutions-r3.md; docs/handoffs/M2-006-a-rereview-r3.md (the owner decisions);
ADR 0006 §3 with erratum E5 and ADR 0003 §2 (the lock record); the changed parts of
docs/specs/m2_registry_storage_v1.md (§§3, 4, 6, 7) and the rollback_status row of docs/specs/m2_service_ipc_v1.md.
Treat author claims as claims to verify; the owner's choices are recorded in the handoff.

Targets: candidate 2217e62ddf42c42573346155727c983d937addc0 (range fc147e0..2217e62, three commits, 13 files:
build/m2/debian_lock_amd64.txt, build/m2/debian_debs_amd64.sha256, core/registryd/src/{anchor,ledger,registry}.rs,
core/registryd/tests/storage_integrity.rs, docs/decisions/{0003_m2_rust_toolchain_dependencies,
0006_m2_storage_anchor_recovery,README}.md, docs/specs/m2_registry_storage_v1.md, docs/specs/m2_service_ipc_v1.md,
tests/m2/harness/test_m2_registry_policy.py, tests/m2/test_inventory.txt) and the docs-only commit after it
(docs/ only). Confirm nothing else changed: no dependency, Cargo.lock, manifest, pinned version
(debian_packages.txt), oracle, vector, workflow or Dockerfile. Do not merge main or review branches; do not edit
implementation files.

1. Your tests, unchanged: re-run review_r2_storage (storage_r2_validation.rs), queue_validation
   (storage_integrity.rs at e34fd8a + queue_additions.rs) and reviewer_matrix at the candidate, debug and release.
   Expect storage 5/6 (only review_r2_compound_failure_must_preserve_restriction fails, in its replay=false
   fail_invalidation=true case: the case erratum E5 excludes), queue 3/4 (queue_expected_partial_flush_read_gate
   fails at its `anchored == before` premise: every record is now anchored as it is appended, so the anchor
   equals the journal and the read it checks is served at an anchored sequence; judge whether the invariant it
   guards holds), matrix 4/4. Compare with the author's reviewer_reproductions/ logs in the VM evidence.
2. R2-01 (anchor.rs open; registry.rs open): the counter continued above the rejected slots so that this run's
   writes outrank them; the quarantine anchored at the same start; both slot orders, two invalid slots, unclean
   and clean restarts. Confirm equal counters are still rejected and that a rejected image can no
   longer outrank the fresh anchor by any sequence of restarts you can construct.
3. R2-02 (registry.rs open): a ledger failure during the start's own replay — with the validity file intact, with
   every write failing — gives an `unverifiable` marker or a refused start, and the next start contains; the
   seeded defect start_marker_ignores_failed_ledger; contain_everyone's own ledger appends at a contained start.
4. R2-03 against erratum E5 (ledger.rs retry_invalidation; registry.rs retry_unverifiable and its hooks): both
   writes owed and retried by every operation; rollback_status.unverifiable_owed; nothing eligible served
   meanwhile. Check that the implementation loses a restriction only in the case E5 excludes (all four marks
   failed, death before any retry, same-boot restart) and that every other combination you can construct
   contains or replays it; check that E5's stated consequences match the code and the test's second case.
   You may comment on E5's wording for the owner; make no owner decision.
5. R2-04 (registry.rs flush_pending): one record at a time, each anchored before the next append; a failed anchor
   leaves exactly one unanchored record behind which the rest waits (reads refused, same-boot restart without
   quarantine); a journal failure mid-queue leaves the appended records anchored and served; every early return.
   Try longer queues, failures at every record, a death after each step.
6. Lock refresh (a2d0ebd): exactly four lines change (libcurl3t64-gnutls, libcurl4t64: deb13u5 -> deb13u7);
   your own no-cache build reproduces both lock files byte for byte; compare with the CI artifacts at 9633ce3
   (run 38577923955) and at the candidate; nothing pinned changes; judge the ADR 0003 §2 record.
7. Harness: 87 seeded defects in test_m2_registry_policy.py (6 new, two anchors re-pointed); judge whether each new
   defect's test discriminates for the right reason. Inventory 206 cargo tests.
8. Evidence: qualification in a clean clone at 2217e62 (CHAM3_M2_REQUIRE_CLEAN=1; the nofile limit as
   before): unit 206+206, provider PASS, ECR 19/96/18 with 23 scenarios / 352 steps, format differential 5955 / 0
   (corpus unchanged), vault oracle 38, harness 147 (12 reserve-probe, 87 registry/policy and 21 identity/vault
   seeded defects); both lock checks PASS; static checks and sh tests/run_tests.sh static; the full-filesystem
   wrapper and the five wrapper cases on the 2 MiB tmpfs. Compare GitHub Actions m2_checks 38584362113, m1_checks 38584362062, m0_checks 38584362077; download the
   artifact and check its digest (artifact 11906127702, sha256:1ed83981cfbe186c42969c96760b96f8bfc189dc2d348d5c78a777078de3937a; at 9633ce3: 11902319817, sha256:1f589df6a09b43bf8d8162b77fa87fdccad3a40962e8c30509fe11634ff444dc).
9. Codex Security plugin on fc147e0..2217e62; record name/version and findings.

Record every check as PASS/FAIL/NOT_RUN/BLOCKED with exact commands and evidence. Return APPROVE or
CHANGES_REQUESTED for M2-006 stage A revision 3 and make no owner decisions (list questions for the owner
separately). Publish only your record on a new review branch (review/m2-006-a-r3-security) and end with:

=== CHAM3 M2-006 STAGE A R3 VERDICT ===
reviewer: Codex
environment: <OS/arch; UID; Docker access: yes/no and how; image id and how built; nofile limit used>
local_clone: <~/cham3 HEAD after step 0; local changes or stashes: none / moved aside (what)>
review_record: <branch> @ <commit> : <path> (sha256 <digest>)
candidate_reviewed: 2217e62ddf42c42573346155727c983d937addc0
docs_commits_reviewed: <shas> (docs-only: yes/no)
scope_exact: <the 13 files and nothing else: yes/no>
verdict: M2-006 stage A revision 3: <APPROVE/CHANGES_REQUESTED>
reviewer_tests_at_candidate: <storage x/6; queue x/4; matrix x/4; debug and release>
r2_01_rejected_slots: <PASS/FAIL> (<..>)
r2_02_start_marker: <PASS/FAIL> (<..>)
r2_03_against_erratum_e5: <PASS/FAIL> (<retries and status; only the excluded case loses a restriction; E5's consequences match>)
r2_04_queue_drain: <PASS/FAIL> (<..>)
lock_refresh: <PASS/FAIL> (<four lines; reproduced by your build: yes/no; CI artifacts>)
harness_and_seeded_defects: <PASS/FAIL> (<counts>)
full_filesystem_wrapper: <PASS/FAIL>
qualification_amd64: <PASS/FAIL/BLOCKED> (<counts>)
static_checks: <..>   static_tests: <..>
ci_comparison: <..>
security_plugin: <..>
new_findings:
- id: <..> | severity: <..> | location: <file:line> | summary: <..> | required: <..>
owner_questions: <..>
notes_for_owner: <..>
=== END VERDICT ===
~~~
