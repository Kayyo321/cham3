# M2-003 finding resolutions (author response, revision 2)

- **Nature of this record:** the implementer's response to the independent M2-003 review. It is **not** a review and approves nothing.
- **Review answered:** `docs/reviews/M2-003-codex-security-review.md`, commit 1cb93c0454583311b04fbbb9e90f01e18aab5ca3 on `review/m2-003-security`. Its parent is 9cf882503848f8c6cc4938a588cc9fc062a9392d, it adds only that file, and its SHA-256 is `e33e8ad44ca54301a1bb5539074e648d7ba30ea22fa487ea29dd212de7dc1f98` (verified). Not merged or edited.
  - It reviewed candidate 4841039 and docs commit f87111d.
  - Verdict: **CHANGES_REQUESTED**. Scope and boundaries, qualification, static gates, the oracle change and CI comparison all PASS. Spec conformance FAILs on R01–R04 (all Low), and I01 is Info.
- **Owner decisions taken for this revision (2026-10-13, answers to explicit questions):**
  - E1: `list_length`;
  - E2 (R03): `wrong_object_type` for any text `typ` other than the expected one;
  - E3 (R04): deterministic-encoding order for text lists.

  Each was the recommended option. They are recorded in [ADR 0004](../decisions/0004_m2_wire_schema_hash.md) and applied to the [wire format](../specs/m2_wire_format_v1.md) and the [CDDL](../specs/m2_ecr_v1.cddl).
- **Branch / base / candidate:** `codex/m2-cryptographic-contract-core`. Base f87111d021a743b6937dcc5a6c0efab4310f135b; revision-2 candidate **8b0ccf37f2437cae0a9668e012ddf55e9952e2aa**. Commands and results: [revision-2 handoff](../handoffs/M2-003-rereview-r2.md).

## Resolution matrix

