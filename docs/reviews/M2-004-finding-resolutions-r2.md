# M2-004 finding resolutions (author response, revision 3)

- **Nature of this record:** the implementer's response to the independent re-reviews of M2-004 revision 2. It is **not** a review and approves nothing.
- **Two independent records exist for M2-004 revision 2 (candidate ca40c62ed7a5fc29ba8910fef514f3ce29ea4fba, docs acefb66), and they disagree:**
  1. `review/m2-004-r2-security` at fd6de588bbee575d936fde448e823bff08f44e7a (`docs/reviews/M2-004-codex-security-rereview-r2.md`, SHA-256 `8736aee2b242e3e6f9ba7bf98733dfd6bb49eee7f0be3e663b1c82b71b8abae7`; parent 9cf8825, adds only that file; published 2026-10-13 12:10 UTC): **CHANGES_REQUESTED**. R01, R02, R03 and R05 PASS; R04 FAIL overall through one new Low finding, **M2-004-R2-01**; I01 documentation corrected. The owner did not relay this record; the author found it on the remote.
  2. The combined record `review/m2-005-security` at f9f3b828fcd117ebcaa4179fa6ae05e782381db1 (`docs/reviews/M2-005-codex-security-review.md`, SHA-256 `8b36e400309a8936f7b8ed287edd226905aaa10f117b904240d46d14bbcc0a18`; parent 9cf8825, adds only that file; published 2026-10-13 14:34 UTC, relayed by the owner): **APPROVE for the recorded staged scope**, R01–R05 and I01 PASS, "no new findings in M2-004 revision 2". It does not mention the first record.
  - The author infers no approval from the later record: a published CHANGES_REQUESTED stands until a reviewer closes it, and the two records do not reference each other. **M2-004 stays IN_REVIEW.** The finding is fixed here as revision 3, which costs nothing extra (the whole branch is re-qualified for M2-005 revision 2 anyway) and lets either record be satisfied. **Owner decision requested:** which M2-004 revision-2 verdict governs, or whether Codex re-reviews revision 3 (the prompt in the handoff asks Codex to state how the two records relate).
- **Owner decisions used:** none were needed for the change itself.
- **Branch / base / candidate:** `codex/m2-cryptographic-contract-core`. Base 24a35c1914d787b936d14b37b500226dde51722f; revision-3 candidate **051b4c8aacdeecfabb23625f93dd16615aec39a0** (the range 24a35c1..051b4c8, two commits, shared with M2-005 revision 2; the M2-004 change is in 4f7080b). Commands and results: [revision-3 handoff](../handoffs/M2-004-rereview-r3.md).

## Reproduction first

The reviewer's twelve additional tests were reconstructed byte for byte from the first record's appendix (`new-tests.rs`, SHA-256 `e80537319a6426b2e26f1c5048e15eb47e4e3413bc8c171363df43994ddb0eb8`, appended to the first review's unchanged `review.rs`; the concatenation's SHA-256 `d619b29f72eea7f71600258a5e41c3f1f0609809c963f5cd690b912b51ea0b20` matches the record) and run unchanged at ca40c62 in a clean UID-1000 clone, debug and release: **10 passed, 2 failed** in each profile, with 39 retained ids against the per-user bound 32 and 319 against the global bound 256, as the review reports.

## Resolution matrix

| ID | Resolution | Changes | Verification |
|---|---|---|---|
| M2-004-R2-01 (Low) — pending-to-retired transitions exceeded the retained-id quotas | Registration counts **every** kept id — pending, used and cancelled — against the retained-id bounds (32 per user, 256 in total), so a pending proposal's later transition to used or cancelled never exceeds them: its retained slot is reserved when it registers. The pending bounds (8 per user, 64 in total) remain the smaller limits on live proposals. No marker is dropped, no id revived, no admitted workflow stranded: a pending proposal still completes or cancels unconditionally. | `vaultd/src/service.rs` `register_proposal` | **Rust test** `pending_proposals_count_against_the_retained_id_bounds`: 30 cancelled and 1 used id, then exactly one more proposal registers and the next two are `quota_exhausted`; after its cancellation nothing registers; a second round ends with an approval instead of a cancellation with the same outcome; a fresh id registers after the lifetime.<br>**Rust test** `the_global_retained_id_bound_counts_pending_proposals_of_every_user`: nine users with 255 retired ids; pending registrations across eight users fill exactly one slot; after its transition the tenth user's registration is `quota_exhausted`; a fresh id registers after the lifetime.<br>**Seeded defect** `pending_proposals_not_counted_as_retained` (the revision-2 count) is detected.<br>**Reviewer's tests** `r2_retired_per_user_quota_reserves_pending_transitions` and `r2_retired_global_quota_reserves_pending_transitions` pass; the other ten of the twelve, and the original seventeen (16 pass, the I01 assertion fails as recorded), are unchanged. |

## Scope of the change

- **Rust:** `vaultd/src/service.rs` (one check); tests `vault_operations` (+2).
- **Harness:** `test_m2_identity_vault.py` (+1 seeded defect, 21). The inventory gains the 2 Rust tests.
- **Not changed:** every other M2-004 module, dependencies and `Cargo.lock`, build inputs, the key-object format and the vault-object oracle, the specifications, ADRs and vectors.

## Not changed, and carried forward

- The staged obligations of revision 2 stand as both records state them: the M2-006 restriction ledger (a not-yet-durable denial across an identityd restart), cross-service serialization of restrictions with signatures (M2-006/M2-007), duplicate request ids at admission (fulfilled by M2-005, which the combined record confirms).
- The Codex Security plugin was BLOCKED in both reviewer runs; it remains required for the final M2-008 review.
- Nothing is merged; neither review record is edited.
