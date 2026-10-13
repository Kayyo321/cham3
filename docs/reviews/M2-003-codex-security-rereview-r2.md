# Independent security re-review: M2-003 revision 2

**Verdict: APPROVE for M2-003.** R01–R04 and I01 are resolved against the owner-amended specification. No new reportable finding was identified. The earlier passing scope, signature, schema, intrinsic-semantics and oracle-fix judgments remain intact. This is an independent exact-revision manual review. The formal Codex Security plugin scan is **BLOCKED**, for the tooling/resource reasons below; its final M2-008 obligation is not waived.

- Reviewer: Codex, independent REVIEWER; did not author this implementation or its resolutions.
- Date: 2026-10-13 UTC.
- Candidate: `8b0ccf37f2437cae0a9668e012ddf55e9952e2aa`, tree `3893e63c985c3ef60b2118438627951d63a5859c`.
- Immutable range: `f87111d021a743b6937dcc5a6c0efab4310f135b..8b0ccf37f2437cae0a9668e012ddf55e9952e2aa` (one implementation commit; 13 modified paths, no deletions).
- Docs successor: `ce83f28a19d1fd46e4ad1c4f61b4c2922ceed6e0`; sole parent is the candidate. **PASS:** one successor, all 44 changed paths under `docs/`.
- Prior review read unchanged: `review/m2-003-security` at `1cb93c0454583311b04fbbb9e90f01e18aab5ca3`, `docs/reviews/M2-003-codex-security-review.md`; SHA-256 `e33e8ad44ca54301a1bb5539074e648d7ba30ea22fa487ea29dd212de7dc1f98`.
- Publication: only this record, `docs/reviews/M2-003-codex-security-rereview-r2.md`, on `review/m2-003-r2-security`, based on `9cf882503848f8c6cc4938a588cc9fc062a9392d`. No branch was merged; no implementation file was edited. Defects were introduced only in disposable copies outside the clean candidate.

## Authority, environment and limits

Read: `AGENTS.md`; relevant PROJECT §§4–5/5.7; workflow, testing, security invariants, roles/orchestration and STATUS; approved ADRs 0003 and 0004; complete amended wire format and CDDL; M2 task/A01–A03; prior review; successor finding resolutions and revision-2 handoff. Author explanations and receipts were checked against code, frozen bytes and independent execution. No owner decision, erratum approval, integration or risk waiver is made here.

Actual machine: managed cloud executor `7c550fe44c1e`, Debian GNU/Linux 13.6 outer container, Linux `6.18.44`, native x86_64/amd64, UID/GID **1000:1000** (`agent`). Clean candidate clone and `.git` owned by that UID. Docker client/server **28.4.0**, vfs storage, default runc and seccomp; local `/var/run/docker.sock` explicitly checked. The unchanged pinned image was rebuilt as `cham3-m2-r2-review`, image ID `sha256:eb2873a830811f67dd4bbcb294b8e9d43426e3917b7da471ef6be98f5c4afc32`. Base index digest: `debian@sha256:913f6706df59a68922d1dd08f78c2476560a8d367897200a6005b00e5f67c2d5`.

Inner environment: Debian 13.7; rustc/Cargo 1.85.1; host `x86_64-unknown-linux-gnu`; rustc commit `4eb161250e340c8f48f66e2b929ef4a5bed7c181`; LLVM 19.1.7; GCC 14.2.0-19; binutils 2.44; glibc 2.41-12+deb13u4. Installed-package and downloaded-.deb locks, compiler identity and per-file crate-source manifest passed qualification. External dependency inventory is byte-identical to the baseline and author receipts (SHA-256 `4c1fbfbcc1021239e8fd39c5057f370c0cc0c72c2f61616cfaedc00bfb8ca44c`).

Network: platform HTTP/HTTPS proxy, restricted egress, no VPN, no TCP grants. The runtime initially reported policy state `unknown`; configured destinations were not presumed usable. Actual GitHub Git fetch/push and Debian package acquisition were tested. The direct `gh` artifact command failed with **Forbidden**; connected GitHub tools retrieved the requested job metadata, selected logs and artifact ZIP. No proxy bypass, TLS weakening, host-protection change, privileged container, added capability or `--record` was used locally. The pinned build uses Debian HTTP sources and authenticated APT metadata; no Dockerfile modification was needed.

**NOT_RUN:** owner's desktop, arm64, coverage-guided fuzzing, Miri, sanitizers, deployment, live services and layers 6–9. The pinned toolchain reports Miri/sanitizers/fuzzing `NOT_AVAILABLE`; this review classifies them **NOT_RUN**, not PASS. Valgrind and bounded mutation evidence do not establish a universal memory-safety or enforcement guarantee. Keys are exclusively published synthetic fixtures.

## Check ledger and dispositions

