# Handoff: M2-003 revision 2 — request for independent re-review

- **State:** M2-003 is **IN_REVIEW** (revision 2). The author approves nothing.
- **Role:** IMPLEMENTER.
- **Model/provider:** Claude Code (Anthropic), cloud VM. Requested reviewer: Codex (independent).
- **Branch:** `codex/m2-cryptographic-contract-core` (pushed fast-forward).
- **Review answered:** `docs/reviews/M2-003-codex-security-review.md` at 1cb93c0454583311b04fbbb9e90f01e18aab5ca3 on `review/m2-003-security`. Its parent is 9cf8825, it adds only that file, and its SHA-256 is `e33e8ad4…1f98` (verified; not merged or edited). Author response: [M2-003 finding resolutions](../reviews/M2-003-finding-resolutions.md).
- **Owner decisions used (2026-10-13):** errata E1 (`list_length`), E2 (`wrong_object_type` for any text `typ` other than the expected one), E3 (deterministic-encoding order for text lists). They are recorded in ADR 0004 and applied to the wire format and the CDDL.
- **Base:** f87111d021a743b6937dcc5a6c0efab4310f135b.
- **Revision-2 candidate:** **8b0ccf37f2437cae0a9668e012ddf55e9952e2aa**.
- **Docs-only follow-up:** the commit adding this handoff, the resolutions record, the evidence, the task progress and STATUS.

## Completed work

| Finding | Change | Where |
|---|---|---|
| R01 | Protected-header decode (layer 1) before the other elements' kind checks (layer 2); wire format §1 clarification | `envelope.rs`, `ecr_oracle.py`, spec §1 |
| R02 | No count-driven `Vec::with_capacity` in the array/map decoders; storage grows only with decoded items | `dcbor.rs` |
| R03 / E2 | Any text `typ` other than the expected one gives `wrong_object_type`; a non-text `typ` gives `protected_header_invalid`. The `typ_unknown` vector's expected code changes (bytes unchanged) | `envelope.rs`, oracle, spec §§4.1/8, CDDL, vectors |
| R04 / E3 | List order = map-key order (deterministic encodings), stated in the spec and CDDL; discriminating vectors | spec §§2.2/6.1/9, CDDL, vectors |
| E1 | `list_length` in spec §§6.1/7 | spec, `error.rs` docs |
| I01 | Expiration equality as a vector, a Rust regression and a seeded defect | vectors, tests |

New controls:

- **Rust tests** (64 per profile):
  - `ecr_vectors`: `protected_header_decode_precedes_other_element_kinds`, `text_typ_other_than_expected_is_wrong_object_type`;
  - `schema_mutations`: `text_lists_ascend_in_encoding_order`, `expiration_equal_to_start_is_a_lifetime_rule_violation`.
- **Harness tests** (130 total):
  - `test_short_container_bodies_reserve_no_declared_capacity` and `test_count_driven_reservation_is_detected` (Valgrind Massif);
  - three more seeded defects, so 8 are now detected by the differential.
- **Vectors:** 19 valid / 96 invalid / 18 admission, 22 scenarios / 346 steps. Apart from the 7 new vectors and `typ_unknown`'s expected code, every vector is byte-identical.

## Verification (at 8b0ccf3)

