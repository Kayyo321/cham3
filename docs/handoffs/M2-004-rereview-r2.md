# Handoff: M2-004 revision 2 — request for independent re-review

- **State:** M2-004 is **IN_REVIEW** (revision 2). The author approves nothing.
- **Role:** IMPLEMENTER. **Model/provider:** Claude Code (Anthropic), cloud VM. Requested reviewer: Codex (independent).
- **Branch:** `codex/m2-cryptographic-contract-core` (pushed fast-forward).
- **Review answered:** `docs/reviews/M2-004-codex-security-review.md` at f2d82b7a663c1579582bc5c88fe2832a6f97e061 on `review/m2-004-security`. Its parent is 9cf8825, it adds only that file, and its SHA-256 is `eb0b011f…596a` (verified; not merged or edited). Author response: [M2-004 finding resolutions](../reviews/M2-004-finding-resolutions.md).
- **Owner decisions used:** none were needed.
- **Base:** d2c29ad7a09b2b1d49a23e9fa92c9656aa74c711 (the revision-1 docs commit after candidate bb3f1ba).
- **Revision-2 candidate:** **ca40c62ed7a5fc29ba8910fef514f3ce29ea4fba** (one commit on the base).
- **Docs-only follow-up:** the commit adding this handoff, the resolutions record, the evidence, the task progress and STATUS.

## Completed work

| Finding | Change | Where |
|---|---|---|
| R01 | A pending denial is cleared only when the durable index covers it (a pending `compromised` needs a durable `compromised`), on both the `already_present` and the newly appended branch | `identityd/src/service.rs` |
| R02 | `check_signing_window`: the key's certificate window at the current time, at registration and immediately before the signature; layer 6's timestamp check unchanged | `identityd/src/index.rs`, `vaultd/src/service.rs` |
| R03 | Key status, scope, window and stored object checked again after the derivation, immediately before `sign`; the identity key's scope (and, for `recover_keys`, the open ceremony) again before identity-key signatures | `vaultd/src/service.rs` |
| R04 | Used and cancelled proposal ids kept (without bytes) until their lifetime ends; re-registration `duplicate_identifier`; a used marker cannot be cancelled; kept ids bounded (32 per user, 256 total) | `vaultd/src/service.rs` |
| R05 | Every `recover_keys` registration/naming failure reconciled through `recovery_status`; nothing erased on an error alone | `vaultd/src/service.rs` |
| I01 | No behavior change; `recover_keys` documentation states that completing a held bundle needs no passphrase (nothing is created or signed) | `vaultd/src/service.rs` docs; this handoff |

Corrections to the revision-1 handoff (which stays as the record of what was submitted):

- "a definitive refusal erases only the bundle's new keys" no longer holds for `recover_keys`: it erases only on positive `recovery_status` evidence (R05). It still holds for enrollment, rotation and invalidation bundles.
- "a retry resumes with the held keys and needs their passphrase" holds for `create_user_keys`. A held recovery bundle is completed without a passphrase, by `recover_keys` and by every other vault operation of the user (I01).
- Approval order is now: owner, unused, unexpired (both clocks), digest; key status, scope and current window; attempt limit, derivation and decryption (`unlock`); proposal still live; key status, scope and current window **again**; signature; the id stays used until the lifetime ends. (The revision-1 list put the attempt limit before the key check; in the code the key check came first then too.)

New controls:

- **Rust tests** (121 per profile, 7 new):
  - identityd `service_operations`: `a_pending_compromise_is_kept_until_a_durable_compromise_covers_it`;
  - vaultd `vault_operations`: `proposal_ids_stay_single_use_for_their_lifetime`, `a_live_proposal_does_not_extend_the_signing_keys_window`, `authority_changes_during_the_derivation_prevent_the_signature`;
  - vaultd `vault_recovery`: `a_definitive_naming_error_is_reconciled_from_the_status`, `after_a_definitive_naming_error_only_an_ended_ceremony_erases`, `a_ceremony_that_ends_during_the_derivation_gets_no_certificates`.