| Check | Status | Evidence and judgment |
|---|---|---|
| E1 / ADR owner record | PASS | ADR 0004 approval/errata section explicitly attributes 2026-10-13 decision to owner: outside either CDDL list bound is `list_length`. Wire §§6.1/7 and `error.rs` agree; CDDL header points to the error contract. No `list_too_long` remains in the amended normative text. |
| E2 / ADR owner record | PASS | Owner decision records any text other than expected kind's `typ` as `wrong_object_type`, including other versions/unregistered values; non-text is `protected_header_invalid`. Wire §§4.1/8 and CDDL comment match. Rust `envelope.rs:235` and oracle `parse_and_verify` implement this. Earlier decode/header errors still take precedence. |
| E3 / ADR owner record | PASS | Owner decision records map-key comparator for ascending lists, complete deterministic encodings bytewise. Wire §§2.2/6.1/9 and CDDL comments explicitly cover it. `operations`, `max_operations`, constraints use `encoded_order`; fixed-length IDs and grant IDs retain plain byte order. |
| Wire §1 and unchanged types/bytes | PASS | Protected decode precedence is recorded as R01 clarification. After stripping comments and whitespace, CDDL definitions are identical to baseline. Domain strings, field types, sizes, limits, algorithm, envelope, hashes and Sig_structure formats are unchanged. A rejection code and comparator documentation changed; no byte-format migration occurred. |
| R01 | PASS | `envelope.rs:174` decodes protected before kinds at line 175; oracle mirrors this. Own `protected_decode_before_bad_unprotected_kind` and vector `protected_decode_before_element_kinds` give `envelope_decode/non_minimal_encoding`. Original authentication-before-malformed-payload control and further controls pass. |
| R02 / source allocation audit | PASS | No `Vec::with_capacity` remains anywhere in `core/contract_format`. Array push occurs only after successful child decode; map push only after key/class/order and value decode (`dcbor.rs:233`, `:255`, `:279`). Every `collect` was read: value construction/encoding, closed schema-field inventory, lists already decoded/bounded, probe hex input, or tests; none reserves from an undecoded container head. String copying follows successful `take` and UTF-8 check. |
| R02 / decode error ordering | PASS | Only the two decoder initializations changed; no aggregate early truncation shortcut was added. Supplemental short-body cases preserve non-minimal child, duplicate/mixed/invalid key and container-limit errors before later truncation. Map key ordering remains checked before value decode. |
| R02 / Massif harness and sensitivity | PASS | New positive test compares both empty 256-item/pair heads with empty-array peak, requiring growth <1,024 bytes; actual own growth is +4 bytes for both. Sensitivity reintroduces **array** reservation and requires growth ≥1,024. Both tests ran in the 130-test harness. This detects the former 8,192-byte reservation; the automated mutation covers the array path, while source review and own separate map trace verify the map repair. It is a whole-process peak control with stated inputs/slack, not proof of every allocator behavior. |
| R03 | PASS | Unknown/empty/future textual types return `wrong_object_type`; integer/boolean/bytes return `protected_header_invalid`; expected kind works. Published future/non-text vectors and direct Rust controls pass. |
| R04 | PASS | Discriminating `["b","aa"]` accepted, `["aa","b"]` rejected, equal-length increasing accepted, ties/reverse rejected. New valid vector distinguishes operations `["read","write","append"]` and constraints `["tor_only","no_signing_keys"]`; opposite plain-order invalid vectors reject. Existing delegation max-operation list uses the identical comparator. |
| I01 | PASS | Equality vector and Rust regression reject equality and accept one second later. Default differential detects both new seeded equality mutation and preserved reviewer's identical mutation: one mismatch each. Independently signed equality input `1cee2deafc885b1c22fa9761a4b430c391cbf972a755d6a37456b6c6ae54483e`: mutated Rust accepts; oracle reports `intrinsic/lifetime_rule`. |
| Frozen vector delta | PASS | Exactly 7 added: 1 valid and 6 invalid. `typ_unknown` changes `error` and explanatory `description`, with identical envelope bytes. All other 107 previous valid/invalid vector objects are raw-text byte-identical; all other sections equal and absent from the saved diff. No key, constant, lineage, admission/scenario, wrapper-limit or other existing signed byte changed. |
| Rust test changes | PASS | Four added tests: `protected_header_decode_precedes_other_element_kinds`, `text_typ_other_than_expected_is_wrong_object_type`, `text_lists_ascend_in_encoding_order`, `expiration_equal_to_start_is_a_lifetime_rule_violation`. Test helper array indexing/insertion is confined to test code. 64 debug and 64 release tests passed, with no required skips/ignored tests. |
| New seeded defects | PASS | Expiration equality: 1 mismatch; element kinds before protected decode: 1; unknown typ as malformed: 31. Original five controls remain effective; unmodified copy reports zero. Counts are from the preserved reviewer `defects.py`, not merely author claims. |
| Clean pinned amd64 qualification | PASS | Exit 0; all required checks PASS. Unit 64+64; provider oracle 7/7; ECR 19/96/18, 22 scenarios/346 steps; format differential 5,950/0 with 56 distinct outcomes; harness 130. Corpus SHA-256 `080d0c86a6787d7a60e58c86a92717aa7e7ad71c8556c2f43de99b634c43257d`. |
| Static checks | PASS | Exit 0; shellcheck, py_compile, yamllint, scan_secrets PASS. |
| Static tests | PASS | Exit 0; 82 tests in 35.234 seconds, OK. |
| Own unchanged adversarial script | PASS | 270 cases (extra valid vector adds 4 kind controls), 0 Rust/oracle mismatches. Raw script reports 2 stale pre-E1 spec expectations; amended adjudication reports 0 spec mismatches. Its raw JSON is identical to successor's author reproduction. |
| Own extra combined-error script | PASS | 45 cases, 0 Rust/oracle and 0 specification mismatches. Details below. |
| Own unchanged allocations script | PASS | Both minimal empty container heads reject `truncated`; peak heap 17,477 bytes each, empty array 17,473. No decoder allocation frames in either trace. |
| Own unchanged defects script | PASS | Unmodified default corpus PASS/0; eight seeded defects plus independently repeated equality defect all detected. Mutation FAIL means sensitivity PASS; candidate never mutated. |
| Requested CI comparison | PASS | Independently fetched all 1/3/3 jobs and their steps success; checked selected source-identity logs; downloaded ZIP hash matches metadata; local and CI qualification summaries/required results match. |
| Codex Security plugin workflow | BLOCKED | Skill is readable, required preflight resource is not; scan orchestration tools and terminal helper/scripts are absent. No scan identity, formal report/SARIF or plugin findings were produced. See exact attempts below. |
| M2-004 obligation recorded | PASS | Successor handoff, resolutions and task explicitly require identityd to call `envelope::public_key_is_valid` on enrolled `public_key` and certified `subject_public_key` before registration/certification. Four bad subject-point certificates still pass layers 1–5 as the earlier bounded observation; M2-004 implementation is NOT_RUN. |

### Earlier PASS items carried forward

Every item below is **PASS for the earlier stated scope**, using the unchanged-file comparisons, current source reading, repeated 64+64/130 tests and independent adversarial corpus. This does not extend the scope of the earlier conclusions.

