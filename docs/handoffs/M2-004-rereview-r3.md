# Handoff: M2-004 revision 3 — request for independent re-review

- **State:** M2-004 is **IN_REVIEW** (revision 3). The author approves nothing. Two independent records for revision 2 disagree (CHANGES_REQUESTED at fd6de58, APPROVE in the combined record f9f3b82); the author treats the earlier CHANGES_REQUESTED as standing and fixes its finding. See the [finding resolutions](../reviews/M2-004-finding-resolutions-r2.md) for both records' identities and digests.
- **Role:** IMPLEMENTER. **Model/provider:** Claude Code (Anthropic), cloud VM. Requested reviewer: Codex (independent).
- **Branch:** `codex/m2-cryptographic-contract-core` (pushed fast-forward).
- **Reviews answered:** `review/m2-004-r2-security` @ fd6de588bbee575d936fde448e823bff08f44e7a (`docs/reviews/M2-004-codex-security-rereview-r2.md`, SHA-256 `8736aee2…abae7`) and the M2-004 part of `review/m2-005-security` @ f9f3b828fcd117ebcaa4179fa6ae05e782381db1 (`docs/reviews/M2-005-codex-security-review.md`, SHA-256 `8b36e400…a018`). Both verified (parent 9cf8825, one file each); neither merged or edited.
- **Owner decisions used:** none for the change. **Owner decision requested:** which revision-2 verdict governs (recorded in the resolutions).
- **Base:** 24a35c1914d787b936d14b37b500226dde51722f (the M2-005 revision-1 docs commit).
- **Revision-3 candidate:** **051b4c8aacdeecfabb23625f93dd16615aec39a0** (range 24a35c1..051b4c8: the implementation 4f7080b, which carries the M2-004 change, and the R03 seeded defects 051b4c8; shared with M2-005 revision 2. The M2-005 handoff has the full verification; this handoff covers the M2-004 change).
- **Docs-only follow-up:** the commit adding this handoff, the resolutions, the evidence, the task progress and STATUS.

## Completed work

| Finding | Change | Where |
|---|---|---|
| M2-004-R2-01 (Low) | `register_proposal` counts every kept proposal id — pending, used and cancelled — against the retained-id bounds (32 per user, 256 in total), so a pending proposal's transition can never exceed them; the pending bounds (8/64) stay the smaller limits on live proposals; no marker dropped, no id revived, no workflow stranded | `vaultd/src/service.rs` |

New controls: Rust tests `pending_proposals_count_against_the_retained_id_bounds` and `the_global_retained_id_bound_counts_pending_proposals_of_every_user` (vaultd `vault_operations`); seeded defect `pending_proposals_not_counted_as_retained` (21 identity/vault defects); inventory +2.

Corrections to the revision-2 handoff: "kept ids bounded (32 per user, 256 total)" now holds for pending ids too; revision 2 bounded only the already retired ones.

## Verification (at 051b4c8aacdeecfabb23625f93dd16615aec39a0)

| Check | Environment / command | Result | Evidence / limits |
|---|---|---|---|
| Reviewer's twelve additional tests (first record's appendix, unchanged) | Clean UID-1000 clones, pinned container, debug and release | **ca40c62: 10 passed, 2 failed** (39 and 319 retained ids). **051b4c8aacdeecfabb23625f93dd16615aec39a0: 12 passed, 0 failed** | [m2_004_amd64_vm_051b4c8](../evidence/M2-cryptographic-contract-core/m2_004_amd64_vm_051b4c8/) `reviewer_reproductions/` |
| Reviewer's original seventeen tests (f2d82b7, unchanged) | same | 051b4c8aacdeecfabb23625f93dd16615aec39a0: 16 passed, 1 failed (the I01 assertion, as both records record) | same |
| Full M2 qualification, static gates, CI, protection positive path | see the [M2-005 revision-2 handoff](M2-005-rereview-r2.md) | PASS | shared candidate |
| Unchanged inputs | `git diff --exit-code 24a35c1 051b4c8aacdeecfabb23625f93dd16615aec39a0` over every vaultd module but `service.rs`, the key-object format and oracle, dependencies, `Cargo.lock` external packages, the specifications and ADRs | exit 0 | `unchanged_inputs.txt` |

## Remaining

- **Known risks and limits:** unchanged from revision 2 (in-process composition; a pending denial lives in identityd's memory until the M2-006 restriction ledger; the second authority check narrows but does not eliminate the cross-service window; proposal ids are kept in memory; passphrase knowledge is not human presence; Miri, sanitizers and fuzzing NOT_RUN).
- **Uncommitted state:** none after the evidence commit.
- **Next:** Codex re-reviews revision 3 together with M2-005 revision 2 (one prompt, in the M2-005 handoff, with both verdict blocks). After an APPROVE that closes both records, M2-004 is DONE.

## Re-review prompt for Codex

The combined prompt is in the [M2-005 revision-2 handoff](M2-005-rereview-r2.md#review-prompt-for-codex); it ends with the `CHAM3 M2-004 R3 VERDICT` block below, which the owner brings back.

~~~text
=== CHAM3 M2-004 R3 VERDICT ===
reviewer: Codex
environment: <home computer: OS/arch; UID; Docker version; network unrestricted yes/no>
review_record: <branch> @ <commit> : <path> (sha256 <digest>)
candidate_reviewed: 051b4c8aacdeecfabb23625f93dd16615aec39a0
docs_commits_reviewed: <shas> (docs-only: yes/no)
revision_2_records: <how review/m2-004-r2-security@fd6de58 (CHANGES_REQUESTED) and review/m2-005-security@f9f3b82 (APPROVE) relate; which stands>
verdict: M2-004: <APPROVE/CHANGES_REQUESTED>
m2_004_r2_01: <PASS/FAIL> (<your two quota tests at the candidate, debug/release; the approval transition>)
earlier_items_intact: <R01-R05, I01, carried obligations: yes/no>
own_tests: <..>
new_findings:
- id: <..> | severity: <..> | location: <file:line> | summary: <..> | required: <..>
notes_for_owner: <..>
=== END VERDICT ===
~~~