- **Seeded defects:** 20 (6 new: `pending_compromise_cleared_by_weaker_record`, `used_proposal_ids_forgotten`, `signing_window_not_checked`, `authority_not_rechecked_after_derivation`, `ceremony_not_rechecked_after_derivation`, `definitive_naming_error_erases_held_keys`; `proposal_reusable` re-anchored; `scope_not_rechecked_before_signing` redefined as `scope_not_checked_at_approval`, removing both approval checks).
- **Test fixture:** `interleaving_identity_t` applies another service's change right after a chosen `resolve_key` answer (an in-process interleaving, not a socket race); the registry stand-in can refuse naming with a definitive error.

## Verification (at ca40c62)

| Check | Environment / command | Result | Evidence / limits |
|---|---|---|---|
| Full M2 qualification | Clean UID-1000 clone, pinned container (rebuilt, every step CACHED), `qualify_m2_rust.sh`, `CHAM3_M2_REQUIRE_CLEAN=1`; no `--privileged`, no added capabilities, no `--record` | **PASS**: unit **121 + 121** cargo tests; provider oracle 7/7; ECR oracle 19/96/18 with 22 scenarios / 346 steps; format differential 5 950 cases, 0 mismatches; vault_object_oracle 38/38; harness **133** (**20** seeded identity/vault defects detected; protection refusal) | [m2_004_amd64_vm_ca40c62](../evidence/M2-cryptographic-contract-core/m2_004_amd64_vm_ca40c62/) |
| Layer-6 differential | `journal_scenarios -- --nocapture` at ca40c62 | PASS: 12 scenarios, 318 comparisons, 0 disagreements (unchanged) | `reviewer_reproductions/at_ca40c62/` |
| Static checks; static tests | same clone and container | PASS; PASS (82); the clone was clean afterwards | `static_checks.log`, `static_tests.log` |
| Unchanged inputs | `git diff --exit-code bb3f1ba ca40c62` over manifests, Cargo.lock, `.cargo`, CI, build, PROJECT, vectors, oracles, harness library, protection script, other crates, the unchanged identityd/vaultd modules, specifications and ADRs | exit 0; only the 9 files listed in `unchanged_inputs.txt` differ; dependency inventory and toolchain identical | `unchanged_inputs.txt` |
| The review's own 17 tests (unchanged; `review.rs` and `Cargo.toml` digests match the record) | read-only clean clones, debug and release | **bb3f1ba: 11 passed, 6 failed** (as the review reported). **ca40c62: 16 passed, 1 failed**: the R01–R05 reproducers pass; the remaining failure is I01's stricter assertion, answered by documentation | `reviewer_reproductions/` |
| Seeded defects | `test_m2_identity_vault` in a disposable copy | PASS: each of the 20 defects makes its named Rust test fail (not the build); the unmodified copy passes | `harness/python_harness.json` |
| Vault protection | refusal in this VM; positive path in CI (`--ulimit memlock=1073741824:1073741824`) | Refusal PASS. Positive path **PASS in CI** (m2_checks 38347259191): non-dumpable, core limit 0, 0 unlocked mappings, `[vdso]` special, release derivation 471 ms. **BLOCKED in this VM** (memlock cannot be raised) | `protection.txt` |
| GitHub Actions at ca40c62 | m2 [38347259191](https://github.com/Kayyo321/cham3/actions/runs/38347259191), m1 [38347259188](https://github.com/Kayyo321/cham3/actions/runs/38347259188), m0 [38347259258](https://github.com/Kayyo321/cham3/actions/runs/38347259258) | **PASS** (every job and step) | [m2_004_amd64_ci_ca40c62](../evidence/M2-cryptographic-contract-core/m2_004_amd64_ci_ca40c62/); artifact 11789878130, 97 312 bytes, `sha256:6315b087…a5d`; ZIP download NOT_RUN by the author |
| Miri, sanitizers, coverage-guided fuzzing | — | NOT_RUN (NOT_AVAILABLE on the pinned toolchain) | Bounded seeded and differential tests only |
| IPC, service processes, registryd admission, restriction ledger, human presence | — | NOT_RUN | M2-005..M2-007; M5 |

## Remaining

- **Known risks and limits:**
  - in-process composition only, with labeled fixtures for registryd;
  - a pending denial is held in identityd's memory; holding it across an identityd restart is the M2-006 restriction ledger's job (the vault's pending bundle retries meanwhile and the vault does not sign for that user until it is durable);
  - the second authority check narrows, but does not eliminate, the window in which a restriction can cross a signature; cross-service serialization is M2-006/M2-007;
  - proposal ids are kept in memory with the pending proposals (a vault restart drops both); admission must reject duplicate request ids (M2-005);
  - passphrase knowledge with exact-digest binding is **not** a trusted display, trusted input path or proof of human presence (ADR 0005 §8); same-UID isolation is M3; the attempt counter is not rollback-protected (ADR 0006 §5);
  - Miri, sanitizers and fuzzing are NOT_RUN.
- **Uncommitted state:** none after the evidence commit.
- **Next:**
  1. Codex re-reviews M2-004 revision 2.
  2. After an APPROVE, M2-004 is DONE and M2-005 starts.
- **Reminder for the end of M2:** the final review uses the Codex Security plugin skill. It has been BLOCKED in every reviewer environment so far; this re-review should try it again.

## Re-review prompt for Codex

~~~text
You are the independent REVIEWER (Codex) for cham3 M2-004 revision 2 in Kayyo321/cham3. You did not author this work.
Read AGENTS.md, your review (review/m2-004-security at f2d82b7a663c1579582bc5c88fe2832a6f97e061,
docs/reviews/M2-004-codex-security-review.md), docs/reviews/M2-004-finding-resolutions.md,
docs/handoffs/M2-004-rereview-r2.md, ADR 0005 section 2, ADR 0006 sections 3, 5 and 10, and docs/specs/m2_service_ipc_v1.md
section 3 (identityd register_invalidation, the vaultd rows, registryd restrict, R14). Treat author claims and evidence
as claims to verify; judge against the approved ADRs and specifications.

Targets: candidate ca40c62ed7a5fc29ba8910fef514f3ce29ea4fba (one commit on d2c29ad7a09b2b1d49a23e9fa92c9656aa74c711; the
code change against the candidate you reviewed is bb3f1bad77080da34069af62c8d2cc390f2fcf7e..ca40c62) and the docs-only
commit after it on origin/codex/m2-cryptographic-contract-core; confirm that commit changes only docs/.
Do not merge main or review branches into the implementation branch; do not edit implementation files.

1. R01: identityd clears a pending denial only when the durable index covers it (core/identityd/src/service.rs
   settle_pending_denial, durable_covers), on the already_present and the newly appended branch. Re-run your
   reproducer and try other orderings (several keys, journal read faults, refresh and rebuild, a pending rotated
   key later compromised durably). Judge the carried-forward obligation: holding a not-yet-durable denial across an
   identityd restart is the M2-006 restriction ledger's; in M2-004 the vault's pending bundle retries meanwhile.
2. R02: index::check_signing_window (current UTC time, inclusive bounds, unreadable times fail closed) applied in
   vaultd check_signing_key at registration and before and after the derivation; layer 6's historical
   payload-timestamp check unchanged. Re-run your reproducer; probe the boundaries.
3. R03: approve_and_sign checks key status, scope, window and the stored object again after the derivation,
   immediately before sign; rotate_key and invalidate_key recheck the identity key, recover_keys the open ceremony
   and the identity key's ceremony scope, before identity-key signatures. Re-run your clock-scheduled reproducer,
   judge the new interleaving fixture (core/vaultd/tests/fixture/mod.rs interleaving_identity_t) and its tests, and
   check that the remaining cross-service serialization limit is stated accurately.
4. R04: proposal_state_t keeps used and cancelled ids, without bytes, until the proposal's lifetime ends;
   re-registration is duplicate_identifier; cancel_proposal cannot remove a used marker; kept ids are bounded
   (32 per user, 256 total; quota_exhausted). Re-run your reproducer with further interleavings. Judge the stated
   limits: ids are kept in memory with the pending proposals (a vault restart drops both); admission must reject
   duplicate request ids (M2-005).
5. R05: recover_keys reconciles every registration or naming failure through the pending bundle and
   recovery_status, erasing only on positive evidence. Re-run your definitive-naming-error reproducer and the abort
   case.
6. I01: confirm there is no behavior change and that the code documentation and the revision-2 handoff now describe
   recovery finalization and the enrollment retry correctly.
7. Confirm every earlier PASS item is intact: scope and boundaries, roles, the layer-6 scenario differential
   (12 scenarios, 318 comparisons, 0 disagreements), registration and the M2-003 obligation, key objects, KDF, store,
   protection, secret custody, author decisions 1-8 (4 as corrected by R01). Confirm bb3f1ba..ca40c62 changes only
   the files listed in the handoff, with no dependency, Cargo.lock, specification, vector or key-object format change.
8. Evidence: in a clean clone at ca40c62 owned by an unprivileged UID, build the pinned image and run the
   qualification (docker run --user "$(id -u):$(id -g)" ... bash build/scripts/qualify_m2_rust.sh with
   CHAM3_M2_REQUIRE_CLEAN=1; no --privileged, no added capabilities, no --record) and confirm: unit 121+121 cargo
   tests, provider oracle 7/7, ECR oracle 19/96/18 with 22 scenarios / 346 steps, format_differential 5950 cases /
   0 mismatches, vault_object_oracle 38 checks PASS, harness 133 (including 20 identity/vault seeded defects); then
   build/scripts/run_static_checks.sh and sh tests/run_tests.sh static. If your Docker allows it, run the protection
   positive path (tests/m2/protection/check_vault_protection.sh with --ulimit memlock=1073741824:1073741824);
   otherwise record BLOCKED and compare the CI step. Re-run your 17 adversarial tests unchanged in debug and
   release (the author expects 16 to pass and your I01 assertion to fail, as it asks for more than R14) and add your
   own tests for the new code.
9. Compare GitHub Actions m2_checks 38347259191, m1_checks 38347259188 and m0_checks 38347259258 for ca40c62 (artifact:
   gh run download 38347259191 -n m2_qualification_amd64), including the "Vault process protection, positive path" step.
10. If the Codex Security plugin skill is available in your environment, run it on the candidate range and record
   its exact name/version, commands and findings; if not, record BLOCKED with the reason (it remains required for
   the final M2-008 review).

Record every check as PASS/FAIL/NOT_RUN/BLOCKED with exact commands, environment and evidence. Return APPROVE or
CHANGES_REQUESTED for M2-004; make no owner decisions. Publish only your review record on a new review branch
(review/m2-004-r2-security), and end your reply with this block filled in:

=== CHAM3 M2-004 R2 VERDICT ===
reviewer: Codex
environment: <machine/OS/arch; UID; Docker version; network notes>
review_record: <branch> @ <commit> : <path> (sha256 <digest>)
candidate_reviewed: ca40c62ed7a5fc29ba8910fef514f3ce29ea4fba
docs_commits_reviewed: <shas> (docs-only: yes/no)
verdict: M2-004: <APPROVE/CHANGES_REQUESTED>
prior_findings: R01=<PASS/FAIL> R02=<PASS/FAIL> R03=<PASS/FAIL> R04=<PASS/FAIL> R05=<PASS/FAIL> I01=<docs corrected: yes/no> earlier PASS items intact=<yes/no>
carried_obligations: <restriction ledger, cross-service serialization, duplicate request ids at admission: recorded and acceptable for staged scope yes/no (notes)>
qualification_amd64: <PASS/FAIL/BLOCKED> (<counts>)
protection_positive_path: <PASS/FAIL/BLOCKED> (<own run or CI step>)
static_checks: <..>   static_tests: <..>
own_tests: <your 17 tests in debug/release, new tests and results>
security_plugin: <ran (name/version, findings) / BLOCKED (reason)>
ci_comparison: <..>
new_findings:
- id: <..> | severity: <..> | location: <file:line> | summary: <..> | required: <..>
notes_for_owner: <..>
=== END VERDICT ===
~~~