| Earlier area | Current evidence |
|---|---|
| Safe Rust / unsafe boundaries | Library/probe forbid unsafe; no added unsafe; unchanged native/provider crates, workspace lint and configurations; qualification lint/unsafe controls pass. |
| Dependencies, versions, lock, source manifest and inventory | No manifest/lock/build/source-pin changes; identical external inventory; all locks and per-file source checks pass. |
| Naming | Current modified-source inspection, rust naming, fmt and clippy pass; existing upstream trait/unittest spellings remain boundaries. |
| Logging | Production library logs no input/key/payload bytes; probe retains static errors/layers/codes and success hash/re-encoding fact. Debug derives are not logging calls. |
| Production signing | No signer/service introduced. Formatting assembles bytes; SigningKey/Signer remains test-only. |
| Authority separation and changed-file scope | Library/resolver docs retain separation from layers 6–9; no service consumer converts a verified object into active authority. All 13 changed paths and supporting paths were inspected. |
| Allowed CBOR items, integer bounds, preferred heads, depth and numeric limits | Preserved decoder rules; own boundary controls and repeated hostile/property tests pass. Protected/payload depth counts remain independent. Forbidden extended simple forms still reject. |
| Map keys / trailing bytes / string-head reading | Encoded key comparator/type class, key-before-value failure, exact top-level trailing check and cap/content/UTF-8 ordering retained. Own short-body combined controls pass; strings allocate after content availability. |
| Header labels, algorithm, unprotected header and signature size | Exact {1,4,16}, −19, fixed kid, empty map and 64 bytes retained; non-integer alg malformed, other integer unsupported. Type error reconciliation is E2 above. |
| Signature path / domain separation | Exact raw protected/payload in Sig_structure, expected-kind AAD, pure Ed25519, canonical public A plus dalek strict S/R checks retained. Original own hostile S, R, resolver-A, wrong-kind/AAD and bad-signature-before-payload cases pass; provider source consulted in pinned image. |
| Closed schema / formats / identity bindings | `schema.rs`, `text.rs`, `hash.rs`, `lib.rs` unchanged; encoded field walk, lists/types, Gregorian ASCII time, grant/actor/subject/lineage/kid/hash bindings retained. New vectors fit the amended error/comparator contract. |
| Message whitespace | Own Unicode edge controls retained: White_Space edges rejected; U+200B/U+2060 accepted as before; forbidden control/bidi rules retained. |
| Layer 5 | `intrinsic.rs` unchanged; type/extra fields, persistent/temporary adds, lifetime strict inequality, delegation subset/depth, per-state network, transition/delta/result and genesis base checks retained. Registry-dependent full state/restore/lineage slot checks remain admission work. |
| Oracle fix | Fullmatch and ASCII patterns remain in place; newline/non-ASCII-digit controls pass in both own corpus and harness. The previous pre-fix 4-mismatch reproduction is historical evidence, not rerun here with the enlarged corpus. |
| Enrolled/certified subject-point boundary | Signer key still validated before verification. Subject-point service obligation recorded; enrollment/certification integration is NOT_RUN. |

## Exact local commands and evidence

All local commands were run as UID/GID 1000, on the machine above. Logs/scripts were saved outside source at `/workspace/m2-r2-evidence`; qualification uses ignored candidate `out/`. This review publishes only this record. SHA-256 identities below identify session observations, not externally available attachments. The prior record contains the unchanged reviewer scripts; supplemental scripts are embedded here for reproducibility.

### Revisions, scope and input checks — PASS

```sh
# Initial repository /workspace/cham3; clean main at 9cf8825
cat AGENTS.md
git status --short
git fetch origin codex/m2-cryptographic-contract-core review/m2-003-security
# Fetch configuration required explicit remote-tracking destinations:
git fetch origin refs/heads/codex/m2-cryptographic-contract-core:refs/remotes/origin/codex/m2-cryptographic-contract-core refs/heads/review/m2-003-security:refs/remotes/origin/review/m2-003-security
git log -5 --oneline origin/codex/m2-cryptographic-contract-core
git clone --no-hardlinks /workspace/cham3 /workspace/m2-r2-candidate
git -C /workspace/m2-r2-candidate checkout --detach 8b0ccf37f2437cae0a9668e012ddf55e9952e2aa
stat -c '%U:%G %u:%g %n' /workspace/m2-r2-candidate /workspace/m2-r2-candidate/.git
git show 1cb93c0:docs/reviews/M2-003-codex-security-review.md > /workspace/m2-r2-evidence/prior-review.md
git show ce83f28:docs/reviews/M2-003-finding-resolutions.md
git show ce83f28:docs/handoffs/M2-003-rereview-r2.md
# /workspace/m2-r2-candidate
git diff --name-status f87111d 8b0ccf3
git diff --name-status 8b0ccf3 ce83f28
git rev-parse 8b0ccf3^{tree} ce83f28^
git rev-list --count 8b0ccf3..ce83f28
git diff --name-only 8b0ccf3 ce83f28 | rg -v '^docs/'
git diff f87111d 8b0ccf3 -- core tests/m2/harness tests/m2/oracle tests/m2/test_inventory.txt tests/m2/vectors/ecr_v1/vectors_readme.md
git diff f87111d 8b0ccf3 -- docs/decisions/0004_m2_wire_schema_hash.md docs/specs
git diff f87111d 8b0ccf3 -- tests/m2/vectors/ecr_v1/vectors.json > /workspace/m2-r2-evidence/vectors.diff
git diff --exit-code f87111d 8b0ccf3 -- Cargo.toml Cargo.lock .cargo build .github PROJECT.md core/native core/crypto_provider core/contractd core/identityd core/vaultd docs/specs/m2_service_ipc_v1.md
git diff --exit-code f87111d 8b0ccf3 -- core/contract_format/src/hash.rs core/contract_format/src/intrinsic.rs core/contract_format/src/lib.rs core/contract_format/src/schema.rs core/contract_format/src/text.rs core/contract_format/src/bin/cham3_format_probe.rs
rg -n 'Vec::with_capacity|collect|reserve' core/contract_format
rg -n 'sign\(|Signer|SigningKey|println!|eprintln!|log::|tracing|verified_object_t' core/contract_format/src
git diff f87111d 8b0ccf3 --unified=0 | rg '^\+.*\bunsafe\b'
rg --files -g AGENTS.md -g SECURITY.md
python3 /workspace/m2-r2-evidence/own/vector_audit.py
git status --porcelain
git diff --exit-code
```

Expected negative searches return rg status 1 (no changed path outside docs; no added unsafe). Exit-code comparisons return 0. Root AGENTS.md is the only local agent policy; no SECURITY.md. The one preliminary remote-tracking log command failed because the explicit refs had not yet been created; the explicit fetch above resolved it without changing revisions.

### Environment, image and qualification — PASS

The cloud runtime skill was applied before Docker/network work. Actual checks:

```sh
pwd
id
uname -a
cat /etc/os-release
cat /etc/codex/network-policy.json
env -u DOCKER_HOST -u DOCKER_CONTEXT -u DOCKER_TLS -u DOCKER_TLS_VERIFY -u DOCKER_CERT_PATH docker --host=unix:///var/run/docker.sock info
# /workspace/m2-r2-candidate
docker build -f build/m2/dev_container.dockerfile -t cham3-m2-r2-review build/m2 > /workspace/m2-r2-evidence/docker_build.log 2>&1
docker image inspect cham3-m2-r2-review --format '{{.Id}} {{.Architecture}} {{.Os}}'
docker run --rm --user "$(id -u):$(id -g)" -v /workspace/m2-r2-candidate:/work -w /work -e CHAM3_M2_REQUIRE_CLEAN=1 cham3-m2-r2-review bash build/scripts/qualify_m2_rust.sh > /workspace/m2-r2-evidence/qualification_console.log 2>&1
```