| ID | Resolution | Changes | Verification |
|---|---|---|---|
| R01 (Low) — protected decode masked by later element kinds | Once the outer item is a tag-18 four-element array whose first element is a byte string, that byte string is decoded (layer 1) **before** the other three elements' kinds are checked (layer 2). The oracle does the same. Wire format §1 states this as a clarification. Authentication before payload parsing is unchanged. | `envelope.rs` `verify_envelope`; `ecr_oracle.py` `parse_and_verify`; wire format §1 | **Vector:** `protected_decode_before_element_kinds` (`envelope_decode/non_minimal_encoding`).<br>**Rust test:** `protected_header_decode_precedes_other_element_kinds`, with six combined-error controls (non-minimal, truncated and trailing protected bytes against bad other elements; valid protected with bad other elements → `not_signed_object`).<br>**Seeded harness defect:** `element_kinds_checked_before_protected_decode` is detected.<br>**Reviewer's own case:** now `envelope_decode non_minimal_encoding`. |
| R02 (Low) — declared capacity reserved before the remaining-input check | The array and map decoders no longer call `Vec::with_capacity(count)`. Storage grows only with items actually decoded, each of which consumes input. The first-error order is unchanged. | `dcbor.rs` | **Harness test** `test_short_container_bodies_reserve_no_declared_capacity`: under Valgrind Massif, the probe's peak heap for `99 01 00` and `b9 01 00` stays within 1 KiB of an empty array's (measured +4 bytes).<br>**Sensitivity control** `test_count_driven_reservation_is_detected`: reintroducing the reservation exceeds the bound.<br>**Reviewer's `allocations.py`, unchanged:** peaks 25 629 / 33 821 bytes at 4841039 become 17 477 / 17 477 bytes at 8b0ccf3, equal to an empty container. |
| R03 (Low) — unknown `typ` code contradicted the prose | **Owner erratum E2.** Any text `typ` other than the expected kind's (another kind, another version or an unregistered value) is `wrong_object_type`, as wire format §§4.1 and 8 say. A non-text `typ` is `protected_header_invalid`. The approved vector `typ_unknown` keeps its bytes and now expects `wrong_object_type`. | `envelope.rs` `check_protected_header` (the known-kind lookup is removed); oracle; wire format §§4.1, 8; CDDL comment; vectors | **Vectors:** `typ_unknown` (changed expectation), `typ_future_version`, `typ_not_text`.<br>**Rust test** `text_typ_other_than_expected_is_wrong_object_type` covers "x", "", an unregistered value, v2, another kind, bytes, an integer and the positive control.<br>**Seeded defect** `unknown_typ_reported_as_malformed_header` is detected.<br>The reviewer's three cases now match the prose. |
| R04 (Low) — text-list comparator unspecified | **Owner erratum E3.** Every "strictly ascending" list uses the map-key comparator: items' complete deterministic encodings, bytewise, so shorter text comes first. Wire format §2.2 and the CDDL now state this. The behavior was already this, so no code change was needed. | wire format §§2.2, 6.1, 9; CDDL comments; vectors | **Valid vector:** `commit_user_1002_encoded_list_order`, with operations `["read","write","append"]` and constraints `["tor_only","no_signing_keys"]`, both distinguishing the two orders.<br>**Invalid vectors:** `schema_operations_plain_order` and `schema_constraints_plain_order` (`list_not_strictly_ascending`).<br>**Rust test** `text_lists_ascend_in_encoding_order` covers both directions, equal length and ties.<br>The reviewer's three label-order cases agree with E3. |
| E1 (known) — `list_too_long` vs `list_length` | **Owner erratum E1:** `list_length` covers a list that is too short or too long. Wire format §§6.1 and 7 are corrected; code and vectors are unchanged. | wire format §§6.1, 7; `error.rs` documentation | The reviewer's two remaining "mismatches" are its pre-erratum `list_too_long` expectations; under E1 the reported `list_length` is the specified code. |
| I01 (Info) — corpus missed expiration equality | Expiration equal to `effective_start` is now a published vector, a direct Rust regression and a seeded harness defect. | vectors; `schema_mutations.rs`; `test_m2_format_verifier.py` | **Vector** `intrinsic_expiration_equals_start` (`intrinsic/lifetime_rule`).<br>**Rust test** `expiration_equal_to_start_is_a_lifetime_rule_violation` (equality rejected, one second later accepted).<br>**Seeded defect** `expiration_equal_to_start_accepted` (`<=` → `<`) is detected by the default differential corpus. |

## Scope of the change

- **Rust:**
  - `envelope.rs` (R01, R03);
  - `dcbor.rs` (R02);
  - `error.rs` documentation;
  - tests `ecr_vectors.rs` (+2) and `schema_mutations.rs` (+2).
- **Oracle and harness:**
  - `ecr_oracle.py` (R01, R03 rules; 7 new vectors; one changed expectation);
  - `test_m2_format_verifier.py` (+2 tests, +3 seeded defects);
  - inventory.
- **Vectors:** 19 valid / 96 invalid / 18 admission, 22 scenarios / 346 steps. Every vector other than the 7 added and `typ_unknown`'s expected code is byte-identical.
- **Specifications:** wire format §§1, 2.2, 4.1, 6.1, 7, 8, 9 and the CDDL comments, as owner errata. No type or byte format changes.
- **Decisions:** ADR 0004 errata record.
- **Not changed:** dependencies and `Cargo.lock`; build inputs; other crates; other ADRs; the IPC specification.

## Not changed, and carried forward

- The final M2-008 review uses the Codex Security plugin skill. The reviewer's plugin workflow was BLOCKED in its environment; it remains an obligation for the final review.
- **M2-004 obligation:** identityd must call `envelope::public_key_is_valid` on `public_key` and `subject_public_key` before it registers an enrollment or certificate. The reviewer's four certificates with bad subject points pass layers 1–5 by design. The review accepts this assignment only if M2-004 implements it.
- Nothing is merged; the review record is not edited.
