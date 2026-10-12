# M2-001 / M2-002 finding resolutions (author response, revision 4)

- **Nature of this record:** the implementer's response to the independent re-review of revision 3. It is **not** a review and approves nothing. Every resolution below awaits independent re-review. Every proposal stays PROPOSED and every owner decision stays open. The re-review's G2 recommendation is the reviewer's, not owner approval.
- **Re-review answered:** `docs/reviews/M2-001-002-codex-security-rereview-r3.md`, commit f451f2e346e6981b24802a7e9070ba09303c40d3 on `review/m2-001-002-r3-security`. Its parent is 9cf882503848f8c6cc4938a588cc9fc062a9392d and it adds only that file; its SHA-256 is `4881c1e9de089af544041d5140492aa2920460b09ed0f04a9a9359a745d80b0d` (verified).
  - It reviewed candidate 84fa6ef172b3fad21bb079549776252bb65624d6 and evidence/handoff 42397304663bff6057e26118014c18e7b90a2040.
  - Verdicts:
    - CHANGES_REQUESTED: M2-001, G1, G3, G4, G5, G6.
    - **APPROVE for owner consideration:** G2.
  - New findings: R14 and R15. R13 was FAIL only through the residual interface gap R14.
- **Regression controls used:** the reviewer's R14 static reproduction and R15 `time_limits_probe.py`, both copied unchanged from the re-review appendix and kept outside the repository. Their results are in the revision-4 evidence. Neither review branch was merged and no review was edited.
- **Branch / base:** `codex/m2-cryptographic-contract-core` from 42397304663bff6057e26118014c18e7b90a2040. Exact revision-4 commits, commands and results: [revision-4 handoff](../handoffs/M2-001-002-rereview-r4.md).

## Resolution matrix

Status vocabulary as in [revision 2](M2-001-002-finding-resolutions.md):
- **implemented:** code or tests changed and were executed.
- **design revised:** the proposal changed; implementation and runtime verification remain gated and NOT_RUN.
- **oracle/vectors:** the independent oracle model and the published vectors changed and were executed.

| ID | Gate(s) | Resolution | Status | Main changes | Verification |
|---|---|---|---|---|---|
| R14 | G3, G4, G6 (and R13) | **New registryd operation `name_recovery_keys`, callable only by `service:vaultd`** (`SO_PEERCRED` role).<br>**Arguments:** a closed map of `owner_uid`, `recovery_id`, `persistent_key_id` and `temporary_key_id`. There is no record, kind, epoch or retirement argument; registryd builds the `recovery_keys` record itself. `append_identity` refuses naming and ceremony records, and no registryd operation appends caller-supplied records.<br>**Checks,** under the user lock immediately before the append:<br>• the user is contained and the anchor's `identity_seq` equals the journal's;<br>• the ceremony is open and has named no keys;<br>• both keys are non-invalidated, of this user, persistent and temporary respectively, certified by the current identity key at exactly the ceremony epoch.<br>**Order:** journal → anchor → reply. The record takes effect (epoch floor, retirement, recovery scope) only once anchored.<br>**Durable but unanchored record:** it is the user's pending tail. Every later operation for the user first completes that anchor update or fails with `storage_unavailable`, and startup rolls it forward.<br>**Idempotence:** a retry with identical keys returns `already_named` without a second record. Identical certificate re-registration is idempotent.<br>**vaultd reconciliation:** vaultd reconciles from `recovery_status`, which now also accepts vaultd and reports named keys after close or abort. It never erases on an unknown status or a named key, and it **resumes with the keys it holds** rather than regenerating. A naming request still in flight across a vaultd restart therefore names the same keys. | design revised + oracle/vectors | IPC: `name_recovery_keys`, `recovery_status`, `recover_keys`, `register_certificate`, `append_identity`; ADR 0006 §10 (step 3 row, crash bullet, bypass wording, vectors); ADR 0005 §2 key-scope bullet; ADR 0008 verification; oracle `registry_service_t`, `journal_t.check_naming`/`admit_naming`, `vault_reconcile`; every scenario step now passes through the interface model, and the naming record has no other path into the model | 3 new signed scenarios (`recovery_naming_interface`, `recovery_naming_anchor_failure_restart`, `recovery_naming_anchor_failure_retry`). The 9 R13 ceremony scenarios now name keys through the interface; each differs from revision 3 only in that step.<br>The reviewer's static reproduction now finds the route (regression test). The specification's registryd role table must equal the model's table.<br>Controlled defects detected: naming open to every role; naming effective before anchoring; durable tail lost; non-idempotent retry; old keys not retired, now wrapping `admit_naming`.<br>Also tested: the vault reconciliation table and an in-flight request across a vaultd restart.<br>The `identity_seq` comparison is specified but not modeled. Real IPC, vault and anchor behavior are NOT_RUN. |
| R13 residual | G3, G4, G6 | Closed by R14. The re-review's reading of the bypass sentence is adopted verbatim. No role regains **grant** authority without fresh commits. Closing after step 3 lifts the key-scope restriction even if no lineage was re-approved, but it revives no suspended grant. | design revised | ADR 0006 §10 "Bypass prevention" | Unchanged R13 scenarios still pass, with the naming step routed through the interface |
| R15 | G5 | **The time rule is unchanged.** The two false limits are replaced by honest ones (ADR 0007 §5.7):<br>**(1) Lag accumulates across boots.** Lag never grows within a boot. At each new boot whose wall clock starts behind, it can grow by the unobserved interval since the previous boot's last persisted floor. It is bounded by the sum of those intervals since trusted time last equalled actual time, **not** by the latest floor's age, and it is unbounded under repeated set-back reboots until an accurate time is accepted.<br>**(2) A forward error is accepted and never corrected.** The trusted clock accepts any wall-clock sample ahead of trusted time and never corrects it. Admitted future starts can therefore become eligible arbitrarily early (fail-open for starts), expirations occur early, and honest timestamps are refused until actual time catches up. The ±300 s admission check bounds only a new commit's timestamp.<br>**New owner choice ADR 0007 §8** (none selected):<br>• **A:** accept the limits.<br>• **B (recommended):** observed-time start eligibility. A persisted BOOTTIME-only counter is recorded at admission, so post-admission forward errors cannot advance a start. The cost is that powered-off time never counts.<br>• **C:** an authenticated time source, as a later ADR.<br>Correcting a large forward error remains an open question. | design revised + executable reference model | ADR 0007 §§5.6–5.7, §8, alternatives, security implications, verification; `trusted_time_model.py` (`admission_timestamp_ok`, `observed_clock_t`, `observed_start_eligible`) | 8 new tests in `test_m2_trusted_time.py`:<br>• both reviewer reproductions (lag 7 200 s with a 5 s-old floor and 1 735 s remaining; a noon-admitted 15:00 start eligible at 12:00:01);<br>• lag over three reboots, repaired by an accurate sample;<br>• a never-corrected forward error carried to restarts and new boots, with honest timestamps refused;<br>• a seeded property (200 × 60 steps) asserting `trusted ≤ actual` and the corrected bound, which also reaches the latest-floor counterexample;<br>• option B: no acceleration, the powered-off cost, same-boot restart, and the bound by the error present at admission.<br>Sensitivity: dropping cross-boot floors fails the property. Service clock NOT_RUN. |
| G1 advisory | M2-001, G1 | No code or input change is possible: the blocker is the **reviewer's** environment (HTTP 403 from security-tracker.debian.org). The author repeated the review at the revision-4 candidate (author evidence only). See *Owner/environment action* below. | not changed | none | Author advisory review r4 (evidence) |
| Identity audit note | M2-001, G1 | The re-review found the claim "no identity pipelines anywhere" too broad. ADR 0003 §9 now says exactly which two identity pipelines remain (`dpkg-query \| sort`; `find \| sort \| xargs sha256sum`), and that both run under `pipefail` with checked status. No script changed. | design revised (wording) | ADR 0003 §9 | Reviewer's audit: no unchecked pipeline yields an identity PASS |