Source identity names candidate, `dirty_tracked 0`, EUID 1000, same Cargo.lock digest. Fresh targets are used by qualification and suites. Required checks: toolchain/packages/.deb and installed-set locks; offline metadata/source inventory; fmt/clippy/naming/profiles; debug/release builds; software and fiat backends; Valgrind probe and secure-memory controls; unit and harness suites. No required check is counted from stale evidence.

### Static gates — PASS / PASS

```sh
# /workspace/m2-r2-candidate; after qualification
 docker run --rm --user "$(id -u):$(id -g)" -v /workspace/m2-r2-candidate:/work -w /work cham3-m2-r2-review bash build/scripts/run_static_checks.sh > /workspace/m2-r2-evidence/static_checks.log 2>&1
 docker run --rm --user "$(id -u):$(id -g)" -v /workspace/m2-r2-candidate:/work -w /work cham3-m2-r2-review sh tests/run_tests.sh static > /workspace/m2-r2-evidence/static_tests.log 2>&1
```

Both exit 0: shellcheck, Python compilation, YAML lint and secret scanner PASS; 82 static tests, zero failures. Candidate remains clean.

### Preserved own scripts and further combined errors — PASS

Scripts were extracted verbatim from the frozen prior record, including final newline, with this command:

```python
from pathlib import Path
root = Path('/workspace/m2-r2-evidence')
own = root / 'own'
own.mkdir(exist_ok=True)
text = (root / 'prior-review.md').read_text()
for name in ['adversarial.py', 'allocations.py', 'defects.py', 'oracle_reproduction.py']:
    section = text.split('### ' + name + '\n', 1)[1]
    code = section.split('```python\n', 1)[1].split('```', 1)[0]
    (own / name).write_text(code)
```

The first three SHA-256 digests match the prior review: adversarial `8d04c0e3239860f66afbd02acde393f557091038442b1c7dded8f74a7b143e44`; allocations `ed3992097b2b4b09fa7a0fa516507cbe58a58e2aebbfb5fab9eb2ce79e96c566`; defects `06cbf1f86988147a907e8c612fc1d45aefa41260d16b3d871cf26df5b53e15ba`. The old oracle_reproduction script was extracted for reference but **NOT_RUN**; its old exact pre-fix corpus is a different target.

```sh
# /workspace/m2-r2-candidate; all own mounts keep implementation read-only
docker run --rm --user "$(id -u):$(id -g)" -v /workspace/m2-r2-candidate:/work:ro -v /workspace/m2-r2-evidence:/evidence -w /work cham3-m2-r2-review env CARGO_TARGET_DIR=/evidence/own/target cargo build --frozen -p cham3_contract_format --bin cham3_format_probe > /workspace/m2-r2-evidence/own/build_probe.log 2>&1
docker run --rm --user "$(id -u):$(id -g)" -v /workspace/m2-r2-candidate:/work:ro -v /workspace/m2-r2-evidence:/evidence -w /work cham3-m2-r2-review python3 /evidence/own/adversarial.py /evidence/own/target/debug/cham3_format_probe /evidence/own/results.json > /workspace/m2-r2-evidence/own/adversarial_console.log 2>&1
docker run --rm --user "$(id -u):$(id -g)" -v /workspace/m2-r2-candidate:/work:ro -v /workspace/m2-r2-evidence:/evidence -w /work cham3-m2-r2-review python3 /evidence/own/allocations.py > /workspace/m2-r2-evidence/own/allocations_console.log 2>&1
docker run --rm --user "$(id -u):$(id -g)" -v /workspace/m2-r2-candidate:/work:ro -v /workspace/m2-r2-evidence:/evidence -w /work cham3-m2-r2-review python3 /evidence/own/defects.py > /workspace/m2-r2-evidence/own/defects_console.log 2>&1
docker run --rm --user "$(id -u):$(id -g)" -v /workspace/m2-r2-candidate:/work:ro -v /workspace/m2-r2-evidence:/evidence -w /work cham3-m2-r2-review python3 /evidence/own/combined_errors.py /evidence/own/target/debug/cham3_format_probe /evidence/own/combined_results.json > /workspace/m2-r2-evidence/own/combined_console.log 2>&1
docker run --rm --user "$(id -u):$(id -g)" -v /workspace/m2-r2-evidence:/evidence cham3-m2-r2-review python3 /evidence/own/judge_amended.py > /workspace/m2-r2-evidence/own/amended_console.log 2>&1
docker run --rm --user "$(id -u):$(id -g)" -v /workspace/m2-r2-evidence:/evidence cham3-m2-r2-review bash -c 'printf "verify commit 80\n" | valgrind --tool=massif --time-unit=B --detailed-freq=1 --max-snapshots=1000 --threshold=0.0 --massif-out-file=/evidence/own/massif_empty_array.txt /evidence/own/target/debug/cham3_format_probe' > /workspace/m2-r2-evidence/own/allocation_empty_array.log 2>&1
docker run --rm --user "$(id -u):$(id -g)" cham3-m2-r2-review bash -c 'sed -n "35,105p" /usr/share/cargo/registry/ed25519-dalek-2.1.1/src/signature.rs; sed -n "385,445p" /usr/share/cargo/registry/ed25519-dalek-2.1.1/src/verifying.rs' > /workspace/m2-r2-evidence/strict_provider_source.log 2>&1
```

A preliminary provider-source `rg` locator was **BLOCKED** because rg is absent inside the pinned image; the sed source read above succeeded without modifying its packages. The fixed provider source still enforces canonical scalar conversion and rejects small-order points, then compares recomputed canonical R. Outer workspace rg was available for all repository searches.

Additional combined-error coverage (45/45 PASS): nine protected decode failures (non-minimal, truncated, trailing, indefinite, reserved, forbidden tag, UTF-8, duplicate key, depth) each combined with bad unprotected/payload/signature kinds and separately with nonempty map/short signature/bad payload: 36 cases. Remaining 9: signature before malformed payload, unknown key before malformed payload, outer trailing before protected decode, wrong arity prevents protected extraction, and the five short-body decoder cases below. Outer decode still completes first; no unsupported precedence is claimed between two unfinished decodes.

| Short-body input | Expected/actual first error |
|---|---|
| `9901001800` | `envelope_decode/non_minimal_encoding` |
| `b90100000000` | `envelope_decode/unsorted_or_duplicate_key` |
| `b90100f4` | `envelope_decode/map_key_type` |
| `b9010000006178` | `envelope_decode/map_key_type` |
| `9901011800` | `envelope_decode/container_too_large` |

