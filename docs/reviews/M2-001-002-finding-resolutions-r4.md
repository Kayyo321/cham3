# M2-001 / M2-002 finding resolutions (author response, revision 4.1)

- **Nature of this record:** the implementer's response to the independent re-review of revision 4. It is **not** a review and approves nothing. The reviewer's G2–G6 APPROVE verdicts are design recommendations for owner consideration, not owner approval. Every proposal stays PROPOSED and every owner decision stays open.
- **Re-review answered:** `docs/reviews/M2-001-002-codex-security-rereview-r4.md`, commit 3a3b5d17ddc44b96c3fcc8265c5b337066aff0fb on `review/m2-001-002-r4-security`. Its parent is 9cf882503848f8c6cc4938a588cc9fc062a9392d and it adds only that file; its SHA-256 is `4d1d22b89e06460d73a2d9e60aa39ef924f83ac02fc5eafce2fdeaae9f8d433d` (verified).
  - It reviewed candidate 9b1aa2b9d1435fb33c303ca7fe01ce7afcb2964a and evidence/handoff 70c431c13a1c76fd2948e6c8f0ae33b720618043.
  - Verdicts:
    - CHANGES_REQUESTED: M2-001 and G1, solely because the reviewer's own Debian tracker retrieval was BLOCKED (HTTP 403).
    - **APPROVE for owner consideration:** G2, G3, G4, G5 and G6.
  - R13, R14 and R15 PASS; every earlier passed item is intact.
  - New note I04 (Low, nonblocking, G4).
- **Branch / base:** `codex/m2-cryptographic-contract-core` from 70c431c13a1c76fd2948e6c8f0ae33b720618043. Exact commits, commands and results: [revision-4.1 handoff](../handoffs/M2-001-002-rereview-r5.md).

## Resolution matrix

| ID | Gate | Resolution | Status | Changes | Verification |
|---|---|---|---|---|---|
| I04 | G4 | The naming scenarios now **observe key-authority state directly**, independent of each step's response. A step may carry `keys = {named, min_key_epoch, retired}`:<br>• it is asserted after every faulted naming, faulted commit and faulted status step while the record is unanchored (unnamed: no recovery key in scope, minimum epoch unchanged, nothing retired);<br>• it is also asserted after the restart roll-forward, the successful retry and the successful first naming (named: the ceremony keys in scope, minimum epoch 2, pre-ceremony contract keys retired).<br>The reviewer's **response-preserving** early-application mutant is now a harness defect test. That mutant applies the naming before anchoring but keeps `storage_unavailable`, the pending-tail block, the retry result and the record counts. The existing response, served-state and record-count controls are kept. | oracle/vectors + test | `ecr_oracle.py` (`keys` observation in `run_scenario`; `keys_observed` in the 3 `recovery_naming_*` scenarios); `test_m2_review_regressions.py` (`test_defect_naming_effective_before_anchor_with_preserved_responses_is_detected`); vectors readme; inventory | The new test passes on the fix and requires a `key state` failure in **both** anchor-failure scenarios with no response or record-count difference. The reviewer's `early_effect_probe.py`, unchanged:<br>• at 9b1aa2b: zero scenario failures (it asserts that and exits 0);<br>• at the revision-4.1 candidate: the scenarios report key-state failures and the probe's own `assert not failures` fails (exit 1).<br>See the evidence. Model only; service durability is NOT_RUN. |
| G1 Debian check | M2-001, G1 | This cannot be fixed in the candidate. The owner now offers a desktop environment for the independent reviewer, where `security-tracker.debian.org` is expected to be reachable. The re-review prompt asks the reviewer to perform the check there with its own retrieval. | owner/environment action | none | Pending the reviewer's run |

## Scope of the change

Revision 4.1 changes only test and oracle files:
- `tests/m2/oracle/ecr_oracle.py` (scenario observation and scenario steps);
- `tests/m2/vectors/ecr_v1/vectors.json`, where only the `keys` fields were added to 9 steps of the 3 `recovery_naming_*` scenarios. Every other section and scenario is byte-identical; the counts are unchanged at 22 scenarios and 346 steps;
- `tests/m2/harness/test_m2_review_regressions.py` (+1 test, 120 harness tests);
- `tests/m2/test_inventory.txt`;
- `tests/m2/vectors/ecr_v1/vectors_readme.md`;
- this record.

Nothing else changed: no ADR, specification, CDDL, IPC text, script, Rust source, build input or CI workflow. The approved-for-consideration texts of G2–G6 are therefore exactly the ones the re-review recommended.

## Not changed

- Every review record and the review evidence archive; `main` and the review branches were not merged.
- Owner decisions, all still open:
  - G1–G6;
  - how G1's independent Debian check is obtained;
  - ADR 0006 §9.1 and §9.2;
  - ADR 0007 §4 and §8, including forward-error recovery;
  - ADR 0008 §3.