## G2 kept intact

The re-review recommended G2 for owner consideration. Revision 4 changes **no normative text** of ADR 0004, the wire format or the CDDL. Only these lines changed:
- ADR 0004: the scenario counts, the review-result row and the approval record.
- Wire format §9: one descriptive clause about the scenario step types.

In `vectors.json`, these sections are byte-identical to revision 3: `valid`, `invalid`, `admission_invalid`, `limits`, `keys`, `constants` and the lineage. In `journal_scenarios`, the 9 network-slot scenarios and the restore scenario are identical. Each of the 9 recovery scenarios differs only in its key-naming step, and 3 scenarios are new (22 scenarios, 346 steps).

## Items the re-review passed (kept intact)

| ID | Re-review result | Revision 4 |
|---|---|---|
| R01-r2 | PASS | Scripts and collectors unchanged; only the ADR 0003 wording above. |
| R07-r2 | PASS (design/model) | Network-slot rules, placement and their scenarios unchanged. |
| R09-r2 | PASS (regression/design model) | Maximum-offset rule and its 12 tests unchanged; R15 adds limits and tests without changing the rule. |
| R12 | PASS (design/model) | Restore predicate, vectors and scenario unchanged. |
| R02, R03, R04, R05, R06, R08, R10, R11, I01, I02, I03 | PASS, intact | Unchanged. The following are byte-identical to revision 3, verified by `git diff --exit-code 84fa6ef -- …` in the evidence: every Rust source, `Cargo.toml`, `Cargo.lock`, `build/`, `.github/`, `PROJECT.md` and ADR 0001. |

## Owner/environment action (G1, M2-001)

The independent Debian advisory check is a reviewer prerequisite that only the reviewer's environment can satisfy. Options for the owner, none taken by the author:
- **(a)** Allow HTTPS to `security-tracker.debian.org` in the reviewer's environment network policy.
- **(b)** Have the reviewer use another independently retrieved, verifiable Debian source and record its identity, for example the tracker's source repository on `salsa.debian.org`, or the signed `trixie-security` indexes for the "fixed version newer than installed" check.
- **(c)** Record an owner decision about how G1 is cleared without it.

The author's tracker snapshot and triage remain author evidence, not clearance.

## Not changed

- Both earlier review records, this re-review record and the review evidence archive; `main` and the review branches were not merged.
- PROJECT.md, ADR 0001, the security invariants, the task's acceptance criteria, the toolchain, the Dockerfile, package pins and locks, crate sources, every Rust source file, every build script and the test runners (`build/`, `tests/m2/run_m2_tests.sh`, `tests/m2/lib/`).
- Owner decisions, all still open:
  - G1–G6;
  - ADR 0006 §9.1: default anchor mode;
  - ADR 0006 §9.2: who opens the ceremony;
  - ADR 0007 §4: one-shot semantics;
  - ADR 0007 §8: time-bound strength (**new**);
  - ADR 0008 §3: deletion granularity.