Preserved script adjudication: the two `list_too_long` expectations are E1's `list_length`; E3 resolves three formerly unspecified list-order cases; empty operations is `list_length`; extended-simple and bad-subject-point cases preserve the earlier bounded readings. Four bad subject-point acceptances are layers 1–5 boundary observations only, not successful enrollment/certification. The amended table has zero mismatches.

| Disposable control | Differential result | Review status |
|---|---|---|
| Unmodified | PASS / 0 | PASS |
| Non-minimal heads accepted | FAIL / 4 | PASS detected |
| Duplicate keys accepted | FAIL / 1 | PASS detected |
| Commit kid binding removed | FAIL / 3 | PASS detected |
| Second network grant accepted | FAIL / 3 | PASS detected |
| Bidi controls accepted | FAIL / 1 | PASS detected |
| Expiration equality accepted (new seeded) | FAIL / 1 | PASS detected |
| Other kinds checked before protected decode (new seeded) | FAIL / 1 | PASS detected |
| Unknown typ reported as malformed (new seeded) | FAIL / 31 | PASS detected |
| Preserved own expiration equality defect | FAIL / 1, own signed case also detects | PASS detected |

## CI comparison — PASS; direct CLI BLOCKED

The exact requested CLI attempt:

```sh
gh run download 38327808145 --repo Kayyo321/cham3 -n m2_qualification_amd64 -D /workspace/m2-r2-evidence/ci_artifact > /workspace/m2-r2-evidence/gh_download.log 2>&1
```

It returned `error fetching artifacts: Get https://api.github.com/repos/Kayyo321/cham3/actions/runs/38327808145/artifacts?per_page=100: Forbidden`. No success was inferred from that failure. Successful actual connector calls (tool names without the `mcp__codex_apps__` prefix for readability):

```text
github_fetch_workflow_run_jobs(repo_full_name="Kayyo321/cham3", run_id=38327808145)
github_fetch_workflow_run_jobs(repo_full_name="Kayyo321/cham3", run_id=38327808083)
github_fetch_workflow_run_jobs(repo_full_name="Kayyo321/cham3", run_id=38327808082)
github_fetch_workflow_run_artifacts(repo_full_name="Kayyo321/cham3", run_id=38327808145, name="m2_qualification_amd64")
github_fetch_workflow_job_logs(repo_full_name="Kayyo321/cham3", job_id=115070439635)
github_fetch_workflow_job_logs(repo_full_name="Kayyo321/cham3", job_id=115070439174)
github_fetch_workflow_job_logs(repo_full_name="Kayyo321/cham3", job_id=115070439367)
github_download_workflow_artifact(repo_full_name="Kayyo321/cham3", artifact_id=11782306047, file_name="m2_qualification_amd64.zip")
download_file(file_id="file_0000000028dc81f5918f9c14fdc9c153")
```

| Run | Status | Independent evidence |
|---|---|---|
| m2_checks 38327808145 | PASS | qualify_amd64 job 115070439635; every returned step success; logs/ZIP source identity candidate. |
| m1_checks 38327808083 | PASS | static_and_desktop 115070439174, privileged_capture 115070439420, package_build 115070439495, every returned step success; static job checkout log candidate. |
| m0_checks 38327808082 | PASS | static_checks 115070439223, image_build 115070439367, package_build 115070439518, every returned step success; image job checkout log candidate. |

These are CI observations. The M0/M1 full image/VM/package/privileged suites were **NOT_RUN locally**. Job metadata returns first page/latest attempt; there are 1/3/3 jobs and no omitted-page indication. Selected logs establish source identity; no claim is made that every full job log was downloaded.

ZIP: artifact 11782306047, 90,097 bytes, downloaded SHA-256 **`0e77977e49149b51d95b2eec23e5a38f2adc51122437ff6f931fa42bf66d0eb8`**, identical to API metadata. File `/workspace/attachments/ba504004-9573-4959-a7ff-3d949e95bd7e/m2_qualification_amd64.zip` was read with Python zipfile; extracted under `/workspace/m2-r2-evidence/ci_artifact`.

PASS: local, CI and successor author qualification summaries are byte-identical. Unit counts 64+64, provider answers 7/7, ECR 19/96/18 with 22/346, harness 130, default differential cases 5,950/mismatches 0, 56 distinct outcomes, seed/corpus/coverage agree. Format probe hashes happen to match; general binary reproducibility is not claimed. CI uses `--record` (visible in job log); local command did not. Both installed-set/.deb locks pass; `--record` did not waive missing locks.

```sh
# /workspace/m2-r2-candidate
python3 /workspace/m2-r2-evidence/own/compare_ci.py > /workspace/m2-r2-evidence/ci_comparison.log
```


## Security plugin — BLOCKED

Exact skill: **`codex-security:security-diff-scan`**, cloud package `c0/security-diff-scan`, resolved resource `skill://Plugin_1e648473be9c8191a91ac3947151af55/security-diff-scan/SKILL.md`. **Version: not exposed/unavailable** (catalog and SKILL frontmatter supply name/description, no version; no installed local plugin manifest found). The skill was read and its setup attempted for the unchanged `f87111d..8b0ccf3` range. It requires the `security_diff_scan` capability preflight before scan creation/review orchestration. This host has no callable `start_codex_security_prompt_only_scan`, config-preflight or other Codex Security scan tool, and no terminal launcher/generator/finalizer installation.

Actual attempts:

```text
skills.read(package="c0/security-diff-scan")                                  # PASS: SKILL.md read
skills.read(package="c0/security-diff-scan", resource="skill://Plugin_1e648473be9c8191a91ac3947151af55/references/config-preflight.md")
skills.read(package="c0/security-diff-scan", resource="skill://Plugin_1e648473be9c8191a91ac3947151af55/security-diff-scan/../../references/config-preflight.md")
# Both references: failed to read skill resource
ALL_TOOLS.filter(x => /codex_security|preflight/.test(x.name))                 # []
```

```sh
rg --files /opt /home/agent /usr/local/share 2>/dev/null | rg 'codex.security|config-preflight|launch_codex_security' | head -50
rg --files /home/agent /opt /usr/local /workspace 2>/dev/null | rg '(SKILL.md|plugin.json|config-preflight|launch_codex_security|generate_in_scope_files)' | head -60
rg --files --hidden /home/agent /opt /usr/local /workspace 2>/dev/null | rg 'config-preflight|launch_codex_security|generate_in_scope_files|finalize_scan_contract|codex.security|plugin.json' | head -60
```