| Check | Environment / command | Result | Evidence / limits |
|---|---|---|---|
| Full M2 qualification | Clean UID-1000 clone, pinned container, `qualify_m2_rust.sh`, `CHAM3_M2_REQUIRE_CLEAN=1` | **PASS**: unit 64 + 64 cargo tests; provider oracle 7/7; ECR oracle 19/96/18 with 22 scenarios / 346 steps; format differential 5 950 cases, 0 mismatches, 56 distinct outcomes; harness 130 | [m2_003_amd64_vm_8b0ccf3](../evidence/M2-cryptographic-contract-core/m2_003_amd64_vm_8b0ccf3/) |
| Static checks; static tests | same container settings | PASS; PASS (82) | same directory |
| Unchanged inputs | `git diff --exit-code f87111d 8b0ccf3` over manifests, Cargo.lock, build, CI, PROJECT, other crates, IPC spec; dependency inventory; vector sections | exit 0; identical; only the 7 added vectors and `typ_unknown`'s expectation differ | `unchanged_inputs.txt` |
| The review's own `adversarial.py` and `allocations.py` (unchanged, digests match the record) | at 4841039 and 8b0ccf3 | 6 → 2 prose mismatches (the 2 left are pre-E1 `list_too_long` expectations); 0 Rust/oracle mismatches in both. Massif peaks 25 629 / 33 821 → 17 477 / 17 477 bytes | `reviewer_reproductions/` |
| GitHub Actions | m2 [38327808145](https://github.com/Kayyo321/cham3/actions/runs/38327808145), m1 [38327808083](https://github.com/Kayyo321/cham3/actions/runs/38327808083), m0 [38327808082](https://github.com/Kayyo321/cham3/actions/runs/38327808082) | **PASS** (every job and step) | [m2_003_amd64_ci_8b0ccf3](../evidence/M2-cryptographic-contract-core/m2_003_amd64_ci_8b0ccf3/); artifact 90 097 bytes, `sha256:0e77977e…0eb8`; ZIP download NOT_RUN by the author |
| Fuzzing, Miri, sanitizers; key authority, admission, services | — | NOT_RUN | Unchanged limits |

## Remaining

- **Known risks:**
  - **M2-004 obligation:** identityd must call `envelope::public_key_is_valid` on enrolled and certified public keys. Bad subject points pass layers 1–5 by design, as the reviewer noted.
  - Fuzzing, Miri and sanitizers are NOT_RUN.
  - Services are NOT_RUN.
- **Uncommitted state:** none after the evidence commit.
- **Next:**
  1. Codex re-reviews M2-003 revision 2.
  2. After an APPROVE, M2-003 is DONE and M2-004 starts.
- **Reminder for the end of M2:** the final review uses the Codex Security plugin skill. Codex's plugin workflow was BLOCKED in its last environment; a later review must record whether it ran.

## Re-review prompt for Codex

~~~text
You are the independent REVIEWER (Codex) for cham3 M2-003 revision 2 in Kayyo321/cham3. You did not author this work.
Read AGENTS.md, your review (review/m2-003-security at 1cb93c0454583311b04fbbb9e90f01e18aab5ca3,
docs/reviews/M2-003-codex-security-review.md), docs/reviews/M2-003-finding-resolutions.md,
docs/handoffs/M2-003-rereview-r2.md, the wire format and CDDL as amended by owner errata E1-E3, and ADR 0004's errata
record. Treat author claims and evidence as claims to verify; judge against the amended specification text.

Targets: candidate 8b0ccf37f2437cae0a9668e012ddf55e9952e2aa (range f87111d021a743b6937dcc5a6c0efab4310f135b..8b0ccf3)
and the docs-only commit after it on origin/codex/m2-cryptographic-contract-core; confirm that commit changes only docs/.
Do not merge main or review branches into the implementation branch; do not edit implementation files.

1. Owner errata: confirm ADR 0004 records E1 (list_length), E2 (any text typ other than the expected kind's is
   wrong_object_type; non-text typ is protected_header_invalid) and E3 (strictly ascending lists use the map-key
   comparator, deterministic encoding order) as owner decisions of 2026-10-13, that the wire format (sections 1,
   2.2, 4.1, 6.1, 7, 8, 9) and CDDL comments match them, and that no type or byte format changed. Make no owner
   decision yourself.
2. R01: the protected header is decoded before the other elements' kinds are checked, in Rust and the oracle;
   your case protected_decode_before_bad_unprotected_kind and the new vector give envelope_decode/non_minimal_encoding;
   authentication still precedes payload parsing. Try further combined-error cases.
3. R02: no count-driven reservation remains in the decoder (check every Vec::with_capacity and collect in
   core/contract_format), the first-error order is unchanged, and your allocations.py shows no declared-capacity
   reservation; judge the new Massif harness test and its sensitivity control.
4. R03/R04/I01: confirm the new and changed vectors (git diff of vectors.json: 7 added, typ_unknown expectation
   changed, everything else byte-identical), the Rust tests, the three new seeded defects, and that the default
   differential detects your expiration-equality defect.
5. Evidence: in a clean clone at 8b0ccf3 owned by an unprivileged UID, build the pinned image and run the
   qualification (docker run --user "$(id -u):$(id -g)" ... bash build/scripts/qualify_m2_rust.sh with
   CHAM3_M2_REQUIRE_CLEAN=1; no --privileged, no added capabilities, no --record) and confirm: unit 64+64 cargo tests,
   provider oracle 7/7, ECR oracle 19/96/18 with 22 scenarios / 346 steps, format_differential 5950 cases with 0
   mismatches, harness 130; then build/scripts/run_static_checks.sh and sh tests/run_tests.sh static. Re-run your own
   adversarial.py, allocations.py and defects.py. Compare GitHub Actions m2_checks 38327808145, m1_checks 38327808083
   and m0_checks 38327808082 (artifact: gh run download 38327808145 -n m2_qualification_amd64).
6. Confirm every earlier PASS item (scope and boundaries, signature path, layer 5, oracle fix) is intact, and that
   the M2-004 obligation (identityd must call public_key_is_valid on enrolled/certified keys) is recorded.
7. If the Codex Security plugin skill is available in your environment, run it on the candidate range and record its
   exact name/version, commands and findings; if not, record BLOCKED with the reason (it remains required for the
   final M2-008 review).

Record every check as PASS/FAIL/NOT_RUN/BLOCKED with exact commands, environment and evidence. Return APPROVE or
CHANGES_REQUESTED for M2-003; make no owner decisions. Publish only your review record on a new review branch
(review/m2-003-r2-security), and end your reply with this block filled in:

=== CHAM3 M2-003 R2 VERDICT ===
reviewer: Codex
environment: <machine/OS/arch; UID; Docker version; network notes>
review_record: <branch> @ <commit> : <path> (sha256 <digest>)
candidate_reviewed: 8b0ccf37f2437cae0a9668e012ddf55e9952e2aa
docs_commits_reviewed: <shas> (docs-only: yes/no)
verdict: M2-003: <APPROVE/CHANGES_REQUESTED>
prior_findings: R01=<PASS/FAIL> R02=<PASS/FAIL> R03=<PASS/FAIL> R04=<PASS/FAIL> I01=<PASS/FAIL> earlier PASS items intact=<yes/no>
errata_record: <E1/E2/E3 recorded and applied consistently: yes/no>
qualification_amd64: <PASS/FAIL/BLOCKED> (<counts, format_differential cases/mismatches>)
static_checks: <..>   static_tests: <..>
own_scripts: <adversarial cases / Rust-oracle mismatches / spec mismatches; allocations; defects>
security_plugin: <ran (name/version, findings) / BLOCKED (reason)>
ci_comparison: <..>
new_findings:
- id: <..> | severity: <..> | location: <file:line> | summary: <..> | required: <..>
notes_for_owner: <..>
=== END VERDICT ===
~~~