Matches were unrelated built-in runtime skills/manifests and review filenames; no Codex Security installation, required helper or version manifest, including hidden directories. No scan was silently replaced or called complete. **Plugin findings: NOT_RUN/unavailable**, not “zero findings.” Manual review found no new reportable security finding; manual evidence does not count as a completed plugin scan. The required final M2-008 Codex Security plugin review remains open in the successor task/handoff and this record.

## Evidence identities and reproducibility appendices

| Session evidence | SHA-256 |
|---|---|
| `/workspace/m2-r2-evidence/prior-review.md` | `e33e8ad44ca54301a1bb5539074e648d7ba30ea22fa487ea29dd212de7dc1f98` |
| `/workspace/m2-r2-evidence/docker_build.log` | `14713137286fddb02d75fe153df5d7ed82ac7f0b4855abcd57081bc4da1a2d0e` |
| `/workspace/m2-r2-evidence/qualification_console.log` | `5c2015c9e9bbd35db6f691ffc457d18288d2eb9a518f5a128629c0b33e77964e` |
| `/workspace/m2-r2-evidence/static_checks.log` | `f200c67153a815b764a0893d509b3ddb6ec8e1ac6836aef2333389b4c62d69e5` |
| `/workspace/m2-r2-evidence/static_tests.log` | `3e43241de3a2ad8b54cccab19de774a4ffe8e778c4473e759060d88222ab8a6b` |
| `/workspace/m2-r2-evidence/vector_audit.json` | `967cfa2fc170f979bff6586d09a98c1bf548bb79bbf32858db4b9fd2d01b5eeb` |
| `/workspace/m2-r2-evidence/vectors.diff` | `bc9bcdea63f041bd8a2ab676aa6e9963a109c4656a629f62e5174e64b3e28064` |
| `/workspace/m2-r2-evidence/candidate_paths.txt` | `eb43e3fee370ec16619719d3548bdab9c7d41e2fcbd0cdcb1a8d94c630ada5c5` |
| `/workspace/m2-r2-evidence/docs_paths.txt` | `c891e9aab40146f53ffd929985b1e60ed20c04e53459ef748270505163fbb80d` |
| `/workspace/m2-r2-evidence/ci_comparison.log` | `19569300d183c63a3942c4583e2fa00e767abdbe58bb69c11c9a907f060e2d9c` |
| `/workspace/m2-r2-evidence/gh_download.log` | `6ff2e4185e38784ff563577c728da99f7c2d600936e6256372942dae8e62cbd6` |
| `/workspace/m2-r2-evidence/ci_metadata_0.json` | `baaedf5c8132ed32fe2588c2a13ab732938da48c89abd44154048f5a3f7a9398` |
| `/workspace/m2-r2-evidence/ci_metadata_1.json` | `a43334e244ae653291f8dd8d33c604378de85e64ea95b3e1fbdc2ddd2faa0891` |
| `/workspace/m2-r2-evidence/ci_metadata_2.json` | `5c5fcbe95741c57285b17f3250a62be04a0d4fcc1458e3069856873c00d29964` |
| `/workspace/m2-r2-evidence/ci_job_0_excerpt.log` | `7874293d2fac464d77d6b5d731126e9ad246d1a60591f5eccc943f56379e1e94` |
| `/workspace/m2-r2-evidence/ci_job_1_excerpt.log` | `6b51e99edf1c9b5b371d4aebdd32d6a686bd1c7abc657390b57716526806aa1a` |
| `/workspace/m2-r2-evidence/ci_job_2_excerpt.log` | `a16c8b98d7c2b9c7508611529424e687f870a992c1fca238f1050f6b076d67ce` |
| `/workspace/m2-r2-evidence/strict_provider_source.log` | `79ab6b7d7946398fa45d57493923376d612e7e563f6e5a9e996c6835d802a03a` |
| `/workspace/m2-r2-evidence/own/results.json` | `d534b9158801ae932b5665ecba0321931574f8e0288d8a7b876f72c13fb448a8` |
| `/workspace/m2-r2-evidence/own/amended_results.json` | `82de507d9487dff02f438b779d827fd5be0d83d5194855a329502502c09d29ba` |
| `/workspace/m2-r2-evidence/own/combined_results.json` | `570ac9d0f0b565622603cf478a865ae09819672fec04e838d31523b9055c6cc5` |
| `/workspace/m2-r2-evidence/own/defects.json` | `e6dd86c08da1200fbe50456b46490070d3e63848751abcd47a9ab268665301b8` |
| `/workspace/m2-r2-evidence/own/massif_array_256_no_items.txt` | `b75738fb22065799a9769ada18dfe72d0afda5baea59b862a41a97b8b9ff2ed8` |
| `/workspace/m2-r2-evidence/own/massif_map_256_no_pairs.txt` | `e42e276703f5a59438ed878efb155978f2658560f9b912608779ae7628e975cb` |
| `/workspace/m2-r2-evidence/own/massif_empty_array.txt` | `4f6ab64370d26c34e9308e605b360e0653c2f2bdd88dfa58832858c3bef50deb` |
| `/workspace/m2-r2-candidate/out/m2/qualification/amd64_20261013T012837Z_1/summary.txt` | `e6066c067941945f43add6f8d20135e1afd808587ffca46f1e8871509300507a` |
| `/workspace/m2-r2-candidate/out/m2/qualification/amd64_20261013T012837Z_1/identity.txt` | `5fc002622dcf36cc9bf824de0c62c504b9f37bd2b7ac98311c68984bf10ceb67` |
| `/workspace/m2-r2-candidate/out/m2/qualification/amd64_20261013T012837Z_1/toolchain.txt` | `ce712cc31413efb60cd80eefce7050f88e146d5a981054c661f7b8d093bac357` |
| `/workspace/m2-r2-candidate/out/m2/qualification/amd64_20261013T012837Z_1/dependency_inventory.md` | `4c1fbfbcc1021239e8fd39c5057f370c0cc0c72c2f61616cfaedc00bfb8ca44c` |
| `/workspace/m2-r2-candidate/out/m2/runs/unit_20261013T012923Z_1658/cargo_test_debug.json` | `ce8915cc1e8bc23fca5b0e2ba991d36e6fa505894179aabcc113714a0406c39a` |
| `/workspace/m2-r2-candidate/out/m2/runs/unit_20261013T012923Z_1658/cargo_test_release.json` | `340a6d5869469fa0fe196b9cb1b96c5afdfd51fac1f6d0554e5013183048b455` |
| `/workspace/m2-r2-candidate/out/m2/runs/unit_20261013T012923Z_1658/provider_oracle.json` | `f524a7b8bdc2e7bff8fa007c90a495662a99a7b2323f6924164c93147e42f648` |
| `/workspace/m2-r2-candidate/out/m2/runs/unit_20261013T012923Z_1658/ecr_vector_oracle.log` | `99c661881019223682dbf595780c7e4f24d09b2b4faba5194ab3b352db4a190a` |
| `/workspace/m2-r2-candidate/out/m2/runs/unit_20261013T012923Z_1658/format_differential.json` | `bdf7ec6331c20e70634abea2d95d99abbc3d2d800a85c02987f4c1e62cdc4519` |
| `/workspace/m2-r2-candidate/out/m2/runs/harness_20261013T013120Z_3105/python_harness.json` | `bc89f16c1213b9f4edf0a2eab874d0be9981a6ae822ef9538e9c30b54a9bc1d0` |
| `/workspace/m2-r2-candidate/out/m2/runs/harness_20261013T013120Z_3105/python_harness.log` | `cad21e5ebc0f7cecd16accabd2efc47bc758dffa9adb5772626c771b5fffdd4b` |


### Supplemental scripts

#### vector_audit.py

SHA-256: `d62bdb8fc5584d4e00db3a47e450307252e717a7e08e697f8f21dde082b615ff`.

```python
"""Exact frozen-vector and CDDL comparison for M2-003 r2."""
import hashlib
import json
import re
import subprocess
from pathlib import Path

def git_file(revision, path):
    return subprocess.check_output(['git', 'show', revision + ':' + path], text=True)

path = 'tests/m2/vectors/ecr_v1/vectors.json'
old_text = git_file('f87111d', path)
new_text = git_file('8b0ccf3', path)
old = json.loads(old_text); new = json.loads(new_text)
decoder = json.JSONDecoder()
def raw_vector(text, section, name):
    start = text.index('"' + section + '": [')
    pos = text.index('"name": "' + name + '"', start)
    start = text.rfind('{', start, pos)
    value, end = decoder.raw_decode(text[start:])
    assert value['name'] == name
    return text[start:start + end]

additions = {}
unchanged = 0
for section in old:
    if section in ['valid', 'invalid']:
        a = {v['name']: v for v in old[section]}; b = {v['name']: v for v in new[section]}
        assert not a.keys() - b.keys()
        additions[section] = sorted(b.keys() - a.keys())
        for name, vector in a.items():
            if name == 'typ_unknown':
                assert vector['envelope'] == b[name]['envelope']
                changed = {key for key in vector.keys() | b[name].keys() if vector.get(key) != b[name].get(key)}
                assert changed == {'error', 'description'}
                assert b[name]['error'] == 'wrong_object_type'
            else:
                assert vector == b[name]
                assert raw_vector(old_text, section, name) == raw_vector(new_text, section, name)
                unchanged += 1
    else:
        assert old[section] == new[section], section
        # Their unchanged raw bytes are also visible in the saved full git diff.
assert sum(map(len, additions.values())) == 7
cddl = 'docs/specs/m2_ecr_v1.cddl'
strip = lambda text: [line.split(';', 1)[0].strip() for line in text.splitlines() if line.split(';', 1)[0].strip()]
assert strip(git_file('f87111d', cddl)) == strip(git_file('8b0ccf3', cddl))
result = dict(additions=additions, old_valid_invalid_vectors_byte_identical=unchanged,
              typ_unknown_changed_fields=['error', 'description'], typ_unknown_envelope_unchanged=True,
              all_other_sections_equal=True, cddl_types_unchanged=True,
              vectors_sha256=hashlib.sha256(new_text.encode()).hexdigest())
Path('/workspace/m2-r2-evidence/vector_audit.json').write_text(json.dumps(result, indent=2))
print(json.dumps(result, indent=2))
```

#### combined_errors.py

SHA-256: `c7068a88c60b4427c1fda757b67ca91597a2c4b132dbe8a41d1bbbbac643aba6`.

```python
"""Additional first-error controls; reuses only the reviewer's encoder/key setup."""
from pathlib import Path
exec(Path('/evidence/own/adversarial.py').read_text().split("add('positive_genesis'", 1)[0])

bad_headers = [
    (b'\x18\x00', 'non_minimal_encoding'),
    (b'\xa3', 'truncated'),
    (b'\xa0\x00', 'trailing_bytes'),
    (b'\xff', 'indefinite_length'),
    (b'\x1c', 'reserved_additional_info'),
    (b'\xc0\x00', 'tag_forbidden'),
    (b'\x61\xff', 'invalid_utf8'),
    (b'\xa2\x00\x00\x00\x00', 'unsorted_or_duplicate_key'),
    (b'\x81' * 9 + b'\x00', 'depth_exceeded'),
]
for raw, code in bad_headers:
    for index, wrong in [(1, False), (2, 'payload'), (3, 0)]:
        elements = [raw, {}, b'\xff', b'\x00' * 64]
        elements[index] = wrong
        add('protected_%s_before_bad_element_%d' % (code, index), b'\xd2' + encode(elements),
            'err envelope_decode ' + code)
    add('protected_%s_before_nonempty_map_and_short_sig' % code,
        envelope(raw, b'\xff', b'\x00', {1: 0}), 'err envelope_decode ' + code)
protected = encode(header)
add('valid_header_bad_signature_before_nonminimal_payload',
    envelope(protected, b'\x18\x00', b'\x00' * 64), 'err signature signature_invalid')
unknown_header = dict(header); unknown_header[4] = b'\x00' * 32
add('unknown_key_before_bad_payload', envelope(encode(unknown_header), b'\xff', b'\x00' * 64),
    'err signature unknown_key')
add('outer_trailing_before_protected_nonminimal',
    envelope(b'\x18\x00', b'\xff', b'\x00' * 64) + b'\x00', 'err envelope_decode trailing_bytes')
add('wrong_arity_prevents_protected_decode', b'\xd2' + encode([b'\x18\x00', {}, b'']),
    'err envelope not_signed_object')
for raw, code in [(b'\x99\x01\x00\x18\x00', 'non_minimal_encoding'),
                  (b'\xb9\x01\x00\x00\x00\x00', 'unsorted_or_duplicate_key'),
                  (b'\xb9\x01\x00\xf4', 'map_key_type'),
                  (b'\xb9\x01\x00\x00\x00\x61x', 'map_key_type'),
                  (b'\x99\x01\x01\x18\x00', 'container_too_large')]:
    add('short_container_first_error_' + raw.hex(), raw, 'err envelope_decode ' + code)

lines = ['key %s %s' % (k.hex(), p.hex()) for k, (p, _) in keys.items()]
lines += ['verify %s %s' % (case['kind'], case['data'].hex()) for case in cases]
done = subprocess.run([sys.argv[1]], input='\n'.join(lines) + '\n', text=True, capture_output=True, check=True)
answers = done.stdout.splitlines(); assert len(answers) == len(cases)
records = []
for case, answer in zip(cases, answers):
    try:
        oracle.verify(case['data'], case['kind'], resolver); oracle_answer = 'ok'
    except oracle.oracle_error_t as error:
        oracle_answer = 'err %s %s' % (error.layer, error.code)
    normalized = 'ok' if answer.startswith('ok ') else answer
    records.append(dict(name=case['name'], input_sha256=hashlib.sha256(case['data']).hexdigest(),
                        expected=case['spec_expected'], rust=normalized, oracle=oracle_answer))
result = dict(cases=len(records), rust_oracle_mismatches=sum(r['rust'] != r['oracle'] for r in records),
              spec_mismatches=sum(r['rust'] != r['expected'] for r in records), records=records)
Path(sys.argv[2]).write_text(json.dumps(result, indent=2))
print(json.dumps({k: v for k, v in result.items() if k != 'records'}))
assert result['rust_oracle_mismatches'] == result['spec_mismatches'] == 0, result
```

#### judge_amended.py

SHA-256: `7f70825f73a5442ae4ada6c813d60352686a00f6b456c04a372178deb46eb52a`.

```python
"""Judge preserved reviewer-script results under owner errata E1-E3."""
import json
from pathlib import Path
path = Path('/evidence/own/results.json')
data = json.loads(path.read_text())
for row in data['records']:
    expected = row['spec_expected']
    if expected == 'err schema list_too_long':
        expected = 'err schema list_length'
    if row['name'].startswith('label_order_'):
        expected = 'ok' if row['name'] in ["label_order_['b', 'aa']", "label_order_['aa', 'ab']"] else 'err schema list_not_strictly_ascending'
    if row['name'] == 'operation_ties_or_length_0':
        expected = 'err schema list_length'
    if row['name'].startswith('nonpreferred_simple_'):
        expected = 'err payload_decode simple_value_forbidden'
    if row['name'].startswith('certificate_bad_subject_point_'):
        expected = 'ok'  # Observed layers 1-5 boundary, not M2-004 key validation.
    assert expected is not None, row
    row['amended_expected'] = expected
    row['amended_match'] = row['rust'] == expected
data['amended_spec_mismatches'] = sum(not row['amended_match'] for row in data['records'])
Path('/evidence/own/amended_results.json').write_text(json.dumps(data, indent=2))
print(json.dumps({k: v for k, v in data.items() if k != 'records'}))
assert data['rust_oracle_mismatches'] == data['amended_spec_mismatches'] == 0
```

#### compare_ci.py

SHA-256: `3f0f96044726611168588f4b74db5b7fbb35a1d1c7d734173569f079c9ed8ebb`.

```python
from pathlib import Path
import json
import subprocess

local = Path('/workspace/m2-r2-candidate/out/m2')
ci = Path('/workspace/m2-r2-evidence/ci_artifact')
local_summary = next(local.glob('qualification/*/summary.txt'))
ci_summary = next(ci.glob('qualification/*/summary.txt'))
assert local_summary.read_bytes() == ci_summary.read_bytes()
author = subprocess.check_output(['git', 'show', 'ce83f28:docs/evidence/M2-cryptographic-contract-core/m2_003_amd64_vm_8b0ccf3/summary.txt'])
assert local_summary.read_bytes() == author
print('qualification summaries local/CI/author byte-identical')
for name in ['cargo_test_debug.json', 'cargo_test_release.json', 'provider_oracle.json', 'format_differential.json', 'python_harness.json']:
    a = json.loads(next(local.glob('runs/*/' + name)).read_text())
    b = json.loads(next(ci.glob('runs/*/' + name)).read_text())
    if name == 'format_differential.json':
        for key in ['result', 'total_tests', 'cases', 'cases_per_object', 'seed', 'mismatch_count', 'distinct_outcomes', 'corpus_sha256', 'coverage']:
            assert a.get(key) == b.get(key), (name, key)
    elif name == 'provider_oracle.json':
        for key in ['result', 'probe_exit_status', 'oracle_exit_status', 'oracle_output', 'probe_output']:
            assert a[key] == b[key], key
    else:
        for key in ['result', 'total_tests', 'exit_status']:
            assert a[key] == b[key], (name, key)
    print(name, 'matching results', a['result'], a.get('total_tests', a.get('cases')))
assert next(local.glob('runs/unit_*/ecr_vector_oracle.log')).read_bytes() == next(ci.glob('runs/unit_*/ecr_vector_oracle.log')).read_bytes()
print('ECR oracle outputs byte-identical')
```


## Owner/integrator boundary

M2-003 recommendation is **APPROVE** for the reviewed layers 1–5 candidate plus documentation successor. No remaining R01–R04/I01 action is requested. No new finding. Owner retains all architecture, completion and merge decisions. M2-004 must enforce enrolled/certified-key validation, and final M2-008 must complete the plugin review. Neither obligation is declared satisfied by this record. Source trees are clean apart from ignored test output; publication contains exactly this review record.

Publication checks — **PASS**: the branch's single added path is this record; candidate and original main remain clean, no merges. The local clone's origin is the original workspace clone, so publication uses the explicit GitHub URL:

```sh
# Worktree created from /workspace/m2-r2-candidate, based on main only
git worktree add -b review/m2-003-r2-security /workspace/m2-r2-review 9cf882503848f8c6cc4938a588cc9fc062a9392d
# /workspace/m2-r2-review
git ls-remote https://github.com/Kayyo321/cham3.git refs/heads/review/m2-003-r2-security
git diff --no-index --check /dev/null docs/reviews/M2-003-codex-security-rereview-r2.md
git add docs/reviews/M2-003-codex-security-rereview-r2.md
git diff --cached --check
git diff --cached --name-status
git -c user.name=Codex -c user.email=codex@openai.com commit -m "docs(review): publish independent M2-003 revision-2 security review"
git push https://github.com/Kayyo321/cham3.git HEAD:refs/heads/review/m2-003-r2-security
git ls-remote https://github.com/Kayyo321/cham3.git refs/heads/review/m2-003-r2-security
git diff --name-status 9cf882503848f8c6cc4938a588cc9fc062a9392d HEAD
sha256sum docs/reviews/M2-003-codex-security-rereview-r2.md
git status --porcelain
git -C /workspace/m2-r2-candidate status --porcelain
git -C /workspace/cham3 status --porcelain
```

The publication commit and record digest are returned in the final verdict block; they are deliberately not embedded in their own hashed file.
